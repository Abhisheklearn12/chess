//! EPD (Extended Position Description) parsing and a best-move test runner.
//!
//! EPD is the de-facto format for engine test suites (e.g. "Win at Chess"). A
//! record is a position followed by `;`-separated operations:
//!
//! ```text
//! 2rr3k/pp3pp1/1nnqbN1p/3pN3/2pP4/2P3Q1/PPB4P/R4RK1 w - - bm Qg6; id "WAC.001";
//! ```
//!
//! We parse the position and the common operations: `bm` (best moves), `am`
//! (avoid moves), and `id`. We then provide [`Epd::solve`], which searches the
//! position and reports whether the engine's choice satisfies the record.

use crate::board::Board;
use crate::san::move_to_san;
use crate::search::{search, SearchLimits};

/// A parsed EPD record.
#[derive(Clone, Debug)]
pub struct Epd {
    /// Full FEN (EPD omits the clocks; we default them to `0 1`).
    pub fen: String,
    /// Best moves in SAN (the `bm` operation); the engine should play one.
    pub best_moves: Vec<String>,
    /// Moves to avoid in SAN (the `am` operation).
    pub avoid_moves: Vec<String>,
    /// The record `id`, if present.
    pub id: Option<String>,
}

impl Epd {
    /// Parse a single EPD line.
    pub fn parse(line: &str) -> Result<Epd, String> {
        let line = line.trim();
        if line.is_empty() {
            return Err("empty EPD".to_string());
        }

        // The position is the first four whitespace tokens.
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 4 {
            return Err("EPD needs at least 4 position fields".to_string());
        }
        let fen = format!(
            "{} {} {} {} 0 1",
            tokens[0], tokens[1], tokens[2], tokens[3]
        );
        // Validate the position eagerly so a bad record fails fast.
        Board::from_fen(&fen)?;

        // Everything after the fourth token is the operation list.
        let ops_start = line
            .match_indices(char::is_whitespace)
            .nth(3)
            .map(|(i, _)| i)
            .unwrap_or(line.len());
        let ops = &line[ops_start..];

        let mut best_moves = Vec::new();
        let mut avoid_moves = Vec::new();
        let mut id = None;

        for op in ops.split(';') {
            let op = op.trim();
            if op.is_empty() {
                continue;
            }
            let mut parts = op.splitn(2, char::is_whitespace);
            let code = parts.next().unwrap_or("");
            let value = parts.next().unwrap_or("").trim();
            match code {
                "bm" => best_moves = value.split_whitespace().map(|s| s.to_string()).collect(),
                "am" => avoid_moves = value.split_whitespace().map(|s| s.to_string()).collect(),
                "id" => id = Some(value.trim_matches('"').to_string()),
                _ => {}
            }
        }

        Ok(Epd {
            fen,
            best_moves,
            avoid_moves,
            id,
        })
    }

    /// `true` if the SAN move `san` satisfies this record (is a best move and
    /// not an avoided move).
    pub fn accepts(&self, san: &str) -> bool {
        let norm = normalize(san);
        let in_best = self.best_moves.is_empty()
            || self.best_moves.iter().any(|m| normalize(m) == norm);
        let in_avoid = self.avoid_moves.iter().any(|m| normalize(m) == norm);
        in_best && !in_avoid
    }

    /// Search this position to `depth` and report whether the engine's chosen
    /// move satisfies the record. Returns `(passed, chosen_san)`.
    pub fn solve(&self, depth: i32) -> (bool, String) {
        let mut board = Board::from_fen(&self.fen).expect("validated in parse");
        let res = search(&mut board, SearchLimits::depth(depth));
        match res.best_move {
            Some(mv) => {
                let san = move_to_san(&board, mv);
                (self.accepts(&san), san)
            }
            None => (false, String::new()),
        }
    }
}

/// Normalize SAN for comparison: drop check/mate marks and annotations.
fn normalize(san: &str) -> String {
    san.trim()
        .chars()
        .filter(|c| !matches!(c, '+' | '#' | '!' | '?'))
        .collect()
}

/// Parse a whole EPD file (one record per non-empty line), skipping blanks and
/// `#` comments.
pub fn parse_suite(text: &str) -> Result<Vec<Epd>, String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        out.push(Epd::parse(line)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bm_and_id() {
        let epd = Epd::parse(
            "2rr3k/pp3pp1/1nnqbN1p/3pN3/2pP4/2P3Q1/PPB4P/R4RK1 w - - bm Qg6; id \"WAC.001\";",
        )
        .unwrap();
        assert_eq!(epd.best_moves, vec!["Qg6"]);
        assert_eq!(epd.id.as_deref(), Some("WAC.001"));
        assert!(epd.fen.ends_with("0 1"));
        assert!(epd.accepts("Qg6+"));
        assert!(!epd.accepts("Qh4"));
    }

    #[test]
    fn parses_avoid_moves() {
        let epd = Epd::parse("4k3/8/8/8/8/8/8/4K2R w K - am Kf2; bm O-O;").unwrap();
        assert!(epd.accepts("O-O"));
        assert!(!epd.accepts("Kf2"));
    }

    #[test]
    fn rejects_bad_position() {
        assert!(Epd::parse("not/a/fen w - -").is_err());
    }

    #[test]
    fn parses_suite_skips_comments() {
        let text = "# a comment\n\n4k3/8/8/8/8/8/8/4K2R w K - bm O-O;\n";
        let suite = parse_suite(text).unwrap();
        assert_eq!(suite.len(), 1);
    }
}
