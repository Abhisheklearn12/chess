//! Attack detection on the `0x88` board.
//!
//! These routines answer "is square `s` attacked by `color`?" which is the
//! primitive underlying check detection, legal-move filtering, and castling
//! legality. All arithmetic is done on signed `i32` indices with the
//! [`square::on_board_i32`] guard so nothing ever wraps or panics.

use crate::board::Board;
use crate::types::{square, Color, Piece, PieceKind, Sq};

/// `0x88` step deltas for a knight (two-then-one in every direction).
pub const KNIGHT_DELTAS: [i32; 8] = [33, 31, 18, 14, -33, -31, -18, -14];
/// `0x88` step deltas for a king (and the eight directions a queen radiates).
pub const KING_DELTAS: [i32; 8] = [16, -16, 1, -1, 17, 15, -17, -15];
/// Orthogonal slider deltas (rook, queen).
pub const ROOK_DELTAS: [i32; 4] = [16, -16, 1, -1];
/// Diagonal slider deltas (bishop, queen).
pub const BISHOP_DELTAS: [i32; 4] = [17, 15, -17, -15];

/// `true` if `color` attacks square `s` in the given position.
///
/// "Attacks" means a piece of `color` could capture onto `s` if an enemy piece
/// were there, the standard definition, ignoring pins and check.
pub fn is_square_attacked(board: &Board, s: Sq, color: Color) -> bool {
    let si = s as i32;

    // --- Pawns --- a pawn of `color` attacks "forward" diagonally, so we look
    // "backward" from `s` for an attacking pawn.
    let (pawn, back_left, back_right) = match color {
        Color::White => (Piece::WP, si - 17, si - 15),
        Color::Black => (Piece::BP, si + 17, si + 15),
    };
    for cand in [back_left, back_right] {
        if square::on_board_i32(cand) && board.cells[cand as usize] == pawn {
            return true;
        }
    }

    // --- Knights ---
    let knight = Piece::make(color, PieceKind::Knight);
    for d in KNIGHT_DELTAS {
        let cand = si + d;
        if square::on_board_i32(cand) && board.cells[cand as usize] == knight {
            return true;
        }
    }

    // --- King ---
    let king = Piece::make(color, PieceKind::King);
    for d in KING_DELTAS {
        let cand = si + d;
        if square::on_board_i32(cand) && board.cells[cand as usize] == king {
            return true;
        }
    }

    // --- Orthogonal sliders (rook / queen) ---
    let rook = Piece::make(color, PieceKind::Rook);
    let queen = Piece::make(color, PieceKind::Queen);
    for d in ROOK_DELTAS {
        let mut cand = si + d;
        while square::on_board_i32(cand) {
            let p = board.cells[cand as usize];
            if !p.is_empty() {
                if p == rook || p == queen {
                    return true;
                }
                break;
            }
            cand += d;
        }
    }

    // --- Diagonal sliders (bishop / queen) ---
    let bishop = Piece::make(color, PieceKind::Bishop);
    for d in BISHOP_DELTAS {
        let mut cand = si + d;
        while square::on_board_i32(cand) {
            let p = board.cells[cand as usize];
            if !p.is_empty() {
                if p == bishop || p == queen {
                    return true;
                }
                break;
            }
            cand += d;
        }
    }

    false
}

/// `true` if the king of `color` is currently in check.
pub fn in_check(board: &Board, color: Color) -> bool {
    match board.king_square(color) {
        Some(k) => is_square_attacked(board, k, color.flip()),
        // A missing king counts as "in check" so illegal lines are pruned.
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::square;

    #[test]
    fn startpos_no_checks() {
        let b = Board::startpos();
        assert!(!in_check(&b, Color::White));
        assert!(!in_check(&b, Color::Black));
    }

    #[test]
    fn rook_attacks_along_rank() {
        let b = Board::from_fen("8/8/8/8/8/8/8/R3k2K w - - 0 1").unwrap();
        let e1 = square::from_alg("e1").unwrap();
        assert!(is_square_attacked(&b, e1, Color::White));
    }

    #[test]
    fn detects_check() {
        // Black rook on e8 checks the white king on e1.
        let b = Board::from_fen("4r3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert!(in_check(&b, Color::White));
    }

    #[test]
    fn pawn_attack_no_wrap() {
        // White pawn on a-file must not be seen as attacking the h-file.
        let b = Board::from_fen("8/8/8/8/8/8/P7/k6K w - - 0 1").unwrap();
        let h3 = square::from_alg("h3").unwrap();
        assert!(!is_square_attacked(&b, h3, Color::White));
        let b2 = square::from_alg("b3").unwrap();
        assert!(is_square_attacked(&b, b2, Color::White));
    }
}
