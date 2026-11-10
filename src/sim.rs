//! Simulation engine, benchmarks, replay generation, and JSON serialization.
//!
//! Provides:
//! - Single-player evaluations (turns to win 17..=100)
//! - Head-to-head match simulation with full replay history
//! - Multi-game statistical benchmarking (mean, median, min, max, std dev, P10, P90, histograms)
//! - Hand-written pure std JSON serializer for replay and benchmark payloads
//! - Terminal-formatted summary tables and ASCII histograms

use crate::game::{
    MoveRecord, Player, ShipPlacement, ShotOutcome, TrackingBoard, TOTAL_SHIP_CELLS,
};
use crate::rng::Rng;
use crate::strategy::{
    AttackStrategy, CheckerboardAttack, DefenseStrategy, HuntAndTargetAttack, RandomDefense,
};

/// Helper function to format (row, col) as "A1".."J10".
pub fn coord_to_str(row: usize, col: usize) -> String {
    let row_char = (b'A' + (row as u8).min(25)) as char;
    format!("{}{}", row_char, col + 1)
}

/// Complete recording of a head-to-head match between two players.
#[derive(Clone, Debug)]
pub struct MatchReplay {
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

/// Comprehensive benchmark report.
#[derive(Clone, Debug)]
pub struct BenchmarkReport {
    pub games_per_strategy: usize,
    pub seed: u64,
    pub strategies: Vec<StrategyStats>,
    pub head_to_head: HeadToHeadStats,
}

/// Simulate a single player game where `attack` tries to sink all ships placed by `defense`.
/// Returns the number of turns taken (17..=100).
pub fn simulate_game(
    attack: &mut dyn AttackStrategy,
    defense: &mut dyn DefenseStrategy,
    rng: &mut Rng,
) -> usize {
    let mut board = defense.place_ships(rng);
    let mut view = TrackingBoard::new();
    attack.reset();

    let mut turns = 0;
    while !board.all_sunk() && turns < 100 {
        turns += 1;
        let pos = attack.choose_shot(&view, rng);
        let outcome = board.receive_shot(pos.0, pos.1);
        view.record_shot(pos.0, pos.1, outcome);
        attack.on_shot_result(pos, outcome, &view);
    }

    turns
}

/// Simulate a head-to-head match between Player 1 and Player 2.
/// Alternates shots until one player's fleet is completely sunk.
/// Returns complete `MatchReplay`.
pub fn simulate_match(
    p1_name: &str,
    p1_attack: &mut dyn AttackStrategy,
    p1_defense: &mut dyn DefenseStrategy,
    p2_name: &str,
    p2_attack: &mut dyn AttackStrategy,
    p2_defense: &mut dyn DefenseStrategy,
    seed: u64,
) -> MatchReplay {
    let mut rng = Rng::with_seed(seed);

    let mut p1_board = p1_defense.place_ships(&mut rng);
    let mut p2_board = p2_defense.place_ships(&mut rng);

    let p1_ships = p1_board.placements().to_vec();
    let p2_ships = p2_board.placements().to_vec();

    let mut p1_view = TrackingBoard::new();
    let mut p2_view = TrackingBoard::new();

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

    loop {
        round += 1;

        // Player 1 fires at Player 2's board
        turn_seq += 1;
        p1_shots += 1;
        let p1_mode = p1_attack.current_mode().to_string();
        let p1_shot = p1_attack.choose_shot(&p1_view, &mut rng);
        let p1_outcome = p2_board.receive_shot(p1_shot.0, p1_shot.1);
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
        let p2_outcome = p1_board.receive_shot(p2_shot.0, p2_shot.1);
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

    let total_moves = moves.len();
    MatchReplay {
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

/// Execute a full benchmark over `games` iterations:
/// 1. Evaluates Random + Hunt/Target solo
/// 2. Evaluates Checkerboard + Hunt/Target solo
/// 3. Simulates head-to-head matches between them
pub fn run_benchmark(games: usize, seed: u64) -> BenchmarkReport {
    let mut rng = Rng::with_seed(seed);
    let mut defense = RandomDefense::new();

    // 1. Evaluate HuntAndTargetAttack
    let mut strat1 = HuntAndTargetAttack::new();
    let mut turns1 = Vec::with_capacity(games);
    for _ in 0..games {
        turns1.push(simulate_game(&mut strat1, &mut defense, &mut rng));
    }
    let stats1 = compute_stats("random_hunt", strat1.name(), turns1);

    // 2. Evaluate CheckerboardAttack
    let mut strat2 = CheckerboardAttack::new();
    let mut turns2 = Vec::with_capacity(games);
    for _ in 0..games {
        turns2.push(simulate_game(&mut strat2, &mut defense, &mut rng));
    }
    let stats2 = compute_stats("checkerboard", strat2.name(), turns2);

    // 3. Head-to-Head Matches
    let mut p1_wins = 0;
    let mut p2_wins = 0;
    let mut def1 = RandomDefense::new();
    let mut def2 = RandomDefense::new();

    for i in 0..games {
        let match_seed = rng.next_u64();
        // Alternate starting player each match for perfect fairness
        let replay = if i % 2 == 0 {
            simulate_match(
                "Random + Hunt/Target",
                &mut strat1,
                &mut def1,
                "Checkerboard + Hunt/Target",
                &mut strat2,
                &mut def2,
                match_seed,
            )
        } else {
            let rev = simulate_match(
                "Checkerboard + Hunt/Target",
                &mut strat2,
                &mut def2,
                "Random + Hunt/Target",
                &mut strat1,
                &mut def1,
                match_seed,
            );
            // Translate winner back
            MatchReplay {
                winner: if rev.winner == Player::Player1 {
                    Player::Player2
                } else {
                    Player::Player1
                },
                ..rev
            }
        };

        if replay.winner == Player::Player1 {
            p1_wins += 1;
        } else {
            p2_wins += 1;
        }
    }

    let h2h = HeadToHeadStats {
        p1_name: "Random + Hunt/Target".to_string(),
        p2_name: "Checkerboard + Hunt/Target".to_string(),
        total_games: games,
        p1_wins,
        p2_wins,
        p1_win_rate: p1_wins as f64 / games as f64,
        p2_win_rate: p2_wins as f64 / games as f64,
    };

    BenchmarkReport {
        games_per_strategy: games,
        seed,
        strategies: vec![stats1, stats2],
        head_to_head: h2h,
    }
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
        "{{\"turn\":{},\"round\":{},\"player\":{},\"player_num\":{},\"row\":{},\"col\":{},\"coord\":{},\"result\":{},\"outcome\":{},\"ship\":{},\"sunk_ship\":{},\"sunk_cells\":{},\"ai_mode\":{},\"mode\":{},\"p1_stats\":{{\"shots\":{},\"hits\":{},\"misses\":{},\"afloat\":{},\"mode\":\"HUNT\",\"queue_len\":0}},\"p2_stats\":{{\"shots\":{},\"hits\":{},\"misses\":{},\"afloat\":{},\"mode\":\"HUNT\",\"queue_len\":0}}}}",
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
            "{{\"seed\":{},\"player1_name\":{},\"player2_name\":{},\"p1_strategy\":{},\"p2_strategy\":{},\"winner\":{},\"winner_name\":{},\"total_rounds\":{},\"total_moves\":{},\"total_turns\":{},\"p1_fleet\":[{}],\"p2_fleet\":[{}],\"player1_ships\":[{}],\"player2_ships\":[{}],\"turns\":[{}],\"moves\":[{}]}}",
            self.seed,
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

impl BenchmarkReport {
    pub fn to_json(&self) -> String {
        let strat1 = self
            .strategies
            .get(0)
            .map(|s| s.to_json())
            .unwrap_or_else(|| "{}".to_string());
        let strat2 = self
            .strategies
            .get(1)
            .map(|s| s.to_json())
            .unwrap_or_else(|| "{}".to_string());
        let strats_list: Vec<String> = self.strategies.iter().map(|s| s.to_json()).collect();

        let p1_rate_pct = self.head_to_head.p1_win_rate * 100.0;
        let p2_rate_pct = self.head_to_head.p2_win_rate * 100.0;

        format!(
            "{{\"games\":{},\"games_per_strategy\":{},\"seed\":{},\"strategies\":{{\"random_hunt\":{},\"random\":{},\"checkerboard_hunt\":{},\"checkerboard\":{},\"strategy_list\":[{}]}},\"head_to_head\":{{\"p1_name\":{},\"p2_name\":{},\"total_games\":{},\"p1_wins\":{},\"p2_wins\":{},\"p1_win_rate\":{:.4},\"p2_win_rate\":{:.4},\"checkerboard_wins\":{},\"random_wins\":{},\"checkerboard_win_rate\":{:.1},\"random_win_rate\":{:.1}}}}}",
            self.games_per_strategy,
            self.games_per_strategy,
            self.seed,
            strat1,
            strat1,
            strat2,
            strat2,
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
            p1_rate_pct
        )
    }
}

/// Print formatted table & ASCII histogram to stdout.
pub fn print_benchmark_summary(report: &BenchmarkReport) {
    println!();
    println!("================================================================================");
    println!("                     BATTLESHIP ENGINE BENCHMARK REPORT                         ");
    println!("================================================================================");
    println!(
        "Games simulated: {} per strategy | Base Seed: {}",
        report.games_per_strategy, report.seed
    );
    println!();
    println!("STRATEGY PERFORMANCE (Turns to sink all 17 ship cells):");
    println!("+----------------------------+-------+--------+-----+-----+-------+-----+-----+");
    println!("| Strategy                   | Mean  | Median | Min | Max | StDev | P10 | P90 |");
    println!("+----------------------------+-------+--------+-----+-----+-------+-----+-----+");
    for s in &report.strategies {
        println!(
            "| {:<26} | {:>5.1} | {:>6.1} | {:>3} | {:>3} | {:>5.2} | {:>3.0} | {:>3.0} |",
            s.name, s.mean, s.median, s.min, s.max, s.std_dev, s.p10, s.p90
        );
    }
    println!("+----------------------------+-------+--------+-----+-----+-------+-----+-----+");
    println!();
    println!("HEAD-TO-HEAD MATCHUP RESULTS ({} games):", report.head_to_head.total_games);
    println!(
        "  {:<28} : {:>4} wins ({:>5.1}%)",
        report.head_to_head.p1_name,
        report.head_to_head.p1_wins,
        report.head_to_head.p1_win_rate * 100.0
    );
    println!(
        "  {:<28} : {:>4} wins ({:>5.1}%)",
        report.head_to_head.p2_name,
        report.head_to_head.p2_wins,
        report.head_to_head.p2_win_rate * 100.0
    );
    println!();
    println!("ASCII TURNS DISTRIBUTION (5-turn buckets):");
    for strat in &report.strategies {
        println!("  Strategy: {}", strat.name);
        // Group into 5-turn buckets from 20..=95
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
    println!("================================================================================");
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

        // Verify JSON serialization doesn't crash and contains essential keys
        let json = replay.to_json();
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        assert!(json.contains("\"player1_ships\""));
        assert!(json.contains("\"player2_ships\""));
        assert!(json.contains("\"moves\""));
    }

    #[test]
    fn test_benchmark_checkerboard_outperforms_random_hunt() {
        // Run a fast benchmark of 50 games to verify checkerboard turns < random_hunt turns
        let report = run_benchmark(50, 888);
        assert_eq!(report.strategies.len(), 2);

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
    }
}
