//! Standalone CLI for the same Stockfish-backed perft bisection that
//! engine/tests/perft.rs now runs automatically on any assertion failure
//! (see its "Debugging a failure" doc section, and `tests/common/mod.rs`
//! for the shared implementation this is a thin wrapper around).
//!
//! Useful for ad-hoc exploration outside the test suite: checking a
//! position/depth that isn't one of the existing test cases, or resuming a
//! bisection at a specific move prefix.
//!
//! Usage:
//!   cargo run -p apefish-engine --example perft_divide -- <depth> "<fen>" [uci_move ...]
//!
//! Trailing UCI moves (e.g. `e2e4`, `e7e8q`) are an optional starting
//! prefix, for resuming a bisection you already narrowed down manually.
//!
//! Looks for a `stockfish` binary on PATH (or /usr/games/stockfish, the
//! usual Debian/Ubuntu location); override with STOCKFISH_BIN=/path/to/it.

#[path = "../tests/common/mod.rs"]
mod common;

use apefish_engine::{Apefish, Engine, Move};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "usage: perft_divide <depth> <fen> [uci_move ...]\n\
             example: perft_divide 4 \"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1\" e2e4"
        );
        std::process::exit(1);
    }
    let depth: u32 = args[1].parse().expect("depth must be a non-negative integer");
    assert!(depth >= 1, "divide needs depth >= 1 (depth 0 has no moves to divide by)");
    let fen = args[2].clone();

    if common::find_stockfish().is_none() {
        eprintln!(
            "error: couldn't find a `stockfish` binary on PATH (checked `stockfish` and \
             /usr/games/stockfish).\nInstall it (e.g. `apt install stockfish`) or point at it \
             with STOCKFISH_BIN=/path/to/stockfish."
        );
        std::process::exit(1);
    }

    let mut engine = Apefish::new();
    let mut prefix: Vec<Move> = Vec::new();
    for uci in &args[3..] {
        engine.set_position(Some(&fen), prefix.as_slice());
        prefix.push(common::parse_uci_move(&mut engine, uci));
    }

    common::bisect_mismatch(&mut engine, &fen, prefix, depth);
}
