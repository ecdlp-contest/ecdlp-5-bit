use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let build_status = Command::new(&cargo)
        .args(["run", "--release", "--bin", "build_circuit"])
        .status()
        .expect("run build_circuit");
    if !build_status.success() {
        std::process::exit(build_status.code().unwrap_or(1));
    }

    let mut eval_args = vec![
        "run".to_string(),
        "--release".to_string(),
        "--bin".to_string(),
        "eval_circuit".to_string(),
        "--".to_string(),
    ];
    eval_args.extend(args);
    let eval_status = Command::new(&cargo)
        .args(eval_args)
        .status()
        .expect("run eval_circuit");
    if !eval_status.success() {
        std::process::exit(eval_status.code().unwrap_or(1));
    }
}
