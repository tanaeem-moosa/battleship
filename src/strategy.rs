//! Battleship defense and attack strategies.
//!
//! Provides:
//! - `DefenseStrategy` trait and `RandomDefense` implementation.
//! - `AttackStrategy` trait with `HuntAndTargetAttack` and `CheckerboardAttack`.
//! - Shared `TargetTracker` providing smart collinear and orthogonal hunt-down logic.

use crate::game::{
    Board, Orientation, ShipKind, ShipPlacement, ShotOutcome, TrackingBoard, TrackingCell,
    HEIGHT, WIDTH,
};
use crate::rng::Rng;

/// Defense placement strategy trait.
pub trait DefenseStrategy {
    /// Strategy identifier (e.g. "random", "spaced", "corner_edge").
    fn id(&self) -> &'static str;
    /// Strategy display name.
    fn name(&self) -> &'static str;
    /// Generate a valid 10x10 board with all 5 ships legally placed.
    fn place_ships(&mut self, rng: &mut Rng) -> Board;
}

/// Attack strategy trait.
pub trait AttackStrategy {
    /// Clone strategy into a Box trait object.
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        unimplemented!("clone_box not implemented for this strategy")
    }
    /// Strategy name.
    fn name(&self) -> &'static str;
    /// Reset internal state before a new game begins.
    fn reset(&mut self);
    /// Choose the coordinate (row, col) for the next shot.
    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize);
    /// Choose multiple coordinates for a Salvo volley using virtual-miss hallucination planning.
    fn choose_salvo(
        &mut self,
        view: &TrackingBoard,
        count: usize,
        rng: &mut Rng,
    ) -> Vec<(usize, usize)> {
        let unshot = view.unshot_cells();
        let k = count.min(unshot.len());
        if k == 0 {
            return Vec::new();
        }
        if k == 1 {
            return vec![self.choose_shot(view, rng)];
        }

        // k > 1 (Virtual-Miss Hallucination / Constant-Liar Batch Planning)
        let first = self.choose_shot(view, rng);
        let mut shots = Vec::with_capacity(k);
        shots.push(first);

        let mut planner = self.clone_box();
        let mut virtual_view = view.clone();
        virtual_view.record_shot(first.0, first.1, ShotOutcome::Miss);
        planner.on_shot_result(first, ShotOutcome::Miss, &virtual_view);

        for _ in 1..k {
            let mut next = planner.choose_shot(&virtual_view, rng);
            if shots.contains(&next) || !virtual_view.is_unshot(next.0, next.1) {
                let avail: Vec<(usize, usize)> = virtual_view
                    .unshot_cells()
                    .into_iter()
                    .filter(|c| !shots.contains(c))
                    .collect();
                if let Some(&fb) = rng.choose(&avail) {
                    next = fb;
                }
            }
            shots.push(next);
            virtual_view.record_shot(next.0, next.1, ShotOutcome::Miss);
            planner.on_shot_result(next, ShotOutcome::Miss, &virtual_view);
        }

        shots
    }
    /// Notify strategy of the result of the shot just fired at pos.
    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard);
    /// Current strategy mode string ("hunt", "checkerboard", "target").
    fn current_mode(&self) -> &'static str;
}

impl Clone for Box<dyn AttackStrategy> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Helper function to generate a valid random 5-ship fleet.
pub fn place_random_fleet(rng: &mut Rng) -> Board {
    loop {
        let mut board = Board::new();
        let mut success = true;

        for &kind in &ShipKind::ALL {
            let len = kind.length();
            let mut placed = false;

            for _ in 0..300 {
                let orient = if rng.next_bool() {
                    Orientation::Horizontal
                } else {
                    Orientation::Vertical
                };

                let max_r = match orient {
                    Orientation::Horizontal => HEIGHT,
                    Orientation::Vertical => HEIGHT + 1 - len,
                };
                let max_c = match orient {
                    Orientation::Horizontal => WIDTH + 1 - len,
                    Orientation::Vertical => WIDTH,
                };

                let r = rng.next_usize(max_r);
                let c = rng.next_usize(max_c);
                let placement = ShipPlacement::new(kind, r, c, orient);

                if board.can_place(&placement) {
                    board.place_ship(placement).unwrap();
                    placed = true;
                    break;
                }
            }

            if !placed {
                success = false;
                break;
            }
        }

        if success && board.is_valid_fleet() {
            return board;
        }
    }
}

/// Defense Strategy 1: Uniform Random
/// - Places all 5 ships with uniform orientation and coordinates across the board.
#[derive(Clone, Debug, Default)]
pub struct RandomDefense;

impl RandomDefense {
    pub fn new() -> Self {
        Self
    }
}

impl DefenseStrategy for RandomDefense {
    fn id(&self) -> &'static str {
        "random"
    }

    fn name(&self) -> &'static str {
        "Uniform Random"
    }

    fn place_ships(&mut self, rng: &mut Rng) -> Board {
        place_random_fleet(rng)
    }
}

/// Defense Strategy 2: Spaced (No-Touching)
/// - Places all 5 ships such that no two ships touch orthogonally or diagonally.
/// - Requires at least a 1-cell water buffer halo around every ship.
#[derive(Clone, Debug, Default)]
pub struct SpacedDefense;

impl SpacedDefense {
    pub fn new() -> Self {
        Self
    }
}

impl DefenseStrategy for SpacedDefense {
    fn id(&self) -> &'static str {
        "spaced"
    }

    fn name(&self) -> &'static str {
        "Spaced (No-Touching)"
    }

    fn place_ships(&mut self, rng: &mut Rng) -> Board {
        // Phase 1: Try full Chebyshev distance >= 2 (no orthogonal or diagonal touch)
        for _ in 0..200 {
            let mut board = Board::new();
            let mut occupied_cells = Vec::new();
            let mut success = true;

            for &kind in &ShipKind::ALL {
                let len = kind.length();
                let mut candidates = Vec::new();

                for &orient in &[Orientation::Horizontal, Orientation::Vertical] {
                    let max_r = match orient {
                        Orientation::Horizontal => HEIGHT,
                        Orientation::Vertical => HEIGHT + 1 - len,
                    };
                    let max_c = match orient {
                        Orientation::Horizontal => WIDTH + 1 - len,
                        Orientation::Vertical => WIDTH,
                    };

                    for r in 0..max_r {
                        for c in 0..max_c {
                            let placement = ShipPlacement::new(kind, r, c, orient);
                            if board.can_place(&placement) {
                                let mut has_buffer = true;
                                for cell in placement.cells() {
                                    for &(or, oc) in &occupied_cells {
                                        if (cell.0 as isize - or as isize).abs() <= 1
                                            && (cell.1 as isize - oc as isize).abs() <= 1
                                        {
                                            has_buffer = false;
                                            break;
                                        }
                                    }
                                    if !has_buffer {
                                        break;
                                    }
                                }
                                if has_buffer {
                                    candidates.push(placement);
                                }
                            }
                        }
                    }
                }

                if candidates.is_empty() {
                    success = false;
                    break;
                }

                let chosen = rng.choose(&candidates).unwrap().clone();
                for cell in chosen.cells() {
                    occupied_cells.push(cell);
                }
                board.place_ship(chosen).unwrap();
            }

            if success && board.is_valid_fleet() {
                return board;
            }
        }

        // Phase 2: Fall back to orthogonal-only buffer
        for _ in 0..100 {
            let mut board = Board::new();
            let mut occupied_cells = Vec::new();
            let mut success = true;

            for &kind in &ShipKind::ALL {
                let len = kind.length();
                let mut candidates = Vec::new();

                for &orient in &[Orientation::Horizontal, Orientation::Vertical] {
                    let max_r = match orient {
                        Orientation::Horizontal => HEIGHT,
                        Orientation::Vertical => HEIGHT + 1 - len,
                    };
                    let max_c = match orient {
                        Orientation::Horizontal => WIDTH + 1 - len,
                        Orientation::Vertical => WIDTH,
                    };

                    for r in 0..max_r {
                        for c in 0..max_c {
                            let placement = ShipPlacement::new(kind, r, c, orient);
                            if board.can_place(&placement) {
                                let mut has_buffer = true;
                                for cell in placement.cells() {
                                    for &(or, oc) in &occupied_cells {
                                        if (cell.0 as isize - or as isize).abs()
                                            + (cell.1 as isize - oc as isize).abs()
                                            <= 1
                                        {
                                            has_buffer = false;
                                            break;
                                        }
                                    }
                                    if !has_buffer {
                                        break;
                                    }
                                }
                                if has_buffer {
                                    candidates.push(placement);
                                }
                            }
                        }
                    }
                }

                if candidates.is_empty() {
                    success = false;
                    break;
                }

                let chosen = rng.choose(&candidates).unwrap().clone();
                for cell in chosen.cells() {
                    occupied_cells.push(cell);
                }
                board.place_ship(chosen).unwrap();
            }

            if success && board.is_valid_fleet() {
                return board;
            }
        }

        place_random_fleet(rng)
    }
}

/// Defense Strategy 3: Corner & Edge Hugging
/// - Prioritizes outer border rows/columns (row 0 or 9, col 0 or 9) and touches corner cells.
/// - Maintains at least 1-cell gaps along perimeter so enemies cannot chain-sink ships.
#[derive(Clone, Debug, Default)]
pub struct CornerEdgeDefense;

impl CornerEdgeDefense {
    pub fn new() -> Self {
        Self
    }
}

impl DefenseStrategy for CornerEdgeDefense {
    fn id(&self) -> &'static str {
        "corner_edge"
    }

    fn name(&self) -> &'static str {
        "Corner & Edge Hugging"
    }

    fn place_ships(&mut self, rng: &mut Rng) -> Board {
        for _ in 0..200 {
            let mut board = Board::new();
            let mut success = true;

            for &kind in &ShipKind::ALL {
                let len = kind.length();
                let mut candidates = Vec::new();

                for &orient in &[Orientation::Horizontal, Orientation::Vertical] {
                    let max_r = match orient {
                        Orientation::Horizontal => HEIGHT,
                        Orientation::Vertical => HEIGHT + 1 - len,
                    };
                    let max_c = match orient {
                        Orientation::Horizontal => WIDTH + 1 - len,
                        Orientation::Vertical => WIDTH,
                    };

                    for r in 0..max_r {
                        for c in 0..max_c {
                            let placement = ShipPlacement::new(kind, r, c, orient);
                            if board.can_place(&placement) {
                                let cells = placement.cells();
                                let on_outer_border = cells
                                    .iter()
                                    .all(|&(cr, cc)| cr == 0 || cr == 9 || cc == 0 || cc == 9);
                                let on_near_border = cells
                                    .iter()
                                    .all(|&(cr, cc)| cr <= 1 || cr >= 8 || cc <= 1 || cc >= 8);

                                let mut has_gap = true;
                                for &(cr, cc) in &cells {
                                    for &(dr, dc) in &[(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                                        let nr = cr as isize + dr;
                                        let nc = cc as isize + dc;
                                        if nr >= 0 && nr < HEIGHT as isize && nc >= 0 && nc < WIDTH as isize {
                                            if board.is_occupied(nr as usize, nc as usize) {
                                                has_gap = false;
                                                break;
                                            }
                                        }
                                    }
                                    if !has_gap {
                                        break;
                                    }
                                }

                                if has_gap {
                                    let touches_corner = cells.iter().any(|&(cr, cc)| {
                                        (cr == 0 || cr == 9) && (cc == 0 || cc == 9)
                                    });

                                    let score = if on_outer_border {
                                        if touches_corner { 100 } else { 80 }
                                    } else if on_near_border {
                                        50
                                    } else {
                                        10
                                    };

                                    candidates.push((placement, score));
                                }
                            }
                        }
                    }
                }

                if candidates.is_empty() {
                    success = false;
                    break;
                }

                let max_score = candidates.iter().map(|(_, s)| *s).max().unwrap();
                let top_cands: Vec<ShipPlacement> = candidates
                    .into_iter()
                    .filter(|(_, s)| *s == max_score)
                    .map(|(p, _)| p)
                    .collect();

                let chosen = rng.choose(&top_cands).unwrap().clone();
                board.place_ship(chosen).unwrap();
            }

            if success && board.is_valid_fleet() {
                return board;
            }
        }

        place_random_fleet(rng)
    }
}

/// Defense Strategy 4: Tightly Clustered Phalanx
/// - Places ships in a tightly packed cluster by maximizing orthogonal contact with existing ships.
/// - Leaves 75%+ of the ocean completely barren.
#[derive(Clone, Debug, Default)]
pub struct ClusteredDefense;

impl ClusteredDefense {
    pub fn new() -> Self {
        Self
    }
}

impl DefenseStrategy for ClusteredDefense {
    fn id(&self) -> &'static str {
        "clustered"
    }

    fn name(&self) -> &'static str {
        "Tightly Clustered Phalanx"
    }

    fn place_ships(&mut self, rng: &mut Rng) -> Board {
        for _ in 0..200 {
            let mut board = Board::new();
            let mut success = true;

            for (i, &kind) in ShipKind::ALL.iter().enumerate() {
                let len = kind.length();
                let mut candidates = Vec::new();

                for &orient in &[Orientation::Horizontal, Orientation::Vertical] {
                    let max_r = match orient {
                        Orientation::Horizontal => HEIGHT,
                        Orientation::Vertical => HEIGHT + 1 - len,
                    };
                    let max_c = match orient {
                        Orientation::Horizontal => WIDTH + 1 - len,
                        Orientation::Vertical => WIDTH,
                    };

                    for r in 0..max_r {
                        for c in 0..max_c {
                            let placement = ShipPlacement::new(kind, r, c, orient);
                            if board.can_place(&placement) {
                                if i == 0 {
                                    candidates.push((placement, 1));
                                } else {
                                    let mut contact = 0usize;
                                    for &(cr, cc) in &placement.cells() {
                                        for &(dr, dc) in &[(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                                            let nr = cr as isize + dr;
                                            let nc = cc as isize + dc;
                                            if nr >= 0 && nr < HEIGHT as isize && nc >= 0 && nc < WIDTH as isize {
                                                if board.is_occupied(nr as usize, nc as usize) {
                                                    contact += 1;
                                                }
                                            }
                                        }
                                    }
                                    candidates.push((placement, contact));
                                }
                            }
                        }
                    }
                }

                if candidates.is_empty() {
                    success = false;
                    break;
                }

                let max_score = candidates.iter().map(|(_, s)| *s).max().unwrap();
                let top_cands: Vec<ShipPlacement> = candidates
                    .into_iter()
                    .filter(|(_, s)| *s == max_score)
                    .map(|(p, _)| p)
                    .collect();

                let chosen = rng.choose(&top_cands).unwrap().clone();
                board.place_ship(chosen).unwrap();
            }

            if success && board.is_valid_fleet() {
                return board;
            }
        }

        place_random_fleet(rng)
    }
}

/// Defense Strategy 5: Anti-Midpoint (Bisection Dodger)
/// - Specifically dodges Largest Empty Rectangle midpoints and bisection lines.
/// - Penalizes primary bisections (rows/cols 4, 5) and secondary bisections (rows/cols 2, 7).
#[derive(Clone, Debug, Default)]
pub struct AntiMidpointDefense;

impl AntiMidpointDefense {
    pub fn new() -> Self {
        Self
    }
}

impl DefenseStrategy for AntiMidpointDefense {
    fn id(&self) -> &'static str {
        "anti_midpoint"
    }

    fn name(&self) -> &'static str {
        "Anti-Midpoint (Bisection Dodger)"
    }

    fn place_ships(&mut self, rng: &mut Rng) -> Board {
        for _ in 0..200 {
            let mut board = Board::new();
            let mut success = true;

            for &kind in &ShipKind::ALL {
                let len = kind.length();
                let mut candidates = Vec::new();

                for &orient in &[Orientation::Horizontal, Orientation::Vertical] {
                    let max_r = match orient {
                        Orientation::Horizontal => HEIGHT,
                        Orientation::Vertical => HEIGHT + 1 - len,
                    };
                    let max_c = match orient {
                        Orientation::Horizontal => WIDTH + 1 - len,
                        Orientation::Vertical => WIDTH,
                    };

                    for r in 0..max_r {
                        for c in 0..max_c {
                            let placement = ShipPlacement::new(kind, r, c, orient);
                            if board.can_place(&placement) {
                                let mut cost = 0usize;
                                for &(cr, cc) in &placement.cells() {
                                    if cr == 4 || cr == 5 {
                                        cost += 10;
                                    }
                                    if cc == 4 || cc == 5 {
                                        cost += 10;
                                    }
                                    if cr == 2 || cr == 7 {
                                        cost += 4;
                                    }
                                    if cc == 2 || cc == 7 {
                                        cost += 4;
                                    }
                                    for &(dr, dc) in &[(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                                        let nr = cr as isize + dr;
                                        let nc = cc as isize + dc;
                                        if nr >= 0 && nr < HEIGHT as isize && nc >= 0 && nc < WIDTH as isize {
                                            if board.is_occupied(nr as usize, nc as usize) {
                                                cost += 2;
                                            }
                                        }
                                    }
                                }
                                candidates.push((placement, cost));
                            }
                        }
                    }
                }

                if candidates.is_empty() {
                    success = false;
                    break;
                }

                let min_cost = candidates.iter().map(|(_, c)| *c).min().unwrap();
                let best_cands: Vec<ShipPlacement> = candidates
                    .into_iter()
                    .filter(|(_, c)| *c == min_cost)
                    .map(|(p, _)| p)
                    .collect();

                let chosen = rng.choose(&best_cands).unwrap().clone();
                board.place_ship(chosen).unwrap();
            }

            if success && board.is_valid_fleet() {
                return board;
            }
        }

        place_random_fleet(rng)
    }
}

/// Defense Strategy 6: Bait Capital + Corner Destroyer
/// - Places small endgame ships (Destroyer, Submarine) in extreme corners.
/// - Places capital ships (Carrier, Battleship, Cruiser) in the interior (rows/cols 2..=7)
///   with a Manhattan buffer >= 2 from small ships, preventing target-mode splash damage.
#[derive(Clone, Debug, Default)]
pub struct BaitAndHideDefense;

impl BaitAndHideDefense {
    pub fn new() -> Self {
        Self
    }
}

impl DefenseStrategy for BaitAndHideDefense {
    fn id(&self) -> &'static str {
        "bait_and_hide"
    }

    fn name(&self) -> &'static str {
        "Bait Capital + Corner Destroyer"
    }

    fn place_ships(&mut self, rng: &mut Rng) -> Board {
        let corners = [(0, 0), (0, 9), (9, 0), (9, 9)];

        for _ in 0..200 {
            let mut board = Board::new();
            let mut small_ship_cells = Vec::new();
            let mut success = true;

            // Step 1: Place small endgame ships (Destroyer, Submarine) first on corners/outer edges
            for &kind in &[ShipKind::Destroyer, ShipKind::Submarine] {
                let len = kind.length();
                let mut candidates = Vec::new();

                for &orient in &[Orientation::Horizontal, Orientation::Vertical] {
                    let max_r = match orient {
                        Orientation::Horizontal => HEIGHT,
                        Orientation::Vertical => HEIGHT + 1 - len,
                    };
                    let max_c = match orient {
                        Orientation::Horizontal => WIDTH + 1 - len,
                        Orientation::Vertical => WIDTH,
                    };

                    for r in 0..max_r {
                        for c in 0..max_c {
                            let placement = ShipPlacement::new(kind, r, c, orient);
                            if board.can_place(&placement) {
                                let cells = placement.cells();
                                let on_outer = cells
                                    .iter()
                                    .all(|&(cr, cc)| cr == 0 || cr == 9 || cc == 0 || cc == 9);
                                if on_outer {
                                    let min_corner_dist = cells
                                        .iter()
                                        .map(|&(cr, cc)| {
                                            corners
                                                .iter()
                                                .map(|&(cor_r, cor_c)| {
                                                    (cr as isize - cor_r as isize).abs()
                                                        + (cc as isize - cor_c as isize).abs()
                                                })
                                                .min()
                                                .unwrap()
                                        })
                                        .min()
                                        .unwrap();

                                    let mut has_gap = true;
                                    for &(cr, cc) in &cells {
                                        for &(sr, sc) in &small_ship_cells {
                                            if (cr as isize - sr as isize).abs() <= 1
                                                && (cc as isize - sc as isize).abs() <= 1
                                            {
                                                has_gap = false;
                                                break;
                                            }
                                        }
                                        if !has_gap {
                                            break;
                                        }
                                    }

                                    if has_gap {
                                        candidates.push((placement, min_corner_dist));
                                    }
                                }
                            }
                        }
                    }
                }

                if candidates.is_empty() {
                    success = false;
                    break;
                }

                let min_dist = candidates.iter().map(|(_, d)| *d).min().unwrap();
                let best: Vec<ShipPlacement> = candidates
                    .into_iter()
                    .filter(|(_, d)| *d == min_dist)
                    .map(|(p, _)| p)
                    .collect();

                let chosen = rng.choose(&best).unwrap().clone();
                for cell in chosen.cells() {
                    small_ship_cells.push(cell);
                }
                board.place_ship(chosen).unwrap();
            }

            if !success {
                continue;
            }

            // Step 2: Place capital ships (Carrier, Battleship, Cruiser) in interior (2..=7)
            for &kind in &[ShipKind::Carrier, ShipKind::Battleship, ShipKind::Cruiser] {
                let len = kind.length();
                let mut candidates = Vec::new();

                for &orient in &[Orientation::Horizontal, Orientation::Vertical] {
                    let max_r = match orient {
                        Orientation::Horizontal => HEIGHT,
                        Orientation::Vertical => HEIGHT + 1 - len,
                    };
                    let max_c = match orient {
                        Orientation::Horizontal => WIDTH + 1 - len,
                        Orientation::Vertical => WIDTH,
                    };

                    for r in 0..max_r {
                        for c in 0..max_c {
                            let placement = ShipPlacement::new(kind, r, c, orient);
                            if board.can_place(&placement) {
                                let cells = placement.cells();
                                let interior_count = cells
                                    .iter()
                                    .filter(|&&(cr, cc)| cr >= 2 && cr <= 7 && cc >= 2 && cc <= 7)
                                    .count();

                                let mut safe_from_splash = true;
                                for &(cr, cc) in &cells {
                                    for &(sr, sc) in &small_ship_cells {
                                        if (cr as isize - sr as isize).abs()
                                            + (cc as isize - sc as isize).abs()
                                            < 2
                                        {
                                            safe_from_splash = false;
                                            break;
                                        }
                                    }
                                    if !safe_from_splash {
                                        break;
                                    }
                                }

                                if safe_from_splash {
                                    candidates.push((placement, interior_count));
                                }
                            }
                        }
                    }
                }

                if candidates.is_empty() {
                    success = false;
                    break;
                }

                let max_interior = candidates.iter().map(|(_, c)| *c).max().unwrap();
                let best: Vec<ShipPlacement> = candidates
                    .into_iter()
                    .filter(|(_, c)| *c == max_interior)
                    .map(|(p, _)| p)
                    .collect();

                let chosen = rng.choose(&best).unwrap().clone();
                board.place_ship(chosen).unwrap();
            }

            if success && board.is_valid_fleet() {
                return board;
            }
        }

        place_random_fleet(rng)
    }
}

/// Defense Strategy 7: Parallel Lanes (Single Axis)
/// - Picks a single orientation (Horizontal or Vertical) for the entire fleet.
/// - Places all 5 ships in that orientation, preferring non-touching parallel lanes.
#[derive(Clone, Debug, Default)]
pub struct ParallelLanesDefense;

impl ParallelLanesDefense {
    pub fn new() -> Self {
        Self
    }
}

impl DefenseStrategy for ParallelLanesDefense {
    fn id(&self) -> &'static str {
        "parallel_lanes"
    }

    fn name(&self) -> &'static str {
        "Parallel Lanes (Single Axis)"
    }

    fn place_ships(&mut self, rng: &mut Rng) -> Board {
        let single_orient = if rng.next_bool() {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        };

        for _ in 0..200 {
            let mut board = Board::new();
            let mut success = true;

            for &kind in &ShipKind::ALL {
                let len = kind.length();
                let mut candidates = Vec::new();

                let max_r = match single_orient {
                    Orientation::Horizontal => HEIGHT,
                    Orientation::Vertical => HEIGHT + 1 - len,
                };
                let max_c = match single_orient {
                    Orientation::Horizontal => WIDTH + 1 - len,
                    Orientation::Vertical => WIDTH,
                };

                for r in 0..max_r {
                    for c in 0..max_c {
                        let placement = ShipPlacement::new(kind, r, c, single_orient);
                        if board.can_place(&placement) {
                            let mut touches = false;
                            for &(cr, cc) in &placement.cells() {
                                for &(dr, dc) in &[(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                                    let nr = cr as isize + dr;
                                    let nc = cc as isize + dc;
                                    if nr >= 0 && nr < HEIGHT as isize && nc >= 0 && nc < WIDTH as isize {
                                        if board.is_occupied(nr as usize, nc as usize) {
                                            touches = true;
                                            break;
                                        }
                                    }
                                }
                                if touches {
                                    break;
                                }
                            }
                            candidates.push((placement, if touches { 0 } else { 1 }));
                        }
                    }
                }

                if candidates.is_empty() {
                    success = false;
                    break;
                }

                let max_score = candidates.iter().map(|(_, s)| *s).max().unwrap();
                let best: Vec<ShipPlacement> = candidates
                    .into_iter()
                    .filter(|(_, s)| *s == max_score)
                    .map(|(p, _)| p)
                    .collect();

                let chosen = rng.choose(&best).unwrap().clone();
                board.place_ship(chosen).unwrap();
            }

            if success && board.is_valid_fleet() {
                return board;
            }
        }

        place_random_fleet(rng)
    }
}

/// Helper tracking active, unsunk hit coordinates and selecting smart collinear/orthogonal target shots.
#[derive(Clone, Debug, Default)]
pub struct TargetTracker {
    pub active_hits: Vec<(usize, usize)>,
}

impl TargetTracker {
    pub fn new() -> Self {
        Self {
            active_hits: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.active_hits.clear();
    }

    pub fn is_targeting(&self) -> bool {
        !self.active_hits.is_empty()
    }

    pub fn record_hit(&mut self, pos: (usize, usize)) {
        if !self.active_hits.contains(&pos) {
            self.active_hits.push(pos);
        }
    }

    /// Resolve hits when a ship is sunk. Identifies the sunk ship cells and removes them.
    pub fn record_sunk(
        &mut self,
        pos: (usize, usize),
        kind: ShipKind,
        view: &TrackingBoard,
    ) {
        self.record_hit(pos);
        let len = kind.length();

        if self.active_hits.len() <= len {
            self.active_hits.clear();
            return;
        }

        // Find contiguous active hits along the horizontal line containing pos
        let mut h_cells: Vec<(usize, usize)> = Vec::new();
        let r = pos.0;
        let mut c = pos.1;
        while c > 0 && self.active_hits.contains(&(r, c - 1)) {
            c -= 1;
        }
        while c < WIDTH && self.active_hits.contains(&(r, c)) {
            h_cells.push((r, c));
            c += 1;
        }

        // Find contiguous active hits along the vertical line containing pos
        let mut v_cells: Vec<(usize, usize)> = Vec::new();
        let c = pos.1;
        let mut r = pos.0;
        while r > 0 && self.active_hits.contains(&(r - 1, c)) {
            r -= 1;
        }
        while r < HEIGHT && self.active_hits.contains(&(r, c)) {
            v_cells.push((r, c));
            r += 1;
        }

        let h_len = h_cells.len();
        let v_len = v_cells.len();

        let cells_to_remove = if h_len >= len && v_len < len {
            Self::pick_ship_cells(&h_cells, pos, len, view, Orientation::Horizontal)
        } else if v_len >= len && h_len < len {
            Self::pick_ship_cells(&v_cells, pos, len, view, Orientation::Vertical)
        } else if h_len >= len && v_len >= len {
            // Both are candidates (e.g. cross intersection).
            // Prefer exact match length == len
            if h_len == len && v_len != len {
                Self::pick_ship_cells(&h_cells, pos, len, view, Orientation::Horizontal)
            } else if v_len == len && h_len != len {
                Self::pick_ship_cells(&v_cells, pos, len, view, Orientation::Vertical)
            } else {
                // If tied, check bounded endpoints
                Self::pick_ship_cells(&h_cells, pos, len, view, Orientation::Horizontal)
            }
        } else {
            // Neither direction alone reached length len.
            // Remove pos and the (len - 1) closest active hits by Manhattan distance.
            let mut by_dist = self.active_hits.clone();
            by_dist.sort_by_key(|&(ar, ac)| {
                (ar as isize - pos.0 as isize).abs() + (ac as isize - pos.1 as isize).abs()
            });
            by_dist.into_iter().take(len).collect()
        };

        for cell in cells_to_remove {
            if let Some(idx) = self.active_hits.iter().position(|&p| p == cell) {
                self.active_hits.remove(idx);
            }
        }
    }

    fn pick_ship_cells(
        line_cells: &[(usize, usize)],
        pos: (usize, usize),
        len: usize,
        view: &TrackingBoard,
        orientation: Orientation,
    ) -> Vec<(usize, usize)> {
        if line_cells.len() == len {
            return line_cells.to_vec();
        }

        // If line_cells is longer than len, check bounded ends
        let first = line_cells[0];
        let last = line_cells[line_cells.len() - 1];

        let first_bounded = match orientation {
            Orientation::Horizontal => {
                first.1 == 0 || view.cell(first.0, first.1 - 1) == TrackingCell::Miss
            }
            Orientation::Vertical => {
                first.0 == 0 || view.cell(first.0 - 1, first.1) == TrackingCell::Miss
            }
        };

        let last_bounded = match orientation {
            Orientation::Horizontal => {
                last.1 + 1 == WIDTH || view.cell(last.0, last.1 + 1) == TrackingCell::Miss
            }
            Orientation::Vertical => {
                last.0 + 1 == HEIGHT || view.cell(last.0 + 1, last.1) == TrackingCell::Miss
            }
        };

        if first_bounded && !last_bounded {
            return line_cells[..len].to_vec();
        }
        if last_bounded && !first_bounded {
            return line_cells[line_cells.len() - len..].to_vec();
        }

        // Find subsegment of length len that covers pos
        let pos_idx = line_cells.iter().position(|&p| p == pos).unwrap_or(0);
        let start = if pos_idx >= len {
            pos_idx + 1 - len
        } else {
            0
        };
        let end = (start + len).min(line_cells.len());
        let start = end - len;
        line_cells[start..end].to_vec()
    }

    /// Choose the next target mode shot:
    /// 1. Prioritize collinear endpoints or 1-cell bridges if 2+ collinear active hits exist.
    /// 2. Fall back to orthogonal neighbors of active hits.
    /// 3. Returns None if all neighbors of active hits are already shot.
    pub fn choose_target_shot(
        &self,
        view: &TrackingBoard,
        rng: &mut Rng,
    ) -> Option<(usize, usize)> {
        let active_hits = if view.ruleset().is_silent_sinking() {
            view.silent_unexplained_hits()
        } else {
            self.active_hits.clone()
        };

        if active_hits.is_empty() {
            return None;
        }

        // 1. Check for collinear chains (contiguous runs >= 2) and 1-cell bridges
        let mut collinear_candidates = Vec::new();

        // Horizontal collinear chains and bridges
        for r in 0..HEIGHT {
            let mut cols: Vec<usize> = active_hits
                .iter()
                .filter(|(hr, _)| *hr == r)
                .map(|(_, hc)| *hc)
                .collect();
            if cols.len() >= 2 {
                cols.sort_unstable();

                // Check for 1-cell collinear bridges
                for i in 0..(cols.len() - 1) {
                    if cols[i + 1] == cols[i] + 2 && view.is_unshot(r, cols[i] + 1) {
                        collinear_candidates.push((r, cols[i] + 1));
                    }
                }

                // Check for maximal contiguous runs of length >= 2
                let mut i = 0;
                while i < cols.len() {
                    let start = cols[i];
                    while i + 1 < cols.len() && cols[i + 1] == cols[i] + 1 {
                        i += 1;
                    }
                    let end = cols[i];
                    let len = end - start + 1;
                    if len >= 2 {
                        // Immediate left endpoint
                        if start > 0 && view.is_unshot(r, start - 1) {
                            collinear_candidates.push((r, start - 1));
                        }
                        // Immediate right endpoint
                        if end + 1 < WIDTH && view.is_unshot(r, end + 1) {
                            collinear_candidates.push((r, end + 1));
                        }
                    }
                    i += 1;
                }
            }
        }

        // Vertical collinear chains and bridges
        for c in 0..WIDTH {
            let mut rows: Vec<usize> = active_hits
                .iter()
                .filter(|(_, hc)| *hc == c)
                .map(|(hr, _)| *hr)
                .collect();
            if rows.len() >= 2 {
                rows.sort_unstable();

                // Check for 1-cell collinear bridges
                for i in 0..(rows.len() - 1) {
                    if rows[i + 1] == rows[i] + 2 && view.is_unshot(rows[i] + 1, c) {
                        collinear_candidates.push((rows[i] + 1, c));
                    }
                }

                // Check for maximal contiguous runs of length >= 2
                let mut i = 0;
                while i < rows.len() {
                    let start = rows[i];
                    while i + 1 < rows.len() && rows[i + 1] == rows[i] + 1 {
                        i += 1;
                    }
                    let end = rows[i];
                    let len = end - start + 1;
                    if len >= 2 {
                        // Immediate top endpoint
                        if start > 0 && view.is_unshot(start - 1, c) {
                            collinear_candidates.push((start - 1, c));
                        }
                        // Immediate bottom endpoint
                        if end + 1 < HEIGHT && view.is_unshot(end + 1, c) {
                            collinear_candidates.push((end + 1, c));
                        }
                    }
                    i += 1;
                }
            }
        }

        if !collinear_candidates.is_empty() {
            collinear_candidates.sort_unstable();
            collinear_candidates.dedup();
            return rng.choose(&collinear_candidates).copied();
        }

        // 2. Fall back to orthogonal neighbors (N, S, E, W) of any active hit
        let mut neighbor_candidates = Vec::new();
        for &(r, c) in &active_hits {
            if r > 0 && view.is_unshot(r - 1, c) {
                neighbor_candidates.push((r - 1, c));
            }
            if r + 1 < HEIGHT && view.is_unshot(r + 1, c) {
                neighbor_candidates.push((r + 1, c));
            }
            if c > 0 && view.is_unshot(r, c - 1) {
                neighbor_candidates.push((r, c - 1));
            }
            if c + 1 < WIDTH && view.is_unshot(r, c + 1) {
                neighbor_candidates.push((r, c + 1));
            }
        }

        if !neighbor_candidates.is_empty() {
            neighbor_candidates.sort_unstable();
            neighbor_candidates.dedup();
            return rng.choose(&neighbor_candidates).copied();
        }

        // All immediate neighbors of active hits are exhausted
        None
    }
}

/// Attack Strategy 1: Random Hunt + Smart Collinear Target
/// - Hunt mode: uniformly selects among all unshot cells.
/// - Target mode: uses smart collinear and orthogonal hunt-down.
#[derive(Clone, Debug, Default)]
pub struct HuntAndTargetAttack {
    tracker: TargetTracker,
    mode: &'static str,
}

impl HuntAndTargetAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            mode: "hunt",
        }
    }
}

impl AttackStrategy for HuntAndTargetAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Random + Hunt/Target"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.mode = "hunt";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        if self.tracker.is_targeting() {
            if let Some(target_pos) = self.tracker.choose_target_shot(view, rng) {
                self.mode = "target";
                return target_pos;
            }
            // If target neighbors are blocked, clear stuck active hits and fall back to hunt
            self.tracker.reset();
        }

        self.mode = "hunt";
        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }
        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");
        *rng.choose(&unshot).unwrap()
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Attack Strategy 2: Checkerboard Hunt + Smart Collinear Target
/// - Hunt mode: fires only at cells matching checkerboard parity ((r + c) % 2 == parity).
///   Since all ships have length >= 2, every ship intersects the checkerboard!
///   Falls back to any unshot cell if parity cells are exhausted.
/// - Target mode: fires at orthogonal neighbors and collinear endpoints regardless of parity.
#[derive(Clone, Debug)]
pub struct CheckerboardAttack {
    tracker: TargetTracker,
    parity: Option<usize>,
    mode: &'static str,
}

impl Default for CheckerboardAttack {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckerboardAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            parity: None,
            mode: "checkerboard",
        }
    }
}

impl AttackStrategy for CheckerboardAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Checkerboard + Hunt/Target"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.parity = None;
        self.mode = "checkerboard";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        // Initialize random parity (0 or 1) on first shot of the game
        let parity = *self.parity.get_or_insert_with(|| rng.next_usize(2));

        if self.tracker.is_targeting() {
            if let Some(target_pos) = self.tracker.choose_target_shot(view, rng) {
                self.mode = "target";
                return target_pos;
            }
            self.tracker.reset();
        }

        self.mode = "checkerboard";
        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        // Filter unshot cells matching parity
        let parity_unshot: Vec<(usize, usize)> = unshot
            .iter()
            .copied()
            .filter(|&(r, c)| (r + c) % 2 == parity)
            .collect();

        if !parity_unshot.is_empty() {
            *rng.choose(&parity_unshot).unwrap()
        } else {
            if let Some(pos) = silent_endgame_fallback(view, rng) {
                return pos;
            }
            // Parity cells exhausted, fall back to any unshot cell
            *rng.choose(&unshot).unwrap()
        }
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Find all largest axis-aligned rectangles (r0..=r1, c0..=c1) consisting entirely
/// of unshot cells. Maximizes area (H * W), breaking ties by min(H, W) (preferring square-ish shapes).
pub fn find_largest_empty_rectangles(
    view: &TrackingBoard,
) -> (usize, usize, Vec<(usize, usize, usize, usize)>) {
    let mut shot_prefix = [[0usize; WIDTH + 1]; HEIGHT + 1];
    for r in 0..HEIGHT {
        for c in 0..WIDTH {
            let val = if view.is_unshot(r, c) { 0 } else { 1 };
            shot_prefix[r + 1][c + 1] =
                shot_prefix[r + 1][c] + shot_prefix[r][c + 1] - shot_prefix[r][c] + val;
        }
    }

    let mut max_area = 0;
    let mut max_min_dim = 0;
    let mut best_rects = Vec::new();

    for r0 in 0..HEIGHT {
        for r1 in r0..HEIGHT {
            let h = r1 - r0 + 1;
            for c0 in 0..WIDTH {
                for c1 in c0..WIDTH {
                    let w = c1 - c0 + 1;
                    let area = h * w;
                    let min_dim = h.min(w);

                    if area < max_area || (area == max_area && min_dim < max_min_dim) {
                        continue;
                    }

                    let shots = shot_prefix[r1 + 1][c1 + 1]
                        + shot_prefix[r0][c0]
                        - shot_prefix[r0][c1 + 1]
                        - shot_prefix[r1 + 1][c0];

                    if shots == 0 {
                        if area > max_area || (area == max_area && min_dim > max_min_dim) {
                            max_area = area;
                            max_min_dim = min_dim;
                            best_rects.clear();
                            best_rects.push((r0, r1, c0, c1));
                        } else if area == max_area && min_dim == max_min_dim {
                            best_rects.push((r0, r1, c0, c1));
                        }
                    }
                }
            }
        }
    }

    (max_area, max_min_dim, best_rects)
}

/// Attack Strategy 3: Largest Empty Rect Midpoint
/// - Hunt mode: Finds the largest all-unshot rectangle (area = H * W, tie-break by min(H, W))
///   and fires at its midpoint ((r0 + r1)/2, (c0 + c1)/2).
/// - Target mode: Smart collinear and orthogonal hunt-down.
#[derive(Clone, Debug, Default)]
pub struct LargestRectMidpointAttack {
    tracker: TargetTracker,
    mode: &'static str,
}

impl LargestRectMidpointAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            mode: "largest_rect",
        }
    }
}

impl AttackStrategy for LargestRectMidpointAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Largest Empty Rect Midpoint"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.mode = "largest_rect";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        if self.tracker.is_targeting() {
            if let Some(target_pos) = self.tracker.choose_target_shot(view, rng) {
                self.mode = "target";
                return target_pos;
            }
            self.tracker.reset();
        }

        self.mode = "largest_rect";
        let (_, _, best_rects) = find_largest_empty_rectangles(view);

        if let Some(&(r0, r1, c0, c1)) = rng.choose(&best_rects) {
            let mid_r = (r0 + r1) / 2;
            let mid_c = (c0 + c1) / 2;
            if view.is_unshot(mid_r, mid_c) {
                return (mid_r, mid_c);
            }
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");
        *rng.choose(&unshot).unwrap()
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Attack Strategy 4: Adaptive Stride (2 -> 3 -> 4 -> 5)
/// - Hunt mode: Determines smallest remaining unsunk ship length k (starts at 2 while Destroyer
///   is unsunk, steps up to 3, 4, 5 as ships sink). Fires on (r + c) % k == offset % k.
/// - Target mode: Smart collinear and orthogonal hunt-down.
#[derive(Clone, Debug)]
pub struct AdaptiveStrideUpAttack {
    tracker: TargetTracker,
    offset: Option<usize>,
    mode: &'static str,
}

impl Default for AdaptiveStrideUpAttack {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveStrideUpAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            offset: None,
            mode: "adaptive_stride",
        }
    }
}

impl AttackStrategy for AdaptiveStrideUpAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Adaptive Stride (2->3->4->5)"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.offset = None;
        self.mode = "adaptive_stride";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        let offset = *self.offset.get_or_insert_with(|| rng.next_usize(60));

        if self.tracker.is_targeting() {
            if let Some(target_pos) = self.tracker.choose_target_shot(view, rng) {
                self.mode = "target";
                return target_pos;
            }
            self.tracker.reset();
        }

        self.mode = "adaptive_stride";
        let k = view.min_remaining_ship_length().max(2);
        let stride_target = offset % k;

        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        let stride_cells: Vec<(usize, usize)> = unshot
            .iter()
            .copied()
            .filter(|&(r, c)| (r + c) % k == stride_target)
            .collect();

        if !stride_cells.is_empty() {
            return *rng.choose(&stride_cells).unwrap();
        }

        // Fallback: unshot cells where a ship of length k can fit
        let fit_cells: Vec<(usize, usize)> = unshot
            .iter()
            .copied()
            .filter(|&(r, c)| view.can_ship_fit_at(r, c, k))
            .collect();

        if !fit_cells.is_empty() {
            return *rng.choose(&fit_cells).unwrap();
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

        *rng.choose(&unshot).unwrap()
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Attack Strategy 5: Staggered Stride (4 -> 2)
/// - Hunt mode: Hunts on 4-stride grid (r + c) % 4 == offset4 while Carrier (5) or Battleship (4)
///   is still afloat. Once both 4 and 5 are sunk (or 4-stride exhausted), switches to 2-stride
///   checkerboard (r + c) % 2 == (offset4 % 2).
/// - Target mode: Smart collinear and orthogonal hunt-down.
#[derive(Clone, Debug)]
pub struct StaggeredFourToTwoAttack {
    tracker: TargetTracker,
    offset4: Option<usize>,
    mode: &'static str,
}

impl Default for StaggeredFourToTwoAttack {
    fn default() -> Self {
        Self::new()
    }
}

impl StaggeredFourToTwoAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            offset4: None,
            mode: "stride4_to_2",
        }
    }
}

impl AttackStrategy for StaggeredFourToTwoAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Staggered Stride (4->2)"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.offset4 = None;
        self.mode = "stride4_to_2";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        let offset4 = *self.offset4.get_or_insert_with(|| rng.next_usize(4));

        if self.tracker.is_targeting() {
            if let Some(target_pos) = self.tracker.choose_target_shot(view, rng) {
                self.mode = "target";
                return target_pos;
            }
            self.tracker.reset();
        }

        self.mode = "stride4_to_2";
        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        let max_len = view.max_remaining_ship_length();

        // Phase 1: 4-stride while length 4 or 5 ship is still afloat
        if max_len >= 4 {
            let stride4_cells: Vec<(usize, usize)> = unshot
                .iter()
                .copied()
                .filter(|&(r, c)| (r + c) % 4 == offset4)
                .collect();
            if !stride4_cells.is_empty() {
                return *rng.choose(&stride4_cells).unwrap();
            }
        }

        // Phase 2: 2-stride checkerboard (using aligned parity offset4 % 2)
        let parity = offset4 % 2;
        let parity_cells: Vec<(usize, usize)> = unshot
            .iter()
            .copied()
            .filter(|&(r, c)| (r + c) % 2 == parity)
            .collect();

        if !parity_cells.is_empty() {
            return *rng.choose(&parity_cells).unwrap();
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

        *rng.choose(&unshot).unwrap()
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Attack Strategy 6: Largest Rect + Stride (4 -> 2)
/// - Hunt mode: Determines active stride s (4 while ship length >= 4 is afloat, else 2).
///   Finds largest empty rectangle, and selects the unshot active-stride cell closest
///   to the center of that rectangle!
/// - Target mode: Smart collinear and orthogonal hunt-down.
#[derive(Clone, Debug)]
pub struct RectStrideFourAttack {
    tracker: TargetTracker,
    offset4: Option<usize>,
    mode: &'static str,
}

impl Default for RectStrideFourAttack {
    fn default() -> Self {
        Self::new()
    }
}

impl RectStrideFourAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            offset4: None,
            mode: "rect_stride4",
        }
    }
}

impl AttackStrategy for RectStrideFourAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Largest Rect + Stride (4->2)"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.offset4 = None;
        self.mode = "rect_stride4";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        let offset4 = *self.offset4.get_or_insert_with(|| rng.next_usize(4));

        if self.tracker.is_targeting() {
            if let Some(target_pos) = self.tracker.choose_target_shot(view, rng) {
                self.mode = "target";
                return target_pos;
            }
            self.tracker.reset();
        }

        self.mode = "rect_stride4";
        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        let max_len = view.max_remaining_ship_length();
        let s = if max_len >= 4 { 4 } else { 2 };
        let target_rem = offset4 % s;

        // Active-stride unshot cells
        let mut stride_cells: Vec<(usize, usize)> = unshot
            .iter()
            .copied()
            .filter(|&(r, c)| (r + c) % s == target_rem)
            .collect();

        // If stride-4 has no cells left, fall back to stride-2
        if stride_cells.is_empty() && s == 4 {
            let parity = offset4 % 2;
            stride_cells = unshot
                .iter()
                .copied()
                .filter(|&(r, c)| (r + c) % 2 == parity)
                .collect();
        }

        // Find largest empty rectangle
        let (_, _, best_rects) = find_largest_empty_rectangles(view);

        if let Some(&(r0, r1, c0, c1)) = rng.choose(&best_rects) {
            let mid_r = (r0 + r1) as f64 / 2.0;
            let mid_c = (c0 + c1) as f64 / 2.0;

            // Check if any stride cells are inside this rectangle
            let inside_stride: Vec<(usize, usize)> = stride_cells
                .iter()
                .copied()
                .filter(|&(r, c)| r >= r0 && r <= r1 && c >= c0 && c <= c1)
                .collect();

            let candidate_pool = if !inside_stride.is_empty() {
                inside_stride
            } else if !stride_cells.is_empty() {
                stride_cells.clone()
            } else {
                let rect_mid = ((r0 + r1) / 2, (c0 + c1) / 2);
                if view.is_unshot(rect_mid.0, rect_mid.1) {
                    vec![rect_mid]
                } else {
                    unshot.clone()
                }
            };

            // Pick candidate closest to rectangle center (mid_r, mid_c)
            let mut min_d2 = f64::INFINITY;
            let mut best_picks = Vec::new();
            for &(r, c) in &candidate_pool {
                let d2 = (r as f64 - mid_r).powi(2) + (c as f64 - mid_c).powi(2);
                if (d2 - min_d2).abs() < 1e-9 {
                    best_picks.push((r, c));
                } else if d2 < min_d2 {
                    min_d2 = d2;
                    best_picks.clear();
                    best_picks.push((r, c));
                }
            }

            if let Some(&chosen) = rng.choose(&best_picks) {
                return chosen;
            }
        }

        if !stride_cells.is_empty() {
            return *rng.choose(&stride_cells).unwrap();
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

        *rng.choose(&unshot).unwrap()
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Attack Strategy 7: Descending Stride (5 -> 4 -> 3 -> 2)
/// - Hunt mode: Active stride s starts at max remaining ship length (5 while Carrier is alive,
///   4 once Carrier sinks, 3 once Battleship sinks, 2 when only Destroyer remains).
///   Hunts on grid (r + c) % s == (offset % s).
/// - Target mode: Smart collinear and orthogonal hunt-down.
#[derive(Clone, Debug)]
pub struct DescendingStrideAttack {
    tracker: TargetTracker,
    offset: Option<usize>,
    mode: &'static str,
}

impl Default for DescendingStrideAttack {
    fn default() -> Self {
        Self::new()
    }
}

impl DescendingStrideAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            offset: None,
            mode: "stride_5432",
        }
    }
}

impl AttackStrategy for DescendingStrideAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Descending Stride (5->4->3->2)"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.offset = None;
        self.mode = "stride_5432";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        let offset = *self.offset.get_or_insert_with(|| rng.next_usize(60));

        if self.tracker.is_targeting() {
            if let Some(target_pos) = self.tracker.choose_target_shot(view, rng) {
                self.mode = "target";
                return target_pos;
            }
            self.tracker.reset();
        }

        self.mode = "stride_5432";
        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        let max_len = view.max_remaining_ship_length().max(2);

        // Try active stride, and decrement down to 2 if current stride cells are exhausted
        for s in (2..=max_len).rev() {
            let target_rem = offset % s;
            let s_cells: Vec<(usize, usize)> = unshot
                .iter()
                .copied()
                .filter(|&(r, c)| (r + c) % s == target_rem)
                .collect();
            if !s_cells.is_empty() {
                return *rng.choose(&s_cells).unwrap();
            }
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

        *rng.choose(&unshot).unwrap()
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Attack Strategy 8: Largest Rect + Stride (5 -> 4 -> 3 -> 2)
/// - Hunt mode: Active stride s = max_remaining_ship_length. Find largest empty rectangle,
///   and fire at the active-stride cell closest to the center of that rectangle.
/// - Target mode: Smart collinear and orthogonal hunt-down.
#[derive(Clone, Debug)]
pub struct RectDescendingStrideAttack {
    tracker: TargetTracker,
    offset: Option<usize>,
    mode: &'static str,
}

impl Default for RectDescendingStrideAttack {
    fn default() -> Self {
        Self::new()
    }
}

impl RectDescendingStrideAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            offset: None,
            mode: "rect_stride_5432",
        }
    }
}

impl AttackStrategy for RectDescendingStrideAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Largest Rect + Stride (5->4->3->2)"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.offset = None;
        self.mode = "rect_stride_5432";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        let offset = *self.offset.get_or_insert_with(|| rng.next_usize(60));

        if self.tracker.is_targeting() {
            if let Some(target_pos) = self.tracker.choose_target_shot(view, rng) {
                self.mode = "target";
                return target_pos;
            }
            self.tracker.reset();
        }

        self.mode = "rect_stride_5432";
        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        let max_len = view.max_remaining_ship_length().max(2);

        let mut stride_cells = Vec::new();
        for s in (2..=max_len).rev() {
            let target_rem = offset % s;
            stride_cells = unshot
                .iter()
                .copied()
                .filter(|&(r, c)| (r + c) % s == target_rem)
                .collect();
            if !stride_cells.is_empty() {
                break;
            }
        }

        let (_, _, best_rects) = find_largest_empty_rectangles(view);
        if let Some(&(r0, r1, c0, c1)) = rng.choose(&best_rects) {
            let mid_r = (r0 + r1) as f64 / 2.0;
            let mid_c = (c0 + c1) as f64 / 2.0;

            let inside_stride: Vec<(usize, usize)> = stride_cells
                .iter()
                .copied()
                .filter(|&(r, c)| r >= r0 && r <= r1 && c >= c0 && c <= c1)
                .collect();

            let candidate_pool = if !inside_stride.is_empty() {
                inside_stride
            } else if !stride_cells.is_empty() {
                stride_cells.clone()
            } else {
                let rect_mid = ((r0 + r1) / 2, (c0 + c1) / 2);
                if view.is_unshot(rect_mid.0, rect_mid.1) {
                    vec![rect_mid]
                } else {
                    unshot.clone()
                }
            };

            let mut min_d2 = f64::INFINITY;
            let mut best_picks = Vec::new();
            for &(r, c) in &candidate_pool {
                let d2 = (r as f64 - mid_r).powi(2) + (c as f64 - mid_c).powi(2);
                if (d2 - min_d2).abs() < 1e-9 {
                    best_picks.push((r, c));
                } else if d2 < min_d2 {
                    min_d2 = d2;
                    best_picks.clear();
                    best_picks.push((r, c));
                }
            }

            if let Some(&chosen) = rng.choose(&best_picks) {
                return chosen;
            }
        }

        if !stride_cells.is_empty() {
            return *rng.choose(&stride_cells).unwrap();
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

        *rng.choose(&unshot).unwrap()
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Attack Strategy 9: Largest Rect + Checkerboard
/// - Hunt mode: Parity stride 2. Finds the largest empty rectangle, and selects the unshot
///   checkerboard cell closest to the center of that rectangle.
/// - Target mode: Smart collinear and orthogonal hunt-down.
#[derive(Clone, Debug)]
pub struct RectCheckerboardAttack {
    tracker: TargetTracker,
    parity: Option<usize>,
    mode: &'static str,
}

impl Default for RectCheckerboardAttack {
    fn default() -> Self {
        Self::new()
    }
}

impl RectCheckerboardAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            parity: None,
            mode: "rect_checkerboard",
        }
    }
}

impl AttackStrategy for RectCheckerboardAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Largest Rect + Checkerboard"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.parity = None;
        self.mode = "rect_checkerboard";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        let parity = *self.parity.get_or_insert_with(|| rng.next_usize(2));

        if self.tracker.is_targeting() {
            if let Some(target_pos) = self.tracker.choose_target_shot(view, rng) {
                self.mode = "target";
                return target_pos;
            }
            self.tracker.reset();
        }

        self.mode = "rect_checkerboard";
        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        let parity_cells: Vec<(usize, usize)> = unshot
            .iter()
            .copied()
            .filter(|&(r, c)| (r + c) % 2 == parity)
            .collect();

        let (_, _, best_rects) = find_largest_empty_rectangles(view);
        if let Some(&(r0, r1, c0, c1)) = rng.choose(&best_rects) {
            let mid_r = (r0 + r1) as f64 / 2.0;
            let mid_c = (c0 + c1) as f64 / 2.0;

            let inside_parity: Vec<(usize, usize)> = parity_cells
                .iter()
                .copied()
                .filter(|&(r, c)| r >= r0 && r <= r1 && c >= c0 && c <= c1)
                .collect();

            let candidate_pool = if !inside_parity.is_empty() {
                inside_parity
            } else if !parity_cells.is_empty() {
                parity_cells.clone()
            } else {
                let rect_mid = ((r0 + r1) / 2, (c0 + c1) / 2);
                if view.is_unshot(rect_mid.0, rect_mid.1) {
                    vec![rect_mid]
                } else {
                    unshot.clone()
                }
            };

            let mut min_d2 = f64::INFINITY;
            let mut best_picks = Vec::new();
            for &(r, c) in &candidate_pool {
                let d2 = (r as f64 - mid_r).powi(2) + (c as f64 - mid_c).powi(2);
                if (d2 - min_d2).abs() < 1e-9 {
                    best_picks.push((r, c));
                } else if d2 < min_d2 {
                    min_d2 = d2;
                    best_picks.clear();
                    best_picks.push((r, c));
                }
            }

            if let Some(&chosen) = rng.choose(&best_picks) {
                return chosen;
            }
        }

        if !parity_cells.is_empty() {
            return *rng.choose(&parity_cells).unwrap();
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

        *rng.choose(&unshot).unwrap()
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Attack Strategy 10: Monte Carlo Sampling
/// - Simulates random valid fleet configurations consistent with observed Miss/Sunk/Hit cells.
/// - Prioritizes valid configurations covering any active unsunk hits.
/// - Accumulates frequency count across unshot cells and fires at the highest frequency cell.
#[derive(Clone, Debug, Default)]
pub struct MonteCarloAttack {
    tracker: TargetTracker,
    mode: &'static str,
}

impl MonteCarloAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            mode: "monte_carlo",
        }
    }
}

impl AttackStrategy for MonteCarloAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Monte Carlo Sampling"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.mode = "monte_carlo";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        if self.tracker.is_targeting() {
            self.mode = "target";
        } else {
            self.mode = "monte_carlo";
        }

        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        let is_silent = view.ruleset().is_silent_sinking();
        let remaining = if is_silent {
            view.effective_remaining_ships()
        } else {
            view.remaining_unsunk_ships()
        };
        if remaining.is_empty() {
            if let Some(pos) = silent_endgame_fallback(view, rng) {
                return pos;
            }
            return *rng.choose(&unshot).unwrap();
        }

        let silent_hits_holder;
        let active_hits: &[(usize, usize)] = if is_silent {
            silent_hits_holder = view.silent_unexplained_hits();
            &silent_hits_holder[..]
        } else {
            &self.tracker.active_hits[..]
        };
        let mut freq = [[0u32; WIDTH]; HEIGHT];
        let target_samples = 100usize;
        let max_attempts = 250usize;
        let mut successful_samples = 0usize;

        struct CandPlacement {
            r: usize,
            c: usize,
            is_horiz: bool,
            len: usize,
            covers_active_hit: bool,
        }

        let mut ship_cands: Vec<Vec<CandPlacement>> = Vec::with_capacity(remaining.len());
        for &kind in &remaining {
            let len = kind.length();
            let mut list = Vec::new();

            // Horizontal
            for r in 0..HEIGHT {
                for c in 0..=(WIDTH - len) {
                    let mut valid = true;
                    let mut covers = false;
                    for dc in 0..len {
                        let cell = view.cell(r, c + dc);
                        if matches!(cell, TrackingCell::Miss | TrackingCell::Sunk(_)) {
                            valid = false;
                            break;
                        }
                        if active_hits.contains(&(r, c + dc)) {
                            covers = true;
                        }
                    }
                    if valid {
                        list.push(CandPlacement {
                            r,
                            c,
                            is_horiz: true,
                            len,
                            covers_active_hit: covers,
                        });
                    }
                }
            }

            // Vertical
            for c in 0..WIDTH {
                for r in 0..=(HEIGHT - len) {
                    let mut valid = true;
                    let mut covers = false;
                    for dr in 0..len {
                        let cell = view.cell(r + dr, c);
                        if matches!(cell, TrackingCell::Miss | TrackingCell::Sunk(_)) {
                            valid = false;
                            break;
                        }
                        if active_hits.contains(&(r + dr, c)) {
                            covers = true;
                        }
                    }
                    if valid {
                        list.push(CandPlacement {
                            r,
                            c,
                            is_horiz: false,
                            len,
                            covers_active_hit: covers,
                        });
                    }
                }
            }

            ship_cands.push(list);
        }

        for _ in 0..max_attempts {
            if successful_samples >= target_samples {
                break;
            }

            let mut occupied = [[false; WIDTH]; HEIGHT];
            let mut active_hits_covered = 0usize;
            let mut sample_success = true;

            for cands in &ship_cands {
                if cands.is_empty() {
                    sample_success = false;
                    break;
                }

                let valid_for_this_sample: Vec<&CandPlacement> = if !active_hits.is_empty()
                    && active_hits_covered < active_hits.len()
                {
                    let hit_covering: Vec<&CandPlacement> = cands
                        .iter()
                        .filter(|cand| {
                            cand.covers_active_hit && {
                                let mut fits = true;
                                if cand.is_horiz {
                                    for dc in 0..cand.len {
                                        if occupied[cand.r][cand.c + dc] {
                                            fits = false;
                                            break;
                                        }
                                    }
                                } else {
                                    for dr in 0..cand.len {
                                        if occupied[cand.r + dr][cand.c] {
                                            fits = false;
                                            break;
                                        }
                                    }
                                }
                                fits
                            }
                        })
                        .collect();

                    if !hit_covering.is_empty() {
                        hit_covering
                    } else {
                        cands
                            .iter()
                            .filter(|cand| {
                                let mut fits = true;
                                if cand.is_horiz {
                                    for dc in 0..cand.len {
                                        if occupied[cand.r][cand.c + dc] {
                                            fits = false;
                                            break;
                                        }
                                    }
                                } else {
                                    for dr in 0..cand.len {
                                        if occupied[cand.r + dr][cand.c] {
                                            fits = false;
                                            break;
                                        }
                                    }
                                }
                                fits
                            })
                            .collect()
                    }
                } else {
                    cands
                        .iter()
                        .filter(|cand| {
                            let mut fits = true;
                            if cand.is_horiz {
                                for dc in 0..cand.len {
                                    if occupied[cand.r][cand.c + dc] {
                                        fits = false;
                                        break;
                                    }
                                }
                            } else {
                                for dr in 0..cand.len {
                                    if occupied[cand.r + dr][cand.c] {
                                        fits = false;
                                        break;
                                    }
                                }
                            }
                            fits
                        })
                        .collect()
                };

                if valid_for_this_sample.is_empty() {
                    sample_success = false;
                    break;
                }

                let chosen = rng.choose(&valid_for_this_sample).unwrap();
                if chosen.is_horiz {
                    for dc in 0..chosen.len {
                        let row = chosen.r;
                        let col = chosen.c + dc;
                        occupied[row][col] = true;
                        if active_hits.contains(&(row, col)) {
                            active_hits_covered += 1;
                        }
                    }
                } else {
                    for dr in 0..chosen.len {
                        let row = chosen.r + dr;
                        let col = chosen.c;
                        occupied[row][col] = true;
                        if active_hits.contains(&(row, col)) {
                            active_hits_covered += 1;
                        }
                    }
                }
            }

            if sample_success && (active_hits.is_empty() || active_hits_covered >= active_hits.len()) {
                successful_samples += 1;
                for &(r, c) in &unshot {
                    if occupied[r][c] {
                        freq[r][c] += 1;
                    }
                }
            }
        }

        if successful_samples > 0 {
            let mut max_count = 0u32;
            let mut best_cells = Vec::new();
            for &(r, c) in &unshot {
                let count = freq[r][c];
                if count > max_count {
                    max_count = count;
                    best_cells.clear();
                    best_cells.push((r, c));
                } else if count == max_count && max_count > 0 {
                    best_cells.push((r, c));
                }
            }

            if !best_cells.is_empty() {
                return *rng.choose(&best_cells).unwrap();
            }
        }

        if self.tracker.is_targeting() {
            if let Some(pos) = self.tracker.choose_target_shot(view, rng) {
                return pos;
            }
        }

        // Analytical Bayesian density fallback if MC sampling produced 0 valid full-board samples
        let density = compute_bayesian_density(view, &self.tracker.active_hits);
        let mut max_d = 0u64;
        let mut best_density_cells = Vec::new();
        for &(r, c) in &unshot {
            let d = density[r][c];
            if d > max_d {
                max_d = d;
                best_density_cells.clear();
                best_density_cells.push((r, c));
            } else if d == max_d && max_d > 0 {
                best_density_cells.push((r, c));
            }
        }
        if !best_density_cells.is_empty() {
            return *rng.choose(&best_density_cells).unwrap();
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

        *rng.choose(&unshot).unwrap()
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Helper computing the analytical Bayesian cell density across all valid remaining ship placements.
/// Placements covering H active hits get weight 1000^H, while placements covering 0 active hits get weight 1.
pub fn compute_bayesian_density(
    view: &TrackingBoard,
    active_hits: &[(usize, usize)],
) -> [[u64; WIDTH]; HEIGHT] {
    let is_silent = view.ruleset().is_silent_sinking();
    let remaining = if is_silent {
        view.effective_remaining_ships()
    } else {
        view.remaining_unsunk_ships()
    };

    let silent_hits_holder;
    let unexplained = if is_silent {
        silent_hits_holder = view.silent_unexplained_hits();
        &silent_hits_holder[..]
    } else {
        active_hits
    };

    let compute_grid = |allow_all_hits: bool| -> [[u64; WIDTH]; HEIGHT] {
        let mut grid = [[0u64; WIDTH]; HEIGHT];
        let has_active_hits = !unexplained.is_empty();

        for &kind in &remaining {
            let len = kind.length();

            // Horizontal placements: (r, c..c+len)
            for r in 0..HEIGHT {
                for c in 0..=(WIDTH - len) {
                    let mut valid = true;
                    let mut h_count = 0u32;
                    for dc in 0..len {
                        let col = c + dc;
                        let cell = view.cell(r, col);
                        if cell == TrackingCell::Miss || cell.is_sunk() {
                            valid = false;
                            break;
                        }
                        if cell == TrackingCell::Hit {
                            if is_silent && !allow_all_hits && !unexplained.contains(&(r, col)) {
                                valid = false;
                                break;
                            }
                            if unexplained.contains(&(r, col)) || allow_all_hits {
                                h_count += 1;
                            }
                        }
                    }
                    if valid {
                        let weight = if has_active_hits || allow_all_hits {
                            if h_count > 0 {
                                1_000u64.saturating_pow(h_count)
                            } else {
                                1u64
                            }
                        } else {
                            1u64
                        };

                        for dc in 0..len {
                            let col = c + dc;
                            if view.is_unshot(r, col) {
                                grid[r][col] = grid[r][col].saturating_add(weight);
                            }
                        }
                    }
                }
            }

            // Vertical placements: (r..r+len, c)
            for c in 0..WIDTH {
                for r in 0..=(HEIGHT - len) {
                    let mut valid = true;
                    let mut h_count = 0u32;
                    for dr in 0..len {
                        let row = r + dr;
                        let cell = view.cell(row, c);
                        if cell == TrackingCell::Miss || cell.is_sunk() {
                            valid = false;
                            break;
                        }
                        if cell == TrackingCell::Hit {
                            if is_silent && !allow_all_hits && !unexplained.contains(&(row, c)) {
                                valid = false;
                                break;
                            }
                            if unexplained.contains(&(row, c)) || allow_all_hits {
                                h_count += 1;
                            }
                        }
                    }
                    if valid {
                        let weight = if has_active_hits || allow_all_hits {
                            if h_count > 0 {
                                1_000u64.saturating_pow(h_count)
                            } else {
                                1u64
                            }
                        } else {
                            1u64
                        };

                        for dr in 0..len {
                            let row = r + dr;
                            if view.is_unshot(row, c) {
                                grid[row][c] = grid[row][c].saturating_add(weight);
                            }
                        }
                    }
                }
            }
        }

        grid
    };

    let grid = compute_grid(false);

    if is_silent {
        let unshot = view.unshot_cells();
        let max_val = unshot.iter().map(|&(r, c)| grid[r][c]).max().unwrap_or(0);
        if max_val == 0 {
            // Recompute allowing placements over all Hit cells
            return compute_grid(true);
        }
    }

    grid
}

/// Attack Strategy 11: Bayesian Probability Density
/// - Computes the exact combinatorial probability density of all remaining unsunk ships.
/// - In Target mode, placements covering active hits receive exponential likelihood weight (1000^H).
/// - Collinear extensions naturally dominate orthogonal probes without hardcoded heuristics.
#[derive(Clone, Debug, Default)]
pub struct BayesianPdfAttack {
    tracker: TargetTracker,
    mode: &'static str,
}

impl BayesianPdfAttack {
    pub fn new() -> Self {
        Self {
            tracker: TargetTracker::new(),
            mode: "bayesian_pdf",
        }
    }
}

impl AttackStrategy for BayesianPdfAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Bayesian Probability Density"
    }

    fn reset(&mut self) {
        self.tracker.reset();
        self.mode = "bayesian_pdf";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        if self.tracker.is_targeting() {
            self.mode = "target";
        } else {
            self.mode = "bayesian_pdf";
        }

        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        let density = compute_bayesian_density(view, &self.tracker.active_hits);

        let mut max_val = 0u64;
        let mut best_cells = Vec::new();
        for &(r, c) in &unshot {
            let val = density[r][c];
            if val > max_val {
                max_val = val;
                best_cells.clear();
                best_cells.push((r, c));
            } else if val == max_val && max_val > 0 {
                best_cells.push((r, c));
            }
        }

        if !best_cells.is_empty() {
            return *rng.choose(&best_cells).unwrap();
        }

        if self.tracker.is_targeting() {
            if let Some(pos) = self.tracker.choose_target_shot(view, rng) {
                return pos;
            }
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

        *rng.choose(&unshot).unwrap()
    }

    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard) {
        match outcome {
            ShotOutcome::Miss => {}
            ShotOutcome::Hit => {
                self.tracker.record_hit(pos);
            }
            ShotOutcome::Sunk(kind) => {
                self.tracker.record_sunk(pos, kind, view);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Count consecutive Unknown cells from (r, c) in direction (dr, dc), up to `max_steps`.
pub fn count_arm(view: &TrackingBoard, r: usize, c: usize, dr: isize, dc: isize, max_steps: usize) -> usize {
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

/// Helper to evaluate crosshair combinatorial score for candidate cell (r, c).
pub fn evaluate_crosshair_score(
    view: &TrackingBoard,
    r: usize,
    c: usize,
    max_len: usize,
    min_len: usize,
) -> usize {
    let up = count_arm(view, r, c, -1, 0, max_len - 1);
    let down = count_arm(view, r, c, 1, 0, max_len - 1);
    let left = count_arm(view, r, c, 0, -1, max_len - 1);
    let right = count_arm(view, r, c, 0, 1, max_len - 1);

    // Exact number of ways `max_len` AND `min_len` ships can cover (r, c):
    let h_ways_max = if left + right + 1 >= max_len {
        left + right + 2 - max_len
    } else {
        0
    };
    let v_ways_max = if up + down + 1 >= max_len {
        up + down + 2 - max_len
    } else {
        0
    };
    let h_ways_min = if left.min(min_len - 1) + right.min(min_len - 1) + 1 >= min_len {
        left.min(min_len - 1) + right.min(min_len - 1) + 2 - min_len
    } else {
        0
    };
    let v_ways_min = if up.min(min_len - 1) + down.min(min_len - 1) + 1 >= min_len {
        up.min(min_len - 1) + down.min(min_len - 1) + 2 - min_len
    } else {
        0
    };

    (h_ways_max + v_ways_max) * 3 + (h_ways_min + v_ways_min)
}

/// Choose next target shot using Open-Side Target logic on active hits.
pub fn choose_open_side_target(
    active_hits: &[(usize, usize)],
    view: &TrackingBoard,
    rng: &mut Rng,
) -> Option<(usize, usize)> {
    let silent_hits_holder;
    let hits = if view.ruleset().is_silent_sinking() {
        silent_hits_holder = view.silent_unexplained_hits();
        &silent_hits_holder[..]
    } else {
        active_hits
    };

    if hits.is_empty() {
        return None;
    }
    let min_len = view.min_remaining_ship_len().max(2);
    let max_len = view.max_remaining_ship_len().max(2);

    let mut best_collinear: Vec<((usize, usize), usize)> = Vec::new();

    // 1-cell collinear bridge check
    for &(r, c) in hits {
        if c + 2 < WIDTH && hits.contains(&(r, c + 2)) && view.is_unshot(r, c + 1) {
            best_collinear.push(((r, c + 1), 200));
        }
        if r + 2 < HEIGHT && hits.contains(&(r + 2, c)) && view.is_unshot(r + 1, c) {
            best_collinear.push(((r + 1, c), 200));
        }
    }

    // Check for contiguous collinear chains of length >= 2
    for &(r, c) in hits {
        // Horizontal chain starting at (r, c)
        if c == 0 || !hits.contains(&(r, c - 1)) {
            let mut end_c = c;
            while end_c + 1 < WIDTH && hits.contains(&(r, end_c + 1)) {
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
        if r == 0 || !hits.contains(&(r - 1, c)) {
            let mut end_r = r;
            while end_r + 1 < HEIGHT && hits.contains(&(end_r + 1, c)) {
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
    for &(r, c) in hits {
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

/// Remove sunk ship's hits from `active_hits` list.
pub fn remove_sunk_hits(
    active_hits: &mut Vec<(usize, usize)>,
    pos: (usize, usize),
    kind: ShipKind,
) {
    if !active_hits.contains(&pos) {
        active_hits.push(pos);
    }
    let ship_len = kind.length();
    if active_hits.len() <= ship_len {
        active_hits.clear();
        return;
    }
    let (sr, sc) = pos;
    let mut h_chain = vec![pos];
    let mut c = sc;
    while c > 0 && active_hits.contains(&(sr, c - 1)) && h_chain.len() < ship_len {
        c -= 1;
        h_chain.push((sr, c));
    }
    c = sc;
    while c + 1 < WIDTH && active_hits.contains(&(sr, c + 1)) && h_chain.len() < ship_len {
        c += 1;
        h_chain.push((sr, c));
    }

    let mut v_chain = vec![pos];
    let mut r = sr;
    while r > 0 && active_hits.contains(&(r - 1, sc)) && v_chain.len() < ship_len {
        r -= 1;
        v_chain.push((r, sc));
    }
    r = sr;
    while r + 1 < HEIGHT && active_hits.contains(&(r + 1, sc)) && v_chain.len() < ship_len {
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
    active_hits.retain(|x| !to_remove.contains(x));
}

/// Silent-Sinking Endgame L/T-Junction Stub Fallback helper.
/// When view is in SilentSinking and hits_count > 0, if no ship of length 2 can fit strictly inside
/// Unknown cells alone (meaning every Unknown cell has 0 Unknown orthogonal neighbors),
/// pick randomly among Unknown cells orthogonally adjacent to `view.silent_touching_stub_hits()`.
pub fn silent_endgame_fallback(view: &TrackingBoard, rng: &mut Rng) -> Option<(usize, usize)> {
    if !view.ruleset().is_silent_sinking() || view.hits_count() == 0 {
        return None;
    }
    let unshot = view.unshot_cells();
    if unshot.is_empty() {
        return None;
    }

    let can_len2_fit = unshot.iter().any(|&(r, c)| {
        (r > 0 && view.is_unshot(r - 1, c))
            || (r + 1 < HEIGHT && view.is_unshot(r + 1, c))
            || (c > 0 && view.is_unshot(r, c - 1))
            || (c + 1 < WIDTH && view.is_unshot(r, c + 1))
    });

    if !can_len2_fit {
        let stubs = view.silent_touching_stub_hits();
        let mut adj_unshot = Vec::new();
        for (r, c) in stubs {
            for (dr, dc) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                let nr = r as isize + dr;
                let nc = c as isize + dc;
                if nr >= 0 && nr < HEIGHT as isize && nc >= 0 && nc < WIDTH as isize {
                    let (ur, uc) = (nr as usize, nc as usize);
                    if view.is_unshot(ur, uc) && !adj_unshot.contains(&(ur, uc)) {
                        adj_unshot.push((ur, uc));
                    }
                }
            }
        }
        if !adj_unshot.is_empty() {
            return rng.choose(&adj_unshot).copied();
        }
    }

    None
}

/// Strategy 12: Crosshair Stride + Open-Target Attack.
///
/// Promoted from human Bayesian approximation:
/// 1. Hunt Mode: Staggered stride lattice (stride 4 while ship of len >= 4 remains, else stride 2).
///    Scores each unshot active-stride cell by its "+" crosshair open arms (N, S, E, W), calculating
///    exact combinatorial placements for max_len and min_len ships.
/// 2. Target Mode: Fires toward open endpoints/neighbors with the longest runway.
#[derive(Clone, Debug)]
pub struct CrosshairStrideAttack {
    offset4: usize,
    initialized: bool,
    active_hits: Vec<(usize, usize)>,
    mode: &'static str,
}

impl Default for CrosshairStrideAttack {
    fn default() -> Self {
        Self::new()
    }
}

impl CrosshairStrideAttack {
    pub fn new() -> Self {
        Self {
            offset4: 0,
            initialized: false,
            active_hits: Vec::new(),
            mode: "crosshair_stride",
        }
    }
}

impl AttackStrategy for CrosshairStrideAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Crosshair Stride + Open-Target"
    }

    fn reset(&mut self) {
        self.initialized = false;
        self.active_hits.clear();
        self.mode = "crosshair_stride";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        if !self.initialized {
            self.offset4 = rng.next_usize(4);
            self.initialized = true;
        }

        if let Some(target) = choose_open_side_target(&self.active_hits, view, rng) {
            self.mode = "target";
            return target;
        }

        self.mode = "crosshair_stride";
        let max_len = view.max_remaining_ship_len().max(2);
        let min_len = view.min_remaining_ship_len().max(2);
        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        let strides_to_try: &[usize] = if max_len >= 4 { &[4, 2] } else { &[2] };
        let mut best_cells = Vec::new();
        let mut best_score;

        for &stride in strides_to_try {
            let target_rem = self.offset4 % stride;
            best_cells.clear();
            best_score = 0;

            for &(r, c) in &unshot {
                if (r + c) % stride != target_rem {
                    continue;
                }
                let score = evaluate_crosshair_score(view, r, c, max_len, min_len);
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
        }

        // Fallback to any unshot cell where min_len fits
        let mut fallback_fits = Vec::new();
        for &(r, c) in &unshot {
            let up = count_arm(view, r, c, -1, 0, min_len - 1);
            let down = count_arm(view, r, c, 1, 0, min_len - 1);
            let left = count_arm(view, r, c, 0, -1, min_len - 1);
            let right = count_arm(view, r, c, 0, 1, min_len - 1);
            if up + 1 + down >= min_len || left + 1 + right >= min_len {
                fallback_fits.push((r, c));
            }
        }
        if !fallback_fits.is_empty() {
            return fallback_fits[rng.next_usize(fallback_fits.len())];
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

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
                remove_sunk_hits(&mut self.active_hits, pos, kind);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

/// Strategy 13: Crosshair Stride (5->4->3->2) Attack.
///
/// Promoted from human Bayesian approximation with descending stride:
/// 1. Hunt Mode: Descending stride lattice matching the max remaining ship length
///    (5 while Carrier alive, 4 for Battleship, 3 for Cruiser/Submarine, 2 for Destroyer).
///    Scores each unshot active-stride cell by its "+" crosshair open arms (N, S, E, W), calculating
///    exact combinatorial placements for max_len and min_len ships.
/// 2. Target Mode: Open-Side Target Mode firing toward endpoints/neighbors with the longest open runway.
#[derive(Clone, Debug)]
pub struct CrosshairDescendingAttack {
    offset: usize,
    initialized: bool,
    active_hits: Vec<(usize, usize)>,
    mode: &'static str,
}

impl Default for CrosshairDescendingAttack {
    fn default() -> Self {
        Self::new()
    }
}

impl CrosshairDescendingAttack {
    pub fn new() -> Self {
        Self {
            offset: 0,
            initialized: false,
            active_hits: Vec::new(),
            mode: "crosshair_5432",
        }
    }
}

impl AttackStrategy for CrosshairDescendingAttack {
    fn clone_box(&self) -> Box<dyn AttackStrategy> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        "Crosshair Stride (5->4->3->2)"
    }

    fn reset(&mut self) {
        self.initialized = false;
        self.active_hits.clear();
        self.mode = "crosshair_5432";
    }

    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize) {
        if !self.initialized {
            self.offset = rng.next_usize(60);
            self.initialized = true;
        }

        if let Some(target) = choose_open_side_target(&self.active_hits, view, rng) {
            self.mode = "target";
            return target;
        }

        self.mode = "crosshair_5432";
        let max_len = view.max_remaining_ship_len().max(2);
        let min_len = view.min_remaining_ship_len().max(2);
        let unshot = view.unshot_cells();
        assert!(!unshot.is_empty(), "No unshot cells remaining on board");

        let mut best_cells = Vec::new();
        let mut best_score;

        for stride in (2..=max_len).rev() {
            let target_rem = self.offset % stride;
            best_cells.clear();
            best_score = 0usize;

            for &(r, c) in &unshot {
                if (r + c) % stride != target_rem {
                    continue;
                }
                let score = evaluate_crosshair_score(view, r, c, max_len, min_len);
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
        }

        // Fallback to any unshot cell where min_len fits
        let mut fallback_fits = Vec::new();
        for &(r, c) in &unshot {
            let up = count_arm(view, r, c, -1, 0, min_len - 1);
            let down = count_arm(view, r, c, 1, 0, min_len - 1);
            let left = count_arm(view, r, c, 0, -1, min_len - 1);
            let right = count_arm(view, r, c, 0, 1, min_len - 1);
            if up + 1 + down >= min_len || left + 1 + right >= min_len {
                fallback_fits.push((r, c));
            }
        }
        if !fallback_fits.is_empty() {
            return fallback_fits[rng.next_usize(fallback_fits.len())];
        }

        if let Some(pos) = silent_endgame_fallback(view, rng) {
            return pos;
        }

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
                remove_sunk_hits(&mut self.active_hits, pos, kind);
            }
        }
    }

    fn current_mode(&self) -> &'static str {
        self.mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Ruleset;

    #[test]
    fn test_random_defense_validity() {
        let mut rng = Rng::with_seed(42);
        let mut defense = RandomDefense::new();

        for _ in 0..50 {
            let board = defense.place_ships(&mut rng);
            assert!(board.is_valid_fleet());
            assert_eq!(board.placements().len(), 5);
        }
    }

    #[test]
    fn test_hunt_and_target_single_game() {
        let mut rng = Rng::with_seed(101);
        let mut defense = RandomDefense::new();
        let mut attack = HuntAndTargetAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_checkerboard_single_game() {
        let mut rng = Rng::with_seed(202);
        let mut defense = RandomDefense::new();
        let mut attack = CheckerboardAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_collinear_targeting_priority() {
        let mut tracker = TargetTracker::new();
        let mut view = TrackingBoard::new();
        let mut rng = Rng::with_seed(999);

        // Record two contiguous horizontal hits at (3, 4) and (3, 5)
        tracker.record_hit((3, 4));
        tracker.record_hit((3, 5));
        view.record_shot(3, 4, ShotOutcome::Hit);
        view.record_shot(3, 5, ShotOutcome::Hit);

        let next = tracker.choose_target_shot(&view, &mut rng).unwrap();
        // Collinear priority dictates either (3, 3) or (3, 6)
        assert!(next == (3, 3) || next == (3, 6), "Expected collinear endpoint, got {:?}", next);
    }

    #[test]
    fn test_find_largest_empty_rectangles_initial() {
        let view = TrackingBoard::new();
        let (area, min_dim, rects) = find_largest_empty_rectangles(&view);
        assert_eq!(area, 100);
        assert_eq!(min_dim, 10);
        assert_eq!(rects, vec![(0, 9, 0, 9)]);
    }

    #[test]
    fn test_largest_rect_midpoint_single_game() {
        let mut rng = Rng::with_seed(303);
        let mut defense = RandomDefense::new();
        let mut attack = LargestRectMidpointAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_adaptive_stride_single_game() {
        let mut rng = Rng::with_seed(404);
        let mut defense = RandomDefense::new();
        let mut attack = AdaptiveStrideUpAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_staggered_stride4_to_2_single_game() {
        let mut rng = Rng::with_seed(505);
        let mut defense = RandomDefense::new();
        let mut attack = StaggeredFourToTwoAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_rect_stride4_single_game() {
        let mut rng = Rng::with_seed(606);
        let mut defense = RandomDefense::new();
        let mut attack = RectStrideFourAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_descending_stride_single_game() {
        let mut rng = Rng::with_seed(707);
        let mut defense = RandomDefense::new();
        let mut attack = DescendingStrideAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_rect_descending_stride_single_game() {
        let mut rng = Rng::with_seed(808);
        let mut defense = RandomDefense::new();
        let mut attack = RectDescendingStrideAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_rect_checkerboard_single_game() {
        let mut rng = Rng::with_seed(909);
        let mut defense = RandomDefense::new();
        let mut attack = RectCheckerboardAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_monte_carlo_single_game() {
        let mut rng = Rng::with_seed(1010);
        let mut defense = RandomDefense::new();
        let mut attack = MonteCarloAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_bayesian_pdf_single_game() {
        let mut rng = Rng::with_seed(1111);
        let mut defense = RandomDefense::new();
        let mut attack = BayesianPdfAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_bayesian_density_prioritizes_collinear_hits() {
        let mut view = TrackingBoard::new();
        let active_hits = vec![(4, 4), (4, 5)];
        view.record_shot(4, 4, ShotOutcome::Hit);
        view.record_shot(4, 5, ShotOutcome::Hit);

        let density = compute_bayesian_density(&view, &active_hits);

        assert!(density[4][3] >= 1_000_000, "Expected collinear (4,3) to have >= 1M density, got {}", density[4][3]);
        assert!(density[4][6] >= 1_000_000, "Expected collinear (4,6) to have >= 1M density, got {}", density[4][6]);
        assert!(density[4][3] > density[3][4] * 100, "Collinear must dominate orthogonal probe");
    }

    #[test]
    fn test_all_seven_defenses_valid_fleets() {
        let mut rng = Rng::with_seed(12345);
        let mut defenses: Vec<Box<dyn DefenseStrategy>> = vec![
            Box::new(RandomDefense::new()),
            Box::new(SpacedDefense::new()),
            Box::new(CornerEdgeDefense::new()),
            Box::new(ClusteredDefense::new()),
            Box::new(AntiMidpointDefense::new()),
            Box::new(BaitAndHideDefense::new()),
            Box::new(ParallelLanesDefense::new()),
        ];

        for def in defenses.iter_mut() {
            assert!(!def.id().is_empty());
            assert!(!def.name().is_empty());
            for _ in 0..10 {
                let board = def.place_ships(&mut rng);
                assert!(board.is_valid_fleet(), "Defense {} produced invalid fleet", def.id());
                assert_eq!(board.placements().len(), 5);
            }
        }
    }

    #[test]
    fn test_spaced_defense_spacing() {
        let mut rng = Rng::with_seed(54321);
        let mut def = SpacedDefense::new();
        for _ in 0..10 {
            let board = def.place_ships(&mut rng);
            assert!(board.is_valid_fleet());
        }
    }

    #[test]
    fn test_parallel_lanes_defense_same_orientation() {
        let mut rng = Rng::with_seed(777);
        let mut def = ParallelLanesDefense::new();
        for _ in 0..10 {
            let board = def.place_ships(&mut rng);
            assert!(board.is_valid_fleet());
            let first_orient = board.placements()[0].orientation;
            for p in board.placements() {
                assert_eq!(p.orientation, first_orient, "All ships in ParallelLanes must share orientation");
            }
        }
    }

    #[test]
    fn test_crosshair_stride_single_game() {
        let mut rng = Rng::with_seed(404);
        let mut defense = RandomDefense::new();
        let mut attack = CrosshairStrideAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_crosshair_descending_single_game() {
        let mut rng = Rng::with_seed(505);
        let mut defense = RandomDefense::new();
        let mut attack = CrosshairDescendingAttack::new();

        let mut board = defense.place_ships(&mut rng);
        let mut view = TrackingBoard::new();
        attack.reset();

        let mut turns = 0;
        while !board.all_sunk() && turns < 100 {
            turns += 1;
            let pos = attack.choose_shot(&view, &mut rng);
            assert!(view.is_unshot(pos.0, pos.1));
            let outcome = board.receive_shot(pos.0, pos.1);
            view.record_shot(pos.0, pos.1, outcome);
            attack.on_shot_result(pos, outcome, &view);
        }

        assert!(board.all_sunk());
        assert!(turns <= 100);
    }

    #[test]
    fn test_all_13_strategies_complete_silent_sinking() {
        let mut rng = Rng::with_seed(999);
        let mut strategies: Vec<Box<dyn AttackStrategy>> = vec![
            Box::new(HuntAndTargetAttack::new()),
            Box::new(CheckerboardAttack::new()),
            Box::new(LargestRectMidpointAttack::new()),
            Box::new(AdaptiveStrideUpAttack::new()),
            Box::new(StaggeredFourToTwoAttack::new()),
            Box::new(RectStrideFourAttack::new()),
            Box::new(DescendingStrideAttack::new()),
            Box::new(RectDescendingStrideAttack::new()),
            Box::new(RectCheckerboardAttack::new()),
            Box::new(MonteCarloAttack::new()),
            Box::new(BayesianPdfAttack::new()),
            Box::new(CrosshairStrideAttack::new()),
            Box::new(CrosshairDescendingAttack::new()),
        ];

        let mut defenses: Vec<Box<dyn DefenseStrategy>> = vec![
            Box::new(RandomDefense::new()),
            Box::new(ClusteredDefense::new()),
        ];

        for strat in &mut strategies {
            for def in &mut defenses {
                let mut board = def.place_ships(&mut rng);
                let mut view = TrackingBoard::with_ruleset(Ruleset::SilentSinking);
                strat.reset();

                let mut shots = 0;
                while !board.all_sunk() && shots < 100 {
                    shots += 1;
                    let pos = strat.choose_shot(&view, &mut rng);
                    assert!(view.is_unshot(pos.0, pos.1), "Strategy {} re-shot cell {:?}", strat.name(), pos);
                    let outcome = board.receive_shot_with_ruleset(pos.0, pos.1, Ruleset::SilentSinking);
                    assert!(!matches!(outcome, ShotOutcome::Sunk(_)), "Silent sinking must never return Sunk");
                    view.record_shot(pos.0, pos.1, outcome);
                    strat.on_shot_result(pos, outcome, &view);
                }

                assert!(
                    board.all_sunk(),
                    "Strategy {} failed to sink fleet under SilentSinking against {}",
                    strat.name(),
                    def.name()
                );
                assert!(shots <= 100);
            }
        }
    }

    #[test]
    fn test_salvo_choice_distinct_cells() {
        let mut rng = Rng::with_seed(777);
        let view = TrackingBoard::with_ruleset(Ruleset::Salvo);
        let mut strat = CrosshairStrideAttack::new();

        let salvo = strat.choose_salvo(&view, 5, &mut rng);
        assert_eq!(salvo.len(), 5);
        let mut dedup = salvo.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), 5, "Salvo shots must be distinct cells");
    }

    #[test]
    fn test_all_13_strategies_complete_salvo_silent() {
        let mut rng = Rng::with_seed(1234);
        let mut strategies: Vec<Box<dyn AttackStrategy>> = vec![
            Box::new(HuntAndTargetAttack::new()),
            Box::new(CheckerboardAttack::new()),
            Box::new(LargestRectMidpointAttack::new()),
            Box::new(AdaptiveStrideUpAttack::new()),
            Box::new(StaggeredFourToTwoAttack::new()),
            Box::new(RectStrideFourAttack::new()),
            Box::new(DescendingStrideAttack::new()),
            Box::new(RectDescendingStrideAttack::new()),
            Box::new(RectCheckerboardAttack::new()),
            Box::new(MonteCarloAttack::new()),
            Box::new(BayesianPdfAttack::new()),
            Box::new(CrosshairStrideAttack::new()),
            Box::new(CrosshairDescendingAttack::new()),
        ];

        let mut defense = RandomDefense::new();

        for strat in &mut strategies {
            let mut board = defense.place_ships(&mut rng);
            let mut view = TrackingBoard::with_ruleset(Ruleset::SalvoSilent);
            strat.reset();

            let mut shots = 0;
            while !board.all_sunk() && shots < 100 {
                let unshot_count = view.unshot_cells().len();
                let k = 5.min(unshot_count);
                if k == 0 {
                    break;
                }
                let salvo = strat.choose_salvo(&view, k, &mut rng);
                assert!(!salvo.is_empty(), "Strategy {} returned empty salvo", strat.name());
                let mut volley_results = Vec::new();
                for pos in salvo {
                    shots += 1;
                    assert!(view.is_unshot(pos.0, pos.1), "Strategy {} re-shot cell {:?}", strat.name(), pos);
                    let outcome = board.receive_shot_with_ruleset(pos.0, pos.1, Ruleset::SalvoSilent);
                    assert!(!matches!(outcome, ShotOutcome::Sunk(_)), "SalvoSilent must never return Sunk outcome");
                    volley_results.push((pos, outcome));
                    if board.all_sunk() || shots >= 100 {
                        break;
                    }
                }
                for (pos, outcome) in volley_results {
                    view.record_shot(pos.0, pos.1, outcome);
                    strat.on_shot_result(pos, outcome, &view);
                }
            }

            assert!(
                board.all_sunk(),
                "Strategy {} failed to sink fleet under SalvoSilent",
                strat.name()
            );
            assert!(shots <= 100);
        }
    }
}
