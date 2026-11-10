# Battleship Simulator Engine & Tactical Lab

> **Disclaimer:** This is an independent personal project and is not affiliated with, endorsed by, or connected to my employer in any way.

A high-performance, deterministic Battleship simulation engine, CLI, and HTTP server written in **pure Rust standard library (`std` only, zero external crate dependencies)**.

Simulates games conforming strictly to standard Hasbro Battleship rules and provides deep statistical benchmarking between attack strategies, including **Random Hunt/Target** and **Checkerboard Hunt/Target**.

---

## Highlights

- **Standard Hasbro Ruleset**: 10×10 ocean grid (`WIDTH = 10`, `HEIGHT = 10`), 5 ships (`Carrier` 5, `Battleship` 4, `Cruiser` 3, `Submarine` 3, `Destroyer` 2 — total 17 cells), orthogonal placements, non-overlapping constraints, and explicit shot outcomes (`Miss`, `Hit`, `Sunk(ShipKind)`).
- **Fast Deterministic PRNG**: `Xoshiro256**` algorithm seeded via `SplitMix64` with unbiased Lemire integer range generation. 100% reproducible simulations given any 64-bit seed.
- **Advanced Attack Strategies**:
  - `Random + Hunt/Target`: Pure random hunt mode with smart collinear and orthogonal hunt-down targeting.
  - `Checkerboard + Hunt/Target`: Parity-restricted hunt mode (`(row + col) % 2 == parity`) leveraging the mathematical certainty that any ship of length $\ge 2$ intersects the parity grid.
  - **Smart Collinear Targeting**: Automatically identifies 2+ collinear active hits, prioritizes unshot line endpoints, falls back to orthogonal neighbors (`N, S, E, W`), and resolves multi-ship contact upon sinking.
- **Statistical Benchmark Engine**: Measures mean, median, min, max, standard deviation, P10, P90, turn distribution histograms (17..=100), and simultaneous head-to-head win rates over $N$ games.
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
│   ├── strategy.rs            # DefenseStrategy, AttackStrategy, TargetTracker
│   ├── sim.rs                 # Simulation, benchmarks, JSON serializers, ASCII histogram
│   └── server.rs              # std::net HTTP server & REST API router
├── viewer/                    # Interactive Tactical Command Center Web UI
│   ├── index.html             # Web viewer interface
│   ├── styles.css             # Tactical cyber command styling
│   ├── app.js                 # Dual-mode viewer engine
│   └── sample_data.js         # Precomputed benchmark & match replays
└── README.md                  # This file
```

---

## Strategy Comparison & Theory

### 1. The Parity Principle (Checkerboard Hunt)
In standard Hasbro Battleship, the smallest ship in the fleet is the **Destroyer** (length 2). Any horizontal or vertical placement of length $\ge 2$ occupies cells of alternating parity:
$$\text{parity}(r, c) = (r + c) \pmod 2$$

Because consecutive steps alternate parity:
- $(r, c) \to (r, c+1) \implies (r + c + 1) \not\equiv (r + c) \pmod 2$
- $(r, c) \to (r+1, c) \implies (r + 1 + c) \not\equiv (r + c) \pmod 2$

Every ship must cover at least one cell where $(r + c) \pmod 2 = 0$ and at least one where $(r + c) \pmod 2 = 1$. By hunting exclusively on a single parity, the search space is immediately halved from **100 cells to 50 cells** without sacrificing the guarantee of locating every ship.

### 2. Smart Collinear Targeting
Once a hit is registered, the strategy switches to **Target Mode**:
1. **Collinear Priority**: If two or more active hits lie along the same row or column and are contiguous, the next shot is prioritized at the immediate unshot endpoints along that line.
2. **Orthogonal Fallback**: If endpoints are blocked (board boundary or known misses), the strategy falls back to orthogonal neighbors (`N, S, E, W`) of all active hits.
3. **Multi-Ship Resolution**: When a ship of length $L$ is sunk, the $L$ collinear cells matching the sunk ship containing the final shot are pruned from the active hit pool. Any remaining hits on touching or parallel ships remain active and continue being targeted.

### Benchmark Results (1,000 Games per Strategy, `seed = 42`)

| Metric | Random + Hunt/Target | Checkerboard + Hunt/Target |
| :--- | :---: | :---: |
| **Mean Turns to Win** | 60.88 | **52.38** (8.50 turns faster) |
| **Median Turns (`P50`)** | 60.0 | **53.0** |
| **Standard Deviation ($\sigma$)** | 15.31 | **9.34** (-39.0% variance) |
| **P10 (Fastest 10%)** | 40 turns | **40 turns** |
| **P90 (Slowest 10%)** | 82 turns | **62 turns** (20 turns faster) |
| **Games $\ge 65$ Turns** | 40.8% | **3.7%** (11× fewer long games) |
| **Head-to-Head Win Rate** | 32.2% (322 wins) | **67.8% (678 wins)** |

---

## CLI Usage

### 1. Run Strategy Benchmark (`simulate`)
Simulates $N$ games (default: 1,000) for both strategies, prints a formatted summary table and ASCII histogram, and outputs `viewer/sample_data.js`:

```bash
cargo run --release -- simulate --games 1000 --seed 42
```

Example terminal output:
```
================================================================================
                     BATTLESHIP ENGINE BENCHMARK REPORT                         
================================================================================
Games simulated: 1000 per strategy | Base Seed: 42

STRATEGY PERFORMANCE (Turns to sink all 17 ship cells):
+----------------------------+-------+--------+-----+-----+-------+-----+-----+
| Strategy                   | Mean  | Median | Min | Max | StDev | P10 | P90 |
+----------------------------+-------+--------+-----+-----+-------+-----+-----+
| Random + Hunt/Target       |  60.9 |   60.0 |  27 |  98 | 15.31 |  40 |  82 |
| Checkerboard + Hunt/Target |  52.4 |   53.0 |  22 |  99 |  9.34 |  40 |  62 |
+----------------------------+-------+--------+-----+-----+-------+-----+-----+

HEAD-TO-HEAD MATCHUP RESULTS (1000 games):
  Random + Hunt/Target         :  322 wins ( 32.2%)
  Checkerboard + Hunt/Target   :  678 wins ( 67.8%)
```

### 2. Run a Single Match (`match`)
Simulates a single head-to-head match and streams the complete JSON replay to stdout:

```bash
cargo run -- match --p1 random_hunt --p2 checkerboard --seed 123
```

### 3. Launch HTTP Server & Web Viewer (`serve`)
Starts the built-in HTTP server on `127.0.0.1:8080`:

```bash
cargo run -- serve --port 8080
```

Navigate to `http://127.0.0.1:8080/` in your browser to inspect matches turn-by-turn with full scrubber playback, live board grids, heatmaps, and distribution histograms.

---

## REST API Endpoints

The built-in HTTP server exposes REST API endpoints with CORS support:

### `GET /api/match`
Generate a fresh head-to-head match replay:
- Query parameters:
  - `p1`: Strategy for Player 1 (`random_hunt` or `checkerboard`, default: `random_hunt`)
  - `p2`: Strategy for Player 2 (`random_hunt` or `checkerboard`, default: `checkerboard`)
  - `seed`: 64-bit integer seed (optional, defaults to system wall-clock time)
- Response: `application/json` with complete ship placements, turn-by-turn shot coordinates, outcomes, AI modes, and winner.

### `GET /api/benchmark`
Run an on-demand statistical benchmark:
- Query parameters:
  - `games`: Number of games per strategy (default: 200, max: 5000)
  - `seed`: 64-bit integer seed (optional)
- Response: `application/json` with distributions, percentiles, histograms, and head-to-head win counts.

---

## Testing & Quality Assurance

Run the comprehensive unit test suite:

```bash
cargo test
```

Test coverage includes:
- **`rng::tests`**: Determinism, reproducibility across seeds, uniform bounds checking, Fisher-Yates shuffle validation.
- **`game::tests`**: Hasbro ship length checks, 17 total cells, legal placement, out-of-bounds rejection, overlapping placement rejection, shot damage and sinking detection, tracking board updates.
- **`strategy::tests`**: Fleet placement legality, single-player game completion in $\le 100$ turns, smart collinear targeting priority.
- **`sim::tests`**: Head-to-head match completion, JSON serialization integrity, statistical proof that Checkerboard outperforms Random Hunt.
- **`server::tests`**: HTTP query string parser, strategy instantiation.

---

## Disclaimer

This work is a personal project and is not affiliated with, sponsored by, or endorsed by my employer.
