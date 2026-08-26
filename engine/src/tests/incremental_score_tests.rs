//! Incremental phase-score and tapered-eval correctness tests, run directly
//! against `Position` (same rationale as `zobrist_tests.rs`: `Position` can
//! only be constructed from inside the crate, since `Position::new` needs a
//! private `ZobristRandoms`).
//!
//! Both scores are updated from the exact same `piece_change_log` inside a
//! single `make_move` call (see `board.rs`), so most scenarios below assert
//! on both together rather than duplicating the FEN/move setup per score.
//!
//! Three properties are exercised:
//! - `fen_setup` initializes both scores correctly - checked against
//!   hand-computed expectations (summing `psqt::value`/known piece weights
//!   directly), not just self-consistency. A bug shared between the
//!   initialization path (`initialise_incrementally_updated_fields`) and the
//!   incremental path (`incremental_phase_score`/`incremental_eval`) would
//!   otherwise be invisible to a "matches fresh recomputation" check alone.
//! - `make_move` updates both scores by exactly the expected delta.
//! - `unmake_move` restores both scores to their exact pre-move value.

use std::sync::Arc;

use crate::basetypes::{UnvalidatedMove, Move, Piece, PieceKind, Side, Square};
use crate::board::Position;
use crate::fen;
use crate::movegen::MoveGen;
use crate::psqt::{self, TaperedValue};
use crate::zobrist::ZobristRandoms;

fn new_position() -> Position {
    Position::new(Arc::new(ZobristRandoms::new()), Arc::new(MoveGen::init()))
}

fn position_from_fen(fen_str: &str) -> Position {
    let mut position = new_position();
    position.fen_setup(fen_str).unwrap();
    position
}

/// Resolves a UCI move string ("e2e4", "e7e8q", ...) against the current
/// board contents. Duplicated from `zobrist_tests.rs` rather than shared,
/// consistent with that file's own choice not to factor this out.
fn uci(position: &Position, s: &str) -> Move {
    let from = Square::from_string(&s[0..2]).unwrap_or_else(|_| panic!("bad square in `{s}`"));
    let to = Square::from_string(&s[2..4]).unwrap_or_else(|_| panic!("bad square in `{s}`"));
    let promotion = s.chars().nth(4).map(|c| match c {
        'q' => PieceKind::Queen,
        'r' => PieceKind::Rook,
        'b' => PieceKind::Bishop,
        'n' => PieceKind::Knight,
        _ => panic!("bad promotion char in `{s}`"),
    });
    UnvalidatedMove { from, to, promotion }.to_internal_move(position).unwrap_or_else(|_| panic!("`{s}` not resolvable against current board"))
}

fn square(s: &str) -> Square {
    Square::from_string(s).unwrap()
}

fn piece(side: Side, kind: PieceKind) -> Piece {
    Piece { side, kind }
}

/// Asserts the position's incrementally-maintained phase score and tapered
/// eval both match a from-scratch recomputation for the same FEN - the
/// strongest check that the incremental path hasn't drifted from what a full
/// recomputation would give.
fn assert_scores_match_fresh_recomputation(position: &Position) {
    let recomputed = position_from_fen(&position.fen());
    assert_eq!(
        position.state.phase_score, recomputed.state.phase_score,
        "incremental phase score doesn't match a from-scratch recomputation for fen `{}`", position.fen()
    );
    assert_eq!(
        position.state.tapered_eval, recomputed.state.tapered_eval,
        "incremental tapered eval doesn't match a from-scratch recomputation for fen `{}`", position.fen()
    );
}

// ---------------------------------------------------------------------
// Initialization anchors: hand-computed, not just self-consistent.
// ---------------------------------------------------------------------

#[test]
fn starting_position_phase_score_is_24() {
    let position = new_position();
    assert_eq!(position.state.phase_score, 24);
}

#[test]
fn bare_kings_phase_score_is_zero() {
    let position = position_from_fen("8/8/8/8/8/8/8/K6k w - - 0 1");
    assert_eq!(position.state.phase_score, 0);
}

#[test]
fn starting_position_tapered_eval_is_zero() {
    // Left-right symmetric, and `psqt::value` is mirror-negated between
    // white and black (see `psqt.rs`'s own tests) - so every piece's
    // contribution is exactly cancelled by its mirror image.
    let position = new_position();
    assert_eq!(position.state.tapered_eval, TaperedValue::new(0, 0));
}

#[test]
fn custom_fen_initialization_matches_hand_computed_values() {
    let position = position_from_fen("8/8/8/4n3/4R3/8/8/K6k w - - 0 1");

    // rook(2) + knight(1), kings contribute 0.
    assert_eq!(position.state.phase_score, 3);

    let expected_eval = psqt::value(piece(Side::White, PieceKind::Rook), square("e4"))
        + psqt::value(piece(Side::Black, PieceKind::Knight), square("e5"))
        + psqt::value(piece(Side::White, PieceKind::King), square("a1"))
        + psqt::value(piece(Side::Black, PieceKind::King), square("h1"));
    assert_eq!(position.state.tapered_eval, expected_eval);
}

// ---------------------------------------------------------------------
// Per move-type scenarios: make (changed by the expected amount, matches
// fresh recomputation), then unmake (restored exactly).
// ---------------------------------------------------------------------

#[test]
fn quiet_move_updates_tapered_eval_by_psqt_delta_and_restores_on_unmake() {
    let mut position = new_position();
    let phase_before = position.state.phase_score;
    let eval_before = position.state.tapered_eval;

    let m = uci(&position, "g1f3");
    position.make_move(m).unwrap();

    // The knight stays on the board - phase is untouched by quiet moves.
    assert_eq!(position.state.phase_score, phase_before);
    let expected_eval = eval_before - psqt::value(piece(Side::White, PieceKind::Knight), square("g1"))
        + psqt::value(piece(Side::White, PieceKind::Knight), square("f3"));
    assert_eq!(position.state.tapered_eval, expected_eval);
    assert_scores_match_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.state.phase_score, phase_before);
    assert_eq!(position.state.tapered_eval, eval_before);
}

#[test]
fn capture_decreases_phase_score_by_captured_pieces_weight_and_restores_on_unmake() {
    let mut position = position_from_fen("8/8/8/4n3/4R3/8/8/K6k w - - 0 1");
    let phase_before = position.state.phase_score;
    let eval_before = position.state.tapered_eval;

    let m = uci(&position, "e4e5");
    position.make_move(m).unwrap();

    // Only the captured knight leaves the board - the capturing rook
    // doesn't - so the phase delta is exactly the knight's weight (1), not
    // the rook's.
    assert_eq!(position.state.phase_score, phase_before - 1);
    let expected_eval = eval_before - psqt::value(piece(Side::Black, PieceKind::Knight), square("e5"))
        - psqt::value(piece(Side::White, PieceKind::Rook), square("e4"))
        + psqt::value(piece(Side::White, PieceKind::Rook), square("e5"));
    assert_eq!(position.state.tapered_eval, expected_eval);
    assert_scores_match_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.state.phase_score, phase_before);
    assert_eq!(position.state.tapered_eval, eval_before);
}

#[test]
fn en_passant_capture_leaves_phase_score_unchanged_and_removes_captured_pawns_own_square_value() {
    let mut position = position_from_fen("8/8/8/3pP3/8/8/8/K6k w - d6 0 1");
    let phase_before = position.state.phase_score;
    let eval_before = position.state.tapered_eval;

    let m = uci(&position, "e5d6");
    position.make_move(m).unwrap();

    // Pawns carry zero phase weight, so an en passant capture - despite
    // removing a piece from the board - must not move the phase score.
    assert_eq!(position.state.phase_score, phase_before);

    // The captured pawn sat on d5, not on the destination square d6.
    let expected_eval = eval_before - psqt::value(piece(Side::Black, PieceKind::Pawn), square("d5"))
        - psqt::value(piece(Side::White, PieceKind::Pawn), square("e5"))
        + psqt::value(piece(Side::White, PieceKind::Pawn), square("d6"));
    assert_eq!(position.state.tapered_eval, expected_eval);
    assert_scores_match_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.state.phase_score, phase_before);
    assert_eq!(position.state.tapered_eval, eval_before);
}

#[test]
fn promotion_increases_phase_score_and_replaces_pawn_value_with_promoted_piece_and_restores_on_unmake() {
    let mut position = position_from_fen("8/P6k/8/8/8/8/7K/8 w - - 0 1");
    let phase_before = position.state.phase_score;
    let eval_before = position.state.tapered_eval;
    assert_eq!(phase_before, 0);

    let m = uci(&position, "a7a8q");
    position.make_move(m).unwrap();

    // The pawn's own delta is 0, so the phase change is entirely the
    // promoted queen's weight.
    assert_eq!(position.state.phase_score, phase_before + 4);
    let expected_eval = eval_before - psqt::value(piece(Side::White, PieceKind::Pawn), square("a7"))
        + psqt::value(piece(Side::White, PieceKind::Queen), square("a8"));
    assert_eq!(position.state.tapered_eval, expected_eval);
    assert_scores_match_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.state.phase_score, phase_before);
    assert_eq!(position.state.tapered_eval, eval_before);
}

#[test]
fn capture_promotion_combines_both_deltas_and_restores_on_unmake() {
    let mut position = position_from_fen("1n5k/P7/8/8/8/8/7K/8 w - - 0 1");
    let phase_before = position.state.phase_score;
    let eval_before = position.state.tapered_eval;
    assert_eq!(phase_before, 1); // just the black knight

    let m = uci(&position, "a7b8q");
    position.make_move(m).unwrap();

    // -1 for the captured knight, +4 for the promoted queen, 0 for the pawn itself.
    assert_eq!(position.state.phase_score, phase_before - 1 + 4);
    let expected_eval = eval_before - psqt::value(piece(Side::Black, PieceKind::Knight), square("b8"))
        - psqt::value(piece(Side::White, PieceKind::Pawn), square("a7"))
        + psqt::value(piece(Side::White, PieceKind::Queen), square("b8"));
    assert_eq!(position.state.tapered_eval, expected_eval);
    assert_scores_match_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.state.phase_score, phase_before);
    assert_eq!(position.state.tapered_eval, eval_before);
}

#[test]
fn castling_moves_both_king_and_rook_and_restores_on_unmake() {
    let mut position = position_from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    let phase_before = position.state.phase_score;
    let eval_before = position.state.tapered_eval;

    let m = uci(&position, "e1g1");
    position.make_move(m).unwrap();

    // Both pieces stay on the board, just change squares - phase is untouched.
    assert_eq!(position.state.phase_score, phase_before);
    let expected_eval = eval_before - psqt::value(piece(Side::White, PieceKind::King), square("e1"))
        + psqt::value(piece(Side::White, PieceKind::King), square("g1"))
        - psqt::value(piece(Side::White, PieceKind::Rook), square("h1"))
        + psqt::value(piece(Side::White, PieceKind::Rook), square("f1"));
    assert_eq!(position.state.tapered_eval, expected_eval);
    assert_scores_match_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.state.phase_score, phase_before);
    assert_eq!(position.state.tapered_eval, eval_before);
}

// ---------------------------------------------------------------------
// Multi-ply: a mixed sequence of quiet moves and captures, checked after
// every ply and unwound one ply at a time.
// ---------------------------------------------------------------------

/// Ruy Lopez Exchange up to the recapture (1.e4 c5 2.Nf3 Nc6 3.Bb5 a6
/// 4.Bxc6 dxc6), the same sequence `zobrist_tests.rs` uses for its own
/// multi-ply check.
#[test]
fn multi_ply_sequence_matches_fresh_recomputation_and_unwinds_correctly() {
    let mut position = position_from_fen(fen::STARTING_FEN);
    let moves = ["e2e4", "c7c5", "g1f3", "b8c6", "f1b5", "a7a6", "b5c6", "d7c6"];

    let mut history = vec![(position.state.phase_score, position.state.tapered_eval)];
    for uci_str in moves {
        let m = uci(&position, uci_str);
        position.make_move(m).unwrap();
        assert_scores_match_fresh_recomputation(&position);
        history.push((position.state.phase_score, position.state.tapered_eval));
    }

    for (expected_phase, expected_eval) in history[..history.len() - 1].iter().rev() {
        position.unmake_move();
        assert_eq!(position.state.phase_score, *expected_phase);
        assert_eq!(position.state.tapered_eval, *expected_eval);
    }
}
