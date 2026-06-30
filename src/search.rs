//! Alpha-beta search.
//!
//! The search is a fairly complete modern negamax:
//!
//! * **Iterative deepening** with a triangular principal-variation table.
//! * **Principal Variation Search** (PVS): the first move gets a full window,
//!   the rest a zero-window scout re-searched only on a fail-high.
//! * **Transposition table** cutoffs and move ordering, with mate scores
//!   corrected for the distance to root so the engine prefers faster mates.
//! * **Quiescence search** at the leaves to resolve captures and avoid the
//!   horizon effect, ordered by MVV-LVA.
//! * **Null-move pruning**, **late-move reductions**, killer moves, and a
//!   history heuristic for move ordering.
//! * **Time management** via a deadline checked periodically; the search can
//!   always return the best move found so far.
//!
//! Scores are centipawns from the side-to-move's perspective. Draws by
//! repetition, the fifty-move rule, and insufficient material are scored 0.

use crate::board::Board;
use crate::eval::{evaluate, MATE, MATE_THRESHOLD};
use crate::log::SearchTelemetry;
use crate::movegen::{gen_captures, legal_moves};
use crate::moves::{Move, MoveList};
use crate::transposition::{NodeType, PackedMove, ProbeResult, TranspositionTable};
use crate::types::{square, Piece, PieceKind};
use crate::zobrist::keys;
use std::time::{Duration, Instant};

/// Maximum search depth / ply we will ever reach.
pub const MAX_PLY: usize = 64;
const INF: i32 = 1_000_000;

/// What bounds a search: a maximum depth and/or a wall-clock budget.
#[derive(Clone, Copy, Debug)]
pub struct SearchLimits {
    /// Hard depth ceiling in plies.
    pub max_depth: i32,
    /// Optional soft time budget in milliseconds.
    pub movetime_ms: Option<u64>,
    /// If `true`, print UCI-style `info` lines for each completed depth.
    pub verbose: bool,
}

impl Default for SearchLimits {
    fn default() -> Self {
        SearchLimits {
            max_depth: 64,
            movetime_ms: Some(2000),
            verbose: false,
        }
    }
}

impl SearchLimits {
    /// Search to a fixed depth with no time limit.
    pub fn depth(d: i32) -> Self {
        SearchLimits {
            max_depth: d,
            movetime_ms: None,
            verbose: false,
        }
    }

    /// Search for a fixed amount of time (capped at [`MAX_PLY`] depth).
    pub fn movetime(ms: u64) -> Self {
        SearchLimits {
            max_depth: MAX_PLY as i32,
            movetime_ms: Some(ms),
            verbose: false,
        }
    }
}

/// The outcome of a search.
#[derive(Clone, Debug, Default)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: i32,
    pub depth: i32,
    pub nodes: u64,
    pub pv: Vec<Move>,
    pub time_ms: u128,
    /// Aggregated search counters (node breakdown, TT hit rate, ordering).
    pub telemetry: SearchTelemetry,
}

impl SearchResult {
    /// Render the score as either a centipawn value or `#N` mate distance.
    pub fn score_string(&self) -> String {
        if self.score >= MATE_THRESHOLD {
            format!("#{}", (MATE - self.score + 1) / 2)
        } else if self.score <= -MATE_THRESHOLD {
            format!("#-{}", (MATE + self.score + 1) / 2)
        } else {
            format!("{:+.2}", self.score as f64 / 100.0)
        }
    }
}

/// Reusable search engine. Holds the transposition table (persisted across
/// moves), killer/history tables, and per-search bookkeeping.
pub struct Searcher {
    tt: TranspositionTable,
    killers: [[Option<Move>; 2]; MAX_PLY],
    history: Vec<i32>, // 128*128 from->to, indexed by raw 0x88 squares
    nodes: u64,
    qnodes: u64,
    beta_cutoffs: u64,
    first_move_cutoffs: u64,
    start: Instant,
    deadline: Option<Instant>,
    stop: bool,
    pv_table: [[Move; MAX_PLY]; MAX_PLY],
    pv_len: [usize; MAX_PLY],
}

impl Default for Searcher {
    fn default() -> Self {
        Self::new(64)
    }
}

impl Searcher {
    /// Create a searcher with a transposition table of roughly `tt_mb`
    /// megabytes.
    pub fn new(tt_mb: usize) -> Self {
        Searcher {
            tt: TranspositionTable::new_strict_size_mb(tt_mb.max(1)),
            killers: [[None; 2]; MAX_PLY],
            history: vec![0; 128 * 128],
            nodes: 0,
            qnodes: 0,
            beta_cutoffs: 0,
            first_move_cutoffs: 0,
            start: Instant::now(),
            deadline: None,
            stop: false,
            pv_table: [[Move::new(0, 0); MAX_PLY]; MAX_PLY],
            pv_len: [0; MAX_PLY],
        }
    }

    /// Clear all learned state (TT, killers, history). Useful between
    /// independent games.
    pub fn reset(&mut self) {
        self.tt.clear();
        self.killers = [[None; 2]; MAX_PLY];
        for h in self.history.iter_mut() {
            *h = 0;
        }
    }

    /// Run an iterative-deepening search and return the best line found.
    pub fn think(&mut self, board: &mut Board, limits: SearchLimits) -> SearchResult {
        self.nodes = 0;
        self.qnodes = 0;
        self.beta_cutoffs = 0;
        self.first_move_cutoffs = 0;
        let tt_probes0 = self.tt.probes;
        let tt_hits0 = self.tt.hits;
        self.stop = false;
        self.start = Instant::now();
        self.deadline = limits.movetime_ms.map(|ms| self.start + Duration::from_millis(ms));
        self.tt.new_search();
        // Decay the history table so old games/moves do not dominate ordering.
        for h in self.history.iter_mut() {
            *h /= 2;
        }

        let mut result = SearchResult::default();
        let mut prev_score = 0;

        for depth in 1..=limits.max_depth {
            self.pv_len = [0; MAX_PLY];

            // Aspiration window around the previous score for depth >= 4.
            let (mut alpha, mut beta) = if depth >= 4 {
                (prev_score - 50, prev_score + 50)
            } else {
                (-INF, INF)
            };

            let score = loop {
                let s = self.negamax(board, depth, 0, alpha, beta, true);
                if self.stop {
                    break s;
                }
                if s <= alpha {
                    alpha = (alpha - 200).max(-INF); // fail low: widen down
                } else if s >= beta {
                    beta = (beta + 200).min(INF); // fail high: widen up
                } else {
                    break s;
                }
                // On a full re-open, drop to an infinite window.
                if alpha <= -INF + 1 && beta >= INF - 1 {
                    break self.negamax(board, depth, 0, -INF, INF, true);
                }
            };

            if self.stop && result.best_move.is_some() {
                // Discard a partial, possibly-wrong result from an aborted depth.
                break;
            }

            prev_score = score;
            let pv = self.collect_pv();
            let elapsed = self.start.elapsed().as_millis();
            result = SearchResult {
                best_move: pv.first().copied(),
                score,
                depth,
                nodes: self.nodes,
                pv: pv.clone(),
                time_ms: elapsed,
                telemetry: SearchTelemetry {
                    nodes: self.nodes,
                    qnodes: self.qnodes,
                    tt_hits: self.tt.hits - tt_hits0,
                    tt_probes: self.tt.probes - tt_probes0,
                    beta_cutoffs: self.beta_cutoffs,
                    first_move_cutoffs: self.first_move_cutoffs,
                    elapsed_ms: elapsed,
                },
            };

            if limits.verbose {
                println!(
                    "info depth {} score {} nodes {} time {} pv {}",
                    depth,
                    result.score_string(),
                    result.nodes,
                    result.time_ms,
                    pv.iter().map(|m| m.to_uci()).collect::<Vec<_>>().join(" "),
                );
            }

            // Stop early on a proven mate or when out of time.
            if score.abs() >= MATE_THRESHOLD {
                break;
            }
            if self.time_up() {
                break;
            }
        }

        result
    }

    #[inline]
    fn time_up(&self) -> bool {
        match self.deadline {
            Some(d) => Instant::now() >= d,
            None => false,
        }
    }

    /// Periodically poll the clock (cheap: only every 2048 nodes).
    #[inline]
    fn check_time(&mut self) {
        if self.nodes & 2047 == 0 && self.time_up() {
            self.stop = true;
        }
    }

    fn negamax(
        &mut self,
        board: &mut Board,
        depth: i32,
        ply: usize,
        mut alpha: i32,
        beta: i32,
        do_null: bool,
    ) -> i32 {
        self.pv_len[ply] = ply;
        self.nodes += 1;
        self.check_time();
        if self.stop {
            return 0;
        }

        let in_check = crate::attacks::in_check(board, board.side_to_move());

        // Draw detection (not at the root).
        if ply > 0 && (board.repetition_count() >= 2
            || board.is_fifty_move_draw()
            || board.is_insufficient_material())
        {
            return 0;
        }

        // Check extension: never drop into quiescence while in check.
        let depth = if in_check && depth < MAX_PLY as i32 - 1 {
            depth + 1
        } else {
            depth
        };

        if depth <= 0 {
            return self.quiescence(board, alpha, beta, ply);
        }

        let is_pv = beta - alpha > 1;

        // --- Transposition table probe ---
        let mut tt_move: Option<Move> = None;
        match self.tt.probe(board.key, depth, alpha, beta) {
            ProbeResult::Usable(score, _best) => {
                if !is_pv {
                    return from_tt(score, ply);
                }
            }
            ProbeResult::Found(entry) => {
                tt_move = unpack(entry.best);
            }
            ProbeResult::Miss => {}
        }

        // --- Null-move pruning ---
        // Give the opponent a free move; if our position is still so good that
        // it fails high, this node is almost certainly a cutoff. Skipped in PV
        // nodes, in check, and in pawn-only endgames (zugzwang risk).
        if do_null && !is_pv && depth >= 3 && !in_check && has_non_pawn_material(board) {
            let r = 2 + depth / 4;
            let saved_ep = board.ep;
            let saved_key = board.key;
            let k = keys();
            if let Some(ep) = board.ep {
                board.key ^= k.ep_file[square::file(ep) as usize];
            }
            board.ep = None;
            board.side_white = !board.side_white;
            board.key ^= k.side;

            let score = -self.negamax(board, depth - 1 - r, ply + 1, -beta, -beta + 1, false);

            board.side_white = !board.side_white;
            board.ep = saved_ep;
            board.key = saved_key;

            if self.stop {
                return 0;
            }
            if score >= beta && score.abs() < MATE_THRESHOLD {
                return beta;
            }
        }

        // --- Generate and order moves ---
        let mut moves: MoveList = Vec::with_capacity(48);
        legal_moves(board, &mut moves);

        if moves.is_empty() {
            // Checkmate (prefer faster mates) or stalemate.
            return if in_check { -MATE + ply as i32 } else { 0 };
        }

        self.order_moves(board, &mut moves, tt_move, ply);

        let mut best = -INF;
        let mut best_move = moves[0];
        let orig_alpha = alpha;
        let mut searched = 0;

        for mv in moves {
            let is_capture = !board.cells[mv.to].is_empty() || mv.is_promotion();
            board.make_move_struct(mv);

            let gives_check = crate::attacks::in_check(board, board.side_to_move());

            let mut score;
            if searched == 0 {
                // Full-window search of the first (best-ordered) move.
                score = -self.negamax(board, depth - 1, ply + 1, -beta, -alpha, true);
            } else {
                // Late-move reductions for quiet, late moves.
                let mut reduction = 0;
                if depth >= 3 && !is_capture && !in_check && !gives_check && searched >= 4 {
                    reduction = 1 + (searched >= 8) as i32;
                }
                // Zero-window scout, reduced.
                score = -self.negamax(
                    board,
                    depth - 1 - reduction,
                    ply + 1,
                    -alpha - 1,
                    -alpha,
                    true,
                );
                // Re-search at full depth/window if it looks promising.
                if score > alpha && (reduction > 0 || score < beta) {
                    score = -self.negamax(board, depth - 1, ply + 1, -beta, -alpha, true);
                }
            }

            board.unmake_move();
            searched += 1;

            if self.stop {
                return 0;
            }

            if score > best {
                best = score;
                best_move = mv;
                if score > alpha {
                    alpha = score;
                    self.update_pv(ply, mv);
                }
            }

            if alpha >= beta {
                self.beta_cutoffs += 1;
                if searched == 1 {
                    self.first_move_cutoffs += 1;
                }
                // Beta cutoff: record killers / history for quiet moves.
                if !is_capture {
                    self.record_killer(ply, mv);
                    self.history[hist_idx(mv)] += depth * depth;
                }
                break;
            }
        }

        // --- Store in TT with mate-distance correction ---
        let node_type = if best <= orig_alpha {
            NodeType::UpperBound
        } else if best >= beta {
            NodeType::LowerBound
        } else {
            NodeType::Exact
        };
        self.tt.store(
            board.key,
            depth,
            to_tt(best, ply),
            node_type,
            Some(pack(best_move)),
        );

        best
    }

    fn quiescence(&mut self, board: &mut Board, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        self.nodes += 1;
        self.qnodes += 1;
        self.check_time();
        if self.stop {
            return 0;
        }

        let stand = evaluate(board);
        if stand >= beta {
            return beta;
        }
        if stand > alpha {
            alpha = stand;
        }
        if ply + 1 >= MAX_PLY {
            return alpha;
        }

        let mut caps: MoveList = Vec::with_capacity(16);
        gen_captures(board, &mut caps);
        self.order_captures(board, &mut caps);

        for mv in caps {
            // Prune captures that lose material outright (SEE < 0). Promotions
            // are never pruned (their gain is not captured by SEE alone).
            if !mv.is_promotion()
                && !board.cells[mv.to].is_empty()
                && crate::see::see(board, mv) < 0
            {
                continue;
            }
            board.make_move_struct(mv);
            let score = -self.quiescence(board, -beta, -alpha, ply + 1);
            board.unmake_move();
            if self.stop {
                return 0;
            }
            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }
        alpha
    }

    // --- Move ordering ------------------------------------------------------

    fn order_moves(&self, board: &Board, moves: &mut MoveList, tt_move: Option<Move>, ply: usize) {
        let killers = self.killers[ply];
        moves.sort_by_key(|m| {
            // Lower key sorts first, so negate "goodness".
            if Some(*m) == tt_move {
                return -1_000_000;
            }
            let victim = board.cells[m.to];
            if !victim.is_empty() || m.is_promotion() {
                return -(500_000 + mvv_lva(board, *m));
            }
            if Some(*m) == killers[0] {
                return -400_000;
            }
            if Some(*m) == killers[1] {
                return -399_000;
            }
            -self.history[hist_idx(*m)]
        });
    }

    fn order_captures(&self, board: &Board, moves: &mut MoveList) {
        // Order by static exchange evaluation (best gains first); promotions
        // and equal/winning captures float to the top.
        moves.sort_by_key(|m| {
            let see = if m.is_promotion() {
                900
            } else {
                crate::see::see(board, *m)
            };
            -see
        });
    }

    fn record_killer(&mut self, ply: usize, mv: Move) {
        if self.killers[ply][0] != Some(mv) {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = Some(mv);
        }
    }

    // --- Principal variation ------------------------------------------------

    fn update_pv(&mut self, ply: usize, mv: Move) {
        self.pv_table[ply][ply] = mv;
        let child_len = self.pv_len[ply + 1];
        for next in (ply + 1)..child_len {
            self.pv_table[ply][next] = self.pv_table[ply + 1][next];
        }
        self.pv_len[ply] = child_len.max(ply + 1);
    }

    fn collect_pv(&self) -> Vec<Move> {
        let len = self.pv_len[0];
        (0..len).map(|i| self.pv_table[0][i]).collect()
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

/// MVV-LVA: prefer capturing valuable victims with cheap attackers. Promotions
/// get a large bonus.
fn mvv_lva(board: &Board, m: Move) -> i32 {
    let attacker = board.cells[m.from].kind().map(piece_value).unwrap_or(0);
    let victim = board.cells[m.to].kind().map(piece_value).unwrap_or(0);
    let promo = m
        .promotion
        .and_then(|p| p.kind())
        .map(piece_value)
        .unwrap_or(0);
    victim * 16 - attacker + promo
}

#[inline]
fn piece_value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Pawn => 100,
        PieceKind::Knight => 320,
        PieceKind::Bishop => 330,
        PieceKind::Rook => 500,
        PieceKind::Queen => 900,
        PieceKind::King => 20000,
    }
}

#[inline]
fn hist_idx(m: Move) -> usize {
    m.from * 128 + m.to
}

/// `true` if the side to move has a piece other than pawns and the king (used
/// to gate null-move pruning, which is unsafe in pawn endgames).
fn has_non_pawn_material(board: &Board) -> bool {
    let color = board.side_to_move();
    for r in 0..8 {
        for f in 0..8 {
            let p = board.cells[square::make(r, f)];
            if p.is_color(color) {
                match p.kind() {
                    Some(PieceKind::Knight)
                    | Some(PieceKind::Bishop)
                    | Some(PieceKind::Rook)
                    | Some(PieceKind::Queen) => return true,
                    _ => {}
                }
            }
        }
    }
    false
}

fn promo_id(p: Option<Piece>) -> u8 {
    match p.and_then(|p| p.kind()) {
        Some(PieceKind::Queen) => 1,
        Some(PieceKind::Rook) => 2,
        Some(PieceKind::Bishop) => 3,
        Some(PieceKind::Knight) => 4,
        _ => 0,
    }
}

fn promo_from_id(id: u8) -> Option<Piece> {
    match id {
        1 => Some(Piece::WQ),
        2 => Some(Piece::WR),
        3 => Some(Piece::WB),
        4 => Some(Piece::WN),
        _ => None,
    }
}

fn pack(m: Move) -> (usize, usize, u8) {
    (m.from, m.to, promo_id(m.promotion))
}

fn unpack(pm: PackedMove) -> Option<Move> {
    if pm == PackedMove::none() {
        return None;
    }
    let (from, to, id) = pm.unpack();
    Some(Move {
        from,
        to,
        promotion: promo_from_id(id),
    })
}

/// Adjust a mate score for storage in the TT (encode distance from this node).
#[inline]
fn to_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_THRESHOLD {
        score + ply as i32
    } else if score <= -MATE_THRESHOLD {
        score - ply as i32
    } else {
        score
    }
}

/// Reverse [`to_tt`] when reading a score back from the TT.
#[inline]
fn from_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_THRESHOLD {
        score - ply as i32
    } else if score <= -MATE_THRESHOLD {
        score + ply as i32
    } else {
        score
    }
}

// ---------------------------------------------------------------------------
// Compatibility / convenience entry points
// ---------------------------------------------------------------------------

/// Find the best move for the side to move. Compatible with the original
/// engine's `ai_move(board, depth, time_ms)` signature used by the UI.
///
/// # Examples
///
/// ```
/// use rust_chess_engine::board::Board;
/// use rust_chess_engine::search::ai_move;
///
/// // White mates in one with a back-rank rook check.
/// let mut board = Board::from_fen("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1").unwrap();
/// let best = ai_move(&mut board, 4, None).unwrap();
/// assert_eq!(best.to_uci(), "a1a8");
/// ```
pub fn ai_move(board: &mut Board, depth: i32, time_ms: Option<u64>) -> Option<Move> {
    let mut searcher = Searcher::new(64);
    let limits = SearchLimits {
        max_depth: depth.max(1),
        movetime_ms: time_ms,
        verbose: false,
    };
    searcher.think(board, limits).best_move
}

/// Run a search and return the full result (move, score, PV, stats).
pub fn search(board: &mut Board, limits: SearchLimits) -> SearchResult {
    let mut searcher = Searcher::new(64);
    searcher.think(board, limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_mate_in_one() {
        // White: Qh5 ... actually use a clean back-rank mate in one.
        // Black king h8, white rook a1 -> a8 is mate.
        let mut b = Board::from_fen("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1").unwrap();
        let res = search(&mut b, SearchLimits::depth(4));
        assert_eq!(res.best_move.map(|m| m.to_uci()), Some("a1a8".to_string()));
        assert!(res.score >= MATE_THRESHOLD);
    }

    #[test]
    fn captures_free_queen() {
        // Black queen hangs on d5; white knight on c3 ... use a simple hang.
        let mut b = Board::from_fen("4k3/8/8/3q4/8/8/3R4/4K3 w - - 0 1").unwrap();
        let res = search(&mut b, SearchLimits::depth(4));
        // Best is to take the queen with the rook.
        assert_eq!(res.best_move.map(|m| m.to), Some(square::from_alg("d5").unwrap()));
    }

    #[test]
    fn search_is_stable_and_legal() {
        let mut b = Board::startpos();
        let res = search(&mut b, SearchLimits::depth(5));
        assert!(res.best_move.is_some());
        // The returned best move must be legal.
        let mut legal = Vec::new();
        legal_moves(&mut b, &mut legal);
        assert!(legal.contains(&res.best_move.unwrap()));
    }
}
