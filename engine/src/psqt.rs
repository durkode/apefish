//! Piece-square tables.
//!
//! These are the well-known "simplified evaluation" values (Tomasz Michniewski's
//! PST set), not anything tuned for this engine. They exist purely as a reasonable
//! starting point to build the rest of eval around; expect these numbers to be
//! replaced by tuned values later.
//!
//! Each table comes as an (mg, eg) pair: a middlegame value and an endgame value.
//! There's no taper function yet to blend the two by game phase — [`value`] just
//! hands both back as a [`TaperedValue`] for a future taper step to combine.
//!
//! Base piece values are folded into every square, so `PSQT`/`value` hold each
//! piece's absolute worth on that square (material + position) rather than a
//! position-only delta — reading the table doesn't require knowing the piece
//! value separately, and it can be summed incrementally as the sole eval term.

use crate::{basetypes::{PerPiece, PerSquare, Piece, PieceKind, Side, Square}, phase::PhaseScore};
use strum::EnumCount;

/// Centipawn PST bonus for a single phase.
pub type PsqtValue = i32;

/// A middlegame/endgame pair of PST values, to be blended later by a taper function
/// based on how much material is left on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaperedValue {
    pub mg: PsqtValue,
    pub eg: PsqtValue,
}

impl TaperedValue {
    pub const fn new(mg: PsqtValue, eg: PsqtValue) -> Self {
        Self { mg, eg }
    }

    pub fn evaluate(&self, phase: PhaseScore) -> i32 {
        (self.mg * phase as i32 + self.eg * (24 - phase as i32))/24
    }
}

impl std::ops::Neg for TaperedValue {
    type Output = TaperedValue;
    fn neg(self) -> TaperedValue {
        TaperedValue::new(-self.mg, -self.eg)
    }
}

impl std::ops::Add for TaperedValue {
    type Output = TaperedValue;
    fn add(self, rhs: TaperedValue) -> TaperedValue {
        TaperedValue::new(self.mg + rhs.mg, self.eg + rhs.eg)
    }
}

impl std::ops::Sub for TaperedValue {
    type Output = TaperedValue;
    fn sub(self, rhs: TaperedValue) -> TaperedValue {
        TaperedValue::new(self.mg - rhs.mg, self.eg - rhs.eg)
    }
}

impl std::ops::AddAssign for TaperedValue {
    fn add_assign(&mut self, rhs: TaperedValue) {
        self.mg += rhs.mg;
        self.eg += rhs.eg;
    }
}

impl std::ops::SubAssign for TaperedValue {
    fn sub_assign(&mut self, rhs: TaperedValue) {
        self.mg -= rhs.mg;
        self.eg -= rhs.eg;
    }
}

// Base piece values, same source as the PST tables above (Tomasz Michniewski's
// "simplified evaluation function"). The king's is a large sentinel rather than a
// real material value — it's never actually traded, and cancels out of the score
// anyway since both sides always have exactly one on the board.
const PAWN_VALUE: PsqtValue = 100;
const KNIGHT_VALUE: PsqtValue = 320;
const BISHOP_VALUE: PsqtValue = 330;
const ROOK_VALUE: PsqtValue = 500;
const QUEEN_VALUE: PsqtValue = 900;
const KING_VALUE: PsqtValue = 20000;

// Each table below is laid out one rank per row, rank 1 first and rank 8 last,
// files a-h left to right within a row — i.e. index order matches `Square`
// (A1, B1, ..., H1, A2, ..., H8), and values are from White's perspective.
//
// Only pawns and the king get a distinct endgame table (pawns should push harder,
// the king should come out and centralize) — the rest reuse their middlegame table
// for the endgame for now, as an easy starting point to split apart when tuning.

#[rustfmt::skip]
const PAWN_MG_TABLE: [PsqtValue; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10,-20,-20, 10, 10,  5,
     5, -5,-10,  0,  0,-10, -5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5,  5, 10, 25, 25, 10,  5,  5,
    10, 10, 20, 30, 30, 20, 10, 10,
    50, 50, 50, 50, 50, 50, 50, 50,
     0,  0,  0,  0,  0,  0,  0,  0,
];

// Flat across files: just rewards advancing, since a passed/advanced pawn matters
// far more once there's less material around to stop it.
#[rustfmt::skip]
const PAWN_EG_TABLE: [PsqtValue; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    10, 10, 10, 10, 10, 10, 10, 10,
    10, 10, 10, 10, 10, 10, 10, 10,
    20, 20, 20, 20, 20, 20, 20, 20,
    30, 30, 30, 30, 30, 30, 30, 30,
    50, 50, 50, 50, 50, 50, 50, 50,
    80, 80, 80, 80, 80, 80, 80, 80,
     0,  0,  0,  0,  0,  0,  0,  0,
];

#[rustfmt::skip]
const KNIGHT_TABLE: [PsqtValue; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50,
];

#[rustfmt::skip]
const BISHOP_TABLE: [PsqtValue; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -20,-10,-10,-10,-10,-10,-10,-20,
];

#[rustfmt::skip]
const ROOK_TABLE: [PsqtValue; 64] = [
     0,  0,  0,  5,  5,  0,  0,  0,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     5, 10, 10, 10, 10, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0,
];

#[rustfmt::skip]
const QUEEN_TABLE: [PsqtValue; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -10,  5,  5,  5,  5,  5,  0,-10,
      0,  0,  5,  5,  5,  5,  0, -5,
     -5,  0,  5,  5,  5,  5,  0, -5,
    -10,  0,  5,  5,  5,  5,  0,-10,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20,
];

// Middlegame: stay behind the pawn shield, castled corner is good.
#[rustfmt::skip]
const KING_MG_TABLE: [PsqtValue; 64] = [
     20, 30, 10,  0,  0, 10, 30, 20,
     20, 20,  0,  0,  0,  0, 20, 20,
    -10,-20,-20,-20,-20,-20,-20,-10,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
];

// Endgame: opposite instinct — come out and help centralize, mating nets/pawn
// races care about the king being active rather than tucked away.
#[rustfmt::skip]
const KING_EG_TABLE: [PsqtValue; 64] = [
    -50,-30,-30,-30,-30,-30,-30,-50,
    -30,-30,  0,  0,  0,  0,-30,-30,
    -30,-10, 20, 30, 30, 20,-10,-30,
    -30,-10, 30, 40, 40, 30,-10,-30,
    -30,-10, 30, 40, 40, 30,-10,-30,
    -30,-10, 20, 30, 30, 20,-10,-30,
    -30,-20,-10,  0,  0,-10,-20,-30,
    -50,-40,-30,-20,-20,-30,-40,-50,
];

const fn combine(base: PsqtValue, mg: [PsqtValue; 64], eg: [PsqtValue; 64]) -> [TaperedValue; 64] {
    let mut out = [TaperedValue::new(0, 0); 64];
    let mut i = 0;
    while i < 64 {
        out[i] = TaperedValue::new(base + mg[i], base + eg[i]);
        i += 1;
    }
    out
}

/// Per piece-kind, per-square (mg, eg) tables, from White's perspective, each entry
/// already including that piece's base value. Index with [`value`] rather than
/// directly, so `Side::Black` gets mirrored/negated correctly.
///
/// Built by assigning each piece kind's table by name (rather than positionally,
/// as a `PerPiece::from_array` literal would) so the mapping stays correct
/// regardless of the order `PieceKind`'s variants are declared in.
pub const PSQT: PerPiece<PerSquare<TaperedValue>> = {
    let empty = PerSquare::from_array([TaperedValue::new(0, 0); 64]);
    let mut tables = [empty; PieceKind::COUNT];
    tables[PieceKind::Pawn as usize] = PerSquare::from_array(combine(PAWN_VALUE, PAWN_MG_TABLE, PAWN_EG_TABLE));
    tables[PieceKind::Knight as usize] = PerSquare::from_array(combine(KNIGHT_VALUE, KNIGHT_TABLE, KNIGHT_TABLE));
    tables[PieceKind::Bishop as usize] = PerSquare::from_array(combine(BISHOP_VALUE, BISHOP_TABLE, BISHOP_TABLE));
    tables[PieceKind::Rook as usize] = PerSquare::from_array(combine(ROOK_VALUE, ROOK_TABLE, ROOK_TABLE));
    tables[PieceKind::Queen as usize] = PerSquare::from_array(combine(QUEEN_VALUE, QUEEN_TABLE, QUEEN_TABLE));
    tables[PieceKind::King as usize] = PerSquare::from_array(combine(KING_VALUE, KING_MG_TABLE, KING_EG_TABLE));
    PerPiece::from_array(tables)
};

/// Absolute (material + position) (mg, eg) value of `piece` sitting on `square`,
/// signed so it can just be summed (or added/subtracted incrementally) into a
/// White-positive tapered score.
pub fn value(piece: Piece, square: Square) -> TaperedValue {
    match piece.side {
        Side::White => PSQT[piece.kind][square],
        Side::Black => -PSQT[piece.kind][mirror(square)],
    }
}

/// Flip a square across the board's horizontal centerline (a1 <-> a8, e1 <-> e8, ...),
/// used to reuse White's tables for Black.
const fn mirror(square: Square) -> Square {
    // Square is laid out as file + 8*rank, so XORing with 0b111000 flips just the
    // rank bits (rank <-> 7 - rank) and leaves the file untouched.
    unsafe { std::mem::transmute::<u8, Square>((square as u8) ^ 0b111000) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basetypes::PieceKind;
    use strum::IntoEnumIterator;

    #[test]
    fn mirror_is_involution() {
        for square in Square::iter() {
            assert!(mirror(mirror(square)) == square);
        }
    }

    #[test]
    fn white_and_black_are_mirrored_negations() {
        for kind in PieceKind::iter() {
            for square in Square::iter() {
                let white = value(Piece { side: Side::White, kind }, square);
                let black = value(Piece { side: Side::Black, kind }, mirror(square));
                assert!(white == -black);
            }
        }
    }
}
