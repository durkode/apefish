//! Workspace task runner behind the `cargo sprt` / `cargo smoke` aliases in
//! `.cargo/config.toml`. Cargo aliases can only expand to cargo commands, so
//! this hands its arguments to the matching script in `sprt/`, which holds all
//! the logic (see sprt/README.md).

use std::path::PathBuf;
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: cargo xtask <sprt|smoke> [script options]

  sprt   SPRT a dev build against a base build (sprt/sprt.sh)
  smoke  self-play robustness check (sprt/smoke.sh)

Pass --help after a task for its options, e.g. `cargo sprt --help`.";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let script = match args.next().as_deref() {
        Some("sprt") => "sprt.sh",
        Some("smoke") => "smoke.sh",
        Some("-h" | "--help") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("sprt")
        .join(script);
    match Command::new("bash").arg(&path).args(args).status() {
        // The scripts' exit codes are meaningful (sprt.sh: 0 pass, 1 fail,
        // 2 undecided), so pass them through unchanged.
        Ok(status) => ExitCode::from(status.code().unwrap_or(2) as u8),
        Err(e) => {
            eprintln!("xtask: failed to run {}: {e}", path.display());
            ExitCode::from(2)
        }
    }
}
