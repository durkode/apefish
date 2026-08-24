//! Verifies that `make_move` + `unmake_move` is an exact round trip - not
//! just "produces the right node count a few plies later" (perft.rs's
//! concern), but "the fen right after unmaking is byte-identical to the fen
//! right before making," checked at *every* node of the traversal, not just
//! the root.
//!
//! This is deliberately a different assertion to perft's: a state-
//! restoration bug can leave some piece of bookkeeping wrong without
//! immediately changing which moves are available at that exact node, so it
//! only shows up in perft as a wrong count several plies downstream, once
//! it's had a chance to cascade - requiring bisection to trace back to the
//! actual node/move where restoration first broke. Checking the round trip
//! directly at every node catches it right there instead.
//!
//! Reuses the same positions perft.rs already relies on (so the move
//! *counts* are independently known to be correct - see perft.rs for
//! sourcing) since the point here isn't move-generation correctness, it's
//! whether undoing a move gets back to exactly where you started.
//!
//! Only the public `Engine` trait is used, consistent with perft.rs's and
//! game_status.rs's own rule.

use apefish_engine::Apefish;

mod common;
use common::assert_make_unmake_round_trips;

fn assert_round_trips(fen: &str, depth: u32) {
    let mut engine = Apefish::new();
    assert_make_unmake_round_trips(&mut engine, fen, depth);
}

#[test]
fn startpos() {
    assert_round_trips("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 4);
}

/// Heavy on captures, pins, and castling - the richest of the six CPW
/// positions for exercising every move type's change-log/undo path.
#[test]
fn kiwipete() {
    assert_round_trips("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 3);
}

/// Promotion-heavy, including the promotion-with-capture case that used to
/// desync `piece_by_square` from the bitboards.
#[test]
fn position_4_promotion_heavy() {
    assert_round_trips("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 3);
}

/// Castling rights lost via rook capture - the second bug this file's
/// round-trip check is meant to guard against a repeat of.
#[test]
fn castling_rights_lost_to_rook_capture() {
    assert_round_trips("r3k2r/1b4bq/8/8/8/8/7B/R3K2R w KQkq - 0 1", 3);
}

/// Castling through/into check - lots of rejected castling candidates for
/// `legal_moves()` to trial internally, which is exactly what leaked the
/// history-stack push this file's helper is designed to catch.
#[test]
fn castling_prevented_by_attacked_squares() {
    assert_round_trips("r3k2r/8/5Q2/8/8/3q4/8/R3K2R w KQkq - 0 1", 3);
}

/// En passant, including the capture-availability window and the capture
/// itself removing two pawns from the board.
#[test]
fn illegal_en_passant_capture() {
    assert_round_trips("8/5bk1/8/2Pp4/8/1K6/8/8 w - d6 0 1", 4);
}
