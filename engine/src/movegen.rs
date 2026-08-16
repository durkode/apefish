// //! Move generation, legality, and game-end detection.


use strum::{EnumCount, IntoEnumIterator};

use crate::{PieceKind, Square};
use crate::basetypes::{BB_FILES, BB_RANKS, Bitboard, File, Move, PerSquare, Rank};
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

pub(super) const KING_MOVES: [PieceMove; 8] = [
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

pub(super) const KNIGHT_MOVES: [PieceMove; 8] = [
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

pub(super) const ROOK_SLIDES: [PieceMove; 4] = [
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

pub(super) const BISHOP_SLIDES: [PieceMove; 4] = [
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

// Guaranteed moves + potential takes
#[derive(Clone, Copy, Debug)]
pub struct MoveTakePair {
    pub moves: Bitboard,
    pub potential_takes: Bitboard
}

pub const KING_BLOCKER_COMBINATIONS: usize = 2usize.pow(8);
pub const KNIGHT_BLOCKER_COMBINATIONS: usize =  2usize.pow(8);
pub const BISHOP_BLOCKER_COMBINATIONS: usize = 2usize.pow(13);
pub const ROOK_BLOCKER_COMBINATIONS: usize = 2usize.pow(14);

// Holds the lookup tables
struct MoveGen {
    // Hold the generated masks, mapping square -> available moves on an empty board
    king_move_mask: PerSquare<Bitboard>,
    knight_move_mask: PerSquare<Bitboard>,
    bishop_move_mask: PerSquare<Bitboard>,
    rook_move_mask: PerSquare<Bitboard>,

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
        let mut king_move_mask: PerSquare<Bitboard> = PerSquare::new(Bitboard(0));
        let mut knight_move_mask: PerSquare<Bitboard> = PerSquare::new(Bitboard(0));
        let mut bishop_move_mask: PerSquare<Bitboard> = PerSquare::new(Bitboard(0));
        let mut rook_move_mask: PerSquare<Bitboard> = PerSquare::new(Bitboard(0));

        let mut king_moves: PerSquare<[Bitboard; KING_BLOCKER_COMBINATIONS]> = PerSquare::new([Bitboard(0); KING_BLOCKER_COMBINATIONS]);
        let mut knight_moves: PerSquare<[Bitboard; KNIGHT_BLOCKER_COMBINATIONS]> = PerSquare::new([Bitboard(0); KNIGHT_BLOCKER_COMBINATIONS]);
        let mut bishop_move_and_takes: PerSquare<Vec<MoveTakePair>> = PerSquare::new(vec![MoveTakePair{moves: Bitboard(0), potential_takes: Bitboard(0)}; BISHOP_BLOCKER_COMBINATIONS]);
        let mut rook_move_and_takes: PerSquare<Vec<MoveTakePair>> = PerSquare::new(vec![MoveTakePair{moves: Bitboard(0), potential_takes: Bitboard(0)}; ROOK_BLOCKER_COMBINATIONS]);

        // Process Masks
        for s in Square::iter() {
            for piece_kind in PieceKind::iter() {
                let movement = Bitboard::from(0);
                match piece_kind {
                    PieceKind::King | PieceKind::Knight => {

                    },
                    PieceKind::Bishop | PieceKind::Rook => {

                    },
                    _ => {}
                }
            }
        }

        // Process Blocker combos


        Self {
            king_move_mask,
            knight_move_mask,
            bishop_move_mask,
            rook_move_mask,
            king_moves,
            knight_moves,
            bishop_move_and_takes,
            rook_move_and_takes,
        }
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
