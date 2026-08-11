//! Board representation and position state.
//!
//! Internal representation (bitboards vs. mailbox) is not yet decided; `Position`
//! is a placeholder type until that's chosen.

use crate::{Move, Square, basetypes::{Bitboard, CastlingRights, PerPiece, PerSide, PerSquare, Piece, PieceKind, Side}, fen};
// use crate::basetypes::{Move, Piece, Side, Square}; // unused while Position methods below are commented out

/// All of the metadata around gamestate (except actual pieces) at a given point in time
#[derive(Debug)]
pub struct PositionState {
    pub active_colour: Side,
    pub castling: CastlingRights,
    pub half_move_clock: u8,
    pub en_passant: Option<Square>,
    pub full_move_number: u16,
    pub next_move: Move
}

/// A chess position.
#[derive(Debug, Clone)]
pub struct Position {
    pub pieces: PerSide<PerPiece<Bitboard>>,
    pub sides_pieces: PerSide<Bitboard>,
    pub piece_by_square: PerSquare<Piece>
}

impl Position {

    /// The standard starting position.
    pub fn new() -> Self {
        let pieces = PerSide::new(PerPiece::new(Bitboard::EMPTY));
        let sides_pieces = PerSide::new(Bitboard::EMPTY);
        let piece_list = PerSquare::new(Piece{side: Side::None, piece_kind: PieceKind::None});
        let mut position = Position {
            pieces,
            sides_pieces,
            piece_by_square: piece_list
        };
        position.reset();
        position
    }

    pub fn reset(&mut self) {
        self.fen_setup(fen::STARTING_FEN).unwrap();
    }

    pub fn fen_setup(&mut self, fen_str: &str) -> Result<(), fen::FenError>{
        let fen_struct = fen::parse_fen(fen_str)?;
        self.piece_by_square = fen_struct.pieces;
        // TODO: Initialise the rest of the position from the fen parts
        self.initialise_bitboards_from_piece_list();
        Ok(())
    }

    fn reset_piece_bitboards(&mut self) {
        for (_side, piece_boards) in self.pieces.iter_mut() {
            for (_piece_kind, bb) in piece_boards.iter_mut() {
                *bb = Bitboard::EMPTY;
            }
        }
    }

    fn initialise_bitboards_from_piece_list(&mut self) {
        self.reset_piece_bitboards();

        for (square, piece) in self.piece_by_square.iter() {
            if piece.side == Side::None {
                continue;
            }
            self.pieces[piece.side][piece.piece_kind] |= square.bitboard_mask();
            self.sides_pieces[piece.side] |= square.bitboard_mask();
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
