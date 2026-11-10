//! Battleship Simulator Engine CLI & Server.
//!
//! Subcommands:
//! - `simulate [--games N] [--seed S] [--ruleset standard|salvo|silent|all]` (default: 1000 games benchmark + writes viewer/sample_data.js)
//! - `match [--p1 P1] [--p2 P2] [--d1 D1] [--d2 D2] [--ruleset standard|salvo|silent] [--seed S]` (single match JSON to stdout)
//! - `serve [--port PORT]` (HTTP static file and API server on 127.0.0.1:PORT)

use std::fs;
use std::path::Path;

use battleship::game::Ruleset;
use battleship::server::{create_defense, create_strategy, run_server};
use battleship::sim::{
    print_benchmark_summary, print_cross_ruleset_summary, run_benchmark_with_ruleset,
    simulate_match_with_ruleset, BenchmarkReport, MatchReplay,
};

fn print_help() {
    println!(
        r#"Battleship Simulator Engine (Standard Hasbro, Salvo Mode, Silent Sinking & Salvo+Fog Rules)

USAGE:
    cargo run -- [SUBCOMMAND] [OPTIONS]

SUBCOMMANDS:
    simulate (default)  Run multi-game statistical benchmark of all 13 attack strategies
                        and all 7 defense strategies across rulesets.
                        Executes:
                          - 13 Attack Strategies Solo Benchmark & 13x13 Round-Robin Tournament
                          - 7 Defenses x 13 Attackers Survival Matrix (Mean Turns Survived)
                          - 7x7 Defense Head-to-Head Tournament & Overall Defense Standings
                        Prints summary tables & ASCII histograms to stdout, and writes `viewer/sample_data.js`.
                        Options:
                          --ruleset <R>  Ruleset to simulate: all, standard, salvo, silent, salvo_silent (default: all)
                          --games <N>    Number of games per strategy (default: 1000)
                          --seed <S>     PRNG seed for reproducible results (default: 42)

    match               Simulate a single head-to-head match and print JSON to stdout.
                        Options:
                          --ruleset <R>  Ruleset: standard, salvo, silent, salvo_silent (default: standard)
                          --p1 <STRAT>   Player 1 attack strategy (default: random_hunt)
                          --p2 <STRAT>   Player 2 attack strategy (default: checkerboard)
                                         Attack Strategies:
                                           - random_hunt       ("Random + Hunt/Target")
                                           - checkerboard      ("Checkerboard + Hunt/Target")
                                           - largest_rect      ("Largest Empty Rect Midpoint")
                                           - adaptive_stride   ("Adaptive Stride (2->3->4->5)")
                                           - stride4_to_2      ("Staggered Stride (4->2)")
                                           - rect_stride4      ("Largest Rect + Stride (4->2)")
                                           - stride_5432       ("Descending Stride (5->4->3->2)")
                                           - rect_stride_5432  ("Largest Rect + Stride (5->4->3->2)")
                                           - rect_checkerboard ("Largest Rect + Checkerboard")
                                           - monte_carlo       ("Monte Carlo Sampling")
                                           - bayesian_pdf      ("Bayesian Probability Density")
                                           - crosshair_stride  ("Crosshair Stride + Open-Target")
                                           - crosshair_5432    ("Crosshair Stride (5->4->3->2)")

                          --d1 <DEF>     Player 1 defense strategy (default: random)
                          --d2 <DEF>     Player 2 defense strategy (default: random)
                                         Defense Strategies:
                                           - random          ("Uniform Random")
                                           - spaced          ("Spaced (No-Touching)")
                                           - corner_edge     ("Corner & Edge Hugging")
                                           - clustered       ("Tightly Clustered Phalanx")
                                           - anti_midpoint   ("Anti-Midpoint (Bisection Dodger)")
                                           - bait_and_hide   ("Bait Capital + Corner Destroyer")
                                           - parallel_lanes  ("Parallel Lanes (Single Axis)")

                          --seed <S>     PRNG seed (default: 42)

    serve               Start local HTTP server serving web viewer and REST APIs.
                        Options:
                          --port <PORT>  HTTP port (default: 8080)

    help, -h, --help    Print this help menu.

EXAMPLES:
    cargo run -- simulate --ruleset all --games 1000 --seed 42
    cargo run -- match --ruleset salvo --p1 bayesian_pdf --p2 monte_carlo --d1 spaced --d2 anti_midpoint --seed 123
    cargo run -- serve --port 8080
"#
    );
}

fn write_sample_data_file(
    primary_report: &BenchmarkReport,
    reports_by_ruleset: &[(Ruleset, BenchmarkReport)],
    matches: &[MatchReplay],
) -> std::io::Result<()> {
    let viewer_dir = Path::new("viewer");
    if !viewer_dir.exists() {
        fs::create_dir_all(viewer_dir)?;
    }

    let matches_json: Vec<String> = matches.iter().map(|m| m.to_json()).collect();
    let rulesets_json: Vec<String> = reports_by_ruleset
        .iter()
        .map(|(r, rep)| format!("\"{}\": {}", r.id(), rep.to_json()))
        .collect();

    let content = format!(
        "// Auto-generated by Battleship Simulator Engine (Rust std)\n\
         window.BATTLESHIP_SAMPLE_DATA = {{\n  \
             \"benchmark\": {},\n  \
             \"benchmarks_by_ruleset\": {{\n    {}\n  }},\n  \
             \"sample_matches\": [\n    {}\n  ]\n\
         }};\n",
        primary_report.to_json(),
        rulesets_json.join(",\n    "),
        matches_json.join(",\n    ")
    );

    let path = viewer_dir.join("sample_data.js");
    fs::write(&path, content)?;
    println!("\nWrote sample simulation data to {}", path.display());
    Ok(())
}

fn run_simulate_cmd(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut games: usize = 1000;
    let mut seed: u64 = 42;
    let mut ruleset_arg = "all".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--games" | "-g" => {
                if i + 1 < args.len() {
                    games = args[i + 1].parse().unwrap_or(1000);
                    i += 1;
                }
            }
            "--seed" | "-s" => {
                if i + 1 < args.len() {
                    seed = args[i + 1].parse().unwrap_or(42);
                    i += 1;
                }
            }
            "--ruleset" | "-r" => {
                if i + 1 < args.len() {
                    ruleset_arg = args[i + 1].to_ascii_lowercase();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let sample_configs = [
        ("Crosshair Stride + Open-Target", "crosshair_stride", "bait_and_hide", "Bayesian Probability Density", "bayesian_pdf", "spaced", seed + 808, Ruleset::Standard),
        ("Crosshair Stride (5->4->3->2)", "crosshair_5432", "corner_edge", "Monte Carlo Sampling", "monte_carlo", "anti_midpoint", seed + 909, Ruleset::Salvo),
        ("Bayesian Probability Density", "bayesian_pdf", "spaced", "Monte Carlo Sampling", "monte_carlo", "anti_midpoint", seed + 505, Ruleset::SilentSinking),
        ("Crosshair Stride (5->4->3->2)", "crosshair_5432", "parallel_lanes", "Bayesian Probability Density", "bayesian_pdf", "spaced", seed + 404, Ruleset::SalvoSilent),
        ("Bayesian Probability Density", "bayesian_pdf", "corner_edge", "Largest Rect + Stride (4->2)", "rect_stride4", "clustered", seed + 101, Ruleset::Standard),
        ("Monte Carlo Sampling", "monte_carlo", "bait_and_hide", "Largest Rect + Stride (5->4->3->2)", "rect_stride_5432", "spaced", seed + 606, Ruleset::Salvo),
        ("Descending Stride (5->4->3->2)", "stride_5432", "parallel_lanes", "Largest Rect + Checkerboard", "rect_checkerboard", "corner_edge", seed + 707, Ruleset::SilentSinking),
        ("Monte Carlo Sampling", "monte_carlo", "clustered", "Crosshair Stride + Open-Target", "crosshair_stride", "bait_and_hide", seed + 333, Ruleset::SalvoSilent),
        ("Largest Rect + Stride (4->2)", "rect_stride4", "anti_midpoint", "Staggered Stride (4->2)", "stride4_to_2", "bait_and_hide", seed + 777, Ruleset::Standard),
        ("Staggered Stride (4->2)", "stride4_to_2", "clustered", "Adaptive Stride (2->3->4->5)", "adaptive_stride", "parallel_lanes", seed + 202, Ruleset::Salvo),
        ("Largest Empty Rect Midpoint", "largest_rect", "spaced", "Checkerboard + Hunt/Target", "checkerboard", "corner_edge", seed + 303, Ruleset::SilentSinking),
        ("Checkerboard + Hunt/Target", "checkerboard", "random", "Random + Hunt/Target", "random_hunt", "random", seed, Ruleset::Standard),
    ];

    let mut sample_matches = Vec::new();
    for (name1, id1, d1_id, name2, id2, d2_id, s, r) in sample_configs {
        let (mut p1_att, _) = create_strategy(id1);
        let (mut p2_att, _) = create_strategy(id2);
        let (mut def1, _) = create_defense(d1_id);
        let (mut def2, _) = create_defense(d2_id);
        let replay = simulate_match_with_ruleset(
            name1,
            p1_att.as_mut(),
            def1.as_mut(),
            name2,
            p2_att.as_mut(),
            def2.as_mut(),
            r,
            s,
        );
        sample_matches.push(replay);
    }

    if ruleset_arg == "all" {
        println!("Running complete Battleship benchmark suite across ALL rulesets ({} games per strategy, seed {})...", games, seed);

        println!("\n>>> 1/4: Running Standard Hasbro Ruleset...");
        let std_report = run_benchmark_with_ruleset(games, seed, Ruleset::Standard);
        print_benchmark_summary(&std_report);

        println!("\n>>> 2/4: Running Salvo Mode Ruleset...");
        let salvo_report = run_benchmark_with_ruleset(games, seed, Ruleset::Salvo);
        print_benchmark_summary(&salvo_report);

        println!("\n>>> 3/4: Running Silent Sinking (Fog of War) Ruleset...");
        let silent_report = run_benchmark_with_ruleset(games, seed, Ruleset::SilentSinking);
        print_benchmark_summary(&silent_report);

        println!("\n>>> 4/4: Running Salvo + Fog of War Ruleset...");
        let salvo_silent_report = run_benchmark_with_ruleset(games, seed, Ruleset::SalvoSilent);
        print_benchmark_summary(&salvo_silent_report);

        let reports = [
            std_report.clone(),
            salvo_report.clone(),
            silent_report.clone(),
            salvo_silent_report.clone(),
        ];
        print_cross_ruleset_summary(&reports);

        let reports_by_ruleset = [
            (Ruleset::Standard, std_report.clone()),
            (Ruleset::Salvo, salvo_report),
            (Ruleset::SilentSinking, silent_report),
            (Ruleset::SalvoSilent, salvo_silent_report),
        ];

        if let Err(e) = write_sample_data_file(&std_report, &reports_by_ruleset, &sample_matches) {
            eprintln!("Warning: could not write viewer/sample_data.js: {}", e);
        }
    } else {
        let ruleset = Ruleset::from_id(&ruleset_arg).unwrap_or(Ruleset::Standard);
        println!("Running Battleship benchmark for ruleset '{}' ({} games per strategy, seed {})...", ruleset.name(), games, seed);
        let report = run_benchmark_with_ruleset(games, seed, ruleset);
        print_benchmark_summary(&report);

        let reports_by_ruleset = [(ruleset, report.clone())];
        if let Err(e) = write_sample_data_file(&report, &reports_by_ruleset, &sample_matches) {
            eprintln!("Warning: could not write viewer/sample_data.js: {}", e);
        }
    }

    Ok(())
}

fn run_match_cmd(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut p1_id = "random_hunt".to_string();
    let mut p2_id = "checkerboard".to_string();
    let mut d1_id = "random".to_string();
    let mut d2_id = "random".to_string();
    let mut ruleset = Ruleset::Standard;
    let mut seed: u64 = 42;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--p1" => {
                if i + 1 < args.len() {
                    p1_id = args[i + 1].clone();
                    i += 1;
                }
            }
            "--p2" => {
                if i + 1 < args.len() {
                    p2_id = args[i + 1].clone();
                    i += 1;
                }
            }
            "--d1" => {
                if i + 1 < args.len() {
                    d1_id = args[i + 1].clone();
                    i += 1;
                }
            }
            "--d2" => {
                if i + 1 < args.len() {
                    d2_id = args[i + 1].clone();
                    i += 1;
                }
            }
            "--ruleset" | "-r" => {
                if i + 1 < args.len() {
                    ruleset = Ruleset::from_id(&args[i + 1]).unwrap_or(Ruleset::Standard);
                    i += 1;
                }
            }
            "--seed" | "-s" => {
                if i + 1 < args.len() {
                    seed = args[i + 1].parse().unwrap_or(42);
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let (mut p1_att, p1_name) = create_strategy(&p1_id);
    let (mut p2_att, p2_name) = create_strategy(&p2_id);
    let (mut def1, _) = create_defense(&d1_id);
    let (mut def2, _) = create_defense(&d2_id);

    let replay = simulate_match_with_ruleset(
        p1_name,
        p1_att.as_mut(),
        def1.as_mut(),
        p2_name,
        p2_att.as_mut(),
        def2.as_mut(),
        ruleset,
        seed,
    );

    println!("{}", replay.to_json());
    Ok(())
}

fn run_serve_cmd(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut port: u16 = 8080;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(8080);
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    run_server(port)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    if raw_args.is_empty() {
        // Default behavior: run simulation
        return run_simulate_cmd(&[]);
    }

    let subcommand = raw_args[0].to_ascii_lowercase();
    let sub_args = &raw_args[1..];

    match subcommand.as_str() {
        "simulate" | "sim" | "benchmark" => run_simulate_cmd(sub_args),
        "match" | "play" => run_match_cmd(sub_args),
        "serve" | "server" => run_serve_cmd(sub_args),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        _ => {
            // Check if user passed flags directly without subcommand (e.g. `--games 500`)
            if subcommand.starts_with('-') {
                run_simulate_cmd(&raw_args)
            } else {
                eprintln!("Unknown subcommand: '{}'", subcommand);
                print_help();
                std::process::exit(1);
            }
        }
    }
}
