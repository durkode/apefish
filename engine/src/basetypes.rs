//! Core value types shared across the engine (side, piece type, moves).

use strum::{EnumCount, IntoEnumIterator};
use subenum::subenum;

use std::{fmt, str::FromStr};

use crate::Position;

#[derive(Debug)]
pub enum GenericErr {
    SquareParseError,
    InvalidMove,
    IllegalMove,
    InvalidCastleSquares,
    InvalidCastleChecked
}

#[derive(Debug, strum::Display, Clone, Copy, PartialEq, Eq, strum::EnumCount, strum::EnumIter)]
#[repr(u8)]
pub enum Side {
    White = 0,
    Black,
}

impl Side {
    pub fn other(&self) -> Side {
        match self {
            Side::White => Side::Black,
            Side::Black => Side::White,
        }
    }
}

/// Piece kinds that move by sliding along a ray until blocked.
/// Piece kinds whose moves are generated from precomputed, per-square lookup tables.
#[subenum(SlidingPieceKind, IndexedPieceKind)]
#[derive(strum::Display, Clone, Copy, PartialEq, Eq, strum::EnumCount, strum::EnumIter)]
#[repr(u8)]
pub enum PieceKind {
    Pawn,
    #[subenum(IndexedPieceKind)]
    Knight,
    #[subenum(SlidingPieceKind, IndexedPieceKind)]
    Bishop,
    #[subenum(SlidingPieceKind, IndexedPieceKind)]
    Rook,
    Queen,
    #[subenum(IndexedPieceKind)]
    King,
}

impl fmt::Debug for PieceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Debug for IndexedPieceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (index {})", PieceKind::from(*self), self.index())
    }
}

impl fmt::Debug for SlidingPieceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (sliding)", PieceKind::from(*self))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub side: Side,
    pub kind: PieceKind,
}

impl Piece {

    pub const NO_PIECE_CHAR: char = '□';

    pub const fn to_unicode_char(self) -> char {
        match (self.side, self.kind) {
            (Side::White, PieceKind::Pawn) => '♟',
            (Side::White, PieceKind::Knight) => '♞',
            (Side::White, PieceKind::Bishop) => '♝',
            (Side::White, PieceKind::Rook) => '♜',
            (Side::White, PieceKind::Queen) => '♛',
            (Side::White, PieceKind::King) => '♚',
            (Side::Black, PieceKind::Pawn) => '♙',
            (Side::Black, PieceKind::Knight) => '♘',
            (Side::Black, PieceKind::Bishop) => '♗',
            (Side::Black, PieceKind::Rook) => '♖',
            (Side::Black, PieceKind::Queen) => '♕',
            (Side::Black, PieceKind::King) => '♔',
        }
    }
}

/// A single square on the board (a1 = 0 ... h8 = 63). Encoding TBD.
#[derive(Clone, Copy, PartialEq, Eq, strum::EnumCount, strum::EnumIter, strum::EnumString)]
#[strum(ascii_case_insensitive)]
#[repr(u8)]
pub enum Square {
    A1 = 0, B1, C1, D1, E1, F1, G1, H1,
    A2, B2, C2, D2, E2, F2, G2, H2,
    A3, B3, C3, D3, E3, F3, G3, H3,
    A4, B4, C4, D4, E4, F4, G4, H4,
    A5, B5, C5, D5, E5, F5, G5, H5,
    A6, B6, C6, D6, E6, F6, G6, H6,
    A7, B7, C7, D7, E7, F7, G7, H7,
    A8, B8, C8, D8, E8, F8, G8, H8,
}

impl Square {
    
    pub const fn bitboard(self) -> Bitboard {
        Bitboard(1u64 << self as u8)
    }

    pub fn from_string(from: &str) -> Result<Self, GenericErr> {
        match Square::from_str(from) {
            Ok(x) => Ok(x),
            Err(_) => Err(GenericErr::SquareParseError)
        }
    }

    pub fn to_string(self) -> String {
        let file_char = (b'a' + self.file() as u8) as char;
        let rank_num = self.rank() as u8 + 1;
        format!("{file_char}{rank_num}")
    }

    pub fn from_coords(file: File, rank: Rank) -> Self {
        // Unsafe code relies on File, Rank, Square remaining relationally static.
        unsafe { std::mem::transmute::<u8, Square>((file as u8) + (8 * rank as u8)) }
    }

    pub fn rank(self) -> Rank {
        unsafe { std::mem::transmute::<u8, Rank>((self as u8) / 8) }
    }

    pub fn file(self) -> File {
        unsafe { std::mem::transmute::<u8, File>((self as u8) % 8) }
    }

}

impl fmt::Debug for Square {
    /// Algebraic notation, e.g. "d4".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.to_string();
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumCount, strum::EnumIter)]
#[repr(u8)]
pub enum File {
    A = 0, 
    B, 
    C, 
    D, 
    E, 
    F, 
    G, 
    H
}

impl File {
    pub const fn as_num(self) -> usize {
        self as usize
    }

    pub fn iter() -> FileIter {
        <File as IntoEnumIterator>::iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumCount, strum::EnumIter)]
#[repr(u8)]
pub enum Rank {
    R1 = 0,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8 
}

impl Rank {
    pub const fn as_num(self) -> usize {
        self as usize
    }

    pub fn iter() -> RankIter {
        <Rank as IntoEnumIterator>::iter()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Move {
    // Internal representation of a move
    // TODO: For performance reasons, Move should really be compressed into either a u64 (with all data, like Rustic)
    // or a u16 (with minimal data like stockfish). For simplicity sake we will now just use a struct, but with plan to refactor later.
    from: Square,
    to: Square,
    piece_kind: PieceKind,
    promotion: Option<PieceKind>,
    captured: Option<PieceKind>,
    castling: bool,
    en_passant: bool,
}

impl Move {

    pub fn new(from: Square, to: Square, piece_kind: PieceKind, 
               promotion: Option<PieceKind>, captured: Option<PieceKind>, 
               castling: bool, en_passant: bool) -> Move {
        
        Move { from, to, piece_kind, promotion, captured, castling, en_passant }

    }

    pub fn to_input_move(&self) -> InputMove {
        InputMove { from: self.from, to: self.to, promotion: self.promotion }
    }

    pub fn from(&self) -> Square {self.from}
    pub fn to(&self) -> Square {self.to}
    pub fn piece(&self) -> PieceKind {self.piece_kind}
    pub fn promotion(&self) -> Option<PieceKind> {self.promotion}
    pub fn captured(&self) -> Option<PieceKind> {self.captured}
    pub fn castling(&self) -> bool {self.castling}
    pub fn en_passant(&self) -> bool {self.en_passant}
}

impl fmt::Display for Move {
    /// UCI-style notation, e.g. "e2e4" or "e7e8q" for a promotion.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}{:?}", self.from, self.to)?;
        if let Some(promotion) = self.promotion {
            let promo_char = match promotion {
                PieceKind::Queen => 'q',
                PieceKind::Rook => 'r',
                PieceKind::Bishop => 'b',
                PieceKind::Knight => 'n',
                PieceKind::Pawn | PieceKind::King => unreachable!("cannot promote to a pawn or king"),
            };
            write!(f, "{promo_char}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct InputMove {
    // A move input from a user of the engine.
    pub from: Square,
    pub to: Square,
    pub promotion: Option<PieceKind>
}

impl InputMove {
    // Take an move from a client, combine it with the position, to create an internal move.
    // Note, this assumes that the move is valid. If it is not valid, the internal move will be gibberish.
    pub fn to_internal_move(&self, pos: &Position) -> Result<Move, GenericErr> {
    
        let Some(from_piece) = pos.piece_by_square[self.from] else { return Err(GenericErr::InvalidMove)};
        let to_piece = pos.piece_by_square[self.to];
        let captured = pos.piece_by_square[self.to].map(|x| x.kind);
        let en_passant = from_piece.kind == PieceKind::Pawn && to_piece.is_none() && self.from.file() != self.to.file();
        Ok(Move {
            from: self.from,
            to: self.to,
            piece_kind: from_piece.kind,
            promotion: self.promotion,
            captured: captured,
            castling: match from_piece.kind {
                PieceKind::King => self.from.file().as_num().abs_diff(self.to.file().as_num()) == 2,
                _ => false,
            },
            en_passant,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumIter)]
pub enum CastlingDirection {
    WK,
    WQ,
    BK,
    BQ
}

impl CastlingDirection {

    const WQ_UNATTACKED_SQUARES: &[Square] = &[Square::E1, Square::D1, Square::C1];
    const WK_UNATTACKED_SQUARES: &[Square] = &[Square::E1, Square::F1, Square::G1];
    const BQ_UNATTACKED_SQUARES: &[Square] = &[Square::E8, Square::D8, Square::C8];
    const BK_UNATTACKED_SQUARES: &[Square] = &[Square::E8, Square::F8, Square::G8];

    pub fn direction(from: Square, to: Square) -> Option<CastlingDirection> {
        match (from, to) {
            (Square::E1, Square::C1) => Some(CastlingDirection::WQ),
            (Square::E1, Square::G1) => Some(CastlingDirection::WK),
            (Square::E8, Square::C8) => Some(CastlingDirection::BQ),
            (Square::E8, Square::G8) => Some(CastlingDirection::BK),
            _ => None
        }
    }

    pub fn rights(self) -> CastlingRights {
        match self {
            CastlingDirection::WQ => CastlingRights::WQ,
            CastlingDirection::WK => CastlingRights::WK,
            CastlingDirection::BQ => CastlingRights::BQ,
            CastlingDirection::BK => CastlingRights::BK,            
        }
    }

    pub fn unattacked_squares_required(self) -> &'static [Square] {
        match self {
            CastlingDirection::WQ => CastlingDirection::WQ_UNATTACKED_SQUARES,
            CastlingDirection::WK => CastlingDirection::WK_UNATTACKED_SQUARES,
            CastlingDirection::BQ => CastlingDirection::BQ_UNATTACKED_SQUARES,
            CastlingDirection::BK => CastlingDirection::BK_UNATTACKED_SQUARES,            
        }
    }

    pub fn rook_from(self) -> Square {
        match self {
            CastlingDirection::WQ => Square::A1,
            CastlingDirection::WK => Square::H1,
            CastlingDirection::BQ => Square::A8,
            CastlingDirection::BK => Square::H8,
        }
    }

    pub fn rook_to(self) -> Square {
        match self {
            CastlingDirection::WQ => Square::D1,
            CastlingDirection::WK => Square::F1,
            CastlingDirection::BQ => Square::D8,
            CastlingDirection::BK => Square::F8,
        }
    }

    pub fn king_from(self) -> Square {
        match self {
            CastlingDirection::WQ => Square::E1,
            CastlingDirection::WK => Square::E1,
            CastlingDirection::BQ => Square::E8,
            CastlingDirection::BK => Square::E8
        }
    }

    pub fn king_to(self) -> Square {
        match self {
            CastlingDirection::WQ => Square::C1,
            CastlingDirection::WK => Square::G1,
            CastlingDirection::BQ => Square::C8,
            CastlingDirection::BK => Square::G8
        }
    }

    pub fn empty_squares(self) -> &'static [Square] {
        match self {
            CastlingDirection::WQ => &[Square::D1, Square::C1, Square::B1],
            CastlingDirection::WK => &[Square::F1, Square::G1],
            CastlingDirection::BQ => &[Square::D8, Square::C8, Square::B8],
            CastlingDirection::BK => &[Square::F8, Square::G8],
        }
    }

    pub fn for_side(side: Side) -> &'static [Self] {
        match side {
            Side::White => &[CastlingDirection::WK, CastlingDirection::WQ],
            Side::Black => &[CastlingDirection::BK, CastlingDirection::BQ]
        }
    }
}

// Struct to both store castling rights, and castling related logic.
#[derive(Debug, Copy, Clone)]
pub struct CastlingRights {
    rights: u8
}

impl CastlingRights {
    pub const NONE: CastlingRights = CastlingRights{rights: 0};
    pub const WK: CastlingRights = CastlingRights{rights: 1};
    pub const WQ: CastlingRights = CastlingRights{rights: 2};
    pub const BK: CastlingRights = CastlingRights{rights: 4};
    pub const BQ: CastlingRights = CastlingRights{rights: 8};
    pub const ALL: CastlingRights = CastlingRights{rights: 15};

    pub fn new(rights: CastlingRights) -> Self {
        rights.clone()
    }

    pub fn any_rights(self) -> bool {
        return self.rights != 0
    }

    pub fn has_rights(self, direction: CastlingDirection) -> bool {
        (self.rights & direction.rights().rights) != 0
    }

    pub fn add_rights(&mut self, direction: CastlingDirection) {
        self.rights |= direction.rights().rights;
    }

    pub fn remove_rights(&mut self, direction: CastlingDirection) {
        self.rights &= !direction.rights().rights;
    }

    pub fn remove_rights_for_move(&mut self, side: Side, from: Square, piece_kind: PieceKind) {
        if self.any_rights() {
            match (side, from, piece_kind) {
                // White
                (Side::White, Square::E1, PieceKind::King) => { 
                    self.remove_rights(CastlingDirection::WK);
                    self.remove_rights(CastlingDirection::WQ);
                },
                (Side::White, Square::A1, PieceKind::Rook) => {
                    self.remove_rights(CastlingDirection::WQ);
                },
                (Side::White, Square::H1, PieceKind::Rook) => {
                    self.remove_rights(CastlingDirection::WK);
                },
                
                // Black
                (Side::Black, Square::E8, PieceKind::King) => { 
                    self.remove_rights(CastlingDirection::BK);
                    self.remove_rights(CastlingDirection::BQ);
                },
                (Side::Black, Square::A8, PieceKind::Rook) => {
                    self.remove_rights(CastlingDirection::BQ);
                },
                (Side::Black, Square::H8, PieceKind::Rook) => {
                    self.remove_rights(CastlingDirection::BK);
                },
                // No match
                _ => {}
            }
        }
    }
        
}

// Bitboards. This is a 'NewType' over a u64, and as such AI has been used to allow 
// bitwise operations on bitboards as if they were a u64.

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Bitboard(pub u64);

impl fmt::Debug for Bitboard {
    /// A list of the set squares, e.g. "[a1, e6, d4]".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter_squares()).finish()
    }
}

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const FULL: Bitboard = Bitboard(u64::MAX);

    pub const fn from(bits: u64) -> Self {
        Bitboard(bits)
    }

    pub fn to(&self) -> u64 {
        self.0
    }

    pub fn shift_offset(&self, offset: i8) -> Self {
        match offset > 0 {
            true => *self << (offset as u32),
            false => *self >> (-offset as u32)
        }
    }

    pub const fn union(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 | other.0)
    }

    // If this bitboard is a mask, return the corresponding square
    // TODO: perhaps this should panic instead of return an option for perf??
    pub fn single_square(self) -> Option<Square> {
        // Unsafe cast code relies on the fact that Square has complete mapping in range 0..=63
        match self.0.count_ones() {
            1 => Some(unsafe { std::mem::transmute::<u8, Square>(self.0.trailing_zeros() as u8) }),
            _ => None
        }
    }

    // For the current bitboard, compress the bits covered in mask to the least significant bits,
    // and then return as a usize
    pub fn compressed_index(self, mask: u64) -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("bmi2") {
                return unsafe { std::arch::x86_64::_pext_u64(self.0, mask) as usize };
            }
        }
        Bitboard::compressed_index_fallback(self.0, mask)
    }

    // Portable software implementation of pext, used when BMI2 is unavailable.
    // Note: written by AI as I really don't care about not my computer yet
    fn compressed_index_fallback(bits: u64, mut mask: u64) -> usize {
        let mut result = 0u64;
        let mut next_bit = 1u64;
        while mask != 0 {
            let lsb = mask & mask.wrapping_neg();
            if bits & lsb != 0 {
                result |= next_bit;
            }
            next_bit <<= 1;
            mask &= mask - 1;
        }
        result as usize
    }

    pub fn iter_squares(self) -> SquaresIter {
        SquaresIter(self.0)
    }

    pub fn print(self, char: char) {
        for rank in Rank::iter() {
            for file in File::iter().rev() {
                let to_shift = 63u8 - ((rank as u8) * 8) - (file as u8);
                let mask = 1u64 << (to_shift as u64);
                let char = if self & Bitboard::from(mask) != Bitboard::EMPTY { char } else { '0' };
                print!("{char} ");
            }
            print!("\n");
        }
    }
}

pub struct SquaresIter(u64);

impl Iterator for SquaresIter {
    type Item = Square;

    fn next(&mut self) -> Option<Square> {
        if self.0 == 0 {
            return None;
        }
        // Unsafe cast code relies on the fact that Square has complete mapping in range 0..=63
        let sq = unsafe { std::mem::transmute::<u8, Square>(self.0.trailing_zeros() as u8) };
        // Clear the lowest set bit
        self.0 &= self.0 - 1;
        Some(sq)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.0.count_ones() as usize;
        (n, Some(n))
    }
}

impl ExactSizeIterator for SquaresIter {}

impl std::ops::BitAnd for Bitboard {
    type Output = Bitboard;
    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 & rhs.0)
    }
}

impl std::ops::BitOr for Bitboard {
    type Output = Bitboard;
    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 | rhs.0)
    }
}

impl std::ops::BitXor for Bitboard {
    type Output = Bitboard;
    fn bitxor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 ^ rhs.0)
    }
}

impl std::ops::Not for Bitboard {
    type Output = Bitboard;
    fn not(self) -> Bitboard {
        Bitboard(!self.0)
    }
}

impl std::ops::BitAndAssign for Bitboard {
    fn bitand_assign(&mut self, rhs: Bitboard) {
        self.0 &= rhs.0;
    }
}

impl std::ops::BitOrAssign for Bitboard {
    fn bitor_assign(&mut self, rhs: Bitboard) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitXorAssign for Bitboard {
    fn bitxor_assign(&mut self, rhs: Bitboard) {
        self.0 ^= rhs.0;
    }
}

impl std::ops::Shl<u32> for Bitboard {
    type Output = Bitboard;
    fn shl(self, rhs: u32) -> Bitboard {
        Bitboard(self.0 << rhs)
    }
}

impl std::ops::Shr<u32> for Bitboard {
    type Output = Bitboard;
    fn shr(self, rhs: u32) -> Bitboard {
        Bitboard(self.0 >> rhs)
    }
}

impl std::ops::ShlAssign<u32> for Bitboard {
    fn shl_assign(&mut self, rhs: u32) {
        self.0 <<= rhs;
    }
}

impl std::ops::ShrAssign<u32> for Bitboard {
    fn shr_assign(&mut self, rhs: u32) {
        self.0 >>= rhs;
    }
}

// Some useful bitboards
pub const BB_RANKS: [Bitboard; 8] = {
    let mut res = [Bitboard::from(0); 8];
    let mut n = 0;
    while n < 8 {
        res[n] = Bitboard::from(0b0000000000000000000000000000000000000000000000000000000011111111 << n*8);
        n += 1;
    }
    res
};

pub const BB_FILES: [Bitboard; 8] = {
    let mut res = [Bitboard::from(0); 8];
    let mut n = 0;
    while n < 8 {
        res[n] = Bitboard::from(0b0000000100000001000000010000000100000001000000010000000100000001 << n);
        n += 1;
    }
    res
};


// Side and piece containers

// Generic Enum Container
// Potentialy replace PerPiece, PerSide etc with aliases to this
pub trait EnumKey: Copy + strum::IntoEnumIterator {
    const COUNT: usize;
    type Array<T>: AsRef<[T]> + AsMut<[T]>;

    fn index(self) -> usize;
    fn from_index(i: usize) -> Self;
    fn array_from_fn<T>(f: impl FnMut(Self) -> T) -> Self::Array<T>;
}

#[derive(Debug)]
pub struct EnumMap<K: EnumKey, T>(K::Array<T>);

impl<K: EnumKey, T> EnumMap<K, T> {
    pub const fn from_array(values: K::Array<T>) -> Self {
        Self(values)
    }

    pub fn from_fn(f: impl FnMut(K) -> T) -> Self {
        Self(K::array_from_fn(f))
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, &T)> {
        K::iter().zip(self.0.as_ref().iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut T)> {
        K::iter().zip(self.0.as_mut().iter_mut())
    }
}

impl<K: EnumKey, T: Copy> EnumMap<K, T> {
    pub fn new(default_value: T) -> Self {
        Self::from_fn(|_| default_value)
    }
}

impl<K: EnumKey, T> std::ops::Index<K> for EnumMap<K, T> {
    type Output = T;
    fn index(&self, index: K) -> &T {
        &self.0.as_ref()[index.index()]
    }
}

impl<K: EnumKey, T> std::ops::IndexMut<K> for EnumMap<K, T> {
    fn index_mut(&mut self, index: K) -> &mut T {
        &mut self.0.as_mut()[index.index()]
    }
}

impl EnumKey for IndexedPieceKind {
    const COUNT: usize = <IndexedPieceKind as EnumCount>::COUNT;
    type Array<T> = [T; <IndexedPieceKind as EnumCount>::COUNT];

    fn index(self) -> usize {
        self as usize
    }

    fn from_index(i: usize) -> Self {
        IndexedPieceKind::iter().nth(i).expect("index out of bounds for IndexedPieces")
    }

    fn array_from_fn<T>(mut f: impl FnMut(Self) -> T) -> Self::Array<T> {
        std::array::from_fn(|i| f(Self::from_index(i)))
    }
}

impl EnumKey for SlidingPieceKind {
    const COUNT: usize = <SlidingPieceKind as EnumCount>::COUNT;
    type Array<T> = [T; <SlidingPieceKind as EnumCount>::COUNT];

    fn index(self) -> usize {
        self as usize
    }

    fn from_index(i: usize) -> Self {
        SlidingPieceKind::iter().nth(i).expect("index out of bounds for SlidingPieces")
    }

    fn array_from_fn<T>(mut f: impl FnMut(Self) -> T) -> Self::Array<T> {
        std::array::from_fn(|i| f(Self::from_index(i)))
    }
}

// PerPiece
#[derive(Debug, Clone, Copy)]
pub struct PerPiece<T>([T; PieceKind::COUNT]);

impl<T> PerPiece<T> {
    pub const fn from_array(values: [T; PieceKind::COUNT]) -> Self {
        Self(values)
    }
}

impl<T> PerPiece<T>
where
    T: Copy
{
    pub fn new(default_value: T) -> Self {
        Self([default_value; PieceKind::COUNT])
    }

    pub fn iter(&self) -> impl Iterator<Item = (PieceKind, &T)> {
        PieceKind::iter().zip(self.0.iter())
    }

    pub fn iter_mut(&mut self) -> std::iter::Zip<PieceKindIter, std::slice::IterMut<'_, T>> {
        PieceKind::iter().zip(self.0.iter_mut())
    }
}

impl<T> std::ops::Index<PieceKind> for PerPiece<T> {
    type Output = T;
    fn index(&self, index: PieceKind) -> &T {
        &self.0[index as usize]
    }
}

impl<T> std::ops::IndexMut<PieceKind> for PerPiece<T> {
    fn index_mut(&mut self, index: PieceKind) -> &mut T {
        &mut self.0[index as usize]
    }
}

// PerSide
#[derive(Debug, Clone, Copy)]
pub struct PerSide<T>([T; Side::COUNT]);

impl<T> PerSide<T> {
    pub const fn from_array(values: [T; Side::COUNT]) -> Self {
        Self(values)
    }
}

impl<T> PerSide<T>
where
    T: Copy
{
    pub fn new(default_value: T) -> Self {
        Self([default_value; Side::COUNT])
    }

    pub fn iter(&self) -> impl Iterator<Item = (Side, &T)> {
        Side::iter().zip(self.0.iter())
    }

    pub fn iter_mut(&mut self) -> std::iter::Zip<SideIter, std::slice::IterMut<'_, T>> {
        Side::iter().zip(self.0.iter_mut())
    }
}

impl<T> std::ops::Index<Side> for PerSide<T> {
    type Output = T;
    fn index(&self, index: Side) -> &T {
        &self.0[index as usize]
    }
}

impl<T> std::ops::IndexMut<Side> for PerSide<T> {
    fn index_mut(&mut self, index: Side) -> &mut T {
        &mut self.0[index as usize]
    }
}

// PerSquare
#[derive(Debug, Clone, Copy)]
pub struct PerSquare<T>([T; Square::COUNT]);

impl<T> PerSquare<T>
where
    T: Clone
{
    pub fn new(default_value: T) -> Self {
        Self(std::array::from_fn(|_| default_value.clone()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (Square, &T)> {
        Square::iter().zip(self.0.iter())
    }

    pub fn iter_mut(&mut self) -> std::iter::Zip<SquareIter, std::slice::IterMut<'_, T>> {
        Square::iter().zip(self.0.iter_mut())
    }
}

impl<T> std::ops::Index<Square> for PerSquare<T> {
    type Output = T;
    fn index(&self, index: Square) -> &T {
        &self.0[index as usize]
    }
}

impl<T> std::ops::IndexMut<Square> for PerSquare<T> {
    fn index_mut(&mut self, index: Square) -> &mut T {
        &mut self.0[index as usize]
    }
}