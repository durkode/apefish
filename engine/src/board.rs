//! Board representation and position state.
//!
//! Internal representation (bitboards vs. mailbox) is not yet decided; `Position`
//! is a placeholder type until that's chosen.

use crate::{PieceKind, basetypes::{Bitboard, CastlingDirection, CastlingRights, File, GenericErr::{self, IllegalMove}, Move, PerPiece, PerSide, PerSquare, Piece, Rank, Side, Square}, fen, movegen::MoveGen};

const MAX_MOVES: usize = 1024;

/// All of the metadata around gamestate (except actual pieces) at a given point in time
#[derive(Debug, Copy, Clone)]
pub struct PositionState {
    pub active_side: Side,
    pub castling: CastlingRights,
    pub half_move_clock: u8,
    pub full_move_number: u16,
    pub en_passant: Option<Square>,
    pub next_move: Option<Move>
}

impl PositionState {
    pub fn new() -> Self {
        Self {
            active_side: Side::White,
            castling: CastlingRights::new(CastlingRights::ALL),
            half_move_clock: 0,
            full_move_number: 0,
            en_passant: None,
            next_move: None,
        }
    }
}

#[derive(Debug, Clone)]
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
    pub piece_by_square: PerSquare<Option<Piece>>,
    pub state: PositionState,
    pub history: PositionHistory,
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
            piece_by_square: piece_list,
            state: PositionState::new(),
            history: PositionHistory::new(),
        };
        position.reset_to_start_fen();
        position
    }

    pub fn reset_to_start_fen(&mut self) {
        self.fen_setup(fen::STARTING_FEN).unwrap();
    }

    pub fn fen_setup(&mut self, fen_str: &str) -> Result<(), fen::FenError>{
        let fen_struct = fen::parse_fen(fen_str)?;
        self.piece_by_square = fen_struct.pieces;
        self.initialise_bitboards_from_piece_list();

        self.state = PositionState { 
            active_side: fen_struct.active_colour, 
            castling: fen_struct.castling_rights, 
            half_move_clock: fen_struct.half_move_clock, 
            full_move_number: fen_struct.full_move_number, 
            en_passant: fen_struct.en_passant_square, 
            next_move: None 
        };
        self.history = PositionHistory::new();

        Ok(())
    }

    pub fn fen(&self) -> &str {
        to_fen()
    }

    fn reset_piece_bitboards(&mut self) {
        for (_side, piece_boards) in self.pieces.iter_mut() {
            for (_piece_kind, bb) in piece_boards.iter_mut() {
                *bb = Bitboard::EMPTY;
            }
        }

        for (_side, bb) in self.sides_pieces.iter_mut() {
            *bb = Bitboard::EMPTY;
        }

    }

    fn initialise_bitboards_from_piece_list(&mut self) {
        self.reset_piece_bitboards();

        for (square, piece) in self.piece_by_square.iter() {
            let Some(piece) = piece else { continue };
            self.pieces[piece.side][piece.kind] |= square.bitboard();
            self.sides_pieces[piece.side] |= square.bitboard();
        }

    }

    // Make the move on the board.
    // Assumes the move is semi-legal. i.e. legal except for if it leaves the king in check, or castles from or through check
    // TODO: using Unwrap which will panic on invalid move. Return error instead.
    pub fn make_move(&mut self, mg: &MoveGen, m: Move) -> Result<(), GenericErr> {
        // Check Castling with check (both before, through, after) preconditions.
        let castling_direction = CastlingDirection::direction(m.from(), m.to());
        // TODO: it feels weird to treat castling as a separate case, but also it feels easier but more convoluted??
        // Think about this structure further
        if m.castling() {
            // Assume that Move Generator has already validated castling rights and that squares are clear
            // Need to check that the move is not in or moving through check.
            // 
            for s in castling_direction.unwrap().unattacked_squares_required() {
                if mg.is_attacked(self, *s) {
                    return Err(GenericErr::InvalidCastleChecked)
                }
            }
        }

        // Put the current move to make on the game state and push to history
        self.state.next_move = Some(m);
        self.history.push(self.state.clone());

        // Remove the moving piece
        self.remove_piece(&m.from(), self.piece_by_square[m.from()].unwrap());
        
        // add the piece to the destination square
        let captured_piece = self.piece_by_square[m.to()];
        if !captured_piece.is_none() {
            self.remove_piece(&m.to(), captured_piece.unwrap());
        }
        let new_piece = Piece{ side: self.state.active_side, kind: m.promotion().unwrap_or(m.piece())};
        self.add_piece(&m.to(), new_piece);

        // En Passant
        if m.en_passant() {
            let ep_square = Square::from_coords(m.to().file(), m.from().rank());
            self.remove_piece(&ep_square, self.piece_by_square[ep_square].unwrap());
        }

        // Castling: move the rook
        if m.castling() {
            self.remove_piece(
                &castling_direction.unwrap().rook_from(), 
                self.piece_by_square[castling_direction.unwrap().rook_from()].unwrap()
            );
            self.add_piece(
                &castling_direction.unwrap().rook_to(), 
                Piece{side: self.state.active_side, kind: PieceKind::Rook}
            );
        } else if mg.is_attacked(self, self.king_square()) {
            // Oh no, we are in check. Revert everything back and return an error
            self.remove_piece(&m.to(), new_piece);
            self.add_piece(&m.from(), Piece{side: self.state.active_side, kind: m.piece()});
            // Add back the taken piece
            if m.en_passant() {
                self.add_piece(&Square::from_coords(m.to().file(), m.from().rank()), Piece{ side: self.state.active_side.other(), kind: PieceKind::Pawn});
            } else if !captured_piece.is_none() {
                self.add_piece(&m.to(), captured_piece.unwrap());
            }

            self.history.pop();
            return Err(IllegalMove)
        }

        // Update game state
        self.state.half_move_clock = if m.piece() == PieceKind::Pawn || !captured_piece.is_none() {0} else {self.state.half_move_clock + 1};
        self.state.full_move_number += 1;
        self.state.castling.remove_rights_for_move(self.state.active_side, m.from(), m.piece());
        self.state.en_passant = match (m.piece(), self.state.active_side, m.from().rank(), m.to().rank()) {
            (PieceKind::Pawn, Side::White, Rank::R2, Rank::R4) => Some(Square::from_coords(m.from().file(), Rank::R3)),
            (PieceKind::Pawn, Side::Black, Rank::R7, Rank::R5) => Some(Square::from_coords(m.from().file(), Rank::R6)),
            _ => None
        };
        self.state.next_move = None;
        self.state.active_side = self.state.active_side.other();


        Ok(())
    }

    pub fn unmake_move(&mut self) {
        self.state = *self.history.peek();
        let previous_move = self.state.next_move.unwrap();
        self.state.next_move = None;
        self.history.pop();

        // State restored, now adjust the board.

        // Add the piece back to the square it moved from
        let piece_moved = Piece { side: self.state.active_side, kind: previous_move.piece() };
        self.add_piece(&previous_move.from(), piece_moved);
        self.remove_piece(&previous_move.to(), piece_moved);

        // Now deal with the other pieces
        if previous_move.castling() {
            let direction = CastlingDirection::direction(previous_move.from(), previous_move.to()).unwrap();
            let rook = Piece{kind: PieceKind::Rook, side: self.state.active_side};
            self.add_piece(&direction.rook_from(), rook);
            self.remove_piece(&direction.rook_to(), rook);
        } else if previous_move.en_passant() {
            let taken_pawn_square = Square::from_coords(previous_move.to().file(), previous_move.from().rank());
            self.add_piece(&taken_pawn_square, Piece{side: self.state.active_side.other(), kind: PieceKind::Pawn});
        } else if let Some(captured_piece_kind) = previous_move.captured() {
            self.add_piece(&previous_move.to(), Piece { side: self.state.active_side.other(), kind: captured_piece_kind });
        }
    }

    // Add a piece to a square, assumes square is empty.
    fn add_piece(&mut self, square: &Square, piece: Piece) {
        self.pieces[piece.side][piece.kind] |= square.bitboard();
        self.sides_pieces[piece.side] |= square.bitboard();
        self.piece_by_square[*square] = Some(piece);
    }

    // Remove a piece from a square.
    fn remove_piece(&mut self, square: &Square, piece: Piece) {
        self.pieces[piece.side][piece.kind] &= !square.bitboard();
        self.sides_pieces[piece.side] &= !square.bitboard();
        self.piece_by_square[*square] = None;
    }

    // What square is the king on for the active side
    fn king_square(&self) -> Square {
        self.pieces[self.state.active_side][PieceKind::King].single_square().unwrap()
    }

    pub fn print_debug_state(&self) {
        for (side, per_piece) in self.pieces.iter() {
            println!("\n============= {side} ==========");
            for (piece_type, bb) in per_piece.iter() {
                println!("\n{piece_type}");
                bb.print(Piece{side, kind: piece_type}.to_unicode_char());
            }
        }

        println!("\n================ BOARD ============\n");
        self.print_board();
    }

    pub fn print_board(&self) {
        for r in Rank::iter().rev() {
            let rank_num = r as u8 + 1;
            print!("{rank_num}  ");
            for f in File::iter() {
                let char = match self.piece_by_square[Square::from_coords(f, r)] {
                    Some(x) => x.to_unicode_char(),
                    None => Piece::NO_PIECE_CHAR                    
                };
                print!("{char} ");
            }
            print!("\n");
        }
        println!("\n   A B C D E F G H");
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
