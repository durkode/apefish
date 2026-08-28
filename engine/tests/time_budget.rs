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

use apefish_engine::search::{SearchLimits, SearchResult};
use apefish_engine::{Apefish, Engine, EngineEvent};

/// Wall-clock elapsed from the `go` call to the terminal `BestMove` event,
/// driving the async `Engine::go` synchronously.
fn time_to_bestmove(engine: &mut Apefish, movetime: Duration) -> (Duration, SearchResult) {
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
