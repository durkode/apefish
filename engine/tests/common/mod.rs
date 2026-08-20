//! Shared perft debugging helpers: a Stockfish-backed "divide" that
//! auto-bisects a leaf-count mismatch down to the exact node and move where
//! the two engines' move generation disagrees.
//!
//! Used by perft.rs (auto-run on any assertion failure, so a failing
//! `cargo test` needs no manual re-running against a separate tool) and by
//! the standalone `perft_divide` example (for ad-hoc/manual bisection from
//! the command line).
//!
//! Only the public `Engine` trait is touched here, consistent with
//! perft.rs's own rule: this file doesn't reach into `Position`, `MoveGen`,
//! or any other internal type.
//!
//! `common/mod.rs` (rather than `common.rs`) is deliberate: it keeps cargo
//! from treating this as its own top-level integration test binary.

use apefish_engine::{Engine, Move, PieceKind, Square};
use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// Resolve a UCI move string (e.g. "e2e4", "e7e8q") against the engine's
/// current legal moves, to recover the full internal `Move`.
/// (Used by the `perft_divide` example for its move-prefix args, not by
/// perft.rs, which never needs to parse a UCI string.)
#[allow(dead_code)]
pub fn parse_uci_move<E: Engine>(engine: &mut E, uci: &str) -> Move {
    let from = Square::from_string(&uci[0..2]).unwrap_or_else(|_| panic!("bad square in move `{uci}`"));
    let to = Square::from_string(&uci[2..4]).unwrap_or_else(|_| panic!("bad square in move `{uci}`"));
    let promotion = uci.chars().nth(4).map(|c| match c {
        'q' => PieceKind::Queen,
        'r' => PieceKind::Rook,
        'b' => PieceKind::Bishop,
        'n' => PieceKind::Knight,
        _ => panic!("bad promotion char in move `{uci}`"),
    });
    engine
        .legal_moves()
        .into_iter()
        .find(|m| m.from() == from && m.to() == to && m.promotion() == promotion)
        .unwrap_or_else(|| panic!("`{uci}` is not legal in the current position"))
}

/// Leaf node count `depth` plies below the node reached by `moves`, replaying
/// from `fen` each time (there's no `unmake_move` on `Engine`, so each node
/// is reached by replaying the whole path from the root).
/// (Used by perft.rs's `assert_perft`, not by the `perft_divide` example,
/// which only ever calls `divide`.)
#[allow(dead_code)]
pub fn perft<E: Engine>(engine: &mut E, fen: &str, depth: u32) -> u64 {
    fn go<E: Engine>(engine: &mut E, fen: &str, moves: &mut Vec<Move>, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        engine.set_position(Some(fen), moves.as_slice());
        let legal = engine.legal_moves();
        if depth == 1 {
            return legal.len() as u64;
        }
        let mut nodes = 0;
        for mv in legal {
            moves.push(mv);
            nodes += go(engine, fen, moves, depth - 1);
            moves.pop();
        }
        nodes
    }
    go(engine, fen, &mut Vec::new(), depth)
}

/// Per-move leaf-node breakdown at the node reached by `prefix`, `depth`
/// plies deep - i.e. `perft(depth - 1)` from each of that node's legal moves.
pub fn divide<E: Engine>(engine: &mut E, fen: &str, prefix: &[Move], depth: u32) -> (Vec<(Move, u64)>, u64) {
    engine.set_position(Some(fen), prefix);
    let legal = engine.legal_moves();
    let mut rows = Vec::new();
    let mut total = 0u64;
    for mv in legal {
        let mut moves = prefix.to_vec();
        moves.push(mv);
        let count = perft_from(engine, fen, &mut moves, depth - 1);
        total += count;
        rows.push((mv, count));
    }
    rows.sort_by_key(|(mv, _)| mv.to_string());
    (rows, total)
}

fn perft_from<E: Engine>(engine: &mut E, fen: &str, moves: &mut Vec<Move>, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    engine.set_position(Some(fen), moves.as_slice());
    let legal = engine.legal_moves();
    if depth == 1 {
        return legal.len() as u64;
    }
    let mut nodes = 0;
    for mv in legal {
        moves.push(mv);
        nodes += perft_from(engine, fen, moves, depth - 1);
        moves.pop();
    }
    nodes
}

/// Locate a `stockfish` binary: `STOCKFISH_BIN` env var, then `stockfish` on
/// PATH, then the usual Debian/Ubuntu location. Returns `None` rather than
/// failing outright, since Stockfish isn't guaranteed to be installed
/// everywhere `cargo test` runs.
pub fn find_stockfish() -> Option<String> {
    if let Ok(p) = env::var("STOCKFISH_BIN") {
        return Some(p);
    }
    for candidate in ["stockfish", "/usr/games/stockfish"] {
        let found = Command::new("sh").arg("-c").arg(format!("command -v {candidate}")).output();
        if matches!(&found, Ok(out) if out.status.success()) {
            return Some(candidate.to_string());
        }
    }
    None
}

/// Run `go perft <depth>` on `sf_bin` from `fen` + `prefix_uci`, returning
/// the same per-move breakdown as [`divide`].
pub fn stockfish_divide(sf_bin: &str, fen: &str, prefix_uci: &[String], depth: u32) -> (Vec<(String, u64)>, u64) {
    let position_cmd = if prefix_uci.is_empty() {
        format!("position fen {fen}")
    } else {
        format!("position fen {fen} moves {}", prefix_uci.join(" "))
    };

    let mut child = Command::new(sf_bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to launch stockfish (`{sf_bin}`): {e}"));

    let mut stdin = child.stdin.take().expect("stockfish stdin");
    writeln!(stdin, "uci").unwrap();
    writeln!(stdin, "{position_cmd}").unwrap();
    writeln!(stdin, "go perft {depth}").unwrap();
    writeln!(stdin, "quit").unwrap();
    drop(stdin);

    let stdout = child.stdout.take().expect("stockfish stdout");
    let mut rows = Vec::new();
    let mut total = None;
    for line in BufReader::new(stdout).lines() {
        let line = line.expect("reading stockfish stdout");
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Nodes searched: ") {
            total = Some(rest.parse::<u64>().expect("bad `Nodes searched` line from stockfish"));
        } else if let Some((mv, count)) = line.split_once(": ") {
            if let Ok(count) = count.parse::<u64>() {
                rows.push((mv.to_string(), count));
            }
        }
    }
    child.wait().ok();
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    let total = total.unwrap_or_else(|| {
        panic!("stockfish produced no `Nodes searched` line for `go perft {depth}` at `{position_cmd}`")
    });
    (rows, total)
}

/// Auto-bisect a perft mismatch at `fen`/`prefix`/`depth`: divides both
/// engines at the current node, and if the move lists agree but a shared
/// move's subtree count doesn't, descends into that move and repeats -
/// until it isolates either an outright move-list disagreement (the bug
/// itself) or, in the unexpected case, a clean match. Prints its trace as
/// it goes; does nothing but print a note if no `stockfish` binary is found.
pub fn bisect_mismatch<E: Engine>(engine: &mut E, fen: &str, mut prefix: Vec<Move>, mut depth: u32) {
    let Some(sf_bin) = find_stockfish() else {
        println!(
            "(auto-bisect skipped: no `stockfish` binary found - install it, e.g. `apt install stockfish`, \
             or set STOCKFISH_BIN=/path/to/stockfish, to get this automatically)"
        );
        return;
    };

    loop {
        let prefix_uci: Vec<String> = prefix.iter().map(|m| m.to_string()).collect();
        println!(
            "--- bisect: depth {depth}, fen `{fen}`{} ---",
            if prefix_uci.is_empty() { String::new() } else { format!(", moves: {}", prefix_uci.join(" ")) }
        );

        let (our_rows, our_total) = divide(engine, fen, &prefix, depth);
        let (sf_rows, sf_total) = stockfish_divide(&sf_bin, fen, &prefix_uci, depth);

        let our_map: HashMap<String, u64> = our_rows.iter().map(|(mv, c)| (mv.to_string(), *c)).collect();
        let sf_map: HashMap<String, u64> = sf_rows.iter().cloned().collect();

        let extra: Vec<(String, u64)> = our_rows
            .iter()
            .filter(|(mv, _)| !sf_map.contains_key(&mv.to_string()))
            .map(|(mv, c)| (mv.to_string(), *c))
            .collect();
        let missing: Vec<(String, u64)> =
            sf_rows.iter().filter(|(mv, _)| !our_map.contains_key(mv)).cloned().collect();
        let mut diff: Vec<(Move, String, u64, u64)> = our_rows
            .iter()
            .filter_map(|(mv, c)| {
                let key = mv.to_string();
                sf_map.get(&key).filter(|&&sfc| sfc != *c).map(|&sfc| (*mv, key, *c, sfc))
            })
            .collect();

        if extra.is_empty() && missing.is_empty() && diff.is_empty() {
            println!("bisect: MATCH ({our_total} nodes) - no divergence found at this node/depth.");
            return;
        }

        if !extra.is_empty() || !missing.is_empty() {
            let matched: Vec<(String, u64)> = our_rows
                .iter()
                .map(|(mv, c)| (mv.to_string(), *c))
                .filter(|(mv, c)| sf_map.get(mv) == Some(c))
                .collect();

            // `divide` leaves the engine sitting wherever its last recursive
            // perft call landed, not at this node - reset to `prefix` so the
            // printed FEN is this exact position, not the wrong one.
            engine.set_position(Some(fen), &prefix);
            let node_fen = engine.fen();

            println!(
                "bisect: DIVERGENCE FOUND (ours: {our_total} nodes, stockfish: {sf_total} nodes) at fen \
                 `{node_fen}` - move list itself disagrees at this node:"
            );
            for (mv, c) in &extra {
                println!("  extra move our engine generates (illegal!): {mv} ({c} nodes under it)");
            }
            for (mv, c) in &missing {
                println!("  move our engine is missing (should be legal): {mv} ({c} nodes expected under it)");
            }
            println!("  matched ({} moves, same count both sides):", matched.len());
            for (mv, c) in &matched {
                println!("    {mv} ({c} nodes under it)");
            }
            return;
        }

        diff.sort_by(|a, b| a.1.cmp(&b.1));
        let (bad_move, bad_str, our_c, sf_c) = diff[0].clone();
        println!(
            "bisect: move list agrees ({} moves) but node counts differ (ours: {our_total}, stockfish: \
             {sf_total}); descending into `{bad_str}` (ours: {our_c}, stockfish: {sf_c})",
            our_rows.len()
        );

        if depth == 1 {
            println!("bisect: unexpected - count mismatch at depth 1 with agreeing move lists.");
            return;
        }
        prefix.push(bad_move);
        depth -= 1;
    }
}
