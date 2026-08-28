//! UCI (Universal Chess Interface) adapter.
//!
//! Speaks the UCI protocol over stdin/stdout, driving an [`Apefish`] purely through
//! the public [`Engine`] trait — see `engine/src/lib.rs` for why that trait exists.
//!
//! Search is asynchronous. [`run`] reads stdin on a dedicated thread and folds
//! every input line and every [`EngineEvent`] into one `mpsc` queue, so the main
//! loop can service `stop`/`isready` while a `go` search is still running.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use apefish_engine::search::SearchLimits;
use apefish_engine::{Apefish, Engine, EngineEvent, PieceKind, Square, UnvalidatedMove};

/// Optional trace log of the UCI conversation, for debugging stalls.
///
/// Enabled by setting `APEFISH_UCI_LOG` to a file path. Every line received from
/// the GUI is logged with `>>`, every line sent with `<<`, and internal markers
/// with `--`; each entry carries a wall-clock timestamp so a hang shows up as a
/// gap between the last line in and the next line out. A no-op (zero overhead
/// beyond a branch) when the variable is unset.
pub struct Logger(Option<Mutex<File>>);

impl Logger {
    pub fn from_env() -> Self {
        let Some(path) = std::env::var_os("APEFISH_UCI_LOG") else {
            return Logger(None);
        };
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => {
                let logger = Logger(Some(Mutex::new(file)));
                logger.mark("--- session start ---");
                logger
            }
            Err(e) => {
                eprintln!("apefish: cannot open APEFISH_UCI_LOG {path:?}: {e}");
                Logger(None)
            }
        }
    }

    /// `dir` is the direction tag: `">>"` received, `"<<"` sent, `"--"` internal.
    pub fn log(&self, dir: &str, text: &str) {
        let Some(file) = &self.0 else { return };
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| format!("{}.{:03}", d.as_secs(), d.subsec_millis()))
            .unwrap_or_default();
        if let Ok(mut file) = file.lock() {
            let _ = writeln!(file, "{ts} {dir} {text}");
            let _ = file.flush();
        }
    }

    fn mark(&self, text: &str) {
        self.log("--", text);
    }
}

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

/// Transposition table size in MiB used when the GUI sends no `setoption name Hash`.
/// Also advertised as the `Hash` option's `default`.
pub const DEFAULT_HASH_MB: usize = 512;

/// Lower/upper bounds advertised for the `Hash` option and enforced on `setoption`.
pub const MIN_HASH_MB: usize = 1;
pub const MAX_HASH_MB: usize = 65536;

/// Parse a `setoption name <id> [value <val>]` tail into `(name, value)`.
/// `name` may contain spaces; `value` is everything after the `value` token
/// (absent for button options). Returns `None` if the `name` token is missing.
pub fn parse_setoption(args: &str) -> Option<(String, Option<String>)> {
    let tokens: Vec<&str> = args.split_whitespace().collect();
    if tokens.first() != Some(&"name") {
        return None;
    }
    let value_pos = tokens.iter().position(|&t| t == "value");
    let name_end = value_pos.unwrap_or(tokens.len());
    let name = tokens[1..name_end].join(" ");
    if name.is_empty() {
        return None;
    }
    let value = value_pos.map(|p| tokens[p + 1..].join(" "));
    Some((name, value))
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
            format!(
                "option name Hash type spin default {DEFAULT_HASH_MB} min {MIN_HASH_MB} max {MAX_HASH_MB}"
            ),
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
        "setoption" => {
            if let Some((name, value)) = parse_setoption(rest) {
                if name.eq_ignore_ascii_case("Hash") {
                    match value.as_deref().and_then(|v| v.parse::<usize>().ok()) {
                        Some(mb) => {
                            engine.set_hash_size(mb.clamp(MIN_HASH_MB, MAX_HASH_MB));
                        }
                        None => eprintln!("apefish: malformed setoption Hash value: {rest}"),
                    }
                }
                // Unknown option names are ignored, per UCI.
            }
            CommandOutcome::Continue(vec![])
        }
        // These carry no state this engine acts on yet, so they are accepted no-ops.
        "debug" | "register" | "ponderhit" => CommandOutcome::Continue(vec![]),
        // Unrecognized input is ignored, per UCI's robustness expectations.
        _ => CommandOutcome::Continue(vec![]),
    }
}

/// Run the UCI protocol loop over stdin/stdout until `quit` or EOF.
///
/// `hash_mb` is the initial transposition table size in MiB (see the `--hash`
/// CLI flag and [`DEFAULT_HASH_MB`]). The GUI can still change it at runtime
/// with `setoption name Hash value <mb>`.
pub fn run(hash_mb: usize) {
    let mut engine = Apefish::new(hash_mb);
    let (tx, rx) = mpsc::channel::<Msg>();
    let logger = Arc::new(Logger::from_env());

    // Dedicated stdin reader: turns each line into a `Msg` so the main loop only
    // ever waits on one queue, and stays responsive during a search.
    let stdin_tx = tx.clone();
    let stdin_logger = Arc::clone(&logger);
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            stdin_logger.log(">>", &line);
            if stdin_tx.send(Msg::Line(line)).is_err() {
                return;
            }
        }
        stdin_logger.log("--", "stdin EOF");
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
                let cmd = line.split_whitespace().next().unwrap_or("").to_string();
                logger.log("--", &format!("dispatch {cmd}"));
                let outcome = handle_line(&mut engine, &line, &tx);
                logger.log("--", &format!("dispatched {cmd}"));
                match outcome {
                    CommandOutcome::Continue(lines) => {
                        if !lines.is_empty() {
                            let mut out = stdout.lock();
                            for l in lines {
                                logger.log("<<", &l);
                                let _ = writeln!(out, "{l}");
                            }
                            let _ = out.flush();
                        }
                    }
                    CommandOutcome::Quit => {
                        logger.log("--", "quit: stopping search");
                        engine.stop();
                        logger.log("--", "quit: search stopped");
                        break;
                    }
                }
            }
            Msg::Engine(event) => {
                let mut out = stdout.lock();
                let elapsed = search_start.map(|s| s.elapsed());
                let formatted = format_event(&event, elapsed);
                logger.log("<<", &formatted);
                let _ = writeln!(out, "{formatted}");
                let _ = out.flush();
            }
            Msg::Eof => {
                logger.log("--", "eof: stopping search");
                engine.stop();
                logger.log("--", "eof: search stopped");
                break;
            }
        }
    }
}
