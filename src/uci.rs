//! A minimal but functional UCI (Universal Chess Interface) front end.
//!
//! UCI is the protocol spoken by chess GUIs (Arena, Cute Chess, Banksia, ...).
//! Implementing it means this engine can play in real GUIs and tournaments.
//! Supported commands: `uci`, `isready`, `ucinewgame`, `position`, `go`
//! (`depth`, `movetime`, `wtime/btime/winc/binc/movestogo`, `infinite`), `d`
//! (debug print), and `quit`.
//!
//! [`UciEngine::handle_line`] processes one command and writes any responses to
//! a generic sink, which keeps the protocol logic unit-testable without real
//! I/O.

use crate::board::Board;
use crate::moves::Move;
use crate::search::{SearchLimits, Searcher, MAX_PLY};
use crate::timeman::TimeControl;
use std::io::{self, BufRead, Write};

/// Engine identification reported to the GUI.
const ENGINE_NAME: &str = "RustChess";
const ENGINE_AUTHOR: &str = "Rust Chess Engine";

/// Holds the position and persistent searcher across UCI commands.
pub struct UciEngine {
    board: Board,
    searcher: Searcher,
}

impl Default for UciEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl UciEngine {
    pub fn new() -> Self {
        UciEngine {
            board: Board::startpos(),
            searcher: Searcher::new(64),
        }
    }

    /// Run the blocking UCI loop, reading commands from stdin until `quit`.
    pub fn run(&mut self) {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut out = stdout.lock();
        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            let keep_going = self.handle_line(&line, &mut out);
            let _ = out.flush();
            if !keep_going {
                break;
            }
        }
    }

    /// Process a single UCI command line, writing responses to `out`. Returns
    /// `false` when the engine should quit.
    pub fn handle_line<W: Write>(&mut self, line: &str, out: &mut W) -> bool {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let Some(&cmd) = tokens.first() else {
            return true;
        };

        match cmd {
            "uci" => {
                let _ = writeln!(out, "id name {}", ENGINE_NAME);
                let _ = writeln!(out, "id author {}", ENGINE_AUTHOR);
                let _ = writeln!(out, "uciok");
            }
            "isready" => {
                let _ = writeln!(out, "readyok");
            }
            "ucinewgame" => {
                self.searcher.reset();
                self.board = Board::startpos();
            }
            "position" => self.handle_position(&tokens),
            "go" => self.handle_go(&tokens, out),
            "d" | "print" => {
                let _ = write!(out, "{}", self.board.ascii());
                let _ = writeln!(out, "fen: {}", self.board.to_fen());
                let _ = writeln!(out, "key: {:016x}", self.board.key);
            }
            "quit" => return false,
            _ => { /* ignore unknown commands per UCI spec */ }
        }
        true
    }

    fn handle_position(&mut self, tokens: &[&str]) {
        let mut idx = 1;
        // Base position.
        if tokens.get(idx) == Some(&"startpos") {
            self.board = Board::startpos();
            idx += 1;
        } else if tokens.get(idx) == Some(&"fen") {
            // FEN is the next up-to-6 tokens, until "moves" or end.
            let fen_tokens: Vec<&str> = tokens[idx + 1..]
                .iter()
                .take_while(|&&t| t != "moves")
                .copied()
                .collect();
            let fen = fen_tokens.join(" ");
            if let Ok(b) = Board::from_fen(&fen) {
                self.board = b;
            }
            idx += 1 + fen_tokens.len();
        }

        // Apply the move list.
        if tokens.get(idx) == Some(&"moves") {
            for &mtok in &tokens[idx + 1..] {
                if let Some(mv) = self.legal_uci_move(mtok) {
                    self.board.make_move_struct(mv);
                }
            }
        }
    }

    /// Parse a UCI move token and confirm it is legal in the current position.
    fn legal_uci_move(&mut self, tok: &str) -> Option<Move> {
        let parsed = Move::from_uci(tok)?;
        let mut legal = Vec::new();
        crate::movegen::legal_moves(&mut self.board, &mut legal);
        legal
            .into_iter()
            .find(|m| m.from == parsed.from && m.to == parsed.to && m.promotion == parsed.promotion)
    }

    fn handle_go<W: Write>(&mut self, tokens: &[&str], out: &mut W) {
        let mut limits = SearchLimits {
            max_depth: MAX_PLY as i32,
            movetime_ms: None,
            verbose: true,
        };
        let mut wtime = None;
        let mut btime = None;
        let mut winc = 0u64;
        let mut binc = 0u64;
        let mut movestogo = None;
        let mut infinite = false;

        let mut i = 1;
        while i < tokens.len() {
            match tokens[i] {
                "depth" => {
                    if let Some(d) = tokens.get(i + 1).and_then(|t| t.parse().ok()) {
                        limits.max_depth = d;
                    }
                    i += 2;
                }
                "movetime" => {
                    limits.movetime_ms = tokens.get(i + 1).and_then(|t| t.parse().ok());
                    i += 2;
                }
                "wtime" => {
                    wtime = tokens.get(i + 1).and_then(|t| t.parse().ok());
                    i += 2;
                }
                "btime" => {
                    btime = tokens.get(i + 1).and_then(|t| t.parse().ok());
                    i += 2;
                }
                "winc" => {
                    winc = tokens.get(i + 1).and_then(|t| t.parse().ok()).unwrap_or(0);
                    i += 2;
                }
                "binc" => {
                    binc = tokens.get(i + 1).and_then(|t| t.parse().ok()).unwrap_or(0);
                    i += 2;
                }
                "movestogo" => {
                    movestogo = tokens.get(i + 1).and_then(|t| t.parse().ok());
                    i += 2;
                }
                "infinite" => {
                    infinite = true;
                    i += 1;
                }
                _ => i += 1,
            }
        }

        // Derive a time budget from the clock if one was given.
        if limits.movetime_ms.is_none() && !infinite {
            let (time_left, inc) = if self.board.side_white {
                (wtime, winc)
            } else {
                (btime, binc)
            };
            if let Some(tl) = time_left {
                let tc = TimeControl {
                    time_left_ms: tl,
                    increment_ms: inc,
                    moves_to_go: movestogo,
                };
                limits.movetime_ms = Some(tc.budget_ms());
            } else {
                // No limits at all: default to a safe fixed time.
                limits.movetime_ms = Some(2000);
            }
        }

        let result = self.searcher.think(&mut self.board, limits);
        let best = result
            .best_move
            .map(|m| m.to_uci())
            .unwrap_or_else(|| "0000".to_string());
        let _ = writeln!(out, "bestmove {}", best);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_cmds(cmds: &[&str]) -> String {
        let mut engine = UciEngine::new();
        let mut out = Vec::new();
        for c in cmds {
            engine.handle_line(c, &mut out);
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn identifies_itself() {
        let out = run_cmds(&["uci"]);
        assert!(out.contains("id name"));
        assert!(out.contains("uciok"));
    }

    #[test]
    fn isready_responds() {
        assert!(run_cmds(&["isready"]).contains("readyok"));
    }

    #[test]
    fn position_and_go_returns_legal_bestmove() {
        let out = run_cmds(&["position startpos moves e2e4 e7e5", "go depth 4"]);
        assert!(out.contains("bestmove"));
        // bestmove must be a 4-or-5 char UCI move, not the null move.
        let line = out.lines().find(|l| l.starts_with("bestmove")).unwrap();
        let mv = line.split_whitespace().nth(1).unwrap();
        assert!(mv.len() >= 4 && mv != "0000");
    }

    #[test]
    fn quit_stops() {
        let mut engine = UciEngine::new();
        let mut out = Vec::new();
        assert!(!engine.handle_line("quit", &mut out));
    }
}
