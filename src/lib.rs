pub mod rng;
pub mod game;
pub mod strategy;
pub mod sim;
pub mod server;

pub use rng::Rng;
pub use game::{
    Board, MoveRecord, Orientation, Player, Ruleset, ShipKind, ShipPlacement, ShotOutcome,
    TrackingBoard, TrackingCell, HEIGHT, TOTAL_SHIP_CELLS, WIDTH,
};
pub use strategy::{
    compute_bayesian_density, count_arm, evaluate_crosshair_score, choose_open_side_target,
    find_largest_empty_rectangles, place_random_fleet, remove_sunk_hits,
    AdaptiveStrideUpAttack, AntiMidpointDefense, AttackStrategy, BaitAndHideDefense,
    BayesianPdfAttack, CheckerboardAttack, ClusteredDefense, CornerEdgeDefense,
    CrosshairDescendingAttack, CrosshairStrideAttack, DefenseStrategy, DescendingStrideAttack,
    HuntAndTargetAttack, LargestRectMidpointAttack, MonteCarloAttack, ParallelLanesDefense,
    RandomDefense, RectCheckerboardAttack, RectDescendingStrideAttack, RectStrideFourAttack,
    SpacedDefense, StaggeredFourToTwoAttack, TargetTracker,
};
pub use sim::{
    compute_stats, print_benchmark_summary, print_cross_ruleset_summary, run_benchmark,
    run_benchmark_with_ruleset, simulate_game, simulate_game_with_ruleset, simulate_match,
    simulate_match_with_ruleset, BenchmarkReport, HeadToHeadStats, MatchReplay, StrategyStats,
};
pub use server::run_server;
