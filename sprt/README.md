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
cargo bench-sig                 # bench signature for commit messages
cargo smoke                     # ~1 min: no time losses / illegal moves / crashes
cargo sprt                      # working tree (uncommitted changes included) vs main
cargo sprt --dev my-branch      # a branch or commit vs main
cargo sprt --mode nonreg        # "doesn't lose Elo" test, e.g. for simplifications
cargo sprt --tc ltc             # long time control confirmation
cargo sprt --help               # all options
cargo rating                    # rough absolute Elo vs Stockfish levels, to ±100
```

The cargo aliases (in `.cargo/config.toml`) go through the `xtask` crate to
`sprt/sprt.sh`, `sprt/smoke.sh` and `sprt/rating.sh`. You can run the scripts directly with the
same options, and exit codes pass through unchanged. The bench signature isn't
`cargo bench`, because that runs the Criterion perft benchmarks.

`cargo sprt` exits with:
- `0`: H1 accepted (pass)
- `1`: H0 accepted (fail)
- `2`: undecided or error

Local runs append a row to `sprt/history.tsv` (gitignored; CI results go to
the `sprt-results` branch, see [CI](#ci)). Each run's PGN, full fastchess log,
`summary.txt` and `result.json` go to `sprt/runs/`.

## Rough rating

SPRT only compares two apefish builds. `cargo rating` estimates an absolute
number instead: it plays a gauntlet against the anchors in
[`anchors.tsv`](anchors.tsv) and fits apefish's Elo with their ratings held
fixed ([`rating.py`](rating.py), logistic model, 95% interval). Games
continue until that interval is within the target, so tighter targets cost
roughly quadratically more games.

```sh
cargo rating                        # working tree, until ±100 Elo, at 60+0.6
cargo rating --ref main --error 50  # about 4x the games
cargo rating --tc 10+0.1            # quicker, but skews the result (see below)
```

It only stops early once every anchor has played `--min-games` (default 10),
and gives up at `--max-games` per anchor (default 500). `summary.txt` says
which of the two ended the run.

The default anchors are Stockfish at `UCI_Elo` 1400, 1700, 2000 and 2300
(fetched from nixpkgs, or set `STOCKFISH`). Stockfish calibrates `UCI_Elo` at
60+0.6 against CCRL 40/4, so the result is on that scale and the default time
control matches it.

Treat the result as a ballpark figure:
- It's an engine-list rating, not FIDE. The two pools were never calibrated
  against each other, and engines play humans differently from how they play
  engines. Expect it to be a couple of hundred Elo off from a FIDE rating.
- Weakened Stockfish plays deliberately random bad moves, which isn't how a
  weaker engine plays. Its strength also drifts away from 60+0.6.
- An anchor scored outside 10–90% is flagged. Such a score says little, so
  edit `anchors.tsv` to bracket apefish more closely. `UCI_Elo` ranges from
  1320 to 3190. Any UCI engine with a known rating can also be added as a line.

Each run's games and `summary.txt` go to `sprt/runs/`, and a row is appended
to `sprt/rating-history.tsv` (gitignored).

## Bench signature

`apefish bench` runs a fixed-depth search over a built-in position set and
prints `<nodes> nodes <nps> nps`. The node count is deterministic, so:

- **Same bench as base:** the change cannot affect play. `sprt.sh` says so and
  stops (use `--force` to run anyway).
- **Different bench:** the search changed, so test it.

CI posts both benches on every PR and records the bench of every commit on
`main`, so you don't need to put it in commit messages. The bench also gives a
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

## CI

Two workflows:

| Workflow | Runs on | When | What |
|---|---|---|---|
| [`ci.yml`](../.github/workflows/ci.yml) | GitHub-hosted | every PR and push to `main` | `cargo test`, `cargo smoke`, bench comment on the PR, bench of each `main` commit recorded |
| [`sprt.yml`](../.github/workflows/sprt.yml) | self-hosted (`apefish-sprt`) | `sprt` label on a PR, or run manually | STC SPRT, then LTC automatically if STC passes |

**Running an SPRT on a PR:**
1. Check the "Bench" comment. If it says *Unchanged*, the PR can't affect play
   and doesn't need a test.
2. Add the `sprt` label. Also add `nonreg` first for a "doesn't lose Elo" test.
3. Results come back as:
   - the `sprt/stc` and `sprt/ltc` commit statuses
   - one PR comment per time control
   - the games and logs, as run artifacts

   The results are advisory and never block a merge. Each new push to the PR
   cancels the running test and starts a new one while the label is on.
4. To test any branch or commit without a PR, use Actions → SPRT → *Run
   workflow*.

**What is tested:** the PR's head against its merge base with `main`, so only
the PR's own changes are measured. If `main` has since gained changes to the
search, rebase the PR and re-run.

**Results history** lives on the `sprt-results` branch, which CI writes to:
- `history.tsv`: one row per CI SPRT run
- `bench.tsv`: the bench of every commit on `main`

Together these replace `Bench:` lines in commit messages. Local runs still
write to `sprt/history.tsv`, which is gitignored.

**Fork PRs** get tests, smoke and bench on GitHub's own runners, but never the
self-hosted runner. `sprt.yml` only runs for branches in this repository.

### Self-hosted runner

[`ci/runner.nix`](ci/runner.nix) is a NixOS module for the runner. It runs as
a separate `apefish-ci` system user with no access to `/home`, and caches in
`/var/lib/apefish-ci`. To install it:

1. Get a registration token: go to Settings → Actions → Runners → *New
   self-hosted runner*, and copy the value after `--token` in the
   `./config.sh` command. It expires after an hour, so do step 2 right after.
   Save it with no trailing newline:
   ```sh
   sudo install -d -m 700 /etc/apefish-ci
   echo -n 'A1B2C3…' | sudo tee /etc/apefish-ci/github-runner.token >/dev/null
   sudo chmod 600 /etc/apefish-ci/github-runner.token
   ```
2. Copy `sprt/ci/runner.nix` to `/etc/nixos/apefish-runner.nix`, add it to
   `imports` in `configuration.nix`, then run `sudo nixos-rebuild switch`.

   After registering, the runner uses its own credentials, which only let it
   run jobs. Leave the token file as it is: an expired token is harmless, but
   changing the file triggers a re-registration. If you later change `url`,
   `extraLabels` or `workDir`, or wipe `/var/lib/github-runner`, it needs a
   fresh token. This setup deliberately avoids a personal access token,
   because registering through one needs repository Administration write,
   which could delete the repo.
3. Check that it registered: `systemctl status github-runner-apefish-sprt`,
   and the runner should show as *Idle* under the repo's Settings → Actions →
   Runners.

Games are pinned to CPUs 2–11 (repo variable `SPRT_CPUS` overrides this),
which leaves cores 0–1 for the Lichess bot and the rest of the machine.

### Testing the CI scripts locally

[`ci/sprt-job.sh`](ci/sprt-job.sh) and [`ci/report.sh`](ci/report.sh) hold all
the CI logic. You can run them by hand with a stub `gh` on `PATH` and a local
bare repository as `origin`. Set `DEV`, `BASE`, `MODE`, `PR`, `GH_TOKEN` and
the `GITHUB_*` variables yourself.
