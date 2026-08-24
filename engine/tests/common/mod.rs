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

use apefish_engine::{Engine, InputMove, Move, PieceKind, Square};
use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::panic::{self, AssertUnwindSafe};
use std::process::{Command, Stdio};

/// Diagnostic captured when the engine records something as an illegal move
/// partway through a perft traversal: either `make_move` outright rejects a
/// move `legal_moves` just offered, or the engine panics internally (e.g. on
/// a corrupt board state a few plies downstream of an earlier bad move).
///
/// Carries the *whole* move chain from the original test position down to
/// the failure, with the fen after each step, not just the immediate
/// fen/move - useful both for pasting any intermediate position straight
/// into an analysis board, and for reproducing the failure independently of
/// the engine's own (possibly also buggy) FEN serialization, e.g. via a UCI
/// `position fen <root_fen> moves <chain...>` command.
#[allow(dead_code)]
#[derive(Debug)]
pub struct EngineFailure {
    /// The fen the perft/divide run started from.
    pub root_fen: String,
    /// Moves already successfully made from `root_fen` to reach `fen`, each
    /// paired with the resulting fen right after that move.
    pub chain: Vec<(Move, String)>,
    /// The position right before the failing move (or, if `mv` is `None`,
    /// right before the panicking `legal_moves()` call). Equal to the fen of
    /// the last `chain` entry, or to `root_fen` if `chain` is empty.
    pub fen: String,
    /// The move that was rejected/panicked on, if the failure happened
    /// while making a specific move rather than while enumerating them.
    pub mv: Option<Move>,
    pub detail: String,
}

impl std::fmt::Display for EngineFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "root fen `{}`", self.root_fen)?;
        for (i, (mv, fen)) in self.chain.iter().enumerate() {
            writeln!(f, "  {}. {mv} -> fen `{fen}`", i + 1)?;
        }
        match self.mv {
            Some(mv) => write!(f, "  {}. attempted move `{mv}` -> fen `{}`: {}", self.chain.len() + 1, self.fen, self.detail),
            None => write!(f, "  (enumerating legal moves at fen `{}`): {}", self.fen, self.detail),
        }
    }
}

fn panic_detail(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        format!("engine panicked: {s}")
    } else if let Some(s) = payload.downcast_ref::<String>() {
        format!("engine panicked: {s}")
    } else {
        "engine panicked with a non-string payload".to_string()
    }
}

/// `legal_moves()`, but a panic inside it (e.g. from a board left corrupt by
/// an earlier bad move) is caught and turned into an [`EngineFailure`]
/// instead of taking the whole test binary down. `root_fen`/`chain` are
/// carried through purely for the diagnostic - they don't affect behaviour.
fn checked_legal_moves<E: Engine>(
    engine: &mut E,
    root_fen: &str,
    chain: &[(Move, String)],
) -> Result<Vec<Move>, EngineFailure> {
    let fen = engine.fen();
    panic::catch_unwind(AssertUnwindSafe(|| engine.legal_moves())).map_err(|payload| EngineFailure {
        root_fen: root_fen.to_string(),
        chain: chain.to_vec(),
        fen,
        mv: None,
        detail: panic_detail(payload),
    })
}

/// `make_move()`, but both an `Err` return (the engine offered a move via
/// `legal_moves` that it then refuses to make - the exact "recorded as an
/// illegal move" case this module exists to make debuggable) and an internal
/// panic are caught and turned into an [`EngineFailure`] instead of
/// propagating. `root_fen`/`chain` are carried through purely for the
/// diagnostic - they don't affect behaviour.
fn checked_make_move<E: Engine>(
    engine: &mut E,
    root_fen: &str,
    chain: &[(Move, String)],
    mv: Move,
) -> Result<(), EngineFailure> {
    let fen = engine.fen();
    let input = to_input_move(mv);
    match panic::catch_unwind(AssertUnwindSafe(|| engine.make_move(input))) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(EngineFailure {
            root_fen: root_fen.to_string(),
            chain: chain.to_vec(),
            fen,
            mv: Some(mv),
            detail: format!("make_move rejected it as illegal: {e:?}"),
        }),
        Err(payload) => Err(EngineFailure {
            root_fen: root_fen.to_string(),
            chain: chain.to_vec(),
            fen,
            mv: Some(mv),
            detail: panic_detail(payload),
        }),
    }
}

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

/// Leaf node count `depth` plies below the node reached by `moves`, using
/// incremental `make_move`/`unmake_move` rather than replaying from `fen` at
/// every node.
/// (Used by perft.rs's `assert_perft`, not by the `perft_divide` example,
/// which only ever calls `divide`.)
#[allow(dead_code)]
pub fn perft<E: Engine>(engine: &mut E, fen: &str, depth: u32) -> u64 {
    engine.set_position(Some(fen), &[]);
    count_leaves(engine, depth)
}

fn to_input_move(mv: Move) -> InputMove {
    InputMove { from: mv.from(), to: mv.to(), promotion: mv.promotion() }
}

/// Leaf node count `depth` plies below the engine's current position, using
/// incremental `make_move`/`unmake_move`. Shared by [`perft`] (from the root)
/// and [`divide`] (from wherever it's positioned after each top-level move).
fn count_leaves<E: Engine>(engine: &mut E, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let legal = engine.legal_moves();
    if depth == 1 {
        return legal.len() as u64;
    }
    let mut nodes = 0;
    for mv in legal {
        engine.make_move(to_input_move(mv)).expect("legal move rejected by make_move");
        nodes += count_leaves(engine, depth - 1);
        engine.unmake_move();
    }
    nodes
}

/// Fallible counterpart to [`count_leaves`]: catches both a rejected
/// `make_move` and an internal engine panic at any depth, surfacing the
/// exact fen/move where it happened - and the full move chain from
/// `root_fen` down to it - rather than unwinding straight out of the test.
/// `chain` holds the moves already made from `root_fen` to the current node;
/// it grows and shrinks with the recursion so a failure at any depth can
/// report the complete path that led to it, not just its immediate move.
fn checked_count_leaves<E: Engine>(
    engine: &mut E,
    root_fen: &str,
    chain: &mut Vec<(Move, String)>,
    depth: u32,
) -> Result<u64, EngineFailure> {
    if depth == 0 {
        return Ok(1);
    }
    let legal = checked_legal_moves(engine, root_fen, chain)?;
    if depth == 1 {
        return Ok(legal.len() as u64);
    }
    let mut nodes = 0;
    for mv in legal {
        checked_make_move(engine, root_fen, chain, mv)?;
        chain.push((mv, engine.fen()));
        // Only unmake (and pop `chain`) on success: on failure the position
        // may be left corrupt by whatever just went wrong (e.g. a change log
        // applied partway through before the panic), and unmake_move()
        // itself can then panic on that corrupt state (observed: "Should
        // not be removing unfound zobrists") - which would blow away the
        // diagnostic we're about to propagate. Callers abandon this engine
        // instance on failure anyway (bisect_mismatch resets via
        // `set_position` before its next attempt), so there's nothing to
        // balance here.
        let sub = checked_count_leaves(engine, root_fen, chain, depth - 1)?;
        chain.pop();
        engine.unmake_move();
        nodes += sub;
    }
    Ok(nodes)
}

/// Fallible counterpart to [`perft`]: used by `assert_perft` so that an
/// engine failure partway through (illegal move or internal panic) is
/// reported with the exact fen/move it happened at - plus the full move
/// chain from `fen` to that point - instead of panicking the whole test with
/// no context.
#[allow(dead_code)]
pub fn checked_perft<E: Engine>(engine: &mut E, fen: &str, depth: u32) -> Result<u64, EngineFailure> {
    engine.set_position(Some(fen), &[]);
    checked_count_leaves(engine, fen, &mut Vec::new(), depth)
}

/// Per-move leaf-node breakdown at the node reached by `prefix`, `depth`
/// plies deep - i.e. `perft(depth - 1)` from each of that node's legal moves.
///
/// Fallible so `bisect_mismatch` can keep narrowing down a divergence even
/// when the underlying cause is an engine failure (rejected/panicking
/// `make_move`) rather than a plain node-count mismatch - the `EngineFailure`
/// carries the exact fen/move that triggered it, chained all the way back to
/// `fen` (i.e. including `prefix` plus however much further the traversal
/// got before failing).
#[allow(dead_code)]
pub fn divide<E: Engine>(
    engine: &mut E,
    fen: &str,
    prefix: &[Move],
    depth: u32,
) -> Result<(Vec<(Move, u64)>, u64), EngineFailure> {
    // Capture the fen after each `prefix` move for the diagnostic chain, via
    // repeated `set_position` calls rather than replaying through
    // `checked_make_move`: `set_position` applies `prefix`'s already-resolved
    // internal `Move`s directly (`position.make_move`), while
    // `checked_make_move` goes through `Engine::make_move`'s
    // `InputMove -> to_internal_move` round-trip - a different path that can
    // re-resolve a move differently (e.g. picking the wrong candidate when
    // several moves share a from/to square) and did, in practice, introduce
    // spurious failures here. Recomputing this way keeps the actual
    // traversal below on the exact same path it was on before this
    // diagnostic was added.
    let mut chain: Vec<(Move, String)> = Vec::with_capacity(prefix.len());
    for i in 0..prefix.len() {
        engine.set_position(Some(fen), &prefix[..=i]);
        chain.push((prefix[i], engine.fen()));
    }
    engine.set_position(Some(fen), prefix);
    let legal = checked_legal_moves(engine, fen, &chain)?;
    let mut rows = Vec::new();
    let mut total = 0u64;
    for mv in legal {
        checked_make_move(engine, fen, &chain, mv)?;
        chain.push((mv, engine.fen()));
        // See the matching comment in `checked_count_leaves`: don't unmake
        // on failure, since the position may be corrupt and unmake_move()
        // can itself panic on that corrupt state.
        let count = checked_count_leaves(engine, fen, &mut chain, depth - 1)?;
        chain.pop();
        engine.unmake_move();
        total += count;
        rows.push((mv, count));
    }
    rows.sort_by_key(|(mv, _)| mv.to_string());
    Ok((rows, total))
}

/// Locate a `stockfish` binary: `STOCKFISH_BIN` env var, then `stockfish` on
/// PATH, then the usual Debian/Ubuntu location. Returns `None` rather than
/// failing outright, since Stockfish isn't guaranteed to be installed
/// everywhere `cargo test` runs.
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
            "[bisect] probing depth {depth} at fen `{fen}`{}",
            if prefix_uci.is_empty() { String::new() } else { format!(" after moves: {}", prefix_uci.join(" ")) }
        );

        let (our_rows, our_total) = match divide(engine, fen, &prefix, depth) {
            Ok(v) => v,
            Err(failure) => {
                println!(
                    "[bisect] engine failed while dividing this node - descending stops here, this is \
                     as precise as bisection can get\n\
                     \n\
                     === BISECT RESULT: engine failure (not just a node-count mismatch) ===\n\
                     {failure}"
                );
                return;
            }
        };
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
            println!("\n=== BISECT RESULT: no divergence found at this node ({our_total} nodes match) ===");
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
                "\n=== BISECT RESULT: move list itself disagrees at this node ===\n\
                 fen: `{node_fen}`\n\
                 node count here: ours {our_total}, stockfish {sf_total}"
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
            "[bisect] move lists agree ({} moves) but node counts differ (ours {our_total}, stockfish \
             {sf_total}); descending into `{bad_str}` (ours {our_c}, stockfish {sf_c})",
            our_rows.len()
        );

        if depth == 1 {
            println!("\n=== BISECT RESULT: unexpected - count mismatch at depth 1 with agreeing move lists ===");
            return;
        }
        prefix.push(bad_move);
        depth -= 1;
    }
}
