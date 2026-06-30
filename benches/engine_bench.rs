//! Criterion micro-benchmarks for the hot paths: move generation (via perft),
//! static evaluation, SEE, and a fixed-depth search. Run with `cargo bench`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_chess_engine::board::Board;
use rust_chess_engine::eval::evaluate;
use rust_chess_engine::movegen::legal_moves;
use rust_chess_engine::perft::perft;
use rust_chess_engine::search::{search, SearchLimits};

const KIWIPETE: &str =
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

fn bench_movegen(c: &mut Criterion) {
    let mut board = Board::from_fen(KIWIPETE).unwrap();
    c.bench_function("legal_moves_kiwipete", |b| {
        b.iter(|| {
            let mut moves = Vec::new();
            legal_moves(black_box(&mut board), &mut moves);
            black_box(moves.len())
        })
    });
}

fn bench_perft(c: &mut Criterion) {
    let mut board = Board::startpos();
    c.bench_function("perft_startpos_depth4", |b| {
        b.iter(|| perft(black_box(&mut board), 4))
    });
}

fn bench_eval(c: &mut Criterion) {
    let board = Board::from_fen(KIWIPETE).unwrap();
    c.bench_function("evaluate_kiwipete", |b| {
        b.iter(|| evaluate(black_box(&board)))
    });
}

fn bench_search(c: &mut Criterion) {
    c.bench_function("search_startpos_depth6", |b| {
        b.iter(|| {
            let mut board = Board::startpos();
            search(black_box(&mut board), SearchLimits::depth(6))
        })
    });
}

criterion_group!(benches, bench_movegen, bench_perft, bench_eval, bench_search);
criterion_main!(benches);
