//! Core Battleship rules, board representation, and game state.
//!
//! Conforms to standard Hasbro Battleship rules:
//! - 10x10 ocean grid (WIDTH = 10, HEIGHT = 10)
//! - 5 ships: Carrier (5), Battleship (4), Cruiser (3), Submarine (3), Destroyer (2)
//! - Total 17 ship cells
//! - Orthogonal (horizontal or vertical) placements without overlap
//! - Explicit shot outcomes: Miss, Hit, Sunk(ShipKind)
//! - Turn tracking for replays and analysis

pub const WIDTH: usize = 10;
pub const HEIGHT: usize = 10;
pub const TOTAL_SHIP_CELLS: usize = 17;

/// The 5 standard Hasbro Battleship ship types.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ShipKind {
    Carrier,
    Battleship,
    Cruiser,
    Submarine,
    Destroyer,
}

impl ShipKind {
    /// Array containing all five ships.
    pub const ALL: [ShipKind; 5] = [
        ShipKind::Carrier,
        ShipKind::Battleship,
        ShipKind::Cruiser,
        ShipKind::Submarine,
        ShipKind::Destroyer,
    ];

    /// Length in grid cells for this ship.
    pub fn length(self) -> usize {
        match self {
            ShipKind::Carrier => 5,
            ShipKind::Battleship => 4,
            ShipKind::Cruiser => 3,
            ShipKind::Submarine => 3,
            ShipKind::Destroyer => 2,
        }
    }

    /// Canonical display name.
    pub fn name(self) -> &'static str {
        match self {
            ShipKind::Carrier => "Carrier",
            ShipKind::Battleship => "Battleship",
            ShipKind::Cruiser => "Cruiser",
            ShipKind::Submarine => "Submarine",
            ShipKind::Destroyer => "Destroyer",
        }
    }

    /// Single character identifier (useful in ASCII displays).
    pub fn symbol(self) -> char {
        match self {
            ShipKind::Carrier => 'C',
            ShipKind::Battleship => 'B',
            ShipKind::Cruiser => 'R',
            ShipKind::Submarine => 'S',
            ShipKind::Destroyer => 'D',
        }
    }

    /// Parse from case-insensitive string.
    pub fn from_name(name: &str) -> Option<ShipKind> {
        match name.to_ascii_lowercase().as_str() {
            "carrier" => Some(ShipKind::Carrier),
            "battleship" => Some(ShipKind::Battleship),
            "cruiser" => Some(ShipKind::Cruiser),
            "submarine" => Some(ShipKind::Submarine),
            "destroyer" => Some(ShipKind::Destroyer),
            _ => None,
        }
    }

    /// Zero-based index into arrays (0..5).
    pub fn index(self) -> usize {
        match self {
            ShipKind::Carrier => 0,
            ShipKind::Battleship => 1,
            ShipKind::Cruiser => 2,
            ShipKind::Submarine => 3,
            ShipKind::Destroyer => 4,
        }
    }
}

/// Ship placement orientation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl Orientation {
    pub fn name(self) -> &'static str {
        match self {
            Orientation::Horizontal => "Horizontal",
            Orientation::Vertical => "Vertical",
        }
    }
}

/// Placement of a single ship on the board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShipPlacement {
    pub kind: ShipKind,
    pub row: usize,
    pub col: usize,
    pub orientation: Orientation,
}

impl ShipPlacement {
    pub fn new(kind: ShipKind, row: usize, col: usize, orientation: Orientation) -> Self {
        Self {
            kind,
            row,
            col,
            orientation,
        }
    }

    /// Returns the list of (row, col) grid cells occupied by this ship.
    pub fn cells(&self) -> Vec<(usize, usize)> {
        let len = self.kind.length();
        let mut result = Vec::with_capacity(len);
        for i in 0..len {
            match self.orientation {
                Orientation::Horizontal => result.push((self.row, self.col + i)),
                Orientation::Vertical => result.push((self.row + i, self.col)),
            }
        }
        result
    }

    /// Whether all cells lie strictly within the 10x10 ocean bounds.
    pub fn is_within_bounds(&self) -> bool {
        let len = self.kind.length();
        match self.orientation {
            Orientation::Horizontal => self.row < HEIGHT && self.col + len <= WIDTH,
            Orientation::Vertical => self.row + len <= HEIGHT && self.col < WIDTH,
        }
    }
}

/// Result of firing a shot at a target grid cell.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ShotOutcome {
    Miss,
    Hit,
    Sunk(ShipKind),
}

impl ShotOutcome {
    pub fn is_hit_or_sunk(self) -> bool {
        matches!(self, ShotOutcome::Hit | ShotOutcome::Sunk(_))
    }

    pub fn is_sunk(self) -> bool {
        matches!(self, ShotOutcome::Sunk(_))
    }

    pub fn sunk_ship(self) -> Option<ShipKind> {
        match self {
            ShotOutcome::Sunk(kind) => Some(kind),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ShotOutcome::Miss => "Miss",
            ShotOutcome::Hit => "Hit",
            ShotOutcome::Sunk(_) => "Sunk",
        }
    }
}

/// Player identifier in head-to-head matches.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Player {
    Player1,
    Player2,
}

impl Player {
    pub fn as_str(self) -> &'static str {
        match self {
            Player::Player1 => "player1",
            Player::Player2 => "player2",
        }
    }

    pub fn other(self) -> Self {
        match self {
            Player::Player1 => Player::Player2,
            Player::Player2 => Player::Player1,
        }
    }
}

/// Detailed record of a single turn fired in a game or match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveRecord {
    pub turn: usize,
    pub round: usize,
    pub player: Player,
    pub row: usize,
    pub col: usize,
    pub coord: String,
    pub outcome: ShotOutcome,
    pub sunk_cells: Option<Vec<(usize, usize)>>,
    pub mode: String,
    pub p1_shots: usize,
    pub p1_hits: usize,
    pub p1_misses: usize,
    pub p1_afloat: usize,
    pub p2_shots: usize,
    pub p2_hits: usize,
    pub p2_misses: usize,
    pub p2_afloat: usize,
}

/// Complete internal board holding ship locations and incoming damage.
#[derive(Clone, Debug)]
pub struct Board {
    grid: [[Option<ShipKind>; WIDTH]; HEIGHT],
    hits: [[bool; WIDTH]; HEIGHT],
    placements: Vec<ShipPlacement>,
    ship_hit_counts: [usize; 5],
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    /// Create a blank 10x10 ocean board.
    pub fn new() -> Self {
        Self {
            grid: [[None; WIDTH]; HEIGHT],
            hits: [[false; WIDTH]; HEIGHT],
            placements: Vec::with_capacity(5),
            ship_hit_counts: [0; 5],
        }
    }

    /// Check if a ship placement is legally in-bounds and does not overlap existing ships.
    pub fn can_place(&self, placement: &ShipPlacement) -> bool {
        if !placement.is_within_bounds() {
            return false;
        }
        // Ensure this ship kind has not already been placed
        if self.placements.iter().any(|p| p.kind == placement.kind) {
            return false;
        }
        for (r, c) in placement.cells() {
            if self.grid[r][c].is_some() {
                return false;
            }
        }
        true
    }

    /// Place a ship onto the board. Returns Err if invalid.
    pub fn place_ship(&mut self, placement: ShipPlacement) -> Result<(), &'static str> {
        if !self.can_place(&placement) {
            return Err("Cannot place ship: out of bounds or overlapping");
        }
        for (r, c) in placement.cells() {
            self.grid[r][c] = Some(placement.kind);
        }
        self.placements.push(placement);
        Ok(())
    }

    /// Checks that all 5 required ships are placed legally (total 17 cells).
    pub fn is_valid_fleet(&self) -> bool {
        if self.placements.len() != 5 {
            return false;
        }
        let mut seen = [false; 5];
        for p in &self.placements {
            let idx = p.kind.index();
            if seen[idx] {
                return false;
            }
            seen[idx] = true;
        }
        let total_cells: usize = self
            .grid
            .iter()
            .flat_map(|row| row.iter())
            .filter(|c| c.is_some())
            .count();
        total_cells == TOTAL_SHIP_CELLS
    }

    /// Return slice of placed ships.
    pub fn placements(&self) -> &[ShipPlacement] {
        &self.placements
    }

    /// Look up which ship (if any) occupies cell (row, col).
    pub fn ship_at(&self, row: usize, col: usize) -> Option<ShipKind> {
        if row < HEIGHT && col < WIDTH {
            self.grid[row][col]
        } else {
            None
        }
    }

    /// Check if cell (row, col) is occupied by any ship.
    pub fn is_occupied(&self, row: usize, col: usize) -> bool {
        self.ship_at(row, col).is_some()
    }

    /// Check if a cell has already received a shot.
    pub fn is_shot(&self, row: usize, col: usize) -> bool {
        if row < HEIGHT && col < WIDTH {
            self.hits[row][col]
        } else {
            false
        }
    }

    /// Receive a shot fired at (row, col).
    pub fn receive_shot(&mut self, row: usize, col: usize) -> ShotOutcome {
        assert!(
            row < HEIGHT && col < WIDTH,
            "Shot ({}, {}) out of grid bounds",
            row,
            col
        );

        // Mark cell as shot
        self.hits[row][col] = true;

        if let Some(kind) = self.grid[row][col] {
            let idx = kind.index();
            self.ship_hit_counts[idx] += 1;
            if self.ship_hit_counts[idx] == kind.length() {
                ShotOutcome::Sunk(kind)
            } else {
                ShotOutcome::Hit
            }
        } else {
            ShotOutcome::Miss
        }
    }

    /// Number of hits scored on a specific ship.
    pub fn hits_on_ship(&self, kind: ShipKind) -> usize {
        self.ship_hit_counts[kind.index()]
    }

    /// Check if a specific ship has been sunk.
    pub fn is_sunk(&self, kind: ShipKind) -> bool {
        self.ship_hit_counts[kind.index()] == kind.length()
    }

    /// Check if all 17 ship cells have been sunk (game over condition).
    pub fn all_sunk(&self) -> bool {
        let total_hits: usize = self.ship_hit_counts.iter().sum();
        total_hits == TOTAL_SHIP_CELLS
    }

    /// Return list of ships that have not yet been sunk.
    pub fn remaining_ships(&self) -> Vec<ShipKind> {
        ShipKind::ALL
            .iter()
            .copied()
            .filter(|&k| !self.is_sunk(k))
            .collect()
    }
}

/// Attacker's tracking view of the opponent's grid.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrackingCell {
    Unknown,
    Miss,
    Hit,
    Sunk(ShipKind),
}

/// The tracking board maintained by an attacking strategy to record shots and outcomes.
#[derive(Clone, Debug)]
pub struct TrackingBoard {
    cells: [[TrackingCell; WIDTH]; HEIGHT],
    shots_fired: usize,
    hits_count: usize,
    sunk_ships: Vec<ShipKind>,
}

impl Default for TrackingBoard {
    fn default() -> Self {
        Self::new()
    }
}

impl TrackingBoard {
    pub fn new() -> Self {
        Self {
            cells: [[TrackingCell::Unknown; WIDTH]; HEIGHT],
            shots_fired: 0,
            hits_count: 0,
            sunk_ships: Vec::with_capacity(5),
        }
    }

    /// Return the cell state at (row, col).
    pub fn cell(&self, row: usize, col: usize) -> TrackingCell {
        if row < HEIGHT && col < WIDTH {
            self.cells[row][col]
        } else {
            TrackingCell::Miss
        }
    }

    /// True if the coordinate has not yet been shot at.
    pub fn is_unshot(&self, row: usize, col: usize) -> bool {
        row < HEIGHT && col < WIDTH && self.cells[row][col] == TrackingCell::Unknown
    }

    /// Return list of all currently unshot coordinates.
    pub fn unshot_cells(&self) -> Vec<(usize, usize)> {
        let mut list = Vec::with_capacity(WIDTH * HEIGHT - self.shots_fired);
        for r in 0..HEIGHT {
            for c in 0..WIDTH {
                if self.cells[r][c] == TrackingCell::Unknown {
                    list.push((r, c));
                }
            }
        }
        list
    }

    /// Record a shot and its outcome on this tracking board.
    pub fn record_shot(&mut self, row: usize, col: usize, outcome: ShotOutcome) {
        if row >= HEIGHT || col >= WIDTH {
            return;
        }
        self.shots_fired += 1;
        match outcome {
            ShotOutcome::Miss => {
                self.cells[row][col] = TrackingCell::Miss;
            }
            ShotOutcome::Hit => {
                self.cells[row][col] = TrackingCell::Hit;
                self.hits_count += 1;
            }
            ShotOutcome::Sunk(kind) => {
                self.cells[row][col] = TrackingCell::Sunk(kind);
                self.hits_count += 1;
                if !self.sunk_ships.contains(&kind) {
                    self.sunk_ships.push(kind);
                }
            }
        }
    }

    /// Return total shots fired so far.
    pub fn shots_fired(&self) -> usize {
        self.shots_fired
    }

    /// Return total hits scored so far.
    pub fn hits_count(&self) -> usize {
        self.hits_count
    }

    /// Return list of ships sunk so far.
    pub fn sunk_ships(&self) -> &[ShipKind] {
        &self.sunk_ships
    }

    /// Check if a specific ship has already been sunk according to this tracker.
    pub fn is_ship_sunk(&self, kind: ShipKind) -> bool {
        self.sunk_ships.contains(&kind)
    }

    /// Return list of unsunk ships remaining in the fleet.
    pub fn remaining_unsunk_ships(&self) -> Vec<ShipKind> {
        ShipKind::ALL
            .iter()
            .copied()
            .filter(|k| !self.sunk_ships.contains(k))
            .collect()
    }

    /// Minimum length among all remaining unsunk ships.
    pub fn min_remaining_ship_length(&self) -> usize {
        self.remaining_unsunk_ships()
            .into_iter()
            .map(|k| k.length())
            .min()
            .unwrap_or(2)
    }

    /// Alias for min_remaining_ship_length.
    pub fn min_remaining_ship_len(&self) -> usize {
        self.min_remaining_ship_length()
    }

    /// Maximum length among all remaining unsunk ships.
    pub fn max_remaining_ship_length(&self) -> usize {
        self.remaining_unsunk_ships()
            .into_iter()
            .map(|k| k.length())
            .max()
            .unwrap_or(5)
    }

    /// Alias for max_remaining_ship_length.
    pub fn max_remaining_ship_len(&self) -> usize {
        self.max_remaining_ship_length()
    }

    /// Check if a ship of length `len` could legally fit covering (row, col)
    /// without crossing known Misses or already-identified Sunk ships.
    pub fn can_ship_fit_at(&self, row: usize, col: usize, len: usize) -> bool {
        if row >= HEIGHT || col >= WIDTH || self.cells[row][col] == TrackingCell::Miss {
            return false;
        }

        // Check horizontal spans containing (row, col)
        let min_c = col.saturating_sub(len - 1);
        let max_c = (col + 1).min(WIDTH.saturating_sub(len - 1));
        for start_c in min_c..max_c {
            let mut fits = true;
            for c in start_c..(start_c + len) {
                if self.cells[row][c] == TrackingCell::Miss {
                    fits = false;
                    break;
                }
            }
            if fits {
                return true;
            }
        }

        // Check vertical spans containing (row, col)
        let min_r = row.saturating_sub(len - 1);
        let max_r = (row + 1).min(HEIGHT.saturating_sub(len - 1));
        for start_r in min_r..max_r {
            let mut fits = true;
            for r in start_r..(start_r + len) {
                if self.cells[r][col] == TrackingCell::Miss {
                    fits = false;
                    break;
                }
            }
            if fits {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ship_lengths_and_total_cells() {
        assert_eq!(ShipKind::Carrier.length(), 5);
        assert_eq!(ShipKind::Battleship.length(), 4);
        assert_eq!(ShipKind::Cruiser.length(), 3);
        assert_eq!(ShipKind::Submarine.length(), 3);
        assert_eq!(ShipKind::Destroyer.length(), 2);

        let total: usize = ShipKind::ALL.iter().map(|s| s.length()).sum();
        assert_eq!(total, TOTAL_SHIP_CELLS);
        assert_eq!(total, 17);
    }

    #[test]
    fn test_valid_board_placement() {
        let mut board = Board::new();

        assert!(board.place_ship(ShipPlacement::new(ShipKind::Carrier, 0, 0, Orientation::Horizontal)).is_ok());
        assert!(board.place_ship(ShipPlacement::new(ShipKind::Battleship, 1, 0, Orientation::Horizontal)).is_ok());
        assert!(board.place_ship(ShipPlacement::new(ShipKind::Cruiser, 2, 0, Orientation::Horizontal)).is_ok());
        assert!(board.place_ship(ShipPlacement::new(ShipKind::Submarine, 3, 0, Orientation::Horizontal)).is_ok());
        assert!(board.place_ship(ShipPlacement::new(ShipKind::Destroyer, 4, 0, Orientation::Horizontal)).is_ok());

        assert!(board.is_valid_fleet());
        assert_eq!(board.placements().len(), 5);
        assert_eq!(board.remaining_ships().len(), 5);
        assert!(!board.all_sunk());
    }

    #[test]
    fn test_overlapping_placement_rejected() {
        let mut board = Board::new();
        assert!(board.place_ship(ShipPlacement::new(ShipKind::Carrier, 0, 0, Orientation::Horizontal)).is_ok());
        // Destroyer placed at (0, 2) intersects Carrier
        assert!(board.place_ship(ShipPlacement::new(ShipKind::Destroyer, 0, 2, Orientation::Vertical)).is_err());
    }

    #[test]
    fn test_out_of_bounds_rejected() {
        let mut board = Board::new();
        // Carrier length 5 placed horizontally at column 7 extends to 11 (out of bounds)
        assert!(board.place_ship(ShipPlacement::new(ShipKind::Carrier, 0, 7, Orientation::Horizontal)).is_err());
        // Battleship length 4 placed vertically at row 8 extends to 11 (out of bounds)
        assert!(board.place_ship(ShipPlacement::new(ShipKind::Battleship, 8, 0, Orientation::Vertical)).is_err());
    }

    #[test]
    fn test_shots_and_sinking() {
        let mut board = Board::new();
        board.place_ship(ShipPlacement::new(ShipKind::Destroyer, 5, 5, Orientation::Horizontal)).unwrap();

        // Miss
        assert_eq!(board.receive_shot(0, 0), ShotOutcome::Miss);
        assert_eq!(board.hits_on_ship(ShipKind::Destroyer), 0);

        // First hit on Destroyer
        assert_eq!(board.receive_shot(5, 5), ShotOutcome::Hit);
        assert_eq!(board.hits_on_ship(ShipKind::Destroyer), 1);
        assert!(!board.is_sunk(ShipKind::Destroyer));

        // Second hit sinks Destroyer
        assert_eq!(board.receive_shot(5, 6), ShotOutcome::Sunk(ShipKind::Destroyer));
        assert!(board.is_sunk(ShipKind::Destroyer));
    }

    #[test]
    fn test_tracking_board() {
        let mut tb = TrackingBoard::new();
        assert!(tb.is_unshot(3, 3));
        assert_eq!(tb.unshot_cells().len(), 100);

        tb.record_shot(3, 3, ShotOutcome::Miss);
        assert!(!tb.is_unshot(3, 3));
        assert_eq!(tb.cell(3, 3), TrackingCell::Miss);
        assert_eq!(tb.unshot_cells().len(), 99);

        tb.record_shot(4, 4, ShotOutcome::Hit);
        assert_eq!(tb.cell(4, 4), TrackingCell::Hit);
        assert_eq!(tb.hits_count(), 1);

        tb.record_shot(4, 5, ShotOutcome::Sunk(ShipKind::Destroyer));
        assert_eq!(tb.sunk_ships(), &[ShipKind::Destroyer]);
        assert_eq!(tb.remaining_unsunk_ships().len(), 4);
    }
}
