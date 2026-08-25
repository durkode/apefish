//! Black-box check that `apefish --uci` actually speaks UCI over stdin/stdout.
//! Everything else about the protocol is covered by the unit tests in `src/uci.rs`;
//! this just confirms the `--uci` flag is wired up end-to-end.

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn uci_flag_speaks_uci_protocol() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_apefish"))
        .arg("--uci")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn apefish --uci");

    let script = "uci\nisready\nposition startpos moves e2e4 e7e5\ngo movetime 50\nquit\n";
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(script.as_bytes())
        .expect("write to stdin");

    let output = child.wait_with_output().expect("wait for apefish");
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");

    assert!(stdout.contains("uciok"), "missing uciok in:\n{stdout}");
    assert!(stdout.contains("readyok"), "missing readyok in:\n{stdout}");
    assert!(
        stdout.lines().any(|l| l.starts_with("bestmove ")),
        "missing bestmove line in:\n{stdout}"
    );
}
