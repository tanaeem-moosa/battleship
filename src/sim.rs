//! Simulation engine, benchmarks, replay generation, and JSON serialization.
//!
//! Provides:
//! - Single-player evaluations (turns to win 17..=100)
//! - Head-to-head match simulation with full replay history
//! - Multi-game statistical benchmarking (mean, median, min, max, std dev, P10, P90, histograms)
//! - Hand-written pure std JSON serializer for replay and benchmark payloads
//! - Terminal-formatted summary tables and ASCII histograms

use crate::game::{
    MoveRecord, Player, Ruleset, ShipPlacement, ShotOutcome, TrackingBoard, TOTAL_SHIP_CELLS,
};
use crate::rng::Rng;
use crate::strategy::{
    AdaptiveStrideUpAttack, AntiMidpointDefense, AttackStrategy, BaitAndHideDefense,
    BayesianPdfAttack, CheckerboardAttack, ClusteredDefense, CornerEdgeDefense,
    CrosshairDescendingAttack, CrosshairStrideAttack, DefenseStrategy, DescendingStrideAttack,
    HuntAndTargetAttack, LargestRectMidpointAttack, MonteCarloAttack, ParallelLanesDefense,
    RandomDefense, RectCheckerboardAttack, RectDescendingStrideAttack, RectStrideFourAttack,
    SpacedDefense, StaggeredFourToTwoAttack,
};

/// Total number of implemented attack strategies in the benchmark suite.
pub const NUM_STRATEGIES: usize = 13;
/// Total number of implemented defense strategies in the benchmark suite.
pub const NUM_DEFENSES: usize = 7;

/// Helper function to format (row, col) as "A1".."J10".
pub fn coord_to_str(row: usize, col: usize) -> String {
    let row_char = (b'A' + (row as u8).min(25)) as char;
    format!("{}{}", row_char, col + 1)
}

/// Complete recording of a head-to-head match between two players.
#[derive(Clone, Debug)]
pub struct MatchReplay {
    pub ruleset: Ruleset,
    pub player1_name: String,
    pub player2_name: String,
    pub player1_ships: Vec<ShipPlacement>,
    pub player2_ships: Vec<ShipPlacement>,
    pub moves: Vec<MoveRecord>,
    pub winner: Player,
    pub total_rounds: usize,
    pub total_moves: usize,
    pub seed: u64,
}

/// Statistical summary for an attack strategy over N games.
#[derive(Clone, Debug)]
pub struct StrategyStats {
    pub id: String,
    pub name: String,
    pub games: usize,
    pub mean: f64,
    pub median: f64,
    pub min: usize,
    pub max: usize,
    pub std_dev: f64,
    pub p10: f64,
    pub p90: f64,
    pub histogram: Vec<(usize, usize)>, // (turns, count) for 17..=100
}

/// Head-to-head match statistics between Strategy 1 and Strategy 2.
#[derive(Clone, Debug)]
pub struct HeadToHeadStats {
    pub p1_name: String,
    pub p2_name: String,
    pub total_games: usize,
    pub p1_wins: usize,
    pub p2_wins: usize,
    pub p1_win_rate: f64,
    pub p2_win_rate: f64,
}

/// Individual pairwise match result between Strategy A and Strategy B in the tournament.
#[derive(Clone, Debug)]
pub struct PairwiseMatchResult {
    pub p1_id: String,
    pub p1_name: String,
    pub p2_id: String,
    pub p2_name: String,
    pub total_games: usize,
    pub p1_wins: usize,
    pub p2_wins: usize,
    pub p1_win_rate: f64,
    pub p2_win_rate: f64,
}

/// Overall standing of a strategy in the round-robin tournament.
#[derive(Clone, Debug)]
pub struct TournamentStanding {
    pub rank: usize,
    pub id: String,
    pub name: String,
    pub total_matches: usize,
    pub wins: usize,
    pub losses: usize,
    pub win_rate: f64,
}

/// Complete 6x6 Head-to-Head Tournament Matrix & Standings.
#[derive(Clone, Debug)]
pub struct HeadToHeadMatrix {
    pub strategy_ids: Vec<String>,
    pub strategy_names: Vec<String>,
    pub matrix: Vec<Vec<f64>>, // matrix[i][j] is win % of strategy i against strategy j
    pub pairings: Vec<PairwiseMatchResult>,
    pub standings: Vec<TournamentStanding>,
}

/// Comprehensive 7-Defense Evaluation: 7x13 survival matrix against all 13 attackers
/// and a complete 7x7 round-robin defense tournament.
#[derive(Clone, Debug)]
pub struct DefenseEvaluation {
    pub defense_ids: Vec<String>,
    pub defense_names: Vec<String>,
    pub attacker_ids: Vec<String>,
    pub attacker_names: Vec<String>,
    pub survival_matrix: Vec<Vec<f64>>, // 7x13: matrix[d][a] is mean turns survived by defense d vs attacker a
    pub defense_stats: Vec<StrategyStats>, // 7 defense overall stats
    pub tournament: HeadToHeadMatrix, // 7x7 defense tournament
}

/// Comprehensive benchmark report.
#[derive(Clone, Debug)]
pub struct BenchmarkReport {
    pub ruleset: Ruleset,
    pub games_per_strategy: usize,
    pub seed: u64,
    pub strategies: Vec<StrategyStats>,
    pub head_to_head: HeadToHeadStats,
    pub tournament: HeadToHeadMatrix,
    pub defense_eval: DefenseEvaluation,
}

/// Simulate a single player game with ruleset where `attack` tries to sink all ships placed by `defense`.
/// Returns the number of shots taken (17..=100).
pub fn simulate_game_with_ruleset(
    attack: &mut dyn AttackStrategy,
    defense: &mut dyn DefenseStrategy,
    ruleset: Ruleset,
    rng: &mut Rng,
) -> usize {
    let mut board = defense.place_ships(rng);
    let mut view = TrackingBoard::with_ruleset(ruleset);
    attack.reset();

    let mut shots = 0;
    if ruleset.is_salvo() {
        while !board.all_sunk() && shots < 100 {
            let unshot_count = view.unshot_cells().len();
            let k = 5.min(unshot_count);
            if k == 0 {
                break;
            }
            let salvo = attack.choose_salvo(&view, k, rng);
            let mut volley_results = Vec::new();
            for shot in salvo {
                shots += 1;
                let outcome = board.receive_shot_with_ruleset(shot.0, shot.1, ruleset);
                volley_results.push((shot, outcome));
                if board.all_sunk() || shots >= 100 {
                    break;
                }
            }
            for (shot, outcome) in volley_results {
                view.record_shot(shot.0, shot.1, outcome);
                attack.on_shot_result(shot, outcome, &view);
            }
        }
    } else {
        while !board.all_sunk() && shots < 100 {
            shots += 1;
            let pos = attack.choose_shot(&view, rng);
            let outcome = board.receive_shot_with_ruleset(pos.0, pos.1, ruleset);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }
    }

    shots
}

/// Backward-compatible simulate_game using Ruleset::Standard.
pub fn simulate_game(
    attack: &mut dyn AttackStrategy,
    defense: &mut dyn DefenseStrategy,
    rng: &mut Rng,
) -> usize {
    simulate_game_with_ruleset(attack, defense, Ruleset::Standard, rng)
}

/// Simulate a head-to-head match between Player 1 and Player 2 with ruleset.
/// Returns complete `MatchReplay`.
pub fn simulate_match_with_ruleset(
    p1_name: &str,
    p1_attack: &mut dyn AttackStrategy,
    p1_defense: &mut dyn DefenseStrategy,
    p2_name: &str,
    p2_attack: &mut dyn AttackStrategy,
    p2_defense: &mut dyn DefenseStrategy,
    ruleset: Ruleset,
    seed: u64,
) -> MatchReplay {
    let mut rng = Rng::with_seed(seed);

    let mut p1_board = p1_defense.place_ships(&mut rng);
    let mut p2_board = p2_defense.place_ships(&mut rng);

    let p1_ships = p1_board.placements().to_vec();
    let p2_ships = p2_board.placements().to_vec();

    let mut p1_view = TrackingBoard::with_ruleset(ruleset);
    let mut p2_view = TrackingBoard::with_ruleset(ruleset);

    p1_attack.reset();
    p2_attack.reset();

    let mut moves = Vec::new();
    let mut round = 0;
    let mut turn_seq = 0;
    let mut p1_shots = 0;
    let mut p1_hits = 0;
    let mut p1_misses = 0;
    let mut p2_shots = 0;
    let mut p2_hits = 0;
    let mut p2_misses = 0;
    let winner;

    if ruleset.is_salvo() {
        loop {
            round += 1;

            // Player 1 salvo
            let k1 = p1_board.remaining_ships().len().max(1).min(p1_view.unshot_cells().len());
            let p1_mode = p1_attack.current_mode().to_string();
            let salvo1 = p1_attack.choose_salvo(&p1_view, k1, &mut rng);
            let mut volley1_results = Vec::new();

            for (idx, shot) in salvo1.into_iter().enumerate() {
                turn_seq += 1;
                p1_shots += 1;
                let outcome = p2_board.receive_shot_with_ruleset(shot.0, shot.1, ruleset);
                if outcome.is_hit_or_sunk() {
                    p1_hits += 1;
                } else {
                    p1_misses += 1;
                }
                let sunk_cells = match outcome {
                    ShotOutcome::Sunk(kind) => p2_ships.iter().find(|p| p.kind == kind).map(|p| p.cells()),
                    _ => None,
                };
                moves.push(MoveRecord {
                    turn: turn_seq,
                    round,
                    player: Player::Player1,
                    row: shot.0,
                    col: shot.1,
                    coord: coord_to_str(shot.0, shot.1),
                    outcome,
                    sunk_cells,
                    mode: p1_mode.clone(),
                    salvo_shot: idx + 1,
                    salvo_size: k1,
                    p1_shots,
                    p1_hits,
                    p1_misses,
                    p1_afloat: p1_board.remaining_ships().len(),
                    p2_shots,
                    p2_hits,
                    p2_misses,
                    p2_afloat: p2_board.remaining_ships().len(),
                });
                volley1_results.push((shot, outcome));
                if p2_board.all_sunk() {
                    break;
                }
            }

            if p2_board.all_sunk() {
                winner = Player::Player1;
                break;
            }

            for (shot, outcome) in volley1_results {
                p1_view.record_shot(shot.0, shot.1, outcome);
                p1_attack.on_shot_result(shot, outcome, &p1_view);
            }

            // Player 2 salvo
            let k2 = p2_board.remaining_ships().len().max(1).min(p2_view.unshot_cells().len());
            let p2_mode = p2_attack.current_mode().to_string();
            let salvo2 = p2_attack.choose_salvo(&p2_view, k2, &mut rng);
            let mut volley2_results = Vec::new();

            for (idx, shot) in salvo2.into_iter().enumerate() {
                turn_seq += 1;
                p2_shots += 1;
                let outcome = p1_board.receive_shot_with_ruleset(shot.0, shot.1, ruleset);
                if outcome.is_hit_or_sunk() {
                    p2_hits += 1;
                } else {
                    p2_misses += 1;
                }
                let sunk_cells = match outcome {
                    ShotOutcome::Sunk(kind) => p1_ships.iter().find(|p| p.kind == kind).map(|p| p.cells()),
                    _ => None,
                };
                moves.push(MoveRecord {
                    turn: turn_seq,
                    round,
                    player: Player::Player2,
                    row: shot.0,
                    col: shot.1,
                    coord: coord_to_str(shot.0, shot.1),
                    outcome,
                    sunk_cells,
                    mode: p2_mode.clone(),
                    salvo_shot: idx + 1,
                    salvo_size: k2,
                    p1_shots,
                    p1_hits,
                    p1_misses,
                    p1_afloat: p1_board.remaining_ships().len(),
                    p2_shots,
                    p2_hits,
                    p2_misses,
                    p2_afloat: p2_board.remaining_ships().len(),
                });
                volley2_results.push((shot, outcome));
                if p1_board.all_sunk() {
                    break;
                }
            }

            if p1_board.all_sunk() {
                winner = Player::Player2;
                break;
            }

            for (shot, outcome) in volley2_results {
                p2_view.record_shot(shot.0, shot.1, outcome);
                p2_attack.on_shot_result(shot, outcome, &p2_view);
            }
        }
    } else {
        // Standard & Silent Sinking
        loop {
            round += 1;

            // Player 1 fires at Player 2's board
            turn_seq += 1;
            p1_shots += 1;
            let p1_mode = p1_attack.current_mode().to_string();
            let p1_shot = p1_attack.choose_shot(&p1_view, &mut rng);
            let p1_outcome = p2_board.receive_shot_with_ruleset(p1_shot.0, p1_shot.1, ruleset);
            p1_view.record_shot(p1_shot.0, p1_shot.1, p1_outcome);
            p1_attack.on_shot_result(p1_shot, p1_outcome, &p1_view);

            if p1_outcome.is_hit_or_sunk() {
                p1_hits += 1;
            } else {
                p1_misses += 1;
            }

            let p1_sunk_cells = match p1_outcome {
                ShotOutcome::Sunk(kind) => p2_ships.iter().find(|p| p.kind == kind).map(|p| p.cells()),
                _ => None,
            };

            moves.push(MoveRecord {
                turn: turn_seq,
                round,
                player: Player::Player1,
                row: p1_shot.0,
                col: p1_shot.1,
                coord: coord_to_str(p1_shot.0, p1_shot.1),
                outcome: p1_outcome,
                sunk_cells: p1_sunk_cells,
                mode: p1_mode,
                salvo_shot: 1,
                salvo_size: 1,
                p1_shots,
                p1_hits,
                p1_misses,
                p1_afloat: p1_board.remaining_ships().len(),
                p2_shots,
                p2_hits,
                p2_misses,
                p2_afloat: p2_board.remaining_ships().len(),
            });

            if p2_board.all_sunk() {
                winner = Player::Player1;
                break;
            }

            // Player 2 fires at Player 1's board
            turn_seq += 1;
            p2_shots += 1;
            let p2_mode = p2_attack.current_mode().to_string();
            let p2_shot = p2_attack.choose_shot(&p2_view, &mut rng);
            let p2_outcome = p1_board.receive_shot_with_ruleset(p2_shot.0, p2_shot.1, ruleset);
            p2_view.record_shot(p2_shot.0, p2_shot.1, p2_outcome);
            p2_attack.on_shot_result(p2_shot, p2_outcome, &p2_view);

            if p2_outcome.is_hit_or_sunk() {
                p2_hits += 1;
            } else {
                p2_misses += 1;
            }

            let p2_sunk_cells = match p2_outcome {
                ShotOutcome::Sunk(kind) => p1_ships.iter().find(|p| p.kind == kind).map(|p| p.cells()),
                _ => None,
            };

            moves.push(MoveRecord {
                turn: turn_seq,
                round,
                player: Player::Player2,
                row: p2_shot.0,
                col: p2_shot.1,
                coord: coord_to_str(p2_shot.0, p2_shot.1),
                outcome: p2_outcome,
                sunk_cells: p2_sunk_cells,
                mode: p2_mode,
                salvo_shot: 1,
                salvo_size: 1,
                p1_shots,
                p1_hits,
                p1_misses,
                p1_afloat: p1_board.remaining_ships().len(),
                p2_shots,
                p2_hits,
                p2_misses,
                p2_afloat: p2_board.remaining_ships().len(),
            });

            if p1_board.all_sunk() {
                winner = Player::Player2;
                break;
            }
        }
    }

    let total_moves = moves.len();
    MatchReplay {
        ruleset,
        player1_name: p1_name.to_string(),
        player2_name: p2_name.to_string(),
        player1_ships: p1_ships,
        player2_ships: p2_ships,
        moves,
        winner,
        total_rounds: round,
        total_moves,
        seed,
    }
}

/// Backward-compatible simulate_match using Ruleset::Standard.
pub fn simulate_match(
    p1_name: &str,
    p1_attack: &mut dyn AttackStrategy,
    p1_defense: &mut dyn DefenseStrategy,
    p2_name: &str,
    p2_attack: &mut dyn AttackStrategy,
    p2_defense: &mut dyn DefenseStrategy,
    seed: u64,
) -> MatchReplay {
    simulate_match_with_ruleset(
        p1_name,
        p1_attack,
        p1_defense,
        p2_name,
        p2_attack,
        p2_defense,
        Ruleset::Standard,
        seed,
    )
}

/// Compute statistical distribution metrics from a list of turn values.
pub fn compute_stats(id: &str, name: &str, mut turns: Vec<usize>) -> StrategyStats {
    let games = turns.len();
    assert!(games > 0, "Cannot compute stats on empty turn list");
    turns.sort_unstable();

    let min = turns[0];
    let max = turns[turns.len() - 1];

    let sum: usize = turns.iter().sum();
    let mean = sum as f64 / games as f64;

    let variance: f64 = turns
        .iter()
        .map(|&x| {
            let diff = x as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / games as f64;
    let std_dev = variance.sqrt();

    let median = if games % 2 == 1 {
        turns[games / 2] as f64
    } else {
        (turns[games / 2 - 1] + turns[games / 2]) as f64 / 2.0
    };

    let p10_idx = ((games as f64) * 0.10).floor() as usize;
    let p10 = turns[p10_idx.min(games - 1)] as f64;

    let p90_idx = ((games as f64) * 0.90).floor() as usize;
    let p90 = turns[p90_idx.min(games - 1)] as f64;

    // Histogram from 17 to 100
    let mut histogram = Vec::with_capacity(100 - TOTAL_SHIP_CELLS + 1);
    for t in TOTAL_SHIP_CELLS..=100 {
        let count = turns.iter().filter(|&&x| x == t).count();
        histogram.push((t, count));
    }

    StrategyStats {
        id: id.to_string(),
        name: name.to_string(),
        games,
        mean,
        median,
        min,
        max,
        std_dev,
        p10,
        p90,
        histogram,
    }
}

/// Helper to construct a boxed AttackStrategy along with its ID and display name.
pub fn create_strategy_instance(
    idx: usize,
) -> (Box<dyn AttackStrategy>, &'static str, &'static str) {
    match idx {
        0 => (
            Box::new(HuntAndTargetAttack::new()),
            "random_hunt",
            "Random + Hunt/Target",
        ),
        1 => (
            Box::new(CheckerboardAttack::new()),
            "checkerboard",
            "Checkerboard + Hunt/Target",
        ),
        2 => (
            Box::new(LargestRectMidpointAttack::new()),
            "largest_rect",
            "Largest Empty Rect Midpoint",
        ),
        3 => (
            Box::new(AdaptiveStrideUpAttack::new()),
            "adaptive_stride",
            "Adaptive Stride (2->3->4->5)",
        ),
        4 => (
            Box::new(StaggeredFourToTwoAttack::new()),
            "stride4_to_2",
            "Staggered Stride (4->2)",
        ),
        5 => (
            Box::new(RectStrideFourAttack::new()),
            "rect_stride4",
            "Largest Rect + Stride (4->2)",
        ),
        6 => (
            Box::new(DescendingStrideAttack::new()),
            "stride_5432",
            "Descending Stride (5->4->3->2)",
        ),
        7 => (
            Box::new(RectDescendingStrideAttack::new()),
            "rect_stride_5432",
            "Largest Rect + Stride (5->4->3->2)",
        ),
        8 => (
            Box::new(RectCheckerboardAttack::new()),
            "rect_checkerboard",
            "Largest Rect + Checkerboard",
        ),
        9 => (
            Box::new(MonteCarloAttack::new()),
            "monte_carlo",
            "Monte Carlo Sampling",
        ),
        10 => (
            Box::new(BayesianPdfAttack::new()),
            "bayesian_pdf",
            "Bayesian Probability Density",
        ),
        11 => (
            Box::new(CrosshairStrideAttack::new()),
            "crosshair_stride",
            "Crosshair Stride + Open-Target",
        ),
        12 => (
            Box::new(CrosshairDescendingAttack::new()),
            "crosshair_5432",
            "Crosshair Stride (5->4->3->2)",
        ),
        _ => (
            Box::new(CrosshairDescendingAttack::new()),
            "crosshair_5432",
            "Crosshair Stride (5->4->3->2)",
        ),
    }
}

pub fn create_defense_instance(
    idx: usize,
) -> (Box<dyn DefenseStrategy>, &'static str, &'static str) {
    match idx {
        0 => (Box::new(RandomDefense::new()), "random", "Uniform Random"),
        1 => (Box::new(SpacedDefense::new()), "spaced", "Spaced (No-Touching)"),
        2 => (Box::new(CornerEdgeDefense::new()), "corner_edge", "Corner & Edge Hugging"),
        3 => (Box::new(ClusteredDefense::new()), "clustered", "Tightly Clustered Phalanx"),
        4 => (Box::new(AntiMidpointDefense::new()), "anti_midpoint", "Anti-Midpoint (Bisection Dodger)"),
        5 => (Box::new(BaitAndHideDefense::new()), "bait_and_hide", "Bait Capital + Corner Destroyer"),
        6 => (Box::new(ParallelLanesDefense::new()), "parallel_lanes", "Parallel Lanes (Single Axis)"),
        _ => (Box::new(RandomDefense::new()), "random", "Uniform Random"),
    }
}

/// Execute a full benchmark with ruleset over `games` iterations:
/// 1. Evaluates all 13 strategies in solo simulation
/// 2. Executes a complete 13x13 All-Pairs Head-to-Head Tournament alternating first-turn player
/// 3. Evaluates all 7 Defenses x 13 Attackers Survival Matrix (91 cells)
/// 4. Executes a complete 7x7 Defense Head-to-Head Tournament
pub fn run_benchmark_with_ruleset(games: usize, seed: u64, ruleset: Ruleset) -> BenchmarkReport {
    let mut rng = Rng::with_seed(seed);
    let mut defense = RandomDefense::new();

    // 1. Evaluate all 13 strategies solo
    let mut strategies = Vec::with_capacity(NUM_STRATEGIES);
    for idx in 0..NUM_STRATEGIES {
        let (mut strat, id, name) = create_strategy_instance(idx);
        let mut turns = Vec::with_capacity(games);
        for _ in 0..games {
            turns.push(simulate_game_with_ruleset(strat.as_mut(), &mut defense, ruleset, &mut rng));
        }
        strategies.push(compute_stats(id, name, turns));
    }

    // 2. Complete 13x13 All-Pairs Head-to-Head Tournament (78 distinct pairings)
    let battles_per_pair = games.min(400);
    let mut win_counts = vec![vec![0usize; NUM_STRATEGIES]; NUM_STRATEGIES];
    let num_pairings = NUM_STRATEGIES * (NUM_STRATEGIES - 1) / 2;
    let mut pairings = Vec::with_capacity(num_pairings);

    for i in 0..NUM_STRATEGIES {
        for j in (i + 1)..NUM_STRATEGIES {
            let (mut strat_i, id_i, name_i) = create_strategy_instance(i);
            let (mut strat_j, id_j, name_j) = create_strategy_instance(j);
            let mut i_wins = 0;
            let mut j_wins = 0;
            let mut def1 = RandomDefense::new();
            let mut def2 = RandomDefense::new();

            for g in 0..battles_per_pair {
                let match_seed = rng.next_u64();
                // Alternate starting player each match to eliminate first-player advantage
                if g % 2 == 0 {
                    let replay = simulate_match_with_ruleset(
                        name_i,
                        strat_i.as_mut(),
                        &mut def1,
                        name_j,
                        strat_j.as_mut(),
                        &mut def2,
                        ruleset,
                        match_seed,
                    );
                    if replay.winner == Player::Player1 {
                        i_wins += 1;
                    } else {
                        j_wins += 1;
                    }
                } else {
                    let replay = simulate_match_with_ruleset(
                        name_j,
                        strat_j.as_mut(),
                        &mut def2,
                        name_i,
                        strat_i.as_mut(),
                        &mut def1,
                        ruleset,
                        match_seed,
                    );
                    if replay.winner == Player::Player1 {
                        j_wins += 1;
                    } else {
                        i_wins += 1;
                    }
                }
            }

            win_counts[i][j] = i_wins;
            win_counts[j][i] = j_wins;
            let i_rate = i_wins as f64 / battles_per_pair as f64;
            let j_rate = j_wins as f64 / battles_per_pair as f64;

            pairings.push(PairwiseMatchResult {
                p1_id: id_i.to_string(),
                p1_name: name_i.to_string(),
                p2_id: id_j.to_string(),
                p2_name: name_j.to_string(),
                total_games: battles_per_pair,
                p1_wins: i_wins,
                p2_wins: j_wins,
                p1_win_rate: i_rate,
                p2_win_rate: j_rate,
            });
        }
    }

    // 3. Build 13x13 Win Rate Matrix (percentages)
    let mut matrix = vec![vec![50.0; NUM_STRATEGIES]; NUM_STRATEGIES];
    for i in 0..NUM_STRATEGIES {
        for j in 0..NUM_STRATEGIES {
            if i != j {
                matrix[i][j] = (win_counts[i][j] as f64 / battles_per_pair as f64) * 100.0;
            }
        }
    }

    // 4. Build Tournament Standings
    let mut standings = Vec::with_capacity(NUM_STRATEGIES);
    for i in 0..NUM_STRATEGIES {
        let (_, id, name) = create_strategy_instance(i);
        let wins: usize = (0..NUM_STRATEGIES)
            .filter(|&j| j != i)
            .map(|j| win_counts[i][j])
            .sum();
        let total_matches = (NUM_STRATEGIES - 1) * battles_per_pair;
        let losses = total_matches - wins;
        let win_rate = wins as f64 / total_matches as f64;
        standings.push(TournamentStanding {
            rank: 0,
            id: id.to_string(),
            name: name.to_string(),
            total_matches,
            wins,
            losses,
            win_rate,
        });
    }
    standings.sort_by(|a, b| {
        b.win_rate
            .partial_cmp(&a.win_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (r, s) in standings.iter_mut().enumerate() {
        s.rank = r + 1;
    }

    let strategy_ids: Vec<String> = (0..NUM_STRATEGIES)
        .map(|i| create_strategy_instance(i).1.to_string())
        .collect();
    let strategy_names: Vec<String> = (0..NUM_STRATEGIES)
        .map(|i| create_strategy_instance(i).2.to_string())
        .collect();

    let tournament = HeadToHeadMatrix {
        strategy_ids: strategy_ids.clone(),
        strategy_names: strategy_names.clone(),
        matrix,
        pairings,
        standings,
    };

    // 5. Complete 7 Defenses x 13 Attackers Survival Matrix (91 cells)
    let cell_games = (games / 10).max(10).min(50);
    let mut survival_matrix = vec![vec![0.0; NUM_STRATEGIES]; NUM_DEFENSES];
    let mut defense_stats = Vec::with_capacity(NUM_DEFENSES);

    for d in 0..NUM_DEFENSES {
        let (mut def, def_id, def_name) = create_defense_instance(d);
        let mut all_turns_for_d = Vec::with_capacity(NUM_STRATEGIES * cell_games);

        for a in 0..NUM_STRATEGIES {
            let (mut att, _, _) = create_strategy_instance(a);
            let mut pair_turns = Vec::with_capacity(cell_games);
            for _ in 0..cell_games {
                let turns = simulate_game_with_ruleset(att.as_mut(), def.as_mut(), ruleset, &mut rng);
                pair_turns.push(turns);
                all_turns_for_d.push(turns);
            }
            let mean_turns = pair_turns.iter().sum::<usize>() as f64 / cell_games as f64;
            survival_matrix[d][a] = mean_turns;
        }

        defense_stats.push(compute_stats(def_id, def_name, all_turns_for_d));
    }

    // 6. Complete 7x7 Defense Head-to-Head Tournament (21 distinct pairings)
    let rounds_per_attacker = (games / 50).max(2).min(10);
    let duels_per_pair = rounds_per_attacker * NUM_STRATEGIES;
    let mut def_win_counts = vec![vec![0usize; NUM_DEFENSES]; NUM_DEFENSES];
    let num_def_pairings = NUM_DEFENSES * (NUM_DEFENSES - 1) / 2;
    let mut def_pairings = Vec::with_capacity(num_def_pairings);

    for i in 0..NUM_DEFENSES {
        for j in (i + 1)..NUM_DEFENSES {
            let (_, def_id_i, def_name_i) = create_defense_instance(i);
            let (_, def_id_j, def_name_j) = create_defense_instance(j);
            let mut i_wins = 0;
            let mut j_wins = 0;

            for k in 0..NUM_STRATEGIES {
                for r in 0..rounds_per_attacker {
                    let (mut att1, _, _) = create_strategy_instance(k);
                    let (mut att2, _, _) = create_strategy_instance(k);
                    let (mut def_i, _, _) = create_defense_instance(i);
                    let (mut def_j, _, _) = create_defense_instance(j);
                    let match_seed = rng.next_u64();

                    if r % 2 == 0 {
                        let replay = simulate_match_with_ruleset(
                            def_name_i,
                            att1.as_mut(),
                            def_i.as_mut(),
                            def_name_j,
                            att2.as_mut(),
                            def_j.as_mut(),
                            ruleset,
                            match_seed,
                        );
                        if replay.winner == Player::Player1 {
                            i_wins += 1;
                        } else {
                            j_wins += 1;
                        }
                    } else {
                        let replay = simulate_match_with_ruleset(
                            def_name_j,
                            att2.as_mut(),
                            def_j.as_mut(),
                            def_name_i,
                            att1.as_mut(),
                            def_i.as_mut(),
                            ruleset,
                            match_seed,
                        );
                        if replay.winner == Player::Player1 {
                            j_wins += 1;
                        } else {
                            i_wins += 1;
                        }
                    }
                }
            }

            def_win_counts[i][j] = i_wins;
            def_win_counts[j][i] = j_wins;
            let i_rate = i_wins as f64 / duels_per_pair as f64;
            let j_rate = j_wins as f64 / duels_per_pair as f64;

            def_pairings.push(PairwiseMatchResult {
                p1_id: def_id_i.to_string(),
                p1_name: def_name_i.to_string(),
                p2_id: def_id_j.to_string(),
                p2_name: def_name_j.to_string(),
                total_games: duels_per_pair,
                p1_wins: i_wins,
                p2_wins: j_wins,
                p1_win_rate: i_rate,
                p2_win_rate: j_rate,
            });
        }
    }

    // 7. Build 7x7 Defense Win Rate Matrix (percentages)
    let mut def_matrix = vec![vec![50.0; NUM_DEFENSES]; NUM_DEFENSES];
    for i in 0..NUM_DEFENSES {
        for j in 0..NUM_DEFENSES {
            if i != j {
                def_matrix[i][j] = (def_win_counts[i][j] as f64 / duels_per_pair as f64) * 100.0;
            }
        }
    }

    // 8. Build Defense Tournament Standings
    let mut def_standings = Vec::with_capacity(NUM_DEFENSES);
    for i in 0..NUM_DEFENSES {
        let (_, id, name) = create_defense_instance(i);
        let wins: usize = (0..NUM_DEFENSES)
            .filter(|&j| j != i)
            .map(|j| def_win_counts[i][j])
            .sum();
        let total_matches = (NUM_DEFENSES - 1) * duels_per_pair;
        let losses = total_matches - wins;
        let win_rate = wins as f64 / total_matches as f64;
        def_standings.push(TournamentStanding {
            rank: 0,
            id: id.to_string(),
            name: name.to_string(),
            total_matches,
            wins,
            losses,
            win_rate,
        });
    }
    def_standings.sort_by(|a, b| {
        b.win_rate
            .partial_cmp(&a.win_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (r, s) in def_standings.iter_mut().enumerate() {
        s.rank = r + 1;
    }

    let def_ids: Vec<String> = (0..NUM_DEFENSES)
        .map(|i| create_defense_instance(i).1.to_string())
        .collect();
    let def_names: Vec<String> = (0..NUM_DEFENSES)
        .map(|i| create_defense_instance(i).2.to_string())
        .collect();

    let def_tournament = HeadToHeadMatrix {
        strategy_ids: def_ids.clone(),
        strategy_names: def_names.clone(),
        matrix: def_matrix,
        pairings: def_pairings,
        standings: def_standings,
    };

    let defense_eval = DefenseEvaluation {
        defense_ids: def_ids,
        defense_names: def_names,
        attacker_ids: strategy_ids,
        attacker_names: strategy_names,
        survival_matrix,
        defense_stats,
        tournament: def_tournament,
    };

    // Classic P1 (Random Hunt) vs P2 (Checkerboard) summary
    let p1_wins = win_counts[0][1];
    let p2_wins = win_counts[1][0];
    let h2h = HeadToHeadStats {
        p1_name: "Random + Hunt/Target".to_string(),
        p2_name: "Checkerboard + Hunt/Target".to_string(),
        total_games: battles_per_pair,
        p1_wins,
        p2_wins,
        p1_win_rate: p1_wins as f64 / battles_per_pair as f64,
        p2_win_rate: p2_wins as f64 / battles_per_pair as f64,
    };

    BenchmarkReport {
        ruleset,
        games_per_strategy: games,
        seed,
        strategies,
        head_to_head: h2h,
        tournament,
        defense_eval,
    }
}

/// Backward-compatible run_benchmark using Ruleset::Standard.
pub fn run_benchmark(games: usize, seed: u64) -> BenchmarkReport {
    run_benchmark_with_ruleset(games, seed, Ruleset::Standard)
}

// =========================================================================
// Pure Rust Hand-Written JSON Serializers
// =========================================================================

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn ship_placement_to_json(p: &ShipPlacement) -> String {
    let cells_objs: Vec<String> = p
        .cells()
        .into_iter()
        .map(|(r, c)| format!("{{\"r\":{},\"c\":{}}}", r, c))
        .collect();
    let cells_coords: Vec<String> = p
        .cells()
        .into_iter()
        .map(|(r, c)| format!("[{},{}]", r, c))
        .collect();
    let id = p.kind.name().to_ascii_lowercase();
    format!(
        "{{\"id\":{},\"kind\":{},\"name\":{},\"size\":{},\"length\":{},\"hits\":0,\"sunk\":false,\"row\":{},\"col\":{},\"orientation\":{},\"cells\":[{}],\"coordinates\":[{}]}}",
        json_str(&id),
        json_str(p.kind.name()),
        json_str(p.kind.name()),
        p.kind.length(),
        p.kind.length(),
        p.row,
        p.col,
        json_str(p.orientation.name().to_ascii_lowercase().as_str()),
        cells_objs.join(","),
        cells_coords.join(",")
    )
}

fn move_record_to_json(m: &MoveRecord) -> String {
    let sunk_str = match m.outcome {
        ShotOutcome::Sunk(kind) => json_str(kind.name()),
        _ => "null".to_string(),
    };
    let result_str = match m.outcome {
        ShotOutcome::Miss => "MISS",
        ShotOutcome::Hit => "HIT",
        ShotOutcome::Sunk(_) => "SUNK",
    };
    let sunk_cells_json = match &m.sunk_cells {
        Some(cells) => {
            let objs: Vec<String> = cells
                .iter()
                .map(|(r, c)| format!("{{\"r\":{},\"c\":{}}}", r, c))
                .collect();
            format!("[{}]", objs.join(","))
        }
        None => "null".to_string(),
    };
    let ai_mode = if m.mode.to_ascii_lowercase().contains("target") {
        "TARGET"
    } else {
        "HUNT"
    };
    let player_num = if m.player == Player::Player1 { 1 } else { 2 };

    format!(
        "{{\"turn\":{},\"round\":{},\"player\":{},\"player_num\":{},\"row\":{},\"col\":{},\"coord\":{},\"result\":{},\"outcome\":{},\"ship\":{},\"sunk_ship\":{},\"sunk_cells\":{},\"ai_mode\":{},\"mode\":{},\"salvo_shot\":{},\"salvo_size\":{},\"p1_stats\":{{\"shots\":{},\"hits\":{},\"misses\":{},\"afloat\":{},\"mode\":\"HUNT\",\"queue_len\":0}},\"p2_stats\":{{\"shots\":{},\"hits\":{},\"misses\":{},\"afloat\":{},\"mode\":\"HUNT\",\"queue_len\":0}}}}",
        m.turn,
        m.round,
        player_num,
        player_num,
        m.row,
        m.col,
        json_str(&m.coord),
        json_str(result_str),
        json_str(m.outcome.as_str()),
        sunk_str,
        sunk_str,
        sunk_cells_json,
        json_str(ai_mode),
        json_str(&m.mode),
        m.salvo_shot,
        m.salvo_size,
        m.p1_shots,
        m.p1_hits,
        m.p1_misses,
        m.p1_afloat,
        m.p2_shots,
        m.p2_hits,
        m.p2_misses,
        m.p2_afloat
    )
}

impl MatchReplay {
    pub fn to_json(&self) -> String {
        let p1_ships: Vec<String> = self.player1_ships.iter().map(ship_placement_to_json).collect();
        let p2_ships: Vec<String> = self.player2_ships.iter().map(ship_placement_to_json).collect();
        let moves_json: Vec<String> = self.moves.iter().map(move_record_to_json).collect();
        let winner_num = if self.winner == Player::Player1 { 1 } else { 2 };
        let p1_strat = if self.player1_name.to_ascii_lowercase().contains("checker") {
            "checkerboard"
        } else {
            "random"
        };
        let p2_strat = if self.player2_name.to_ascii_lowercase().contains("checker") {
            "checkerboard"
        } else {
            "random"
        };

        format!(
            "{{\"seed\":{},\"ruleset\":{},\"ruleset_name\":{},\"player1_name\":{},\"player2_name\":{},\"p1_strategy\":{},\"p2_strategy\":{},\"winner\":{},\"winner_name\":{},\"total_rounds\":{},\"total_moves\":{},\"total_turns\":{},\"p1_fleet\":[{}],\"p2_fleet\":[{}],\"player1_ships\":[{}],\"player2_ships\":[{}],\"turns\":[{}],\"moves\":[{}]}}",
            self.seed,
            json_str(self.ruleset.id()),
            json_str(self.ruleset.name()),
            json_str(&self.player1_name),
            json_str(&self.player2_name),
            json_str(p1_strat),
            json_str(p2_strat),
            winner_num,
            json_str(self.winner.as_str()),
            self.total_rounds,
            self.total_moves,
            self.total_moves,
            p1_ships.join(","),
            p2_ships.join(","),
            p1_ships.join(","),
            p2_ships.join(","),
            moves_json.join(","),
            moves_json.join(",")
        )
    }
}

impl StrategyStats {
    pub fn to_json(&self) -> String {
        let hist_json: Vec<String> = self
            .histogram
            .iter()
            .map(|(t, c)| format!("{{\"turns\":{},\"count\":{}}}", t, c))
            .collect();
        let dist_entries: Vec<String> = self
            .histogram
            .iter()
            .map(|(t, c)| format!("\"{}\":{}", t, c))
            .collect();

        format!(
            "{{\"id\":{},\"name\":{},\"games\":{},\"mean\":{:.2},\"median\":{:.1},\"min\":{},\"max\":{},\"std_dev\":{:.2},\"stdDev\":{:.2},\"p10\":{:.1},\"p90\":{:.1},\"distribution\":{{{}}},\"histogram\":[{}]}}",
            json_str(&self.id),
            json_str(&self.name),
            self.games,
            self.mean,
            self.median,
            self.min,
            self.max,
            self.std_dev,
            self.std_dev,
            self.p10,
            self.p90,
            dist_entries.join(","),
            hist_json.join(",")
        )
    }
}

fn pairwise_match_to_json(p: &PairwiseMatchResult) -> String {
    format!(
        "{{\"strategy_a\":{},\"strategy_b\":{},\"a_name\":{},\"b_name\":{},\"total_games\":{},\"a_wins\":{},\"b_wins\":{},\"a_win_rate\":{:.4},\"b_win_rate\":{:.4}}}",
        json_str(&p.p1_id),
        json_str(&p.p2_id),
        json_str(&p.p1_name),
        json_str(&p.p2_name),
        p.total_games,
        p.p1_wins,
        p.p2_wins,
        p.p1_win_rate,
        p.p2_win_rate
    )
}

fn tournament_standing_to_json(s: &TournamentStanding) -> String {
    format!(
        "{{\"rank\":{},\"id\":{},\"name\":{},\"total_matches\":{},\"wins\":{},\"losses\":{},\"win_rate\":{:.4}}}",
        s.rank,
        json_str(&s.id),
        json_str(&s.name),
        s.total_matches,
        s.wins,
        s.losses,
        s.win_rate
    )
}

impl HeadToHeadMatrix {
    pub fn to_json(&self) -> String {
        let ids_json: Vec<String> = self.strategy_ids.iter().map(|s| json_str(s)).collect();
        let names_json: Vec<String> = self.strategy_names.iter().map(|s| json_str(s)).collect();
        let matrix_json: Vec<String> = self
            .matrix
            .iter()
            .map(|row| {
                let row_str: Vec<String> = row.iter().map(|v| format!("{:.1}", v)).collect();
                format!("[{}]", row_str.join(","))
            })
            .collect();
        let pairings_json: Vec<String> = self.pairings.iter().map(pairwise_match_to_json).collect();
        let standings_json: Vec<String> =
            self.standings.iter().map(tournament_standing_to_json).collect();

        format!(
            "{{\"strategy_ids\":[{}],\"strategy_names\":[{}],\"matrix\":[{}],\"pairings\":[{}],\"standings\":[{}]}}",
            ids_json.join(","),
            names_json.join(","),
            matrix_json.join(","),
            pairings_json.join(","),
            standings_json.join(",")
        )
    }
}

impl DefenseEvaluation {
    pub fn to_json(&self) -> String {
        let def_ids_json: Vec<String> = self.defense_ids.iter().map(|s| json_str(s)).collect();
        let def_names_json: Vec<String> = self.defense_names.iter().map(|s| json_str(s)).collect();
        let att_ids_json: Vec<String> = self.attacker_ids.iter().map(|s| json_str(s)).collect();
        let att_names_json: Vec<String> = self.attacker_names.iter().map(|s| json_str(s)).collect();

        let surv_matrix_json: Vec<String> = self
            .survival_matrix
            .iter()
            .map(|row| {
                let row_str: Vec<String> = row.iter().map(|v| format!("{:.2}", v)).collect();
                format!("[{}]", row_str.join(","))
            })
            .collect();

        let stats_json: Vec<String> = self.defense_stats.iter().map(|s| s.to_json()).collect();

        let def_map_entries: Vec<String> = self
            .defense_stats
            .iter()
            .map(|s| format!("{}:{}", json_str(&s.id), s.to_json()))
            .collect();

        format!(
            "{{\"defense_ids\":[{}],\"defense_names\":[{}],\"attacker_ids\":[{}],\"attacker_names\":[{}],\"survival_matrix\":[{}],\"stats_by_id\":{{{}}},\"defense_stats\":[{}],\"tournament\":{}}}",
            def_ids_json.join(","),
            def_names_json.join(","),
            att_ids_json.join(","),
            att_names_json.join(","),
            surv_matrix_json.join(","),
            def_map_entries.join(","),
            stats_json.join(","),
            self.tournament.to_json()
        )
    }
}

impl BenchmarkReport {
    pub fn to_json(&self) -> String {
        let s: Vec<String> = (0..NUM_STRATEGIES)
            .map(|i| {
                self.strategies
                    .get(i)
                    .map(|st| st.to_json())
                    .unwrap_or_else(|| "{}".to_string())
            })
            .collect();

        let strats_list: Vec<String> = self.strategies.iter().map(|st| st.to_json()).collect();

        let p1_rate_pct = self.head_to_head.p1_win_rate * 100.0;
        let p2_rate_pct = self.head_to_head.p2_win_rate * 100.0;

        let def_eval_json = self.defense_eval.to_json();
        let def_tourn_json = self.defense_eval.tournament.to_json();

        format!(
            "{{\"ruleset\":{},\"ruleset_name\":{},\"games\":{},\"games_per_strategy\":{},\"seed\":{},\"strategies\":{{\"random_hunt\":{},\"random\":{},\"checkerboard_hunt\":{},\"checkerboard\":{},\"largest_rect\":{},\"adaptive_stride\":{},\"stride4_to_2\":{},\"rect_stride4\":{},\"stride_5432\":{},\"rect_stride_5432\":{},\"rect_checkerboard\":{},\"monte_carlo\":{},\"bayesian_pdf\":{},\"crosshair_stride\":{},\"crosshair_5432\":{},\"strategy_list\":[{}]}},\"head_to_head\":{{\"p1_name\":{},\"p2_name\":{},\"total_games\":{},\"p1_wins\":{},\"p2_wins\":{},\"p1_win_rate\":{:.4},\"p2_win_rate\":{:.4},\"checkerboard_wins\":{},\"random_wins\":{},\"checkerboard_win_rate\":{:.1},\"random_win_rate\":{:.1}}},\"head_to_head_matrix\":{},\"tournament\":{},\"defense_eval\":{},\"defense_evaluation\":{},\"defense_tournament\":{}}}",
            json_str(self.ruleset.id()),
            json_str(self.ruleset.name()),
            self.games_per_strategy,
            self.games_per_strategy,
            self.seed,
            s[0],
            s[0],
            s[1],
            s[1],
            s[2],
            s[3],
            s[4],
            s[5],
            s[6],
            s[7],
            s[8],
            s[9],
            s[10],
            s[11],
            s[12],
            strats_list.join(","),
            json_str(&self.head_to_head.p1_name),
            json_str(&self.head_to_head.p2_name),
            self.head_to_head.total_games,
            self.head_to_head.p1_wins,
            self.head_to_head.p2_wins,
            self.head_to_head.p1_win_rate,
            self.head_to_head.p2_win_rate,
            self.head_to_head.p2_wins,
            self.head_to_head.p1_wins,
            p2_rate_pct,
            p1_rate_pct,
            self.tournament.to_json(),
            self.tournament.to_json(),
            def_eval_json,
            def_eval_json,
            def_tourn_json
        )
    }
}

/// Print formatted table & ASCII histogram to stdout.
pub fn print_benchmark_summary(report: &BenchmarkReport) {
    println!();
    println!("=========================================================================================================");
    println!("                      BATTLESHIP ENGINE BENCHMARK REPORT - {:<36}", report.ruleset.name().to_uppercase());
    println!("=========================================================================================================");
    let battles_count = report
        .tournament
        .pairings
        .first()
        .map(|p| p.total_games)
        .unwrap_or(0);
    println!(
        "Ruleset: {} | Games: {} solo per strategy | Tournament: {} duels per pair | Seed: {}",
        report.ruleset.name(), report.games_per_strategy, battles_count, report.seed
    );
    println!();
    println!("ATTACK STRATEGY PERFORMANCE (Turns to sink all 17 ship cells):");
    println!("+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+");
    println!("| Strategy                          | Mean  | Median | Min | Max | StDev | P10 | P90 |");
    println!("+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+");
    for s in &report.strategies {
        println!(
            "| {:<33} | {:>5.1} | {:>6.1} | {:>3} | {:>3} | {:>5.2} | {:>3.0} | {:>3.0} |",
            s.name, s.mean, s.median, s.min, s.max, s.std_dev, s.p10, s.p90
        );
    }
    println!("+-----------------------------------+-------+--------+-----+-----+-------+-----+-----+");
    println!();
    println!("=========================================================================================================");
    println!("                             13x13 ATTACK HEAD-TO-HEAD WIN RATE MATRIX (%)                               ");
    println!("=========================================================================================================");
    println!("Columns indicate opponent. Value is row strategy's win % against column.");
    for (i, name) in report.tournament.strategy_names.iter().enumerate() {
        if i % 2 == 0 {
            print!("  [{:>2}] {:<33}", i + 1, name);
        } else {
            println!("[{:>2}] {}", i + 1, name);
        }
    }
    if report.tournament.strategy_names.len() % 2 != 0 {
        println!();
    }
    println!();
    println!("+-----------------------------------+------+------+------+------+------+------+------+------+------+------+------+------+------+");
    println!("| Strategy                          |  [1] |  [2] |  [3] |  [4] |  [5] |  [6] |  [7] |  [8] |  [9] | [10] | [11] | [12] | [13] |");
    println!("+-----------------------------------+------+------+------+------+------+------+------+------+------+------+------+------+------+");
    for (i, name) in report.tournament.strategy_names.iter().enumerate() {
        print!("| [{:>2}] {:<29} |", i + 1, name);
        for j in 0..report.tournament.matrix.len() {
            if i == j {
                print!("  --- |");
            } else {
                let rate = report.tournament.matrix[i][j];
                print!(" {:>4.0}%|", rate);
            }
        }
        println!();
    }
    println!("+-----------------------------------+------+------+------+------+------+------+------+------+------+------+------+------+------+");
    println!();
    println!("=========================================================================================================");
    println!("                          OVERALL ATTACK TOURNAMENT STANDINGS (ALL-PAIRS DUELS)                          ");
    println!("=========================================================================================================");
    println!("+------+-----------------------------------+---------+-------+--------+---------------+");
    println!("| Rank | Strategy                          | Matches |  Wins | Losses | Overall Win % |");
    println!("+------+-----------------------------------+---------+-------+--------+---------------+");
    for s in &report.tournament.standings {
        println!(
            "| {:>4} | {:<33} | {:>7} | {:>5} | {:>6} | {:>12.1}% |",
            s.rank, s.name, s.total_matches, s.wins, s.losses, s.win_rate * 100.0
        );
    }
    println!("+------+-----------------------------------+---------+-------+--------+---------------+");
    println!();
    println!("=========================================================================================================");
    println!("            7 DEFENSES x 13 ATTACKERS SURVIVAL MATRIX (Mean Turns Survived - Higher is Better)           ");
    println!("=========================================================================================================");
    println!("Values indicate mean turns survived by defense against each attacker. Higher values = tougher defense.");
    for (i, name) in report.defense_eval.defense_names.iter().enumerate() {
        if i % 2 == 0 {
            print!("  [D{:>1}] {:<34}", i + 1, name);
        } else {
            println!("[D{:>1}] {}", i + 1, name);
        }
    }
    if report.defense_eval.defense_names.len() % 2 != 0 {
        println!();
    }
    println!();
    println!("+-----------------------------------+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+---------+");
    println!("| Defense Strategy                  | A1  | A2  | A3  | A4  | A5  | A6  | A7  | A8  | A9  | A10 | A11 | A12 | A13 | Overall |");
    println!("+-----------------------------------+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+---------+");
    for (d, name) in report.defense_eval.defense_names.iter().enumerate() {
        print!("| [D{}] {:<30} |", d + 1, name);
        for a in 0..report.defense_eval.survival_matrix[d].len() {
            let val = report.defense_eval.survival_matrix[d][a];
            print!(" {:>3.0} |", val);
        }
        let overall = report.defense_eval.defense_stats[d].mean;
        println!("  {:>5.1}  |", overall);
    }
    println!("+-----------------------------------+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+-----+---------+");
    println!();
    println!("=========================================================================================================");
    println!("                                7x7 DEFENSE TOURNAMENT WIN RATE MATRIX (%)                               ");
    println!("=========================================================================================================");
    println!("Columns indicate opponent. Value is row defense's win % against column when both face identical attackers.");
    println!("+-----------------------------------+------+------+------+------+------+------+------+");
    println!("| Defense Strategy                  | [D1] | [D2] | [D3] | [D4] | [D5] | [D6] | [D7] |");
    println!("+-----------------------------------+------+------+------+------+------+------+------+");
    for (i, name) in report.defense_eval.tournament.strategy_names.iter().enumerate() {
        print!("| [D{}] {:<29} |", i + 1, name);
        for j in 0..report.defense_eval.tournament.matrix.len() {
            if i == j {
                print!("  --- |");
            } else {
                let rate = report.defense_eval.tournament.matrix[i][j];
                print!(" {:>4.0}%|", rate);
            }
        }
        println!();
    }
    println!("+-----------------------------------+------+------+------+------+------+------+------+");
    println!();
    println!("=========================================================================================================");
    println!("                             OVERALL DEFENSE TOURNAMENT STANDINGS                                        ");
    println!("=========================================================================================================");
    println!("+------+-----------------------------------+---------+-------+--------+---------------+---------------+");
    println!("| Rank | Defense Strategy                  | Matches |  Wins | Losses | Overall Win % | Mean Survived |");
    println!("+------+-----------------------------------+---------+-------+--------+---------------+---------------+");
    for s in &report.defense_eval.tournament.standings {
        let mean_surv = report
            .defense_eval
            .defense_stats
            .iter()
            .find(|ds| ds.id == s.id)
            .map(|ds| ds.mean)
            .unwrap_or(0.0);
        println!(
            "| {:>4} | {:<33} | {:>7} | {:>5} | {:>6} | {:>12.1}% | {:>9.1} turns |",
            s.rank, s.name, s.total_matches, s.wins, s.losses, s.win_rate * 100.0, mean_surv
        );
    }
    println!("+------+-----------------------------------+---------+-------+--------+---------------+---------------+");
    println!();
    println!("ASCII TURNS DISTRIBUTION (5-turn buckets):");
    for strat in &report.strategies {
        println!("  Strategy: {}", strat.name);
        let mut buckets = vec![(20, 24, 0); 16];
        for (i, b) in buckets.iter_mut().enumerate() {
            let start = 20 + i * 5;
            let end = start + 4;
            let count = strat
                .histogram
                .iter()
                .filter(|(t, _)| *t >= start && *t <= end)
                .map(|(_, c)| *c)
                .sum();
            *b = (start, end, count);
        }
        let max_bucket_count = buckets.iter().map(|(_, _, c)| *c).max().unwrap_or(1).max(1);
        let max_bar_width = 40;

        for (start, end, count) in buckets {
            if count > 0 || (start >= strat.min && end <= strat.max + 4) {
                let bar_len = ((count as f64 / max_bucket_count as f64) * max_bar_width as f64).round() as usize;
                let bar = "█".repeat(bar_len);
                println!("    [{:>2}-{:>2}] {:<40} {:>4}", start, end, bar, count);
            }
        }
        println!();
    }
    println!("=========================================================================================================");
}

/// Print clean ASCII comparison tables of Solo Mean Shots & Tournament Win% across rulesets.
pub fn print_cross_ruleset_summary(reports: &[BenchmarkReport]) {
    if reports.is_empty() {
        return;
    }
    println!();
    println!("=========================================================================================================");
    println!("                               CROSS-RULESET BENCHMARK COMPARISON SUMMARY                                ");
    println!("=========================================================================================================");
    println!();
    println!("ATTACK STRATEGIES CROSS-RULESET PERFORMANCE (Solo Mean Shots & Tournament Win %):");

    let mut att_div = String::from("+----+-----------------------------------+");
    let mut att_h1 = String::from("| #  | Strategy                          |");
    let mut att_h2 = String::from("|    |                                   |");
    for r in reports {
        att_div.push_str("---------+----------+");
        att_h1.push_str(&format!(" {:^18} |", r.ruleset.name()));
        att_h2.push_str("  Mean   |  Win %   |");
    }

    println!("{}", att_div);
    println!("{}", att_h1);
    println!("{}", att_h2);
    println!("{}", att_div);

    for i in 0..NUM_STRATEGIES {
        let name = &reports[0].strategies[i].name;
        print!("| {:>2} | {:<33} |", i + 1, name);
        for r in reports {
            let mean = r.strategies[i].mean;
            let win_rate = r
                .tournament
                .standings
                .iter()
                .find(|s| s.id == r.strategies[i].id)
                .map(|s| s.win_rate * 100.0)
                .unwrap_or(0.0);
            print!("  {:>5.1}  |  {:>5.1}%  |", mean, win_rate);
        }
        println!();
    }
    println!("{}", att_div);
    println!();

    println!("DEFENSE STRATEGIES CROSS-RULESET PERFORMANCE (Survival Mean & Tournament Win %):");

    let mut def_div = String::from("+-----+-----------------------------------+");
    let mut def_h1 = String::from("|  #  | Defense Strategy                  |");
    let mut def_h2 = String::from("|     |                                   |");
    for r in reports {
        def_div.push_str("---------+----------+");
        def_h1.push_str(&format!(" {:^18} |", r.ruleset.name()));
        def_h2.push_str("  Surv   |  Win %   |");
    }

    println!("{}", def_div);
    println!("{}", def_h1);
    println!("{}", def_h2);
    println!("{}", def_div);

    for d in 0..NUM_DEFENSES {
        let name = &reports[0].defense_eval.defense_names[d];
        let def_id = &reports[0].defense_eval.defense_ids[d];
        print!("| [D{}] | {:<33} |", d + 1, name);
        for r in reports {
            let surv = r.defense_eval.defense_stats[d].mean;
            let win_rate = r
                .defense_eval
                .tournament
                .standings
                .iter()
                .find(|s| s.id == *def_id)
                .map(|s| s.win_rate * 100.0)
                .unwrap_or(0.0);
            print!("  {:>5.1}  |  {:>5.1}%  |", surv, win_rate);
        }
        println!();
    }
    println!("{}", def_div);
    println!("=========================================================================================================");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_player_simulation() {
        let mut rng = Rng::with_seed(123);
        let mut attack = CheckerboardAttack::new();
        let mut defense = RandomDefense::new();

        let turns = simulate_game(&mut attack, &mut defense, &mut rng);
        assert!(turns >= TOTAL_SHIP_CELLS, "Must take at least 17 turns");
        assert!(turns <= 100, "Must take at most 100 turns");
    }

    #[test]
    fn test_simulate_match_completion() {
        let mut p1_att = HuntAndTargetAttack::new();
        let mut p1_def = RandomDefense::new();
        let mut p2_att = CheckerboardAttack::new();
        let mut p2_def = RandomDefense::new();

        let replay = simulate_match(
            "P1", &mut p1_att, &mut p1_def, "P2", &mut p2_att, &mut p2_def, 42,
        );

        assert_eq!(replay.player1_ships.len(), 5);
        assert_eq!(replay.player2_ships.len(), 5);
        assert!(!replay.moves.is_empty());
        assert!(replay.total_rounds > 0);
        assert!(replay.total_moves > 0);

        let json = replay.to_json();
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        assert!(json.contains("\"player1_ships\""));
        assert!(json.contains("\"player2_ships\""));
        assert!(json.contains("\"moves\""));
    }

    #[test]
    fn test_benchmark_checkerboard_outperforms_random_hunt() {
        let report = run_benchmark(20, 888);
        assert_eq!(report.strategies.len(), 13);
        assert_eq!(report.tournament.matrix.len(), 13);
        assert_eq!(report.tournament.standings.len(), 13);
        assert_eq!(report.tournament.pairings.len(), 78);

        assert_eq!(report.defense_eval.survival_matrix.len(), 7);
        assert_eq!(report.defense_eval.survival_matrix[0].len(), 13);
        assert_eq!(report.defense_eval.tournament.matrix.len(), 7);
        assert_eq!(report.defense_eval.tournament.standings.len(), 7);
        assert_eq!(report.defense_eval.tournament.pairings.len(), 21);

        let random_hunt_mean = report.strategies[0].mean;
        let checkerboard_mean = report.strategies[1].mean;

        assert!(
            checkerboard_mean < random_hunt_mean,
            "Checkerboard mean ({:.2}) should be lower than Random Hunt mean ({:.2})",
            checkerboard_mean,
            random_hunt_mean
        );

        let json = report.to_json();
        assert!(json.contains("\"head_to_head\""));
        assert!(json.contains("\"strategies\""));
        assert!(json.contains("\"tournament\""));
        assert!(json.contains("\"head_to_head_matrix\""));
        assert!(json.contains("\"bayesian_pdf\""));
        assert!(json.contains("\"monte_carlo\""));
        assert!(json.contains("\"crosshair_stride\""));
        assert!(json.contains("\"crosshair_5432\""));
        assert!(json.contains("\"defense_evaluation\""));
        assert!(json.contains("\"survival_matrix\""));
        assert!(json.contains("\"defense_tournament\""));
    }

    #[test]
    fn test_salvo_match_dynamic_shrink_and_records() {
        let mut p1_att = CrosshairStrideAttack::new();
        let mut p1_def = RandomDefense::new();
        let mut p2_att = BayesianPdfAttack::new();
        let mut p2_def = RandomDefense::new();

        let replay = simulate_match_with_ruleset(
            "SalvoP1",
            &mut p1_att,
            &mut p1_def,
            "SalvoP2",
            &mut p2_att,
            &mut p2_def,
            Ruleset::Salvo,
            333,
        );

        assert_eq!(replay.ruleset, Ruleset::Salvo);
        assert!(!replay.moves.is_empty());
        // Verify salvo_shot and salvo_size fields
        for m in &replay.moves {
            assert!(m.salvo_shot >= 1);
            assert!(m.salvo_size >= 1 && m.salvo_size <= 5);
            assert!(m.salvo_shot <= m.salvo_size);
        }

        let json = replay.to_json();
        assert!(json.contains("\"ruleset\":\"salvo\""));
        assert!(json.contains("\"ruleset_name\":\"Salvo Mode\""));
        assert!(json.contains("\"salvo_shot\":"));
        assert!(json.contains("\"salvo_size\":"));
    }

    #[test]
    fn test_match_with_silent_sinking() {
        let mut p1_att = MonteCarloAttack::new();
        let mut p1_def = ClusteredDefense::new();
        let mut p2_att = CheckerboardAttack::new();
        let mut p2_def = SpacedDefense::new();

        let replay = simulate_match_with_ruleset(
            "SilentP1",
            &mut p1_att,
            &mut p1_def,
            "SilentP2",
            &mut p2_att,
            &mut p2_def,
            Ruleset::SilentSinking,
            444,
        );

        assert_eq!(replay.ruleset, Ruleset::SilentSinking);
        for m in &replay.moves {
            assert!(!matches!(m.outcome, ShotOutcome::Sunk(_)));
            assert!(m.sunk_cells.is_none());
        }

        let json = replay.to_json();
        assert!(json.contains("\"ruleset\":\"silent\""));
        assert!(json.contains("\"ruleset_name\":\"Silent Sinking\""));
    }

    #[test]
    fn test_json_serialization_ruleset_fields() {
        let report = run_benchmark_with_ruleset(5, 777, Ruleset::SilentSinking);
        let json = report.to_json();
        assert!(json.contains("\"ruleset\":\"silent\""));
        assert!(json.contains("\"ruleset_name\":\"Silent Sinking\""));
    }

    #[test]
    fn test_salvo_silent_match_properties() {
        let mut p1_att = CrosshairStrideAttack::new();
        let mut p1_def = SpacedDefense::new();
        let mut p2_att = CrosshairDescendingAttack::new();
        let mut p2_def = BaitAndHideDefense::new();

        let replay = simulate_match_with_ruleset(
            "SalvoSilentP1",
            &mut p1_att,
            &mut p1_def,
            "SalvoSilentP2",
            &mut p2_att,
            &mut p2_def,
            Ruleset::SalvoSilent,
            555,
        );

        assert_eq!(replay.ruleset, Ruleset::SalvoSilent);
        assert!(!replay.moves.is_empty());
        for m in &replay.moves {
            assert!(!matches!(m.outcome, ShotOutcome::Sunk(_)));
            assert!(m.sunk_cells.is_none());
            assert!(m.salvo_shot >= 1);
            assert!(m.salvo_size >= 1 && m.salvo_size <= 5);
            assert!(m.salvo_shot <= m.salvo_size);
        }

        let json = replay.to_json();
        assert!(json.contains("\"ruleset\":\"salvo_silent\""));
        assert!(json.contains("\"ruleset_name\":\"Salvo + Fog of War\""));
    }
}
