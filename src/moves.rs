//! Move representation.
//!
//! A [`Move`] keeps the minimal, color-agnostic information needed to apply it:
//! origin square, destination square, and an optional promotion piece. Special
//! move semantics (captures, double pawn pushes, en passant, castling) are
//! *derived* from the board state at apply time in [`crate::board`], which keeps
//! this type small and makes it trivial for the UI to construct moves.

use crate::types::{square, Piece, Sq};
use std::fmt;

/// A chess move from one square to another, with optional promotion.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Move {
    pub from: Sq,
    pub to: Sq,
    /// The piece a pawn promotes to (always a queen/rook/bishop/knight of the
    /// moving side), or `None` for non-promotions.
    pub promotion: Option<Piece>,
}

impl Move {
    /// A quiet (non-promoting) move.
    #[inline]
    pub const fn new(from: Sq, to: Sq) -> Move {
        Move {
            from,
            to,
            promotion: None,
        }
    }

    /// A promotion move.
    #[inline]
    pub const fn promo(from: Sq, to: Sq, piece: Piece) -> Move {
        Move {
            from,
            to,
            promotion: Some(piece),
        }
    }

    /// `true` if this move promotes a pawn.
    #[inline]
    pub const fn is_promotion(&self) -> bool {
        self.promotion.is_some()
    }

    /// Render in long algebraic / UCI form, e.g. `"e2e4"` or `"e7e8q"`.
    pub fn to_uci(&self) -> String {
        let mut s = format!("{}{}", square::to_alg(self.from), square::to_alg(self.to));
        if let Some(p) = self.promotion {
            s.push(p.to_char().to_ascii_lowercase());
        }
        s
    }

    /// Parse a UCI move string such as `"e2e4"` or `"e7e8q"`.
    ///
    /// The promotion color is normalized to white here; callers that make the
    /// move will receive a correctly-colored promotion piece because the board
    /// re-colors promotions to the side to move. Returns `None` on malformed
    /// input rather than panicking.
    pub fn from_uci(s: &str) -> Option<Move> {
        let s = s.trim();
        if s.len() < 4 {
            return None;
        }
        let from = square::from_alg(&s[0..2])?;
        let to = square::from_alg(&s[2..4])?;
        let promotion = if s.len() >= 5 {
            let c = s.as_bytes()[4].to_ascii_lowercase() as char;
            match c {
                'q' | 'r' | 'b' | 'n' => Some(Piece::from_char(c.to_ascii_uppercase())),
                _ => return None,
            }
        } else {
            None
        };
        Some(Move {
            from,
            to,
            promotion,
        })
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_uci())
    }
}

/// A reusable buffer of moves. Move generation appends into one of these.
pub type MoveList = Vec<Move>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::square;

    #[test]
    fn uci_roundtrip() {
        let m = Move::new(square::from_alg("e2").unwrap(), square::from_alg("e4").unwrap());
        assert_eq!(m.to_uci(), "e2e4");
        assert_eq!(Move::from_uci("e2e4"), Some(m));
    }

    #[test]
    fn uci_promotion() {
        let m = Move::from_uci("e7e8q").unwrap();
        assert_eq!(m.from, square::from_alg("e7").unwrap());
        assert_eq!(m.to, square::from_alg("e8").unwrap());
        assert_eq!(m.promotion, Some(Piece::WQ));
        assert_eq!(m.to_uci(), "e7e8q");
    }

    #[test]
    fn uci_rejects_garbage() {
        assert_eq!(Move::from_uci("zz99"), None);
        assert_eq!(Move::from_uci("e2"), None);
        assert_eq!(Move::from_uci("e2e4x"), None);
    }
}
