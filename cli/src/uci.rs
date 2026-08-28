//! UCI (Universal Chess Interface) adapter.
//!
//! Speaks the UCI protocol over stdin/stdout, driving an [`Apefish`] purely through
//! the public [`Engine`] trait — see `engine/src/lib.rs` for why that trait exists.
//!
//! Search is asynchronous. [`run`] reads stdin on a dedicated thread and folds
//! every input line and every [`EngineEvent`] into one `mpsc` queue, so the main
//! loop can service `stop`/`isready` while a `go` search is still running.

use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use apefish_engine::search::SearchLimits;
use apefish_engine::{Apefish, Engine, EngineEvent, PieceKind, Square, UnvalidatedMove};

/// Everything the main loop waits on: a line typed by the GUI, an event from the
/// engine's search thread, or the end of stdin.
#[derive(Debug)]
pub enum Msg {
    Line(String),
    Engine(EngineEvent),
    Eof,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    /// Immediate response lines for the GUI (may be empty). A search's own
    /// output (`info`, `bestmove`) does not return this way — it arrives later
    /// as [`Msg::Engine`].
    Continue(Vec<String>),
    Quit,
}

/// Parse a UCI long-algebraic move (e.g. "e2e4", "e7e8q") into an [`UnvalidatedMove`].
pub fn parse_uci_move(s: &str) -> Option<UnvalidatedMove> {
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
    Some(UnvalidatedMove { from, to, promotion })
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

/// Render an [`EngineEvent`] as the UCI line it produces on stdout.
///
/// `elapsed` is wall-clock time since the `go` that started this search, measured
/// on the UCI side because the engine reports no timing. It drives `time` and
/// `nps`; pass `None` when no search start was recorded.
///
/// `nps` and the ponder move are derived entirely here from data the engine
/// already sends (`nodes`, `pv`). `tbhits` is a fixed `0` — the engine has no
/// tablebase probing. `hashfull` is not emitted at all; see the note in `run`.
pub fn format_event(event: &EngineEvent, elapsed: Option<Duration>) -> String {
    match event {
        EngineEvent::Info { depth, result } => {
            let mut line = format!(
                "info depth {depth} score cp {} nodes {}",
                result.score, result.nodes
            );
            if let Some(elapsed) = elapsed {
                let ms = elapsed.as_millis();
                line.push_str(&format!(" time {ms}"));
                if ms > 0 {
                    let nps = u128::from(result.nodes) * 1000 / ms;
                    line.push_str(&format!(" nps {nps}"));
                }
            }
            // Fixed 0: the engine does no tablebase probing. Becomes real once it
            // reports a tbhits count.
            line.push_str(" tbhits 0");
            if !result.pv.is_empty() {
                line.push_str(" pv");
                for mv in &result.pv {
                    line.push(' ');
                    line.push_str(&mv.to_string());
                }
            }
            line
        }
        EngineEvent::Stats { depth, nodes, hashfull, tbhits } => {
            let mut line = format!("info depth {depth} nodes {nodes}");
            if let Some(elapsed) = elapsed {
                let ms = elapsed.as_millis();
                line.push_str(&format!(" time {ms}"));
                if ms > 0 {
                    let nps = u128::from(*nodes) * 1000 / ms;
                    line.push_str(&format!(" nps {nps}"));
                }
            }
            line.push_str(&format!(" hashfull {hashfull} tbhits {tbhits}"));
            line
        }
        EngineEvent::BestMove(result) => {
            let mv = match result.best_move {
                Some(mv) => mv.to_string(),
                None => "0000".to_string(),
            };
            let mut line = format!("bestmove {mv}");
            // Ponder move = the reply the engine expects next, i.e. the second
            // entry of the principal variation.
            if result.best_move.is_some() {
                if let Some(ponder) = result.pv.get(1) {
                    line.push_str(&format!(" ponder {ponder}"));
                }
            }
            line
        }
    }
}

/// Handle a single line of UCI input against `engine`, returning any immediate
/// response lines. A `go` command starts an asynchronous search that reports
/// through `events`; its `info`/`bestmove` output does not come back here.
pub fn handle_line(engine: &mut Apefish, line: &str, events: &Sender<Msg>) -> CommandOutcome {
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
            let sink = events.clone();
            engine.go(
                limits,
                Box::new(move |ev| {
                    let _ = sink.send(Msg::Engine(ev));
                }),
            );
            CommandOutcome::Continue(vec![])
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
    let mut engine = Apefish::new(512);
    let (tx, rx) = mpsc::channel::<Msg>();

    // Dedicated stdin reader: turns each line into a `Msg` so the main loop only
    // ever waits on one queue, and stays responsive during a search.
    let stdin_tx = tx.clone();
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if stdin_tx.send(Msg::Line(line)).is_err() {
                return;
            }
        }
        let _ = stdin_tx.send(Msg::Eof);
    });

    let stdout = io::stdout();
    // Stamped when a `go` is seen so `format_event` can report `time`/`nps`.
    // `hashfull` would be added here too, but the engine owns the transposition
    // table and exposes no fill level, so the UCI side cannot compute it.
    let mut search_start: Option<Instant> = None;
    for msg in rx {
        match msg {
            Msg::Line(line) => {
                if line.split_whitespace().next() == Some("go") {
                    search_start = Some(Instant::now());
                }
                match handle_line(&mut engine, &line, &tx) {
                    CommandOutcome::Continue(lines) => {
                        if !lines.is_empty() {
                            let mut out = stdout.lock();
                            for l in lines {
                                let _ = writeln!(out, "{l}");
                            }
                            let _ = out.flush();
                        }
                    }
                    CommandOutcome::Quit => {
                        engine.stop();
                        break;
                    }
                }
            }
            Msg::Engine(event) => {
                let mut out = stdout.lock();
                let elapsed = search_start.map(|s| s.elapsed());
                let _ = writeln!(out, "{}", format_event(&event, elapsed));
                let _ = out.flush();
            }
            Msg::Eof => {
                engine.stop();
                break;
            }
        }
    }
}
