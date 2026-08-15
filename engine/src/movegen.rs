// //! Move generation, legality, and game-end detection.


use crate::Square;
use crate::basetypes::{BB_RANKS, BB_FILES, Bitboard, File, Move, Rank};
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

pub(super) struct KingOffset;

impl KingOffset {
    pub const NORTH: i8 = 8;
    pub const SOUTH: i8 = -8;
    pub const EAST: i8 = 1;
    pub const WEST: i8 = -1;
    pub const NORTH_WEST: i8 = 7;
    pub const SOUTH_EAST: i8 = -7;
    pub const NORTH_EAST: i8 = 9;
    pub const SOUTH_WEST: i8 = -9;
}

pub(super) struct KingMove {
    pub offset: i8,
    pub impossible: Bitboard
}

pub(super) struct KnightMove {
    pub offset: i8,
    pub impossible: Bitboard
}

pub(super) struct RookSlide {
    pub offset: i8,
    pub impossible: Bitboard
}

pub(super) struct BishopSlide {
    pub offset: i8,
    pub impossible: Bitboard
}

pub(super) const KING_MOVES: [KingMove; 8] = [
    KingMove {
        offset: KingOffset::NORTH,
        impossible: BB_RANKS[Rank::R8.as_num()]
    },
    KingMove {
        offset: KingOffset::SOUTH,
        impossible: BB_RANKS[Rank::R1.as_num()]
    },
    KingMove {
        offset: KingOffset::EAST,
        impossible: BB_FILES[File::H.as_num()]
    },
    KingMove {
        offset: KingOffset::WEST,
        impossible: BB_FILES[File::A.as_num()]
    },
    KingMove {
        offset: KingOffset::NORTH_WEST,
        impossible: BB_RANKS[Rank::R8.as_num()].union(BB_FILES[File::A.as_num()])
    },
    KingMove {
        offset: KingOffset::NORTH_EAST,
        impossible: BB_RANKS[Rank::R8.as_num()].union(BB_FILES[File::H.as_num()])
    },
    KingMove {
        offset: KingOffset::SOUTH_WEST,
        impossible: BB_RANKS[Rank::R1.as_num()].union(BB_FILES[File::A.as_num()])
    },
    KingMove {
        offset: KingOffset::SOUTH_EAST,
        impossible: BB_RANKS[Rank::R1.as_num()].union(BB_FILES[File::H.as_num()])
    }
];


struct MoveGen {

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
