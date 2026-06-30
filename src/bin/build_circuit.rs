//! Untrusted build stage.
//!
//! Builds the contestant 5-bit Shor ECDLP circuit and streams the primitive op
//! artifact to `ops.bin`. The trusted `eval_circuit` binary re-loads this file
//! in a separate process and does all validation/scoring.

use ecdlp_5_bit::ops_io::{ArtifactFormat, OpsArtifactWriter, OP_BYTES};
use ecdlp_5_bit::shor_oracle;
use std::path::Path;

const OPS_PATH: &str = "ops.bin";

fn main() {
    println!("=== ecdlp_5_bit: build_circuit (untrusted stage) ===\n");
    println!("-- building circuit --");

    let format = ArtifactFormat::from_env();
    let path = Path::new(OPS_PATH);
    let writer = match OpsArtifactWriter::create(path, format) {
        Ok(writer) => writer,
        Err(error) => {
            eprintln!("error: failed to create {OPS_PATH}: {error}");
            std::process::exit(2);
        }
    };

    let writer = shor_oracle::build_into(writer);
    let stats = writer.stats();
    println!("  emitted ops : {}", stats.ops);
    println!("  static CCX  : {}", stats.ccx);
    println!("  static CCZ  : {}", stats.ccz);
    println!("  qubits      : {}", stats.qubits());
    println!("  format      : {}", writer.format().label());

    let format = writer.format();
    if let Err(error) = writer.finish() {
        eprintln!("error: failed to write {OPS_PATH}: {error}");
        std::process::exit(2);
    }

    let bytes = std::fs::metadata(path)
        .map(|m| m.len())
        .unwrap_or_else(|_| {
            if format == ArtifactFormat::Raw {
                stats.ops * OP_BYTES as u64 + 16
            } else {
                0
            }
        });
    println!("  wrote       : {} ({} bytes)", OPS_PATH, bytes);
    println!("\n=== build_circuit OK ===");
}
