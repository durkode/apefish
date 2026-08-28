NOTE: This repo is not ready for use yet. This readme is just an AI generated file, I will hand write and make clearer once it is ready for use.

The core of the engine is nearly all hand written, the UCI client, docs, tests, are nearly all AI generated for now, and need some cleanup before I'd put my name behind it.

# apefish

A chess engine, written in Rust as a Cargo workspace:

- `engine/` (`apefish-engine`) - the engine itself: board representation, move
  generation, search, eval, exposed through the `Engine` trait.
- `cli/` (`apefish-cli`) - a terminal front-end for playing against the engine
  locally, plus a [UCI](https://www.chessprogramming.org/UCI) adapter for use
  with UCI-compatible GUIs and tools.

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
is still early/hardcoded (see `cli/src/main.rs`) - other front-ends are
expected to follow, driving the same `Engine` trait.

### UCI mode

```sh
cargo build --release -p apefish-cli
./target/release/apefish --uci
```

Speaks the [UCI protocol](https://www.chessprogramming.org/UCI) over
stdin/stdout (see `cli/src/uci.rs`), so any UCI-compatible GUI or tool can
drive it by being pointed at the built `apefish` binary above. Point tools
at the binary directly rather than via `cargo run` - most of them invoke the
engine as a subprocess and don't go through Cargo.

Note: `go`'s search is currently a stub that returns the first legal move
instantly - the protocol itself is fully wired up ahead of real search
landing in `apefish-engine`, so games played this way won't be meaningful
yet, but the UCI handshake and move application can already be exercised
end-to-end.

`run_apefish_uci.sh` (repo root) wraps the two lines above into one command
- `./run_apefish_uci.sh` - for tools that just need something to launch
without having to remember to build `--release` or pass `--uci` themselves.
It resolves paths relative to its own location, so it can be invoked from
anywhere. Build the release binary before using it (see above); it doesn't
build it for you.

### Testing with cutechess-cli

[cutechess-cli](https://github.com/cutechess/cutechess) runs UCI engines
against each other from the command line, useful for sanity-checking the
UCI adapter or running test matches. Install it (package manager, or build
from source), then:

```sh
cutechess-cli \
  -engine cmd=./run_apefish_uci.sh name=apefish \
  -engine cmd=./run_apefish_uci.sh name=apefish-2 \
  -each proto=uci tc=40/60 \
  -rounds 1
```

This plays apefish against a second copy of itself. Swap the second
`-engine` line for another UCI engine's binary to test against something
else instead, e.g. Stockfish:

```sh
cutechess-cli \
  -engine cmd=./run_apefish_uci.sh name=apefish \
  -engine cmd=stockfish name=stockfish \
  -each proto=uci tc=40/60 \
  -rounds 1
```

### Playing against a human with Cute Chess

[Cute Chess](https://cutechess.com/) is the GUI counterpart to
`cutechess-cli`, for playing interactive games rather than running
automated matches:

1. `cargo build --release -p apefish-cli`
2. In Cute Chess: Tools → Settings → Engines → Add, then point it at
   `run_apefish_uci.sh` (repo root) with protocol "UCI" (name it e.g.
   `apefish`). The wrapper script already passes `--uci` for you, so unlike
   pointing Cute Chess straight at the `apefish` binary, no Arguments need
   to be set.
3. Game → New Game, set one side to "Human" and the other to the `apefish`
   engine, then play.
