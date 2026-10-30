//! Time-budget regression guard.
//!
//! `go movetime N` must produce a move at roughly `N`, not well past it.
//! This guards the class of bug where the engine does work proportional to
//! how much it searched *after* its deadline, before replying — the search
//! itself is fine, but every reply lands late and a GUI match is lost on
//! time.
//!
//! Concretely: `get_pv` on the search-abort path cloned the whole
//! `Position`, and `Position`'s internal repetition map (`MultiSet`) leaked
//! one entry per node ever visited (`remove` decremented a count to zero
//! but never dropped the entry). So the clone — and thus every `bestmove` —
//! ran hundreds of ms late once a search got deep enough.
//!
//! Timing tests are machine- and load-sensitive, so this is deliberately
//! coarse: it targets gross overruns (≈2x+), not tight deadline accuracy.
//! The core assertion is *differential* — the overrun must not grow with
//! think time. A correct engine's post-deadline work (stack unwind + one
//! event send) is ~constant regardless of search size; an O(work-done) cost
//! on the abort path shows up as the long search overrunning far more than
//! the short one, and comparing the two cancels most common-mode noise.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use apefish_engine::search::{SearchLimits, SearchRootResult};
use apefish_engine::{Apefish, Engine, EngineEvent};

/// Wall-clock elapsed from the `go` call to the terminal `BestMove` event,
/// driving the async `Engine::go` synchronously.
fn time_to_bestmove(engine: &mut Apefish, movetime: Duration) -> (Duration, SearchRootResult) {
    let (tx, rx) = mpsc::channel();
    let limits = SearchLimits { movetime: Some(movetime), ..Default::default() };
    let start = Instant::now();
    engine.go(limits, Box::new(move |event| {
        let _ = tx.send(event);
    }));
    loop {
        match rx.recv().expect("search ended before emitting a BestMove event") {
            EngineEvent::BestMove(result) => return (start.elapsed(), result),
            EngineEvent::Info { .. } | EngineEvent::Stats { .. } => {}
        }
    }
}

/// Tactical positions where quiescence search has the most captures to chew
/// through, plus a quiet endgame where iterative deepening runs deepest.
const CLOCK_TEST_FENS: &[&str] = &[
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "8/8/8/4k3/8/8/4P3/4K3 w - - 0 1",
];

/// Under a game clock (the smoke test's 2+0.02), every search must reply well
/// within the remaining time. Waits with a timeout, so a search that never
/// stops fails the test rather than hanging `cargo test`.
#[test]
fn clock_search_replies_in_time_on_tactical_positions() {
    let clock = Duration::from_millis(2000);
    let inc = Duration::from_millis(20);
    // Losing half the clock on one move is already a broken time cutoff.
    let deadline = clock / 2;

    let mut engine = Apefish::new(16);
    for fen in CLOCK_TEST_FENS {
        engine.new_game();
        engine.set_position(Some(fen), &[]);

        let (tx, rx) = mpsc::channel();
        let limits = SearchLimits {
            wtime: Some(clock),
            btime: Some(clock),
            winc: Some(inc),
            binc: Some(inc),
            ..Default::default()
        };
        let start = Instant::now();
        engine.go(limits, Box::new(move |event| {
            if let EngineEvent::BestMove(result) = event {
                let _ = tx.send(result);
            }
        }));

        // Generous wait so a slow machine reports the elapsed time below
        // rather than a bare timeout.
        let result = rx
            .recv_timeout(clock * 5)
            .unwrap_or_else(|_| panic!("no bestmove within {:?} on {fen}: search did not stop", clock * 5));
        let elapsed = start.elapsed();

        assert!(result.best_move.is_some(), "no move returned on {fen}");
        assert!(
            elapsed < deadline,
            "search on {fen} took {elapsed:?} with {clock:?}+{inc:?} on the clock (limit {deadline:?})"
        );
    }
}

#[test]
fn movetime_overrun_stays_bounded_and_does_not_scale_with_think_time() {
    let mut engine = Apefish::new(64);

    // Warm up: the first search in the process pays one-off allocator / cold
    // TT costs that would otherwise land on whichever measurement runs first.
    let _ = time_to_bestmove(&mut engine, Duration::from_millis(150));

    let short = Duration::from_millis(500);
    let long = Duration::from_millis(5000);

    engine.new_game();
    let (short_elapsed, short_result) = time_to_bestmove(&mut engine, short);
    engine.new_game();
    let (long_elapsed, long_result) = time_to_bestmove(&mut engine, long);

    assert!(
        short_result.best_move.is_some() && long_result.best_move.is_some(),
        "search returned no move (short: {short_result:?}, long: {long_result:?})"
    );

    let short_overrun = short_elapsed.saturating_sub(short);
    let long_overrun = long_elapsed.saturating_sub(long);

    // Gross absolute bound: nothing should reply a full second past its
    // deadline. Catches a broken or missing time cutoff outright.
    assert!(
        long_overrun < Duration::from_millis(1000),
        "movetime {long:?} search replied {long_overrun:?} past its deadline \
         (elapsed {long_elapsed:?})"
    );

    // Differential bound: the long search does ~10x the work of the short
    // one. If its post-deadline cost scales with work done, its overrun
    // balloons relative to the short search's; a correct engine keeps the
    // two within a roughly constant margin of each other.
    let growth = long_overrun.saturating_sub(short_overrun);
    assert!(
        growth < Duration::from_millis(200),
        "post-deadline overrun grew from {short_overrun:?} (movetime {short:?}) \
         to {long_overrun:?} (movetime {long:?}); something on the search-abort \
         path costs O(nodes searched)"
    );
}
