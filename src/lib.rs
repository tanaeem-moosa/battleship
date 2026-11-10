pub mod rng;
pub mod game;
pub mod strategy;
pub mod sim;
pub mod server;

pub use rng::Rng;
pub use game::{
    Board, MoveRecord, Orientation, Player, ShipKind, ShipPlacement, ShotOutcome, TrackingBoard,
    TrackingCell, HEIGHT, TOTAL_SHIP_CELLS, WIDTH,
};
pub use strategy::{
    AttackStrategy, CheckerboardAttack, DefenseStrategy, HuntAndTargetAttack, RandomDefense,
};
pub use sim::{
    compute_stats, print_benchmark_summary, run_benchmark, simulate_game, simulate_match,
    BenchmarkReport, HeadToHeadStats, MatchReplay, StrategyStats,
};
pub use server::run_server;
