use strum::EnumCount;

use crate::basetypes::{CastlingRights, PerSquare, Piece, PieceKind, Rank, Side, Square};

// A lot of the fen related code isn't the most efficient. That said, Creating from FEN isn't a contested path
// so will refactor it later if there is a need for performance.

/// Error parsing a FEN string. Detail fields TBD.
#[derive(Debug)]
pub enum FenError {
    IncorrectPartLength,
    IncorrectNumRanks,
    InvalidPiecePlacement,
    InvalidColour,
    InvalidEPSquare,
    InvalidHalfMoveClock,
    InvalidFullMoveNumber,
    InvalidCastlingRights,
}

pub const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub struct FenParts {
    pub pieces: PerSquare<Option<Piece>>,
    pub active_colour: Side,
    pub castling_rights: CastlingRights,
    pub en_passant_square: Option<Square>,
    pub half_move_clock: u8,
    pub full_move_number: u16
}

pub fn parse_fen(fen: &str) -> Result<FenParts, FenError>  {
    let fen_parts: Vec<_> = fen.split(" ").collect();
    if fen_parts.len() != 6 {
        return Err(FenError::IncorrectPartLength)
    }

    println!("Fen parts {fen_parts:?}");

    let result = FenParts{
        pieces: fen_parse_pieces(fen_parts[0])?,
        active_colour: fen_parse_colour(fen_parts[1])?,
        castling_rights: fen_parse_castling_rights(fen_parts[2])?,
        en_passant_square: fen_parse_en_passant_square(fen_parts[3])?,
        half_move_clock: fen_parse_half_move_clock(fen_parts[4])?,
        full_move_number: fen_parse_full_move_number(fen_parts[5])?
    };

    Ok(result)

}

fn fen_parse_pieces(piece_string: &str) -> Result<PerSquare<Option<Piece>>, FenError> {
    let rank_strings: Vec<_> = piece_string.split('/').rev().collect();
    if rank_strings.len() != Rank::COUNT {
        return Err(FenError::IncorrectNumRanks);
    }

    let mut pieces = PerSquare::new(None);
    let mut square_iter = pieces.iter_mut();

    for rstring in rank_strings.iter() {
        for c in rstring.chars() {
            let square_piece = match c {
                '0'..='9' => None,
                'P' => Some(Piece{kind: PieceKind::Pawn, side: Side::White}),
                'N' => Some(Piece{kind: PieceKind::Knight, side: Side::White}),
                'B' => Some(Piece{kind: PieceKind::Bishop, side: Side::White}),
                'R' => Some(Piece{kind: PieceKind::Rook, side: Side::White}),
                'Q' => Some(Piece{kind: PieceKind::Queen, side: Side::White}),
                'K' => Some(Piece{kind: PieceKind::King, side: Side::White}),
                'p' => Some(Piece{kind: PieceKind::Pawn, side: Side::Black}),
                'n' => Some(Piece{kind: PieceKind::Knight, side: Side::Black}),
                'b' => Some(Piece{kind: PieceKind::Bishop, side: Side::Black}),
                'r' => Some(Piece{kind: PieceKind::Rook, side: Side::Black}),
                'q' => Some(Piece{kind: PieceKind::Queen, side: Side::Black}),
                'k' => Some(Piece{kind: PieceKind::King, side: Side::Black}),
                _ => return Err(FenError::InvalidPiecePlacement)
            };
            let num_squares = if ('0'..='9').contains(&c) {c.to_digit(10).unwrap() as u8} else {1};
            for _ in 0..num_squares {
                if let Some((_, dest)) = square_iter.next() {
                    *dest = square_piece;
                } else {
                    return Err(FenError::InvalidPiecePlacement);
                }
            }
        }
    }

    Ok(pieces)
}

fn fen_parse_colour(colour_string: &str) -> Result<Side, FenError> {
    match colour_string {
        "w" => Ok(Side::White),
        "b" => Ok(Side::Black),
        _ => Err(FenError::InvalidColour)
    }
}

fn fen_parse_castling_rights(castling_string: &str) -> Result<CastlingRights, FenError> {
    let mut castling_rights = CastlingRights::new(CastlingRights::NONE);
    if castling_string.is_empty() {
        return Err(FenError::InvalidCastlingRights);
    }
    for char in castling_string.chars() {
        match char {
            'K' => castling_rights.set_rights(CastlingRights::WK),
            'Q' => castling_rights.set_rights(CastlingRights::WQ),
            'k' => castling_rights.set_rights(CastlingRights::BK),
            'q' => castling_rights.set_rights(CastlingRights::BQ),
            '-' if castling_string.len() == 1 => break,
            _ => return Err(FenError::InvalidCastlingRights)
        }
    }
    Ok(castling_rights)
}

fn fen_parse_en_passant_square(ep_string: &str) -> Result<Option<Square>, FenError> {
    match ep_string {
        "-" => Ok(None),
        x => {
            match Square::from_string(x) {
                Ok(y) => Ok(Some(y)),
                Err(_) => Err(FenError::InvalidEPSquare)
            }
        }
    }
}

fn fen_parse_half_move_clock(half_move_string: &str) -> Result<u8, FenError> {
    match half_move_string.parse::<u8>() {
        Ok(x) => Ok(x),
        _ => Err(FenError::InvalidHalfMoveClock)
    }
}

fn fen_parse_full_move_number(full_move_string: &str) -> Result<u16, FenError> {
    match full_move_string.parse::<u16>() {
        Ok(x) => Ok(x),
        _ => Err(FenError::InvalidFullMoveNumber)
    }
}