//! Move generation throughput benchmark - nodes/sec through `perft` on a
//! handful of positions, distinct from engine/tests/perft.rs which checks
//! move generation *correctness*. This file only cares about speed: node
//! counts below are for sizing each case, not asserted against.
//!
//! Run it with:
//!
//!   cargo bench -p apefish-engine
//!
//! which prints a nodes/sec ("thrpt") figure per position, backed by
//! `Throughput::Elements` so criterion does the nodes-per-second division
//! itself. To see whether an engine change sped things up or slowed them
//! down, save a baseline before the change and compare after it:
//!
//!   cargo bench -p apefish-engine -- --save-baseline before
//!   ... make your change ...
//!   cargo bench -p apefish-engine -- --baseline before
//!
//! The second run prints a regressed/improved percentage per benchmark
//! instead of just an absolute number, and `target/criterion/report/index.html`
//! has the full history/plots. Criterion also warns about outliers/noise, so
//! a run on a busy machine is flagged rather than silently trusted.
//!
//! Positions are the same well-known ones engine/tests/perft.rs verifies
//! (chessprogramming.org's "Perft Results" set), at depths chosen to land in
//! the low millions of nodes: enough to swamp per-call overhead, short
//! enough that the whole suite runs in well under a minute.

use apefish_engine::Apefish;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};

#[path = "../tests/common/mod.rs"]
mod common;

struct Case {
    name: &'static str,
    fen: &'static str,
    depth: u32,
    /// Expected leaf count at `depth`, used only to compute nodes/sec -
    /// see engine/tests/perft.rs for the correctness check these came from.
    nodes: u64,
}

const CASES: &[Case] = &[
    Case {
        name: "startpos_d5",
        fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        depth: 5,
        nodes: 4_865_609,
    },
    Case {
        name: "kiwipete_d4",
        fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        depth: 4,
        nodes: 4_085_603,
    },
    Case {
        name: "position3_d5",
        fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        depth: 5,
        nodes: 674_624,
    },
    Case {
        name: "position4_d4",
        fen: "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        depth: 4,
        nodes: 422_333,
    },
    Case {
        name: "position5_d4",
        fen: "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        depth: 4,
        nodes: 2_103_487,
    },
    Case {
        name: "position6_d4",
        fen: "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        depth: 4,
        nodes: 3_894_594,
    },
];

fn perft_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("perft_nps");
    // Each iteration is a multi-million-node tree walk, so cap the sample
    // count well below criterion's default of 100 - otherwise a full run
    // balloons to several minutes for no extra precision.
    group.sample_size(10);
    for case in CASES {
        group.throughput(Throughput::Elements(case.nodes));
        group.bench_function(case.name, |b| {
            b.iter_batched(
                Apefish::new,
                |mut engine| common::perft(&mut engine, case.fen, case.depth),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, perft_throughput);
criterion_main!(benches);
