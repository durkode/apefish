//! Game-status detection tests, run purely against the public [`Engine`] trait
//! (`set_position`, `make_move`, `game_status`, `fen`). Nothing here touches
//! `Position`, `MoveGen`, or any other internal type - consistent with
//! perft.rs's rule that a need to reach into engine internals is a sign the
//! `Engine` trait is missing something, not a reason to import them.
//!
//! Every position below was independently verified against a local Stockfish
//! binary (`position fen ... / d / go perft 1`, reading its `Checkers:` and
//! `Nodes searched:` output) before being written down here, so a test
//! failure means this engine's `game_status()` disagrees with an
//! independently-checked ground truth, not that the test fixture itself is
//! wrong. The two hand-known-famous positions (fool's mate, scholar's mate)
//! are additionally well-published FENs, not just Stockfish-checked.
//!
//! Sources for the non-hand-rolled positions:
//! - Fool's mate / Scholar's mate final positions: standard, widely-published
//!   opening traps.
//! - "Wrong-colour-bishop"-style K+Q vs K stalemate corner trap: the classic
//!   textbook stalemate example.
//! - FIDE insufficient-material rule (Article 5.2.2): king vs king; king and
//!   minor piece vs king; king and bishop vs king and bishop with same-colour
//!   bishops. Everything else (opposite-colour bishops, knight vs knight, two
//!   knights vs king, any pawn/rook/queen on the board) is deliberately
//!   *not* automatically drawn under that rule, and is tested as such.
//!
//! This suite intentionally does not fix any engine bug it finds - per the
//! task, a failing assertion here is the desired outcome when the engine's
//! behaviour disagrees with the verified expectation.

use apefish_engine::basetypes::{DrawReason, GameStatus, WinReason};
use apefish_engine::{Apefish, Engine, UnvalidatedMove, Side};

mod common;
use common::parse_uci_move;

/// Apply a sequence of UCI move strings (e.g. "e2e4", "e7e8q") to `engine`,
/// resolving each against the engine's own legal moves at the time it's
/// played (so illegal-move bugs in `make_move`/`legal_moves` would surface
/// here too, not just in the final `game_status()` assertion).
fn play(engine: &mut Apefish, moves: &[&str]) {
    for uci in moves {
        let mv = parse_uci_move(engine, uci);
        engine
            .make_move(UnvalidatedMove { from: mv.from(), to: mv.to(), promotion: mv.promotion() })
            .unwrap_or_else(|_| panic!("`{uci}` rejected by make_move"));
    }
}

fn status_at(fen: &str) -> GameStatus {
    let mut engine = Apefish::new(0);
    engine.set_position(Some(fen), &[]);
    engine.game_status()
}

fn assert_status(fen: &str, expected: GameStatus) {
    let actual = status_at(fen);
    assert_eq!(actual, expected, "fen `{fen}`: expected {expected:?}, got {actual:?}");
}

mod checkmate {
    use super::*;

    /// 1.f3 e5 2.g4 Qh4# - the fastest possible checkmate. White is mated.
    /// Verified: Stockfish reports `Checkers: h4`, `go perft 1` -> 0 nodes.
    #[test]
    fn fools_mate() {
        assert_status(
            "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3",
            GameStatus::Won { side: Side::Black, reason: WinReason::Checkmate },
        );
    }

    /// 1.e4 e5 2.Bc4 Nc6 3.Qh5 Nf6?? 4.Qxf7# - Black is mated.
    /// Verified: Stockfish reports `Checkers: f7`, `go perft 1` -> 0 nodes.
    #[test]
    fn scholars_mate() {
        assert_status(
            "r1bqkb1r/pppp1Qpp/2n2n2/4p3/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 0 4",
            GameStatus::Won { side: Side::White, reason: WinReason::Checkmate },
        );
    }

    /// Bare-king-and-pawns back-rank mate: Re8#, black king boxed in by its
    /// own pawns with no piece able to block or capture the rook.
    /// Verified: Stockfish reports `Checkers: e8`, `go perft 1` -> 0 nodes.
    #[test]
    fn back_rank_mate() {
        assert_status(
            "4R1k1/5ppp/8/8/8/8/8/6K1 b - - 0 1",
            GameStatus::Won { side: Side::White, reason: WinReason::Checkmate },
        );
    }

    /// Double check (rook along the h-file plus a knight both attacking the
    /// king simultaneously) with every escape square covered - only one
    /// attacker can be a block/capture target in a normal check, so this
    /// also exercises the "only the king may move" double-check rule.
    /// Verified: Stockfish reports `Checkers: h1 g6` (two attackers),
    /// `go perft 1` -> 0 nodes.
    #[test]
    fn double_check_mate() {
        assert_status(
            "7k/6p1/6N1/8/8/1B6/8/1K5R b - - 0 1",
            GameStatus::Won { side: Side::White, reason: WinReason::Checkmate },
        );
    }
}

mod stalemate {
    use super::*;

    /// The textbook K+Q vs K "wrong corner" stalemate: the black king on a8
    /// has no legal move (a7/b7/b8 all covered by the queen) and is not in
    /// check.
    /// Verified: Stockfish reports empty `Checkers:`, `go perft 1` -> 0 nodes.
    #[test]
    fn queen_corners_king() {
        assert_status(
            "k7/8/1Q6/8/8/8/8/7K b - - 0 1",
            GameStatus::Drawn { reason: DrawReason::Stalemate },
        );
    }

    /// Sanity check that stalemate and checkmate are actually distinguished
    /// by check status, not just by "zero legal moves": the classic K+R vs K
    /// stalemate blunder - white king h6 boxes in both of black's king-move
    /// squares (g7, h7) while the rook on g1 covers g8, but nothing attacks
    /// h8 itself.
    /// Verified: Stockfish reports empty `Checkers:`, `go perft 1` -> 0 nodes.
    #[test]
    fn cornered_king_without_check_is_stalemate_not_checkmate() {
        assert_status(
            "7k/8/7K/8/8/8/8/6R1 b - - 0 1",
            GameStatus::Drawn { reason: DrawReason::Stalemate },
        );
    }
}

/// FIDE Article 5.2.2 insufficient-material cases (automatic draw) and the
/// deliberately-not-covered neighbours (must stay `Ongoing`).
mod insufficient_material {
    use super::*;

    #[test]
    fn bare_kings() {
        assert_status(
            "8/8/4k3/8/8/4K3/8/8 w - - 0 1",
            GameStatus::Drawn { reason: DrawReason::InsufficientMaterial },
        );
    }

    #[test]
    fn king_and_bishop_vs_king() {
        assert_status(
            "8/8/4k3/8/8/4K3/8/2B5 w - - 0 1",
            GameStatus::Drawn { reason: DrawReason::InsufficientMaterial },
        );
    }

    #[test]
    fn king_and_knight_vs_king() {
        assert_status(
            "8/8/4k3/8/8/4K3/8/2N5 w - - 0 1",
            GameStatus::Drawn { reason: DrawReason::InsufficientMaterial },
        );
    }

    /// Bishops on c1 and e3 sit on the same diagonal, so they're the same
    /// square colour - the one same-side-bishop case FIDE 5.2.2 draws.
    #[test]
    fn same_coloured_bishops_each_side() {
        assert_status(
            "k7/8/8/8/8/4b3/8/K1B5 w - - 0 1",
            GameStatus::Drawn { reason: DrawReason::InsufficientMaterial },
        );
    }

    /// Bishops on c1 (dark) and g6 (light) are opposite-coloured - not one
    /// of FIDE's listed automatic draws, so this must stay `Ongoing` even
    /// though neither side can actually force mate.
    #[test]
    fn opposite_coloured_bishops_each_side_is_not_automatic_draw() {
        assert_status("k7/8/6b1/8/8/8/8/K1B5 w - - 0 1", GameStatus::Ongoing);
    }

    /// King and knight vs king and knight is not in FIDE's automatic-draw
    /// list (only a single king+minor vs bare king is), so this stays
    /// `Ongoing` too.
    #[test]
    fn knight_vs_knight_is_not_automatic_draw() {
        assert_status("k7/8/6n1/8/8/8/8/K1N5 w - - 0 1", GameStatus::Ongoing);
    }

    /// Two knights against a bare king is famously not forceable to mate in
    /// practice, but it is still not one of FIDE's listed automatic draws.
    #[test]
    fn two_knights_vs_king_is_not_automatic_draw() {
        assert_status("8/8/4k3/8/8/4K3/8/2N1N3 w - - 0 1", GameStatus::Ongoing);
    }

    /// A single pawn is enough to keep the game going.
    #[test]
    fn extra_pawn_is_not_insufficient_material() {
        assert_status("8/8/4k3/8/4P3/4K3/8/8 w - - 0 1", GameStatus::Ongoing);
    }
}

mod fifty_move_rule {
    use super::*;

    /// One half-move short of the threshold: still ongoing.
    #[test]
    fn ninety_nine_halfmoves_not_yet_drawn() {
        assert_status("8/8/4k3/8/8/4K3/8/4R3 w - - 99 1", GameStatus::Ongoing);
    }

    /// Exactly 100 half-moves (50 full moves) since the last pawn move or
    /// capture: drawn.
    #[test]
    fn hundred_halfmoves_draws() {
        assert_status(
            "8/8/4k3/8/8/4K3/8/4R3 w - - 100 1",
            GameStatus::Drawn { reason: DrawReason::FiftyMoveRule },
        );
    }

    /// Past the threshold should still be a draw, not only exactly-100 (in
    /// case the check is `==` instead of `>=`).
    #[test]
    fn past_the_threshold_still_draws() {
        assert_status(
            "8/8/4k3/8/8/4K3/8/4R3 w - - 150 1",
            GameStatus::Drawn { reason: DrawReason::FiftyMoveRule },
        );
    }

    /// A pawn push resets the clock to zero, so one ply before the old
    /// threshold would have fired, the position is ongoing again.
    #[test]
    fn pawn_move_resets_the_clock() {
        let mut engine = Apefish::new(0);
        engine.set_position(Some("8/8/4k3/8/4P3/4K3/8/8 w - - 99 1"), &[]);
        play(&mut engine, &["e4e5"]);
        assert_eq!(engine.game_status(), GameStatus::Ongoing);
    }

    /// A capture resets the clock to zero too, same as a pawn move.
    #[test]
    fn capture_resets_the_clock() {
        let mut engine = Apefish::new(0);
        engine.set_position(Some("7n/8/4k3/8/8/4K3/8/7R w - - 99 1"), &[]);
        play(&mut engine, &["h1h8"]);
        assert_eq!(engine.game_status(), GameStatus::Ongoing);
    }

    /// A quiet, non-pawn, non-capture move at 99 half-moves ticks the clock
    /// over to 100 and the position becomes drawn.
    #[test]
    fn quiet_move_ticks_clock_over_to_draw() {
        let mut engine = Apefish::new(0);
        engine.set_position(Some("8/8/3k4/8/8/4K3/8/4R3 w - - 99 1"), &[]);
        play(&mut engine, &["e3d2"]);
        assert_eq!(engine.game_status(), GameStatus::Drawn { reason: DrawReason::FiftyMoveRule });
    }
}

mod threefold_repetition {
    use super::*;

    /// Shuffling both knights out and back (Nf3/Ng8 then Ng1/Ng8) restores
    /// the exact starting position - same pieces, same side to move, same
    /// castling rights, no en passant square - for the second time (the
    /// starting position itself is the first occurrence), which is only two
    /// occurrences and not yet a draw.
    #[test]
    fn two_occurrences_is_not_yet_a_draw() {
        let mut engine = Apefish::new(0);
        play(&mut engine, &["g1f3", "g8f6", "f3g1", "f6g8"]);
        assert_eq!(engine.game_status(), GameStatus::Ongoing);
    }

    /// Repeating the same shuffle a second time brings the starting
    /// position to a third occurrence, which draws by threefold repetition.
    #[test]
    fn three_occurrences_draws() {
        let mut engine = Apefish::new(0);
        play(&mut engine, &["g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8"]);
        assert_eq!(
            engine.game_status(),
            GameStatus::Drawn { reason: DrawReason::ThreefoldRepetition }
        );
    }

    /// Edge case: repetition must compare the *whole* position, not just
    /// piece placement. Shuffling the h1 rook out and back to h1 looks like
    /// the starting position on the board, but the first Rg1 permanently
    /// forfeits White's kingside castling right, so the "rook back on h1"
    /// position after that is a genuinely different position from the
    /// original (which still had the right) - it must not be counted as a
    /// repeat of move 0.
    #[test]
    fn lost_castling_rights_are_not_the_same_position() {
        let mut engine = Apefish::new(0);
        engine.set_position(Some("4k3/8/8/8/8/8/8/4K2R w K - 0 1"), &[]);
        play(&mut engine, &["h1g1", "e8e7", "g1h1", "e7e8"]);
        // Rook and both kings are back where they started, but the castling
        // right is gone for good - this must read as only the *first*
        // occurrence of this (rights-less) position, not a repeat of the
        // original (rights-having) start position.
        assert_eq!(engine.fen(), "4k3/8/8/8/8/8/8/4K2R w - - 4 3");
        assert_eq!(engine.game_status(), GameStatus::Ongoing);
    }

    /// Continuing the same rook shuffle for two more round trips reaches a
    /// third occurrence of the *rights-less* "rook on g1" position (ply 1,
    /// 5, 9), which does draw - just three plies later than a naive
    /// piece-placement-only comparison would suggest.
    #[test]
    fn lost_castling_rights_position_still_eventually_repeats() {
        let mut engine = Apefish::new(0);
        engine.set_position(Some("4k3/8/8/8/8/8/8/4K2R w K - 0 1"), &[]);
        play(
            &mut engine,
            &["h1g1", "e8e7", "g1h1", "e7e8", "h1g1", "e8e7", "g1h1", "e7e8", "h1g1"],
        );
        assert_eq!(engine.fen(), "4k3/8/8/8/8/8/8/4K1R1 b - - 9 5");
        assert_eq!(
            engine.game_status(),
            GameStatus::Drawn { reason: DrawReason::ThreefoldRepetition }
        );
    }
}

/// The four end-of-game checks run in a specific order inside `game_status`
/// (checkmate/stalemate, then fifty-move, then repetition, then insufficient
/// material) - these pin that ordering down so it can't silently regress,
/// using positions engineered to satisfy two conditions at once.
mod status_priority {
    use super::*;

    /// A checkmated position whose halfmove clock also happens to be at the
    /// fifty-move threshold: checkmate must win, since the game is already
    /// over the instant mate is delivered, before any draw claim applies.
    #[test]
    fn checkmate_takes_priority_over_fifty_move_rule() {
        assert_status(
            "4R1k1/5ppp/8/8/8/8/8/6K1 b - - 100 1",
            GameStatus::Won { side: Side::White, reason: WinReason::Checkmate },
        );
    }

    /// Same idea for stalemate: a position with zero legal moves and no
    /// check must report `Stalemate`, not `FiftyMoveRule`, even when the
    /// halfmove clock is also past 100.
    #[test]
    fn stalemate_takes_priority_over_fifty_move_rule() {
        assert_status(
            "k7/8/1Q6/8/8/8/8/7K b - - 100 1",
            GameStatus::Drawn { reason: DrawReason::Stalemate },
        );
    }

    /// A king-and-bishop-vs-king position (insufficient material) whose
    /// halfmove clock is also at the fifty-move threshold: the fifty-move
    /// check runs first, so this should report `FiftyMoveRule`.
    #[test]
    fn fifty_move_rule_takes_priority_over_insufficient_material() {
        assert_status(
            "8/8/4k3/8/8/4K3/8/2B5 w - - 100 1",
            GameStatus::Drawn { reason: DrawReason::FiftyMoveRule },
        );
    }
}

mod ongoing {
    use super::*;

    #[test]
    fn starting_position() {
        assert_status(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            GameStatus::Ongoing,
        );
    }

    /// In check but with legal escape squares available - must not be
    /// mistaken for checkmate just because the side to move is in check.
    /// Verified: Stockfish reports `Checkers: e2`, `go perft 1` -> 3 nodes.
    #[test]
    fn in_check_with_escape_is_ongoing() {
        assert_status("4k3/8/8/8/8/8/4r3/4K3 w - - 0 1", GameStatus::Ongoing);
    }
}
