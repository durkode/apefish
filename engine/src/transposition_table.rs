use std::fmt::Debug;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use crate::basetypes::{MATE_BOUND, Move, Score};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ScoreBound {
    // 1 index so 0 index is explicitly a missing entry when marshalled into a TTEntry.data field
    Lower = 1,
    Upper = 2,
    Exact = 3
}

impl ScoreBound {
    fn from_bits(bits: u64) -> Option<ScoreBound> {
        match bits {
            1 => Some(ScoreBound::Lower),
            2 => Some(ScoreBound::Upper),
            3 => Some(ScoreBound::Exact),
            _ => None
        }
    }
}

#[derive(Debug, Default)]
struct TTEntry {
    key: AtomicU64,
    data: AtomicU64
}

impl TTEntry {
    pub fn is_data_empty(&self) -> bool {
        self.data.load(Ordering::Relaxed) == 0
    }
}

#[derive(Copy, Clone, Debug)]
pub struct TTHit {
    pub mv: Move,
    pub score: Score,
    pub depth: u8,
    pub bound: ScoreBound,
    pub search_iteration: u8,
}

impl TTHit {
    // Marshall a Table Hit into a single u64
    // bits  0-15: move
    // bits 16-31: score as i16
    // bits 32-39: depth
    // bits 40-41: bound
    // bits 42-47: search_iteration of the entry
    // 
    // Note: That leaves 16 bits for use later, likely as 

    pub fn marshall(&self) -> u64 {
        let score = self.score;
        debug_assert!(self.score.abs() <= i16::MAX as i32, "score {score} does not fit the TT field");
        debug_assert!(self.search_iteration <= SEARCH_ITERATION_NUM_MASK, "search iteration is outside bounds");

        (self.mv.bits() as u64)
            | ((self.score as u16 as u64) << 16)
            | ((self.depth as u64) << 32)
            | ((self.bound as u64) << 40)
            | ((self.search_iteration as u64) << 42)
    }

    // Unmarshall bits into a TTHit
    // Assume bits are safe
    pub fn unmarshall(bits: u64) -> Self {
        TTHit { 
            mv: Move::from_bits(bits as u16), 
            score: ((bits >> 16) & 0xffff) as u16 as i16 as Score,
            depth: ((bits >> 32) & 0xff) as u8, 
            bound: ScoreBound::from_bits((bits >> 40) & 0x03).unwrap(), 
            search_iteration: (bits >> 42) as u8 & SEARCH_ITERATION_NUM_MASK 
        }
    }
}

const SEARCH_ITERATION_NUM_MASK: u8 = 0b0011_1111; // Take 6 LSB

pub trait TT: Debug + Send + Sync {
    fn new_search(&self);
    fn fetch(&self, zobrist: u64, ply: usize) -> Option<TTHit>;
    fn store(&self, zobrist: u64, mv: Move, score: Score, bound: ScoreBound, depth: u8, ply: i32);
    // Number of first 1000 slots that are full
    fn hashfull(&self) -> u16;
}

#[derive(Debug)]
pub struct ActiveTranspositionTable {
    // Use boxed slice rather than Vec to enforce it can't be resized
    entries: Box<[TTEntry]>,
    search_iteration: AtomicU8,
}

impl ActiveTranspositionTable {
    pub fn new(size_in_megabytes: usize) -> Self {
        if size_in_megabytes == 0 {
            panic!("TT size can't be 0")
        }
        Self::new_with_num_slots((size_in_megabytes * 1024 * 1024) / std::mem::size_of::<TTEntry>())
    }

    pub fn new_with_num_slots(num_slots: usize) -> Self {
        let mut entry_vec = Vec::new();
        entry_vec.resize_with(num_slots, TTEntry::default);
        Self { entries: entry_vec.into_boxed_slice(), search_iteration: AtomicU8::new(0)}
    }

    fn hash_zobrist(&self, zobrist: u64) -> usize {
        // TODO: More efficient ways to do this if we assume table size is a power of 2. Maybe investigate in the future.

        // Given zobrist should be randomly distributed across u64, multiplying by num slots should randomly distribute 64 MSB
        // of a u128 across 0..(num slots).
        // alternative think of as 1/z * num_slots = randomly distributed int in 0..num_slots.
        (((zobrist as u128) * (self.entries.len() as u128)) >> 64) as usize
    }
}

impl TT for ActiveTranspositionTable {

    fn new_search(&self) {
        // Increment by 1, wrapping at 64
        self.search_iteration.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            Some(v.wrapping_add(1) & SEARCH_ITERATION_NUM_MASK)
        }).unwrap();
    }

    fn hashfull(&self) -> u16 {
        if self.entries.len() < 1000 {
            return 0; // Not supported for tiny hash tables
        }
        self.entries[..1000].iter().filter(|x| !x.is_data_empty()).count() as u16
    }

    // Attempt to find an entry in the TT
    fn fetch(&self, zobrist: u64, ply: usize) -> Option<TTHit> {
        let entry = &self.entries[self.hash_zobrist(zobrist)];
        let key = entry.key.load(Ordering::Relaxed);
        let data = entry.data.load(Ordering::Relaxed);
        if key ^ data != zobrist {
            return None;
        }

        // TODO: Investigate unmarshalling only needed fields, might be quicker than the whole thing.
        let mut hit = TTHit::unmarshall(data);
        // Take the ply out of the score
        // TODO: move this to a common function w/ eval and store.
        if hit.score >= MATE_BOUND {
            hit.score -= ply as Score;
        } else if hit.score <= -MATE_BOUND {
            hit.score += ply as Score;
        }

        Some(hit)
    }

    fn store(&self, zobrist: u64, mv: Move, score: Score, bound: ScoreBound, depth: u8, ply: i32) {
        let entry = &self.entries[self.hash_zobrist(zobrist)];
        let old_data = entry.data.load(Ordering::Relaxed);
        let search_iteration = self.search_iteration.load(Ordering::Relaxed);

        // Only replace if newer search or greater depth at same search
        let replace_slot = {
            if old_data == 0 {
                true
            } else {
                let old_hit = TTHit::unmarshall(old_data);
                // Cast depth to i32 to deal with overflow
                old_hit.search_iteration != search_iteration || depth as i32 + 2 > old_hit.depth as i32
            }
        };
        if !replace_slot {
            return;
        }

        // TODO: Future search enhancements may store bounds in TT without providing a move. Likely will need to modify the signature to allow None Move
        // and if analysing the same position, use the found move instead. Must check the move is for the right position however
        // let old_key = entry.key.load(Ordering::Relaxed);
        // let table_hit_matches_position = old_key ^ old_data == zobrist;

        //TODO: as with fetch, factor out the mate bound + ply logic into common function
        let score = if score >= MATE_BOUND {
            score + ply
        } else if score <= -MATE_BOUND {
            score - ply
        } else {
            score
        };

        let data = TTHit::marshall(&TTHit {
            mv,
            score,
            depth,
            bound,
            search_iteration
        });
        entry.key.store(zobrist ^ data, Ordering::Relaxed);
        entry.data.store(data, Ordering::Relaxed);
    }

}


// No-op transposition table
#[derive(Debug)]
pub struct NoopTranspositionTable;
impl TT for NoopTranspositionTable {
    fn new_search(&self) {}
    #[allow(unused_variables)]
    fn fetch(&self, zobrist: u64, ply: usize) -> Option<TTHit> {None}
    #[allow(unused_variables)]
    fn store(&self, zobrist: u64, mv: Move, score: Score, bound: ScoreBound, depth: u8, ply: i32) {}
    fn hashfull(&self) -> u16 { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basetypes::{PieceKind, Square};

    fn sample_move() -> Move {
        Move::new(Square::A1, Square::H8, None)
    }

    fn other_move() -> Move {
        Move::new(Square::A7, Square::A8, Some(PieceKind::Queen))
    }

    fn assert_hit_eq(actual: &TTHit, expected: &TTHit) {
        assert_eq!(actual.mv.bits(), expected.mv.bits());
        assert_eq!(actual.score, expected.score);
        assert_eq!(actual.depth, expected.depth);
        assert_eq!(actual.bound, expected.bound);
        assert_eq!(actual.search_iteration, expected.search_iteration);
    }

    fn assert_roundtrip(hit: TTHit) {
        let unmarshalled = TTHit::unmarshall(hit.marshall());
        assert_hit_eq(&unmarshalled, &hit);
    }

    fn write_raw_entry(table: &ActiveTranspositionTable, zobrist: u64, hit: &TTHit) {
        let idx = table.hash_zobrist(zobrist);
        let data = hit.marshall();
        table.entries[idx].key.store(zobrist ^ data, Ordering::Relaxed);
        table.entries[idx].data.store(data, Ordering::Relaxed);
    }

    fn raw_score_at(table: &ActiveTranspositionTable, zobrist: u64) -> Score {
        let idx = table.hash_zobrist(zobrist);
        let data = table.entries[idx].data.load(Ordering::Relaxed);
        TTHit::unmarshall(data).score
    }

    // --- TTHit marshall/unmarshall ---

    #[test]
    fn roundtrip_positive_score_all_bounds() {
        for bound in [ScoreBound::Lower, ScoreBound::Upper, ScoreBound::Exact] {
            assert_roundtrip(TTHit { mv: sample_move(), score: 1234, depth: 12, bound, search_iteration: 5 });
        }
    }

    #[test]
    fn roundtrip_negative_score() {
        assert_roundtrip(TTHit { mv: sample_move(), score: -1234, depth: 12, bound: ScoreBound::Exact, search_iteration: 5 });
    }

    #[test]
    fn roundtrip_zero_score() {
        assert_roundtrip(TTHit { mv: sample_move(), score: 0, depth: 0, bound: ScoreBound::Lower, search_iteration: 0 });
    }

    #[test]
    fn roundtrip_score_at_mate_bound() {
        assert_roundtrip(TTHit { mv: sample_move(), score: MATE_BOUND, depth: 30, bound: ScoreBound::Exact, search_iteration: 10 });
        assert_roundtrip(TTHit { mv: sample_move(), score: -MATE_BOUND, depth: 30, bound: ScoreBound::Exact, search_iteration: 10 });
    }

    #[test]
    fn roundtrip_score_near_i16_extremes() {
        assert_roundtrip(TTHit { mv: sample_move(), score: i16::MAX as Score, depth: 1, bound: ScoreBound::Upper, search_iteration: 1 });
        assert_roundtrip(TTHit { mv: sample_move(), score: -(i16::MAX as Score), depth: 1, bound: ScoreBound::Upper, search_iteration: 1 });
    }

    #[test]
    fn roundtrip_depth_extremes() {
        assert_roundtrip(TTHit { mv: sample_move(), score: 50, depth: 0, bound: ScoreBound::Exact, search_iteration: 0 });
        assert_roundtrip(TTHit { mv: sample_move(), score: 50, depth: 255, bound: ScoreBound::Exact, search_iteration: 0 });
    }

    #[test]
    fn roundtrip_search_iteration_extremes() {
        assert_roundtrip(TTHit { mv: sample_move(), score: 50, depth: 10, bound: ScoreBound::Exact, search_iteration: 0 });
        assert_roundtrip(TTHit { mv: sample_move(), score: 50, depth: 10, bound: ScoreBound::Exact, search_iteration: SEARCH_ITERATION_NUM_MASK });
    }

    #[test]
    fn roundtrip_move_bits_preserved() {
        let mv = other_move();
        let hit = TTHit { mv, score: 7, depth: 3, bound: ScoreBound::Lower, search_iteration: 2 };
        let unmarshalled = TTHit::unmarshall(hit.marshall());
        assert_eq!(unmarshalled.mv.from(), mv.from());
        assert_eq!(unmarshalled.mv.to(), mv.to());
        assert_eq!(unmarshalled.mv.promotion(), mv.promotion());
    }

    // --- ActiveTranspositionTable::fetch ---

    #[test]
    fn fetch_empty_slot_returns_none() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        assert!(table.fetch(0x1234_5678_9abc_def0, 0).is_none());
    }

    #[test]
    fn fetch_after_store_returns_hit() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist = 0x1111_2222_3333_4444;
        let mv = sample_move();
        table.store(zobrist, mv, 250, ScoreBound::Exact, 8, 0);

        let hit = table.fetch(zobrist, 0).expect("expected a cache hit");
        assert_eq!(hit.mv.bits(), mv.bits());
        assert_eq!(hit.score, 250);
        assert_eq!(hit.depth, 8);
        assert_eq!(hit.bound, ScoreBound::Exact);
        assert_eq!(hit.search_iteration, 0);
    }

    #[test]
    fn fetch_checksum_mismatch_returns_none() {
        let table = ActiveTranspositionTable::new_with_num_slots(1);
        let zobrist_a = 0xaaaa_bbbb_cccc_dddd;
        let zobrist_b = 0x1111_2222_3333_4444;
        table.store(zobrist_a, sample_move(), 100, ScoreBound::Exact, 5, 0);

        assert!(table.fetch(zobrist_b, 0).is_none());
    }

    #[test]
    fn fetch_adjusts_positive_mate_score_by_ply() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist = 0x2222_3333_4444_5555;
        write_raw_entry(&table, zobrist, &TTHit { mv: sample_move(), score: MATE_BOUND + 10, depth: 20, bound: ScoreBound::Exact, search_iteration: 3 });

        let hit = table.fetch(zobrist, 4).unwrap();
        assert_eq!(hit.score, MATE_BOUND + 10 - 4);
    }

    #[test]
    fn fetch_adjusts_negative_mate_score_by_ply() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist = 0x2222_3333_4444_5556;
        write_raw_entry(&table, zobrist, &TTHit { mv: sample_move(), score: -MATE_BOUND - 10, depth: 20, bound: ScoreBound::Exact, search_iteration: 3 });

        let hit = table.fetch(zobrist, 4).unwrap();
        assert_eq!(hit.score, -MATE_BOUND - 10 + 4);
    }

    #[test]
    fn fetch_leaves_normal_score_unchanged() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist = 0x2222_3333_4444_5557;
        write_raw_entry(&table, zobrist, &TTHit { mv: sample_move(), score: 500, depth: 20, bound: ScoreBound::Exact, search_iteration: 3 });

        let hit = table.fetch(zobrist, 4).unwrap();
        assert_eq!(hit.score, 500);
    }

    // --- ActiveTranspositionTable::store ---

    #[test]
    fn store_empty_slot_always_writes() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist = 0x3333_4444_5555_6666;
        table.store(zobrist, sample_move(), 42, ScoreBound::Lower, 1, 0);
        assert!(table.fetch(zobrist, 0).is_some());
    }

    #[test]
    fn store_different_search_iteration_replaces_regardless_of_depth() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist = 0x4444_5555_6666_7777;
        table.store(zobrist, sample_move(), 10, ScoreBound::Exact, 20, 0);
        table.new_search();
        table.store(zobrist, other_move(), 99, ScoreBound::Upper, 1, 0);

        let hit = table.fetch(zobrist, 0).unwrap();
        assert_eq!(hit.score, 99);
        assert_eq!(hit.depth, 1);
        assert_eq!(hit.bound, ScoreBound::Upper);
        assert_eq!(hit.search_iteration, 1);
    }

    #[test]
    fn store_same_iteration_depth_minus_one_replaces() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist = 0x5555_6666_7777_8888;
        table.store(zobrist, sample_move(), 10, ScoreBound::Exact, 20, 0);
        table.store(zobrist, other_move(), 20, ScoreBound::Exact, 19, 0);

        let hit = table.fetch(zobrist, 0).unwrap();
        assert_eq!(hit.score, 20);
        assert_eq!(hit.depth, 19);
    }

    #[test]
    fn store_same_iteration_depth_minus_two_does_not_replace() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist = 0x6666_7777_8888_9999;
        table.store(zobrist, sample_move(), 10, ScoreBound::Exact, 20, 0);
        table.store(zobrist, other_move(), 20, ScoreBound::Exact, 18, 0);

        let hit = table.fetch(zobrist, 0).unwrap();
        assert_eq!(hit.score, 10);
        assert_eq!(hit.depth, 20);
    }

    #[test]
    fn store_same_iteration_equal_depth_replaces() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist = 0x7777_8888_9999_aaaa;
        table.store(zobrist, sample_move(), 10, ScoreBound::Exact, 20, 0);
        table.store(zobrist, other_move(), 20, ScoreBound::Exact, 20, 0);

        let hit = table.fetch(zobrist, 0).unwrap();
        assert_eq!(hit.score, 20);
    }

    #[test]
    fn store_same_iteration_greater_depth_replaces() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist = 0x8888_9999_aaaa_bbbb;
        table.store(zobrist, sample_move(), 10, ScoreBound::Exact, 20, 0);
        table.store(zobrist, other_move(), 20, ScoreBound::Exact, 25, 0);

        let hit = table.fetch(zobrist, 0).unwrap();
        assert_eq!(hit.score, 20);
        assert_eq!(hit.depth, 25);
    }

    #[test]
    fn store_applies_ply_adjustment_to_mate_scores() {
        let table = ActiveTranspositionTable::new_with_num_slots(64);
        let zobrist_pos = 0x9999_aaaa_bbbb_cccc;
        let zobrist_neg = 0x9999_aaaa_bbbb_cccd;

        table.store(zobrist_pos, sample_move(), MATE_BOUND, ScoreBound::Exact, 10, 5);
        assert_eq!(raw_score_at(&table, zobrist_pos), MATE_BOUND + 5);

        table.store(zobrist_neg, sample_move(), -MATE_BOUND, ScoreBound::Exact, 10, 5);
        assert_eq!(raw_score_at(&table, zobrist_neg), -MATE_BOUND - 5);
    }

    #[test]
    fn store_collision_overwrites_and_invalidates_other_key() {
        let table = ActiveTranspositionTable::new_with_num_slots(1);
        let zobrist_a = 0xaaaa_1111_2222_3333;
        let zobrist_b = 0xbbbb_1111_2222_3333;

        table.store(zobrist_a, sample_move(), 10, ScoreBound::Exact, 5, 0);
        table.store(zobrist_b, other_move(), 20, ScoreBound::Exact, 5, 0);

        assert!(table.fetch(zobrist_a, 0).is_none());
        let hit = table.fetch(zobrist_b, 0).unwrap();
        assert_eq!(hit.score, 20);
    }

    #[test]
    fn new_search_increments_iteration() {
        let table = ActiveTranspositionTable::new_with_num_slots(1);
        assert_eq!(table.search_iteration.load(Ordering::Relaxed), 0);
        table.new_search();
        assert_eq!(table.search_iteration.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn new_search_wraps_at_63() {
        let table = ActiveTranspositionTable::new_with_num_slots(1);
        table.search_iteration.store(SEARCH_ITERATION_NUM_MASK, Ordering::Relaxed);
        table.new_search();
        assert_eq!(table.search_iteration.load(Ordering::Relaxed), 0);
    }
}