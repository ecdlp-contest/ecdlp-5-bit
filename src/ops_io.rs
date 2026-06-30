use crate::circuit::{BitId, Op, OperationType, QubitId, RegisterId, NO_QUBIT};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const RAW_MAGIC: &[u8; 8] = b"QECCOPS1";
pub const ZSTD_MAGIC: &[u8; 8] = b"QECCOPSZ";
pub const OP_BYTES: usize = 56;
pub const MAX_OPS: u64 = 4_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactFormat {
    Raw,
    Zstd,
}

impl ArtifactFormat {
    pub fn from_env() -> Self {
        match std::env::var("QECCOPS_FORMAT")
            .or_else(|_| std::env::var("QECC_OPS_FORMAT"))
            .ok()
            .as_deref()
        {
            Some("raw") | Some("RAW") | Some("QECCOPS1") => Self::Raw,
            _ => Self::Zstd,
        }
    }

    pub fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::Raw => RAW_MAGIC,
            Self::Zstd => ZSTD_MAGIC,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Raw => "QECCOPS1",
            Self::Zstd => "QECCOPSZ",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OpStats {
    pub ops: u64,
    pub ccx: u64,
    pub ccz: u64,
    pub max_qubit: Option<u64>,
}

impl OpStats {
    pub fn observe(&mut self, op: &Op) {
        self.ops += 1;
        match op.kind {
            OperationType::CCX => self.ccx += 1,
            OperationType::CCZ => self.ccz += 1,
            _ => {}
        }
        for qubit in [op.q_control2, op.q_control1, op.q_target] {
            if qubit != NO_QUBIT {
                self.max_qubit = Some(self.max_qubit.map_or(qubit.0, |max| max.max(qubit.0)));
            }
        }
    }

    pub fn qubits(self) -> u64 {
        self.max_qubit.map_or(0, |max| max + 1)
    }
}

pub trait OpSink {
    fn push_op(&mut self, op: Op);
}

#[derive(Default)]
pub struct VecOpSink {
    ops: Vec<Op>,
    stats: OpStats,
}

impl VecOpSink {
    pub fn into_ops(self) -> Vec<Op> {
        self.ops
    }

    pub fn stats(&self) -> OpStats {
        self.stats
    }
}

impl OpSink for VecOpSink {
    fn push_op(&mut self, op: Op) {
        self.stats.observe(&op);
        self.ops.push(op);
    }
}

enum ArtifactWriterInner {
    Raw(BufWriter<File>),
    Zstd(zstd::stream::Encoder<'static, BufWriter<File>>),
}

pub struct OpsArtifactWriter {
    path: PathBuf,
    tmp_path: PathBuf,
    format: ArtifactFormat,
    inner: ArtifactWriterInner,
    stats: OpStats,
}

impl OpsArtifactWriter {
    pub fn create(path: &Path, format: ArtifactFormat) -> io::Result<Self> {
        let tmp_path = path.with_extension("bin.tmp");
        let file = File::create(&tmp_path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(format.magic())?;
        writer.write_all(&0u64.to_le_bytes())?;
        let inner = match format {
            ArtifactFormat::Raw => ArtifactWriterInner::Raw(writer),
            ArtifactFormat::Zstd => {
                let encoder = zstd::stream::Encoder::new(writer, 3)?;
                ArtifactWriterInner::Zstd(encoder)
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            tmp_path,
            format,
            inner,
            stats: OpStats::default(),
        })
    }

    pub fn stats(&self) -> OpStats {
        self.stats
    }

    pub fn format(&self) -> ArtifactFormat {
        self.format
    }

    pub fn finish(self) -> io::Result<OpStats> {
        let stats = self.stats;
        match self.inner {
            ArtifactWriterInner::Raw(mut writer) => {
                writer.flush()?;
                let mut file = writer.into_inner()?;
                patch_count_and_rename(&mut file, &self.tmp_path, &self.path, stats.ops)?;
            }
            ArtifactWriterInner::Zstd(encoder) => {
                let mut writer = encoder.finish()?;
                writer.flush()?;
                let mut file = writer.into_inner()?;
                patch_count_and_rename(&mut file, &self.tmp_path, &self.path, stats.ops)?;
            }
        }
        Ok(stats)
    }
}

impl OpSink for OpsArtifactWriter {
    fn push_op(&mut self, op: Op) {
        self.stats.observe(&op);
        match &mut self.inner {
            ArtifactWriterInner::Raw(writer) => write_op(writer, &op),
            ArtifactWriterInner::Zstd(encoder) => write_op(encoder, &op),
        }
        .expect("write op artifact");
    }
}

fn patch_count_and_rename(
    file: &mut File,
    tmp_path: &Path,
    path: &Path,
    count: u64,
) -> io::Result<()> {
    file.seek(SeekFrom::Start(RAW_MAGIC.len() as u64))?;
    file.write_all(&count.to_le_bytes())?;
    file.flush()?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn write_op(writer: &mut impl Write, op: &Op) -> io::Result<()> {
    writer.write_all(&(op.kind as u32).to_le_bytes())?;
    writer.write_all(&[0u8; 4])?;
    writer.write_all(&op.q_control2.0.to_le_bytes())?;
    writer.write_all(&op.q_control1.0.to_le_bytes())?;
    writer.write_all(&op.q_target.0.to_le_bytes())?;
    writer.write_all(&op.c_target.0.to_le_bytes())?;
    writer.write_all(&op.c_condition.0.to_le_bytes())?;
    writer.write_all(&op.r_target.0.to_le_bytes())?;
    Ok(())
}

pub fn load_ops(path: &str) -> Result<Vec<Op>, String> {
    let mut file = File::open(path).map_err(|error| format!("read {path}: {error}"))?;
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)
        .map_err(|error| format!("{path}: read magic: {error}"))?;
    let mut count_bytes = [0u8; 8];
    file.read_exact(&mut count_bytes)
        .map_err(|error| format!("{path}: read op count: {error}"))?;
    let count = u64::from_le_bytes(count_bytes);
    if count > MAX_OPS {
        return Err(format!("{path}: op count {count} exceeds cap {MAX_OPS}"));
    }

    if &magic == RAW_MAGIC {
        let expected = RAW_MAGIC.len() + 8 + (count as usize).saturating_mul(OP_BYTES);
        let actual = file
            .metadata()
            .map_err(|error| format!("{path}: stat: {error}"))?
            .len() as usize;
        if actual != expected {
            return Err(format!(
                "{path}: length mismatch: got {actual} expected {expected} for {count} ops"
            ));
        }
        read_ops_from(&mut file, count as usize)
    } else if &magic == ZSTD_MAGIC {
        let mut decoder =
            zstd::stream::Decoder::new(file).map_err(|error| format!("{path}: zstd: {error}"))?;
        read_ops_from(&mut decoder, count as usize)
    } else {
        Err(format!("{path}: bad magic"))
    }
}

fn read_ops_from(reader: &mut impl Read, count: usize) -> Result<Vec<Op>, String> {
    let mut ops = Vec::with_capacity(count);
    for i in 0..count {
        let op = read_op(reader).map_err(|error| format!("op {i}: {error}"))?;
        let validated = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| op.validate()));
        if let Err(error) = validated {
            let message = error
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| error.downcast_ref::<&'static str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "validation panic".to_string());
            return Err(format!("op {i}: {message}"));
        }
        ops.push(op);
    }
    Ok(ops)
}

fn read_op(reader: &mut impl Read) -> io::Result<Op> {
    let mut bytes = [0u8; OP_BYTES];
    reader.read_exact(&mut bytes)?;
    let kind_raw = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let kind = op_kind_from_u32(kind_raw)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unknown op kind"))?;
    Ok(Op {
        kind,
        q_control2: QubitId(read_u64(&bytes, 8)),
        q_control1: QubitId(read_u64(&bytes, 16)),
        q_target: QubitId(read_u64(&bytes, 24)),
        c_target: BitId(read_u64(&bytes, 32)),
        c_condition: BitId(read_u64(&bytes, 40)),
        r_target: RegisterId(read_u64(&bytes, 48)),
    })
}

fn read_u64(bytes: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap())
}

fn op_kind_from_u32(v: u32) -> Option<OperationType> {
    Some(match v {
        0 => OperationType::Neg,
        1 => OperationType::Register,
        2 => OperationType::AppendToRegister,
        3 => OperationType::BitInvert,
        4 => OperationType::BitStore0,
        5 => OperationType::BitStore1,
        6 => OperationType::X,
        7 => OperationType::Z,
        8 => OperationType::CX,
        9 => OperationType::CZ,
        10 => OperationType::Swap,
        11 => OperationType::R,
        12 => OperationType::Hmr,
        13 => OperationType::CCX,
        14 => OperationType::CCZ,
        15 => OperationType::PushCondition,
        16 => OperationType::PopCondition,
        17 => OperationType::DebugPrint,
        _ => return None,
    })
}
