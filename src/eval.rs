//! Static evaluation.
//!
//! Uses a *tapered* evaluation built on the well-known PeSTO piece-square
//! tables: a separate middlegame and endgame score is accumulated and then
//! blended according to the amount of material left on the board (the "game
//! phase"). On top of that we add a bishop-pair bonus, a doubled-pawn penalty,
//! a passed-pawn bonus, and a side-to-move tempo bonus.
//!
//! All scores are in centipawns and returned **relative to the side to move**
//! (positive = good for the player about to move), which is the convention the
//! negamax search expects.

use crate::board::Board;
use crate::types::{square, Color, PieceKind, Sq};

/// Middlegame material values, indexed by [`PieceKind::index`].
const MG_VALUE: [i32; 6] = [82, 337, 365, 477, 1025, 0];
/// Endgame material values.
const EG_VALUE: [i32; 6] = [94, 281, 297, 512, 936, 0];

/// Per-piece game-phase weights (knight/bishop = 1, rook = 2, queen = 4).
const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0];
/// Phase value of the full starting array (4 minors·1 + 4 minors·1 + 4 rooks·2
/// + 2 queens·4 ... summed = 24).
const TOTAL_PHASE: i32 = 24;

/// Bonus for holding both bishops.
const BISHOP_PAIR: i32 = 30;
/// Penalty per doubled pawn (per extra pawn on a file).
const DOUBLED_PAWN: i32 = 12;
/// Bonus for a passed pawn, scaled by how advanced it is.
const PASSED_PAWN_BASE: i32 = 15;
/// Small bonus for having the move.
const TEMPO: i32 = 10;

/// Score returned for a checkmate, offset by ply in the search so shorter mates
/// are preferred. Kept well inside `i32` range.
pub const MATE: i32 = 30_000;
/// Threshold above which a score is considered a forced mate.
pub const MATE_THRESHOLD: i32 = MATE - 1000;

// PeSTO piece-square tables. Written rank-8-first (index 0 == a8), so a white
// piece on square `sq64` reads `table[sq64 ^ 56]` and a black piece reads
// `table[sq64]`.

#[rustfmt::skip]
const MG_PAWN: [i32; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
     98, 134,  61,  95,  68, 126,  34, -11,
     -6,   7,  26,  31,  65,  56,  25, -20,
    -14,  13,   6,  21,  23,  12,  17, -23,
    -27,  -2,  -5,  12,  17,   6,  10, -25,
    -26,  -4,  -4, -10,   3,   3,  33, -12,
    -35,  -1, -20, -23, -15,  24,  38, -22,
      0,   0,   0,   0,   0,   0,   0,   0,
];
#[rustfmt::skip]
const EG_PAWN: [i32; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
    178, 173, 158, 134, 147, 132, 165, 187,
     94, 100,  85,  67,  56,  53,  82,  84,
     32,  24,  13,   5,  -2,   4,  17,  17,
     13,   9,  -3,  -7,  -7,  -8,   3,  -1,
      4,   7,  -6,   1,   0,  -5,  -1,  -8,
     13,   8,   8,  10,  13,   0,   2,  -7,
      0,   0,   0,   0,   0,   0,   0,   0,
];
#[rustfmt::skip]
const MG_KNIGHT: [i32; 64] = [
   -167, -89, -34, -49,  61, -97, -15,-107,
    -73, -41,  72,  36,  23,  62,   7, -17,
    -47,  60,  37,  65,  84, 129,  73,  44,
     -9,  17,  19,  53,  37,  69,  18,  22,
    -13,   4,  16,  13,  28,  19,  21,  -8,
    -23,  -9,  12,  10,  19,  17,  25, -16,
    -29, -53, -12,  -3,  -1,  18, -14, -19,
   -105, -21, -58, -33, -17, -28, -19, -23,
];
#[rustfmt::skip]
const EG_KNIGHT: [i32; 64] = [
    -58, -38, -13, -28, -31, -27, -63, -99,
    -25,  -8, -25,  -2,  -9, -25, -24, -52,
    -24, -20,  10,   9,  -1,  -9, -19, -41,
    -17,   3,  22,  22,  22,  11,   8, -18,
    -18,  -6,  16,  25,  16,  17,   4, -18,
    -23,  -3,  -1,  15,  10,  -3, -20, -22,
    -42, -20, -10,  -5,  -2, -20, -23, -44,
    -29, -51, -23, -15, -22, -18, -50, -64,
];
#[rustfmt::skip]
const MG_BISHOP: [i32; 64] = [
    -29,   4, -82, -37, -25, -42,   7,  -8,
    -26,  16, -18, -13,  30,  59,  18, -47,
    -16,  37,  43,  40,  35,  50,  37,  -2,
     -4,   5,  19,  50,  37,  37,   7,  -2,
     -6,  13,  13,  26,  34,  12,  10,   4,
      0,  15,  15,  15,  14,  27,  18,  10,
      4,  15,  16,   0,   7,  21,  33,   1,
    -33,  -3, -14, -21, -13, -12, -39, -21,
];
#[rustfmt::skip]
const EG_BISHOP: [i32; 64] = [
    -14, -21, -11,  -8,  -7,  -9, -17, -24,
     -8,  -4,   7, -12,  -3, -13,  -4, -14,
      2,  -8,   0,  -1,  -2,   6,   0,   4,
     -3,   9,  12,   9,  14,  10,   3,   2,
     -6,   3,  13,  19,   7,  10,  -3,  -9,
    -12,  -3,   8,  10,  13,   3,  -7, -15,
    -14, -18,  -7,  -1,   4,  -9, -15, -27,
    -23,  -9, -23,  -5,  -9, -16,  -5, -17,
];
#[rustfmt::skip]
const MG_ROOK: [i32; 64] = [
     32,  42,  32,  51,  63,   9,  31,  43,
     27,  32,  58,  62,  80,  67,  26,  44,
     -5,  19,  26,  36,  17,  45,  61,  16,
    -24, -11,   7,  26,  24,  35,  -8, -20,
    -36, -26, -12,  -1,   9,  -7,   6, -23,
    -45, -25, -16, -17,   3,   0,  -5, -33,
    -44, -16, -20,  -9,  -1,  11,  -6, -71,
    -19, -13,   1,  17,  16,   7, -37, -26,
];
#[rustfmt::skip]
const EG_ROOK: [i32; 64] = [
     13,  10,  18,  15,  12,  12,   8,   5,
     11,  13,  13,  11,  -3,   3,   8,   3,
      7,   7,   7,   5,   4,  -3,  -5,  -3,
      4,   3,  13,   1,   2,   1,  -1,   2,
      3,   5,   8,   4,  -5,  -6,  -8, -11,
     -4,   0,  -5,  -1,  -7, -12,  -8, -16,
     -6,  -6,   0,   2,  -9,  -9, -11,  -3,
     -9,   2,   3,  -1,  -5, -13,   4, -20,
];
#[rustfmt::skip]
const MG_QUEEN: [i32; 64] = [
    -28,   0,  29,  12,  59,  44,  43,  45,
    -24, -39,  -5,   1, -16,  57,  28,  54,
    -13, -17,   7,   8,  29,  56,  47,  57,
    -27, -27, -16, -16,  -1,  17,  -2,   1,
     -9, -26,  -9, -10,  -2,  -4,   3,  -3,
    -14,   2, -11,  -2,  -5,   2,  14,   5,
    -35,  -8,  11,   2,   8,  15,  -3,   1,
     -1, -18,  -9,  10, -15, -25, -31, -50,
];
#[rustfmt::skip]
const EG_QUEEN: [i32; 64] = [
     -9,  22,  22,  27,  27,  19,  10,  20,
    -17,  20,  32,  41,  58,  25,  30,   0,
    -20,   6,   9,  49,  47,  35,  19,   9,
      3,  22,  24,  45,  57,  40,  57,  36,
    -18,  28,  19,  47,  31,  34,  39,  23,
    -16, -27,  15,   6,   9,  17,  10,   5,
    -22, -23, -30, -16, -16, -23, -36, -32,
    -33, -28, -22, -43,  -5, -32, -20, -41,
];
#[rustfmt::skip]
const MG_KING: [i32; 64] = [
    -65,  23,  16, -15, -56, -34,   2,  13,
     29,  -1, -20,  -7,  -8,  -4, -38, -29,
     -9,  24,   2, -16, -20,   6,  22, -22,
    -17, -20, -12, -27, -30, -25, -14, -36,
    -49,  -1, -27, -39, -46, -44, -33, -51,
    -14, -14, -22, -46, -44, -30, -15, -27,
      1,   7,  -8, -64, -43, -16,   9,   8,
    -15,  36,  12, -54,   8, -28,  24,  14,
];
#[rustfmt::skip]
const EG_KING: [i32; 64] = [
    -74, -35, -18, -18, -11,  15,   4, -17,
    -12,  17,  14,  17,  17,  38,  23,  11,
     10,  17,  23,  15,  20,  45,  44,  13,
     -8,  22,  24,  27,  26,  33,  26,   3,
    -18,  -4,  21,  24,  27,  23,   9, -11,
    -19,  -3,  11,  21,  23,  16,   7,  -9,
    -27, -11,   4,  13,  14,   4,  -5, -17,
    -53, -34, -21, -11, -28, -14, -24, -43,
];

#[inline]
fn mg_table(kind: PieceKind) -> &'static [i32; 64] {
    match kind {
        PieceKind::Pawn => &MG_PAWN,
        PieceKind::Knight => &MG_KNIGHT,
        PieceKind::Bishop => &MG_BISHOP,
        PieceKind::Rook => &MG_ROOK,
        PieceKind::Queen => &MG_QUEEN,
        PieceKind::King => &MG_KING,
    }
}

#[inline]
fn eg_table(kind: PieceKind) -> &'static [i32; 64] {
    match kind {
        PieceKind::Pawn => &EG_PAWN,
        PieceKind::Knight => &EG_KNIGHT,
        PieceKind::Bishop => &EG_BISHOP,
        PieceKind::Rook => &EG_ROOK,
        PieceKind::Queen => &EG_QUEEN,
        PieceKind::King => &EG_KING,
    }
}

/// 0x88 square to a 0..63 index (a1 = 0 ... h8 = 63).
#[inline]
fn sq64(s: Sq) -> usize {
    (square::rank(s) * 8 + square::file(s)) as usize
}

/// PST index for a piece of `color` on 0..63 square `idx64`.
#[inline]
fn pst_index(idx64: usize, color: Color) -> usize {
    match color {
        Color::White => idx64 ^ 56,
        Color::Black => idx64,
    }
}

/// A breakdown of the evaluation, useful for the UI's analysis screen. All
/// fields are from White's perspective, in centipawns.
#[derive(Clone, Copy, Debug, Default)]
pub struct EvalTerms {
    pub material: i32,
    pub positional: i32,
    pub pawns: i32,
    pub bishop_pair: i32,
    pub mobility: i32,
    pub king_safety: i32,
    pub rook_files: i32,
    pub total: i32,
}

/// Centipawns per legal destination square for each sliding/jumping piece.
const MOBILITY_WEIGHT: [i32; 6] = [0, 4, 4, 2, 1, 0];
/// Bonus for a friendly pawn shielding the king (per shield pawn).
const KING_SHIELD: i32 = 9;
/// Bonus for a rook on a fully open / semi-open file.
const ROOK_OPEN_FILE: i32 = 18;
const ROOK_SEMI_OPEN_FILE: i32 = 9;

/// Compute the full evaluation breakdown from White's perspective.
pub fn eval_terms(board: &Board) -> EvalTerms {
    let mut mg = [0i32; 2];
    let mut eg = [0i32; 2];
    let mut material = [0i32; 2];
    let mut positional = [0i32; 2];
    let mut phase = 0i32;

    let mut bishops = [0i32; 2];
    // pawn file counts per color for doubled-pawn detection
    let mut pawn_files = [[0i32; 8]; 2];

    for r in 0..8 {
        for f in 0..8 {
            let s = square::make(r, f);
            let p = board.cells[s];
            let (color, kind) = match (p.color(), p.kind()) {
                (Some(c), Some(k)) => (c, k),
                _ => continue,
            };
            let ci = color.index();
            let ki = kind.index();
            let idx = pst_index(sq64(s), color);

            material[ci] += MG_VALUE[ki];
            positional[ci] += mg_table(kind)[idx];

            mg[ci] += MG_VALUE[ki] + mg_table(kind)[idx];
            eg[ci] += EG_VALUE[ki] + eg_table(kind)[idx];
            phase += PHASE_WEIGHT[ki];

            if kind == PieceKind::Bishop {
                bishops[ci] += 1;
            }
            if kind == PieceKind::Pawn {
                pawn_files[ci][f as usize] += 1;
            }
        }
    }

    // Bishop pair.
    let mut bishop_pair = 0;
    if bishops[0] >= 2 {
        bishop_pair += BISHOP_PAIR;
    }
    if bishops[1] >= 2 {
        bishop_pair -= BISHOP_PAIR;
    }

    // Doubled pawns + passed pawns.
    let pawn_struct = pawn_structure(board, &pawn_files);

    // Mobility, king safety, and rook files (all White's perspective).
    let extra = positional_extras(board, &pawn_files);

    // Taper between middlegame and endgame.
    let phase = phase.min(TOTAL_PHASE);
    let blend = |w: i32, b: i32| w - b;
    let positional_extra = extra.mobility + extra.king_safety + extra.rook_files;
    let mg_score = blend(mg[0], mg[1]) + bishop_pair + pawn_struct + positional_extra;
    let eg_score = blend(eg[0], eg[1]) + bishop_pair + pawn_struct + positional_extra;
    let total = (mg_score * phase + eg_score * (TOTAL_PHASE - phase)) / TOTAL_PHASE;

    EvalTerms {
        material: material[0] - material[1],
        positional: positional[0] - positional[1],
        pawns: pawn_struct,
        bishop_pair,
        mobility: extra.mobility,
        king_safety: extra.king_safety,
        rook_files: extra.rook_files,
        total,
    }
}

/// White-perspective extra terms computed symmetrically for both colors.
struct Extras {
    mobility: i32,
    king_safety: i32,
    rook_files: i32,
}

fn positional_extras(board: &Board, pawn_files: &[[i32; 8]; 2]) -> Extras {
    use crate::attacks::{BISHOP_DELTAS, KNIGHT_DELTAS, ROOK_DELTAS};

    let mut mobility = [0i32; 2];
    let mut king_safety = [0i32; 2];
    let mut rook_files = [0i32; 2];

    for r in 0..8 {
        for f in 0..8 {
            let s = square::make(r, f);
            let (color, kind) = match (board.cells[s].color(), board.cells[s].kind()) {
                (Some(c), Some(k)) => (c, k),
                _ => continue,
            };
            let ci = color.index();
            match kind {
                PieceKind::Knight => {
                    mobility[ci] += MOBILITY_WEIGHT[kind.index()] * leaper_mobility(board, s, &KNIGHT_DELTAS, color);
                }
                PieceKind::Bishop => {
                    mobility[ci] += MOBILITY_WEIGHT[kind.index()] * slider_mobility(board, s, &BISHOP_DELTAS, color);
                }
                PieceKind::Rook => {
                    mobility[ci] += MOBILITY_WEIGHT[kind.index()] * slider_mobility(board, s, &ROOK_DELTAS, color);
                    rook_files[ci] += rook_file_bonus(pawn_files, color, f as usize);
                }
                PieceKind::Queen => {
                    let m = slider_mobility(board, s, &ROOK_DELTAS, color)
                        + slider_mobility(board, s, &BISHOP_DELTAS, color);
                    mobility[ci] += MOBILITY_WEIGHT[kind.index()] * m;
                }
                PieceKind::King => {
                    king_safety[ci] += king_shield(board, s, color);
                }
                PieceKind::Pawn => {}
            }
        }
    }

    Extras {
        mobility: mobility[0] - mobility[1],
        king_safety: king_safety[0] - king_safety[1],
        rook_files: rook_files[0] - rook_files[1],
    }
}

/// Count squares a leaper (knight) can move to (empty or enemy-occupied).
fn leaper_mobility(board: &Board, s: Sq, deltas: &[i32], color: Color) -> i32 {
    let si = s as i32;
    let mut count = 0;
    for &d in deltas {
        let t = si + d;
        if square::on_board_i32(t) {
            let p = board.cells[t as usize];
            if p.is_empty() || p.is_color(color.flip()) {
                count += 1;
            }
        }
    }
    count
}

/// Count squares a slider can reach along its rays (stops at the first piece,
/// counting it if it is an enemy).
fn slider_mobility(board: &Board, s: Sq, deltas: &[i32], color: Color) -> i32 {
    let si = s as i32;
    let mut count = 0;
    for &d in deltas {
        let mut t = si + d;
        while square::on_board_i32(t) {
            let p = board.cells[t as usize];
            if p.is_empty() {
                count += 1;
            } else {
                if p.is_color(color.flip()) {
                    count += 1;
                }
                break;
            }
            t += d;
        }
    }
    count
}

/// Bonus for a rook on a file with no friendly pawns (semi-open) or no pawns at
/// all (open).
fn rook_file_bonus(pawn_files: &[[i32; 8]; 2], color: Color, file: usize) -> i32 {
    let own = pawn_files[color.index()][file];
    let enemy = pawn_files[color.flip().index()][file];
    if own == 0 && enemy == 0 {
        ROOK_OPEN_FILE
    } else if own == 0 {
        ROOK_SEMI_OPEN_FILE
    } else {
        0
    }
}

/// Count friendly pawns sheltering the king on the three files around it, one
/// rank in front.
fn king_shield(board: &Board, king_sq: Sq, color: Color) -> i32 {
    let pawn = crate::types::Piece::make(color, PieceKind::Pawn);
    let dir = if color == Color::White { 1 } else { -1 };
    let rank = square::rank(king_sq) + dir;
    if !(0..8).contains(&rank) {
        return 0;
    }
    let mut count = 0;
    let kf = square::file(king_sq);
    for f in (kf - 1)..=(kf + 1) {
        if (0..8).contains(&f) && board.cells[square::make(rank, f)] == pawn {
            count += 1;
        }
    }
    count * KING_SHIELD
}

/// Doubled-pawn penalty and passed-pawn bonus, from White's perspective.
fn pawn_structure(board: &Board, pawn_files: &[[i32; 8]; 2]) -> i32 {
    let mut score = 0;
    // Doubled pawns.
    for (white_count, black_count) in pawn_files[0].iter().zip(pawn_files[1].iter()) {
        if *white_count > 1 {
            score -= (*white_count - 1) * DOUBLED_PAWN;
        }
        if *black_count > 1 {
            score += (*black_count - 1) * DOUBLED_PAWN;
        }
    }
    // Passed pawns: no enemy pawn on the same or adjacent files ahead of it.
    for r in 0..8 {
        for f in 0..8 {
            let s = square::make(r, f);
            match (board.cells[s].color(), board.cells[s].kind()) {
                (Some(Color::White), Some(PieceKind::Pawn))
                    if is_passed(board, s, Color::White) => {
                        score += PASSED_PAWN_BASE + r * 8;
                    }
                (Some(Color::Black), Some(PieceKind::Pawn))
                    if is_passed(board, s, Color::Black) => {
                        score -= PASSED_PAWN_BASE + (7 - r) * 8;
                    }
                _ => {}
            }
        }
    }
    score
}

/// `true` if the pawn of `color` on `s` has no enemy pawns ahead on its own or
/// adjacent files.
fn is_passed(board: &Board, s: Sq, color: Color) -> bool {
    let file = square::file(s);
    let rank = square::rank(s);
    let enemy_pawn = crate::types::Piece::make(color.flip(), PieceKind::Pawn);
    let ranks_ahead: Vec<i32> = match color {
        Color::White => (rank + 1..8).collect(),
        Color::Black => (0..rank).collect(),
    };
    for rr in ranks_ahead {
        for ff in (file - 1)..=(file + 1) {
            if (0..8).contains(&ff) && board.cells[square::make(rr, ff)] == enemy_pawn {
                return false;
            }
        }
    }
    true
}

/// The static evaluation in centipawns, relative to the side to move.
pub fn evaluate(board: &Board) -> i32 {
    let terms = eval_terms(board);
    let white_pov = terms.total + TEMPO * board.side_to_move().sign();
    white_pov * board.side_to_move().sign()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_is_balanced() {
        let b = Board::startpos();
        // Symmetric position: only the tempo bonus remains.
        assert_eq!(evaluate(&b), TEMPO);
    }

    #[test]
    fn extra_queen_is_winning() {
        let b = Board::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        assert!(evaluate(&b) > 800);
    }

    #[test]
    fn perspective_flips() {
        // Same material imbalance evaluated from each side should be opposite.
        let w = Board::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        let b = Board::from_fen("3qk3/8/8/8/8/8/8/4K3 b - - 0 1").unwrap();
        // White up a queen (to move) and black up a queen (to move) are both
        // winning for the side to move.
        assert!(evaluate(&w) > 0);
        assert!(evaluate(&b) > 0);
    }
}
