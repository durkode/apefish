
use rand::{RngExt, SeedableRng, random};
use rand_chacha::ChaChaRng;
use crate::basetypes::{CastlingRights, PerPiece, PerSide, PerSquare};


type ZR = u64;
const RNG_SEED: [u8; 32] = [54; 32];

pub struct ZobristRandoms {
    piece_randoms: PerSide<PerPiece<PerSquare<ZR>>>,
    side_randoms: PerSide<ZR>,
    castling_randoms: [ZR; CastlingRights::NUM_RIGHTS_COMBOS],
    ep_square_randoms: PerSquare<ZR>,
    ep_empty_random: ZR,
}

impl ZobristRandoms {
    pub fn new() -> Self {
        // TODO: inject this.
        let mut random = ChaChaRng::from_seed(RNG_SEED);

        let piece_randoms = PerSide::from_array(std::array::from_fn(|_|
            PerPiece::from_array(std::array::from_fn(|_|
                PerSquare::from_array(std::array::from_fn(|_| random.random::<ZR>()))
            ))
        ));

        let side_randoms = PerSide::from_array(
            std::array::from_fn(|_| random.random::<ZR>())
        );
        
        let castling_randoms: [ZR; CastlingRights::NUM_RIGHTS_COMBOS] = std::array::from_fn(|_| random.random::<ZR>());
        
        let ep_square_randoms = PerSquare::from_array(
            std::array::from_fn(|_| random.random::<ZR>())
        );
        let ep_empty_random = random.random::<ZR>();

        ZobristRandoms { 
            piece_randoms, 
            side_randoms, 
            castling_randoms,
            ep_square_randoms, 
            ep_empty_random 
        }
    }
}