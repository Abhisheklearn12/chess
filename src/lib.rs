//! A complete, handwritten chess engine in safe Rust.
//!
//! The crate is organized as a stack of focused modules:
//!
//! ```text
//!   types  ──>  moves ──>  board ──>  attacks ──>  movegen ──>  search
//!     │           │          │           │            │           │
//!     └─ zobrist ─┴──────────┴─ eval ────┴── perft ───┴─ san/pgn ─┴─ uci
//! ```
//!
//! * [`types`]:      colors, pieces, the `0x88` square model.
//! * [`moves`]:      the [`moves::Move`] value type.
//! * [`board`]:      position state, FEN, make/unmake, draw detection.
//! * [`attacks`]:    square-attack and check detection.
//! * [`movegen`]:    pseudo-legal and fully legal move generation.
//! * [`eval`]:       tapered piece-square-table evaluation.
//! * [`search`]:     iterative-deepening PVS with TT, null-move, LMR.
//! * [`transposition`], [`zobrist`]: hashing infrastructure.
//! * [`perft`]:      move-generation correctness oracle.
//! * [`san`], [`pgn`], [`uci`], [`book`], [`timeman`]: protocols & I/O.
//! * [`engine`]:     a stable facade re-exporting the public surface.
//! * [`ui`], [`auth`]: the interactive terminal application.

pub mod attacks;
pub mod auth;
pub mod board;
pub mod book;
pub mod engine;
pub mod epd;
pub mod eval;
pub mod game;
#[macro_use]
pub mod log;
pub mod movegen;
pub mod moves;
pub mod perft;
pub mod see;
pub mod pgn;
pub mod san;
pub mod search;
pub mod timeman;
pub mod transposition;
pub mod types;
pub mod uci;
pub mod ui;
pub mod zobrist;
