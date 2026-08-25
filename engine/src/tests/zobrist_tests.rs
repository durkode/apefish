//! Zobrist hashing correctness tests, run directly against `Position`
//! (rather than the public `Engine` trait like `tests/perft.rs` and
//! `tests/game_status.rs`) via `Position::get_zobrist()` - the hash itself
//! has no representation in the `Engine` trait, and is exactly the kind of
//! thing a black-box `Engine` test can't reach without one.
//!
//! Two properties are exercised throughout:
//! - `make_move` followed by `unmake_move` must restore the exact prior
//!   hash, including through castling, en passant and promotion, where the
//!   hash update touches more than just the moving piece.
//! - The incrementally-maintained hash must always match a hash computed
//!   completely from scratch (`fen_setup` on the resulting FEN) - the
//!   strongest check that incremental updates haven't drifted from what a
//!   full recomputation would give.

use std::sync::Arc;

use crate::basetypes::{InputMove, Move, PieceKind, Square};
use crate::board::Position;
use crate::fen;
use crate::movegen::MoveGen;
use crate::zobrist::{ZobristKey, ZobristRandoms};

fn new_position() -> (Position, MoveGen) {
    (Position::new(Arc::new(ZobristRandoms::new())), MoveGen::init())
}

fn position_from_fen(fen_str: &str) -> (Position, MoveGen) {
    let (mut position, movegen) = new_position();
    position.fen_setup(fen_str).unwrap();
    (position, movegen)
}

fn hash_of_fen(fen_str: &str) -> ZobristKey {
    position_from_fen(fen_str).0.get_zobrist()
}

/// Resolves a UCI move string ("e2e4", "e7e8q", ...) against the current
/// board contents. Mirrors what `tests/common/mod.rs::parse_uci_move` does
/// through the public `Engine` trait, but goes through
/// `InputMove::to_internal_move` directly since these tests work against
/// `Position`, not `Apefish`.
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
    InputMove { from, to, promotion }.to_internal_move(position).unwrap_or_else(|_| panic!("`{s}` not resolvable against current board"))
}

/// Asserts the position's incrementally-maintained hash matches a hash
/// computed completely from scratch for the same FEN.
fn assert_hash_matches_fresh_recomputation(position: &Position) {
    let recomputed = hash_of_fen(&position.fen());
    assert_eq!(
        position.get_zobrist(), recomputed,
        "incremental hash doesn't match a from-scratch recomputation for fen `{}`", position.fen()
    );
}

#[test]
fn quiet_move_make_then_unmake_restores_hash() {
    let (mut position, movegen) = new_position();
    let original_hash = position.get_zobrist();

    let m = uci(&position, "g1f3");
    position.make_move(&movegen, m).unwrap();
    assert_ne!(position.get_zobrist(), original_hash);
    assert_hash_matches_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.get_zobrist(), original_hash);
}

#[test]
fn capture_make_then_unmake_restores_hash() {
    let (mut position, movegen) = position_from_fen("8/8/8/3p4/4P3/8/8/K6k w - - 0 1");
    let original_hash = position.get_zobrist();

    let m = uci(&position, "e4d5");
    assert_eq!(m.captured(), Some(PieceKind::Pawn));
    position.make_move(&movegen, m).unwrap();
    assert_ne!(position.get_zobrist(), original_hash);
    assert_hash_matches_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.get_zobrist(), original_hash);
}

#[test]
fn en_passant_capture_make_then_unmake_restores_hash() {
    let (mut position, movegen) = position_from_fen("8/8/8/3pP3/8/8/8/K6k w - d6 0 1");
    let original_hash = position.get_zobrist();

    let m = uci(&position, "e5d6");
    assert!(m.en_passant());
    position.make_move(&movegen, m).unwrap();
    assert_ne!(position.get_zobrist(), original_hash);
    assert_hash_matches_fresh_recomputation(&position);
    // The captured pawn sat on d5, not on the destination square d6 -
    // confirm it's actually gone, not just that the hash moved.
    assert_eq!(position.fen(), "8/8/3P4/8/8/8/8/K6k b - - 0 1");

    position.unmake_move();
    assert_eq!(position.get_zobrist(), original_hash);
}

#[test]
fn white_kingside_castling_make_then_unmake_restores_hash() {
    let (mut position, movegen) = position_from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    let original_hash = position.get_zobrist();

    let m = uci(&position, "e1g1");
    assert!(m.castling());
    position.make_move(&movegen, m).unwrap();
    assert_ne!(position.get_zobrist(), original_hash);
    assert_hash_matches_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.get_zobrist(), original_hash);
}

#[test]
fn white_queenside_castling_make_then_unmake_restores_hash() {
    let (mut position, movegen) = position_from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    let original_hash = position.get_zobrist();

    let m = uci(&position, "e1c1");
    assert!(m.castling());
    position.make_move(&movegen, m).unwrap();
    assert_ne!(position.get_zobrist(), original_hash);
    assert_hash_matches_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.get_zobrist(), original_hash);
}

#[test]
fn black_kingside_castling_make_then_unmake_restores_hash() {
    let (mut position, movegen) = position_from_fen("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1");
    let original_hash = position.get_zobrist();

    let m = uci(&position, "e8g8");
    assert!(m.castling());
    position.make_move(&movegen, m).unwrap();
    assert_ne!(position.get_zobrist(), original_hash);
    assert_hash_matches_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.get_zobrist(), original_hash);
}

#[test]
fn black_queenside_castling_make_then_unmake_restores_hash() {
    let (mut position, movegen) = position_from_fen("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1");
    let original_hash = position.get_zobrist();

    let m = uci(&position, "e8c8");
    assert!(m.castling());
    position.make_move(&movegen, m).unwrap();
    assert_ne!(position.get_zobrist(), original_hash);
    assert_hash_matches_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.get_zobrist(), original_hash);
}

#[test]
fn promotion_make_then_unmake_restores_hash() {
    let (mut position, movegen) = position_from_fen("8/P6k/8/8/8/8/7K/8 w - - 0 1");
    let original_hash = position.get_zobrist();

    let m = uci(&position, "a7a8q");
    position.make_move(&movegen, m).unwrap();
    assert_ne!(position.get_zobrist(), original_hash);
    assert_hash_matches_fresh_recomputation(&position);
    // The hash must reflect a queen on a8, not a pawn - not just "some
    // piece changed on that square".
    assert_ne!(position.get_zobrist(), hash_of_fen("P7/8/8/8/8/8/7K/7k w - - 0 1"));

    position.unmake_move();
    assert_eq!(position.get_zobrist(), original_hash);
}

#[test]
fn capture_promotion_make_then_unmake_restores_hash() {
    let (mut position, movegen) = position_from_fen("1n5k/P7/8/8/8/8/7K/8 w - - 0 1");
    let original_hash = position.get_zobrist();

    let m = uci(&position, "a7b8q");
    assert_eq!(m.captured(), Some(PieceKind::Knight));
    position.make_move(&movegen, m).unwrap();
    assert_ne!(position.get_zobrist(), original_hash);
    assert_hash_matches_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.get_zobrist(), original_hash);
    // Unmake must restore the captured knight, not a pawn - the piece
    // sitting on the destination square right before unmake is the
    // promoted queen, and `previous_move.captured()` records what the
    // capture actually took.
    assert_eq!(position.fen(), "1n5k/P7/8/8/8/8/7K/8 w - - 0 1");
}

#[test]
fn double_pawn_push_sets_en_passant_hash_and_unmake_clears_it() {
    let (mut position, movegen) = position_from_fen(fen::STARTING_FEN);
    let original_hash = position.get_zobrist();

    let m = uci(&position, "e2e4");
    position.make_move(&movegen, m).unwrap();
    assert_eq!(position.state.en_passant, Some(Square::from_string("e3").unwrap()));
    assert_hash_matches_fresh_recomputation(&position);

    position.unmake_move();
    assert_eq!(position.state.en_passant, None);
    assert_eq!(position.get_zobrist(), original_hash);
}

#[test]
fn en_passant_availability_changes_hash_for_otherwise_identical_board() {
    let with_ep = hash_of_fen("8/8/8/3pP3/8/8/8/K6k w - d6 0 1");
    let without_ep = hash_of_fen("8/8/8/3pP3/8/8/8/K6k w - - 0 1");
    assert_ne!(with_ep, without_ep);
}

#[test]
fn castling_rights_change_hash_for_otherwise_identical_board() {
    let all_rights = hash_of_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    let no_white_kingside = hash_of_fen("r3k2r/8/8/8/8/8/8/R3K2R w Qkq - 0 1");
    let no_rights = hash_of_fen("r3k2r/8/8/8/8/8/8/R3K2R w - - 0 1");

    assert_ne!(all_rights, no_white_kingside);
    assert_ne!(all_rights, no_rights);
    assert_ne!(no_white_kingside, no_rights);
}

/// Rook shuffles out to g1 and back to h1, ending on the same square with
/// the same side to move as the start - but White's kingside right was
/// permanently forfeited the moment the rook first left h1. The hash
/// must not return to its original value even though every piece is back
/// on its starting square.
#[test]
fn losing_castling_rights_changes_hash_even_when_pieces_return_to_start_squares() {
    let (mut position, movegen) = position_from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1");
    let original_hash = position.get_zobrist();

    for uci_str in ["h1g1", "e8e7", "g1h1", "e7e8"] {
        let m = uci(&position, uci_str);
        position.make_move(&movegen, m).unwrap();
    }

    assert_eq!(position.fen(), "4k3/8/8/8/8/8/8/4K2R w - - 4 3");
    assert_ne!(position.get_zobrist(), original_hash);
    assert_hash_matches_fresh_recomputation(&position);
}

/// 1.Nf3 Nf6 2.Nc3 Nc6 and 1.Nc3 Nc6 2.Nf3 Nf6 reach the identical
/// position by two different move orders - the hash must agree, not
/// just the FEN.
#[test]
fn different_move_order_reaching_same_position_gives_same_hash() {
    let (mut pos_a, movegen) = position_from_fen(fen::STARTING_FEN);
    for uci_str in ["g1f3", "g8f6", "b1c3", "b8c6"] {
        let m = uci(&pos_a, uci_str);
        pos_a.make_move(&movegen, m).unwrap();
    }

    let (mut pos_b, _) = position_from_fen(fen::STARTING_FEN);
    for uci_str in ["b1c3", "b8c6", "g1f3", "g8f6"] {
        let m = uci(&pos_b, uci_str);
        pos_b.make_move(&movegen, m).unwrap();
    }

    assert_eq!(pos_a.fen(), pos_b.fen());
    assert_eq!(pos_a.get_zobrist(), pos_b.get_zobrist());
}

/// Ruy Lopez Exchange up to the recapture (1.e4 c5 2.Nf3 Nc6 3.Bb5 a6
/// 4.Bxc6 dxc6): a mixed sequence of quiet moves and captures, unmade one
/// ply at a time, each of which must land back on the exact hash that was
/// current before that ply was played.
#[test]
fn multi_ply_sequence_unmake_restores_hash_at_every_step() {
    let (mut position, movegen) = position_from_fen(fen::STARTING_FEN);
    let moves = ["e2e4", "c7c5", "g1f3", "b8c6", "f1b5", "a7a6", "b5c6", "d7c6"];

    let mut hashes = vec![position.get_zobrist()];
    for uci_str in moves {
        let m = uci(&position, uci_str);
        position.make_move(&movegen, m).unwrap();
        assert_hash_matches_fresh_recomputation(&position);
        hashes.push(position.get_zobrist());
    }

    for expected in hashes[..hashes.len() - 1].iter().rev() {
        position.unmake_move();
        assert_eq!(position.get_zobrist(), *expected);
    }
}
