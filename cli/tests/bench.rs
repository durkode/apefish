//! `apefish bench` must be deterministic: SPRT tooling treats an unchanged node
//! count as proof that a change cannot affect play.

use apefish_cli::bench;

#[test]
fn bench_node_count_is_deterministic() {
    let first = bench::run(3);
    let second = bench::run(3);
    assert!(first.nodes > 0);
    assert_eq!(first.nodes, second.nodes);
}
