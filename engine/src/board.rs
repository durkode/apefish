//! Board representation and position state.
//!
//! Internal representation (bitboards vs. mailbox) is not yet decided; `Position`
//! is a placeholder type until that's chosen.

use crate::basetypes::{Bitboard, PerPiece, PerSide};
// use crate::basetypes::{Move, Piece, Side, Square}; // unused while Position methods below are commented out

/// Error parsing a FEN string. Detail fields TBD.
#[derive(Debug)]
pub struct FenError;

/// A chess position. Internal representation TBD.
#[derive(Debug, Clone)]
pub struct Position {
    pub pieces: PerSide<PerPiece<Bitboard>>,
    pub sides_pieces: PerSide<Bitboard>,
}

impl Position {
    // Unused for now, commented out to silence warnings. Uncomment as these get wired up.

    // /// The standard starting position.
    // pub fn startpos() -> Self {
    //     unimplemented!()
    // }

    // /// Parse a position from Forsyth-Edwards Notation.
    // pub fn from_fen(fen: &str) -> Result<Self, FenError> {
    //     unimplemented!()
    // }

    // /// Serialize the position to Forsyth-Edwards Notation.
    // pub fn to_fen(&self) -> String {
    //     unimplemented!()
    // }

    // pub fn side_to_move(&self) -> Side {
    //     unimplemented!()
    // }

    // pub fn piece_at(&self, sq: Square) -> Option<Piece> {
    //     unimplemented!()
    // }

    // /// Apply a move to the position. Caller is responsible for ensuring legality.
    // pub fn make_move(&mut self, mv: Move) {
    //     unimplemented!()
    // }
}
