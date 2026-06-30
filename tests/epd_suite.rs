//! EPD-driven test-suite runner.
//!
//! Parses an embedded mini "Win at Chess" style suite and asserts the engine
//! finds the documented best move in each position. All records here have been
//! confirmed to solve at the chosen search depth.

use rust_chess_engine::epd::{parse_suite, Epd};

/// A handful of EPD records the engine solves at depth 8.
const SUITE: &str = "\
# Mini tactical suite (subset of Win at Chess + a study)
2rr3k/pp3pp1/1nnqbN1p/3pN3/2pP4/2P3Q1/PPB4P/R4RK1 w - - bm Qg6; id \"WAC.001\";
5rk1/1ppb3p/p1pb4/6q1/3P1p1r/2P1R2P/PP1BQ1P1/5RKN w - - bm Rg3; id \"WAC.003\";
kbK5/pp6/1P6/8/8/8/8/R7 w - - bm Ra6; id \"Loyd.Excelsior\";
";

#[test]
fn engine_solves_embedded_suite() {
    let records = parse_suite(SUITE).expect("suite parses");
    assert_eq!(records.len(), 3);

    let mut solved = 0;
    for epd in &records {
        let (ok, played) = epd.solve(8);
        let id = epd.id.as_deref().unwrap_or("?");
        assert!(
            ok,
            "{}: engine played {} but expected one of {:?}",
            id, played, epd.best_moves
        );
        solved += 1;
    }
    assert_eq!(solved, records.len());
}

#[test]
fn epd_records_are_well_formed() {
    let records = parse_suite(SUITE).unwrap();
    for epd in &records {
        // Every record must have an id and at least one best move, and its FEN
        // must be parseable (parse_suite already validated the position).
        assert!(epd.id.is_some());
        assert!(!epd.best_moves.is_empty());
        assert!(rust_chess_engine::board::Board::from_fen(&epd.fen).is_ok());
    }
}

#[test]
fn single_record_round_trip() {
    let epd = Epd::parse("4k3/8/8/8/8/8/8/4K2R w K - bm O-O; id \"castle\";").unwrap();
    assert!(epd.accepts("O-O"));
    assert_eq!(epd.id.as_deref(), Some("castle"));
}
