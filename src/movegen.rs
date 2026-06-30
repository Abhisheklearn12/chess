//! Move generation.
//!
//! Generation is two-staged for clarity and correctness:
//!
//! 1. [`gen_pseudo`] enumerates *pseudo-legal* moves, legal except that they
//!    may leave the moving side's own king in check.
//! 2. [`legal_moves`] / [`gen_moves`] filter those by actually making the move
//!    and testing king safety, which is the simplest provably-correct approach
//!    and is validated exhaustively by the perft suite.
//!
//! Castling is special-cased in [`gen_pseudo`] with full legality (rights, an
//! empty path, the rook present, and the king not moving out of, through, or
//! into check) because the squares the king *passes over* are not covered by
//! the final-position king-safety test.

use crate::attacks::{
    is_square_attacked, BISHOP_DELTAS, KING_DELTAS, KNIGHT_DELTAS, ROOK_DELTAS,
};
use crate::board::Board;
use crate::moves::{Move, MoveList};
use crate::types::{
    square, Color, Piece, PieceKind, Sq, CASTLE_BK, CASTLE_BQ, CASTLE_WK, CASTLE_WQ,
};

/// Generate all pseudo-legal moves for the side to move into `out` (cleared
/// first). Pseudo-legal moves may leave the king in check; use [`legal_moves`]
/// for a filtered list.
pub fn gen_pseudo(board: &Board, out: &mut MoveList) {
    out.clear();
    let color = board.side_to_move();
    for r in 0..8 {
        for f in 0..8 {
            let s = square::make(r, f);
            let p = board.cells[s];
            if !p.is_color(color) {
                continue;
            }
            match p.kind().unwrap() {
                PieceKind::Pawn => gen_pawn(board, s, color, out),
                PieceKind::Knight => gen_leaper(board, s, color, &KNIGHT_DELTAS, out),
                PieceKind::King => gen_leaper(board, s, color, &KING_DELTAS, out),
                PieceKind::Bishop => gen_slider(board, s, color, &BISHOP_DELTAS, out),
                PieceKind::Rook => gen_slider(board, s, color, &ROOK_DELTAS, out),
                PieceKind::Queen => {
                    gen_slider(board, s, color, &ROOK_DELTAS, out);
                    gen_slider(board, s, color, &BISHOP_DELTAS, out);
                }
            }
        }
    }
    gen_castling(board, color, out);
}

fn gen_leaper(board: &Board, s: Sq, color: Color, deltas: &[i32], out: &mut MoveList) {
    let si = s as i32;
    for &d in deltas {
        let cand = si + d;
        if !square::on_board_i32(cand) {
            continue;
        }
        let target = board.cells[cand as usize];
        if target.is_empty() || target.is_color(color.flip()) {
            out.push(Move::new(s, cand as usize));
        }
    }
}

fn gen_slider(board: &Board, s: Sq, color: Color, deltas: &[i32], out: &mut MoveList) {
    let si = s as i32;
    for &d in deltas {
        let mut cand = si + d;
        while square::on_board_i32(cand) {
            let target = board.cells[cand as usize];
            if target.is_empty() {
                out.push(Move::new(s, cand as usize));
            } else {
                if target.is_color(color.flip()) {
                    out.push(Move::new(s, cand as usize));
                }
                break;
            }
            cand += d;
        }
    }
}

fn gen_pawn(board: &Board, s: Sq, color: Color, out: &mut MoveList) {
    let si = s as i32;
    let (dir, start_rank, promo_rank) = match color {
        Color::White => (16, 1, 7),
        Color::Black => (-16, 6, 0),
    };
    let rank = square::rank(s);

    // --- Single and double pushes ---
    let one = si + dir;
    if square::on_board_i32(one) && board.cells[one as usize].is_empty() {
        let one_sq = one as usize;
        if square::rank(one_sq) == promo_rank {
            push_promotions(s, one_sq, color, out);
        } else {
            out.push(Move::new(s, one_sq));
            // Double push only from the starting rank, over an empty square.
            if rank == start_rank {
                let two = si + 2 * dir;
                if square::on_board_i32(two) && board.cells[two as usize].is_empty() {
                    out.push(Move::new(s, two as usize));
                }
            }
        }
    }

    // --- Captures (incl. promotion captures) and en passant ---
    for cap in [dir + 1, dir - 1] {
        let cand = si + cap;
        if !square::on_board_i32(cand) {
            continue;
        }
        let cand_sq = cand as usize;
        let target = board.cells[cand_sq];
        if target.is_color(color.flip()) {
            if square::rank(cand_sq) == promo_rank {
                push_promotions(s, cand_sq, color, out);
            } else {
                out.push(Move::new(s, cand_sq));
            }
        } else if Some(cand_sq) == board.ep {
            // En passant: the destination square is the ep target.
            out.push(Move::new(s, cand_sq));
        }
    }
}

fn push_promotions(from: Sq, to: Sq, color: Color, out: &mut MoveList) {
    for kind in [
        PieceKind::Queen,
        PieceKind::Rook,
        PieceKind::Bishop,
        PieceKind::Knight,
    ] {
        out.push(Move::promo(from, to, Piece::make(color, kind)));
    }
}

fn gen_castling(board: &Board, color: Color, out: &mut MoveList) {
    let (rank, kingside_right, queenside_right) = match color {
        Color::White => (0, CASTLE_WK, CASTLE_WQ),
        Color::Black => (7, CASTLE_BK, CASTLE_BQ),
    };
    let enemy = color.flip();
    let king_sq = square::make(rank, 4);

    // The king must actually be home and not currently in check.
    if board.cells[king_sq] != Piece::make(color, PieceKind::King) {
        return;
    }
    if is_square_attacked(board, king_sq, enemy) {
        return;
    }
    let rook = Piece::make(color, PieceKind::Rook);

    // Kingside: squares f and g empty, rook on h, neither f nor g attacked.
    if board.castling & kingside_right != 0 {
        let f1 = square::make(rank, 5);
        let g1 = square::make(rank, 6);
        let h1 = square::make(rank, 7);
        if board.cells[f1].is_empty()
            && board.cells[g1].is_empty()
            && board.cells[h1] == rook
            && !is_square_attacked(board, f1, enemy)
            && !is_square_attacked(board, g1, enemy)
        {
            out.push(Move::new(king_sq, g1));
        }
    }

    // Queenside: squares b, c, d empty, rook on a, c and d not attacked.
    if board.castling & queenside_right != 0 {
        let b1 = square::make(rank, 1);
        let c1 = square::make(rank, 2);
        let d1 = square::make(rank, 3);
        let a1 = square::make(rank, 0);
        if board.cells[b1].is_empty()
            && board.cells[c1].is_empty()
            && board.cells[d1].is_empty()
            && board.cells[a1] == rook
            && !is_square_attacked(board, c1, enemy)
            && !is_square_attacked(board, d1, enemy)
        {
            out.push(Move::new(king_sq, c1));
        }
    }
}

/// Filter pseudo-legal moves to fully legal ones by making each move and
/// verifying the moving side's king is not left in check. Operates in place on
/// `board` via make/unmake (no allocation of board copies).
pub fn legal_moves(board: &mut Board, out: &mut MoveList) {
    let color = board.side_to_move();
    let mut pseudo = Vec::with_capacity(48);
    gen_pseudo(board, &mut pseudo);
    out.clear();
    for mv in pseudo {
        board.make_move_struct(mv);
        // After making, `color` is the side that just moved.
        if !crate::attacks::in_check(board, color) {
            out.push(mv);
        }
        board.unmake_move();
    }
}

/// Generate fully legal moves without requiring a mutable board (clones once
/// internally). Kept for the UI's `&Board` call sites.
pub fn gen_moves(board: &Board, out: &mut MoveList) {
    let mut scratch = board.clone();
    legal_moves(&mut scratch, out);
}

/// Generate only "loud" moves (captures, en passant, and promotions), used by
/// quiescence search. Returns fully legal moves.
pub fn gen_captures(board: &mut Board, out: &mut MoveList) {
    let color = board.side_to_move();
    let mut pseudo = Vec::with_capacity(32);
    gen_pseudo(board, &mut pseudo);
    out.clear();
    for mv in pseudo {
        let target = board.cells[mv.to];
        let is_ep = matches!(board.cells[mv.from].kind(), Some(PieceKind::Pawn))
            && Some(mv.to) == board.ep
            && square::file(mv.from) != square::file(mv.to);
        let loud = !target.is_empty() || mv.is_promotion() || is_ep;
        if !loud {
            continue;
        }
        board.make_move_struct(mv);
        if !crate::attacks::in_check(board, color) {
            out.push(mv);
        }
        board.unmake_move();
    }
}

/// Compatibility wrapper used by the UI: is the king of the given side in
/// check? `white_king == true` queries the white king.
pub fn is_king_attacked(board: &Board, white_king: bool) -> bool {
    let color = if white_king {
        Color::White
    } else {
        Color::Black
    };
    crate::attacks::in_check(board, color)
}

/// `true` if the side to move has at least one legal move.
pub fn has_legal_move(board: &mut Board) -> bool {
    let mut moves = Vec::with_capacity(48);
    legal_moves(board, &mut moves);
    !moves.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_legal(fen: &str) -> usize {
        let mut b = Board::from_fen(fen).unwrap();
        let mut moves = Vec::new();
        legal_moves(&mut b, &mut moves);
        moves.len()
    }

    #[test]
    fn startpos_has_twenty_moves() {
        assert_eq!(count_legal(crate::board::START_FEN), 20);
    }

    #[test]
    fn castling_blocked_through_check() {
        // Black rook on e8 controls e-file: white may not castle either side
        // because the king starts in check (and e1 is attacked).
        let fen = "4r3/8/8/8/8/8/8/R3K2R w KQ - 0 1";
        let mut b = Board::from_fen(fen).unwrap();
        let mut moves = Vec::new();
        legal_moves(&mut b, &mut moves);
        assert!(!moves.iter().any(|m| m.to == square::from_alg("g1").unwrap()));
        assert!(!moves.iter().any(|m| m.to == square::from_alg("c1").unwrap()));
    }

    #[test]
    fn castling_allowed_when_clear() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let mut b = Board::from_fen(fen).unwrap();
        let mut moves = Vec::new();
        legal_moves(&mut b, &mut moves);
        assert!(moves.iter().any(|m| m.to == square::from_alg("g1").unwrap()));
        assert!(moves.iter().any(|m| m.to == square::from_alg("c1").unwrap()));
    }

    #[test]
    fn pinned_piece_cannot_move() {
        // White knight on e2 is pinned by the black rook on e8 to the king e1.
        let fen = "4r3/8/8/8/8/8/4N3/4K3 w - - 0 1";
        let mut b = Board::from_fen(fen).unwrap();
        let mut moves = Vec::new();
        legal_moves(&mut b, &mut moves);
        assert!(!moves.iter().any(|m| m.from == square::from_alg("e2").unwrap()));
    }

    #[test]
    fn checkmate_has_no_moves() {
        // Fool's mate position: white is checkmated.
        let fen = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
        assert_eq!(count_legal(fen), 0);
    }
}
