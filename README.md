# Battleship Simulator Engine & Tactical Lab

> **Disclaimer:** This is an independent personal project and is not affiliated with, endorsed by, or connected to my employer in any way.

A high-performance, deterministic Battleship simulation engine, CLI, and HTTP server written in **pure Rust standard library (`std` only, zero external crate dependencies)**.

Simulates games conforming strictly to standard Hasbro Battleship rules and provides deep statistical benchmarking across **11 distinct attack strategies**, including a complete **11x11 All-Pairs Head-to-Head Tournament Matrix** and overall standings.

---

## Highlights

- **Standard Hasbro Ruleset**: 10×10 ocean grid (`WIDTH = 10`, `HEIGHT = 10`), 5 ships (`Carrier` 5, `Battleship` 4, `Cruiser` 3, `Submarine` 3, `Destroyer` 2 — total 17 cells), orthogonal placements, non-overlapping constraints, and explicit shot outcomes (`Miss`, `Hit`, `Sunk(ShipKind)`).
- **Fast Deterministic PRNG**: `Xoshiro256**` algorithm seeded via `SplitMix64` with unbiased Lemire integer range generation. 100% reproducible simulations given any 64-bit seed.
- **11 Advanced Attack Strategies**:
  1. `random_hunt`: **Random + Hunt/Target** — uniform random hunt mode with smart collinear and orthogonal hunt-down targeting.
  2. `checkerboard`: **Checkerboard + Hunt/Target** — parity-restricted hunt mode (`(row + col) % 2 == parity`) leveraging the mathematical certainty that any ship of length $\ge 2$ intersects the parity grid.
  3. `largest_rect`: **Largest Empty Rect Midpoint** — 2D prefix-sum algorithm finding the largest all-unshot axis-aligned rectangle (maximizing area, breaking ties by `min(height, width)`), firing at its midpoint.
  4. `adaptive_stride`: **Adaptive Stride (2->3->4->5)** — dynamically adapts the hunt stride to the minimum remaining unsunk ship length $k$ (starts at 2 while Destroyer is alive, stepping up to 3, 4, 5 as small ships sink).
  5. `stride4_to_2`: **Staggered Stride (4->2)** — searches on a staggered 4-stride grid while Carrier (5) or Battleship (4) are afloat, then drops seamlessly to 2-stride checkerboard.
  6. `rect_stride4`: **Largest Rect + Stride (4->2)** — combines the largest empty rectangle center with the active 4-to-2 stride, targeting the center of the largest unexplored territory.
  7. `stride_5432`: **Descending Stride (5->4->3->2)** — active stride $s$ begins at the maximum remaining ship length (5 for Carrier, stepping down to 4, 3, 2 as capital ships are eliminated).
  8. `rect_stride_5432`: **Largest Rect + Stride (5->4->3->2)** — combines the largest empty rectangle center with the active descending stride (5→4→3→2).
  9. `rect_checkerboard`: **Largest Rect + Checkerboard** — targets the checkerboard cell closest to the center of the largest unexplored rectangle.
  10. `monte_carlo`: **Monte Carlo Sampling** — simulates valid fleet configurations consistent with observed shots, accumulating a probability density heatmap to select optimal shots.
  11. `bayesian_pdf`: **Bayesian Probability Density** — calculates the exact combinatorial probability density across all remaining unsunk ship placements with exponential target-mode likelihood weighting ($1000^H$).
- **Smart Collinear Targeting**: Automatically identifies 2+ collinear active hits, prioritizes unshot line endpoints, falls back to orthogonal neighbors (`N, S, E, W`), and resolves multi-ship contact upon sinking.
- **11x11 Tournament Engine**: Runs an all-pairs round-robin tournament across all 11 strategies with alternating first-mover turns to eliminate starting-player bias, producing an 11x11 win rate matrix and overall standings.
- **Embedded HTTP Server & REST API**: Pure `std::net::TcpListener` server hosting static assets from `viewer/` and exposing JSON API endpoints (`/api/match` and `/api/benchmark`).
- **Zero External Dependencies**: 100% pure Rust standard library.

---

## Architecture & Code Structure

```
.
├── Cargo.toml                 # Package definition (0 external dependencies)
├── src/
│   ├── lib.rs                 # Library exports & public API re-exports
│   ├── main.rs                # CLI entry point (simulate, match, serve)
│   ├── rng.rs                 # Deterministic PRNG (Xoshiro256** + SplitMix64)
│   ├── game.rs                # Hasbro rules, Board, TrackingBoard, ShipKind, MoveRecord
│   ├── strategy.rs            # 11 AttackStrategy implementations, DefenseStrategy, TargetTracker
│   ├── sim.rs                 # 11-way benchmarks, 11x11 tournament matrix, JSON serializers
│   └── server.rs              # std::net HTTP server & REST API router
├── viewer/                    # Interactive Tactical Command Center Web UI
│   ├── index.html             # Web viewer interface
│   ├── styles.css             # Tactical cyber command styling
│   ├── app.js                 # Dual-mode viewer engine
│   └── sample_data.js         # Precomputed benchmark & match replays
└── README.md                  # This file
```

---

## The 11 Attack Strategies Explained

### 1. `Random + Hunt/Target` (`random_hunt`)
- **Hunt**: Uniformly samples from all unshot cells.
- **Target**: Once a hit occurs, switches to smart collinear and orthogonal hunt-down.

### 2. `Checkerboard + Hunt/Target` (`checkerboard`)
- **Hunt**: Fires exclusively at cells matching $(r + c) \pmod 2 = \text{parity}$.
- **Theory**: The smallest ship is the Destroyer (length 2). Every ship of length $\ge 2$ intersects the checkerboard parity grid, halving the hunt search space from 100 to 50 cells.

### 3. `Largest Empty Rect Midpoint` (`largest_rect`)
- **Hunt**: Computes a 2D prefix-sum of shot cells to find the largest axis-aligned rectangle $(r_0..=r_1, c_0..=c_1)$ consisting entirely of unshot cells. Breaks ties by preferring square-like rectangles ($\max(\min(H, W))$). Fires at the midpoint $((r_0 + r_1)/2, (c_0 + c_1)/2)$.
- **Theory**: The center of the largest unexplored region is statistically the most likely to contain undiscovered ship segments.

### 4. `Adaptive Stride (2->3->4->5)` (`adaptive_stride`)
- **Hunt**: Observes $k = \text{min\_remaining\_ship\_len()}$. While Destroyer is unsunk, $k=2$ (fires on stride 2). Once Destroyer sinks, $k=3$ (fires on stride 3). When Cruiser/Submarine sink, $k=4$, and finally $k=5$ when only Carrier remains.
- **Theory**: Adapts sampling density dynamically as small ships are eliminated.

### 5. `Staggered Stride (4->2)` (`stride4_to_2`)
- **Hunt**: While Carrier (5) or Battleship (4) are afloat, fires on a staggered 4-stride grid $(r + c) \pmod 4 = \text{offset}_4$. Once both 4 and 5 are sunk, tightens to a 2-stride checkerboard $(r + c) \pmod 2 = (\text{offset}_4 \pmod 2)$.
- **Theory**: All 4-stride shots are naturally aligned with the 2-stride checkerboard, wasting zero shots while discovering capital ships early.

### 6. `Largest Rect + Stride (4->2)` (`rect_stride4`)
- **Hunt**: Combines the geometric discovery of the largest empty rectangle with the active 4-to-2 stride. Locates the largest unshot rectangle and fires at the unshot stride cell closest to its center.

### 7. `Descending Stride (5->4->3->2)` (`stride_5432`)
- **Hunt**: Starts with stride 5 while Carrier is afloat, dropping to 4 (Battleship), 3 (Cruiser/Submarine), and 2 (Destroyer) as ships sink.
- **Theory**: Exploits the pigeonhole principle to maximize probing efficiency early in the match.

### 8. `Largest Rect + Stride (5->4->3->2)` (`rect_stride_5432`)
- **Hunt**: Combines the largest empty rectangle center with the descending stride (5→4→3→2). Targets the center of largest unexplored territory matching the current stride.

### 9. `Largest Rect + Checkerboard` (`rect_checkerboard`)
- **Hunt**: Probes checkerboard parity cells located closest to the midpoint of the largest empty rectangle.
- **Theory**: Guarantees parity intersection with all ships while keeping probes centered in deep unexplored regions.

### 10. `Monte Carlo Sampling` (`monte_carlo`)
- **Hunt & Target**: Runs Monte Carlo simulations of legal fleet configurations consistent with observed misses, hits, and sunk ships. Accumulates placement frequency across all unshot cells and fires at the cell with highest frequency.

### 11. `Bayesian Probability Density` (`bayesian_pdf`)
- **Hunt & Target**: Computes the analytical marginal probability density for every unshot cell across all remaining unsunk ships. Placements overlapping active unsunk hits receive exponential likelihood weight ($1000^H$), causing collinear extensions to dominate orthogonal probes naturally.

---

## Benchmark & Tournament Results

```
=========================================================================================================
                                   BATTLESHIP ENGINE BENCHMARK REPORT                                    
=========================================================================================================
Games simulated: 100 solo per strategy | Tournament: 100 duels per pair | Base Seed: 42

STRATEGY PERFORMANCE (Turns to sink all 17 ship cells):
+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+
| Strategy                          | Mean  | Median | Min | Max | StDev | P10 | P90 |
+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+
| Random + Hunt/Target              |  59.9 |   59.0 |  27 |  98 | 15.67 |  40 |  82 |
| Checkerboard + Hunt/Target        |  51.6 |   53.0 |  32 |  82 |  9.89 |  39 |  62 |
| Largest Empty Rect Midpoint       |  53.6 |   55.0 |  25 |  93 | 12.83 |  36 |  72 |
| Adaptive Stride (2->3->4->5)      |  52.3 |   53.0 |  29 |  74 |  9.27 |  40 |  65 |
| Staggered Stride (4->2)           |  51.3 |   51.0 |  30 | 100 | 11.08 |  36 |  63 |
| Largest Rect + Stride (4->2)      |  48.0 |   48.0 |  27 |  90 |  9.65 |  35 |  60 |
| Descending Stride (5->4->3->2)    |  51.9 |   50.0 |  27 |  95 | 10.85 |  39 |  67 |
| Largest Rect + Stride (5->4->3->2) |  48.4 |   47.0 |  30 |  70 |  9.09 |  37 |  61 |
| Largest Rect + Checkerboard       |  47.3 |   46.5 |  27 |  62 |  7.87 |  39 |  58 |
| Monte Carlo Sampling              |  47.4 |   45.0 |  24 |  91 | 11.05 |  35 |  62 |
| Bayesian Probability Density      |  45.3 |   45.5 |  29 |  72 |  8.76 |  35 |  57 |
+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+

=========================================================================================================
                                11x11 HEAD-TO-HEAD WIN RATE MATRIX (%)                                   
=========================================================================================================
Columns indicate opponent. Value is row strategy's win % against column.
  [ 1] Random + Hunt/Target             [ 2] Checkerboard + Hunt/Target
  [ 3] Largest Empty Rect Midpoint      [ 4] Adaptive Stride (2->3->4->5)
  [ 5] Staggered Stride (4->2)          [ 6] Largest Rect + Stride (4->2)
  [ 7] Descending Stride (5->4->3->2)   [ 8] Largest Rect + Stride (5->4->3->2)
  [ 9] Largest Rect + Checkerboard      [10] Monte Carlo Sampling
  [11] Bayesian Probability Density     

+-----------------------------------+------+------+------+------+------+------+------+------+------+------+------+
| Strategy                          |  [1] |  [2] |  [3] |  [4] |  [5] |  [6] |  [7] |  [8] |  [9] | [10] | [11] |
+-----------------------------------+------+------+------+------+------+------+------+------+------+------+------+
| [ 1] Random + Hunt/Target          |  --- |   31%|   33%|   33%|   26%|   19%|   21%|   25%|   26%|   27%|   14%|
| [ 2] Checkerboard + Hunt/Target    |   69%|  --- |   53%|   45%|   50%|   44%|   42%|   41%|   36%|   42%|   29%|
| [ 3] Largest Empty Rect Midpoint   |   67%|   47%|  --- |   44%|   44%|   33%|   47%|   38%|   33%|   37%|   28%|
| [ 4] Adaptive Stride (2->3->4->5)  |   67%|   55%|   56%|  --- |   48%|   39%|   53%|   45%|   47%|   33%|   28%|
| [ 5] Staggered Stride (4->2)       |   74%|   50%|   56%|   52%|  --- |   38%|   49%|   41%|   44%|   40%|   41%|
| [ 6] Largest Rect + Stride (4->2)  |   81%|   56%|   67%|   61%|   62%|  --- |   57%|   52%|   54%|   47%|   36%|
| [ 7] Descending Stride (5->4->3->2) |   79%|   58%|   53%|   47%|   51%|   43%|  --- |   41%|   44%|   42%|   31%|
| [ 8] Largest Rect + Stride (5->4->3->2) |   75%|   59%|   62%|   55%|   59%|   48%|   59%|  --- |   46%|   49%|   37%|
| [ 9] Largest Rect + Checkerboard   |   74%|   64%|   67%|   53%|   56%|   46%|   56%|   54%|  --- |   52%|   42%|
| [10] Monte Carlo Sampling          |   73%|   58%|   63%|   67%|   60%|   53%|   58%|   51%|   48%|  --- |   46%|
| [11] Bayesian Probability Density  |   86%|   71%|   72%|   72%|   59%|   64%|   69%|   63%|   58%|   54%|  --- |
+-----------------------------------+------+------+------+------+------+------+------+------+------+------+------+

=========================================================================================================
                             OVERALL TOURNAMENT STANDINGS (ALL-PAIRS DUELS)                              
=========================================================================================================
+------+-----------------------------------+---------+-------+--------+---------------+
| Rank | Strategy                          | Matches |  Wins | Losses | Overall Win % |
+------+-----------------------------------+---------+-------+--------+---------------+
|    1 | Bayesian Probability Density      |    1000 |   672 |    328 |         67.2% |
|    2 | Largest Rect + Checkerboard       |    1000 |   592 |    408 |         59.2% |
|    3 | Monte Carlo Sampling              |    1000 |   577 |    423 |         57.7% |
|    4 | Largest Rect + Stride (4->2)      |    1000 |   576 |    424 |         57.6% |
|    5 | Largest Rect + Stride (5->4->3->2) |    1000 |   563 |    437 |         56.3% |
|    6 | Staggered Stride (4->2)           |    1000 |   506 |    494 |         50.6% |
|    7 | Descending Stride (5->4->3->2)    |    1000 |   497 |    503 |         49.7% |
|    8 | Adaptive Stride (2->3->4->5)      |    1000 |   477 |    523 |         47.7% |
|    9 | Checkerboard + Hunt/Target        |    1000 |   451 |    549 |         45.1% |
|   10 | Largest Empty Rect Midpoint       |    1000 |   418 |    582 |         41.8% |
|   11 | Random + Hunt/Target              |    1000 |   231 |    769 |         23.1% |
+------+-----------------------------------+---------+-------+--------+---------------+
```

---

## CLI Usage

### 1. Run Strategy Benchmark & Tournament (`simulate`)
Simulates $N$ games (default: 1,000) for all 11 strategies and executes a complete 11x11 round-robin tournament, printing formatted tables, the 11x11 matrix, standings, and ASCII histograms:

```bash
cargo run --release -- simulate --games 1000 --seed 42
```

### 2. Run a Single Match (`match`)
Simulates a single head-to-head match between any pair of strategies and streams the complete JSON replay to stdout:

```bash
cargo run -- match --p1 bayesian_pdf --p2 monte_carlo --seed 123
```

Available strategy IDs:
- `random_hunt`
- `checkerboard`
- `largest_rect`
- `adaptive_stride`
- `stride4_to_2`
- `rect_stride4`
- `stride_5432`
- `rect_stride_5432`
- `rect_checkerboard`
- `monte_carlo`
- `bayesian_pdf`

### 3. Launch HTTP Server & Web Viewer (`serve`)
Starts the built-in HTTP server on `127.0.0.1:8080`:

```bash
cargo run -- serve --port 8080
```

Navigate to `http://127.0.0.1:8080/` in your browser to inspect matches turn-by-turn with full scrubber playback, live board grids, heatmaps, and distribution histograms.

---

## REST API Endpoints

### `GET /api/match`
Generate a fresh head-to-head match replay:
- Query parameters:
  - `p1`: Strategy for Player 1 (default: `random_hunt`)
  - `p2`: Strategy for Player 2 (default: `checkerboard`)
  - `seed`: 64-bit integer seed (optional)
- Response: `application/json` with complete ship placements, turn-by-turn shot coordinates, outcomes, AI modes, and winner.

### `GET /api/benchmark`
Run an on-demand statistical benchmark:
- Query parameters:
  - `games`: Number of games per strategy (default: 200, max: 5000)
  - `seed`: 64-bit integer seed (optional)
- Response: `application/json` with 11-strategy distributions, percentiles, histograms, 11x11 head-to-head tournament matrix, pairings, and standings.

---

## Testing & Quality Assurance

Run the unit test suite:

```bash
cargo test
```

Test coverage includes:
- **`rng::tests`**: Determinism, reproducibility across seeds, uniform bounds checking, Fisher-Yates shuffle validation.
- **`game::tests`**: Hasbro ship length checks, 17 total cells, legal placement, out-of-bounds rejection, overlapping placement rejection, shot damage and sinking detection, tracking board updates.
- **`strategy::tests`**: Fleet placement legality, single-player game completion in $\le 100$ turns for all 11 strategies, smart collinear targeting priority, 2D prefix-sum empty rectangle detection, Bayesian collinear dominance.
- **`sim::tests`**: Head-to-head match completion, JSON serialization integrity, 11x11 tournament matrix and standings verification, statistical proof that Checkerboard outperforms Random Hunt.
- **`server::tests`**: HTTP query string parser, strategy instantiation for all 11 strategy IDs.

---

## Disclaimer

This work is a personal project and is not affiliated with, sponsored by, or endorsed by my employer.
