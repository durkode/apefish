//! Black-box check that `apefish --uci` actually speaks UCI over stdin/stdout.
//! Everything else about the protocol is covered by the unit tests in `tests/uci_unit.rs`;
//! this just confirms the `--uci` flag is wired up end-to-end.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn uci_flag_speaks_uci_protocol() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_apefish"))
        .arg("--uci")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn apefish --uci");

    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));

    // Search is asynchronous, so drive it like a real GUI: send the commands,
    // read output until the search reports its move, then end the session.
    stdin
        .write_all(b"uci\nisready\nposition startpos moves e2e4 e7e5\ngo movetime 50\n")
        .expect("write to stdin");
    stdin.flush().expect("flush stdin");

    let mut transcript = String::new();
    loop {
        let mut line = String::new();
        let n = stdout.read_line(&mut line).expect("read line");
        assert!(n != 0, "engine closed stdout before bestmove:\n{transcript}");
        transcript.push_str(&line);
        if line.starts_with("bestmove ") {
            break;
        }
    }

    stdin.write_all(b"quit\n").expect("write quit");
    stdin.flush().expect("flush quit");
    child.wait().expect("wait for apefish");

    assert!(transcript.contains("uciok"), "missing uciok in:\n{transcript}");
    assert!(
        transcript.contains("readyok"),
        "missing readyok in:\n{transcript}"
    );
    assert!(
        transcript.lines().any(|l| l.starts_with("bestmove ")),
        "missing bestmove line in:\n{transcript}"
    );
}
