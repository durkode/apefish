# apefish

A chess engine, written in Rust as a Cargo workspace:

- `engine/` (`apefish-engine`) - the engine itself: board representation, move
  generation, search, eval, exposed through the `Engine` trait.
- `cli/` (`apefish-cli`) - a terminal front-end for playing against the engine
  locally.

## Build

```sh
cargo build              # whole workspace, debug
cargo build --release    # whole workspace, optimized - use this for anything
                          # performance-sensitive (perft, benchmarks, real games)
cargo build -p apefish-engine   # just the engine
```

## Test

```sh
cargo test                      # whole workspace
cargo test -p apefish-engine    # just the engine's tests
```

The engine's test suite (`engine/tests/perft.rs`) checks move generation
correctness via [perft](https://www.chessprogramming.org/Perft) against
known-good node counts from two independently verified sources: the
chessprogramming.org "Perft Results" position set and Martin Sedlak's
targeted move generator test positions. Run in `--release` these still
finish in well under a second; in debug they're significantly slower but
still fine for routine use.

On a node-count mismatch, the failing test auto-bisects against a local
Stockfish binary to isolate the exact node and move that diverges, printing
the trace as part of the test output. This needs `stockfish` on `PATH` (or
`STOCKFISH_BIN=/path/to/it` set); if it's not found, bisection is skipped
with a note and the test just fails on the count as normal.

For ad-hoc bisection outside the test suite (e.g. a position/depth that
isn't one of the existing cases), use the standalone example:

```sh
cargo run --release -p apefish-engine --example perft_divide -- <depth> "<fen>" [uci_move ...]
```

## Benchmark

```sh
cargo bench -p apefish-engine --bench perft_bench
```

Measures move generation throughput (nodes/sec) via perft over the same
well-known positions the correctness tests use, reported by
[Criterion](https://bheisler.github.io/criterion.rs/book/) as "thrpt"
(elements/sec = nodes/sec). This is about *speed*, not correctness - node
counts aren't asserted here, only used to compute throughput.

To check whether an engine change made move generation faster or slower,
save a baseline before the change and compare against it after:

```sh
cargo bench -p apefish-engine --bench perft_bench -- --save-baseline before
# ... make your change ...
cargo bench -p apefish-engine --bench perft_bench -- --baseline before
```

The second run prints a regressed/improved percentage per position instead
of just an absolute number. `target/criterion/report/index.html` has the
full report with plots.

## Run

```sh
cargo run -p apefish-cli
```

Starts the terminal front-end for playing against the engine locally. This
is still early/hardcoded (see `cli/src/main.rs`) - a UCI adapter and other
front-ends are expected to follow, driving the same `Engine` trait.
