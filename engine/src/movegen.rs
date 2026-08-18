// //! Move generation, legality, and game-end detection.

use strum::{IntoEnumIterator};

use crate::Side;
use crate::basetypes::{BB_FILES, BB_RANKS, Bitboard, EnumKey, EnumMap, File, IndexedPieceKind, PieceKind, SlidingPieceKind, Move, PerSquare, Rank, Square};
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

// Pawn and Queen have no entry: pawn moves aren't modelled as PieceMove offsets,
// and queen moves are the union of the rook and bishop slides.
pub(super) const fn indexed_piece_directions(piece: IndexedPieceKind) -> &'static [PieceMove] {
    match piece {
        IndexedPieceKind::Knight => &KNIGHT_MOVES,
        IndexedPieceKind::Bishop => &BISHOP_SLIDES,
        IndexedPieceKind::Rook => &ROOK_SLIDES,
        IndexedPieceKind::King => &KING_MOVES,
    }
}

pub const BISHOP_BLOCKER_COMBINATIONS: u64 = 2u64.pow(9);
pub const ROOK_BLOCKER_COMBINATIONS: u64 = 2u64.pow(12);

// Holds the lookup tables
#[derive(Debug)]
pub struct MoveGen {
    // For a given indexed piece (Bishop, King, Knight, Rook), map from square to available moves on empty board
    raw_move: EnumMap<IndexedPieceKind, PerSquare<Bitboard>>,
    // For a given sliding piece, map from square to all the positions blockers can be that affect moves
    blocker_mask: EnumMap<SlidingPieceKind, PerSquare<Bitboard>>,
    // For a given sliding piece and square, hold a vector of available moves indexed by blocker hash.
    sliding_moves: EnumMap<SlidingPieceKind, PerSquare<Vec<Bitboard>>>,
}

impl MoveGen {
    pub fn init() -> Self {
        let mut raw_move: EnumMap<IndexedPieceKind, PerSquare<Bitboard>> = EnumMap::new(PerSquare::new(Bitboard::from(0)));
        let mut blocker_mask: EnumMap<SlidingPieceKind, PerSquare<Bitboard>> = EnumMap::new(PerSquare::new(Bitboard::from(0)));
        let mut sliding_moves: EnumMap<SlidingPieceKind, PerSquare<Vec<Bitboard>>> = EnumMap::from_fn(|_| PerSquare::new(vec![]));
        // Initialise sliding move vectors to be the right size
        for (spk, x) in sliding_moves.iter_mut() {
            for (_, v) in x.iter_mut() {
                let vec_length = match spk {
                    SlidingPieceKind::Bishop => BISHOP_BLOCKER_COMBINATIONS as usize,
                    SlidingPieceKind::Rook => ROOK_BLOCKER_COMBINATIONS as usize,
                };
                v.resize_with(vec_length, Default::default);
            } 
        }

        // Generate move and blocker masks
        let edge_of_board = BB_RANKS[Rank::R1.as_num()] | BB_RANKS[Rank::R8.as_num()]  | BB_FILES[File::A.as_num()] | BB_FILES[File::H.as_num()];
        let corners = Square::A1.bitboard() | Square::A8.bitboard() | Square::H1.bitboard() | Square::H8.bitboard();
        for ipk in IndexedPieceKind::iter() {
            for s in Square::iter() {
                let moves = MoveGen::calculate_moves_from_square(s, ipk.into(), Bitboard::from(0));
                raw_move[ipk][s] = moves;
                if let Ok(spk) = SlidingPieceKind::try_from(PieceKind::from(ipk)) {
                    let side_squares = match s.rank() {
                        r@ Rank::R1| r@ Rank::R8 => BB_RANKS[r.as_num()],
                        _ => Bitboard::from(0)
                    } | match s.file() {
                        f@ File::A| f@ File::H => BB_FILES[f.as_num()],
                        _ => Bitboard::from(0)
                    };
                    // blocker mask for rook is moves - edge of board + edge of board square is on - corners (cumulates in that order)
                    blocker_mask[spk][s] = match spk {
                        SlidingPieceKind::Rook => ((moves & !edge_of_board) | side_squares) & !corners,
                        SlidingPieceKind::Bishop => moves & !edge_of_board
                    }
                }
            }
        }

        // Process Blocker combos
        // For every square and SlidingPieceKind, iterate through 0..MAX_BLOCKERS (non-inclusive), project that onto the move path, and calculate the moves.
        for spk in SlidingPieceKind::iter() {
            let ipk = IndexedPieceKind::try_from(PieceKind::from(spk)).unwrap();
            for s in Square::iter() {
                let blocker_mask = blocker_mask[spk][s].to();

                // Use the Carrie Rippler algo to enumerate over all possible blockers for a given mask
                let mut blocker: u64 = 0;
                loop {
                    blocker = blocker.wrapping_sub(blocker_mask) & blocker_mask;
                    let bb = Bitboard::from(blocker);
                    let moves = MoveGen::calculate_moves_from_square(s, ipk, bb);
                    
                    let blocker_mask_bb = Bitboard::from(blocker_mask);

                    sliding_moves[spk][s][bb.compressed_index(blocker_mask)] = moves;
                    if blocker == 0 {
                        break;
                    }
                }
            }
        }

        Self {
            raw_move,
            blocker_mask,
            sliding_moves,
        }
    }

    // For a given square and piece type, calculate all the possible moves with the given blockers (square occupancy).
    // Does not calculate a) pawn moves or b) castling.
    // Pawn moves may be added here in the future, still undecided.
    // This function is used for initial computation and then caching, should not be used in live search / movegen path.
    fn calculate_moves_from_square(square: Square, piece_kind: IndexedPieceKind, blocker_occupancy: Bitboard) -> Bitboard {
        let directions = indexed_piece_directions(piece_kind);
        let square_bb = square.bitboard();
        let mut moves = Bitboard::from(0);
        for d in directions {
            match piece_kind {
                IndexedPieceKind::King | IndexedPieceKind::Knight => {
                    // For King and Knight, just make the move and add to moves
                    if (square_bb & d.impossible) != Bitboard::EMPTY { continue; }
                    if d.offset > 0 {
                        moves |= square_bb << d.offset as u32;
                    } else {
                        moves |= square_bb >> -d.offset as u32;
                    }
                },
                IndexedPieceKind::Bishop | IndexedPieceKind::Rook => {
                    // For Bishop and Rook, keep pushing in direction adding to moves as you go
                    // until you hit an impossible move (end of board) or another piece.
                    let mut curr = square_bb;
                    while (curr & d.impossible) == Bitboard::EMPTY {
                        if d.offset > 0 {
                            curr = curr << d.offset as u32;
                        } else {
                            curr = curr >> -d.offset as u32;
                        }
                        moves |= curr;
                        if (curr & blocker_occupancy) != Bitboard::EMPTY { break; }
                    }
                },
                _ => { panic!("Invalid piece in calculate_moves_from_square()")}
            } 
        }

        moves
    }

    // /// Moves following piece movement rules, without filtering for king safety.
    pub fn pseudo_legal_moves(&self, pos: &Position) -> Vec<Move> {
        let mut moves: Vec<Move> = vec![];
        

        for pk in PieceKind::iter() {
            for from_square in pos.pieces[pos.state.active_side][pk].iter_squares() {
                match pk {
                    PieceKind::Pawn => {}, // TODO
                    PieceKind::Knight | PieceKind::King | PieceKind::Bishop | PieceKind::Rook => {
                        self.append_piece_moves(&mut moves, pos, pk, from_square);
                    },
                    PieceKind::Queen => {
                        self.append_piece_moves(&mut moves, pos, PieceKind::Bishop, from_square);
                        self.append_piece_moves(&mut moves, pos, PieceKind::Rook, from_square);
                    }
                }
            }
        }

        moves
    }

    fn append_piece_moves(&self, moves: &mut Vec<Move>, pos: &Position, pk: PieceKind, from_square: Square) {
        let active_side = pos.state.active_side;
        let other_side = active_side.other();
        let all_blockers = pos.sides_pieces[active_side] | pos.sides_pieces[other_side];
        match pk {
            PieceKind::King | PieceKind::Knight => {
                let move_map = self.raw_move[IndexedPieceKind::try_from(pk).unwrap()];
                let square_moves = move_map[from_square] & !pos.sides_pieces[active_side];
                for ts in square_moves.iter_squares() {
                    moves.push(Move::new(
                        from_square,
                        ts,
                        pk,
                        None,
                        false,
                        false,
                    ));
                }
            },
            PieceKind::Pawn => {} // TODO
            PieceKind::Bishop | PieceKind::Rook => {
                let spk = SlidingPieceKind::try_from(pk).unwrap();
                let blocker_mask = self.blocker_mask[spk][from_square];
                let blockers = blocker_mask & all_blockers;
                let square_moves = self.sliding_moves[spk][from_square][blockers.compressed_index(blocker_mask.to())] & !pos.sides_pieces[active_side];
                for ts in square_moves.iter_squares() {
                    moves.push(Move::new(
                        from_square,
                        ts,
                        pk,
                        None,
                        false,
                        false
                    ))
                }
            }
            PieceKind::Queen => { panic!("Should never call append_piece_moves with Queen")}

        }
    }

    pub fn is_attacked(pos: &Position, Square: Square) -> bool {
        unimplemented!()
    }

    // /// The game's current status, derived from `legal_moves` and draw conditions.
    // pub fn game_status(pos: &Position) -> GameStatus {
    //     unimplemented!()
    // }

}
