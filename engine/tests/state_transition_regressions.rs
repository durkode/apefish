//! Regression tests for specific engine bugs found via code review, each
//! isolated to a minimal position and a single `make_move`, checked directly
//! against the resulting `fen()` - deliberately narrower than the whole-tree
//! perft/game-status suites, so a failure here points straight at the
//! specific mechanism (a particular move type's board-state update) rather
//! than surfacing as an aggregate node-count or draw-detection mismatch.
//!
//! Only the public `Engine` trait is used, consistent with perft.rs's and
//! game_status.rs's own rule: this file doesn't reach into `Position`,
//! `MoveGen`, or any other internal type.
//!
//! Every expected fen below was verified against a local Stockfish binary
//! (`position fen ... moves ... / d`, reading its `Fen:` line).

use apefish_engine::basetypes::GameStatus;
use apefish_engine::{Apefish, Engine, InputMove};

mod common;
use common::parse_uci_move;

/// Apply a sequence of UCI move strings (e.g. "e2e4", "b7a8q") to `engine`,
/// resolving each against the engine's own legal moves at the time it's
/// played (so an illegal-move bug in `make_move`/`legal_moves` would surface
/// here too, not just in the final `fen()`/`game_status()` assertion).
fn play(engine: &mut Apefish, moves: &[&str]) {
    for uci in moves {
        let mv = parse_uci_move(engine, uci);
        engine
            .make_move(InputMove { from: mv.from(), to: mv.to(), promotion: mv.promotion() })
            .unwrap_or_else(|_| panic!("`{uci}` rejected by make_move"));
    }
}

fn fen_after(start_fen: &str, moves: &[&str]) -> String {
    let mut engine = Apefish::new();
    engine.set_position(Some(start_fen), &[]);
    play(&mut engine, moves);
    engine.fen()
}

/// A pawn promoting by capturing an enemy piece corrupted the board:
/// `apply_change_log_to_board` applies a move's change-log entries in the
/// order they were appended, but a promotion-with-capture appends them in
/// the opposite relative order to a normal capture (pawn-removal, then
/// promoted-piece-added, then captured-piece-removed). Applied in that
/// order, the captured piece was cleared from the destination square
/// *after* the promoted piece had already been placed there, and since
/// `remove_piece` unconditionally clears `piece_by_square` at that square,
/// it wiped the just-placed promoted piece back out of the mailbox (while
/// its bitboard bit stayed set).
///
/// This is a narrower, single-move companion to the multi-ply
/// `regression_promotion_with_capture` perft suite in perft.rs, which also
/// catches the downstream cascade (the corrupted mailbox desyncing from the
/// bitboards feeds wrong results into every later `legal_moves()` trial that
/// touches the same square) - this test just pins down the immediate,
/// one-move symptom directly.
#[test]
fn promotion_with_capture_leaves_correct_board() {
    let fen = fen_after("r1r4k/1P6/8/8/8/8/8/4K3 w - - 0 1", &["b7a8q"]);
    assert_eq!(fen, "Q1r4k/8/8/8/8/8/8/4K3 b - - 0 1");
}

/// Castling rights weren't revoked when a rook was captured on its home
/// square - `Position::make_move` only calls
/// `CastlingRights::remove_rights_for_move` with the *mover's own*
/// side/from-square/piece; it never checks the captured piece/square
/// against the *other* side's rook home squares. So a rook captured in
/// place (never having moved itself) left that side's castling right
/// standing.
///
/// White has only kingside rights ("K") on its h1 rook; black's g2 bishop
/// captures it. The right must disappear from the resulting fen's castling
/// field.
#[test]
fn castling_rights_lost_when_rook_is_captured() {
    let fen = fen_after("4k3/8/8/8/8/8/6b1/4K2R b K - 0 1", &["g2h1"]);
    assert_eq!(fen, "4k3/8/8/8/8/8/8/4K2b w - - 0 2");
}

/// A plain (non-capturing) king move must drop *both* of that side's
/// castling rights, not just as a side effect of a capture test elsewhere.
/// Isolated separately from `castling_rights_lost_when_rook_is_captured`
/// so a failure here points specifically at the king-move path rather than
/// the capture path.
#[test]
fn castling_rights_lost_when_king_moves() {
    let fen = fen_after("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1", &["e1e2"]);
    assert_eq!(fen, "4k3/8/8/8/8/8/4K3/R6R b - - 1 1");
}

/// A plain (non-capturing) rook move must drop only *that rook's own*
/// castling right, leaving the other side's untouched.
#[test]
fn castling_rights_lost_when_queenside_rook_moves() {
    let fen = fen_after("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1", &["a1b1"]);
    assert_eq!(fen, "4k3/8/8/8/8/8/8/1R2K2R b K - 1 1");
}
#[test]
fn castling_rights_lost_when_kingside_rook_moves() {
    let fen = fen_after("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1", &["h1g1"]);
    assert_eq!(fen, "4k3/8/8/8/8/8/8/R3K1R1 b Q - 1 1");
}

/// `insufficient_material` (board.rs) walks a nested loop over (side, piece
/// kind) and returns as soon as it hits the first non-empty bitboard for a
/// Knight or Bishop, based on that side's *total* piece count rather than
/// having confirmed no other piece kind exists on either side. On paper that
/// reads as if a knight found before a same-side rook (in whatever order
/// `PieceKind::iter()` produces) could return the wrong answer without ever
/// examining the rook.
///
/// In practice this doesn't reproduce: every branch compares
/// `sides_pieces[side].num_pieces()` (a total, not a per-kind flag), so any
/// extra piece on that side - rook or otherwise - already pushes the count
/// past the "king + minor only" threshold regardless of which kind was
/// iterated first, and the existing `game_status.rs` insufficient-material
/// suite (knight-vs-knight, two-knights-vs-king, opposite/same-coloured
/// bishops, extra pawn) already exercises this exact code shape without
/// failing. This test doesn't reproduce a live bug - it's a hardening
/// regression for one more combination (a knight *and* a rook together on
/// the same side) in case a future refactor of that function loses the
/// total-count property that currently keeps it correct.
#[test]
fn knight_and_rook_together_is_not_insufficient_material() {
    let mut engine = Apefish::new();
    engine.set_position(Some("4k3/8/8/8/8/8/8/2N1K2R w - - 0 1"), &[]);
    assert_eq!(engine.game_status(), GameStatus::Ongoing);
}
