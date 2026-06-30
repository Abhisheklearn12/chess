//! Tests that each evaluation term actually fires with the expected sign.
//!
//! These complement the color-symmetry invariant test: symmetry proves the
//! terms are unbiased, while these prove they are *present* and oriented
//! correctly (a term that was accidentally zero would pass symmetry but fail
//! here).

use rust_chess_engine::board::Board;
use rust_chess_engine::eval::{eval_terms, evaluate};

fn terms(fen: &str) -> rust_chess_engine::eval::EvalTerms {
    eval_terms(&Board::from_fen(fen).unwrap())
}

#[test]
fn material_term_tracks_imbalance() {
    // White is up a full queen.
    let t = terms("4k3/8/8/8/8/8/8/3QK3 w - - 0 1");
    assert!(t.material >= 900, "material was {}", t.material);
    // And the mirror is the opposite.
    let mirror = terms("3qk3/8/8/8/8/8/8/4K3 w - - 0 1");
    assert_eq!(t.material, -mirror.material);
}

#[test]
fn passed_pawn_is_rewarded() {
    // A lone white pawn with no black pawns anywhere is passed.
    let t = terms("4k3/8/8/8/8/P7/8/4K3 w - - 0 1");
    assert!(t.pawns > 0, "passed-pawn bonus missing (pawns={})", t.pawns);
}

#[test]
fn doubled_pawns_are_penalized() {
    // White has doubled, blockaded a-pawns (not passed); the structure term
    // must be negative from White's perspective.
    let t = terms("4k3/p7/8/8/8/P7/P7/4K3 w - - 0 1");
    assert!(t.pawns < 0, "doubled-pawn penalty missing (pawns={})", t.pawns);
}

#[test]
fn bishop_pair_bonus() {
    // White has two bishops, Black none.
    let t = terms("4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1");
    assert!(t.bishop_pair > 0, "bishop pair bonus missing");
}

#[test]
fn rook_on_open_file_bonus() {
    // White rook on a completely open a-file.
    let t = terms("4k3/8/8/8/8/8/8/R3K3 w - - 0 1");
    assert!(t.rook_files > 0, "rook open-file bonus missing");
}

#[test]
fn king_shield_bonus() {
    // White king tucked behind a wall of pawns; Black king exposed.
    let t = terms("4k3/8/8/8/8/8/3PPP2/4K3 w - - 0 1");
    assert!(t.king_safety > 0, "king-shield bonus missing");
}

#[test]
fn mobility_favors_active_pieces() {
    // A centralized white knight (8 moves) vs a cornered black knight (2 moves).
    let t = terms("n3k3/8/8/4N3/8/8/8/4K3 w - - 0 1");
    assert!(t.mobility > 0, "mobility term missing (mobility={})", t.mobility);
}

#[test]
fn total_folds_into_side_to_move_score() {
    // evaluate() returns a side-to-move-relative score; for a White-favorable
    // position with White to move it must be clearly positive.
    let s = evaluate(&Board::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap());
    assert!(s > 800, "expected a winning score, got {}", s);
}
