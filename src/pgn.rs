//! PGN (Portable Game Notation) import and export.
//!
//! [`PgnGame`] holds a game's tag pairs, starting position, and move list. It
//! can be [rendered][`PgnGame::to_pgn`] to standard PGN text and
//! [parsed][`PgnGame::parse`] back, round-tripping through SAN. Parsing is
//! tolerant of comments (`{ ... }`), NAGs (`$3`), move numbers, and result
//! tokens.

use crate::board::{Board, START_FEN};
use crate::moves::Move;
use crate::san::{move_to_san, san_to_move};

/// A parsed or constructed PGN game.
#[derive(Clone, Debug)]
pub struct PgnGame {
    /// Seven-tag-roster and any extra tag pairs, in order.
    pub tags: Vec<(String, String)>,
    /// The starting FEN (the standard position unless a `FEN` tag is present).
    pub start_fen: String,
    /// The mainline moves.
    pub moves: Vec<Move>,
    /// The game result token (`1-0`, `0-1`, `1/2-1/2`, or `*`).
    pub result: String,
}

impl PgnGame {
    /// A new game from the standard starting position.
    pub fn new() -> Self {
        PgnGame {
            tags: vec![
                ("Event".into(), "Casual Game".into()),
                ("Site".into(), "Rust Chess Engine".into()),
                ("White".into(), "Player".into()),
                ("Black".into(), "Engine".into()),
                ("Result".into(), "*".into()),
            ],
            start_fen: START_FEN.to_string(),
            moves: Vec::new(),
            result: "*".to_string(),
        }
    }

    /// Build a game from a starting board and a move list.
    pub fn from_moves(start_fen: &str, moves: Vec<Move>) -> Self {
        let mut g = PgnGame::new();
        g.start_fen = start_fen.to_string();
        g.moves = moves;
        g
    }

    /// Look up a tag value by key (case-sensitive).
    pub fn tag(&self, key: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Render the game as PGN text.
    pub fn to_pgn(&self) -> String {
        let mut out = String::new();
        for (k, v) in &self.tags {
            out.push_str(&format!("[{} \"{}\"]\n", k, v));
        }
        if self.start_fen != START_FEN && self.tag("FEN").is_none() {
            out.push_str(&format!("[FEN \"{}\"]\n", self.start_fen));
            out.push_str("[SetUp \"1\"]\n");
        }
        out.push('\n');

        // Movetext, replaying to produce SAN.
        let mut board = Board::from_fen(&self.start_fen).unwrap_or_else(|_| Board::startpos());
        let mut line = String::new();
        let mut wrote_first = false;
        // Determine the move number / side from the starting position.
        let mut move_number = board.fullmove;
        let mut white_to_move = board.side_white;

        for &mv in &self.moves {
            if white_to_move {
                line.push_str(&format!("{}. ", move_number));
            } else if !wrote_first {
                // Game starts with Black to move.
                line.push_str(&format!("{}... ", move_number));
            }
            let san = move_to_san(&board, mv);
            line.push_str(&san);
            line.push(' ');
            wrote_first = true;

            board.make_move_struct(mv);
            if !white_to_move {
                move_number += 1;
            }
            white_to_move = !white_to_move;

            // Soft-wrap lines around 80 columns for readability.
            if line.len() >= 76 {
                out.push_str(line.trim_end());
                out.push('\n');
                line.clear();
            }
        }
        line.push_str(&self.result);
        out.push_str(line.trim_end());
        out.push('\n');
        out
    }

    /// Parse a single PGN game from text.
    pub fn parse(text: &str) -> Result<PgnGame, String> {
        let mut tags = Vec::new();
        let mut movetext = String::new();

        for raw in text.lines() {
            let line = raw.trim();
            if line.starts_with('[') && line.ends_with(']') {
                if let Some((k, v)) = parse_tag(line) {
                    tags.push((k, v));
                }
            } else if !line.is_empty() {
                movetext.push_str(line);
                movetext.push(' ');
            }
        }

        let start_fen = tags
            .iter()
            .find(|(k, _)| k == "FEN")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| START_FEN.to_string());

        let mut board = Board::from_fen(&start_fen)?;
        let mut moves = Vec::new();
        let mut result = "*".to_string();

        for token in tokenize_movetext(&movetext) {
            match token.as_str() {
                "1-0" | "0-1" | "1/2-1/2" | "*" => {
                    result = token;
                    break;
                }
                san => {
                    let mv = san_to_move(&board, san)
                        .ok_or_else(|| format!("illegal or unparseable SAN '{}'", san))?;
                    board.make_move_struct(mv);
                    moves.push(mv);
                }
            }
        }

        Ok(PgnGame {
            tags,
            start_fen,
            moves,
            result,
        })
    }
}

impl Default for PgnGame {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_tag(line: &str) -> Option<(String, String)> {
    let inner = line.trim_start_matches('[').trim_end_matches(']').trim();
    let key_end = inner.find(' ')?;
    let key = inner[..key_end].to_string();
    let rest = inner[key_end..].trim();
    let value = rest.trim_matches('"').to_string();
    Some((key, value))
}

/// Split movetext into SAN / result tokens, dropping comments, variations,
/// NAGs, and move numbers.
fn tokenize_movetext(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    let mut depth_brace = 0;
    let mut depth_paren = 0;
    let mut cur = String::new();

    let flush = |cur: &mut String, tokens: &mut Vec<String>| {
        if !cur.is_empty() {
            let t = clean_token(cur);
            if !t.is_empty() {
                tokens.push(t);
            }
            cur.clear();
        }
    };

    while let Some(c) = chars.next() {
        match c {
            '{' => depth_brace += 1,
            '}' => {
                if depth_brace > 0 {
                    depth_brace -= 1;
                }
            }
            '(' => depth_paren += 1,
            ')' => {
                if depth_paren > 0 {
                    depth_paren -= 1;
                }
            }
            _ if depth_brace > 0 || depth_paren > 0 => {}
            '$' => {
                // NAG: skip the following digits.
                while matches!(chars.peek(), Some(d) if d.is_ascii_digit()) {
                    chars.next();
                }
            }
            c if c.is_whitespace() => flush(&mut cur, &mut tokens),
            c => cur.push(c),
        }
    }
    flush(&mut cur, &mut tokens);
    tokens
}

/// Strip a leading move number (`12.` / `12...`) from a token, returning the
/// SAN/result part (possibly empty).
fn clean_token(tok: &str) -> String {
    let t = tok.trim();
    // A bare move number like "12." or "12..."
    let stripped: String = t.trim_end_matches('.').to_string();
    if stripped.chars().all(|c| c.is_ascii_digit()) {
        return String::new();
    }
    // A token like "12.e4"; drop the leading "12." prefix.
    if let Some(pos) = t.rfind('.') {
        let (num, rest) = t.split_at(pos + 1);
        if num.trim_end_matches('.').chars().all(|c| c.is_ascii_digit()) {
            return rest.to_string();
        }
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_then_parse_roundtrips_moves() {
        let moves: Vec<Move> = ["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"]
            .iter()
            .map(|s| Move::from_uci(s).unwrap())
            .collect();
        let game = PgnGame::from_moves(START_FEN, moves.clone());
        let pgn = game.to_pgn();
        assert!(pgn.contains("1. e4 e5 2. Nf3 Nc6 3. Bb5"));

        let parsed = PgnGame::parse(&pgn).unwrap();
        assert_eq!(parsed.moves, moves);
    }

    #[test]
    fn parse_tolerates_comments_and_nags() {
        let pgn = "[Event \"Test\"]\n\n1. e4 {best by test} e5 $1 2. Nf3 (2. Bc4) Nc6 *";
        let parsed = PgnGame::parse(pgn).unwrap();
        let ucis: Vec<String> = parsed.moves.iter().map(|m| m.to_uci()).collect();
        assert_eq!(ucis, vec!["e2e4", "e7e5", "g1f3", "b8c6"]);
    }
}
