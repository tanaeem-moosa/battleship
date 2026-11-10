# Battleship Simulator Engine & Tactical Lab

> **Disclaimer:** This is an independent personal project and is not affiliated with, endorsed by, or connected to my employer in any way.

A high-performance, deterministic Battleship simulation engine, CLI, and HTTP server written in **pure Rust standard library (`std` only, zero external crate dependencies)**.

Simulates games across **3 distinct rulesets** (`Standard Hasbro`, `Salvo Mode`, and `Silent Sinking / Fog of War`) and provides deep statistical benchmarking across **13 distinct attack strategies** and **7 defense strategies**, including:
- **13x13 All-Pairs Attack Tournament Matrix** & standings
- **7 Defenses x 13 Attackers Survival Matrix** (mean turns to defeat fleet)
- **7x7 Defense Head-to-Head Tournament Matrix** & overall defense rankings
- **Cross-Ruleset Meta Comparison** (`Standard` vs. `Salvo` vs. `Silent Sinking`)

---

## Highlights

- **3 Ruleset Variants (`Ruleset`)**:
  - `standard`: **Standard Hasbro Rules** — 10×10 ocean grid (`WIDTH = 10`, `HEIGHT = 10`), 5 ships (`Carrier` 5, `Battleship` 4, `Cruiser` 3, `Submarine` 3, `Destroyer` 2 — total 17 cells), 1 shot/turn, explicit `Miss`, `Hit`, and `Sunk(ShipKind)` announcements.
  - `salvo`: **Salvo Mode** — simultaneous round volleys where shots per round equal your surviving ships (`1..=5`), coupling offensive firepower to fleet survival and planned via Virtual-Miss ("Constant Liar") batch diversification.
  - `silent`: **Silent Sinking (Fog of War)** — 1 shot/turn announcing *only* `Miss` or `Hit` (never `Sunk(ShipKind)`), testing contiguous capped-chain ship deduction, L/T-junction cluster camouflage, and joint 5-ship Monte Carlo inference.
- **Fast Deterministic PRNG**: `Xoshiro256**` algorithm seeded via `SplitMix64` with unbiased Lemire integer range generation. 100% reproducible simulations given any 64-bit seed.
- **13 Advanced Attack Strategies**:
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
  12. `crosshair_stride`: **Crosshair Stride + Open-Target** — staggered 4-to-2 lattice evaluated with a "+" crosshair arm scoring algorithm (`score = 3 * ways_max + ways_min`), paired with open-runway collinear target prioritization.
  13. `crosshair_5432`: **Crosshair Stride (5->4->3->2)** — descending 5→4→3→2 stride lattice matching maximum afloat ship length evaluated with crosshair scoring and open-runway collinear targeting.
- **7 Specialized Defense Strategies**:
  1. `random`: **Uniform Random** — uniform random orientation and non-overlapping placement.
  2. `spaced`: **Spaced (No-Touching)** — enforces a 1-cell halo buffer around every ship so no two ships touch orthogonally or diagonally.
  3. `corner_edge`: **Corner & Edge Hugging** — places ships on the outer perimeter (`row 0/9`, `col 0/9`), touching the 4 corners while maintaining gap spacing.
  4. `clustered`: **Tightly Clustered Phalanx** — bunches all ships into a contiguous touching formation, leaving ~75%+ of the grid completely barren.
  5. `anti_midpoint`: **Anti-Midpoint (Bisection Dodger)** — penalizes primary bisection lines (rows/cols 4, 5) and quarter lines (rows/cols 2, 7) to evade midpoint algorithms.
  6. `bait_and_hide`: **Bait Capital + Corner Destroyer** — hides small endgame ships (Destroyer, Submarine) in extreme corners and baits capital ships into interior cells with $\ge 2$-cell buffer against target splash.
  7. `parallel_lanes`: **Parallel Lanes (Single Axis)** — forces the entire fleet into a single shared axis (Horizontal or Vertical), placed in parallel lanes.
- **Dual Round-Robin Tournaments**:
  - **13x13 Attack Tournament**: Pairs all attack algorithms with alternating first-mover advantage.
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
│   ├── strategy.rs            # 13 Attack Strategies, 7 Defense Strategies, TargetTracker
│   ├── sim.rs                 # Dual tournaments (13x13 Attack, 7x7 Defense), 7x13 survival matrix
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
Games simulated: 400 solo per strategy | Tournament: 400 duels per pair | Base Seed: 42

ATTACK STRATEGY PERFORMANCE (Turns to sink all 17 ship cells):
+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+
| Strategy                          | Mean  | Median | Min | Max | StDev | P10 | P90 |
+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+
| Crosshair Stride + Open-Target    |  44.8 |   45.0 |  25 |  63 |  8.63 |  33 |  56 |
| Crosshair Stride (5->4->3->2)     |  45.6 |   46.0 |  25 |  71 |  9.15 |  34 |  58 |
| Bayesian Probability Density      |  45.7 |   44.0 |  24 |  73 |  9.62 |  34 |  59 |
| Monte Carlo Sampling              |  47.5 |   46.0 |  28 | 100 | 10.41 |  35 |  62 |
| Largest Rect + Stride (5->4->3->2) |  48.1 |   47.0 |  25 |  99 | 10.75 |  35 |  62 |
| Largest Rect + Stride (4->2)      |  48.3 |   48.0 |  25 | 100 | 10.19 |  37 |  59 |
| Largest Rect + Checkerboard       |  49.0 |   50.0 |  25 |  96 |  9.22 |  38 |  59 |
| Adaptive Stride (2->3->4->5)      |  50.5 |   50.5 |  26 |  73 |  9.06 |  38 |  61 |
| Staggered Stride (4->2)           |  51.9 |   53.0 |  26 |  99 | 11.27 |  38 |  63 |
| Checkerboard + Hunt/Target        |  52.4 |   53.5 |  22 |  99 |  9.53 |  40 |  63 |
| Descending Stride (5->4->3->2)    |  53.0 |   52.0 |  25 |  97 | 11.30 |  40 |  68 |
| Largest Empty Rect Midpoint       |  53.1 |   53.0 |  22 |  99 | 11.55 |  39 |  67 |
| Random + Hunt/Target              |  60.2 |   59.0 |  27 |  98 | 15.37 |  41 |  81 |
+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+

=========================================================================================================
                          OVERALL ATTACK TOURNAMENT STANDINGS (ALL-PAIRS DUELS)                          
=========================================================================================================
+------+-----------------------------------+---------+-------+--------+---------------+
| Rank | Strategy                          | Matches |  Wins | Losses | Overall Win % |
+------+-----------------------------------+---------+-------+--------+---------------+
|    1 | Crosshair Stride + Open-Target    |    4800 |  3091 |   1709 |         64.4% |
|    2 | Bayesian Probability Density      |    4800 |  3003 |   1797 |         62.6% |
|    3 | Crosshair Stride (5->4->3->2)     |    4800 |  2974 |   1826 |         62.0% |
|    4 | Largest Rect + Stride (4->2)      |    4800 |  2715 |   2085 |         56.6% |
|    5 | Monte Carlo Sampling              |    4800 |  2691 |   2109 |         56.1% |
|    6 | Largest Rect + Checkerboard       |    4800 |  2502 |   2298 |         52.1% |
|    7 | Largest Rect + Stride (5->4->3->2) |    4800 |  2494 |   2306 |         52.0% |
|    8 | Staggered Stride (4->2)           |    4800 |  2190 |   2610 |         45.6% |
|    9 | Descending Stride (5->4->3->2)    |    4800 |  2122 |   2678 |         44.2% |
|   10 | Adaptive Stride (2->3->4->5)      |    4800 |  2085 |   2715 |         43.4% |
|   11 | Largest Empty Rect Midpoint       |    4800 |  2017 |   2783 |         42.0% |
|   12 | Checkerboard + Hunt/Target        |    4800 |  1939 |   2861 |         40.4% |
|   13 | Random + Hunt/Target              |    4800 |  1377 |   3423 |         28.7% |
+------+-----------------------------------+---------+-------+--------+---------------+

=========================================================================================================
            7 DEFENSES x 13 ATTACKERS SURVIVAL MATRIX (Mean Turns Survived - Higher is Better)           
=========================================================================================================
Values indicate mean turns survived by defense against each attacker. Higher values = tougher defense.
  [D1] Uniform Random                    [D2] Spaced (No-Touching)
  [D3] Corner & Edge Hugging             [D4] Tightly Clustered Phalanx
  [D5] Anti-Midpoint (Bisection Dodger)  [D6] Bait Capital + Corner Destroyer
  [D7] Parallel Lanes (Single Axis)      

+-----------------------------------+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+---------+
| Defense Strategy                  | A1  | A2  | A3  | A4  | A5  | A6  | A7  | A8  | A9  | A10 | A11 | A12 | A13 | Overall |
+-----------------------------------+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+---------+
| [D1] Uniform Random                 |  59 |  51 |  50 |  54 |  48 |  45 |  50 |  48 |  47 |  49 |  47 |  47 |  46 |   49.4  |
| [D2] Spaced (No-Touching)           |  63 |  53 |  50 |  47 |  51 |  46 |  53 |  49 |  49 |  50 |  47 |  45 |  46 |   49.9  |
| [D3] Corner & Edge Hugging          |  59 |  49 |  68 |  51 |  51 |  49 |  50 |  56 |  56 |  53 |  51 |  54 |  55 |   54.0  |
| [D4] Tightly Clustered Phalanx      |  47 |  45 |  49 |  45 |  44 |  51 |  40 |  50 |  51 |  49 |  42 |  48 |  45 |   46.6  |
| [D5] Anti-Midpoint (Bisection Dodger) |  60 |  52 |  64 |  54 |  52 |  49 |  53 |  52 |  52 |  54 |  52 |  49 |  49 |   53.3  |
| [D6] Bait Capital + Corner Destroyer |  61 |  55 |  65 |  53 |  51 |  59 |  53 |  57 |  57 |  59 |  56 |  56 |  55 |   56.7  |
| [D7] Parallel Lanes (Single Axis)   |  60 |  51 |  53 |  53 |  54 |  49 |  50 |  48 |  49 |  51 |  50 |  44 |  46 |   50.5  |
+-----------------------------------+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+---------+

=========================================================================================================
                                7x7 DEFENSE TOURNAMENT WIN RATE MATRIX (%)                               
=========================================================================================================
Columns indicate opponent. Value is row defense's win % against column when both face identical attackers.
+-----------------------------------+------+------+------+------+------+------+------+
| Defense Strategy                  | [D1] | [D2] | [D3] | [D4] | [D5] | [D6] | [D7] |
+-----------------------------------+------+------+------+------+------+------+------+
| [D1] Uniform Random                |  --- |   40%|   31%|   51%|   39%|   31%|   53%|
| [D2] Spaced (No-Touching)          |   60%|  --- |   38%|   54%|   43%|   33%|   48%|
| [D3] Corner & Edge Hugging         |   69%|   62%|  --- |   61%|   54%|   43%|   53%|
| [D4] Tightly Clustered Phalanx     |   49%|   46%|   39%|  --- |   35%|   26%|   36%|
| [D5] Anti-Midpoint (Bisection Dodger) |   61%|   57%|   46%|   65%|  --- |   33%|   69%|
| [D6] Bait Capital + Corner Destroyer |   69%|   67%|   57%|   74%|   67%|  --- |   64%|
| [D7] Parallel Lanes (Single Axis)  |   47%|   52%|   47%|   64%|   31%|   36%|  --- |
+-----------------------------------+------+------+------+------+------+------+------+

=========================================================================================================
                             OVERALL DEFENSE TOURNAMENT STANDINGS                                        
=========================================================================================================
+------+-----------------------------------+---------+-------+--------+---------------+---------------+
| Rank | Defense Strategy                  | Matches |  Wins | Losses | Overall Win % | Mean Survived |
+------+-----------------------------------+---------+-------+--------+---------------+---------------+
|    1 | Bait Capital + Corner Destroyer   |     624 |   415 |    209 |         66.5% |    56.7 turns |
|    2 | Corner & Edge Hugging             |     624 |   356 |    268 |         57.1% |    54.0 turns |
|    3 | Anti-Midpoint (Bisection Dodger)  |     624 |   344 |    280 |         55.1% |    53.3 turns |
|    4 | Parallel Lanes (Single Axis)      |     624 |   288 |    336 |         46.2% |    50.5 turns |
|    5 | Spaced (No-Touching)              |     624 |   286 |    338 |         45.8% |    49.9 turns |
|    6 | Uniform Random                    |     624 |   255 |    369 |         40.9% |    49.4 turns |
|    7 | Tightly Clustered Phalanx         |     624 |   240 |    384 |         38.5% |    46.6 turns |
+------+-----------------------------------+---------+-------+--------+---------------+---------------+
```

### Cross-Ruleset Benchmark Comparison (`400` solo + `31,200` attack duels + `4,368` defense duels per ruleset, `seed = 42`):

```
ATTACK STRATEGIES CROSS-RULESET PERFORMANCE (Solo Mean Shots & Tournament Win %):
+----+-----------------------------------+--------------------+--------------------+--------------------+
| #  | Strategy                          | Standard Hasbro    | Salvo Mode         | Silent Sinking     |
|    |                                   |  Mean   |  Win %   |  Mean   |  Win %   |  Mean   |  Win %   |
+----+-----------------------------------+---------+----------+---------+----------+---------+----------+
|  1 | Random + Hunt/Target              |   60.2  |   27.0%  |   65.4  |   32.6%  |   66.4  |   31.2%  |
|  2 | Checkerboard + Hunt/Target        |   52.4  |   40.9%  |   59.6  |   41.8%  |   57.8  |   46.9%  |
|  3 | Largest Empty Rect Midpoint       |   53.1  |   40.9%  |   57.8  |   45.9%  |   62.1  |   40.8%  |
|  4 | Adaptive Stride (2->3->4->5)      |   50.6  |   43.5%  |   59.2  |   42.7%  |   57.6  |   47.5%  |
|  5 | Staggered Stride (4->2)           |   52.3  |   46.4%  |   56.8  |   46.0%  |   55.7  |   47.7%  |
|  6 | Largest Rect + Stride (4->2)      |   48.4  |   55.7%  |   54.6  |   54.1%  |   55.7  |   53.5%  |
|  7 | Descending Stride (5->4->3->2)    |   52.4  |   44.7%  |   59.4  |   43.7%  |   58.8  |   45.7%  |
|  8 | Largest Rect + Stride (5->4->3->2) |   48.0  |   53.4%  |   57.0  |   52.4%  |   58.0  |   50.3%  |
|  9 | Largest Rect + Checkerboard       |   49.1  |   52.5%  |   55.6  |   51.0%  |   56.8  |   52.8%  |
| 10 | Monte Carlo Sampling              |   46.8  |   56.5%  |   53.7  |   57.6%  |   51.2  |   65.1%  |
| 11 | Bayesian Probability Density      |   45.7  |   62.3%  |   53.2  |   61.4%  |   51.7  |   62.3%  |
| 12 | Crosshair Stride + Open-Target    |   45.5  |   63.4%  |   53.1  |   61.4%  |   55.7  |   54.5%  |
| 13 | Crosshair Stride (5->4->3->2)     |   45.5  |   62.7%  |   55.1  |   59.4%  |   57.6  |   51.8%  |
+----+-----------------------------------+---------+----------+---------+----------+---------+----------+

DEFENSE STRATEGIES CROSS-RULESET PERFORMANCE (Survival Mean & Tournament Win %):
+-----+-----------------------------------+--------------------+--------------------+--------------------+
|  #  | Defense Strategy                  | Standard Hasbro    | Salvo Mode         | Silent Sinking     |
|     |                                   |  Surv   |  Win %   |  Surv   |  Win %   |  Surv   |  Win %   |
+-----+-----------------------------------+---------+----------+---------+----------+---------+----------+
| [D1] | Uniform Random                    |   49.2  |   45.0%  |   56.9  |   41.8%  |   56.2  |   58.0%  |
| [D2] | Spaced (No-Touching)              |   50.3  |   47.8%  |   60.6  |   54.0%  |   52.1  |   45.2%  |
| [D3] | Corner & Edge Hugging             |   53.9  |   60.7%  |   60.9  |   68.1%  |   55.7  |   57.7%  |
| [D4] | Tightly Clustered Phalanx         |   47.0  |   35.7%  |   37.9  |    8.7%  |   45.3  |   27.2%  |
| [D5] | Anti-Midpoint (Bisection Dodger)  |   53.2  |   52.6%  |   62.2  |   65.9%  |   54.1  |   49.7%  |
| [D6] | Bait Capital + Corner Destroyer   |   55.3  |   63.9%  |   61.7  |   54.8%  |   61.0  |   68.6%  |
| [D7] | Parallel Lanes (Single Axis)      |   50.2  |   44.2%  |   60.4  |   56.7%  |   52.2  |   43.6%  |
+-----+-----------------------------------+---------+----------+---------+----------+---------+----------+
```

---

## CLI Usage

### 1. Run Complete Benchmark (`simulate`)
Simulates $N$ games across rulesets (`all`, `standard`, `salvo`, `silent`) for all 13 attack strategies and 7 defense strategies, printing all summary tables, cross-matrices, standings, histograms, and generating `viewer/sample_data.js`:

```bash
cargo run --release -- simulate --games 400 --seed 42 --ruleset all
```

### 2. Run a Single Match (`match`)
Simulates a single head-to-head match with custom attack, defense, and ruleset (`standard`, `salvo`, `silent`):

```bash
cargo run -- match --p1 bayesian_pdf --p2 monte_carlo --d1 spaced --d2 bait_and_hide --ruleset salvo --seed 123
```

Attack options:
- `random_hunt`, `checkerboard`, `largest_rect`, `adaptive_stride`, `stride4_to_2`, `rect_stride4`, `stride_5432`, `rect_stride_5432`, `rect_checkerboard`, `monte_carlo`, `bayesian_pdf`, `crosshair_stride`, `crosshair_5432`

Defense options:
- `random`, `spaced`, `corner_edge`, `clustered`, `anti_midpoint`, `bait_and_hide`, `parallel_lanes`

Ruleset options:
- `standard`, `salvo`, `silent`

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
  - `ruleset`: `standard`, `salvo`, or `silent` (default: `standard`)
  - `seed`: 64-bit integer seed (optional)
- Response: `application/json` with complete ship placements, turn-by-turn / salvo shot coordinates, outcomes, AI modes, and winner.

### `GET /api/benchmark`
- Query parameters:
  - `games`: Number of games per strategy (default: 200, max: 5000)
  - `ruleset`: `standard`, `salvo`, or `silent` (default: `standard`)
  - `seed`: 64-bit integer seed (optional)
- Response: `application/json` with 13-attack and 7-defense distributions, 13x13 attack matrix, 7x13 survival matrix, and 7x7 defense tournament standings.

---

## Testing & Quality Assurance

Run the unit test suite:

```bash
cargo test
```

Test coverage includes:
- **`rng::tests`**: Determinism, reproducibility across seeds, uniform bounds checking, Fisher-Yates shuffle validation.
- **`game::tests`**: Hasbro ship length checks, 17 total cells, legal placement, out-of-bounds rejection, overlapping placement rejection, shot damage and sinking detection, tracking board updates.
- **`strategy::tests`**: Fleet placement legality, single-player game completion in $\le 100$ turns for all 13 attack strategies and all 7 defense strategies, Chebyshev spaced buffer, parallel lane consistency, smart collinear targeting priority, 2D prefix-sum empty rectangle detection, Bayesian collinear dominance, Crosshair Stride (`4->2` and `5->4->3->2`).
- **`sim::tests`**: Head-to-head match completion, JSON serialization integrity, 13x13 attack tournament matrix, 7x13 defense survival matrix, 7x7 defense tournament matrix and standings verification.
- **`server::tests`**: HTTP query string parser, strategy instantiation for all 13 attack IDs and all 7 defense IDs.

---

## Disclaimer

This work is a personal project and is not affiliated with, sponsored by, or endorsed by my employer.
