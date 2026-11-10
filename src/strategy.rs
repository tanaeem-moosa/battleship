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
    /// Strategy name.
    fn name(&self) -> &'static str;
    /// Generate a valid 10x10 board with all 5 ships legally placed.
    fn place_ships(&mut self, rng: &mut Rng) -> Board;
}

/// Attack strategy trait.
pub trait AttackStrategy {
    /// Strategy name.
    fn name(&self) -> &'static str;
    /// Reset internal state before a new game begins.
    fn reset(&mut self);
    /// Choose the coordinate (row, col) for the next shot.
    fn choose_shot(&mut self, view: &TrackingBoard, rng: &mut Rng) -> (usize, usize);
    /// Notify strategy of the result of the shot just fired at pos.
    fn on_shot_result(&mut self, pos: (usize, usize), outcome: ShotOutcome, view: &TrackingBoard);
    /// Current strategy mode string ("hunt", "checkerboard", "target").
    fn current_mode(&self) -> &'static str;
}

/// Standard random defense: places all 5 ships with uniform orientation and coordinates.
#[derive(Clone, Debug, Default)]
pub struct RandomDefense;

impl RandomDefense {
    pub fn new() -> Self {
        Self
    }
}

impl DefenseStrategy for RandomDefense {
    fn name(&self) -> &'static str {
        "Random Placement"
    }

    fn place_ships(&mut self, rng: &mut Rng) -> Board {
        loop {
            let mut board = Board::new();
            let mut success = true;

            for &kind in &ShipKind::ALL {
                let len = kind.length();
                let mut placed = false;

                // Try up to 300 random positions per ship
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
    /// 1. Prioritize collinear endpoints if 2+ collinear active hits exist.
    /// 2. Fall back to orthogonal neighbors of active hits.
    /// 3. Returns None if all neighbors of active hits are already shot.
    pub fn choose_target_shot(
        &self,
        view: &TrackingBoard,
        rng: &mut Rng,
    ) -> Option<(usize, usize)> {
        if self.active_hits.is_empty() {
            return None;
        }

        // 1. Check for collinear chains (length >= 2 in active hits)
        let mut collinear_candidates = Vec::new();

        // Horizontal collinear chains
        for r in 0..HEIGHT {
            let mut cols: Vec<usize> = self
                .active_hits
                .iter()
                .filter(|(hr, _)| *hr == r)
                .map(|(_, hc)| *hc)
                .collect();
            if cols.len() >= 2 {
                cols.sort_unstable();
                // Check contiguous chains or connected hits
                let min_c = cols[0];
                let max_c = cols[cols.len() - 1];

                // Immediate left endpoint
                if min_c > 0 && view.is_unshot(r, min_c - 1) {
                    collinear_candidates.push((r, min_c - 1));
                }
                // Immediate right endpoint
                if max_c + 1 < WIDTH && view.is_unshot(r, max_c + 1) {
                    collinear_candidates.push((r, max_c + 1));
                }
            }
        }

        // Vertical collinear chains
        for c in 0..WIDTH {
            let mut rows: Vec<usize> = self
                .active_hits
                .iter()
                .filter(|(_, hc)| *hc == c)
                .map(|(hr, _)| *hr)
                .collect();
            if rows.len() >= 2 {
                rows.sort_unstable();
                let min_r = rows[0];
                let max_r = rows[rows.len() - 1];

                // Immediate top endpoint
                if min_r > 0 && view.is_unshot(min_r - 1, c) {
                    collinear_candidates.push((min_r - 1, c));
                }
                // Immediate bottom endpoint
                if max_r + 1 < HEIGHT && view.is_unshot(max_r + 1, c) {
                    collinear_candidates.push((max_r + 1, c));
                }
            }
        }

        if !collinear_candidates.is_empty() {
            // Deduplicate
            collinear_candidates.sort_unstable();
            collinear_candidates.dedup();
            return rng.choose(&collinear_candidates).copied();
        }

        // 2. Fall back to orthogonal neighbors (N, S, E, W) of any active hit
        let mut neighbor_candidates = Vec::new();
        for &(r, c) in &self.active_hits {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
