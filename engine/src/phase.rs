use strum::EnumCount;

use crate::{PieceKind, basetypes::PerPiece, board::PieceChange};

pub type PhaseScore = u8;
type PhaseDelta = i8;

/// Mapping of PieceKind => int, using the phase value weights to determine
/// weighting from middle game to ending. Idea being sum of these in range [0, 24]
/// and taper over that.
const FRUIT_PHASE_DELTA: PerPiece<PhaseDelta> = {
    let mut values = [0i8; PieceKind::COUNT];
    values[PieceKind::Pawn as usize] = 0;
    values[PieceKind::Knight as usize] = 1;
    values[PieceKind::Bishop as usize] = 1;
    values[PieceKind::Rook as usize] = 2;
    values[PieceKind::Queen as usize] = 4;
    values[PieceKind::King as usize] = 0;
    PerPiece::from_array(values)
};

pub fn incremental_phase_score(old_score: PhaseScore, piece_changes: &[PieceChange]) -> PhaseScore {
    let mut delta: PhaseDelta = 0;
    
    for ap in piece_changes {
        if ap.from.is_some() {
            delta -= FRUIT_PHASE_DELTA[ap.piece.kind];
        }
        if ap.to.is_some() {
            delta += FRUIT_PHASE_DELTA[ap.piece.kind];
        }
    } 

    (old_score as PhaseDelta + delta) as PhaseScore
}
