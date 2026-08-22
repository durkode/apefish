
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaChaRng;
use crate::{PieceKind, Square, basetypes::{CastlingRights, PerPiece, PerSide, PerSquare, Side}};


pub type ZobristKey = u64;
const RNG_SEED: [u8; 32] = [54; 32];

#[derive(Debug)]
pub struct ZobristRandoms {
    piece_randoms: PerSide<PerPiece<PerSquare<ZobristKey>>>,
    side_randoms: PerSide<ZobristKey>,
    castling_randoms: [ZobristKey; CastlingRights::NUM_RIGHTS_COMBOS],
    // Technically this could be much smaller, but easier to just do for every square.
    ep_square_randoms: PerSquare<ZobristKey>,
    ep_empty_random: ZobristKey,
}

impl ZobristRandoms {
    pub fn new() -> Self {
        // TODO: inject this.
        let mut random = ChaChaRng::from_seed(RNG_SEED);

        let piece_randoms = PerSide::from_array(std::array::from_fn(|_|
            PerPiece::from_array(std::array::from_fn(|_|
                PerSquare::from_array(std::array::from_fn(|_| random.random::<ZobristKey>()))
            ))
        ));

        let side_randoms = PerSide::from_array(
            std::array::from_fn(|_| random.random::<ZobristKey>())
        );
        
        let castling_randoms: [ZobristKey; CastlingRights::NUM_RIGHTS_COMBOS] = std::array::from_fn(|_| random.random::<ZobristKey>());
        
        let ep_square_randoms = PerSquare::from_array(
            std::array::from_fn(|_| random.random::<ZobristKey>())
        );
        let ep_empty_random = random.random::<ZobristKey>();

        ZobristRandoms { 
            piece_randoms, 
            side_randoms, 
            castling_randoms,
            ep_square_randoms, 
            ep_empty_random 
        }
    }

    pub fn piece_key(&self, side: Side, piece_kind: PieceKind, square: Square) -> ZobristKey {
        self.piece_randoms[side][piece_kind][square]
    }

    pub fn side_key(&self, side: Side) -> ZobristKey {
        self.side_randoms[side]
    }

    pub fn castling_key(&self, rights: u8) -> ZobristKey {
        self.castling_randoms[rights as usize]
    }

    pub fn ep_key(&self, ep_square: Option<Square>) -> ZobristKey {
        match ep_square {
            Some(s) => self.ep_square_randoms[s],
            None => self.ep_empty_random
        }
    }

}