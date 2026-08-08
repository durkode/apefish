//! Core value types shared across the engine (side, piece type, moves).

use strum::EnumCount;
// use std::default; // unused for now

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumCount)]
#[repr(u8)]
pub enum Side {
    White,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumCount)]
#[repr(u8)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub side: Side,
    pub kind: PieceType,
}

/// A single square on the board (a1 = 0 ... h8 = 63). Encoding TBD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumCount)]
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



#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumCount)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumCount)]
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

/// A single move: source/destination square plus optional promotion piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub promotion: Option<PieceType>,
}


// Bitboards. This is a 'NewType' over a u64, and as such AI has been used to allow 
// bitwise operations on bitboards as if they were a u64.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const FULL: Bitboard = Bitboard(u64::MAX);

    pub const fn new(bits: u64) -> Self {
        Bitboard(bits)
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
#[derive(Debug, Clone)]
pub struct PerPiece<T>([T; PieceType::COUNT]);

impl<T> PerPiece<T>
where
    T: Copy
{
    pub fn new(default_value: T) -> Self {
        Self([default_value; PieceType::COUNT])
    }
}

impl<T> std::ops::Index<PieceType> for PerPiece<T> {
    type Output = T;
    fn index(&self, index: PieceType) -> &T {
        &self.0[index as usize]
    }
}

impl<T> std::ops::IndexMut<PieceType> for PerPiece<T> {
    fn index_mut(&mut self, index: PieceType) -> &mut T {
        &mut self.0[index as usize]
    }
}

// PerSide
#[derive(Debug, Clone)]
pub struct PerSide<T>([T; Side::COUNT]);

impl<T> PerSide<T>
where
    T: Copy
{
    pub fn new(default_value: T) -> Self {
        Self([default_value; Side::COUNT])
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
#[derive(Debug, Clone)]
pub struct PerSquare<T>([T; Square::COUNT]);

impl<T> PerSquare<T>
where
    T: Copy
{
    pub fn new(default_value: T) -> Self {
        Self([default_value; Square::COUNT])
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