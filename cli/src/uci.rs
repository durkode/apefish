//! UCI (Universal Chess Interface) adapter.
//!
//! Speaks the UCI protocol over stdin/stdout, driving an [`Apefish`] purely through
//! the public [`Engine`] trait — see `engine/src/lib.rs` for why that trait exists.

use std::io::{self, BufRead, Write};
use std::time::Duration;

use apefish_engine::search::SearchLimits;
use apefish_engine::{Apefish, Engine, InputMove, PieceKind, Square};

#[derive(Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    Continue(Vec<String>),
    Quit,
}

/// Parse a UCI long-algebraic move (e.g. "e2e4", "e7e8q") into an [`InputMove`].
pub fn parse_uci_move(s: &str) -> Option<InputMove> {
    if s.len() != 4 && s.len() != 5 {
        return None;
    }
    let from = Square::from_string(&s[0..2]).ok()?;
    let to = Square::from_string(&s[2..4]).ok()?;
    let promotion = match s.as_bytes().get(4) {
        None => None,
        Some(b'q') => Some(PieceKind::Queen),
        Some(b'r') => Some(PieceKind::Rook),
        Some(b'b') => Some(PieceKind::Bishop),
        Some(b'n') => Some(PieceKind::Knight),
        Some(_) => return None,
    };
    Some(InputMove { from, to, promotion })
}

/// Parse the argument tail of a `go` command into [`SearchLimits`].
/// Tokens this engine doesn't act on yet (`ponder`, `infinite`, `nodes`, `mate`,
/// `searchmoves`, ...) are skipped rather than rejected.
pub fn parse_go_limits(args: &str) -> SearchLimits {
    let tokens: Vec<&str> = args.split_whitespace().collect();
    let mut limits = SearchLimits::default();
    let mut i = 0;
    while i < tokens.len() {
        let value_ms = || tokens.get(i + 1).and_then(|v| v.parse::<u64>().ok());
        match tokens[i] {
            "depth" => {
                limits.depth = tokens.get(i + 1).and_then(|v| v.parse::<u8>().ok());
                i += 2;
            }
            "movetime" => {
                limits.movetime = value_ms().map(Duration::from_millis);
                i += 2;
            }
            "wtime" => {
                limits.wtime = value_ms().map(Duration::from_millis);
                i += 2;
            }
            "btime" => {
                limits.btime = value_ms().map(Duration::from_millis);
                i += 2;
            }
            "winc" => {
                limits.winc = value_ms().map(Duration::from_millis);
                i += 2;
            }
            "binc" => {
                limits.binc = value_ms().map(Duration::from_millis);
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }
    limits
}

/// Apply a `position [startpos | fen <fen>] [moves <uci-move>...]` command to `engine`.
fn handle_position(engine: &mut Apefish, args: &str) {
    let tokens: Vec<&str> = args.split_whitespace().collect();
    engine.new_game();

    let mut idx = 0;
    if tokens.first() == Some(&"startpos") {
        idx = 1;
    } else if tokens.first() == Some(&"fen") {
        let moves_idx = tokens.iter().position(|&t| t == "moves");
        let fen_end = moves_idx.unwrap_or(tokens.len());
        let fen_str = tokens[1..fen_end].join(" ");
        engine.set_position(Some(&fen_str), &[]);
        idx = fen_end;
    }

    if tokens.get(idx) == Some(&"moves") {
        for mv_str in &tokens[idx + 1..] {
            let Some(input_move) = parse_uci_move(mv_str) else {
                eprintln!("apefish: malformed move in position command: {mv_str}");
                break;
            };
            if engine.make_move(input_move).is_err() {
                eprintln!("apefish: illegal move in position command: {mv_str}");
                break;
            }
        }
    }
}

/// Handle a single line of UCI input against `engine`, returning any response lines.
pub fn handle_line(engine: &mut Apefish, line: &str) -> CommandOutcome {
    let line = line.trim();
    let mut parts = line.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();

    match cmd {
        "uci" => CommandOutcome::Continue(vec![
            "id name apefish".to_string(),
            "id author Hamish Durkin".to_string(),
            "uciok".to_string(),
        ]),
        "isready" => CommandOutcome::Continue(vec!["readyok".to_string()]),
        "ucinewgame" => {
            engine.new_game();
            CommandOutcome::Continue(vec![])
        }
        "position" => {
            handle_position(engine, rest);
            CommandOutcome::Continue(vec![])
        }
        "go" => {
            let limits = parse_go_limits(rest);
            let result = engine.go(limits);
            let mv_str = match result.best_move {
                Some(mv) => mv.to_string(),
                None => "0000".to_string(),
            };
            CommandOutcome::Continue(vec![format!("bestmove {mv_str}")])
        }
        "stop" => {
            engine.stop();
            CommandOutcome::Continue(vec![])
        }
        "quit" => CommandOutcome::Quit,
        // No configurable options are exposed yet, so these are accepted no-ops.
        "setoption" | "debug" | "register" | "ponderhit" => CommandOutcome::Continue(vec![]),
        // Unrecognized input is ignored, per UCI's robustness expectations.
        _ => CommandOutcome::Continue(vec![]),
    }
}

/// Run the UCI protocol loop over stdin/stdout until `quit` or EOF.
pub fn run() {
    let mut engine = Apefish::new();
    let stdout = io::stdout();
    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        match handle_line(&mut engine, &line) {
            CommandOutcome::Continue(lines) => {
                if !lines.is_empty() {
                    let mut out = stdout.lock();
                    for l in lines {
                        let _ = writeln!(out, "{l}");
                    }
                    let _ = out.flush();
                }
            }
            CommandOutcome::Quit => break,
        }
    }
}
