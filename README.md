# Battleship Simulator Engine & Tactical Lab

> **Disclaimer:** This is an independent personal project and is not affiliated with, endorsed by, or connected to my employer in any way.

A high-performance, deterministic Battleship simulation engine, CLI, and HTTP server written in **pure Rust standard library (`std` only, zero external crate dependencies)**.

Simulates games conforming strictly to standard Hasbro Battleship rules and provides deep statistical benchmarking across **11 distinct attack strategies** and **7 defense strategies**, including:
- **11x11 All-Pairs Attack Tournament Matrix** & standings
- **7 Defenses x 11 Attackers Survival Matrix** (mean turns to defeat fleet)
- **7x7 Defense Head-to-Head Tournament Matrix** & overall defense rankings

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
- **7 Specialized Defense Strategies**:
  1. `random`: **Uniform Random** — uniform random orientation and non-overlapping placement.
  2. `spaced`: **Spaced (No-Touching)** — enforces a 1-cell halo buffer around every ship so no two ships touch orthogonally or diagonally.
  3. `corner_edge`: **Corner & Edge Hugging** — places ships on the outer perimeter (`row 0/9`, `col 0/9`), touching the 4 corners while maintaining gap spacing.
  4. `clustered`: **Tightly Clustered Phalanx** — bunches all ships into a contiguous touching formation, leaving ~75%+ of the grid completely barren.
  5. `anti_midpoint`: **Anti-Midpoint (Bisection Dodger)** — penalizes primary bisection lines (rows/cols 4, 5) and quarter lines (rows/cols 2, 7) to evade midpoint algorithms.
  6. `bait_and_hide`: **Bait Capital + Corner Destroyer** — hides small endgame ships (Destroyer, Submarine) in extreme corners and baits capital ships into interior cells with $\ge 2$-cell buffer against target splash.
  7. `parallel_lanes`: **Parallel Lanes (Single Axis)** — forces the entire fleet into a single shared axis (Horizontal or Vertical), placed in parallel lanes.
- **Dual Round-Robin Tournaments**:
  - **11x11 Attack Tournament**: Pairs all attack algorithms with alternating first-mover advantage.
  - **7x7 Defense Tournament**: Pairs all defenses where both sides face identical attack algorithms.
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
│   ├── strategy.rs            # 11 Attack Strategies, 7 Defense Strategies, TargetTracker
│   ├── sim.rs                 # Dual tournaments (11x11 Attack, 7x7 Defense), 7x11 survival matrix
│   └── server.rs              # std::net HTTP server & REST API router
├── viewer/                    # Interactive Tactical Command Center Web UI
│   ├── index.html             # Web viewer interface
│   ├── styles.css             # Tactical cyber command styling
│   ├── app.js                 # Dual-mode viewer engine
│   └── sample_data.js         # Precomputed benchmark & match replays
└── README.md                  # This file
```

---

## Defense Strategies Explained

### 1. `Uniform Random` (`random`)
- Randomly places ships with uniform orientation and coordinate distribution. Baseline benchmark.

### 2. `Spaced (No-Touching)` (`spaced`)
- Strictly enforces a Chebyshev buffer ($\Delta r \le 1 \land \Delta c \le 1$ forbidden between different ships).
- **Theory**: Eliminates multi-ship collisions during targeting mode; when an enemy hits a ship, their hunt-down probe never accidentally hits a neighboring ship.

### 3. `Corner & Edge Hugging` (`corner_edge`)
- Maximizes ship presence along row 0, row 9, col 0, and col 9, anchoring to `(0,0), (0,9), (9,0), (9,9)` while leaving empty gap cells between ships along the edge.
- **Theory**: Exploits attackers that focus attention on the board center or interior cells.

### 4. `Tightly Clustered Phalanx` (`clustered`)
- Maximizes orthogonal contact between placed ships, bunching all 5 vessels into a dense cluster and leaving 75%+ of the ocean empty.
- **Theory**: Maximizes early-game probe misses, but risks catastrophic cascade once discovered.

### 5. `Anti-Midpoint (Bisection Dodger)` (`anti_midpoint`)
- Adds heavy penalty cost for cells intersecting rows/cols 4 and 5 (primary bisection crosshairs) and moderate penalty for rows/cols 2 and 7 (quarter lines).
- **Theory**: Directly counters geometric empty-rectangle midpoint probing algorithms.

### 6. `Bait Capital + Corner Destroyer` (`bait_and_hide`)
- Places `Destroyer` (len 2) and `Submarine` (len 3) first in extreme outer corners. Then places capital ships (`Carrier`, `Battleship`, `Cruiser`) in the interior ($2..=7$) with a Manhattan distance buffer $\ge 2$ from corner ships.
- **Theory**: The tournament-winning defense! Enemies discover and sink the large interior capital ships quickly, but spend dozens of wasted turns searching empty water for the tiny corner vessels.

### 7. `Parallel Lanes (Single Axis)` (`parallel_lanes`)
- Chooses a single fleet orientation (Horizontal or Vertical) and places all 5 ships in parallel lanes without touching.
- **Theory**: Confounds diagonal stride patterns and collinear orientation assumptions.

---

## Benchmark Results

```
=========================================================================================================
                                   BATTLESHIP ENGINE BENCHMARK REPORT                                    
=========================================================================================================
Games simulated: 100 solo per strategy | Tournament: 100 duels per pair | Base Seed: 42

ATTACK STRATEGY PERFORMANCE (Turns to sink all 17 ship cells):
+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+
| Strategy                          | Mean  | Median | Min | Max | StDev | P10 | P90 |
+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+
| Bayesian Probability Density      |  45.3 |   45.5 |  29 |  72 |  8.76 |  35 |  57 |
| Largest Rect + Checkerboard       |  47.3 |   46.5 |  27 |  62 |  7.87 |  39 |  58 |
| Monte Carlo Sampling              |  47.4 |   45.0 |  24 |  91 | 11.05 |  35 |  62 |
| Largest Rect + Stride (4->2)      |  48.0 |   48.0 |  27 |  90 |  9.65 |  35 |  60 |
| Largest Rect + Stride (5->4->3->2) |  48.4 |   47.0 |  30 |  70 |  9.09 |  37 |  61 |
| Staggered Stride (4->2)           |  51.3 |   51.0 |  30 | 100 | 11.08 |  36 |  63 |
| Checkerboard + Hunt/Target        |  51.6 |   53.0 |  32 |  82 |  9.89 |  39 |  62 |
| Descending Stride (5->4->3->2)    |  51.9 |   50.0 |  27 |  95 | 10.85 |  39 |  67 |
| Adaptive Stride (2->3->4->5)      |  52.3 |   53.0 |  29 |  74 |  9.27 |  40 |  65 |
| Largest Empty Rect Midpoint       |  53.6 |   55.0 |  25 |  93 | 12.83 |  36 |  72 |
| Random + Hunt/Target              |  59.9 |   59.0 |  27 |  98 | 15.67 |  40 |  82 |
+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+

=========================================================================================================
                          OVERALL ATTACK TOURNAMENT STANDINGS (ALL-PAIRS DUELS)                          
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

=========================================================================================================
            7 DEFENSES x 11 ATTACKERS SURVIVAL MATRIX (Mean Turns Survived - Higher is Better)           
=========================================================================================================
Values indicate mean turns survived by defense against each attacker. Higher values = tougher defense.
  [D1] Uniform Random                    [D2] Spaced (No-Touching)
  [D3] Corner & Edge Hugging             [D4] Tightly Clustered Phalanx
  [D5] Anti-Midpoint (Bisection Dodger)  [D6] Bait Capital + Corner Destroyer
  [D7] Parallel Lanes (Single Axis)      

+-----------------------------------+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+---------+
| Defense Strategy                  | A1  | A2  | A3  | A4  | A5  | A6  | A7  | A8  | A9  | A10 | A11 | Overall |
+-----------------------------------+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+---------+
| [D1] Uniform Random                 |  61 |  58 |  58 |  54 |  59 |  52 |  45 |  49 |  48 |  46 |  49 |   52.7  |
| [D2] Spaced (No-Touching)           |  63 |  54 |  53 |  52 |  51 |  50 |  53 |  45 |  48 |  53 |  54 |   52.4  |
| [D3] Corner & Edge Hugging          |  63 |  49 |  70 |  46 |  50 |  51 |  49 |  53 |  52 |  57 |  50 |   53.6  |
| [D4] Tightly Clustered Phalanx      |  53 |  61 |  56 |  46 |  41 |  48 |  42 |  46 |  44 |  40 |  37 |   46.6  |
| [D5] Anti-Midpoint (Bisection Dodger) |  61 |  49 |  65 |  50 |  52 |  54 |  45 |  55 |  50 |  51 |  52 |   53.1  |
| [D6] Bait Capital + Corner Destroyer |  60 |  54 |  65 |  52 |  50 |  53 |  52 |  53 |  48 |  54 |  56 |   54.4  |
| [D7] Parallel Lanes (Single Axis)   |  64 |  55 |  50 |  52 |  51 |  49 |  48 |  50 |  50 |  53 |  43 |   51.3  |
+-----------------------------------+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+---------+

=========================================================================================================
                                7x7 DEFENSE TOURNAMENT WIN RATE MATRIX (%)                               
=========================================================================================================
Columns indicate opponent. Value is row defense's win % against column when both face identical attackers.
+-----------------------------------+------+------+------+------+------+------+------+
| Defense Strategy                  | [D1] | [D2] | [D3] | [D4] | [D5] | [D6] | [D7] |
+-----------------------------------+------+------+------+------+------+------+------+
| [D1] Uniform Random                |  --- |   59%|   27%|   64%|   41%|   32%|   41%|
| [D2] Spaced (No-Touching)          |   41%|  --- |   45%|   59%|   27%|   32%|   55%|
| [D3] Corner & Edge Hugging         |   73%|   55%|  --- |   55%|   68%|   36%|   45%|
| [D4] Tightly Clustered Phalanx     |   36%|   41%|   45%|  --- |   36%|   45%|   45%|
| [D5] Anti-Midpoint (Bisection Dodger) |   59%|   73%|   32%|   64%|  --- |   55%|   45%|
| [D6] Bait Capital + Corner Destroyer |   68%|   68%|   64%|   55%|   45%|  --- |   68%|
| [D7] Parallel Lanes (Single Axis)  |   59%|   45%|   55%|   55%|   55%|   32%|  --- |
+-----------------------------------+------+------+------+------+------+------+------+

=========================================================================================================
                             OVERALL DEFENSE TOURNAMENT STANDINGS                                        
=========================================================================================================
+------+-----------------------------------+---------+-------+--------+---------------+---------------+
| Rank | Defense Strategy                  | Matches |  Wins | Losses | Overall Win % | Mean Survived |
+------+-----------------------------------+---------+-------+--------+---------------+---------------+
|    1 | Bait Capital + Corner Destroyer   |     132 |    81 |     51 |         61.4% |    54.4 turns |
|    2 | Corner & Edge Hugging             |     132 |    73 |     59 |         55.3% |    53.6 turns |
|    3 | Anti-Midpoint (Bisection Dodger)  |     132 |    72 |     60 |         54.5% |    53.1 turns |
|    4 | Parallel Lanes (Single Axis)      |     132 |    66 |     66 |         50.0% |    51.3 turns |
|    5 | Uniform Random                    |     132 |    58 |     74 |         43.9% |    52.7 turns |
|    6 | Spaced (No-Touching)              |     132 |    57 |     75 |         43.2% |    52.4 turns |
|    7 | Tightly Clustered Phalanx         |     132 |    55 |     77 |         41.7% |    46.6 turns |
+------+-----------------------------------+---------+-------+--------+---------------+---------------+
```

---

## CLI Usage

### 1. Run Complete Benchmark (`simulate`)
Simulates $N$ games (default: 1,000) for all 11 attack strategies and 7 defense strategies, printing all summary tables, cross-matrices, standings, and histograms:

```bash
cargo run --release -- simulate --games 1000 --seed 42
```

### 2. Run a Single Match (`match`)
Simulates a single head-to-head match with custom attack and defense strategies:

```bash
cargo run -- match --p1 bayesian_pdf --p2 monte_carlo --d1 spaced --d2 bait_and_hide --seed 123
```

Attack options:
- `random_hunt`, `checkerboard`, `largest_rect`, `adaptive_stride`, `stride4_to_2`, `rect_stride4`, `stride_5432`, `rect_stride_5432`, `rect_checkerboard`, `monte_carlo`, `bayesian_pdf`

Defense options:
- `random`, `spaced`, `corner_edge`, `clustered`, `anti_midpoint`, `bait_and_hide`, `parallel_lanes`

### 3. Launch HTTP Server & Web Viewer (`serve`)
Starts the built-in HTTP server on `127.0.0.1:8080`:

```bash
cargo run -- serve --port 8080
```

---

## REST API Endpoints

### `GET /api/match`
- Query parameters:
  - `p1`: Attack strategy for Player 1 (default: `random_hunt`)
  - `p2`: Attack strategy for Player 2 (default: `checkerboard`)
  - `d1`: Defense strategy for Player 1 (default: `random`)
  - `d2`: Defense strategy for Player 2 (default: `random`)
  - `seed`: 64-bit integer seed (optional)
- Response: `application/json` with complete ship placements, turn-by-turn shot coordinates, outcomes, AI modes, and winner.

### `GET /api/benchmark`
- Query parameters:
  - `games`: Number of games per strategy (default: 200, max: 5000)
  - `seed`: 64-bit integer seed (optional)
- Response: `application/json` with 11-attack and 7-defense distributions, 11x11 attack matrix, 7x11 survival matrix, and 7x7 defense tournament standings.

---

## Testing & Quality Assurance

Run the unit test suite:

```bash
cargo test
```

Test coverage includes:
- **`rng::tests`**: Determinism, reproducibility across seeds, uniform bounds checking, Fisher-Yates shuffle validation.
- **`game::tests`**: Hasbro ship length checks, 17 total cells, legal placement, out-of-bounds rejection, overlapping placement rejection, shot damage and sinking detection, tracking board updates.
- **`strategy::tests`**: Fleet placement legality, single-player game completion in $\le 100$ turns for all 11 attack strategies and all 7 defense strategies, Chebyshev spaced buffer, parallel lane consistency, smart collinear targeting priority, 2D prefix-sum empty rectangle detection, Bayesian collinear dominance.
- **`sim::tests`**: Head-to-head match completion, JSON serialization integrity, 11x11 attack tournament matrix, 7x11 defense survival matrix, 7x7 defense tournament matrix and standings verification.
- **`server::tests`**: HTTP query string parser, strategy instantiation for all 11 attack IDs and all 7 defense IDs.

---

## Disclaimer

This work is a personal project and is not affiliated with, sponsored by, or endorsed by my employer.
