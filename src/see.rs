//! Static Exchange Evaluation (SEE).
//!
//! SEE answers "if I make this capture and we then trade off optimally on the
//! destination square, what material do I net?" It lets the search order
//! captures better and prune obviously losing ones in quiescence, without
//! actually searching them.
//!
//! The implementation is the classic swap-off algorithm: repeatedly bring the
//! *least valuable attacker* of the target square to bear, alternating sides.
//! X-ray attackers (a rook or queen behind the piece that just captured) are
//! handled naturally because each step re-scans the current occupancy.
//!
//! This only affects *strength*, never legality; a wrong SEE can never make
//! the engine play an illegal move.

use crate::board::Board;
use crate::moves::Move;
use crate::types::{square, Color, PieceKind, Sq};

/// Material values used by SEE (centipawns). The king is effectively infinite.
const VALUES: [i32; 6] = [100, 320, 330, 500, 900, 10_000];

#[inline]
fn value(kind: PieceKind) -> i32 {
    VALUES[kind.index()]
}

/// Evaluate the static exchange on `mv.to` if `mv` is played. Returns the net
/// centipawn gain for the side making the move (positive = good capture).
///
/// `mv` should be a capture (or promotion-capture); for a quiet move the result
/// is simply 0 minus any retaliation, which callers generally do not use.
pub fn see(board: &Board, mv: Move) -> i32 {
    let to = mv.to;
    let mut occ = board.cells; // a working copy we vacate as pieces capture

    let mover = board.cells[mv.from];
    let Some(mut side) = mover.color() else {
        return 0;
    };

    // Initial victim value (handle en passant: the victim sits behind `to`).
    let victim_val = if board.cells[to].is_empty() {
        if mover.kind() == Some(PieceKind::Pawn) && square::file(mv.from) != square::file(to) {
            // En passant capture of a pawn.
            value(PieceKind::Pawn)
        } else {
            0
        }
    } else {
        board.cells[to].kind().map(value).unwrap_or(0)
    };

    // The attacker steps onto `to`; vacate its origin.
    occ[mv.from] = crate::types::Piece::Empty;
    // Clear the en-passant victim from the working occupancy too.
    if board.cells[to].is_empty() && mover.kind() == Some(PieceKind::Pawn) {
        let cap_sq = square::make(square::rank(mv.from), square::file(to));
        occ[cap_sq] = crate::types::Piece::Empty;
    }

    let mut gains = [0i32; 32];
    let mut depth = 0;
    gains[0] = victim_val;
    // Value of the piece now standing on `to` (it can be recaptured next).
    let mut piece_on_to = mover.kind().map(value).unwrap_or(0);
    side = side.flip();

    loop {
        depth += 1;
        // Speculative gain if the side to move captures on `to` now.
        gains[depth] = piece_on_to - gains[depth - 1];

        // Find the least valuable attacker of `to` for `side`.
        let Some((from_sq, kind)) = least_valuable_attacker(&occ, to, side) else {
            break;
        };

        piece_on_to = value(kind);
        occ[from_sq] = crate::types::Piece::Empty; // it moves onto `to`
        side = side.flip();

        // Cap recursion to the array size (impossible to exceed in practice).
        if depth + 1 >= gains.len() {
            break;
        }
    }

    // Negamax the gain array back to the root. This mirrors the canonical
    // `while(--d) gain[d-1] = -max(-gain[d-1], gain[d])`: the last, purely
    // speculative level (for which no real recapture existed) is discarded.
    while depth > 1 {
        depth -= 1;
        gains[depth - 1] = -((-gains[depth - 1]).max(gains[depth]));
    }
    gains[0]
}

/// `true` if the capture does not lose material (SEE >= 0).
pub fn see_ge_zero(board: &Board, mv: Move) -> bool {
    see(board, mv) >= 0
}

/// Find the least valuable piece of `color` that attacks `to` given the current
/// occupancy `occ`, returning its square and kind. Handles x-ray implicitly via
/// the caller's repeated invocation on updated occupancy.
fn least_valuable_attacker(
    occ: &[crate::types::Piece; 128],
    to: Sq,
    color: Color,
) -> Option<(Sq, PieceKind)> {
    use crate::attacks::{BISHOP_DELTAS, KING_DELTAS, KNIGHT_DELTAS, ROOK_DELTAS};
    use crate::types::Piece;
    let ti = to as i32;

    // 1. Pawns (cheapest).
    let pawn = Piece::make(color, PieceKind::Pawn);
    let pawn_srcs = match color {
        Color::White => [ti - 15, ti - 17],
        Color::Black => [ti + 15, ti + 17],
    };
    for s in pawn_srcs {
        if square::on_board_i32(s) && occ[s as usize] == pawn {
            return Some((s as usize, PieceKind::Pawn));
        }
    }

    // 2. Knights.
    let knight = Piece::make(color, PieceKind::Knight);
    for d in KNIGHT_DELTAS {
        let s = ti + d;
        if square::on_board_i32(s) && occ[s as usize] == knight {
            return Some((s as usize, PieceKind::Knight));
        }
    }

    // 3. Bishops then 4. rooks (scan the rays; pick first blocker if it fits).
    let bishop = Piece::make(color, PieceKind::Bishop);
    let queen = Piece::make(color, PieceKind::Queen);
    if let Some(sq) = first_slider_on_rays(occ, ti, &BISHOP_DELTAS, bishop, queen) {
        // Only return a bishop here; a revealed queen on a diagonal is returned
        // by the rook scan's queen check too, so prefer the cheaper bishop.
        if occ[sq] == bishop {
            return Some((sq, PieceKind::Bishop));
        }
    }
    let rook = Piece::make(color, PieceKind::Rook);
    if let Some(sq) = first_slider_on_rays(occ, ti, &ROOK_DELTAS, rook, queen)
        && occ[sq] == rook {
            return Some((sq, PieceKind::Rook));
        }

    // 5. Queens (on either ray).
    if let Some(sq) = first_slider_on_rays(occ, ti, &BISHOP_DELTAS, queen, queen) {
        return Some((sq, PieceKind::Queen));
    }
    if let Some(sq) = first_slider_on_rays(occ, ti, &ROOK_DELTAS, queen, queen) {
        return Some((sq, PieceKind::Queen));
    }

    // 6. King (most valuable; only legal if no enemy defends, but SEE ignores
    //    legality of the final recapture, handled by the value cap in callers).
    let king = Piece::make(color, PieceKind::King);
    for d in KING_DELTAS {
        let s = ti + d;
        if square::on_board_i32(s) && occ[s as usize] == king {
            return Some((s as usize, PieceKind::King));
        }
    }

    None
}

/// Scan outward from `from` along each delta; return the square of the first
/// piece encountered if it is `want_a` or `want_b`, else keep looking on other
/// rays. Returns the nearest matching slider square across all rays.
fn first_slider_on_rays(
    occ: &[crate::types::Piece; 128],
    from: i32,
    deltas: &[i32],
    want_a: crate::types::Piece,
    want_b: crate::types::Piece,
) -> Option<Sq> {
    for &d in deltas {
        let mut s = from + d;
        while square::on_board_i32(s) {
            let p = occ[s as usize];
            if !p.is_empty() {
                if p == want_a || p == want_b {
                    return Some(s as usize);
                }
                break; // blocked by an irrelevant piece
            }
            s += d;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_pawn_trade_is_zero() {
        // White pawn d4 captures the black pawn on e5, which is defended by the
        // black pawn on d6 (a black pawn on d6 attacks e5). Pawn for pawn = 0.
        let b = Board::from_fen("4k3/8/3p4/4p3/3P4/8/8/4K3 w - - 0 1").unwrap();
        let mv = Move::from_uci("d4e5").unwrap();
        assert_eq!(see(&b, mv), 0);
    }

    #[test]
    fn winning_capture_is_positive() {
        // White rook takes an undefended black rook.
        let b = Board::from_fen("4k3/8/8/3r4/8/8/3R4/4K3 w - - 0 1").unwrap();
        let mv = Move::from_uci("d2d5").unwrap();
        assert_eq!(see(&b, mv), value(PieceKind::Rook));
    }

    #[test]
    fn losing_capture_is_negative() {
        // White rook captures a pawn that is defended by a pawn: win 100, lose 500.
        let b = Board::from_fen("4k3/8/2p5/3p4/8/8/3R4/4K3 w - - 0 1").unwrap();
        let mv = Move::from_uci("d2d5").unwrap();
        assert_eq!(see(&b, mv), value(PieceKind::Pawn) - value(PieceKind::Rook));
    }

    #[test]
    fn xray_recapture_counts() {
        // Doubled white rooks (d2, d3) attack a black rook on d5 that is
        // defended by a black rook behind it on d6. The kings are far away.
        // Rxd5 Rxd5 R(x-ray)xd5: white nets a rook because the rook on d3,
        // once it moves, reveals the d2 rook's attack on d5.
        let b = Board::from_fen("4k3/8/3r4/3r4/8/3R4/3R4/4K3 w - - 0 1").unwrap();
        let mv = Move::from_uci("d3d5").unwrap();
        // +rook, -rook, +rook => +rook.
        assert_eq!(see(&b, mv), value(PieceKind::Rook));
    }
}
