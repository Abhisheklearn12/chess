//! Integration-level perft suite.
//!
//! These are the canonical perft positions from the Chess Programming Wiki.
//! Matching their published node counts is the gold-standard proof that move
//! generation, make/unmake, and every special move (castling, en passant,
//! promotions, pins, double check) are handled correctly.
//!
//! Shallow depths run by default (fast even in a debug build). The deeper,
//! slower checks are marked `#[ignore]`; run them with:
//!
//! ```text
//! cargo test --release -- --ignored
//! ```

use rust_chess_engine::board::Board;
use rust_chess_engine::perft::perft;

fn assert_perft(fen: &str, depth: u32, expected: u64) {
    let mut b = Board::from_fen(fen).expect("valid FEN");
    let got = perft(&mut b, depth);
    assert_eq!(
        got, expected,
        "perft({}) for '{}' = {}, expected {}",
        depth, fen, got, expected
    );
}

const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const KIWIPETE: &str =
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
const POS3: &str = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
const POS4: &str = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
const POS5: &str = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";
const POS6: &str =
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10";

#[test]
fn perft_startpos() {
    assert_perft(STARTPOS, 1, 20);
    assert_perft(STARTPOS, 2, 400);
    assert_perft(STARTPOS, 3, 8_902);
    assert_perft(STARTPOS, 4, 197_281);
}

#[test]
fn perft_kiwipete() {
    assert_perft(KIWIPETE, 1, 48);
    assert_perft(KIWIPETE, 2, 2_039);
    assert_perft(KIWIPETE, 3, 97_862);
}

#[test]
fn perft_position_3() {
    assert_perft(POS3, 1, 14);
    assert_perft(POS3, 2, 191);
    assert_perft(POS3, 3, 2_812);
    assert_perft(POS3, 4, 43_238);
}

#[test]
fn perft_position_4() {
    assert_perft(POS4, 1, 6);
    assert_perft(POS4, 2, 264);
    assert_perft(POS4, 3, 9_467);
}

#[test]
fn perft_position_5() {
    assert_perft(POS5, 1, 44);
    assert_perft(POS5, 2, 1_486);
    assert_perft(POS5, 3, 62_379);
}

#[test]
fn perft_position_6() {
    assert_perft(POS6, 1, 46);
    assert_perft(POS6, 2, 2_079);
    assert_perft(POS6, 3, 89_890);
}

// ---- Deep checks (slow; run with `cargo test --release -- --ignored`) ----

#[test]
#[ignore = "slow; run in release"]
fn perft_startpos_deep() {
    assert_perft(STARTPOS, 5, 4_865_609);
    assert_perft(STARTPOS, 6, 119_060_324);
}

#[test]
#[ignore = "slow; run in release"]
fn perft_kiwipete_deep() {
    assert_perft(KIWIPETE, 4, 4_085_603);
    assert_perft(KIWIPETE, 5, 193_690_690);
}

#[test]
#[ignore = "slow; run in release"]
fn perft_position_3_deep() {
    assert_perft(POS3, 5, 674_624);
    assert_perft(POS3, 6, 11_030_083);
}

#[test]
#[ignore = "slow; run in release"]
fn perft_position_4_deep() {
    assert_perft(POS4, 4, 422_333);
    assert_perft(POS4, 5, 15_833_292);
}
