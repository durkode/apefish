//! Board representation and position state.
//!
//! Internal representation (bitboards vs. mailbox) is not yet decided; `Position`
//! is a placeholder type until that's chosen.

use crate::{Square, basetypes::{Bitboard, CastlingRights, Move, PerPiece, PerSide, PerSquare, Piece, Side}, fen};

const MAX_MOVES: usize = 1024;

/// All of the metadata around gamestate (except actual pieces) at a given point in time
#[derive(Debug, Copy, Clone)]
pub struct PositionState {
    pub active_colour: Side,
    pub castling: CastlingRights,
    pub half_move_clock: u8,
    pub full_move_number: u16,
    pub en_passant: Option<Square>,
    pub next_move: Option<Move>
}

impl PositionState {
    pub fn new() -> Self {
        Self {
            active_colour: Side::White,
            castling: CastlingRights::new(CastlingRights::ALL),
            half_move_clock: 0,
            full_move_number: 0,
            en_passant: None,
            next_move: None,
        }
    }
}

pub struct PositionHistory {
    stack_array: [PositionState; MAX_MOVES],
    stack_pointer: usize
}

// Position history is effecitvely a stack of position states.
// No error checking for out of bounds, assume the caller is tracking.
impl PositionHistory {
    pub fn new() -> Self {
        Self {
            stack_array: [PositionState::new(); MAX_MOVES],
            stack_pointer: 0
        }
    }

    // push to stack
    pub fn push(&mut self, state: PositionState) {
        self.stack_pointer += 1;
        self.stack_array[self.stack_pointer] = state;
    }

    // pop from stack. Don't actually need to return or delete.
    pub fn pop(&mut self) {
        self.stack_pointer -= 1;
    }

    pub fn peek(&self) -> &PositionState {
        &self.stack_array[self.stack_pointer]
    }


}

/// A chess position.
#[derive(Debug, Clone)]
pub struct Position {
    pub pieces: PerSide<PerPiece<Bitboard>>,
    pub sides_pieces: PerSide<Bitboard>,
    pub piece_by_square: PerSquare<Option<Piece>>
}

impl Position {

    /// The standard starting position.
    pub fn new() -> Self {
        let pieces = PerSide::new(PerPiece::new(Bitboard::EMPTY));
        let sides_pieces = PerSide::new(Bitboard::EMPTY);
        let piece_list = PerSquare::new(None);
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
            let Some(piece) = piece else { continue };
            self.pieces[piece.side][piece.kind] |= square.bitboard_mask();
            self.sides_pieces[piece.side] |= square.bitboard_mask();
        }

    }

    pub fn print_state(&self) {
        for (side, per_piece) in self.pieces.iter() {
            println!("\n============= {side} ==========");
            for (piece_type, bb) in per_piece.iter() {
                println!("\n{piece_type}");
                bb.print(Piece{side, kind: piece_type}.to_unicode_char());
            }
        }

        println!("\n================ BOARD ============\n");
        let mut file_counter = 0;
        for (_, piece) in self.piece_by_square.iter() {
            let char = match piece {
                Some(x) => x.to_unicode_char(),
                None => Piece::NO_PIECE_CHAR,
            };
            print!("{char} ");
            file_counter += 1;
            if file_counter == 8 {
                print!("\n");
                file_counter = 0;
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
