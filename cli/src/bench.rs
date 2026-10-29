//! `apefish bench`: a fixed-depth search over a built-in position set.
//!
//! The total node count is a fingerprint of the engine's search behaviour: two
//! builds with the same count search identically, so a change that leaves it
//! untouched (refactor, UCI plumbing, a pure speedup) cannot change playing
//! strength and needs no SPRT test. Record it in commit messages as `Bench: N`.
//!
//! Every position starts from a freshly allocated transposition table, so the
//! count depends only on the build and the depth, never on run order or timing.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use apefish_engine::search::SearchLimits;
use apefish_engine::{Apefish, Engine, EngineEvent};

/// Iterative-deepening depth used when `apefish bench` is given no depth.
/// Changing this changes every bench signature, so only do it deliberately.
pub const DEFAULT_BENCH_DEPTH: u8 = 7;

/// Transposition table size for each bench search. Small enough that
/// allocating a fresh one per position is negligible.
const BENCH_HASH_MB: usize = 16;

/// Openings, middlegames and endgames. Appending or editing a position changes
/// every bench signature, so treat this list as fixed.
pub const BENCH_FENS: &[&str] = &[
    // Openings
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3",
    "rnbqkb1r/pp2pppp/3p1n2/8/3NP3/8/PPP2PPP/RNBQKB1R w KQkq - 1 5",
    "rnbqkb1r/ppp2ppp/4pn2/3p4/2PP4/2N5/PP2PPPP/R1BQKBNR w KQkq - 2 4",
    "r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/2NP1N2/PPP2PPP/R1BQK2R b KQkq - 0 5",
    // Middlegames
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r1bq1rk1/pp2bppp/2n1pn2/3p4/2PP4/2N2N2/PP2BPPP/R2QKB1R w KQ - 0 8",
    "2r2rk1/pp1bqppp/2n1pn2/3p4/3P4/2PBPN2/P1Q2PPP/R1B2RK1 w - - 4 13",
    "r2q1rk1/1b1nbppp/pp1ppn2/8/2PNP3/1PN1B3/P3BPPP/R2Q1RK1 w - - 2 11",
    "3r1rk1/p4ppp/1pn1p3/2q5/2P5/P3BP2/1Q3P1P/2R2RK1 b - - 1 20",
    // Endgames
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "8/k7/3p4/p2P1p2/P2P1P2/8/8/K7 w - - 0 1",
    "1K1k4/1P6/8/8/8/8/r7/2R5 w - - 0 1",
    "8/8/8/4k3/8/8/4P3/4K3 w - - 0 1",
    "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1",
    "8/5pk1/6p1/7p/3R3P/6P1/r4PK1/8 w - - 0 40",
    "8/8/4kpp1/3p4/p2P2PP/P3K3/8/8 b - - 0 45",
];

pub struct BenchResult {
    pub nodes: u64,
    pub elapsed: Duration,
}

impl BenchResult {
    pub fn nps(&self) -> u128 {
        let micros = self.elapsed.as_micros().max(1);
        u128::from(self.nodes) * 1_000_000 / micros
    }
}

/// Search every position in [`BENCH_FENS`] to `depth`, returning the node total
/// and wall-clock time. Per-position progress goes to stderr.
pub fn run(depth: u8) -> BenchResult {
    let mut engine = Apefish::new(BENCH_HASH_MB);
    let mut total_nodes = 0;
    let mut total_elapsed = Duration::ZERO;

    for (i, fen) in BENCH_FENS.iter().enumerate() {
        engine.set_hash_size(BENCH_HASH_MB);
        engine.new_game();
        engine.set_position(Some(fen), &[]);

        let (tx, rx) = mpsc::channel();
        let limits = SearchLimits { depth: Some(depth), ..SearchLimits::default() };
        let start = Instant::now();
        engine.go(
            limits,
            Box::new(move |ev| {
                if let EngineEvent::BestMove(result) = ev {
                    let _ = tx.send(result.nodes);
                }
            }),
        );
        let nodes = rx.recv().expect("bench search ended without a bestmove");
        total_elapsed += start.elapsed();
        total_nodes += nodes;

        eprintln!("position {:>2}/{}: {nodes:>12} nodes  {fen}", i + 1, BENCH_FENS.len());
    }

    BenchResult { nodes: total_nodes, elapsed: total_elapsed }
}
