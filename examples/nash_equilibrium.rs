use battleship::rng::Rng;
use battleship::sim::{
    create_defense_instance, create_strategy_instance, simulate_game,
};

fn main() {
    let num_att = 9usize; // Human-playable strategies A1..A9
    let num_def = 7usize; // D1..D7
    let games_per_cell = 1000usize;

    let mut m = vec![vec![0.0f64; num_def]; num_att];

    for a in 0..num_att {
        for d in 0..num_def {
            let mut sum = 0usize;
            for g in 0..games_per_cell {
                let seed = 5_000_000 + (a as u64) * 100_000 + (d as u64) * 10_000 + (g as u64);
                let mut rng = Rng::with_seed(seed);
                let (mut att, _, _) = create_strategy_instance(a);
                let (mut def, _, _) = create_defense_instance(d);
                sum += simulate_game(att.as_mut(), def.as_mut(), &mut rng);
            }
            m[a][d] = sum as f64 / (games_per_cell as f64);
        }
    }

    println!("=========================================================================================================");
    println!("        9 HUMAN ATTACKERS x 7 DEFENSES PAYOFF MATRIX (Mean Turns over 1,000 Games per Cell)              ");
    println!("        Attacker wants to MINIMIZE turns; Defender wants to MAXIMIZE turns.                              ");
    println!("=========================================================================================================");
    print!("| {:<34} |", "Attack \\ Defense");
    for d in 0..num_def {
        print!("  [D{}]  |", d + 1);
    }
    println!(" Max(Row)|");
    println!("+------------------------------------+--------+--------+--------+--------+--------+--------+--------+---------+");

    for a in 0..num_att {
        let (_, _, a_name) = create_strategy_instance(a);
        print!("| [A{}] {:<29} |", a + 1, a_name);
        let mut row_max = 0.0f64;
        for d in 0..num_def {
            let v = m[a][d];
            if v > row_max {
                row_max = v;
            }
            print!(" {:6.2} |", v);
        }
        println!("  {:6.2} |", row_max);
    }
    println!("+------------------------------------+--------+--------+--------+--------+--------+--------+--------+---------+");
    print!("| {:<34} |", "Min(Col) [Attacker Best Response]");
    for d in 0..num_def {
        let mut col_min = 999.0f64;
        for a in 0..num_att {
            if m[a][d] < col_min {
                col_min = m[a][d];
            }
        }
        print!(" {:6.2} |", col_min);
    }
    println!("         |\n");

    // Solve zero-sum matrix game via Multiplicative Weights / Regret Matching (1,000,000 iterations)
    // Attacker minimizes, Defender maximizes
    let iters = 500_000usize;
    let mut reg_a = vec![0.0f64; num_att];
    let mut reg_d = vec![0.0f64; num_def];
    let mut sum_p = vec![0.0f64; num_att];
    let mut sum_q = vec![0.0f64; num_def];

    let mut p = vec![1.0 / (num_att as f64); num_att];
    let mut q = vec![1.0 / (num_def as f64); num_def];

    for _ in 0..iters {
        // Accumulate average strategy
        for a in 0..num_att {
            sum_p[a] += p[a];
        }
        for d in 0..num_def {
            sum_q[d] += q[d];
        }

        // Expected payoff under current (p, q)
        let mut val = 0.0f64;
        let mut row_payoffs = vec![0.0f64; num_att]; // E[M[a, q]]
        let mut col_payoffs = vec![0.0f64; num_def]; // E[M[p, d]]

        for a in 0..num_att {
            for d in 0..num_def {
                let v = m[a][d];
                row_payoffs[a] += v * q[d];
                col_payoffs[d] += p[a] * v;
                val += p[a] * v * q[d];
            }
        }

        // Attacker wants to MINIMIZE, so regret for action a is (val - row_payoffs[a])
        for a in 0..num_att {
            reg_a[a] = (reg_a[a] + (val - row_payoffs[a])).max(0.0);
        }
        // Defender wants to MAXIMIZE, so regret for action d is (col_payoffs[d] - val)
        for d in 0..num_def {
            reg_d[d] = (reg_d[d] + (col_payoffs[d] - val)).max(0.0);
        }

        let pos_a: f64 = reg_a.iter().sum();
        if pos_a > 1e-12 {
            for a in 0..num_att {
                p[a] = reg_a[a] / pos_a;
            }
        } else {
            for a in 0..num_att {
                p[a] = 1.0 / (num_att as f64);
            }
        }

        let pos_d: f64 = reg_d.iter().sum();
        if pos_d > 1e-12 {
            for d in 0..num_def {
                q[d] = reg_d[d] / pos_d;
            }
        } else {
            for d in 0..num_def {
                q[d] = 1.0 / (num_def as f64);
            }
        }
    }

    for a in 0..num_att {
        p[a] = sum_p[a] / (iters as f64);
    }
    for d in 0..num_def {
        q[d] = sum_q[d] / (iters as f64);
    }

    let mut game_value = 0.0f64;
    for a in 0..num_att {
        for d in 0..num_def {
            game_value += p[a] * m[a][d] * q[d];
        }
    }

    println!("=========================================================================================================");
    println!("                     EXACT MIXED-STRATEGY NASH EQUILIBRIUM (Game Value = {:.2} Turns)                    ", game_value);
    println!("=========================================================================================================");
    println!("\nOptimal Attacker Mix p* (Minimizing Expected Turns):");
    for a in 0..num_att {
        let (_, _, a_name) = create_strategy_instance(a);
        let mut exp_turns = 0.0;
        for d in 0..num_def {
            exp_turns += m[a][d] * q[d];
        }
        println!(
            "  [A{}] {:<32} : {:6.2}%  (Expected Turns vs q*: {:.2})",
            a + 1,
            a_name,
            p[a] * 100.0,
            exp_turns
        );
    }

    println!("\nOptimal Defender Mix q* (Maximizing Expected Turns Survived):");
    for d in 0..num_def {
        let (_, _, d_name) = create_defense_instance(d);
        let mut exp_turns = 0.0;
        for a in 0..num_att {
            exp_turns += p[a] * m[a][d];
        }
        println!(
            "  [D{}] {:<32} : {:6.2}%  (Expected Turns Survived vs p*: {:.2})",
            d + 1,
            d_name,
            q[d] * 100.0,
            exp_turns
        );
    }
}
