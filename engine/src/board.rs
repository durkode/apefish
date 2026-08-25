//! Board representation and position state.
//!
//! Internal representation (bitboards vs. mailbox) is not yet decided; `Position`
//! is a placeholder type until that's chosen.

use std::sync::Arc;

use crate::eval::incremental_eval;
use crate::phase::{PhaseScore, incremental_phase_score};
use crate::psqt::TaperedValue;
use crate::{fen, movegen::MoveGen};
use crate::basetypes::{Bitboard, CastlingDirection, CastlingRights, File, GenericErr::{self, IllegalMove}, Move, PerPiece, PerSide, PerSquare, Piece, PieceKind, Rank, Side, Square};
use crate::multiset::{MultiSet};
use crate::zobrist::{ZobristKey, ZobristRandoms};

const MAX_MOVES: usize = 1024;

/// All of the metadata around gamestate (except actual pieces) at a given point in time
#[derive(Debug, Copy, Clone)]
pub struct PositionState {
    pub active_side: Side,
    pub castling: CastlingRights,
    pub half_move_clock: u8,
    pub full_move_number: u16,
    pub en_passant: Option<Square>,
    pub zobrist_hash: ZobristKey,
    pub phase_score: PhaseScore,
    pub tapered_eval: TaperedValue,
}

impl PositionState {
    pub fn new() -> Self {
        Self {
            active_side: Side::White,
            castling: CastlingRights::new(CastlingRights::ALL),
            half_move_clock: 0,
            full_move_number: 0,
            en_passant: None,
            zobrist_hash: 0,
            phase_score: 0,
            tapered_eval: TaperedValue { mg: 0, eg: 0 }
        }
    }
}

pub trait Reset {
    // Reset a struct back to original state
    fn reset(&mut self) -> &mut Self;
}

type PositionHistory = Stack<PositionState, MAX_MOVES>;
type AlteredPieceHistory = Stack<AlteredPieces, MAX_MOVES>;

#[derive(Debug, Clone)]
pub struct Stack<T, const N: usize> {
    stack_array: [T; N],
    stack_pointer: usize
}

// Position history is effecitvely a stack of position states.
// No error checking for out of bounds, assume the caller is tracking.
impl<T: Copy, const N: usize> Stack<T, N> {
    pub fn new(default: T) -> Self {
        Self {
            stack_array: [default; N],
            stack_pointer: 0
        }
    }
}

impl<T, const N: usize> Stack<T, N> {
    // push to stack
    pub fn push(&mut self, item: T) {
        self.stack_pointer += 1;
        self.stack_array[self.stack_pointer - 1] = item;
    }

    // pop from stack. Don't actually need to return or delete.
    pub fn pop(&mut self) -> &T {
        self.stack_pointer -= 1;
        &self.stack_array[self.stack_pointer]
    }

    pub fn peek(&self) -> &T {
        &self.stack_array[self.stack_pointer - 1]
    }
}

impl <T: Copy + Reset, const N: usize> Stack<T, N> {
    // Essentially a push to the stack, but repurposing the existing next element of array
    pub fn increment_and_reset(&mut self) -> &mut T {
        self.stack_pointer += 1;
        self.stack_array[self.stack_pointer - 1].reset()
    }
}


// Piece move change log.
#[derive(Clone, Copy, Debug, Default)]
pub struct PieceChange {
    pub piece: Piece,
    pub from: Option<Square>,  // None = the piece is created
    pub to: Option<Square>,    // None = the piece is removed
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AlteredPieces {
    pub changes: [PieceChange; 3],
    pub count: usize,
}

impl AlteredPieces {

    pub fn piece_changes(&self) -> &[PieceChange] {
        &self.changes[0..self.count]
    }

    pub fn add(&mut self, piece_change: PieceChange) {
        self.changes[self.count] = piece_change;
        self.count += 1;
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }
}

impl Reset for AlteredPieces {
    fn reset(&mut self) -> &mut Self {
        self.count = 0;
        self
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
    piece_change_log: AlteredPieceHistory,
    zobrist_randoms: Arc<ZobristRandoms>,
    zobrists_visited: MultiSet<ZobristKey>
}

impl Position {

    /// The standard starting position.
    pub fn new(zobrists: Arc<ZobristRandoms>) -> Self {
        let pieces = PerSide::new(PerPiece::new(Bitboard::EMPTY));
        let sides_pieces = PerSide::new(Bitboard::EMPTY);
        let piece_list = PerSquare::new(None);
        let mut position = Position {
            pieces,
            sides_pieces,
            piece_by_square: piece_list,
            state: PositionState::new(),
            history: PositionHistory::new(PositionState::new()),
            piece_change_log: AlteredPieceHistory::new(AlteredPieces::default()),
            zobrist_randoms: zobrists,
            zobrists_visited: MultiSet::new(),
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
            zobrist_hash: 0u64,
            phase_score: 0,
            tapered_eval: TaperedValue { mg: 0, eg: 0 }
        };
        self.initialise_incrementally_updated_fields();
        self.history = PositionHistory::new(PositionState::new());

        Ok(())
    }

    pub fn fen(&self) -> String {
        fen::to_fen(&self)
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

    fn initialise_incrementally_updated_fields(&mut self) {

        self.state.zobrist_hash ^= self.zobrist_randoms.ep_key(self.state.en_passant);
        self.state.zobrist_hash ^= self.zobrist_randoms.castling_key(self.state.castling.rights_u8());
        self.state.zobrist_hash ^= self.zobrist_randoms.side_key(self.state.active_side);

        let mut piece_value = TaperedValue{mg: 0, eg: 0};
        let mut phase_score: PhaseScore = 0;
        for (square, maybe_piece) in self.piece_by_square.iter() {
            if let Some(piece) = maybe_piece {
                let Piece{side, kind} = *piece;

                self.state.zobrist_hash ^= self.zobrist_randoms.piece_key(side, kind, square);

                let piece_changes: &[PieceChange] = &[PieceChange{piece: *piece, from: None, to: Some(square)}];
                piece_value = incremental_eval(piece_value, piece_changes);
                phase_score = incremental_phase_score(phase_score, piece_changes);
            }
        }
        self.state.tapered_eval = piece_value;
        self.state.phase_score = phase_score;

        self.zobrists_visited.add(self.state.zobrist_hash);
    }

    // fn initialise_zobrists(&mut self) {
    //     self.state.zobrist_hash ^= self.zobrist_randoms.ep_key(self.state.en_passant);
    //     self.state.zobrist_hash ^= self.zobrist_randoms.castling_key(self.state.castling.rights_u8());
    //     self.state.zobrist_hash ^= self.zobrist_randoms.side_key(self.state.active_side);

    //     // Now do the zobrists for all pieces
    //     for (square, p) in self.piece_by_square.iter() {
    //         if let Some(Piece{side, kind}) = *p {
    //             self.state.zobrist_hash ^= self.zobrist_randoms.piece_key(side, kind, square);
    //         }
    //     }

    //     self.zobrists_visited.add(self.state.zobrist_hash);
    // }

    // fn initialise_phase_score(&mut self) {
    //     let mut phase_score: PhaseScore = 0;
    //     for
    // }

    // fn initialise_tapered_eval(&mut self) {
    //     let mut piece_value = TaperedValue{mg: 0, eg: 0};
    //     for (square, maybe_piece) in self.piece_by_square.iter() {
    //         if let Some(piece) = maybe_piece {
    //             let piece_changes: &[PieceChange] = &[PieceChange{piece: *piece, from: None, to: Some(square)}];
    //             piece_value = incremental_eval(piece_value, self, piece_changes)
    //         }
    //     }
    //     self.state.tapered_eval = piece_value;
    // }

    // Make the move on the board.
    // Assumes the move is semi-legal. i.e. legal except for if it leaves the king in check, or castles from or through check
    // TODO: using Unwrap which will panic on invalid move. Return error instead.
    pub fn make_move(&mut self, mg: &MoveGen, m: Move) -> Result<(), GenericErr> {

        // Put the current move to make on the game state and push to history
        self.history.push(self.state.clone());
        let mut altered_pieces = AlteredPieces::default();

        // Iterate through move types to generate the change log. The order should be:
        //   - Castling
        //   - En Passant
        //   - Promotion
        // Note: the order that board changes are added to altered_pieces matters to avoid corruption!!
        if m.castling() {
            let castling_direction = CastlingDirection::direction(m.from(), m.to());
            for s in castling_direction.unwrap().unattacked_squares_required() {
                if mg.is_attacked(self, *s) {
                    self.history.pop();
                    return Err(GenericErr::InvalidCastleChecked)
                }
            }
            altered_pieces.add(PieceChange { 
                piece: Piece{ side: self.state.active_side, kind: PieceKind::King }, 
                from: Some(m.from()), 
                to: Some(m.to()) 
            });
            altered_pieces.add(PieceChange { 
                piece: Piece{ side: self.state.active_side, kind: PieceKind::Rook }, 
                from: Some(castling_direction.unwrap().rook_from()), 
                to: Some(castling_direction.unwrap().rook_to()) 
            });
        } else if m.en_passant() {
            altered_pieces.add(PieceChange { 
                piece: Piece{ side: self.state.active_side, kind: PieceKind::Pawn}, 
                from: Some(m.from()), 
                to: Some(m.to()), 
            });
            altered_pieces.add(PieceChange { 
                piece: Piece{ side: self.state.active_side.other(), kind: PieceKind::Pawn}, 
                from: Some(Square::from_coords(m.to().file(), m.from().rank())), 
                to: None, 
            });
        } else if let Some(promotion_kind) = m.promotion() {
            if let Some(captured) = m.captured() {
                altered_pieces.add(PieceChange{
                    piece: Piece{side: self.state.active_side.other(), kind: captured},
                    from: Some(m.to()),
                    to: None,
                });
            }
            altered_pieces.add(PieceChange{
                piece: Piece{side: self.state.active_side, kind: PieceKind::Pawn},
                from: Some(m.from()),
                to: None,
            });
            altered_pieces.add(PieceChange{
                piece: Piece{side: self.state.active_side, kind: promotion_kind},
                from: None,
                to: Some(m.to()),
            });
        } else {
            // Base case for all normal moves (Non castling / EP / Promotion)
            if let Some(captured) = m.captured() {
                altered_pieces.add(PieceChange { 
                    piece: Piece{ side: self.state.active_side.other(), kind: captured }, 
                    from: Some(m.to()), 
                    to: None
                });
            }
            altered_pieces.add(PieceChange { 
                piece: Piece{ side: self.state.active_side, kind: m.piece() }, 
                from: Some(m.from()), 
                to: Some(m.to()) 
            });
        }

        // Now move the pieces
        self.apply_change_log_to_board(&altered_pieces);

        // If in check, delete change log and return err
        if mg.is_attacked(self, self.king_square()) {
            self.reverse_change_log_to_board(&altered_pieces);
            self.history.pop();
            return Err(IllegalMove)
        }

        // ------------  Move Confirmed good at this point.  -----------

        self.piece_change_log.push(altered_pieces);


        // Update incremental tallies
        self.state.phase_score = incremental_phase_score(self.state.phase_score, altered_pieces.piece_changes());
        self.state.tapered_eval = incremental_eval(self.state.tapered_eval, altered_pieces.piece_changes());

        // Update game state
        let reset_half_move_clock = m.piece() == PieceKind::Pawn || !m.captured().is_none();
        self.state.half_move_clock = if reset_half_move_clock {0} else {self.state.half_move_clock + 1};
        if self.state.active_side == Side::Black {
            self.state.full_move_number += 1;
        }

        // Update castling rights
        // TODO: Only update castling zobrists if there is a change
        self.state.zobrist_hash ^= self.zobrist_randoms.castling_key(self.state.castling.rights_u8());
        self.state.castling.remove_rights_for_square_touched(m.from());
        self.state.castling.remove_rights_for_square_touched(m.to());
        self.state.zobrist_hash ^= self.zobrist_randoms.castling_key(self.state.castling.rights_u8());

        // Update EP square
        let new_ep_square = match (m.piece(), self.state.active_side, m.from().rank(), m.to().rank()) {
            (PieceKind::Pawn, Side::White, Rank::R2, Rank::R4) => Some(Square::from_coords(m.from().file(), Rank::R3)),
            (PieceKind::Pawn, Side::Black, Rank::R7, Rank::R5) => Some(Square::from_coords(m.from().file(), Rank::R6)),
            _ => None
        };
        if new_ep_square != self.state.en_passant {
            self.state.zobrist_hash ^= self.zobrist_randoms.ep_key(self.state.en_passant);
            self.state.zobrist_hash ^= self.zobrist_randoms.ep_key(new_ep_square);
            self.state.en_passant = new_ep_square;
        }
        
        // Switch side
        self.state.zobrist_hash ^= self.zobrist_randoms.side_key(self.state.active_side);
        self.state.active_side = self.state.active_side.other();
        self.state.zobrist_hash ^= self.zobrist_randoms.side_key(self.state.active_side);

        self.zobrists_visited.add(self.state.zobrist_hash);

        Ok(())
    }

    pub fn unmake_move(&mut self) {
        if let Err(_) = self.zobrists_visited.remove(self.state.zobrist_hash) {
            panic!("Should not be removing unfound zobrists");
        };

        let ap = *self.piece_change_log.pop();
        self.reverse_change_log_to_board(&ap);

        self.state = *self.history.pop();
    }

    fn apply_change_log_to_board(&mut self, altered_pieces: &AlteredPieces) {
        for ap in altered_pieces.piece_changes() {
            if let Some(from) = ap.from {
                self.remove_piece(&from, ap.piece);
            }
            if let Some(to) = ap.to {
                self.add_piece(&to, ap.piece);
            }
        }
    }

    fn reverse_change_log_to_board(&mut self, altered_pieces: &AlteredPieces) {
        for ap in altered_pieces.piece_changes().iter().rev() {
            if let Some(from) = ap.from {
                self.add_piece(&from, ap.piece);
            }
            if let Some(to) = ap.to {
                self.remove_piece(&to, ap.piece);
            }
        }
    }

    // Add a piece to a square, assumes square is empty.
    fn add_piece(&mut self, square: &Square, piece: Piece) {
        self.pieces[piece.side][piece.kind] |= square.bitboard();
        self.sides_pieces[piece.side] |= square.bitboard();
        self.piece_by_square[*square] = Some(piece);
        self.state.zobrist_hash ^= self.zobrist_randoms.piece_key(piece.side, piece.kind, *square);
    }

    // Remove a piece from a square.
    fn remove_piece(&mut self, square: &Square, piece: Piece) {
        self.pieces[piece.side][piece.kind] &= !square.bitboard();
        self.sides_pieces[piece.side] &= !square.bitboard();
        self.piece_by_square[*square] = None;
        self.state.zobrist_hash ^= self.zobrist_randoms.piece_key(piece.side, piece.kind, *square);
    }

    // What square is the king on for the active side
    fn king_square(&self) -> Square {
        self.pieces[self.state.active_side][PieceKind::King].single_square().unwrap()
    }

    pub fn all_pieces(&self) -> Bitboard {
        self.sides_pieces[Side::White] | self.sides_pieces[Side::Black]
    }

    pub fn side_to_move(&self) -> Side {
        self.state.active_side
    }

    pub fn in_check(&self, mg: &MoveGen) -> bool {
        mg.is_attacked(self, self.king_square())
    }

    pub fn half_move_clock(&self) -> u8 {
        self.state.half_move_clock
    }

    pub fn get_zobrist(&self) -> ZobristKey {
        self.state.zobrist_hash
    }

    pub fn times_position_reached(&self) -> u64 {
        self.zobrists_visited.count(self.state.zobrist_hash)
    }

    pub fn insufficient_material(&self) -> bool {
        // FIDE definition for insufficient material

        for (side, pieces) in self.pieces.iter() {
            for (pk, bb) in pieces.iter() {
                if *bb != Bitboard::EMPTY {
                    match pk {
                        PieceKind::King => {},
                        PieceKind::Pawn | PieceKind::Rook | PieceKind::Queen => {return false},
                        PieceKind::Knight => {
                            if self.sides_pieces[side].num_pieces() > 2 || self.sides_pieces[side.other()].num_pieces() > 1 {
                                return false
                            } else {
                                return true
                            }
                        },
                        PieceKind::Bishop => {
                            let active_side_has_only_bishops = self.sides_pieces[side].num_pieces() <= bb.num_pieces() + 1;
                            let other_bishops = self.pieces[side.other()][PieceKind::Bishop];
                            let other_side_has_only_bishops = self.sides_pieces[side.other()].num_pieces() <= other_bishops.num_pieces() + 1;
                            if active_side_has_only_bishops && other_side_has_only_bishops {
                                // Now check all bishops are the same colour
                                let first_bishop_is_white = bb.iter_squares().next().unwrap().is_white();
                                let all_bishops_same_colour = (*bb | other_bishops).iter_squares().all(|s| s.is_white() == first_bishop_is_white);
                                return all_bishops_same_colour;   
                            } else {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    }

    pub fn print_debug_state(&self) {
        // for (side, per_piece) in self.pieces.iter() {
        //     println!("\n============= {side} ==========");
        //     for (piece_type, bb) in per_piece.iter() {
        //         println!("\n{piece_type}");
        //         bb.print(Piece{side, kind: piece_type}.to_unicode_char());
        //     }
        // }

        println!("\n================ BOARD ============\n");
        self.print_board();
        let eval = self.state.tapered_eval.evaluate(self.state.phase_score);
        let phase = self.state.phase_score;
        println!("Phase Score: {phase}, Eval Score: {eval:+}")
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

}
