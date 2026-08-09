//! Board representation and position state.
//!
//! Internal representation (bitboards vs. mailbox) is not yet decided; `Position`
//! is a placeholder type until that's chosen.

use crate::{Piece, basetypes::{Bitboard, PerPiece, PerSide, PerSquare, PieceKind, Side}};
// use crate::basetypes::{Move, Piece, Side, Square}; // unused while Position methods below are commented out

/// Error parsing a FEN string. Detail fields TBD.
#[derive(Debug)]
pub struct FenError;

/// A chess position. Internal representation TBD.
#[derive(Debug, Clone)]
pub struct Position {
    pub pieces: PerSide<PerPiece<Bitboard>>,
    pub sides_pieces: PerSide<Bitboard>,
    pub piece_list: PerSquare<Piece>
}

impl Position {

    /// The standard starting position.
    pub fn new() -> Self {
        let pieces = PerSide::new(PerPiece::new(Bitboard::EMPTY));
        let sides_pieces = PerSide::new(Bitboard::EMPTY);
        let piece_list = PerSquare::new(Piece{side: Side::None, piece_kind: PieceKind::None});
        Position {
            pieces,
            sides_pieces,
            piece_list
        }
    }

    pub fn print_state(&self) {
        for (side, per_piece) in self.pieces.iter() {
            println!("\n============= {side} ==========");
            for (piece_type, bb) in per_piece.iter() {
                println!("\n{piece_type}");
                bb.print(Piece{side, piece_kind: piece_type}.to_unicode_char());
            }
        }
    }

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
