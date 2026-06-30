# Rust Chess Engine

A complete, **fully hand-written** chess engine in safe Rust (no `unsafe`, no
chess-logic crates). Move generation is **perft-verified** against the canonical
reference node counts, the search is a modern iterative-deepening PVS, and the
whole thing ships with a UCI interface, PGN/SAN support, an opening book, an
interactive terminal UI, observability hooks, and an extensive test suite.

```
   ██████╗ ██╗   ██╗███████╗████████╗     ██████╗██╗  ██╗███████╗███████╗███████╗
   ██╔══██╗██║   ██║██╔════╝╚══██╔══╝    ██╔════╝██║  ██║██╔════╝██╔════╝██╔════╝
   ██████╔╝██║   ██║███████╗   ██║       ██║     ███████║█████╗  ███████╗███████╗
   ██╔══██╗██║   ██║╚════██║   ██║       ██║     ██╔══██║██╔══╝  ╚════██║╚════██║
   ██║  ██║╚██████╔╝███████║   ██║       ╚██████╗██║  ██║███████╗███████║███████║
   ╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝        ╚═════╝╚═╝  ╚═╝╚══════╝╚══════╝╚══════╝
```

---

## Table of contents

- [Feature overview](#feature-overview)
- [Architecture](#architecture)
- [Build & run](#build--run)
- [Usage modes](#usage-modes)
- [Correctness: perft](#correctness-perft)
- [Testing](#testing)
- [Observability](#observability)
- [Module guide](#module-guide)

---

## Feature overview

| Area | What's implemented |
| --- | --- |
| **Board** | `0x88` mailbox board, FEN parse/serialize (validated, never panics), incremental Zobrist key, color-mirror utility |
| **Moves** | Fully **legal** move generation, special moves (castling with through-check rules, en passant incl. pin edge cases, under-promotions), make/**unmake** with full reversibility |
| **Search** | Iterative deepening, Principal Variation Search, transposition table, quiescence with SEE pruning, null-move pruning, late-move reductions, killer + history move ordering, aspiration windows, mate-distance scoring, time management |
| **Evaluation** | Tapered (mid/endgame) **PeSTO** piece-square tables, material, bishop pair, doubled & passed pawns, mobility, king pawn-shield, rook on open/semi-open file, **proven color-symmetric** by test |
| **Draws** | Threefold repetition, fifty-move rule, insufficient material |
| **Notation / I/O** | SAN render & parse, PGN import/export (comments/NAGs/variations tolerated), **UCI** protocol, small opening book |
| **App** | Interactive colored terminal UI, login/registration, Human-vs-Engine / Human-vs-Human / Engine-vs-Engine / Analysis modes, hints, undo/redo, PGN save/load, perft & eval commands |
| **Quality** | 100+ unit/integration tests, perft suite (to 119M+ nodes), tactical suite, random-game invariant fuzzing, criterion benchmarks, leveled logging + search telemetry |

## Architecture

The engine is a layered stack of small, focused modules. Each layer depends only
on the layers beneath it, which keeps the move generator independent of the
search, the search independent of the protocols, and so on.

![Architecture](assets/architecture.svg)

<sub>Diagram source: [`assets/architecture.excalidraw`](assets/architecture.excalidraw).</sub>

### Search pipeline (per move)

![Search pipeline](assets/search-pipeline.svg)

<sub>Diagram source: [`assets/search-pipeline.excalidraw`](assets/search-pipeline.excalidraw).</sub>

A deeper write-up (including the make/unmake + incremental-Zobrist scheme and
the perft methodology) lives in [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Build & run

Requires a recent stable Rust toolchain (edition 2024).

```bash
git clone <repo-url>
cd chess-engine
cargo build --release
```

## Usage modes

The single binary dispatches on its first argument:

```bash
# Interactive terminal app (login, menus, colored board, game modes)
cargo run --release

# UCI engine: point Arena / Cute Chess / Banksia at the built binary, or:
cargo run --release -- uci

# Perft "divide" for a position (defaults to the start position)
cargo run --release -- perft 5
cargo run --release -- perft 4 "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"

# Fixed-depth search benchmark with telemetry
cargo run --release -- bench 9
```

Inside the interactive app you can type moves in either **UCI** (`e2e4`,
`e7e8q`) or **SAN** (`Nf3`, `O-O`, `exd5`), plus commands like `undo`, `redo`,
`hint`, `analyze`, `eval`, `fen`, `flip`, `save`, and `perft`.

## Correctness: perft

The single strongest correctness guarantee is **perft**, counting the exact
number of legal-move leaf nodes at a given depth and comparing against published
reference values. The suite verifies the six canonical positions; the deep
checks reach over 100 million nodes:

| Position | Depth | Nodes |
| --- | --- | --- |
| Start position | 6 | 119,060,324 |
| Kiwipete | 5 | 193,690,690 |
| Endgame (pos 3) | 6 | 11,030,083 |
| Middlegame (pos 4) | 5 | 15,833,292 |

```bash
cargo test                          # fast suite (shallow perft + everything else)
cargo test --release -- --ignored   # deep perft (100M+ nodes), runs in seconds
```

## Testing

```bash
cargo test                          # all unit + integration tests
cargo test --release -- --ignored   # deep perft
cargo bench                         # criterion micro-benchmarks
```

The suite includes: per-module unit tests, the perft suite, a **tactical**
regression suite (mate-in-1/2, winning material, avoiding losing captures,
promotion), random-game **invariant fuzzing** (incremental-key integrity,
make/unmake reversibility, legality), an **evaluation color-symmetry** proof,
and chess-**rules** edge cases (en-passant timing & pins, castling-right loss,
under-promotion, the draw rules, FEN robustness).

## Observability

* Set `RUSTCHESS_LOG=off|error|warn|info|debug|trace` to control leveled logging
  (written to stderr so it never corrupts UCI stdout).
* Every search returns [`SearchTelemetry`](src/log.rs): node/qnode counts,
  nodes-per-second, TT hit rate, and move-ordering quality (the fraction of beta
  cutoffs that occur on the first move). `bench` prints this summary.
* UCI emits standard `info depth … score … nodes … pv …` lines.

## Module guide

| Module | Responsibility |
| --- | --- |
| `types` | Colors, pieces, `0x88` square helpers |
| `moves` | The `Move` value type + UCI parsing |
| `board` | Position state, FEN, make/unmake, draw detection, mirror |
| `attacks` | Square-attack & check detection |
| `movegen` | Pseudo-legal + fully legal move generation |
| `perft` | Move-generation correctness oracle |
| `eval` | Tapered PeSTO evaluation + positional terms |
| `see` | Static exchange evaluation |
| `search` | Iterative-deepening PVS, TT, pruning, ordering |
| `transposition`, `zobrist` | Hashing infrastructure |
| `san`, `pgn` | Notation rendering & parsing |
| `uci` | UCI protocol front end |
| `book`, `timeman` | Opening book & time management |
| `game` | High-level game API (play/undo/redo/status/PGN) |
| `log` | Leveled logging + search telemetry |
| `engine` | Stable facade re-exporting the public surface |
| `ui`, `auth` | Interactive terminal application |

---
