use battleship::rng::Rng;
use battleship::sim::{
    compute_stats, create_defense_instance, create_strategy_instance, simulate_game, simulate_match,
};
use battleship::game::Player;

fn main() {
    // 9 Human-Playable Attack Strategies: indices 0..9
    // Top 3 Defenses:
    //   2 => Corner & Edge Hugging (D3)
    //   4 => Anti-Midpoint (Bisection Dodger) (D5)
    //   5 => Bait Capital + Corner Destroyer (D6)
    let top3_def_indices = [5usize, 2usize, 4usize];
    let games_per_def = 400usize; // 1,200 solo games per attacker (400 vs each of Top 3 Defenses)
    let duels_per_def = 200usize; // 600 head-to-head duels per attacker pair (200 vs each of Top 3 Defenses)

    println!("=========================================================================================================");
    println!("   9 HUMAN-PLAYABLE ATTACKERS vs. TOP 3 DEFENSES (Bait+Corner, Corner&Edge, Anti-Midpoint)               ");
    println!("   1,200 Solo Games per Attacker (400 per Defense) + 600 Head-to-Head Duels per Pair                     ");
    println!("=========================================================================================================\n");

    let mut solo_means_by_def = vec![vec![0.0; 3]; 9];
    let mut combined_stats = Vec::new();

    for a_idx in 0..9 {
        let (_, a_id, a_name) = create_strategy_instance(a_idx);
        let mut all_turns = Vec::with_capacity(games_per_def * 3);

        for (d_pos, &d_idx) in top3_def_indices.iter().enumerate() {
            let mut def_turns = Vec::with_capacity(games_per_def);
            for g in 0..games_per_def {
                let seed = 100_000 + (a_idx as u64) * 10_000 + (d_idx as u64) * 1_000 + (g as u64);
                let mut rng = Rng::with_seed(seed);
                let (mut att, _, _) = create_strategy_instance(a_idx);
                let (mut def, _, _) = create_defense_instance(d_idx);
                let t = simulate_game(att.as_mut(), def.as_mut(), &mut rng);
                def_turns.push(t);
                all_turns.push(t);
            }
            let sum: usize = def_turns.iter().sum();
            solo_means_by_def[a_idx][d_pos] = sum as f64 / (games_per_def as f64);
        }

        combined_stats.push((a_idx, compute_stats(a_id, a_name, all_turns)));
    }

    println!("SOLO PERFORMANCE AGAINST TOP 3 DEFENSES (Lower Turns = Better Attacker):");
    println!("+------------------------------------+--------+--------+--------+-------+--------+-----+-----+-------+-----+-----+");
    println!("| Attack Strategy                    | vs D6  | vs D3  | vs D5  | Mean  | Median | Min | Max | StDev | P10 | P90 |");
    println!("|                                    | (Bait) | (Edge) | (Anti) | (All) |        |     |     |       |     |     |");
    println!("+------------------------------------+--------+--------+--------+-------+--------+-----+-----+-------+-----+-----+");
    for (a_idx, st) in &combined_stats {
        println!(
            "| [{:1}] {:<30} | {:6.1} | {:6.1} | {:6.1} | {:5.1} | {:6.1} | {:3} | {:3} | {:5.2} | {:3.0} | {:3.0} |",
            a_idx + 1,
            st.name,
            solo_means_by_def[*a_idx][0],
            solo_means_by_def[*a_idx][1],
            solo_means_by_def[*a_idx][2],
            st.mean,
            st.median,
            st.min,
            st.max,
            st.std_dev,
            st.p10,
            st.p90
        );
    }
    println!("+------------------------------------+--------+--------+--------+-------+--------+-----+-----+-------+-----+-----+\n");

    // 9x9 All-Pairs Head-to-Head Tournament when both defenders pick the same Top-3 Defense
    let mut win_matrix = vec![vec![0.0; 9]; 9];
    let mut total_wins = vec![0usize; 9];
    let mut total_losses = vec![0usize; 9];
    let mut series_wins = vec![0usize; 9];
    let mut series_losses = vec![0usize; 9];
    let mut series_ties = vec![0usize; 9];

    for i in 0..9 {
        for j in (i + 1)..9 {
            let mut i_wins = 0usize;
            let mut j_wins = 0usize;
            let mut game_counter = 0usize;

            for &d_idx in &top3_def_indices {
                for g in 0..duels_per_def {
                    let seed = 900_000
                        + (i as u64) * 50_000
                        + (j as u64) * 5_000
                        + (d_idx as u64) * 1_000
                        + (g as u64);
                    let (mut att_i, _, name_i) = create_strategy_instance(i);
                    let (mut att_j, _, name_j) = create_strategy_instance(j);
                    let (mut def_1, _, _) = create_defense_instance(d_idx);
                    let (mut def_2, _, _) = create_defense_instance(d_idx);

                    if game_counter % 2 == 0 {
                        let rep = simulate_match(
                            name_i,
                            att_i.as_mut(),
                            def_1.as_mut(),
                            name_j,
                            att_j.as_mut(),
                            def_2.as_mut(),
                            seed,
                        );
                        if rep.winner == Player::Player1 {
                            i_wins += 1;
                        } else {
                            j_wins += 1;
                        }
                    } else {
                        let rep = simulate_match(
                            name_j,
                            att_j.as_mut(),
                            def_1.as_mut(),
                            name_i,
                            att_i.as_mut(),
                            def_2.as_mut(),
                            seed,
                        );
                        if rep.winner == Player::Player1 {
                            j_wins += 1;
                        } else {
                            i_wins += 1;
                        }
                    }
                    game_counter += 1;
                }
            }

            let total_pair = i_wins + j_wins;
            win_matrix[i][j] = (i_wins as f64 / total_pair as f64) * 100.0;
            win_matrix[j][i] = (j_wins as f64 / total_pair as f64) * 100.0;
            total_wins[i] += i_wins;
            total_losses[i] += j_wins;
            total_wins[j] += j_wins;
            total_losses[j] += i_wins;

            if i_wins > j_wins {
                series_wins[i] += 1;
                series_losses[j] += 1;
            } else if j_wins > i_wins {
                series_wins[j] += 1;
                series_losses[i] += 1;
            } else {
                series_ties[i] += 1;
                series_ties[j] += 1;
            }
        }
    }

    println!("9x9 HUMAN-PLAYABLE ATTACK HEAD-TO-HEAD WIN RATE MATRIX VS. TOP 3 DEFENSES (%):");
    println!("+------------------------------------+------+------+------+------+------+------+------+------+------+");
    println!("| Strategy                           |  [1] |  [2] |  [3] |  [4] |  [5] |  [6] |  [7] |  [8] |  [9] |");
    println!("+------------------------------------+------+------+------+------+------+------+------+------+------+");
    for i in 0..9 {
        let (_, _, name_i) = create_strategy_instance(i);
        print!("| [{:1}] {:<30} |", i + 1, name_i);
        for j in 0..9 {
            if i == j {
                print!("  --- |");
            } else {
                print!(" {:4.0}%|", win_matrix[i][j]);
            }
        }
        println!();
    }
    println!("+------------------------------------+------+------+------+------+------+------+------+------+------+\n");

    let mut standings: Vec<usize> = (0..9).collect();
    standings.sort_by(|&a, &b| total_wins[b].cmp(&total_wins[a]));

    println!("OVERALL HUMAN-PLAYABLE ATTACK STANDINGS AGAINST TOP 3 DEFENSES (4,800 Duels per Strategy):");
    println!("+------+------------------------------------+---------+-------+--------+--------+---------------+-------------+");
    println!("| Rank | Attack Strategy                    | Series  | Duels |  Wins  | Losses | Overall Win % | Solo Mean   |");
    println!("+------+------------------------------------+---------+-------+--------+--------+---------------+-------------+");
    for (rank, &idx) in standings.iter().enumerate() {
        let (_, _, name) = create_strategy_instance(idx);
        let duels = total_wins[idx] + total_losses[idx];
        let win_pct = (total_wins[idx] as f64 / duels as f64) * 100.0;
        println!(
            "| {:4} | [{:1}] {:<30} |  {}-{}-{}  | {:5} | {:6} | {:6} | {:12.1}% | {:6.1} turns |",
            rank + 1,
            idx + 1,
            name,
            series_wins[idx],
            series_losses[idx],
            series_ties[idx],
            duels,
            total_wins[idx],
            total_losses[idx],
            win_pct,
            combined_stats[idx].1.mean
        );
    }
    println!("+------+------------------------------------+---------+-------+--------+--------+---------------+-------------+");
}
