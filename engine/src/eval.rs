//! Static position evaluation.

use crate::{board::{AlteredPieces, PieceChange, Position}, psqt::{self, TaperedValue}};

/// Centipawn score, from the perspective of the side to move.
pub struct Score {
    opening_score: i32,
    endgame_score: i32
}

pub fn incremental_eval(old_eval: TaperedValue, pos: &Position, piece_changes: &[PieceChange]) -> TaperedValue {
    let mut eval = old_eval;
    
    // TODO: incrementally calculate phase
    for ap in piece_changes {
        if let Some(from) = ap.from {
            eval -= psqt::value(ap.piece, from);
        }
        if let Some(to) = ap.to {
            eval += psqt::value(ap.piece, to)
        }
    } 

    eval
}
