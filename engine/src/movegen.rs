// //! Move generation, legality, and game-end detection.


use strum::{EnumCount, IntoEnumIterator};

use crate::{PieceKind, Square};
use crate::basetypes::{BB_FILES, BB_RANKS, Bitboard, File, Move, PerPiece, PerSquare, Rank};
use crate::board::Position;

// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum GameStatus {
//     Ongoing,
//     /// The side that has been checkmated.
//     Checkmate(Side),
//     Stalemate,
//     DrawByRepetition,
//     DrawByFiftyMoveRule,
//     DrawByInsufficientMaterial,
// }

pub(super) struct SquareOffset;

impl SquareOffset {
    pub const NORTH: i8 = 8;
    pub const SOUTH: i8 = -8;
    pub const EAST: i8 = 1;
    pub const WEST: i8 = -1;
    pub const NORTH_WEST: i8 = 7;
    pub const SOUTH_EAST: i8 = -7;
    pub const NORTH_EAST: i8 = 9;
    pub const SOUTH_WEST: i8 = -9;

    pub const KNIGHT_NNW: i8 = 15;
    pub const KNIGHT_WNW: i8 = 6;
    pub const KNIGHT_WSW: i8 = -10;
    pub const KNIGHT_SWS: i8 = -17;
    pub const KNIGHT_SES: i8 = -15;
    pub const KNIGHT_ESE: i8 = -6;
    pub const KNIGHT_ENE: i8 = 10;
    pub const KNIGHT_NNE: i8 = 17;
}

// Modelling shifting a piece on a bitboard, and with a mask of where that is impossible to do so from.
pub(super) struct PieceMove {
    pub offset: i8,
    pub impossible: Bitboard
}

// 

const KING_MOVES: [PieceMove; 8] = [
    PieceMove {
        offset: SquareOffset::NORTH,
        impossible: BB_RANKS[Rank::R8.as_num()]
    },
    PieceMove {
        offset: SquareOffset::SOUTH,
        impossible: BB_RANKS[Rank::R1.as_num()]
    },
    PieceMove {
        offset: SquareOffset::EAST,
        impossible: BB_FILES[File::H.as_num()]
    },
    PieceMove {
        offset: SquareOffset::WEST,
        impossible: BB_FILES[File::A.as_num()]
    },
    PieceMove {
        offset: SquareOffset::NORTH_WEST,
        impossible: BB_RANKS[Rank::R8.as_num()].union(BB_FILES[File::A.as_num()])
    },
    PieceMove {
        offset: SquareOffset::NORTH_EAST,
        impossible: BB_RANKS[Rank::R8.as_num()].union(BB_FILES[File::H.as_num()])
    },
    PieceMove {
        offset: SquareOffset::SOUTH_WEST,
        impossible: BB_RANKS[Rank::R1.as_num()].union(BB_FILES[File::A.as_num()])
    },
    PieceMove {
        offset: SquareOffset::SOUTH_EAST,
        impossible: BB_RANKS[Rank::R1.as_num()].union(BB_FILES[File::H.as_num()])
    }
];

const KNIGHT_MOVES: [PieceMove; 8] = [
    PieceMove {
        offset: SquareOffset::KNIGHT_NNW,
        impossible: BB_RANKS[Rank::R8.as_num()].union(BB_RANKS[Rank::R7.as_num()]).union(BB_FILES[File::A.as_num()])
    },
    PieceMove {
        offset: SquareOffset::KNIGHT_WNW,
        impossible: BB_RANKS[Rank::R8.as_num()].union(BB_FILES[File::A.as_num()]).union(BB_FILES[File::B.as_num()])
    },
    PieceMove {
        offset: SquareOffset::KNIGHT_WSW,
        impossible: BB_RANKS[Rank::R1.as_num()].union(BB_FILES[File::A.as_num()]).union(BB_FILES[File::B.as_num()])
    },
    PieceMove {
        offset: SquareOffset::KNIGHT_SWS,
        impossible: BB_RANKS[Rank::R1.as_num()].union(BB_RANKS[Rank::R2.as_num()]).union(BB_FILES[File::A.as_num()])
    },
    PieceMove {
        offset: SquareOffset::KNIGHT_SES,
        impossible: BB_RANKS[Rank::R1.as_num()].union(BB_RANKS[Rank::R2.as_num()]).union(BB_FILES[File::H.as_num()])
    },
    PieceMove {
        offset: SquareOffset::KNIGHT_ESE,
        impossible: BB_RANKS[Rank::R1.as_num()].union(BB_FILES[File::H.as_num()]).union(BB_FILES[File::G.as_num()])
    },
    PieceMove {
        offset: SquareOffset::KNIGHT_ENE,
        impossible: BB_RANKS[Rank::R8.as_num()].union(BB_FILES[File::H.as_num()]).union(BB_FILES[File::G.as_num()])
    },
    PieceMove {
        offset: SquareOffset::KNIGHT_NNE,
        impossible: BB_RANKS[Rank::R8.as_num()].union(BB_RANKS[Rank::R7.as_num()]).union(BB_FILES[File::H.as_num()])
    },
];

const ROOK_SLIDES: [PieceMove; 4] = [
    PieceMove {
        offset: SquareOffset::NORTH,
        impossible: BB_RANKS[Rank::R8.as_num()]
    },
    PieceMove {
        offset: SquareOffset::SOUTH,
        impossible: BB_RANKS[Rank::R1.as_num()]
    },
    PieceMove {
        offset: SquareOffset::EAST,
        impossible: BB_FILES[File::H.as_num()]
    },
    PieceMove {
        offset: SquareOffset::WEST,
        impossible: BB_FILES[File::A.as_num()]
    }
];

const BISHOP_SLIDES: [PieceMove; 4] = [
    PieceMove {
        offset: SquareOffset::NORTH_WEST,
        impossible: BB_RANKS[Rank::R8.as_num()].union(BB_FILES[File::A.as_num()])
    },
    PieceMove {
        offset: SquareOffset::NORTH_EAST,
        impossible: BB_RANKS[Rank::R8.as_num()].union(BB_FILES[File::H.as_num()])
    },
    PieceMove {
        offset: SquareOffset::SOUTH_WEST,
        impossible: BB_RANKS[Rank::R1.as_num()].union(BB_FILES[File::A.as_num()])
    },
    PieceMove {
        offset: SquareOffset::SOUTH_EAST,
        impossible: BB_RANKS[Rank::R1.as_num()].union(BB_FILES[File::H.as_num()])
    }
];

// Indexed by PieceKind. Pawn and Queen have no entry here: pawn moves aren't modelled as
// PieceMove offsets, and queen moves are the union of the rook and bishop slides.
pub(super) const PIECE_DIRECTIONS: PerPiece<&[PieceMove]> = PerPiece::from_array([
    &[],            // Pawn
    &KNIGHT_MOVES,
    &BISHOP_SLIDES,
    &ROOK_SLIDES,
    &[],            // Queen
    &KING_MOVES,
]);


// Guaranteed moves + potential takes
#[derive(Clone, Copy, Debug)]
pub struct MoveTakePair {
    pub moves: Bitboard,
    pub potential_takes: Bitboard
}

pub const PRECALCULATED_PIECE_KINDS = [PieceKind::Bishop, PieceKind::King, PieceKind::Knight, PieceKind::Rook];
pub const KING_BLOCKER_COMBINATIONS: usize = 2usize.pow(8);
pub const KNIGHT_BLOCKER_COMBINATIONS: usize =  2usize.pow(8);
pub const BISHOP_BLOCKER_COMBINATIONS: usize = 2usize.pow(13);
pub const ROOK_BLOCKER_COMBINATIONS: usize = 2usize.pow(14);


// Holds the lookup tables
struct MoveGen {
    // Hold the generated masks, mapping square -> available moves on an empty board.
    // Indexed by piece kind; Pawn and Queen entries are unused (see PIECE_DIRECTIONS).
    move_mask: PerPiece<PerSquare<Bitboard>>,

    // Mapping of (Square + Blockers) -> Moves
    king_moves: PerSquare<[Bitboard; KING_BLOCKER_COMBINATIONS]>,
    knight_moves: PerSquare<[Bitboard; KNIGHT_BLOCKER_COMBINATIONS]>,
    // Mapping of (Square + Blockers) -> Moves + Potential Takes
    // Use a vector as to not bust stack limit. stack def is left here to show what we are trying to do
    // TODO: test increasing stack size limit and moving to stack.
    // TODO: test whether using PerSquare slows performance at all vs raw array
    // bishop_move_and_takes: PerSquare<[MoveTakePair; BISHOP_BLOCKER_COMBINATIONS]>,
    // rook_move_and_takes: PerSquare<[MoveTakePair; ROOK_BLOCKER_COMBINATIONS]>
    bishop_move_and_takes: PerSquare<Vec<MoveTakePair>>,
    rook_move_and_takes: PerSquare<Vec<MoveTakePair>>,
}

impl MoveGen {
    pub fn init() -> Self {
        let mut move_mask: PerPiece<PerSquare<Bitboard>> = PerPiece::new(PerSquare::new(Bitboard(0)));

        let mut king_moves: PerSquare<[Bitboard; KING_BLOCKER_COMBINATIONS]> = PerSquare::new([Bitboard(0); KING_BLOCKER_COMBINATIONS]);
        let mut knight_moves: PerSquare<[Bitboard; KNIGHT_BLOCKER_COMBINATIONS]> = PerSquare::new([Bitboard(0); KNIGHT_BLOCKER_COMBINATIONS]);
        let mut bishop_move_and_takes: PerSquare<Vec<MoveTakePair>> = PerSquare::new(vec![MoveTakePair{moves: Bitboard(0), potential_takes: Bitboard(0)}; BISHOP_BLOCKER_COMBINATIONS]);
        let mut rook_move_and_takes: PerSquare<Vec<MoveTakePair>> = PerSquare::new(vec![MoveTakePair{moves: Bitboard(0), potential_takes: Bitboard(0)}; ROOK_BLOCKER_COMBINATIONS]);

        // Generate blank move masks
        for s in Square::iter() {
            for piece_kind in PRECALCULATED_PIECE_KINDS {
                    move_mask[piece_kind][s] = MoveGen::calculate_moves_from_square(s, piece_kind, Bitboard::from(0)).moves;
                }
            }
        }

        // Process Blocker combos
        // For every square and piecetype, iterate through 0..MAX_BLOCKERS (non-inclusive), project that onto the move path, and calculate the moves.
        for pk in PRECALCULATED_PIECE_KINDS {
            let max_blockers = match pk {
                PieceKind::Bishop => BISHOP_BLOCKER_COMBINATIONS,
                PieceKind::King => KING_BLOCKER_COMBINATIONS,
                PieceKind::Knight => KNIGHT_BLOCKER_COMBINATIONS,
                PieceKind::Rook => ROOK_BLOCKER_COMBINATIONS
            };
            for s in Square::iter() {
                for n: usize in 0..max_blockers {
                    let blockers = Bitboard::from(n);
                    move_take_pair = MoveGen::calculate_moves_from_square(s, piece_kind, blockers).moves;
                }
            }
        }
        }


        Self {
            move_mask,
            king_moves,
            knight_moves,
            bishop_move_and_takes,
            rook_move_and_takes,
        }
    }

    // For a given square and piece type, calculate all the possible moves with the given blockers (square occupancy).
    // Does not calculate a) pawn moves or b) castling.
    // Pawn moves may be added here in the future, still undecided.
    // This function is used for initial computation and then caching, should not be used in live search / movegen path.
    //
    // Returns a MoveTakePair with 2 bitboards:
    //    - moves: Bitboard of all the blank squares the piece can move to
    //    - potential_takes: Bitboard of occupied squares that mark potential takes. Note that this includes squares with the current side occupying,
    //                       so will need to & with enemy occupancy to confirm.
    fn calculate_moves_from_square(square: Square, piece_kind: PieceKind, blocker_occupancy: Bitboard) -> MoveTakePair {
        let directions = PIECE_DIRECTIONS[piece_kind];
        let square_bb = square.bitboard();
        let mut moves = Bitboard::from(0);
        for d in directions {
            match piece_kind {
                PieceKind::King | PieceKind::Knight => {
                    // For King and Knight, just make the move and add to moves
                    if (square_bb & d.impossible) != Bitboard::EMPTY { continue; }
                    if d.offset > 0 {
                        moves |= square_bb << d.offset as u32;
                    } else {
                        moves |= square_bb >> -d.offset as u32;
                    }
                },
                PieceKind::Bishop | PieceKind::Rook => {
                    // For Bishop and Rook, keep pushing in direction adding to moves as you go
                    // until you hit an impossible move (end of board) or another piece.
                    let mut curr = square_bb;
                    while (curr & d.impossible) == Bitboard::EMPTY {
                        if d.offset > 0 {
                            curr = square_bb << d.offset as u32;
                        } else {
                            curr = square_bb >> -d.offset as u32;
                        }
                        moves |= curr;
                        if (curr & blocker_occupancy) != Bitboard::EMPTY { break; }
                    }
                },
                _ => { panic!("Invalid piece in calculate_moves_from_square()")}
            } 
        }

        MoveTakePair { moves: moves & !blocker_occupancy, potential_takes: moves & blocker_occupancy }
    }

}


// /// Moves following piece movement rules, without filtering for king safety.
pub fn pseudo_legal_moves(pos: &Position) -> Vec<Move> {
    unimplemented!()
}

pub fn is_attacked(pos: &Position, Square: Square) -> bool {
    unimplemented!()
}

// /// The game's current status, derived from `legal_moves` and draw conditions.
// pub fn game_status(pos: &Position) -> GameStatus {
//     unimplemented!()
// }
