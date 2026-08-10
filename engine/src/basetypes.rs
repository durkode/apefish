//! Core value types shared across the engine (side, piece type, moves).

use strum::{EnumCount, IntoEnumIterator};

use std::str::FromStr;

#[derive(Debug)]
pub enum GenericErr {
    SquareParseError
}

#[derive(Debug, strum::Display, Clone, Copy, PartialEq, Eq, strum::EnumCount, strum::EnumIter)]
#[repr(u8)]
pub enum Side {
    White = 0,
    Black,
    None,
}

#[derive(Debug, strum::Display, Clone, Copy, PartialEq, Eq, strum::EnumCount, strum::EnumIter)]
#[repr(u8)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
    None, //(Unsure if we want this)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub side: Side,
    pub piece_kind: PieceKind,
}

impl Piece {

    pub const fn to_unicode_char(self) -> char {
        match (self.side, self.piece_kind) {
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
            (_, _) => '□',
        }
    }
}

/// A single square on the board (a1 = 0 ... h8 = 63). Encoding TBD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumCount, strum::EnumIter, strum::EnumString)]
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
    
    pub const fn bitboard_mask(self) -> Bitboard {
        Bitboard(1u64 << self as u8)
    }

    pub fn from_string(from: &str) -> Result<Self, GenericErr> {
        match Square::from_str(from) {
            Ok(x) => Ok(x),
            Err(_) => Err(GenericErr::SquareParseError)
        }
    }

    pub fn from_coords(file: File, rank: Rank) -> Self {
        // Unsafe code relies on File, Rank, Square remaining relationally static.
        unsafe { std::mem::transmute::<u8, Square>((file as u8) + (8 * rank as u8)) }
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Move {
    // Implement later
}

pub struct CastlingRights {
    rights: Bitboard
}

impl CastlingRights {
    pub const NONE: Bitboard = Bitboard(0);
    pub const WK: Bitboard = Bitboard(1);
    pub const WQ: Bitboard = Bitboard(2);
    pub const BK: Bitboard = Bitboard(4);
    pub const BQ: Bitboard = Bitboard(8);
    pub const ALL: Bitboard = Bitboard(15);

    pub fn new(rights: Bitboard) -> Self {
        CastlingRights { rights }
    }

    pub fn has_rights(self, direction: Bitboard) -> bool {
        (self.rights & direction) != Bitboard::EMPTY
    }

    pub fn set_rights(&mut self, direction: Bitboard) {
        self.rights |= direction;
    }

    pub fn remove_rights(&mut self, direction: Bitboard) {
        self.rights &= !direction;
    }
}

// Bitboards. This is a 'NewType' over a u64, and as such AI has been used to allow 
// bitwise operations on bitboards as if they were a u64.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const FULL: Bitboard = Bitboard(u64::MAX);

    pub const fn from(bits: u64) -> Self {
        Bitboard(bits)
    }

    // If this bitboard is a mask, return the corresponding square
    pub fn single_square(self) -> Option<Square> {
        // Unsafe cast code relies on the fact that Square has complete mapping in range 0..=63
        match self.0.count_ones() {
            1 => Some(unsafe { std::mem::transmute::<u8, Square>(self.0.trailing_zeros() as u8) }),
            _ => None
        }
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

// Side and piece containers

// PerPiece
#[derive(Debug, Clone, Copy)]
pub struct PerPiece<T>([T; PieceKind::COUNT]);

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
    T: Copy
{
    pub fn new(default_value: T) -> Self {
        Self([default_value; Square::COUNT])
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