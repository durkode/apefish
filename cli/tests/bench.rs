//! `apefish bench` must be deterministic: SPRT tooling treats an unchanged node
//! count as proof that a change cannot affect play.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use apefish_cli::bench::{self, BenchResult};

/// Run `bench::run(depth)` on a worker thread, failing the test instead of
/// hanging it if the search doesn't finish within `limit`.
fn run_with_timeout(depth: u8, limit: Duration) -> BenchResult {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(bench::run(depth));
    });
    rx.recv_timeout(limit)
        .unwrap_or_else(|_| panic!("bench at depth {depth} did not finish within {limit:?}: search hang or node explosion"))
}

#[test]
fn bench_node_count_is_deterministic() {
    let first = run_with_timeout(3, Duration::from_secs(60));
    let second = run_with_timeout(3, Duration::from_secs(60));
    assert!(first.nodes > 0);
    assert_eq!(first.nodes, second.nodes);
}

/// Guards against search blow-ups, e.g. quiescence search without move
/// ordering, which took Kiwipete from ~400 to ~18M nodes at depth 3 and made
/// bench (and anything else searching tactical positions) look hung.
///
/// The budget is ~10x the current count, so normal search changes pass and
/// only an order-of-magnitude regression fails. Raise it deliberately if a
/// change legitimately costs that many more nodes.
#[test]
fn bench_node_count_stays_within_budget() {
    const DEPTH_3_NODE_BUDGET: u64 = 500_000;
    let result = run_with_timeout(3, Duration::from_secs(60));
    assert!(
        result.nodes < DEPTH_3_NODE_BUDGET,
        "bench at depth 3 searched {} nodes, budget is {DEPTH_3_NODE_BUDGET}",
        result.nodes
    );
}
