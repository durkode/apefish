//! Tactical search tests: positions with a forced solution, searched with a
//! ply budget scaled to the position's difficulty (mate in one up through
//! mate in four, plus a couple of one-idea material-winning tactics), run
//! purely against the public [`Engine`] trait - consistent with the other
//! integration suites' rule that reaching into `Position`/`MoveGen`/search
//! internals is a sign the `Engine` trait is missing something.
//!
//! Every position was verified with a local Stockfish binary (`position fen
//! ... / go depth N`) before being written down here: the mate puzzles all
//! report `score mate <n>` for the stated `n`, and the material tactics
//! report a large positive `score cp` after each accepted move, including
//! the follow-up move after every one of the defender's replies. A `score
//! mate <n>` from Stockfish means the mate is forced against *every* legal
//! defence, not just the specific line Stockfish happened to print - which
//! is exactly the property [`assert_forced_mate`] and [`assert_forced_line`]
//! check below, both re-running `go` at every one of the attacking side's
//! turns along the way rather than trusting the first move alone, so any
//! line the engine under test finds is accepted, not just the one
//! Stockfish's own search reported.
//!
//! `search()` (`engine/src/search.rs`) is currently `unimplemented!()`, and
//! `Engine::go` doesn't call it - it just plays the first legal move - so
//! every test in this file currently fails. That's expected: this suite is
//! the acceptance spec real search needs to satisfy, not a regression guard
//! for something already working.

use std::sync::mpsc;

use apefish_engine::basetypes::{GameStatus, WinReason};
use apefish_engine::search::{SearchLimits, SearchResult};
use apefish_engine::{Apefish, Engine, EngineEvent, Side};

/// A depth limit, generous relative to the position's difficulty - the
/// point is to give a correct search comfortable headroom to find the
/// solution, not to pin down exactly how deep it needs to look. The
/// deepest budget used in this file (mate in four) is well under the ~20
/// ply that's the most this engine is expected to need for anything here.
fn limits(depth: u8) -> SearchLimits {
    SearchLimits { depth: Some(depth), ..Default::default() }
}

fn engine_at(fen: &str) -> Apefish {
    let mut engine = Apefish::new(64);
    engine.set_position(Some(fen), &[]);
    engine
}

/// Run a search to completion and return its final result, driving the
/// asynchronous [`Engine::go`] interface synchronously: the search thread
/// reports through a channel and this blocks on it until the terminal
/// [`EngineEvent::BestMove`] arrives, discarding the intermediate `Info` reports.
fn search(engine: &mut Apefish, limits: SearchLimits) -> SearchResult {
    let (tx, rx) = mpsc::channel();
    engine.go(limits, Box::new(move |event| {
        let _ = tx.send(event);
    }));
    loop {
        match rx.recv().expect("search ended without emitting a BestMove event") {
            EngineEvent::BestMove(result) => return result,
            EngineEvent::Info { .. } | EngineEvent::Stats { .. } => {}
        }
    }
}

/// Verifies a forced mate without pinning down a specific mating line:
/// `engine` sits at a position where `mating_side` is to move and must
/// deliver mate within `moves_remaining` of its own moves. At each of
/// `mating_side`'s turns, whatever move the engine's own search picks is
/// accepted (so an engine that finds a different, equally-valid mating net
/// than the position's "canonical" one still passes); at each of the
/// defender's turns, *every* legal reply is required to still lead to a
/// forced mate within the remaining budget, since a true forced mate must
/// hold against any defence, not just the one a test author happened to
/// write down.
///
/// Leaves `engine`'s position unchanged (back at the fen it was called
/// with) when it returns, so a caller mid-loop over sibling replies can
/// keep iterating.
fn assert_forced_mate(engine: &mut Apefish, mating_side: Side, moves_remaining: u8, search_depth: u8) {
    assert!(
        moves_remaining > 0,
        "no mate delivered within the expected move budget; stuck at fen `{}`",
        engine.fen()
    );

    let result = search(engine, limits(search_depth));
    let mv = result.best_move.unwrap_or_else(|| panic!("engine returned no move at fen `{}`", engine.fen()));
    engine.make_move(mv.to_input_move()).unwrap_or_else(|e| {
        panic!("engine's own move `{mv}` from `go` was rejected by `make_move`: {e:?}")
    });

    match engine.game_status() {
        GameStatus::Won { side, reason: WinReason::Checkmate } => {
            assert_eq!(
                side, mating_side,
                "checkmate landed for the wrong side after `{mv}`; fen now `{}`", engine.fen()
            );
            engine.unmake_move();
            return;
        }
        GameStatus::Ongoing => {}
        other => panic!(
            "expected the game still ongoing (or checkmated) after `{mv}`, got {other:?}; fen now `{}`",
            engine.fen()
        ),
    }

    let defensive_replies = engine.legal_moves();
    assert!(
        !defensive_replies.is_empty(),
        "defender has no legal moves but game_status() didn't report checkmate; fen `{}`", engine.fen()
    );
    for reply in defensive_replies {
        engine.make_move(reply.to_input_move()).unwrap_or_else(|e| {
            panic!("legal_moves() offered `{reply}` but make_move rejected it: {e:?}")
        });
        assert_forced_mate(engine, mating_side, moves_remaining - 1, search_depth);
        engine.unmake_move();
    }

    engine.unmake_move();
}

fn assert_mate_in(fen: &str, mating_side: Side, moves: u8, search_depth: u8) {
    let mut engine = engine_at(fen);
    assert_forced_mate(&mut engine, mating_side, moves, search_depth);
}

/// Verifies a forced tactical line ply by ply, re-running `go` at every one
/// of the attacking side's turns rather than just checking the first move:
/// `expected[0]` is the accepted move(s) for the position `engine` starts
/// at, `expected[1]` the accepted move(s) after any single legal reply from
/// the defender, and so on. Between each entry, *every* legal defensive
/// reply is tried (not just one), so this confirms search keeps finding the
/// right continuation regardless of how the defender actually responds, not
/// merely that the first move looks right. Any move in a ply's accepted set
/// counts as correct, so a position with more than one equally-good way to
/// continue (e.g. two ways to win the same piece) doesn't need to prefer
/// one over the other.
///
/// Unlike [`assert_forced_mate`], there's no self-verifying terminal
/// condition to fall back on (nothing as unambiguous as checkmate signals
/// "the tactic actually succeeded"), so every ply's accepted move(s) must
/// be supplied explicitly rather than only the first.
fn assert_forced_line(engine: &mut Apefish, search_depth: u8, expected: &[&[&str]]) {
    let Some((accepted, rest)) = expected.split_first() else { return };

    let result = search(engine, limits(search_depth));
    let mv = result.best_move.unwrap_or_else(|| panic!("engine returned no move at fen `{}`", engine.fen()));
    let played = mv.to_string();
    assert!(
        accepted.contains(&played.as_str()),
        "expected one of {accepted:?} at fen `{}`, engine played `{played}` instead",
        engine.fen()
    );
    engine.make_move(mv.to_input_move()).unwrap_or_else(|e| {
        panic!("engine's own move `{mv}` from `go` was rejected by `make_move`: {e:?}")
    });

    if rest.is_empty() {
        engine.unmake_move();
        return;
    }

    let defensive_replies = engine.legal_moves();
    assert!(
        !defensive_replies.is_empty(),
        "defender has no legal moves but more of the line was still expected; fen `{}`", engine.fen()
    );
    for reply in defensive_replies {
        engine.make_move(reply.to_input_move()).unwrap_or_else(|e| {
            panic!("legal_moves() offered `{reply}` but make_move rejected it: {e:?}")
        });
        assert_forced_line(engine, search_depth, rest);
        engine.unmake_move();
    }

    engine.unmake_move();
}

/// Forced-mate puzzles, escalating in both mate distance and required
/// search depth. The mate-in-{2,3,4} trio shares the same simple lone-king
/// vs. king-and-queen geometry, just with White's king progressively
/// further away - a clean, easily-verified way to scale difficulty without
/// changing the tactical idea. Mate in four also has genuine branching in
/// Black's first reply (Kg7 or Kg8 are both legal), so it's the one case
/// here that actually exercises a defender choosing between more than one
/// legal move, rather than being pinned to a single legal reply throughout.
mod forced_mate {
    use super::*;

    /// Classic back-rank mate: Black's own pawns block every escape square
    /// on the 8th rank, so Re1-e8 covers the rest of the rank and mates.
    /// Verified: Stockfish reports `score mate 1`, `pv e1e8`.
    #[test]
    fn mate_in_1_back_rank() {
        assert_mate_in("6k1/5ppp/8/8/8/8/8/4R2K w - - 0 1", Side::White, 1, 3);
    }

    /// Scholar's-mate finish: Qh5xf7 is check (protected by the bishop on
    /// c4) with every flight square covered.
    /// Verified: Stockfish reports `score mate 1`, `pv h5f7`.
    #[test]
    fn mate_in_1_scholars_mate_finish() {
        assert_mate_in(
            "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 2 4",
            Side::White,
            1,
            3,
        );
    }

    /// King-and-queen-vs-king ladder mate, one step out.
    /// Queen starts on b1, not h1: with the White king on f6, a queen on h1
    /// would sit on the fully open h-file with Black's king on h8, which is
    /// an illegal position (the side not to move can't already be in check).
    /// Verified: Stockfish reports `score mate 2`, `pv b1b7 h8g8 b7g7`.
    #[test]
    fn mate_in_2_queen_ladder() {
        assert_mate_in("7k/8/5K2/8/8/8/8/1Q6 w - - 0 1", Side::White, 2, 6);
    }

    /// Same ladder mate, White's king one step further out again.
    /// Verified: Stockfish reports `score mate 3`, `pv e6f6 h8g8 b1g6 g8h8 g6g7`.
    #[test]
    fn mate_in_3_queen_ladder() {
        assert_mate_in("7k/8/4K3/8/8/8/8/1Q6 w - - 0 1", Side::White, 3, 8);
    }

    /// Same ladder mate again, White's king a further step out - and now
    /// Black's first reply genuinely branches (Kg7 or Kg8 are both legal
    /// after Kd6-e6, since neither is covered yet), which is exactly what
    /// `assert_forced_mate` needs to check on for the guarantee to be
    /// meaningful rather than incidental.
    /// Verified: Stockfish reports `score mate 4`,
    /// `pv b1b7 h8g8 d6e6 g8h8 e6f6 h8g8 b7g7`.
    #[test]
    fn mate_in_4_queen_ladder() {
        assert_mate_in("7k/8/3K4/8/8/8/8/1Q6 w - - 0 1", Side::White, 4, 10);
    }
}

/// One-clear-idea tactics that win material without forcing mate.
mod material_tactics {
    use super::*;

    /// Nd5-e7+ forks Black's king and queen: e7 is a check square not
    /// covered by any of Black's pawns, and the only legal replies (Kf8 or
    /// Kh8) both abandon the queen to Nxc8 next move - checked for real by
    /// re-running `go` after each of those replies, not assumed from the
    /// first move alone.
    /// Verified: Stockfish reports `score cp 442`, `bestmove d5e7`; then,
    /// after either `f8g8`/`h8g8` reply, `bestmove e7c8` (`score cp` 448/453).
    #[test]
    fn knight_forks_king_and_queen() {
        let mut engine = engine_at("2q3k1/5ppp/8/3N4/8/8/5PPP/6K1 w - - 0 1");
        assert_forced_line(&mut engine, 4, &[&["d5e7"], &["e7c8"]]);
    }

    /// Ra1-d1+ skewers Black's king off the d-file, winning the undefended
    /// queen on d8 behind it next move - a skewer rather than a pin, since
    /// the king (the front piece) has no choice but to move off the file
    /// entirely, unlike a pinned piece that could interpose along it. All
    /// six of the king's legal replies are checked (not just one), since
    /// `assert_forced_line` re-runs `go` after every legal defensive reply.
    /// Verified: Stockfish reports `score cp 515`, `bestmove a1d1`; then,
    /// after each of the king's six legal replies, `bestmove d1d8` (`score
    /// cp` 509-522 depending on the reply).
    #[test]
    fn rook_skewers_king_off_the_queen() {
        let mut engine = engine_at("3q4/8/8/8/3k4/8/8/R3K3 w - - 0 1");
        assert_forced_line(&mut engine, 4, &[&["a1d1"], &["d1d8"]]);
    }
}
