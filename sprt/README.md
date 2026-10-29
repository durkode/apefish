# SPRT testing

Tells you whether a change made apefish stronger, weaker or neither, by playing
a "dev" build against a "base" build with
[fastchess](https://github.com/Disservin/fastchess) until a
[Sequential Probability Ratio Test](https://www.chessprogramming.org/Sequential_Probability_Ratio_Test)
can decide.

Fastchess and the opening book (`8moves_v3.pgn`) are fetched on first use.
Fastchess comes from `$FASTCHESS`, then `PATH`, then nixpkgs, and if none of
those is available it's built from source, which needs `make` and `g++`.

## Quick start

```sh
sprt/smoke.sh                     # ~1 min: no time losses / illegal moves / crashes
sprt/sprt.sh                      # working tree (uncommitted changes included) vs main
sprt/sprt.sh --dev my-branch      # a branch or commit vs main
sprt/sprt.sh --mode nonreg        # "doesn't lose Elo" test, e.g. for simplifications
sprt/sprt.sh --tc ltc             # long time control confirmation
```

`sprt/sprt.sh` exits with:
- `0`: H1 accepted (pass)
- `1`: H0 accepted (fail)
- `2`: undecided or error

It also appends a row to [`history.tsv`](history.tsv), which is committed. Each
run's PGN, full fastchess log and summary go to `sprt/runs/`, which is
gitignored.

## Bench signature

`apefish bench` runs a fixed-depth search over a built-in position set and
prints `<nodes> nodes <nps> nps`. The node count is deterministic, so:

- **Same bench as base:** the change cannot affect play. `sprt.sh` says so and
  stops (use `--force` to run anyway).
- **Different bench:** the search changed, so test it.

Put it in commit messages as `Bench: <nodes>`. The bench also gives a
quick NPS comparison for speed-only changes.

## When to run what

| Change | Test |
|---|---|
| Same bench (refactor, UCI plumbing, docs, tests) | None |
| Speedup with the same bench | Compare `apefish bench` NPS |
| Search / eval change meant to gain Elo | `sprt.sh` (STC gainer); merge on PASS |
| Pruning / reductions, anything that may scale with depth | STC gainer, then `--tc ltc` |
| Time management | `smoke.sh`, then STC **and** `--tc ltc` (never fixed depth) |
| Simplification / removing code | `sprt.sh --mode nonreg` |
| Bug fix that changes bench | `--mode nonreg`; merge regardless for crashes and illegal moves |
| Time management or UCI layer changes | `smoke.sh` first |

Only redeploy the Lichess bot (`docker/redeploy.sh`) from a `main` whose changes
have passed.

## Defaults and why

| Setting | Value | Reason |
|---|---|---|
| Bounds | gainer `[0, 20]`, nonreg `[-20, 0]` | Changes are expected to be worth 20+ Elo while the engine is young. Tighten with `--elo0/--elo1` as gains shrink. |
| Elo model | logistic | Bounds mean ordinary Elo. fastchess's default normalized Elo is roughly 2× larger at apefish's draw rates. |
| Error rates | α = β = 0.05 | Standard |
| STC | 8+0.08, 16 MB hash | |
| LTC | 40+0.4, 64 MB hash | About 5× slower per game |
| VSTC | 4+0.04, 8 MB hash | Quick checks. Only if `smoke.sh` passes at that speed. |
| Openings | `8moves_v3.pgn`, random order, each played with both colours | A balanced book suits a young engine; UHO books are for very strong engines. |
| Adjudication | draw after move 40 if \|score\| ≤ 10 cp for 8 moves; resign at ≥ 1000 cp for 3 moves | Saves time on decided games |
| Concurrency | cores − 2 | Leaves headroom so neither engine loses time to contention |
| Game cap | 20000 games | Ends as "undecided" (exit 2) |

**Run on a quiet machine.** Both scripts warn if `apefish-bot.service` is
running. A live Lichess game competes for CPU and distorts both engines' clocks.

## Adding to CI later

The scripts don't prompt and take everything from flags and env vars, so a CI
job only needs Rust and either `make` + `g++` or `FASTCHESS=/path`:

- **Every push/PR (cheap):** `cargo test` plus `sprt/smoke.sh --games 100`.
- **On demand (expensive):** `sprt/sprt.sh --dev $SHA --base main` on a
  self-hosted runner. Hosted runners have too few cores and noisy timing.

Cache `sprt/.tools`, `sprt/books` and `sprt/bins` between runs.
