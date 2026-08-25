//! Static position evaluation.

pub type Score = i32;

use crate::{board::PieceChange, psqt::{self, TaperedValue}};


pub fn incremental_eval(old_eval: TaperedValue, piece_changes: &[PieceChange]) -> TaperedValue {
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
