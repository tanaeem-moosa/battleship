use battleship::game::{ShotOutcome, TrackingBoard, TrackingCell, HEIGHT, WIDTH};
use battleship::rng::Rng;
use battleship::sim::{create_defense_instance, create_strategy_instance, simulate_game};
use battleship::strategy::AttackStrategy;

/// Count consecutive Unknown cells from (r, c) in direction (dr, dc), up to `max_steps`.
fn count_arm(view: &TrackingBoard, r: usize, c: usize, dr: isize, dc: isize, max_steps: usize) -> usize {
    let mut steps = 0;
    let mut cr = r as isize + dr;
    let mut cc = c as isize + dc;
    while steps < max_steps && cr >= 0 && cr < HEIGHT as isize && cc >= 0 && cc < WIDTH as isize {
        if view.cell(cr as usize, cc as usize) == TrackingCell::Unknown {
            steps += 1;
            cr += dr;
            cc += dc;
        } else {
            break;
        }
    }
    steps
}

/// Human-Playable Strategy: "Crosshair Stride + Open-Side Target"
/// 1. Hunt Mode: Uses Staggered Stride (4 -> 2) lattice, and scores each lattice point by its
///    "+" Crosshair open arms (up to target ship length L-1 in N, S, E, W), ignoring directions
///    where total span < L.
/// 2. Target Mode: Always fires toward the neighbor/endpoint with the longest open runway!
pub struct CrosshairStrideAttack {
    offset4: usize,
    initialized: bool,
    active_hits: Vec<(usize, usize)>,
}

impl CrosshairStrideAttack {
    pub fn new() -> Self {
        Self {
            offset4: 0,
            initialized: false,
            active_hits: Vec::new(),
        }
    }

    fn choose_open_side_target(&self, view: &TrackingBoard, rng: &mut Rng) -> Option<(usize, usize)> {
        if self.active_hits.is_empty() {
            return None;
        }
        let min_len = view.min_remaining_ship_len().max(2);
        let max_len = view.max_remaining_ship_len().max(2);

        // Check for contiguous collinear chains of length >= 2
        let mut best_collinear: Vec<((usize, usize), usize)> = Vec::new();

        for &(r, c) in &self.active_hits {
            // Horizontal chain starting at (r, c)
            if c == 0 || !self.active_hits.contains(&(r, c - 1)) {
                let mut end_c = c;
                while end_c + 1 < WIDTH && self.active_hits.contains(&(r, end_c + 1)) {
                    end_c += 1;
                }
                let chain_len = end_c - c + 1;
                if chain_len >= 2 {
                    let left_open = if c > 0 && view.is_unshot(r, c - 1) {
                        1 + count_arm(view, r, c - 1, 0, -1, max_len)
                    } else {
                        0
                    };
                    let right_open = if end_c + 1 < WIDTH && view.is_unshot(r, end_c + 1) {
                        1 + count_arm(view, r, end_c + 1, 0, 1, max_len)
                    } else {
                        0
                    };
                    if left_open + chain_len + right_open >= min_len {
                        if left_open > 0 {
                            best_collinear.push(((r, c - 1), left_open * 10 + right_open));
                        }
                        if right_open > 0 {
                            best_collinear.push(((r, end_c + 1), right_open * 10 + left_open));
                        }
                    }
                }
            }
            // Vertical chain starting at (r, c)
            if r == 0 || !self.active_hits.contains(&(r - 1, c)) {
                let mut end_r = r;
                while end_r + 1 < HEIGHT && self.active_hits.contains(&(end_r + 1, c)) {
                    end_r += 1;
                }
                let chain_len = end_r - r + 1;
                if chain_len >= 2 {
                    let up_open = if r > 0 && view.is_unshot(r - 1, c) {
                        1 + count_arm(view, r - 1, c, -1, 0, max_len)
                    } else {
                        0
                    };
                    let down_open = if end_r + 1 < HEIGHT && view.is_unshot(end_r + 1, c) {
                        1 + count_arm(view, end_r + 1, c, 1, 0, max_len)
                    } else {
                        0
                    };
                    if up_open + chain_len + down_open >= min_len {
                        if up_open > 0 {
                            best_collinear.push(((r - 1, c), up_open * 10 + down_open));
                        }
                        if down_open > 0 {
                            best_collinear.push(((end_r + 1, c), down_open * 10 + up_open));
                        }
                    }
                }
            }
        }

        if !best_collinear.is_empty() {
            let max_score = best_collinear.iter().map(|x| x.1).max().unwrap();
            let top: Vec<(usize, usize)> = best_collinear
                .into_iter()
                .filter(|x| x.1 == max_score)
                .map(|x| x.0)
                .collect();
            return top.get(rng.next_usize(top.len())).copied();
        }

        // Single hit (or blocked collinear): score all 4 orthogonal neighbors by open runway!
        let mut candidates: Vec<((usize, usize), usize)> = Vec::new();
        for &(r, c) in &self.active_hits {
            let up = count_arm(view, r, c, -1, 0, max_len - 1);
            let down = count_arm(view, r, c, 1, 0, max_len - 1);
            let left = count_arm(view, r, c, 0, -1, max_len - 1);
            let right = count_arm(view, r, c, 0, 1, max_len - 1);

            let v_span = up + 1 + down;
            let h_span = left + 1 + right;

            if v_span >= min_len {
                if r > 0 && view.is_unshot(r - 1, c) {
                    candidates.push(((r - 1, c), up * 10 + v_span));
                }
                if r + 1 < HEIGHT && view.is_unshot(r + 1, c) {
                    candidates.push(((r + 1, c), down * 10 + v_span));
                }
            }
            if h_span >= min_len {
                if c > 0 && view.is_unshot(r, c - 1) {
                    candidates.push(((r, c - 1), left * 10 + h_span));
                }
                if c + 1 < WIDTH && view.is_unshot(r, c + 1) {
                    candidates.push(((r, c + 1), right * 10 + h_span));
                }
            }
        }

        if !candidates.is_empty() {
            let max_score = candidates.iter().map(|x| x.1).max().unwrap();
            let top: Vec<(usize, usize)> = candidates
                .into_iter()
                .filter(|x| x.1 == max_score)
                .map(|x| x.0)
                .collect();
            return top.get(rng.next_usize(top.len())).copied();
        }

        None
    }
}

impl AttackStrategy for CrosshairStrideAttack {
    fn name(&self) -> &'static str {
        "Crosshair Stride + Open-Side Target"
    }

    fn reset(&mut self) {
        self.initialized = false;
        self.active_hits.clear();
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        if !self.initialized {
            self.offset4 = rng.next_usize(4);
            self.initialized = true;
        }

        if let Some(target) = self.choose_open_side_target(view, rng) {
            return target;
        }

        let max_len = view.max_remaining_ship_len().max(2);
        let min_len = view.min_remaining_ship_len().max(2);
        let stride = if max_len >= 4 { 4 } else { 2 };
        let target_rem = self.offset4 % stride;

        let unshot = view.unshot_cells();
        let mut best_cells = Vec::new();
        let mut best_score = 0usize;

        // Evaluate Crosshair score on active stride cells first
        for &(r, c) in &unshot {
            if (r + c) % stride != target_rem {
                continue;
            }
            let up = count_arm(view, r, c, -1, 0, max_len - 1);
            let down = count_arm(view, r, c, 1, 0, max_len - 1);
            let left = count_arm(view, r, c, 0, -1, max_len - 1);
            let right = count_arm(view, r, c, 0, 1, max_len - 1);

            // Exact number of ways `max_len` AND `min_len` ships can cover (r, c):
            let h_ways_max = (left + right + 1).saturating_sub(max_len - 1);
            let v_ways_max = (up + down + 1).saturating_sub(max_len - 1);
            let h_ways_min = (left.min(min_len - 1) + right.min(min_len - 1) + 1).saturating_sub(min_len - 1);
            let v_ways_min = (up.min(min_len - 1) + down.min(min_len - 1) + 1).saturating_sub(min_len - 1);

            let score = (h_ways_max + v_ways_max) * 3 + (h_ways_min + v_ways_min);
            if score > best_score {
                best_score = score;
                best_cells.clear();
                best_cells.push((r, c));
            } else if score == best_score && score > 0 {
                best_cells.push((r, c));
            }
        }

        if !best_cells.is_empty() {
            return best_cells[rng.next_usize(best_cells.len())];
        }

        // Fallback to any unshot cell where min_len fits
        unshot[rng.next_usize(unshot.len())]
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, _view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                if !self.active_hits.contains(&pos) {
                    self.active_hits.push(pos);
                }
            }
            ShotOutcome::Sunk(kind) => {
                if !self.active_hits.contains(&pos) {
                    self.active_hits.push(pos);
                }
                let ship_len = kind.length();
                if self.active_hits.len() <= ship_len {
                    self.active_hits.clear();
                } else {
                    // Remove collinear connected hits of length `ship_len` containing `pos`
                    let (sr, sc) = pos;
                    let mut h_chain = vec![pos];
                    let mut c = sc;
                    while c > 0 && self.active_hits.contains(&(sr, c - 1)) && h_chain.len() < ship_len {
                        c -= 1;
                        h_chain.push((sr, c));
                    }
                    c = sc;
                    while c + 1 < WIDTH && self.active_hits.contains(&(sr, c + 1)) && h_chain.len() < ship_len {
                        c += 1;
                        h_chain.push((sr, c));
                    }

                    let mut v_chain = vec![pos];
                    let mut r = sr;
                    while r > 0 && self.active_hits.contains(&(r - 1, sc)) && v_chain.len() < ship_len {
                        r -= 1;
                        v_chain.push((r, sc));
                    }
                    r = sr;
                    while r + 1 < HEIGHT && self.active_hits.contains(&(r + 1, sc)) && v_chain.len() < ship_len {
                        r += 1;
                        v_chain.push((r, sc));
                    }

                    let to_remove = if h_chain.len() == ship_len {
                        h_chain
                    } else if v_chain.len() == ship_len {
                        v_chain
                    } else if h_chain.len() >= v_chain.len() {
                        h_chain
                    } else {
                        v_chain
                    };
                    self.active_hits.retain(|x| !to_remove.contains(x));
                }
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        if self.active_hits.is_empty() {
            "HUNT"
        } else {
            "TARGET"
        }
    }
}

fn main() {
    let games = 1000usize;
    let defs = [0usize, 1, 2, 3, 4, 5, 6]; // All 7 Defenses D1..D7

    println!("=================================================================================================================");
    println!("         ALL 7 DEFENSES EVALUATED AGAINST CROSSHAIR STRIDE + OPEN-TARGET (1,000 Games per Defense)               ");
    println!("=================================================================================================================");
    println!("+--------------------------------------+--------+--------+--------+--------+--------+--------+--------+---------+");
    println!("| Attack Strategy                      |  [D1]  |  [D2]  |  [D3]  |  [D4]  |  [D5]  |  [D6]  |  [D7]  |  Mean   |");
    println!("|                                      | Random | Spaced | Corner | Clustr | AntiMd | BaitHd | Parall |  (All7) |");
    println!("+--------------------------------------+--------+--------+--------+--------+--------+--------+--------+---------+");

    for &(idx, label) in &[
        (1usize, "[A2] Checkerboard + Hunt/Target"),
        (4usize, "[A5] Staggered Stride (4->2)"),
        (5usize, "[A6] Largest Rect + Stride (4->2)"),
        (9usize, "[A10] Monte Carlo Sampling"),
        (10usize, "[A11] Bayesian Probability Density"),
    ] {
        let g_cnt = if idx == 9 { 200 } else { games };
        print!("| {:<36} |", label);
        let mut tot = 0.0;
        for &d_idx in &defs {
            let mut sum = 0usize;
            for g in 0..g_cnt {
                let mut rng = Rng::with_seed(8_000_000 + (idx as u64)*100_000 + (d_idx as u64)*10_000 + g as u64);
                let (mut att, _, _) = create_strategy_instance(idx);
                let (mut def, _, _) = create_defense_instance(d_idx);
                sum += simulate_game(att.as_mut(), def.as_mut(), &mut rng);
            }
            let m = sum as f64 / (g_cnt as f64);
            tot += m;
            print!(" {:6.2} |", m);
        }
        println!("  {:6.2} |", tot / (defs.len() as f64));
    }

    // Now test CrosshairStrideAttack across all 7 Defenses!
    print!("| {:<36} |", "[A12] Crosshair Stride + Open-Target");
    let mut tot = 0.0;
    let mut def_stats = Vec::new();
    for &d_idx in &defs {
        let mut turns_vec = Vec::with_capacity(games);
        for g in 0..games {
            let mut rng = Rng::with_seed(8_000_000 + 99*100_000 + (d_idx as u64)*10_000 + g as u64);
            let mut att = CrosshairStrideAttack::new();
            let (mut def, _, _) = create_defense_instance(d_idx);
            turns_vec.push(simulate_game(&mut att, def.as_mut(), &mut rng));
        }
        let sum: usize = turns_vec.iter().sum();
        let m = sum as f64 / (games as f64);
        tot += m;
        print!(" {:6.2} |", m);
        let (_, d_id, d_name) = create_defense_instance(d_idx);
        def_stats.push((d_idx, battleship::sim::compute_stats(d_id, d_name, turns_vec)));
    }
    println!("  {:6.2} |", tot / (defs.len() as f64));
    println!("+--------------------------------------+--------+--------+--------+--------+--------+--------+--------+---------+\n");

    // 7x7 Defense Head-to-Head Tournament specifically against Crosshair Stride + Open-Target (400 duels per pair)
    let duels_per_pair = 400usize;
    let mut def_wins = vec![0usize; 7];
    let mut def_losses = vec![0usize; 7];
    let mut def_series_w = vec![0usize; 7];
    let mut def_series_l = vec![0usize; 7];
    let mut def_matrix = vec![vec![0.0f64; 7]; 7];

    for d1 in 0..7 {
        for d2 in (d1 + 1)..7 {
            let mut w1 = 0usize;
            let mut w2 = 0usize;
            for g in 0..duels_per_pair {
                let seed = 15_000_000 + (d1 as u64)*100_000 + (d2 as u64)*10_000 + g as u64;
                let mut att1 = CrosshairStrideAttack::new();
                let mut att2 = CrosshairStrideAttack::new();
                let (mut def1, _, n1) = create_defense_instance(d1);
                let (mut def2, _, n2) = create_defense_instance(d2);
                if g % 2 == 0 {
                    let rep = battleship::sim::simulate_match(n1, &mut att1, def1.as_mut(), n2, &mut att2, def2.as_mut(), seed);
                    if rep.winner == battleship::game::Player::Player1 {
                        w1 += 1;
                    } else {
                        w2 += 1;
                    }
                } else {
                    let rep = battleship::sim::simulate_match(n2, &mut att2, def2.as_mut(), n1, &mut att1, def1.as_mut(), seed);
                    if rep.winner == battleship::game::Player::Player1 {
                        w2 += 1;
                    } else {
                        w1 += 1;
                    }
                }
            }
            def_matrix[d1][d2] = (w1 as f64 / duels_per_pair as f64) * 100.0;
            def_matrix[d2][d1] = (w2 as f64 / duels_per_pair as f64) * 100.0;
            def_wins[d1] += w1;
            def_losses[d1] += w2;
            def_wins[d2] += w2;
            def_losses[d2] += w1;
            if w1 > w2 {
                def_series_w[d1] += 1;
                def_series_l[d2] += 1;
            } else if w2 > w1 {
                def_series_w[d2] += 1;
                def_series_l[d1] += 1;
            }
        }
    }

    let mut order: Vec<usize> = (0..7).collect();
    order.sort_by(|&a, &b| def_wins[b].cmp(&def_wins[a]));

    println!("DEFENSE LEADERBOARD SPECIFICALLY AGAINST CROSSHAIR STRIDE + OPEN-TARGET (2,400 Duels + 1,000 Solo Games):");
    println!("+------+------------------------------------+--------+-------+--------+--------+-------+--------+-----+-----+-----+-----+");
    println!("| Rank | Defense Strategy                   | Series | Duels |  Wins  | Win %  | Mean  | Median | Min | Max | P10 | P90 |");
    println!("+------+------------------------------------+--------+-------+--------+--------+-------+--------+-----+-----+-----+-----+");
    for (rk, &d_idx) in order.iter().enumerate() {
        let st = &def_stats[d_idx].1;
        let duels = def_wins[d_idx] + def_losses[d_idx];
        let wp = (def_wins[d_idx] as f64 / duels as f64) * 100.0;
        println!(
            "| {:4} | [D{}] {:<29} |  {}-{}   | {:5} | {:6} | {:5.1}% | {:5.2} | {:6.1} | {:3} | {:3} | {:3.0} | {:3.0} |",
            rk + 1,
            d_idx + 1,
            st.name,
            def_series_w[d_idx],
            def_series_l[d_idx],
            duels,
            def_wins[d_idx],
            wp,
            st.mean,
            st.median,
            st.min,
            st.max,
            st.p10,
            st.p90
        );
    }
    println!("+------+------------------------------------+--------+-------+--------+--------+-------+--------+-----+-----+-----+-----+");
}
