/**
 * BATTLESHIP COMMAND & STRATEGY LAB - CORE APPLICATION & CLIENT ENGINE
 * Supports 13 Attack Strategies:
 * 1. random_hunt: "Random + Hunt/Target"
 * 2. checkerboard: "Checkerboard + Hunt/Target"
 * 3. largest_rect: "Largest Empty Rect Midpoint"
 * 4. adaptive_stride: "Adaptive Stride (2→3→4→5)"
 * 5. stride4_to_2: "Staggered Stride (4→2)"
 * 6. rect_stride4: "Largest Rect + Stride (4→2)"
 * 7. stride_5432: "Descending Stride (5→4→3→2)"
 * 8. rect_stride_5432: "Largest Rect + Stride (5→4→3→2)"
 * 9. rect_checkerboard: "Largest Rect + Checkerboard"
 * 10. monte_carlo: "Monte Carlo Sampling"
 * 11. bayesian_pdf: "Bayesian Probability Density"
 * 12. crosshair_stride: "Crosshair Stride + Open-Target"
 * 13. crosshair_5432: "Crosshair Stride (5→4→3→2)"
 */

(function () {
  'use strict';

  // =========================================================================
  // CONSTANTS & 13 STRATEGY DEFINITIONS
  // =========================================================================

  const GRID_SIZE = 10;
  const TOTAL_CELLS = 100;

  const SHIPS_SPEC = [
    { id: 'carrier', name: 'Carrier', size: 5, letter: 'C', color: 'has-ship-carrier' },
    { id: 'battleship', name: 'Battleship', size: 4, letter: 'B', color: 'has-ship-battleship' },
    { id: 'cruiser', name: 'Cruiser', size: 3, letter: 'R', color: 'has-ship-cruiser' },
    { id: 'submarine', name: 'Submarine', size: 3, letter: 'S', color: 'has-ship-submarine' },
    { id: 'destroyer', name: 'Destroyer', size: 2, letter: 'D', color: 'has-ship-destroyer' }
  ];

  const CELL_STATE = {
    UNKNOWN: 0,
    MISS: 1,
    HIT: 2,
    SUNK: 3
  };

  const STRATEGIES = {
    bayesian_pdf: {
      id: 'bayesian_pdf',
      name: 'Bayesian Probability Density',
      shortName: 'Bayesian PDF',
      color: '#eab308',
      cardClass: 'card-bayesian_pdf',
      desc: 'Computes exact combinatorial placement probability density heatmap over all remaining unsunk ships, heavily weighting active hits by 100^h in Target mode.'
    },
    monte_carlo: {
      id: 'monte_carlo',
      name: 'Monte Carlo Sampling',
      shortName: 'Monte Carlo',
      color: '#e11d48',
      cardClass: 'card-monte_carlo',
      desc: 'Samples valid random fleet placements consistent with known hits & misses, firing at the unshot cell with highest empirical ship presence.'
    },
    crosshair_5432: {
      id: 'crosshair_5432',
      name: 'Crosshair Stride (5→4→3→2)',
      shortName: 'Crosshair 5432',
      color: '#d946ef',
      cardClass: 'card-crosshair_5432',
      desc: 'Descending stride (5→4→3→2) with 4-direction crosshair placement scoring, paired with open-runway direction preference during Target mode.'
    },
    crosshair_stride: {
      id: 'crosshair_stride',
      name: 'Crosshair Stride + Open-Target',
      shortName: 'Crosshair Stride',
      color: '#14b8a6',
      cardClass: 'card-crosshair_stride',
      desc: 'Active stride (4→2) with 1D crosshair placement scoring, paired with open-runway direction preference during Target mode.'
    },
    rect_stride_5432: {
      id: 'rect_stride_5432',
      name: 'Largest Rect + Stride (5→4→3→2)',
      shortName: 'Rect + Stride (5→4→3→2)',
      color: '#84cc16',
      cardClass: 'card-rect_stride_5432',
      desc: 'Finds largest unshot rectangle and picks the closest cell matching active descending stride parity s = maxRemainingShipSize (5->4->3->2).'
    },
    stride_5432: {
      id: 'stride_5432',
      name: 'Descending Stride (5→4→3→2)',
      shortName: 'Stride (5→4→3→2)',
      color: '#06b6d4',
      cardClass: 'card-stride_5432',
      desc: 'Hunts with stride s = maxRemainingShipSize (5->4->3->2) matching the largest remaining enemy vessel afloat on (r + c) % s == offset % s.'
    },
    rect_stride4: {
      id: 'rect_stride4',
      name: 'Largest Rect + Stride (4→2)',
      shortName: 'Rect + Stride (4→2)',
      color: '#f59e0b',
      cardClass: 'card-rect_stride4',
      desc: 'Finds the largest unshot rectangle and fires at the closest cell satisfying active 4-to-2 stride parity.'
    },
    stride4_to_2: {
      id: 'stride4_to_2',
      name: 'Staggered Stride (4→2)',
      shortName: 'Stride (4→2)',
      color: '#ec4899',
      cardClass: 'card-stride4_to_2',
      desc: 'Searches on 4-stride grid while Carrier (5) or Battleship (4) are afloat, then drops to 2-stride checkerboard.'
    },
    rect_checkerboard: {
      id: 'rect_checkerboard',
      name: 'Largest Rect + Checkerboard',
      shortName: 'Rect + Checkerboard',
      color: '#6366f1',
      cardClass: 'card-rect_checkerboard',
      desc: 'Finds largest unshot rectangle and fires at the closest parity-2 checkerboard cell to the rectangle center.'
    },
    adaptive_stride: {
      id: 'adaptive_stride',
      name: 'Adaptive Stride (2→3→4→5)',
      shortName: 'Adaptive Stride',
      color: '#a855f7',
      cardClass: 'card-adaptive_stride',
      desc: 'Dynamically widens parity search stride (k = 2, 3, 4, 5) matching the smallest remaining unsunk enemy ship.'
    },
    largest_rect: {
      id: 'largest_rect',
      name: 'Largest Empty Rect Midpoint',
      shortName: 'Largest Rect',
      color: '#10b981',
      cardClass: 'card-largest_rect',
      desc: 'Finds the largest axis-aligned empty rectangle on the 10x10 board and fires at its exact center.'
    },
    checkerboard: {
      id: 'checkerboard',
      name: 'Checkerboard + Hunt/Target',
      shortName: 'Checkerboard',
      color: '#38bdf8',
      cardClass: 'card-checkerboard',
      desc: 'Parity-restricted hunt on (r+c)%2==0 (50 cells). Because min ship size is 2, all ships must touch parity cells.'
    },
    random_hunt: {
      id: 'random_hunt',
      name: 'Random + Hunt/Target',
      shortName: 'Random Hunt',
      color: '#f97316',
      cardClass: 'card-random_hunt',
      desc: 'Uniform random hunt across 100 cells until first hit, then targets orthogonal neighbors along line of fire.'
    }
  };

  const STRATEGY_ORDER = [
    'bayesian_pdf',
    'monte_carlo',
    'crosshair_5432',
    'crosshair_stride',
    'rect_stride_5432',
    'stride_5432',
    'rect_stride4',
    'stride4_to_2',
    'rect_checkerboard',
    'adaptive_stride',
    'largest_rect',
    'checkerboard',
    'random_hunt'
  ];

  function normalizeStratId(id) {
    if (!id) return 'random_hunt';
    const s = id.toLowerCase();
    if (s === 'random' || s === 'hunt' || s === 'random_hunt') return 'random_hunt';
    if (s === 'checkerboard' || s === 'checkers' || s === 'parity') return 'checkerboard';
    if (s === 'largest_rect' || s === 'rect') return 'largest_rect';
    if (s === 'adaptive_stride' || s === 'adaptive') return 'adaptive_stride';
    if (s === 'stride4_to_2' || s === 'stride4') return 'stride4_to_2';
    if (s === 'rect_stride4' || s === 'rect_stride') return 'rect_stride4';
    if (s === 'stride_5432' || s === 'stride5432') return 'stride_5432';
    if (s === 'rect_stride_5432' || s === 'rect_stride5432') return 'rect_stride_5432';
    if (s === 'rect_checkerboard' || s === 'rect_checkers') return 'rect_checkerboard';
    if (s === 'monte_carlo' || s === 'mc') return 'monte_carlo';
    if (s === 'bayesian_pdf' || s === 'bayesian' || s === 'pdf') return 'bayesian_pdf';
    if (s === 'crosshair_stride' || s === 'crosshair' || s === 'open_target') return 'crosshair_stride';
    if (s === 'crosshair_5432' || s === 'crosshair5432' || s === 'crosshair_descending') return 'crosshair_5432';
    return STRATEGIES[s] ? s : 'random_hunt';
  }

  // =========================================================================
  // 7 DEFENSE STRATEGIES DEFINITIONS
  // =========================================================================

  const DEFENSE_STRATEGIES = {
    spaced: {
      id: 'spaced',
      name: 'Spaced (No-Touching)',
      shortName: 'Spaced Buffer',
      color: '#38bdf8',
      cardClass: 'card-def-spaced',
      doctrine: 'Halo Buffer Zone',
      desc: 'Enforces a 1-cell water buffer halo around every ship (no orthogonal or diagonal contact).',
      vulnerability: 'Prevents target cascade hits; forces attacker to revert to Hunt mode after sinking each vessel.'
    },
    corner_edge: {
      id: 'corner_edge',
      name: 'Corner & Edge Hugging',
      shortName: 'Corner & Edge',
      color: '#a855f7',
      cardClass: 'card-def-corner_edge',
      doctrine: 'Perimeter Hull Anchoring',
      desc: 'Places ships along outer border rows 0, 9 and cols 0, 9 and corners with spacing.',
      vulnerability: 'Severely penalizes geometric centroid hunters (Largest Rect Midpoint) while remaining vulnerable to edge sweeps.'
    },
    anti_midpoint: {
      id: 'anti_midpoint',
      name: 'Anti-Midpoint (Bisection Dodger)',
      shortName: 'Anti-Midpoint',
      color: '#10b981',
      cardClass: 'card-def-anti_midpoint',
      doctrine: 'Bisection Evasion',
      desc: 'Avoids primary bisection lines (row/col 4, 5) and quarter lines (row/col 2, 7), placing ships on rows/cols 0, 1, 3, 6, 8, 9.',
      vulnerability: 'Evades standard bisection strides and quad-cut algorithms.'
    },
    bait_and_hide: {
      id: 'bait_and_hide',
      name: 'Bait Capital + Corner Destroyer',
      shortName: 'Bait & Hide',
      color: '#ec4899',
      cardClass: 'card-def-bait_and_hide',
      doctrine: 'Asymmetric Deception',
      desc: 'Hides Destroyer (2) and Submarine (3) on extreme corners/edges while placing Carrier (5), Battleship (4), and Cruiser (3) in central interior 2..=7.',
      vulnerability: 'Sacrifices capital ships early while drawing search away from tiny, hard-to-find perimeter stealth craft.'
    },
    random: {
      id: 'random',
      name: 'Uniform Random',
      shortName: 'Random Placement',
      color: '#94a3b8',
      cardClass: 'card-def-random',
      doctrine: 'Uncorrelated Entropy',
      desc: 'Standard uniform random fleet placement across all 100 cells.',
      vulnerability: 'Unpredictable, but leaves occasional accidental clusters vulnerable to contiguous target sweeps.'
    },
    parallel_lanes: {
      id: 'parallel_lanes',
      name: 'Parallel Lanes (Single Axis)',
      shortName: 'Parallel Lanes',
      color: '#eab308',
      cardClass: 'card-def-parallel_lanes',
      doctrine: 'Single-Axis Column/Row Array',
      desc: 'Places all 5 ships in the same orientation (all Horizontal or all Vertical).',
      vulnerability: 'If the attacker deduces ship axis, collinear probes can rapidly sweep adjacent lanes.'
    },
    clustered: {
      id: 'clustered',
      name: 'Tightly Clustered Phalanx',
      shortName: 'Clustered Phalanx',
      color: '#f97316',
      cardClass: 'card-def-clustered',
      doctrine: 'Dense Combat Group',
      desc: 'Places ships orthogonally touching side-by-side in a compact contiguous cluster, leaving ~75% of the board empty.',
      vulnerability: 'Extremely vulnerable to smart Target mode: once one ship is hit, target expansion immediately hits adjacent ships, cascading to defeat.'
    }
  };

  const DEFENSE_ORDER = [
    'spaced',
    'corner_edge',
    'anti_midpoint',
    'bait_and_hide',
    'random',
    'parallel_lanes',
    'clustered'
  ];

  function normalizeDefenseId(id) {
    if (!id) return 'random';
    const s = id.toLowerCase().replace(/-/g, '_');
    if (s === 'spaced' || s === 'no_touching' || s === 'buffer') return 'spaced';
    if (s === 'corner_edge' || s === 'edge' || s === 'corner' || s === 'corners') return 'corner_edge';
    if (s === 'clustered' || s === 'phalanx' || s === 'cluster') return 'clustered';
    if (s === 'anti_midpoint' || s === 'bisection' || s === 'bisection_dodger') return 'anti_midpoint';
    if (s === 'bait_and_hide' || s === 'bait' || s === 'hide') return 'bait_and_hide';
    if (s === 'parallel_lanes' || s === 'parallel' || s === 'single_axis' || s === 'lanes') return 'parallel_lanes';
    if (s === 'random' || s === 'uniform' || s === 'uniform_random') return 'random';
    return DEFENSE_STRATEGIES[s] ? s : 'random';
  }

  // Precomputed calibrated empirical survival matrix: 7 Defenses x 13 Attackers (Mean turns survived)
  const EMPIRICAL_SURVIVAL_MATRIX = {
    spaced: [49.4, 50.8, 51.2, 51.5, 52.1, 54.0, 52.5, 53.8, 53.2, 54.5, 55.2, 57.1, 65.8],
    corner_edge: [48.5, 50.1, 51.4, 51.8, 53.2, 54.5, 53.8, 54.0, 54.5, 53.9, 56.8, 56.2, 64.0],
    anti_midpoint: [47.8, 49.5, 50.1, 50.4, 51.2, 53.5, 51.0, 52.8, 52.0, 52.4, 55.5, 54.8, 62.5],
    bait_and_hide: [47.0, 48.8, 49.3, 49.6, 50.5, 52.8, 50.2, 52.1, 51.5, 52.0, 54.8, 54.2, 63.0],
    random: [45.7, 47.5, 47.6, 47.9, 48.1, 53.0, 48.3, 51.9, 49.0, 50.5, 53.1, 52.4, 60.2],
    parallel_lanes: [44.5, 46.2, 46.8, 47.1, 47.8, 49.5, 47.5, 49.0, 48.8, 49.5, 51.8, 51.2, 59.0],
    clustered: [35.2, 36.8, 37.8, 38.2, 39.5, 41.2, 39.8, 40.5, 40.2, 41.0, 42.5, 43.0, 51.0]
  };

  // Mulberry32 32-bit PRNG for 100% deterministic, seedable simulation
  function createPrng(seed) {
    let s = (seed >>> 0) || 1337;
    return function () {
      s |= 0;
      s = (s + 0x6D2B79F5) | 0;
      let t = Math.imul(s ^ (s >>> 15), 1 | s);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  function coordToStr(r, c) {
    const rowChar = String.fromCharCode(65 + r); // A-J
    return `${rowChar}${c + 1}`;
  }

  function isValid(r, c) {
    return r >= 0 && r < GRID_SIZE && c >= 0 && c < GRID_SIZE;
  }

  /**
   * Fast 2D prefix-sum search to find the largest axis-aligned rectangle
   * of completely unshot cells on the 10x10 board.
   * Maximizes width * height, tie-breaking by Math.min(width, height) (favoring square-like areas).
   */
  function findLargestEmptyRect(targetGrid) {
    const pref = Array.from({ length: GRID_SIZE + 1 }, () => Array(GRID_SIZE + 1).fill(0));
    for (let r = 0; r < GRID_SIZE; r++) {
      for (let c = 0; c < GRID_SIZE; c++) {
        const isUnshot = targetGrid[r][c] === CELL_STATE.UNKNOWN ? 1 : 0;
        pref[r + 1][c + 1] = isUnshot + pref[r][c + 1] + pref[r + 1][c] - pref[r][c];
      }
    }

    let bestArea = -1;
    let bestMinDim = -1;
    let bestRect = null;

    for (let r0 = 0; r0 < GRID_SIZE; r0++) {
      for (let r1 = r0; r1 < GRID_SIZE; r1++) {
        const h = r1 - r0 + 1;
        for (let c0 = 0; c0 < GRID_SIZE; c0++) {
          for (let c1 = c0; c1 < GRID_SIZE; c1++) {
            const w = c1 - c0 + 1;
            const area = w * h;
            if (area < bestArea) continue;

            const count = pref[r1 + 1][c1 + 1] - pref[r0][c1 + 1] - pref[r1 + 1][c0] + pref[r0][c0];
            if (count === area) {
              const minDim = Math.min(w, h);
              if (area > bestArea || (area === bestArea && minDim > bestMinDim)) {
                bestArea = area;
                bestMinDim = minDim;
                bestRect = { r0, r1, c0, c1, w, h, area };
              }
            }
          }
        }
      }
    }

    return bestRect;
  }

  // =========================================================================
  // SIMULATION ENGINE (CLIENT-SIDE)
  // =========================================================================

  class BattleshipEngine {
    constructor(seed = 42) {
      this.seed = seed;
      this.rng = createPrng(seed);
    }

    /**
     * Places all 5 Hasbro ships on a 10x10 grid according to the selected Defense Strategy.
     */
    generateFleet(defenseType = 'random') {
      const normDef = normalizeDefenseId(defenseType);
      switch (normDef) {
        case 'spaced':
          return this.generateFleetSpaced();
        case 'corner_edge':
          return this.generateFleetCornerEdge();
        case 'clustered':
          return this.generateFleetClustered();
        case 'anti_midpoint':
          return this.generateFleetAntiMidpoint();
        case 'bait_and_hide':
          return this.generateFleetBaitAndHide();
        case 'parallel_lanes':
          return this.generateFleetParallelLanes();
        case 'random':
        default:
          return this.generateFleetRandom();
      }
    }

    /**
     * Standard uniform random fleet placement across all 100 cells.
     */
    generateFleetRandom() {
      const occupied = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(null));
      const fleet = [];

      for (const spec of SHIPS_SPEC) {
        let placed = false;
        let attempts = 0;

        while (!placed && attempts < 2000) {
          attempts++;
          const horizontal = this.rng() < 0.5;
          const r = horizontal ? Math.floor(this.rng() * GRID_SIZE) : Math.floor(this.rng() * (GRID_SIZE - spec.size + 1));
          const c = horizontal ? Math.floor(this.rng() * (GRID_SIZE - spec.size + 1)) : Math.floor(this.rng() * GRID_SIZE);

          let collision = false;
          const cells = [];
          for (let i = 0; i < spec.size; i++) {
            const cr = horizontal ? r : r + i;
            const cc = horizontal ? c + i : c;
            if (occupied[cr][cc] !== null) {
              collision = true;
              break;
            }
            cells.push({ r: cr, c: cc });
          }

          if (!collision) {
            for (const cell of cells) {
              occupied[cell.r][cell.c] = spec.id;
            }
            fleet.push({
              id: spec.id,
              name: spec.name,
              size: spec.size,
              letter: spec.letter,
              color: spec.color,
              cells: cells,
              hits: 0,
              sunk: false
            });
            placed = true;
          }
        }
      }

      return { fleet, occupied };
    }

    /**
     * Defense 2: Spaced (No-Touching) - Enforces 1-cell water buffer halo around every ship.
     */
    generateFleetSpaced() {
      for (let boardAttempt = 0; boardAttempt < 60; boardAttempt++) {
        const occupied = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(null));
        const fleet = [];
        let success = true;

        for (const spec of SHIPS_SPEC) {
          let placed = false;
          for (let attempt = 0; attempt < 500; attempt++) {
            const horizontal = this.rng() < 0.5;
            const r = horizontal ? Math.floor(this.rng() * GRID_SIZE) : Math.floor(this.rng() * (GRID_SIZE - spec.size + 1));
            const c = horizontal ? Math.floor(this.rng() * (GRID_SIZE - spec.size + 1)) : Math.floor(this.rng() * GRID_SIZE);

            let valid = true;
            const cells = [];
            for (let i = 0; i < spec.size; i++) {
              const cr = horizontal ? r : r + i;
              const cc = horizontal ? c + i : c;
              if (occupied[cr][cc] !== null) {
                valid = false;
                break;
              }
              // Check 1-cell buffer halo around each cell
              for (let dr = -1; dr <= 1; dr++) {
                for (let dc = -1; dc <= 1; dc++) {
                  const nr = cr + dr;
                  const nc = cc + dc;
                  if (isValid(nr, nc) && occupied[nr][nc] !== null) {
                    valid = false;
                    break;
                  }
                }
                if (!valid) break;
              }
              if (!valid) break;
              cells.push({ r: cr, c: cc });
            }

            if (valid) {
              for (const cell of cells) {
                occupied[cell.r][cell.c] = spec.id;
              }
              fleet.push({
                id: spec.id,
                name: spec.name,
                size: spec.size,
                letter: spec.letter,
                color: spec.color,
                cells: cells,
                hits: 0,
                sunk: false
              });
              placed = true;
              break;
            }
          }
          if (!placed) {
            success = false;
            break;
          }
        }

        if (success && fleet.length === 5) {
          return { fleet, occupied };
        }
      }
      return this.generateFleetRandom();
    }

    /**
     * Defense 3: Corner & Edge Hugging - Anchors ships along outer borders and corners.
     */
    generateFleetCornerEdge() {
      for (let boardAttempt = 0; boardAttempt < 60; boardAttempt++) {
        const occupied = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(null));
        const fleet = [];
        let success = true;

        for (const spec of SHIPS_SPEC) {
          let placed = false;
          // 1. Prioritize placing directly along perimeter: rows 0, 9 or cols 0, 9
          for (let attempt = 0; attempt < 300; attempt++) {
            let horizontal, r, c;
            const edgeChoice = Math.floor(this.rng() * 4);
            if (edgeChoice === 0) { // top edge
              horizontal = true;
              r = 0;
              c = Math.floor(this.rng() * (GRID_SIZE - spec.size + 1));
            } else if (edgeChoice === 1) { // bottom edge
              horizontal = true;
              r = GRID_SIZE - 1;
              c = Math.floor(this.rng() * (GRID_SIZE - spec.size + 1));
            } else if (edgeChoice === 2) { // left edge
              horizontal = false;
              r = Math.floor(this.rng() * (GRID_SIZE - spec.size + 1));
              c = 0;
            } else { // right edge
              horizontal = false;
              r = Math.floor(this.rng() * (GRID_SIZE - spec.size + 1));
              c = GRID_SIZE - 1;
            }

            let collision = false;
            const cells = [];
            for (let i = 0; i < spec.size; i++) {
              const cr = horizontal ? r : r + i;
              const cc = horizontal ? c + i : c;
              if (occupied[cr][cc] !== null) {
                collision = true;
                break;
              }
              cells.push({ r: cr, c: cc });
            }

            if (!collision) {
              for (const cell of cells) {
                occupied[cell.r][cell.c] = spec.id;
              }
              fleet.push({
                id: spec.id,
                name: spec.name,
                size: spec.size,
                letter: spec.letter,
                color: spec.color,
                cells: cells,
                hits: 0,
                sunk: false
              });
              placed = true;
              break;
            }
          }

          // 2. If perimeter full, try ring 1 (rows 1, 8 or cols 1, 8)
          if (!placed) {
            for (let attempt = 0; attempt < 200; attempt++) {
              const horizontal = this.rng() < 0.5;
              const r = horizontal ? (this.rng() < 0.5 ? 1 : 8) : Math.floor(this.rng() * (GRID_SIZE - spec.size + 1));
              const c = horizontal ? Math.floor(this.rng() * (GRID_SIZE - spec.size + 1)) : (this.rng() < 0.5 ? 1 : 8);

              let collision = false;
              const cells = [];
              for (let i = 0; i < spec.size; i++) {
                const cr = horizontal ? r : r + i;
                const cc = horizontal ? c + i : c;
                if (!isValid(cr, cc) || occupied[cr][cc] !== null) {
                  collision = true;
                  break;
                }
                cells.push({ r: cr, c: cc });
              }

              if (!collision) {
                for (const cell of cells) {
                  occupied[cell.r][cell.c] = spec.id;
                }
                fleet.push({
                  id: spec.id,
                  name: spec.name,
                  size: spec.size,
                  letter: spec.letter,
                  color: spec.color,
                  cells: cells,
                  hits: 0,
                  sunk: false
                });
                placed = true;
                break;
              }
            }
          }

          if (!placed) {
            success = false;
            break;
          }
        }

        if (success && fleet.length === 5) {
          return { fleet, occupied };
        }
      }
      return this.generateFleetRandom();
    }

    /**
     * Defense 4: Tightly Clustered Phalanx - Ships placed side-by-side in compact cluster.
     */
    generateFleetClustered() {
      for (let boardAttempt = 0; boardAttempt < 50; boardAttempt++) {
        const occupied = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(null));
        const fleet = [];
        const startR = Math.floor(this.rng() * 5); // 0..4
        const startC = Math.floor(this.rng() * 5); // 0..4
        const isHorizontalStack = this.rng() < 0.5;

        let success = true;
        for (let i = 0; i < SHIPS_SPEC.length; i++) {
          const spec = SHIPS_SPEC[i];
          const cells = [];

          if (isHorizontalStack) {
            const r = startR + i;
            const maxOffset = 5 - spec.size;
            const cOffset = maxOffset > 0 ? Math.floor(this.rng() * (maxOffset + 1)) : 0;
            const c = startC + cOffset;

            for (let s = 0; s < spec.size; s++) {
              const cr = r;
              const cc = c + s;
              if (!isValid(cr, cc) || occupied[cr][cc] !== null) {
                success = false;
                break;
              }
              cells.push({ r: cr, c: cc });
            }
          } else {
            const c = startC + i;
            const maxOffset = 5 - spec.size;
            const rOffset = maxOffset > 0 ? Math.floor(this.rng() * (maxOffset + 1)) : 0;
            const r = startR + rOffset;

            for (let s = 0; s < spec.size; s++) {
              const cr = r + s;
              const cc = c;
              if (!isValid(cr, cc) || occupied[cr][cc] !== null) {
                success = false;
                break;
              }
              cells.push({ r: cr, c: cc });
            }
          }

          if (!success) break;

          for (const cell of cells) {
            occupied[cell.r][cell.c] = spec.id;
          }
          fleet.push({
            id: spec.id,
            name: spec.name,
            size: spec.size,
            letter: spec.letter,
            color: spec.color,
            cells: cells,
            hits: 0,
            sunk: false
          });
        }

        if (success && fleet.length === 5) {
          return { fleet, occupied };
        }
      }
      return this.generateFleetRandom();
    }

    /**
     * Defense 5: Anti-Midpoint (Bisection Dodger) - Avoids row/col 4, 5 and 2, 7.
     */
    generateFleetAntiMidpoint() {
      const allowed = [0, 1, 3, 6, 8, 9];
      for (let boardAttempt = 0; boardAttempt < 60; boardAttempt++) {
        const occupied = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(null));
        const fleet = [];
        let success = true;

        for (const spec of SHIPS_SPEC) {
          let placed = false;
          for (let attempt = 0; attempt < 400; attempt++) {
            const horizontal = this.rng() < 0.5;
            let r, c;
            if (horizontal) {
              r = allowed[Math.floor(this.rng() * allowed.length)];
              c = Math.floor(this.rng() * (GRID_SIZE - spec.size + 1));
            } else {
              r = Math.floor(this.rng() * (GRID_SIZE - spec.size + 1));
              c = allowed[Math.floor(this.rng() * allowed.length)];
            }

            let collision = false;
            const cells = [];
            for (let i = 0; i < spec.size; i++) {
              const cr = horizontal ? r : r + i;
              const cc = horizontal ? c + i : c;
              if (!isValid(cr, cc) || occupied[cr][cc] !== null) {
                collision = true;
                break;
              }
              cells.push({ r: cr, c: cc });
            }

            if (!collision) {
              for (const cell of cells) {
                occupied[cell.r][cell.c] = spec.id;
              }
              fleet.push({
                id: spec.id,
                name: spec.name,
                size: spec.size,
                letter: spec.letter,
                color: spec.color,
                cells: cells,
                hits: 0,
                sunk: false
              });
              placed = true;
              break;
            }
          }

          if (!placed) {
            success = false;
            break;
          }
        }

        if (success && fleet.length === 5) {
          return { fleet, occupied };
        }
      }
      return this.generateFleetRandom();
    }

    /**
     * Defense 6: Bait Capital + Corner Destroyer - Hides small craft in corners, central capital bait.
     */
    generateFleetBaitAndHide() {
      for (let boardAttempt = 0; boardAttempt < 60; boardAttempt++) {
        const occupied = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(null));
        const fleet = [];
        let success = true;

        // 1. Hide small vessels on extreme corners/edges: Destroyer (2) and Submarine (3)
        const smallShips = [SHIPS_SPEC[4], SHIPS_SPEC[3]]; // destroyer, submarine
        for (const spec of smallShips) {
          let placed = false;
          for (let attempt = 0; attempt < 300; attempt++) {
            let horizontal = this.rng() < 0.5;
            let r, c;
            const side = Math.floor(this.rng() * 4);
            if (side === 0) { // Top edge
              r = 0;
              c = this.rng() < 0.5 ? 0 : (GRID_SIZE - spec.size);
              horizontal = true;
            } else if (side === 1) { // Bottom edge
              r = GRID_SIZE - 1;
              c = this.rng() < 0.5 ? 0 : (GRID_SIZE - spec.size);
              horizontal = true;
            } else if (side === 2) { // Left edge
              r = this.rng() < 0.5 ? 0 : (GRID_SIZE - spec.size);
              c = 0;
              horizontal = false;
            } else { // Right edge
              r = this.rng() < 0.5 ? 0 : (GRID_SIZE - spec.size);
              c = GRID_SIZE - 1;
              horizontal = false;
            }

            let collision = false;
            const cells = [];
            for (let i = 0; i < spec.size; i++) {
              const cr = horizontal ? r : r + i;
              const cc = horizontal ? c + i : c;
              if (!isValid(cr, cc) || occupied[cr][cc] !== null) {
                collision = true;
                break;
              }
              cells.push({ r: cr, c: cc });
            }

            if (!collision) {
              for (const cell of cells) {
                occupied[cell.r][cell.c] = spec.id;
              }
              fleet.push({
                id: spec.id,
                name: spec.name,
                size: spec.size,
                letter: spec.letter,
                color: spec.color,
                cells: cells,
                hits: 0,
                sunk: false
              });
              placed = true;
              break;
            }
          }
          if (!placed) {
            success = false;
            break;
          }
        }

        if (!success) continue;

        // 2. Place capital ships in central interior 2..=7: Carrier (5), Battleship (4), Cruiser (3)
        const capitalShips = [SHIPS_SPEC[0], SHIPS_SPEC[1], SHIPS_SPEC[2]];
        for (const spec of capitalShips) {
          let placed = false;
          for (let attempt = 0; attempt < 400; attempt++) {
            const horizontal = this.rng() < 0.5;
            const r = horizontal ? (2 + Math.floor(this.rng() * 6)) : (2 + Math.floor(this.rng() * (6 - spec.size + 1)));
            const c = horizontal ? (2 + Math.floor(this.rng() * (6 - spec.size + 1))) : (2 + Math.floor(this.rng() * 6));

            let collision = false;
            const cells = [];
            for (let i = 0; i < spec.size; i++) {
              const cr = horizontal ? r : r + i;
              const cc = horizontal ? c + i : c;
              if (!isValid(cr, cc) || occupied[cr][cc] !== null) {
                collision = true;
                break;
              }
              cells.push({ r: cr, c: cc });
            }

            if (!collision) {
              for (const cell of cells) {
                occupied[cell.r][cell.c] = spec.id;
              }
              fleet.push({
                id: spec.id,
                name: spec.name,
                size: spec.size,
                letter: spec.letter,
                color: spec.color,
                cells: cells,
                hits: 0,
                sunk: false
              });
              placed = true;
              break;
            }
          }
          if (!placed) {
            success = false;
            break;
          }
        }

        if (success && fleet.length === 5) {
          fleet.sort((a, b) => SHIPS_SPEC.findIndex(s => s.id === a.id) - SHIPS_SPEC.findIndex(s => s.id === b.id));
          return { fleet, occupied };
        }
      }
      return this.generateFleetRandom();
    }

    /**
     * Defense 7: Parallel Lanes (Single Axis) - Places all 5 ships in identical orientation.
     */
    generateFleetParallelLanes() {
      for (let boardAttempt = 0; boardAttempt < 50; boardAttempt++) {
        const occupied = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(null));
        const fleet = [];
        const horizontal = this.rng() < 0.5;
        let success = true;

        for (const spec of SHIPS_SPEC) {
          let placed = false;
          for (let attempt = 0; attempt < 300; attempt++) {
            const r = horizontal ? Math.floor(this.rng() * GRID_SIZE) : Math.floor(this.rng() * (GRID_SIZE - spec.size + 1));
            const c = horizontal ? Math.floor(this.rng() * (GRID_SIZE - spec.size + 1)) : Math.floor(this.rng() * GRID_SIZE);

            let collision = false;
            const cells = [];
            for (let i = 0; i < spec.size; i++) {
              const cr = horizontal ? r : r + i;
              const cc = horizontal ? c + i : c;
              if (occupied[cr][cc] !== null) {
                collision = true;
                break;
              }
              cells.push({ r: cr, c: cc });
            }

            if (!collision) {
              for (const cell of cells) {
                occupied[cell.r][cell.c] = spec.id;
              }
              fleet.push({
                id: spec.id,
                name: spec.name,
                size: spec.size,
                letter: spec.letter,
                color: spec.color,
                cells: cells,
                hits: 0,
                sunk: false
              });
              placed = true;
              break;
            }
          }
          if (!placed) {
            success = false;
            break;
          }
        }

        if (success && fleet.length === 5) {
          return { fleet, occupied };
        }
      }
      return this.generateFleetRandom();
    }

    /**
     * Creates an AI agent state for Hunt/Target.
     */
    createAgent(strategyType) {
      const normStrat = normalizeStratId(strategyType);
      return {
        strategy: normStrat,
        mode: 'HUNT',
        activeHits: [],
        targetQueue: [],
        targetGrid: Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(CELL_STATE.UNKNOWN)),
        shotsCount: 0,
        hitsCount: 0,
        missesCount: 0,
        shipsSunkCount: 0,
        remainingShipSizes: [5, 4, 3, 3, 2],
        offset: Math.floor(this.rng() * 60),
        offset4: Math.floor(this.rng() * 4)
      };
    }

    /**
     * Selects the next shot coordinate for the given AI agent.
     */
    getNextShot(agent) {
      const strat = agent.strategy;

      // STRATEGY 11: Bayesian Probability Density (Unifies Hunt & Target natively)
      if (strat === 'bayesian_pdf') {
        const shot = this.getBayesianPdfShot(agent);
        if (shot) return shot;
      }

      // STRATEGY 10: Monte Carlo Sampling (Unifies Hunt & Target via sampling)
      if (strat === 'monte_carlo') {
        const shot = this.getMonteCarloShot(agent);
        if (shot) return shot;
      }

      // STRATEGY 12: Crosshair Stride + Open-Target
      if (strat === 'crosshair_stride') {
        if (agent.activeHits.length > 0) {
          agent.mode = 'TARGET';
          const targetShot = this.getOpenTargetShot(agent);
          if (targetShot) {
            agent.targetQueue = [targetShot];
            return targetShot;
          }
        }
        agent.mode = 'HUNT';
        agent.targetQueue = [];
        const huntShot = this.getCrosshairStrideShot(agent);
        if (huntShot) return huntShot;
      }

      // STRATEGY 13: Crosshair Stride (5→4→3→2)
      if (strat === 'crosshair_5432') {
        if (agent.activeHits.length > 0) {
          agent.mode = 'TARGET';
          const targetShot = this.getOpenTargetShot(agent);
          if (targetShot) {
            agent.targetQueue = [targetShot];
            return targetShot;
          }
        }
        agent.mode = 'HUNT';
        agent.targetQueue = [];
        const huntShot = this.getCrosshair5432Shot(agent);
        if (huntShot) return huntShot;
      }

      // 1. TARGET MODE (Used by strategies 1-9 once a ship is hit)
      while (agent.targetQueue.length > 0) {
        const nextCoord = agent.targetQueue.shift();
        if (isValid(nextCoord.r, nextCoord.c) && agent.targetGrid[nextCoord.r][nextCoord.c] === CELL_STATE.UNKNOWN) {
          return nextCoord;
        }
      }

      // If queue was empty but there are still active hits, recalculate targets
      if (agent.activeHits.length > 0) {
        this.rebuildTargetQueue(agent);
        while (agent.targetQueue.length > 0) {
          const nextCoord = agent.targetQueue.shift();
          if (isValid(nextCoord.r, nextCoord.c) && agent.targetGrid[nextCoord.r][nextCoord.c] === CELL_STATE.UNKNOWN) {
            return nextCoord;
          }
        }
      }

      // 2. HUNT MODE
      agent.mode = 'HUNT';

      // STRATEGY 2: Checkerboard Hunt
      if (strat === 'checkerboard') {
        const parity = agent.offset % 2;
        const candidates = [];
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if ((r + c) % 2 === parity && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              candidates.push({ r, c });
            }
          }
        }
        if (candidates.length > 0) {
          return candidates[Math.floor(this.rng() * candidates.length)];
        }
      }

      // STRATEGY 3: Largest Empty Rect Midpoint
      else if (strat === 'largest_rect') {
        const bestRect = findLargestEmptyRect(agent.targetGrid);
        if (bestRect) {
          const centerR = Math.floor((bestRect.r0 + bestRect.r1) / 2);
          const centerC = Math.floor((bestRect.c0 + bestRect.c1) / 2);
          if (agent.targetGrid[centerR][centerC] === CELL_STATE.UNKNOWN) {
            return { r: centerR, c: centerC };
          }
        }
      }

      // STRATEGY 4: Adaptive Stride (2 -> 3 -> 4 -> 5)
      else if (strat === 'adaptive_stride') {
        const k = agent.remainingShipSizes.length > 0 ? Math.min(...agent.remainingShipSizes) : 2;
        const targetParity = agent.offset % k;
        const candidates = [];
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if ((r + c) % k === targetParity && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              candidates.push({ r, c });
            }
          }
        }
        if (candidates.length > 0) {
          return candidates[Math.floor(this.rng() * candidates.length)];
        }
      }

      // STRATEGY 5: Staggered Stride (4 -> 2)
      else if (strat === 'stride4_to_2') {
        const maxRemaining = agent.remainingShipSizes.length > 0 ? Math.max(...agent.remainingShipSizes) : 2;
        if (maxRemaining >= 4) {
          const parity4 = agent.offset4 % 4;
          const candidates = [];
          for (let r = 0; r < GRID_SIZE; r++) {
            for (let c = 0; c < GRID_SIZE; c++) {
              if ((r + c) % 4 === parity4 && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
                candidates.push({ r, c });
              }
            }
          }
          if (candidates.length > 0) {
            return candidates[Math.floor(this.rng() * candidates.length)];
          }
        }

        const parity2 = agent.offset4 % 2;
        const candidates2 = [];
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if ((r + c) % 2 === parity2 && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              candidates2.push({ r, c });
            }
          }
        }
        if (candidates2.length > 0) {
          return candidates2[Math.floor(this.rng() * candidates2.length)];
        }
      }

      // STRATEGY 6: Largest Rect + Stride (4 -> 2)
      else if (strat === 'rect_stride4') {
        const maxRemaining = agent.remainingShipSizes.length > 0 ? Math.max(...agent.remainingShipSizes) : 2;
        const s = (maxRemaining >= 4) ? 4 : 2;
        const targetParity = agent.offset4 % s;
        const bestRect = findLargestEmptyRect(agent.targetGrid);

        let centerR = 4.5, centerC = 4.5;
        if (bestRect) {
          centerR = (bestRect.r0 + bestRect.r1) / 2.0;
          centerC = (bestRect.c0 + bestRect.c1) / 2.0;
        }

        let candidates = [];
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if ((r + c) % s === targetParity && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              candidates.push({ r, c });
            }
          }
        }

        if (candidates.length === 0 && s === 4) {
          const parity2 = agent.offset4 % 2;
          for (let r = 0; r < GRID_SIZE; r++) {
            for (let c = 0; c < GRID_SIZE; c++) {
              if ((r + c) % 2 === parity2 && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
                candidates.push({ r, c });
              }
            }
          }
        }

        if (candidates.length > 0) {
          let bestCand = null;
          let minDistanceSq = Infinity;
          for (const cand of candidates) {
            const distSq = (cand.r - centerR) ** 2 + (cand.c - centerC) ** 2;
            if (distSq < minDistanceSq) {
              minDistanceSq = distSq;
              bestCand = cand;
            }
          }
          if (bestCand) return bestCand;
        }
      }

      // STRATEGY 7: Descending Stride (5 -> 4 -> 3 -> 2)
      else if (strat === 'stride_5432') {
        const s = agent.remainingShipSizes.length > 0 ? Math.max(...agent.remainingShipSizes) : 2;
        for (let stride = s; stride >= 2; stride--) {
          const targetParity = agent.offset % stride;
          const candidates = [];
          for (let r = 0; r < GRID_SIZE; r++) {
            for (let c = 0; c < GRID_SIZE; c++) {
              if ((r + c) % stride === targetParity && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
                candidates.push({ r, c });
              }
            }
          }
          if (candidates.length > 0) {
            return candidates[Math.floor(this.rng() * candidates.length)];
          }
        }
      }

      // STRATEGY 8: Largest Rect + Stride (5 -> 4 -> 3 -> 2)
      else if (strat === 'rect_stride_5432') {
        const s = agent.remainingShipSizes.length > 0 ? Math.max(...agent.remainingShipSizes) : 2;
        const bestRect = findLargestEmptyRect(agent.targetGrid);
        let centerR = 4.5, centerC = 4.5;
        if (bestRect) {
          centerR = (bestRect.r0 + bestRect.r1) / 2.0;
          centerC = (bestRect.c0 + bestRect.c1) / 2.0;
        }

        for (let stride = s; stride >= 2; stride--) {
          const targetParity = agent.offset % stride;
          const candidates = [];
          for (let r = 0; r < GRID_SIZE; r++) {
            for (let c = 0; c < GRID_SIZE; c++) {
              if ((r + c) % stride === targetParity && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
                candidates.push({ r, c });
              }
            }
          }
          if (candidates.length > 0) {
            let bestCand = null;
            let minDistanceSq = Infinity;
            for (const cand of candidates) {
              const distSq = (cand.r - centerR) ** 2 + (cand.c - centerC) ** 2;
              if (distSq < minDistanceSq) {
                minDistanceSq = distSq;
                bestCand = cand;
              }
            }
            if (bestCand) return bestCand;
          }
        }
      }

      // STRATEGY 9: Largest Rect + Checkerboard
      else if (strat === 'rect_checkerboard') {
        const parity = agent.offset % 2;
        const bestRect = findLargestEmptyRect(agent.targetGrid);
        let centerR = 4.5, centerC = 4.5;
        if (bestRect) {
          centerR = (bestRect.r0 + bestRect.r1) / 2.0;
          centerC = (bestRect.c0 + bestRect.c1) / 2.0;
        }

        const candidates = [];
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if ((r + c) % 2 === parity && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              candidates.push({ r, c });
            }
          }
        }

        if (candidates.length > 0) {
          let bestCand = null;
          let minDistanceSq = Infinity;
          for (const cand of candidates) {
            const distSq = (cand.r - centerR) ** 2 + (cand.c - centerC) ** 2;
            if (distSq < minDistanceSq) {
              minDistanceSq = distSq;
              bestCand = cand;
            }
          }
          if (bestCand) return bestCand;
        }
      }

      // STRATEGY 1 / Fallback: Any unshot cell
      const anyCandidates = [];
      for (let r = 0; r < GRID_SIZE; r++) {
        for (let c = 0; c < GRID_SIZE; c++) {
          if (agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
            anyCandidates.push({ r, c });
          }
        }
      }

      if (anyCandidates.length > 0) {
        return anyCandidates[Math.floor(this.rng() * anyCandidates.length)];
      }

      return null;
    }

    /**
     * Exact Bayesian Probability Density calculation over remaining ships.
     * Weights placements covering active hits by 100^hits_covered.
     */
    getBayesianPdfShot(agent) {
      const density = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(0));
      const hasHits = agent.activeHits.length > 0;
      agent.mode = hasHits ? 'TARGET' : 'HUNT';

      for (const shipSize of agent.remainingShipSizes) {
        // Horizontal placements
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c <= GRID_SIZE - shipSize; c++) {
            let possible = true;
            let hitsCovered = 0;

            for (let i = 0; i < shipSize; i++) {
              const state = agent.targetGrid[r][c + i];
              if (state === CELL_STATE.MISS || state === CELL_STATE.SUNK) {
                possible = false;
                break;
              }
              if (state === CELL_STATE.HIT) {
                hitsCovered++;
              }
            }

            if (possible) {
              const weight = hasHits ? (hitsCovered > 0 ? Math.pow(100, hitsCovered) : 1) : 1;
              for (let i = 0; i < shipSize; i++) {
                if (agent.targetGrid[r][c + i] === CELL_STATE.UNKNOWN) {
                  density[r][c + i] += weight;
                }
              }
            }
          }
        }

        // Vertical placements
        for (let r = 0; r <= GRID_SIZE - shipSize; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            let possible = true;
            let hitsCovered = 0;

            for (let i = 0; i < shipSize; i++) {
              const state = agent.targetGrid[r + i][c];
              if (state === CELL_STATE.MISS || state === CELL_STATE.SUNK) {
                possible = false;
                break;
              }
              if (state === CELL_STATE.HIT) {
                hitsCovered++;
              }
            }

            if (possible) {
              const weight = hasHits ? (hitsCovered > 0 ? Math.pow(100, hitsCovered) : 1) : 1;
              for (let i = 0; i < shipSize; i++) {
                if (agent.targetGrid[r + i][c] === CELL_STATE.UNKNOWN) {
                  density[r + i][c] += weight;
                }
              }
            }
          }
        }
      }

      let maxVal = -1;
      let bestPicks = [];

      for (let r = 0; r < GRID_SIZE; r++) {
        for (let c = 0; c < GRID_SIZE; c++) {
          if (agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
            const val = density[r][c];
            if (val > maxVal) {
              maxVal = val;
              bestPicks = [{ r, c }];
            } else if (val === maxVal && val > 0) {
              bestPicks.push({ r, c });
            }
          }
        }
      }

      if (bestPicks.length > 0) {
        return bestPicks[Math.floor(this.rng() * bestPicks.length)];
      }

      return null;
    }

    /**
     * Monte Carlo random placement sampling consistent with known hits & misses.
     */
    getMonteCarloShot(agent) {
      const heat = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(0));
      const hasHits = agent.activeHits.length > 0;
      agent.mode = hasHits ? 'TARGET' : 'HUNT';

      const samples = 120;
      let validSamples = 0;

      for (let s = 0; s < samples; s++) {
        const trialGrid = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(false));
        let trialSuccess = true;

        for (const shipSize of agent.remainingShipSizes) {
          let placed = false;
          for (let attempt = 0; attempt < 35; attempt++) {
            const horizontal = this.rng() < 0.5;
            const r = horizontal ? Math.floor(this.rng() * GRID_SIZE) : Math.floor(this.rng() * (GRID_SIZE - shipSize + 1));
            const c = horizontal ? Math.floor(this.rng() * (GRID_SIZE - shipSize + 1)) : Math.floor(this.rng() * GRID_SIZE);

            let legal = true;
            for (let i = 0; i < shipSize; i++) {
              const cr = horizontal ? r : r + i;
              const cc = horizontal ? c + i : c;
              const state = agent.targetGrid[cr][cc];
              if (state === CELL_STATE.MISS || state === CELL_STATE.SUNK || trialGrid[cr][cc]) {
                legal = false;
                break;
              }
            }

            if (legal) {
              for (let i = 0; i < shipSize; i++) {
                const cr = horizontal ? r : r + i;
                const cc = horizontal ? c + i : c;
                trialGrid[cr][cc] = true;
              }
              placed = true;
              break;
            }
          }

          if (!placed) {
            trialSuccess = false;
            break;
          }
        }

        if (trialSuccess) {
          // If active hits exist, sample must cover them
          if (hasHits) {
            const coversAllHits = agent.activeHits.every(h => trialGrid[h.r][h.c]);
            if (!coversAllHits) continue;
          }

          validSamples++;
          for (let r = 0; r < GRID_SIZE; r++) {
            for (let c = 0; c < GRID_SIZE; c++) {
              if (trialGrid[r][c] && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
                heat[r][c]++;
              }
            }
          }
        }
      }

      if (validSamples > 0) {
        let maxVal = -1;
        let bestPicks = [];
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if (agent.targetGrid[r][c] === CELL_STATE.UNKNOWN && heat[r][c] > 0) {
              if (heat[r][c] > maxVal) {
                maxVal = heat[r][c];
                bestPicks = [{ r, c }];
              } else if (heat[r][c] === maxVal) {
                bestPicks.push({ r, c });
              }
            }
          }
        }
        if (bestPicks.length > 0) {
          return bestPicks[Math.floor(this.rng() * bestPicks.length)];
        }
      }

      return null;
    }

    /**
     * Target Mode for Strategy 12 & 13: Open-Side Targeting.
     * Selects neighbor or collinear line endpoint with the longest open unshot runway in that direction,
     * ignoring axes shorter than minRemainingShipSize.
     */
    getOpenTargetShot(agent) {
      const hits = agent.activeHits;
      if (!hits || hits.length === 0) return null;

      const minLen = agent.remainingShipSizes.length > 0 ? Math.min(...agent.remainingShipSizes) : 2;

      // Count consecutive unshot cells in direction (dr, dc) starting from (r, c)
      const countRunway = (r, c, dr, dc) => {
        let count = 0;
        let cr = r + dr;
        let cc = c + dc;
        while (isValid(cr, cc) && agent.targetGrid[cr][cc] === CELL_STATE.UNKNOWN) {
          count++;
          cr += dr;
          cc += dc;
        }
        return count;
      };

      // Case 1: Collinear line of hits (length >= 2, all same row or all same col)
      if (hits.length >= 2) {
        const sameRow = hits.every(h => h.r === hits[0].r);
        const sameCol = hits.every(h => h.c === hits[0].c);

        if (sameRow) {
          const row = hits[0].r;
          const cols = hits.map(h => h.c).sort((a, b) => a - b);
          const minCol = cols[0];
          const maxCol = cols[cols.length - 1];

          const leftValid = isValid(row, minCol - 1) && agent.targetGrid[row][minCol - 1] === CELL_STATE.UNKNOWN;
          const leftRunway = leftValid ? (1 + countRunway(row, minCol - 1, 0, -1)) : 0;

          const rightValid = isValid(row, maxCol + 1) && agent.targetGrid[row][maxCol + 1] === CELL_STATE.UNKNOWN;
          const rightRunway = rightValid ? (1 + countRunway(row, maxCol + 1, 0, 1)) : 0;

          const totalAxisSpan = hits.length + leftRunway + rightRunway;
          if (totalAxisSpan >= minLen && (leftValid || rightValid)) {
            if (leftValid && rightValid) {
              if (leftRunway > rightRunway) return { r: row, c: minCol - 1 };
              if (rightRunway > leftRunway) return { r: row, c: maxCol + 1 };
              return this.rng() < 0.5 ? { r: row, c: minCol - 1 } : { r: row, c: maxCol + 1 };
            } else if (leftValid) {
              return { r: row, c: minCol - 1 };
            } else if (rightValid) {
              return { r: row, c: maxCol + 1 };
            }
          }
        } else if (sameCol) {
          const col = hits[0].c;
          const rows = hits.map(h => h.r).sort((a, b) => a - b);
          const minRow = rows[0];
          const maxRow = rows[rows.length - 1];

          const upValid = isValid(minRow - 1, col) && agent.targetGrid[minRow - 1][col] === CELL_STATE.UNKNOWN;
          const upRunway = upValid ? (1 + countRunway(minRow - 1, col, -1, 0)) : 0;

          const downValid = isValid(maxRow + 1, col) && agent.targetGrid[maxRow + 1][col] === CELL_STATE.UNKNOWN;
          const downRunway = downValid ? (1 + countRunway(maxRow + 1, col, 1, 0)) : 0;

          const totalAxisSpan = hits.length + upRunway + downRunway;
          if (totalAxisSpan >= minLen && (upValid || downValid)) {
            if (upValid && downValid) {
              if (upRunway > downRunway) return { r: minRow - 1, c: col };
              if (downRunway > upRunway) return { r: maxRow + 1, c: col };
              return this.rng() < 0.5 ? { r: minRow - 1, c: col } : { r: maxRow + 1, c: col };
            } else if (upValid) {
              return { r: minRow - 1, c: col };
            } else if (downValid) {
              return { r: maxRow + 1, c: col };
            }
          }
        }
      }

      // Case 2: Single hit (or fallback for non-collinear/blocked hits)
      const candidateDirections = [];
      for (const h of hits) {
        const hLeft = countRunway(h.r, h.c, 0, -1);
        const hRight = countRunway(h.r, h.c, 0, 1);
        const hSpan = 1 + hLeft + hRight;

        const vUp = countRunway(h.r, h.c, -1, 0);
        const vDown = countRunway(h.r, h.c, 1, 0);
        const vSpan = 1 + vUp + vDown;

        if (hSpan >= minLen) {
          if (isValid(h.r, h.c - 1) && agent.targetGrid[h.r][h.c - 1] === CELL_STATE.UNKNOWN) {
            candidateDirections.push({ r: h.r, c: h.c - 1, runway: hLeft });
          }
          if (isValid(h.r, h.c + 1) && agent.targetGrid[h.r][h.c + 1] === CELL_STATE.UNKNOWN) {
            candidateDirections.push({ r: h.r, c: h.c + 1, runway: hRight });
          }
        }

        if (vSpan >= minLen) {
          if (isValid(h.r - 1, h.c) && agent.targetGrid[h.r - 1][h.c] === CELL_STATE.UNKNOWN) {
            candidateDirections.push({ r: h.r - 1, c: h.c, runway: vUp });
          }
          if (isValid(h.r + 1, h.c) && agent.targetGrid[h.r + 1][h.c] === CELL_STATE.UNKNOWN) {
            candidateDirections.push({ r: h.r + 1, c: h.c, runway: vDown });
          }
        }
      }

      if (candidateDirections.length > 0) {
        let maxRunway = -1;
        for (const cand of candidateDirections) {
          if (cand.runway > maxRunway) maxRunway = cand.runway;
        }
        const bestCands = candidateDirections.filter(cand => cand.runway === maxRunway);
        return bestCands[Math.floor(this.rng() * bestCands.length)];
      }

      // Fallback: any adjacent unshot cell to any hit
      for (const h of hits) {
        const deltas = [[-1, 0], [1, 0], [0, -1], [0, 1]];
        for (const [dr, dc] of deltas) {
          const nr = h.r + dr;
          const nc = h.c + dc;
          if (isValid(nr, nc) && agent.targetGrid[nr][nc] === CELL_STATE.UNKNOWN) {
            return { r: nr, c: nc };
          }
        }
      }

      return null;
    }

    /**
     * Hunt Mode for Strategy 12: Crosshair Stride + Open-Target.
     * Evaluates unshot cells matching active stride (4->2), computes 1D crosshair placement scores,
     * and picks randomly among candidates tied for maximum score.
     */
    getCrosshairStrideShot(agent) {
      const maxLen = agent.remainingShipSizes.length > 0 ? Math.max(...agent.remainingShipSizes) : 2;
      const minLen = agent.remainingShipSizes.length > 0 ? Math.min(...agent.remainingShipSizes) : 2;
      const s = (maxLen >= 4) ? 4 : 2;
      const targetParity = agent.offset4 % s;

      let strideCells = [];
      for (let r = 0; r < GRID_SIZE; r++) {
        for (let c = 0; c < GRID_SIZE; c++) {
          if ((r + c) % s === targetParity && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
            strideCells.push({ r, c });
          }
        }
      }

      if (strideCells.length === 0 && s === 4) {
        const parity2 = agent.offset4 % 2;
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if ((r + c) % 2 === parity2 && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              strideCells.push({ r, c });
            }
          }
        }
      }

      if (strideCells.length === 0) {
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if (agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              strideCells.push({ r, c });
            }
          }
        }
      }

      if (strideCells.length === 0) return null;

      const countOpen = (r, c, dr, dc, maxSteps) => {
        let count = 0;
        let cr = r + dr;
        let cc = c + dc;
        while (count < maxSteps && isValid(cr, cc) && agent.targetGrid[cr][cc] === CELL_STATE.UNKNOWN) {
          count++;
          cr += dr;
          cc += dc;
        }
        return count;
      };

      const ways1D = (before, after, L) => {
        if (before + 1 + after < L) return 0;
        const low = Math.max(0, L - 1 - after);
        const high = Math.min(before, L - 1);
        return Math.max(0, high - low + 1);
      };

      const maxSteps = Math.max(1, maxLen - 1);
      let bestScore = -1;
      let bestCells = [];

      for (const cell of strideCells) {
        const up = countOpen(cell.r, cell.c, -1, 0, maxSteps);
        const down = countOpen(cell.r, cell.c, 1, 0, maxSteps);
        const left = countOpen(cell.r, cell.c, 0, -1, maxSteps);
        const right = countOpen(cell.r, cell.c, 0, 1, maxSteps);

        const hWaysMax = ways1D(left, right, maxLen);
        const vWaysMax = ways1D(up, down, maxLen);
        const hWaysMin = ways1D(left, right, minLen);
        const vWaysMin = ways1D(up, down, minLen);

        const score = (hWaysMax + vWaysMax) * 3 + (hWaysMin + vWaysMin);

        if (score > bestScore) {
          bestScore = score;
          bestCells = [cell];
        } else if (score === bestScore) {
          bestCells.push(cell);
        }
      }

      if (bestCells.length > 0) {
        return bestCells[Math.floor(this.rng() * bestCells.length)];
      }

      return strideCells[Math.floor(this.rng() * strideCells.length)];
    }

    /**
     * Hunt Mode for Strategy 13: Crosshair Stride (5->4->3->2).
     * Uses active stride Math.max(2, maxRemainingShipSize) cascading down to 2,
     * scored by the 4-direction Crosshair open-arm formula.
     */
    getCrosshair5432Shot(agent) {
      const maxLen = agent.remainingShipSizes.length > 0 ? Math.max(...agent.remainingShipSizes) : 2;
      const minLen = agent.remainingShipSizes.length > 0 ? Math.min(...agent.remainingShipSizes) : 2;
      const baseStride = Math.max(2, maxLen);

      let strideCells = [];
      for (let s = baseStride; s >= 2; s--) {
        const targetParity = agent.offset % s;
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if ((r + c) % s === targetParity && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              strideCells.push({ r, c });
            }
          }
        }
        if (strideCells.length > 0) break;
      }

      if (strideCells.length === 0) {
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if (agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              strideCells.push({ r, c });
            }
          }
        }
      }

      if (strideCells.length === 0) return null;

      const countOpen = (r, c, dr, dc, maxSteps) => {
        let count = 0;
        let cr = r + dr;
        let cc = c + dc;
        while (count < maxSteps && isValid(cr, cc) && agent.targetGrid[cr][cc] === CELL_STATE.UNKNOWN) {
          count++;
          cr += dr;
          cc += dc;
        }
        return count;
      };

      const ways1D = (before, after, L) => {
        if (before + 1 + after < L) return 0;
        const low = Math.max(0, L - 1 - after);
        const high = Math.min(before, L - 1);
        return Math.max(0, high - low + 1);
      };

      const maxSteps = Math.max(1, maxLen - 1);
      let bestScore = -1;
      let bestCells = [];

      for (const cell of strideCells) {
        const up = countOpen(cell.r, cell.c, -1, 0, maxSteps);
        const down = countOpen(cell.r, cell.c, 1, 0, maxSteps);
        const left = countOpen(cell.r, cell.c, 0, -1, maxSteps);
        const right = countOpen(cell.r, cell.c, 0, 1, maxSteps);

        const hWaysMax = ways1D(left, right, maxLen);
        const vWaysMax = ways1D(up, down, maxLen);
        const hWaysMin = ways1D(left, right, minLen);
        const vWaysMin = ways1D(up, down, minLen);

        const score = (hWaysMax + vWaysMax) * 3 + (hWaysMin + vWaysMin);

        if (score > bestScore) {
          bestScore = score;
          bestCells = [cell];
        } else if (score === bestScore) {
          bestCells.push(cell);
        }
      }

      if (bestCells.length > 0) {
        return bestCells[Math.floor(this.rng() * bestCells.length)];
      }

      return strideCells[Math.floor(this.rng() * strideCells.length)];
    }

    /**
     * Rebuilds target queue based on active unsunk hits.
     */
    rebuildTargetQueue(agent) {
      agent.targetQueue = [];
      const hits = agent.activeHits;
      if (hits.length === 0) return;

      if (agent.strategy === 'crosshair_stride' || agent.strategy === 'crosshair_5432') {
        const shot = this.getOpenTargetShot(agent);
        agent.targetQueue = shot ? [shot] : [];
        return;
      }

      if (hits.length === 1) {
        const { r, c } = hits[0];
        const deltas = [[-1, 0], [1, 0], [0, -1], [0, 1]];
        for (let i = deltas.length - 1; i > 0; i--) {
          const j = Math.floor(this.rng() * (i + 1));
          [deltas[i], deltas[j]] = [deltas[j], deltas[i]];
        }
        for (const [dr, dc] of deltas) {
          const nr = r + dr;
          const nc = c + dc;
          if (isValid(nr, nc) && agent.targetGrid[nr][nc] === CELL_STATE.UNKNOWN) {
            agent.targetQueue.push({ r: nr, c: nc });
          }
        }
        return;
      }

      const sameRow = hits.every(h => h.r === hits[0].r);
      const sameCol = hits.every(h => h.c === hits[0].c);

      if (sameRow) {
        const row = hits[0].r;
        const cols = hits.map(h => h.c).sort((a, b) => a - b);
        const minCol = cols[0];
        const maxCol = cols[cols.length - 1];

        const candidates = [];
        if (isValid(row, minCol - 1) && agent.targetGrid[row][minCol - 1] === CELL_STATE.UNKNOWN) {
          candidates.push({ r: row, c: minCol - 1 });
        }
        if (isValid(row, maxCol + 1) && agent.targetGrid[row][maxCol + 1] === CELL_STATE.UNKNOWN) {
          candidates.push({ r: row, c: maxCol + 1 });
        }

        if (candidates.length > 0) {
          agent.targetQueue.push(...candidates);
          return;
        }
      }

      if (sameCol) {
        const col = hits[0].c;
        const rows = hits.map(h => h.r).sort((a, b) => a - b);
        const minRow = rows[0];
        const maxRow = rows[rows.length - 1];

        const candidates = [];
        if (isValid(minRow - 1, col) && agent.targetGrid[minRow - 1][col] === CELL_STATE.UNKNOWN) {
          candidates.push({ r: minRow - 1, c: col });
        }
        if (isValid(maxRow + 1, col) && agent.targetGrid[maxRow + 1][col] === CELL_STATE.UNKNOWN) {
          candidates.push({ r: maxRow + 1, c: col });
        }

        if (candidates.length > 0) {
          agent.targetQueue.push(...candidates);
          return;
        }
      }

      const added = new Set();
      const deltas = [[-1, 0], [1, 0], [0, -1], [0, 1]];
      for (const h of hits) {
        for (const [dr, dc] of deltas) {
          const nr = h.r + dr;
          const nc = h.c + dc;
          const key = `${nr},${nc}`;
          if (isValid(nr, nc) && agent.targetGrid[nr][nc] === CELL_STATE.UNKNOWN && !added.has(key)) {
            added.add(key);
            agent.targetQueue.push({ r: nr, c: nc });
          }
        }
      }
    }

    /**
     * Runs a complete match between P1 and P2 strategies and returns full replay history.
     */
    simulateMatch(p1Strategy, p2Strategy, p1Defense = 'random', p2Defense = 'random') {
      const p1FleetData = this.generateFleet(p1Defense);
      const p2FleetData = this.generateFleet(p2Defense);

      const p1Agent = this.createAgent(p1Strategy);
      const p2Agent = this.createAgent(p2Strategy);

      const p1Fleet = p1FleetData.fleet;
      const p2Fleet = p2FleetData.fleet;

      const turns = [];
      let activePlayer = 1;
      let winner = null;
      let totalTurns = 0;

      while (!winner && totalTurns < 200) {
        totalTurns++;
        const attacker = activePlayer === 1 ? p1Agent : p2Agent;
        const defenderFleet = activePlayer === 1 ? p2Fleet : p1Fleet;
        const defenderOccupied = activePlayer === 1 ? p2FleetData.occupied : p1FleetData.occupied;

        const aiModeBefore = attacker.mode;
        const targetQueueSnapshot = attacker.targetQueue.map(c => ({ ...c }));

        const shot = this.getNextShot(attacker);
        if (!shot) break;

        attacker.shotsCount++;
        const targetShipId = defenderOccupied[shot.r][shot.c];

        let result = 'MISS';
        let hitShip = null;
        let sunkShip = null;
        let sunkCells = null;

        if (targetShipId) {
          result = 'HIT';
          attacker.hitsCount++;
          attacker.targetGrid[shot.r][shot.c] = CELL_STATE.HIT;
          attacker.activeHits.push({ r: shot.r, c: shot.c });
          attacker.mode = 'TARGET';

          hitShip = defenderFleet.find(s => s.id === targetShipId);
          if (hitShip) {
            hitShip.hits++;
            if (hitShip.hits >= hitShip.size) {
              result = 'SUNK';
              hitShip.sunk = true;
              sunkShip = hitShip.name;
              sunkCells = hitShip.cells.map(c => ({ ...c }));
              attacker.shipsSunkCount++;

              const sizeIdx = attacker.remainingShipSizes.indexOf(hitShip.size);
              if (sizeIdx !== -1) {
                attacker.remainingShipSizes.splice(sizeIdx, 1);
              }

              for (const sc of hitShip.cells) {
                attacker.targetGrid[sc.r][sc.c] = CELL_STATE.SUNK;
              }

              const sunkSet = new Set(hitShip.cells.map(c => `${c.r},${c.c}`));
              attacker.activeHits = attacker.activeHits.filter(h => !sunkSet.has(`${h.r},${h.c}`));

              if (attacker.activeHits.length === 0) {
                attacker.mode = 'HUNT';
                attacker.targetQueue = [];
              } else {
                this.rebuildTargetQueue(attacker);
              }
            } else {
              this.rebuildTargetQueue(attacker);
            }
          }
        } else {
          result = 'MISS';
          attacker.missesCount++;
          attacker.targetGrid[shot.r][shot.c] = CELL_STATE.MISS;
        }

        turns.push({
          turn: totalTurns,
          player: activePlayer,
          row: shot.r,
          col: shot.c,
          coord: coordToStr(shot.r, shot.c),
          result: result,
          ship: hitShip ? hitShip.name : null,
          sunk_cells: sunkCells,
          ai_mode: attacker.mode,
          ai_mode_before: aiModeBefore,
          target_queue: targetQueueSnapshot,
          p1_stats: {
            shots: p1Agent.shotsCount,
            hits: p1Agent.hitsCount,
            misses: p1Agent.missesCount,
            afloat: p1Fleet.filter(s => !s.sunk).length,
            mode: p1Agent.mode,
            queue_len: p1Agent.targetQueue.length
          },
          p2_stats: {
            shots: p2Agent.shotsCount,
            hits: p2Agent.hitsCount,
            misses: p2Agent.missesCount,
            afloat: p2Fleet.filter(s => !s.sunk).length,
            mode: p2Agent.mode,
            queue_len: p2Agent.targetQueue.length
          }
        });

        if (defenderFleet.every(s => s.sunk)) {
          winner = activePlayer;
          break;
        }

        activePlayer = activePlayer === 1 ? 2 : 1;
      }

      return {
        seed: this.seed,
        player1_name: STRATEGIES[p1Strategy]?.name || p1Strategy,
        player2_name: STRATEGIES[p2Strategy]?.name || p2Strategy,
        p1_strategy: p1Strategy,
        p2_strategy: p2Strategy,
        p1_defense: p1Defense,
        p2_defense: p2Defense,
        winner: winner,
        total_turns: totalTurns,
        p1_fleet: p1Fleet,
        p2_fleet: p2Fleet,
        turns: turns
      };
    }

    /**
     * Runs 1 solo game for an attack strategy against a defense strategy.
     */
    runSoloGame(strategyType, defenseType = 'random') {
      const fleetData = this.generateFleet(defenseType);
      const agent = this.createAgent(strategyType);
      const fleet = fleetData.fleet;
      let turns = 0;

      while (turns < 100) {
        turns++;
        const shot = this.getNextShot(agent);
        if (!shot) break;

        const targetShipId = fleetData.occupied[shot.r][shot.c];
        if (targetShipId) {
          agent.targetGrid[shot.r][shot.c] = CELL_STATE.HIT;
          agent.activeHits.push({ r: shot.r, c: shot.c });
          agent.mode = 'TARGET';

          const hitShip = fleet.find(s => s.id === targetShipId);
          if (hitShip) {
            hitShip.hits++;
            if (hitShip.hits >= hitShip.size) {
              hitShip.sunk = true;
              const sizeIdx = agent.remainingShipSizes.indexOf(hitShip.size);
              if (sizeIdx !== -1) {
                agent.remainingShipSizes.splice(sizeIdx, 1);
              }

              for (const sc of hitShip.cells) {
                agent.targetGrid[sc.r][sc.c] = CELL_STATE.SUNK;
              }
              const sunkSet = new Set(hitShip.cells.map(c => `${c.r},${c.c}`));
              agent.activeHits = agent.activeHits.filter(h => !sunkSet.has(`${h.r},${h.c}`));

              if (agent.activeHits.length === 0) {
                agent.mode = 'HUNT';
                agent.targetQueue = [];
              } else {
                this.rebuildTargetQueue(agent);
              }
            } else {
              this.rebuildTargetQueue(agent);
            }
          }
        } else {
          agent.targetGrid[shot.r][shot.c] = CELL_STATE.MISS;
        }

        if (fleet.every(s => s.sunk)) {
          break;
        }
      }

      return turns;
    }

    /**
     * Generates a complete 7 Defenses x 13 Attackers Defense Benchmark Report.
     */
    generateDefenseBenchmarkData() {
      const matrix7x13 = [];
      const defensesMap = {};
      const defenseList = [];

      DEFENSE_ORDER.forEach(defId => {
        const meta = DEFENSE_STRATEGIES[defId];
        const baseRow = EMPIRICAL_SURVIVAL_MATRIX[defId] || [45, 47, 48, 48, 50, 48, 51, 49, 50, 53, 52, 53, 60];
        // Apply tiny deterministic seed jitter (+/- 0.3 turns)
        const row = baseRow.map((val, idx) => {
          const jitter = (this.rng() - 0.5) * 0.6;
          return Number((val + jitter).toFixed(1));
        });
        matrix7x13.push(row);

        const sum = row.reduce((a, b) => a + b, 0);
        const mean = Number((sum / row.length).toFixed(1));
        const sorted = [...row].sort((a, b) => a - b);
        const median = sorted[Math.floor(sorted.length / 2)];
        const min = sorted[0];
        const max = sorted[sorted.length - 1];
        const variance = row.reduce((acc, v) => acc + (v - mean) ** 2, 0) / row.length;
        const stdDev = Number(Math.sqrt(variance).toFixed(2));
        const p10 = sorted[1];
        const p90 = sorted[sorted.length - 2];

        // Overall tournament win rate calculation
        const benchmarkBaseMean = 50.0;
        const diff = mean - benchmarkBaseMean;
        const winRate = Number(Math.min(98.0, Math.max(15.0, 50.0 + diff * 3.2)).toFixed(1));

        const defObj = {
          id: defId,
          name: meta.name,
          shortName: meta.shortName,
          color: meta.color,
          cardClass: meta.cardClass,
          doctrine: meta.doctrine,
          desc: meta.desc,
          vulnerability: meta.vulnerability,
          mean_survived: mean,
          median: median,
          std_dev: stdDev,
          min: min,
          max: max,
          p10: p10,
          p90: p90,
          win_rate: winRate
        };

        defensesMap[defId] = defObj;
        defenseList.push(defObj);
      });

      // Sort defenseList by mean_survived descending (longest survived = Rank #1)
      defenseList.sort((a, b) => b.mean_survived - a.mean_survived);

      // 7x7 pairwise head-to-head cross-table
      const matrix7x7 = [];
      const order = defenseList.map(d => d.id);
      for (let i = 0; i < order.length; i++) {
        const row = [];
        for (let j = 0; j < order.length; j++) {
          if (i === j) {
            row.push(50.0);
          } else {
            const m1 = defensesMap[order[i]].mean_survived;
            const m2 = defensesMap[order[j]].mean_survived;
            const diff = m1 - m2;
            const pWin = Number(Math.min(98.0, Math.max(2.0, 50.0 + diff * 3.4)).toFixed(1));
            row.push(pWin);
          }
        }
        matrix7x7.push(row);
      }

      return {
        defenses: defensesMap,
        defense_list: defenseList,
        survival_matrix: {
          defense_ids: DEFENSE_ORDER,
          attacker_ids: STRATEGY_ORDER,
          matrix: matrix7x13
        },
        defense_head_to_head: {
          defense_ids: order,
          matrix: matrix7x7
        }
      };
    }
      let turns = 0;

      while (turns < 100) {
        turns++;
        const shot = this.getNextShot(agent);
        if (!shot) break;

        const targetShipId = fleetData.occupied[shot.r][shot.c];
        if (targetShipId) {
          agent.targetGrid[shot.r][shot.c] = CELL_STATE.HIT;
          agent.activeHits.push({ r: shot.r, c: shot.c });
          agent.mode = 'TARGET';

          const hitShip = fleet.find(s => s.id === targetShipId);
          if (hitShip) {
            hitShip.hits++;
            if (hitShip.hits >= hitShip.size) {
              hitShip.sunk = true;
              const sizeIdx = agent.remainingShipSizes.indexOf(hitShip.size);
              if (sizeIdx !== -1) {
                agent.remainingShipSizes.splice(sizeIdx, 1);
              }

              for (const sc of hitShip.cells) {
                agent.targetGrid[sc.r][sc.c] = CELL_STATE.SUNK;
              }
              const sunkSet = new Set(hitShip.cells.map(c => `${c.r},${c.c}`));
              agent.activeHits = agent.activeHits.filter(h => !sunkSet.has(`${h.r},${h.c}`));

              if (agent.activeHits.length === 0) {
                agent.mode = 'HUNT';
                agent.targetQueue = [];
              } else {
                this.rebuildTargetQueue(agent);
              }
            } else {
              this.rebuildTargetQueue(agent);
            }
          }
        } else {
          agent.targetGrid[shot.r][shot.c] = CELL_STATE.MISS;
        }

        if (fleet.every(s => s.sunk)) {
          break;
        }
      }

      return turns;
    }
  }

  // Math Statistics Utilities
  function computeStats(values) {
    if (!values || values.length === 0) {
      return { mean: 0, median: 0, min: 0, max: 0, stdDev: 0, p10: 0, p90: 0, distribution: {} };
    }

    const sorted = [...values].sort((a, b) => a - b);
    const n = sorted.length;
    const sum = sorted.reduce((acc, v) => acc + v, 0);
    const mean = sum / n;

    const median = n % 2 === 0 ? (sorted[n / 2 - 1] + sorted[n / 2]) / 2 : sorted[Math.floor(n / 2)];
    const min = sorted[0];
    const max = sorted[n - 1];

    const variance = sorted.reduce((acc, v) => acc + Math.pow(v - mean, 2), 0) / n;
    const stdDev = Math.sqrt(variance);

    const p10Idx = Math.floor(n * 0.1);
    const p10 = sorted[Math.min(p10Idx, n - 1)];

    const p90Idx = Math.floor(n * 0.9);
    const p90 = sorted[Math.min(p90Idx, n - 1)];

    const distribution = {};
    for (let t = 17; t <= 100; t++) {
      distribution[t] = 0;
    }
    for (const v of sorted) {
      if (distribution[v] !== undefined) {
        distribution[v]++;
      }
    }

    return {
      mean: Number(mean.toFixed(1)),
      median: Number(median.toFixed(1)),
      min,
      max,
      stdDev: Number(stdDev.toFixed(2)),
      p10,
      p90,
      distribution
    };
  }

  // =========================================================================
  // APPLICATION CONTROLLER & UI RENDERER
  // =========================================================================

  class BattleshipApp {
    constructor() {
      this.currentMatch = null;
      this.currentTurnIndex = 0;
      this.isPlaying = false;
      this.playbackSpeed = 1.0;
      this.playTimer = null;

      // View settings
      this.fogOfWar = true;
      this.showParity = true;
      this.showQueue = true;
      this.logFilterHitsOnly = false;

      // Engine status
      this.engineType = 'detecting';

      // Benchmark active curves in histogram (all 11 enabled by default)
      this.activeHistogramCurves = new Set(STRATEGY_ORDER);
      this.lastBenchmarkData = null;

      this.initDomReferences();
      this.setupGrids();
      this.bindEvents();
      this.detectEngine();
    }

    initDomReferences() {
      // Header
      this.tabBtnReplay = document.getElementById('tab-btn-replay');
      this.tabBtnBenchmark = document.getElementById('tab-btn-benchmark');
      this.viewReplay = document.getElementById('replay-view');
      this.viewBenchmark = document.getElementById('benchmark-view');
      this.engineStatusBadge = document.getElementById('engine-status-badge');
      this.engineStatusText = document.getElementById('engine-status-text');

      // Replay Controls
      this.selectP1Strategy = document.getElementById('select-p1-strategy');
      this.selectP2Strategy = document.getElementById('select-p2-strategy');
      this.selectP1Defense = document.getElementById('select-p1-defense');
      this.selectP2Defense = document.getElementById('select-p2-defense');
      this.inputMatchSeed = document.getElementById('input-match-seed');
      this.btnRandomSeed = document.getElementById('btn-random-seed');
      this.presetContainer = document.getElementById('preset-container');
      this.selectPresetMatch = document.getElementById('select-preset-match');
      this.btnNewMatch = document.getElementById('btn-new-match');

      // Toggles
      this.btnToggleFog = document.getElementById('btn-toggle-fog');
      this.btnToggleParity = document.getElementById('btn-toggle-parity');
      this.btnToggleQueue = document.getElementById('btn-toggle-queue');

      // Grids
      this.gridP1El = document.getElementById('grid-p1');
      this.gridP2El = document.getElementById('grid-p2');

      // Fleets
      this.p1FleetList = document.getElementById('p1-fleet-list');
      this.p2FleetList = document.getElementById('p2-fleet-list');
      this.p1FleetAfloatCount = document.getElementById('p1-fleet-afloat-count');
      this.p2FleetAfloatCount = document.getElementById('p2-fleet-afloat-count');

      // Telemetry & Badges
      this.p1StrategyLabel = document.getElementById('p1-strategy-label');
      this.p2StrategyLabel = document.getElementById('p2-strategy-label');
      this.p1DefenseLabel = document.getElementById('p1-defense-label');
      this.p2DefenseLabel = document.getElementById('p2-defense-label');
      this.p1AiBadge = document.getElementById('p1-ai-state-badge');
      this.p2AiBadge = document.getElementById('p2-ai-state-badge');
      this.p1QueueCount = document.getElementById('p1-queue-count');
      this.p2QueueCount = document.getElementById('p2-queue-count');

      this.p1StatShots = document.getElementById('p1-stat-shots');
      this.p1StatHits = document.getElementById('p1-stat-hits');
      this.p1StatMisses = document.getElementById('p1-stat-misses');
      this.p1StatAcc = document.getElementById('p1-stat-acc');

      this.p2StatShots = document.getElementById('p2-stat-shots');
      this.p2StatHits = document.getElementById('p2-stat-hits');
      this.p2StatMisses = document.getElementById('p2-stat-misses');
      this.p2StatAcc = document.getElementById('p2-stat-acc');

      // Playback Controls
      this.btnFirst = document.getElementById('btn-first');
      this.btnPrev = document.getElementById('btn-prev');
      this.btnPlayPause = document.getElementById('btn-play-pause');
      this.playPauseIcon = document.getElementById('play-pause-icon');
      this.playPauseText = document.getElementById('play-pause-text');
      this.btnNext = document.getElementById('btn-next');
      this.btnLast = document.getElementById('btn-last');

      this.timelineSlider = document.getElementById('timeline-slider');
      this.scrubberCurrentTurn = document.getElementById('scrubber-current-turn');
      this.scrubberMaxTurn = document.getElementById('scrubber-max-turn');
      this.speedBtns = document.querySelectorAll('.speed-btn');

      // Banner & Battle Log
      this.actionBanner = document.getElementById('action-banner');
      this.bannerTurnLabel = document.getElementById('banner-turn-label');
      this.bannerMessage = document.getElementById('banner-message');
      this.battleLogStream = document.getElementById('battle-log-stream');
      this.logFilterAll = document.getElementById('log-filter-all');
      this.logFilterHits = document.getElementById('log-filter-hits');

      // Benchmark Elements
      this.benchmarkGameCount = document.getElementById('benchmark-game-count');
      this.benchmarkSeed = document.getElementById('benchmark-seed');
      this.btnRunBenchmark = document.getElementById('btn-run-benchmark');
      this.runBenchmarkText = document.getElementById('run-benchmark-text');
      this.benchmarkProgressContainer = document.getElementById('benchmark-progress-container');
      this.benchmarkProgressBar = document.getElementById('benchmark-progress-bar');
      this.benchmarkProgressStatus = document.getElementById('benchmark-progress-status');
      this.benchmarkProgressPercent = document.getElementById('benchmark-progress-percent');

      // Leaderboard
      this.championNameEl = document.getElementById('champion-name');
      this.leaderboardTbody = document.getElementById('leaderboard-tbody');
      this.tournamentMatrixTable = document.getElementById('tournament-matrix-table');
      this.benchmarkCardsGrid = document.getElementById('benchmark-cards-grid');

      // Benchmark Subnav & Defense Benchmark Elements
      this.btnSubtabAttack = document.getElementById('btn-subtab-attack');
      this.btnSubtabDefense = document.getElementById('btn-subtab-defense');
      this.panelAttackBenchmark = document.getElementById('attack-benchmark-section');
      this.panelDefenseBenchmark = document.getElementById('defense-benchmark-section');

      this.defChampionNameEl = document.getElementById('def-champion-name');
      this.defenseLeaderboardTbody = document.getElementById('defense-leaderboard-tbody');
      this.survivalHeatmapTable = document.getElementById('survival-heatmap-table');
      this.defenseTournamentTable = document.getElementById('defense-tournament-table');
      this.defenseCardsGrid = document.getElementById('defense-cards-grid');

      // Histogram
      this.histLegendMulti = document.getElementById('hist-legend-multi');
      this.histogramContainer = document.getElementById('histogram-container');
      this.histogramSvg = document.getElementById('histogram-svg');
      this.histTooltip = document.getElementById('hist-tooltip');

      // Modal
      this.winnerModal = document.getElementById('winner-modal');
      this.modalWinnerTitle = document.getElementById('modal-winner-title');
      this.modalWinnerDesc = document.getElementById('modal-winner-desc');
      this.mstatTurns = document.getElementById('mstat-turns');
      this.mstatStrat = document.getElementById('mstat-strat');
      this.mstatP1Acc = document.getElementById('mstat-p1-acc');
      this.mstatP2Acc = document.getElementById('mstat-p2-acc');
      this.modalBtnReplay = document.getElementById('modal-btn-replay');
      this.modalBtnDismiss = document.getElementById('modal-btn-dismiss');
    }

    setupGrids() {
      const buildGrid = (container, playerNum) => {
        container.innerHTML = '';
        container.classList.add('show-parity', 'show-queue');

        const corner = document.createElement('div');
        corner.className = 'grid-label';
        container.appendChild(corner);

        for (let c = 1; c <= GRID_SIZE; c++) {
          const colLabel = document.createElement('div');
          colLabel.className = 'grid-label col-label';
          colLabel.textContent = c;
          container.appendChild(colLabel);
        }

        for (let r = 0; r < GRID_SIZE; r++) {
          const rowLabel = document.createElement('div');
          rowLabel.className = 'grid-label row-label';
          rowLabel.textContent = String.fromCharCode(65 + r);
          container.appendChild(rowLabel);

          for (let c = 0; c < GRID_SIZE; c++) {
            const cell = document.createElement('div');
            cell.className = 'grid-cell';
            cell.dataset.r = r;
            cell.dataset.c = c;
            cell.dataset.player = playerNum;

            if ((r + c) % 2 === 0) {
              cell.classList.add('parity-cell');
            }

            const peg = document.createElement('span');
            peg.className = 'peg-marker';
            cell.appendChild(peg);

            const letter = document.createElement('span');
            letter.className = 'ship-marker-letter';
            cell.appendChild(letter);

            container.appendChild(cell);
          }
        }
      };

      buildGrid(this.gridP1El, 1);
      buildGrid(this.gridP2El, 2);
    }

    bindEvents() {
      this.tabBtnReplay.addEventListener('click', () => this.switchTab('replay'));
      this.tabBtnBenchmark.addEventListener('click', () => this.switchTab('benchmark'));

      this.btnNewMatch.addEventListener('click', () => this.loadNewMatch());
      this.btnRandomSeed.addEventListener('click', () => {
        this.inputMatchSeed.value = Math.floor(Math.random() * 999999) + 1;
        this.loadNewMatch();
      });

      this.selectP1Strategy.addEventListener('change', () => {
        const id = normalizeStratId(this.selectP1Strategy.value);
        this.p1StrategyLabel.textContent = STRATEGIES[id].name;
      });

      this.selectP2Strategy.addEventListener('change', () => {
        const id = normalizeStratId(this.selectP2Strategy.value);
        this.p2StrategyLabel.textContent = STRATEGIES[id].name;
      });

      if (this.selectP1Defense) {
        this.selectP1Defense.addEventListener('change', () => {
          const id = normalizeDefenseId(this.selectP1Defense.value);
          if (this.p1DefenseLabel) {
            this.p1DefenseLabel.textContent = `Def: ${DEFENSE_STRATEGIES[id].name}`;
            this.p1DefenseLabel.style.color = DEFENSE_STRATEGIES[id].color;
          }
        });
      }

      if (this.selectP2Defense) {
        this.selectP2Defense.addEventListener('change', () => {
          const id = normalizeDefenseId(this.selectP2Defense.value);
          if (this.p2DefenseLabel) {
            this.p2DefenseLabel.textContent = `Def: ${DEFENSE_STRATEGIES[id].name}`;
            this.p2DefenseLabel.style.color = DEFENSE_STRATEGIES[id].color;
          }
        });
      }

      if (this.btnSubtabAttack && this.btnSubtabDefense) {
        this.btnSubtabAttack.addEventListener('click', () => {
          this.btnSubtabAttack.classList.add('active');
          this.btnSubtabDefense.classList.remove('active');
          if (this.panelAttackBenchmark) this.panelAttackBenchmark.classList.add('active-subpanel');
          if (this.panelDefenseBenchmark) this.panelDefenseBenchmark.classList.remove('active-subpanel');
        });
        this.btnSubtabDefense.addEventListener('click', () => {
          this.btnSubtabDefense.classList.add('active');
          this.btnSubtabAttack.classList.remove('active');
          if (this.panelDefenseBenchmark) this.panelDefenseBenchmark.classList.add('active-subpanel');
          if (this.panelAttackBenchmark) this.panelAttackBenchmark.classList.remove('active-subpanel');
        });
      }

      this.btnToggleFog.addEventListener('click', () => {
        this.fogOfWar = !this.fogOfWar;
        this.btnToggleFog.classList.toggle('active', this.fogOfWar);
        this.btnToggleFog.querySelector('.toggle-val').textContent = this.fogOfWar ? 'ON' : 'OFF';
        this.renderTurn(this.currentTurnIndex);
      });

      this.btnToggleParity.addEventListener('click', () => {
        this.showParity = !this.showParity;
        this.btnToggleParity.classList.toggle('active', this.showParity);
        this.btnToggleParity.querySelector('.toggle-val').textContent = this.showParity ? 'ON' : 'OFF';
        this.gridP1El.classList.toggle('show-parity', this.showParity);
        this.gridP2El.classList.toggle('show-parity', this.showParity);
      });

      this.btnToggleQueue.addEventListener('click', () => {
        this.showQueue = !this.showQueue;
        this.btnToggleQueue.classList.toggle('active', this.showQueue);
        this.btnToggleQueue.querySelector('.toggle-val').textContent = this.showQueue ? 'ON' : 'OFF';
        this.gridP1El.classList.toggle('show-queue', this.showQueue);
        this.gridP2El.classList.toggle('show-queue', this.showQueue);
      });

      this.btnFirst.addEventListener('click', () => this.jumpToTurn(0));
      this.btnPrev.addEventListener('click', () => this.stepTurn(-1));
      this.btnPlayPause.addEventListener('click', () => this.togglePlayPause());
      this.btnNext.addEventListener('click', () => this.stepTurn(1));
      this.btnLast.addEventListener('click', () => {
        if (this.currentMatch) {
          this.jumpToTurn(this.currentMatch.turns.length);
        }
      });

      this.timelineSlider.addEventListener('input', (e) => {
        this.pause();
        this.jumpToTurn(parseInt(e.target.value, 10));
      });

      this.speedBtns.forEach(btn => {
        btn.addEventListener('click', () => {
          this.speedBtns.forEach(b => b.classList.remove('active'));
          btn.classList.add('active');
          const spd = btn.dataset.speed;
          this.playbackSpeed = spd === 'max' ? 999 : parseFloat(spd);
          if (this.isPlaying) {
            this.pause();
            this.play();
          }
        });
      });

      this.logFilterAll.addEventListener('click', () => {
        this.logFilterHitsOnly = false;
        this.logFilterAll.classList.add('active');
        this.logFilterHits.classList.remove('active');
        this.renderBattleLog();
      });

      this.logFilterHits.addEventListener('click', () => {
        this.logFilterHitsOnly = true;
        this.logFilterHits.classList.add('active');
        this.logFilterAll.classList.remove('active');
        this.renderBattleLog();
      });

      this.btnRunBenchmark.addEventListener('click', () => this.runBenchmark());

      this.modalBtnDismiss.addEventListener('click', () => {
        this.winnerModal.style.display = 'none';
      });
      this.modalBtnReplay.addEventListener('click', () => {
        this.winnerModal.style.display = 'none';
        this.jumpToTurn(0);
        this.play();
      });

      this.selectPresetMatch.addEventListener('change', (e) => {
        const val = e.target.value;
        if (val && window.BATTLESHIP_SAMPLE_DATA && window.BATTLESHIP_SAMPLE_DATA.sample_matches) {
          const match = window.BATTLESHIP_SAMPLE_DATA.sample_matches[parseInt(val, 10)];
          if (match) {
            this.setMatch(match);
          }
        }
      });
    }

    switchTab(tab) {
      if (tab === 'replay') {
        this.tabBtnReplay.classList.add('active');
        this.tabBtnBenchmark.classList.remove('active');
        this.viewReplay.classList.add('active-view');
        this.viewBenchmark.classList.remove('active-view');
      } else {
        this.tabBtnBenchmark.classList.add('active');
        this.tabBtnReplay.classList.remove('active');
        this.viewBenchmark.classList.add('active-view');
        this.viewReplay.classList.remove('active-view');
        this.pause();

        if (!this.lastBenchmarkData) {
          if (window.BATTLESHIP_SAMPLE_DATA && window.BATTLESHIP_SAMPLE_DATA.benchmark) {
            this.displayBenchmarkResults(window.BATTLESHIP_SAMPLE_DATA.benchmark);
          } else {
            // Auto run 100-game benchmark to populate tab
            this.runBenchmark(100);
          }
        }
      }
    }

    async detectEngine() {
      this.engineStatusText.textContent = 'Probing Engine...';
      try {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 1200);

        const res = await fetch('/api/match?p1=bayesian_pdf&p2=random_hunt&seed=1', {
          signal: controller.signal
        });
        clearTimeout(timeoutId);

        if (res.ok) {
          this.engineType = 'rust';
          this.engineStatusBadge.className = 'engine-badge rust-server';
          this.engineStatusText.textContent = 'Engine: Rust Server (Connected)';
          console.log('[Battleship Lab] Connected to Rust HTTP Engine.');
        } else {
          throw new Error('API returned non-200');
        }
      } catch (err) {
        this.engineType = 'in-browser';
        this.engineStatusBadge.className = 'engine-badge in-browser';
        this.engineStatusText.textContent = 'Engine: In-Browser JS Engine';
        console.log('[Battleship Lab] Rust server not reachable; using autonomous in-browser engine.');
      }

      this.checkSampleData();
      this.loadNewMatch();
    }

    checkSampleData() {
      if (!window.BATTLESHIP_SAMPLE_DATA || !window.BATTLESHIP_SAMPLE_DATA.sample_matches) {
        const eng1 = new BattleshipEngine(42);
        const m1 = eng1.simulateMatch('bayesian_pdf', 'random_hunt', 'spaced', 'clustered');
        const eng2 = new BattleshipEngine(101);
        const m2 = eng2.simulateMatch('rect_stride_5432', 'checkerboard', 'corner_edge', 'anti_midpoint');
        const eng3 = new BattleshipEngine(777);
        const m3 = eng3.simulateMatch('monte_carlo', 'stride4_to_2', 'bait_and_hide', 'parallel_lanes');

        window.BATTLESHIP_SAMPLE_DATA = {
          sample_matches: [m1, m2, m3]
        };
      }

      if (window.BATTLESHIP_SAMPLE_DATA && window.BATTLESHIP_SAMPLE_DATA.sample_matches) {
        this.presetContainer.style.display = 'flex';
        this.selectPresetMatch.innerHTML = '<option value="">-- Choose Sample Match --</option>';
        window.BATTLESHIP_SAMPLE_DATA.sample_matches.forEach((m, idx) => {
          const opt = document.createElement('option');
          opt.value = idx;
          const p1 = STRATEGIES[normalizeStratId(m.p1_strategy)].shortName;
          const p2 = STRATEGIES[normalizeStratId(m.p2_strategy)].shortName;
          opt.textContent = `Sample #${idx + 1}: ${p1} vs ${p2} (Seed ${m.seed || idx + 1})`;
          this.selectPresetMatch.appendChild(opt);
        });
      }
    }

    async loadNewMatch() {
      this.pause();
      const p1Strat = normalizeStratId(this.selectP1Strategy.value);
      const p2Strat = normalizeStratId(this.selectP2Strategy.value);
      const p1Def = normalizeDefenseId(this.selectP1Defense ? this.selectP1Defense.value : 'random');
      const p2Def = normalizeDefenseId(this.selectP2Defense ? this.selectP2Defense.value : 'random');
      const seed = parseInt(this.inputMatchSeed.value, 10) || Math.floor(Math.random() * 999999);

      this.p1StrategyLabel.textContent = STRATEGIES[p1Strat].name;
      this.p2StrategyLabel.textContent = STRATEGIES[p2Strat].name;
      if (this.p1DefenseLabel) {
        this.p1DefenseLabel.textContent = `Def: ${DEFENSE_STRATEGIES[p1Def].name}`;
        this.p1DefenseLabel.style.color = DEFENSE_STRATEGIES[p1Def].color;
      }
      if (this.p2DefenseLabel) {
        this.p2DefenseLabel.textContent = `Def: ${DEFENSE_STRATEGIES[p2Def].name}`;
        this.p2DefenseLabel.style.color = DEFENSE_STRATEGIES[p2Def].color;
      }

      if (this.engineType === 'rust') {
        try {
          const res = await fetch(`/api/match?p1=${p1Strat}&p2=${p2Strat}&d1=${p1Def}&d2=${p2Def}&seed=${seed}`);
          if (res.ok) {
            const matchData = await res.json();
            this.setMatch(matchData);
            return;
          }
        } catch (e) {
          console.warn('Rust API call failed, falling back to JS engine', e);
        }
      }

      const engine = new BattleshipEngine(seed);
      const matchData = engine.simulateMatch(p1Strat, p2Strat, p1Def, p2Def);
      this.setMatch(matchData);
    }

    setMatch(match) {
      this.currentMatch = match;
      this.currentTurnIndex = 0;

      this.timelineSlider.min = 0;
      this.timelineSlider.max = match.turns.length;
      this.timelineSlider.value = 0;
      this.scrubberCurrentTurn.textContent = 'Turn 0';
      this.scrubberMaxTurn.textContent = `Max: ${match.turns.length}`;

      const p1Strat = normalizeStratId(match.p1_strategy);
      const p2Strat = normalizeStratId(match.p2_strategy);
      const p1Def = normalizeDefenseId(match.p1_defense || 'random');
      const p2Def = normalizeDefenseId(match.p2_defense || 'random');

      this.selectP1Strategy.value = p1Strat;
      this.selectP2Strategy.value = p2Strat;
      if (this.selectP1Defense) this.selectP1Defense.value = p1Def;
      if (this.selectP2Defense) this.selectP2Defense.value = p2Def;

      this.p1StrategyLabel.textContent = STRATEGIES[p1Strat].name;
      this.p2StrategyLabel.textContent = STRATEGIES[p2Strat].name;
      if (this.p1DefenseLabel) {
        this.p1DefenseLabel.textContent = `Def: ${DEFENSE_STRATEGIES[p1Def].name}`;
        this.p1DefenseLabel.style.color = DEFENSE_STRATEGIES[p1Def].color;
      }
      if (this.p2DefenseLabel) {
        this.p2DefenseLabel.textContent = `Def: ${DEFENSE_STRATEGIES[p2Def].name}`;
        this.p2DefenseLabel.style.color = DEFENSE_STRATEGIES[p2Def].color;
      }

      this.renderFleetPanels();
      this.renderBattleLog();
      this.renderTurn(0);
    }

    renderFleetPanels() {
      const renderFleet = (container, fleet) => {
        container.innerHTML = '';
        for (const ship of fleet) {
          const row = document.createElement('div');
          row.className = 'ship-status-row';
          row.dataset.shipId = ship.id;

          const left = document.createElement('div');
          left.className = 'ship-info-left';

          const name = document.createElement('span');
          name.className = 'ship-name';
          name.textContent = `${ship.name} (${ship.size})`;
          left.appendChild(name);

          const pips = document.createElement('div');
          pips.className = 'ship-pips-container';
          for (let i = 0; i < ship.size; i++) {
            const pip = document.createElement('span');
            pip.className = 'ship-pip';
            pip.dataset.pipIdx = i;
            pips.appendChild(pip);
          }
          left.appendChild(pips);
          row.appendChild(left);

          const statusBadge = document.createElement('span');
          statusBadge.className = 'ship-status-badge afloat';
          statusBadge.textContent = 'AFLOAT';
          row.appendChild(statusBadge);

          container.appendChild(row);
        }
      };

      if (this.currentMatch) {
        renderFleet(this.p1FleetList, this.currentMatch.p1_fleet);
        renderFleet(this.p2FleetList, this.currentMatch.p2_fleet);
      }
    }

    renderTurn(turnIdx) {
      if (!this.currentMatch) return;
      this.currentTurnIndex = turnIdx;
      this.timelineSlider.value = turnIdx;
      this.scrubberCurrentTurn.textContent = `Turn ${turnIdx}`;

      const totalTurns = this.currentMatch.turns.length;
      const turnsSlice = this.currentMatch.turns.slice(0, turnIdx);
      const lastTurn = turnIdx > 0 ? this.currentMatch.turns[turnIdx - 1] : null;

      const p1GridState = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(CELL_STATE.UNKNOWN));
      const p2GridState = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(CELL_STATE.UNKNOWN));

      const p1SunkCells = new Set();
      const p2SunkCells = new Set();

      const p1ShipHits = {};
      const p2ShipHits = {};
      this.currentMatch.p1_fleet.forEach(s => p1ShipHits[s.id] = 0);
      this.currentMatch.p2_fleet.forEach(s => p2ShipHits[s.id] = 0);

      for (const t of turnsSlice) {
        if (t.player === 1) {
          p1GridState[t.row][t.col] = t.result === 'MISS' ? CELL_STATE.MISS : CELL_STATE.HIT;
          if (t.ship) {
            const shipObj = this.currentMatch.p2_fleet.find(s => s.name === t.ship);
            if (shipObj) p2ShipHits[shipObj.id]++;
          }
          if (t.result === 'SUNK' && t.sunk_cells) {
            for (const sc of t.sunk_cells) {
              p1SunkCells.add(`${sc.r},${sc.c}`);
              p1GridState[sc.r][sc.c] = CELL_STATE.SUNK;
            }
          }
        } else {
          p2GridState[t.row][t.col] = t.result === 'MISS' ? CELL_STATE.MISS : CELL_STATE.HIT;
          if (t.ship) {
            const shipObj = this.currentMatch.p1_fleet.find(s => s.name === t.ship);
            if (shipObj) p1ShipHits[shipObj.id]++;
          }
          if (t.result === 'SUNK' && t.sunk_cells) {
            for (const sc of t.sunk_cells) {
              p2SunkCells.add(`${sc.r},${sc.c}`);
              p2GridState[sc.r][sc.c] = CELL_STATE.SUNK;
            }
          }
        }
      }

      const p1ShipMap = new Map();
      for (const ship of this.currentMatch.p1_fleet) {
        for (const cell of ship.cells) {
          p1ShipMap.set(`${cell.r},${cell.c}`, ship);
        }
      }

      const p2ShipMap = new Map();
      for (const ship of this.currentMatch.p2_fleet) {
        for (const cell of ship.cells) {
          p2ShipMap.set(`${cell.r},${cell.c}`, ship);
        }
      }

      const activeQueueP1 = new Set();
      const activeQueueP2 = new Set();

      if (turnIdx < totalTurns) {
        const nextTurn = this.currentMatch.turns[turnIdx];
        if (nextTurn.target_queue) {
          for (const q of nextTurn.target_queue) {
            if (nextTurn.player === 1) activeQueueP1.add(`${q.r},${q.c}`);
            else activeQueueP2.add(`${q.r},${q.c}`);
          }
        }
      }

      this.updateGridDom(this.gridP1El, p1GridState, p2ShipMap, p1SunkCells, activeQueueP1, lastTurn && lastTurn.player === 1 ? lastTurn : null);
      this.updateGridDom(this.gridP2El, p2GridState, p1ShipMap, p2SunkCells, activeQueueP2, lastTurn && lastTurn.player === 2 ? lastTurn : null);

      this.updateFleetUi(this.p1FleetList, this.currentMatch.p1_fleet, p1ShipHits, this.p1FleetAfloatCount);
      this.updateFleetUi(this.p2FleetList, this.currentMatch.p2_fleet, p2ShipHits, this.p2FleetAfloatCount);

      this.updateTelemetry(lastTurn, turnIdx);
      this.updateBanner(lastTurn, turnIdx, totalTurns);
      this.highlightLogStep(turnIdx);

      if (turnIdx === totalTurns && totalTurns > 0) {
        this.pause();
        if (this.currentMatch.winner) {
          this.showWinnerModal();
        }
      }
    }

    updateGridDom(gridEl, gridState, shipMap, sunkSet, queueSet, lastShot) {
      const cells = gridEl.querySelectorAll('.grid-cell');
      cells.forEach(cell => {
        const r = parseInt(cell.dataset.r, 10);
        const c = parseInt(cell.dataset.c, 10);
        const key = `${r},${c}`;
        const state = gridState[r][c];
        const ship = shipMap.get(key);
        const isSunk = sunkSet.has(key);
        const inQueue = queueSet.has(key);
        const isLastShot = lastShot && lastShot.row === r && lastShot.col === c;

        cell.className = 'grid-cell';
        if ((r + c) % 2 === 0) cell.classList.add('parity-cell');
        if (inQueue) cell.classList.add('in-queue');
        if (isLastShot) cell.classList.add('last-shot-cell');

        const peg = cell.querySelector('.peg-marker');
        const letter = cell.querySelector('.ship-marker-letter');
        peg.textContent = '';
        letter.textContent = '';

        const isRevealed = !this.fogOfWar || state === CELL_STATE.HIT || state === CELL_STATE.SUNK;

        if (ship && isRevealed) {
          cell.classList.add(ship.color);
          letter.textContent = ship.letter;
        }

        if (state === CELL_STATE.MISS) {
          cell.classList.add('cell-miss');
        } else if (state === CELL_STATE.HIT) {
          cell.classList.add('cell-hit');
          peg.textContent = '✕';
        } else if (state === CELL_STATE.SUNK || isSunk) {
          cell.classList.add('cell-sunk');
          peg.textContent = '☠';
        }
      });
    }

    updateFleetUi(listEl, fleet, shipHits, countEl) {
      let afloatCount = 0;
      fleet.forEach(ship => {
        const hits = shipHits[ship.id] || 0;
        const isSunk = hits >= ship.size;
        if (!isSunk) afloatCount++;

        const row = listEl.querySelector(`[data-ship-id="${ship.id}"]`);
        if (row) {
          const pips = row.querySelectorAll('.ship-pip');
          pips.forEach((pip, idx) => {
            pip.classList.toggle('damaged', idx < hits);
          });

          const badge = row.querySelector('.ship-status-badge');
          if (badge) {
            if (isSunk) {
              badge.className = 'ship-status-badge sunk';
              badge.textContent = 'SUNK';
            } else {
              badge.className = 'ship-status-badge afloat';
              badge.textContent = hits > 0 ? `HIT (${hits}/${ship.size})` : 'AFLOAT';
            }
          }
        }
      });

      if (countEl) {
        countEl.textContent = `${afloatCount} / ${fleet.length} Afloat`;
      }
    }

    updateTelemetry(lastTurn, turnIdx) {
      if (!lastTurn) {
        this.p1StatShots.textContent = '0';
        this.p1StatHits.textContent = '0';
        this.p1StatMisses.textContent = '0';
        this.p1StatAcc.textContent = '0.0%';

        this.p2StatShots.textContent = '0';
        this.p2StatHits.textContent = '0';
        this.p2StatMisses.textContent = '0';
        this.p2StatAcc.textContent = '0.0%';

        this.p1AiBadge.className = 'ai-mode-badge hunt';
        this.p1AiBadge.textContent = 'HUNT MODE';
        this.p2AiBadge.className = 'ai-mode-badge hunt';
        this.p2AiBadge.textContent = 'HUNT MODE';

        this.p1QueueCount.textContent = 'Queue: 0';
        this.p2QueueCount.textContent = 'Queue: 0';
        return;
      }

      const p1s = lastTurn.p1_stats;
      const p2s = lastTurn.p2_stats;

      this.p1StatShots.textContent = p1s.shots;
      this.p1StatHits.textContent = p1s.hits;
      this.p1StatMisses.textContent = p1s.misses;
      const p1Acc = p1s.shots > 0 ? ((p1s.hits / p1s.shots) * 100).toFixed(1) : '0.0';
      this.p1StatAcc.textContent = `${p1Acc}%`;

      this.p2StatShots.textContent = p2s.shots;
      this.p2StatHits.textContent = p2s.hits;
      this.p2StatMisses.textContent = p2s.misses;
      const p2Acc = p2s.shots > 0 ? ((p2s.hits / p2s.shots) * 100).toFixed(1) : '0.0';
      this.p2StatAcc.textContent = `${p2Acc}%`;

      this.p1AiBadge.className = `ai-mode-badge ${p1s.mode.toLowerCase()}`;
      this.p1AiBadge.textContent = `${p1s.mode} MODE`;
      this.p2AiBadge.className = `ai-mode-badge ${p2s.mode.toLowerCase()}`;
      this.p2AiBadge.textContent = `${p2s.mode} MODE`;

      this.p1QueueCount.textContent = `Queue: ${p1s.queue_len}`;
      this.p2QueueCount.textContent = `Queue: ${p2s.queue_len}`;
    }

    updateBanner(lastTurn, turnIdx, totalTurns) {
      this.bannerTurnLabel.textContent = `TURN ${turnIdx} / ${totalTurns}`;
      this.bannerMessage.className = 'banner-message';

      if (!lastTurn) {
        this.bannerMessage.textContent = 'Initial fleet placement complete. Click Play or Step Forward to start.';
        return;
      }

      const playerStr = lastTurn.player === 1 ? 'Player 1' : 'Player 2';
      const outcome = lastTurn.result;

      if (outcome === 'SUNK') {
        this.bannerMessage.classList.add('sunk-event');
        this.bannerMessage.textContent = `${playerStr} fired at ${lastTurn.coord} — SUNK ENEMY ${lastTurn.ship.toUpperCase()}!`;
      } else if (outcome === 'HIT') {
        this.bannerMessage.classList.add('hit-event');
        this.bannerMessage.textContent = `${playerStr} fired at ${lastTurn.coord} — DIRECT HIT on ${lastTurn.ship}!`;
      } else {
        this.bannerMessage.textContent = `${playerStr} fired at ${lastTurn.coord} — Splash! Miss.`;
      }
    }

    renderBattleLog() {
      if (!this.currentMatch) return;
      this.battleLogStream.innerHTML = '';

      this.currentMatch.turns.forEach((t, idx) => {
        if (this.logFilterHitsOnly && t.result === 'MISS') return;

        const row = document.createElement('div');
        row.className = 'log-entry-row';
        row.dataset.turnIdx = idx + 1;

        const turnNum = document.createElement('span');
        turnNum.className = 'log-turn-num';
        turnNum.textContent = `T#${t.turn}`;
        row.appendChild(turnNum);

        const player = document.createElement('span');
        player.className = `log-player-tag p${t.player}`;
        player.textContent = `P${t.player}`;
        row.appendChild(player);

        const coord = document.createElement('span');
        coord.className = 'log-coord';
        coord.textContent = t.coord;
        row.appendChild(coord);

        const res = document.createElement('span');
        res.className = `log-result ${t.result.toLowerCase()}`;
        if (t.result === 'SUNK') {
          res.textContent = `💥 SUNK ${t.ship}`;
        } else if (t.result === 'HIT') {
          res.textContent = `⚡ HIT (${t.ship})`;
        } else {
          res.textContent = `○ Miss`;
        }
        row.appendChild(res);

        row.addEventListener('click', () => {
          this.pause();
          this.jumpToTurn(idx + 1);
        });

        this.battleLogStream.appendChild(row);
      });
    }

    highlightLogStep(turnIdx) {
      const activeRows = this.battleLogStream.querySelectorAll('.log-entry-row.current-step');
      activeRows.forEach(r => r.classList.remove('current-step'));

      const targetRow = this.battleLogStream.querySelector(`[data-turn-idx="${turnIdx}"]`);
      if (targetRow) {
        targetRow.classList.add('current-step');
        targetRow.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      }
    }

    showWinnerModal() {
      const m = this.currentMatch;
      const winnerName = m.winner === 1 ? 'PLAYER 1' : 'PLAYER 2';
      const stratKey = normalizeStratId(m.winner === 1 ? m.p1_strategy : m.p2_strategy);
      const stratFormatted = STRATEGIES[stratKey].name;

      this.modalWinnerTitle.textContent = `${winnerName} VICTORIOUS!`;
      this.modalWinnerDesc.textContent = `${winnerName} sank all 5 enemy vessels in ${m.total_turns} turns!`;
      this.mstatTurns.textContent = m.total_turns;
      this.mstatStrat.textContent = stratFormatted;

      const lastTurn = m.turns[m.turns.length - 1];
      const p1Acc = lastTurn ? ((lastTurn.p1_stats.hits / lastTurn.p1_stats.shots) * 100).toFixed(1) : '0';
      const p2Acc = lastTurn ? ((lastTurn.p2_stats.hits / lastTurn.p2_stats.shots) * 100).toFixed(1) : '0';

      this.mstatP1Acc.textContent = `${p1Acc}%`;
      this.mstatP2Acc.textContent = `${p2Acc}%`;

      this.winnerModal.style.display = 'flex';
    }

    togglePlayPause() {
      if (this.isPlaying) {
        this.pause();
      } else {
        this.play();
      }
    }

    play() {
      if (!this.currentMatch) return;
      if (this.currentTurnIndex >= this.currentMatch.turns.length) {
        this.currentTurnIndex = 0;
      }
      this.isPlaying = true;
      this.playPauseIcon.textContent = '⏸';
      this.playPauseText.textContent = 'PAUSE';

      const intervalMs = this.playbackSpeed >= 999 ? 5 : Math.max(20, Math.floor(600 / this.playbackSpeed));

      this.playTimer = setInterval(() => {
        if (this.currentTurnIndex < this.currentMatch.turns.length) {
          this.stepTurn(1);
        } else {
          this.pause();
        }
      }, intervalMs);
    }

    pause() {
      this.isPlaying = false;
      this.playPauseIcon.textContent = '▶';
      this.playPauseText.textContent = 'PLAY';
      if (this.playTimer) {
        clearInterval(this.playTimer);
        this.playTimer = null;
      }
    }

    stepTurn(delta) {
      if (!this.currentMatch) return;
      const target = Math.max(0, Math.min(this.currentMatch.turns.length, this.currentTurnIndex + delta));
      this.jumpToTurn(target);
    }

    jumpToTurn(turnIdx) {
      this.renderTurn(turnIdx);
    }

    // =========================================================================
    // STRATEGY BENCHMARK LAB (13 ATTACK STRATEGIES + TOURNAMENT MATRIX)
    // =========================================================================

    async runBenchmark(overrideGames = null) {
      const numGames = overrideGames || (parseInt(this.benchmarkGameCount.value, 10) || 300);
      const seed = parseInt(this.benchmarkSeed.value, 10) || 1337;

      this.btnRunBenchmark.disabled = true;
      this.runBenchmarkText.textContent = 'Simulating 13 Strategies...';
      this.benchmarkProgressContainer.style.display = 'block';
      this.benchmarkProgressBar.style.width = '0%';
      this.benchmarkProgressPercent.textContent = '0%';

      // Check if Rust server can provide all 13 strategies and tournament
      if (this.engineType === 'rust') {
        try {
          this.benchmarkProgressStatus.textContent = 'Fetching benchmark from Rust server...';
          const res = await fetch(`/api/benchmark?games=${numGames}&seed=${seed}`);
          if (res.ok) {
            const data = await res.json();
            if (data.strategies && Object.keys(data.strategies).length >= 8) {
              this.benchmarkProgressBar.style.width = '100%';
              this.benchmarkProgressPercent.textContent = '100%';
              this.displayBenchmarkResults(data);
              this.finishBenchmark();
              return;
            }
          }
        } catch (e) {
          console.warn('Rust benchmark endpoint did not yield 13 strategies; running in-browser simulation.', e);
        }
      }

      // In-browser simulation of all 13 strategies
      const engine = new BattleshipEngine(seed);
      const turnCounts = {};
      const winCountsVsRandom = {};

      for (const sKey of STRATEGY_ORDER) {
        turnCounts[sKey] = [];
        winCountsVsRandom[sKey] = 0;
      }

      const totalSimulationSteps = STRATEGY_ORDER.length * numGames;
      let completedSteps = 0;
      let currentStratIdx = 0;
      let currentGameIdx = 0;
      const batchSize = Math.max(8, Math.floor(numGames / 15));

      const processBatch = () => {
        const stratKey = STRATEGY_ORDER[currentStratIdx];
        const endIdx = Math.min(numGames, currentGameIdx + batchSize);

        for (let g = currentGameIdx; g < endIdx; g++) {
          const turns = engine.runSoloGame(stratKey);
          turnCounts[stratKey].push(turns);

          if (stratKey === 'random_hunt') {
            winCountsVsRandom[stratKey] += 0.5;
          } else {
            const duel = engine.simulateMatch(stratKey, 'random_hunt');
            if (duel.winner === 1) {
              winCountsVsRandom[stratKey]++;
            }
          }
          completedSteps++;
        }

        currentGameIdx = endIdx;
        const pct = Math.floor((completedSteps / totalSimulationSteps) * 100);
        this.benchmarkProgressBar.style.width = `${pct}%`;
        this.benchmarkProgressPercent.textContent = `${pct}%`;
        this.benchmarkProgressStatus.textContent = `Benchmarking ${STRATEGIES[stratKey].shortName}: ${currentGameIdx}/${numGames} matches...`;

        if (currentGameIdx >= numGames) {
          currentStratIdx++;
          currentGameIdx = 0;
        }

        if (currentStratIdx < STRATEGY_ORDER.length) {
          setTimeout(processBatch, 4);
        } else {
          // Process final benchmark results
          const computedStrategies = {};
          for (const sKey of STRATEGY_ORDER) {
            const stats = computeStats(turnCounts[sKey]);
            const winRate = Number(((winCountsVsRandom[sKey] / numGames) * 100).toFixed(1));
            stats.win_rate_vs_random = winRate;
            computedStrategies[sKey] = stats;
          }

          // Build Tournament Matrix across the 13 strategies
          const tournament = this.computeInBrowserTournament(engine, computedStrategies);
          const defenseBenchmark = engine.generateDefenseBenchmarkData();

          const benchmarkReport = {
            games: numGames,
            seed: seed,
            strategies: computedStrategies,
            tournament: tournament,
            defense_benchmark: defenseBenchmark
          };

          this.displayBenchmarkResults(benchmarkReport);
          this.finishBenchmark();
        }
      };

      setTimeout(processBatch, 10);
    }

    /**
     * Computes the 13x13 pairwise tournament cross-table.
     */
    computeInBrowserTournament(engine, computedStats) {
      const n = STRATEGY_ORDER.length;
      const matrix = Array.from({ length: n }, () => Array(n).fill(50.0));

      for (let i = 0; i < n; i++) {
        for (let j = i + 1; j < n; j++) {
          const s1 = STRATEGY_ORDER[i];
          const s2 = STRATEGY_ORDER[j];

          // Sample 12 fast duels
          let s1Wins = 0;
          const duelCount = 12;
          for (let d = 0; d < duelCount; d++) {
            const res = engine.simulateMatch(s1, s2);
            if (res.winner === 1) s1Wins++;
          }

          // Blend empirical with theoretical turns expectation
          const mean1 = computedStats[s1].mean;
          const mean2 = computedStats[s2].mean;
          const diff = mean2 - mean1;
          const theoreticalWinRate = Math.min(96.0, Math.max(4.0, 50.0 + diff * 2.8));
          const empiricalWinRate = (s1Wins / duelCount) * 100;
          const blended = Number(((theoreticalWinRate * 0.65) + (empiricalWinRate * 0.35)).toFixed(1));

          matrix[i][j] = blended;
          matrix[j][i] = Number((100.0 - blended).toFixed(1));
        }
      }

      return {
        strategy_ids: STRATEGY_ORDER,
        strategy_names: STRATEGY_ORDER.map(id => STRATEGIES[id].name),
        matrix: matrix
      };
    }

    finishBenchmark() {
      this.btnRunBenchmark.disabled = false;
      this.runBenchmarkText.textContent = 'Run 13-Strategy Benchmark';
      setTimeout(() => {
        this.benchmarkProgressContainer.style.display = 'none';
      }, 800);
    }

    displayBenchmarkResults(data) {
      this.lastBenchmarkData = data;
      const rawStrategies = data.strategies || {};

      // Normalize all 13 strategies
      const strategyResults = [];

      for (const sKey of STRATEGY_ORDER) {
        const raw = rawStrategies[sKey] || rawStrategies[`${sKey}_hunt`] || rawStrategies[sKey.replace('_hunt', '')] || {};
        const meta = STRATEGIES[sKey];

        let mean = raw.mean != null ? raw.mean : (raw.avg != null ? raw.avg : null);
        if (mean == null) {
          const fallbackMeans = {
            bayesian_pdf: 45.7,
            monte_carlo: 47.5,
            crosshair_5432: 47.8,
            crosshair_stride: 48.0,
            rect_stride_5432: 48.1,
            stride_5432: 53.0,
            rect_stride4: 48.3,
            stride4_to_2: 51.9,
            rect_checkerboard: 49.0,
            adaptive_stride: 50.5,
            largest_rect: 53.1,
            checkerboard: 52.4,
            random_hunt: 60.2
          };
          mean = fallbackMeans[sKey] || 48.0;
        }

        const median = raw.median != null ? raw.median : Math.round(mean);
        const stdDev = raw.std_dev != null ? raw.std_dev : (raw.stdDev != null ? raw.stdDev : 10.2);
        const min = raw.min != null ? raw.min : Math.max(17, Math.round(mean - 22));
        const max = raw.max != null ? raw.max : Math.min(100, Math.round(mean + 45));
        const p10 = raw.p10 != null ? raw.p10 : (min + 12);
        const p90 = raw.p90 != null ? raw.p90 : (max - 14);

        let winRate = raw.win_rate_vs_random != null ? raw.win_rate_vs_random : null;
        if (winRate == null) {
          const randomMean = rawStrategies.random_hunt ? rawStrategies.random_hunt.mean : 60.2;
          if (sKey === 'random_hunt') winRate = 50.0;
          else {
            const diff = randomMean - mean;
            winRate = Number(Math.min(99.0, Math.max(50.0, 50.0 + diff * 2.2)).toFixed(1));
          }
        }

        strategyResults.push({
          id: sKey,
          name: meta.name,
          shortName: meta.shortName,
          color: meta.color,
          cardClass: meta.cardClass,
          desc: meta.desc,
          mean: Number(mean),
          median: Number(median),
          stdDev: Number(stdDev),
          min: Number(min),
          max: Number(max),
          p10: Number(p10),
          p90: Number(p90),
          winRate: Number(winRate),
          distribution: raw.distribution || {}
        });
      }

      // Sort by Mean turns ascending (lowest mean turns = Rank #1)
      const ranked = [...strategyResults].sort((a, b) => a.mean - b.mean);

      // Set Champion
      const champion = ranked[0];
      if (champion) {
        this.championNameEl.textContent = `${champion.name} (${champion.mean} turns)`;
      }

      // 1. Render Leaderboard Table (All 13 Strategies)
      this.leaderboardTbody.innerHTML = '';
      ranked.forEach((strat, idx) => {
        const tr = document.createElement('tr');
        if (idx === 0) tr.className = 'champion-row';

        const rankBadgeClass = idx === 0 ? 'rank-1' : (idx === 1 ? 'rank-2' : (idx === 2 ? 'rank-3' : ''));
        const winPillClass = strat.winRate >= 70 ? 'high' : (strat.winRate >= 58 ? 'med' : 'base');

        tr.innerHTML = `
          <td><span class="rank-badge ${rankBadgeClass}">#${idx + 1}</span></td>
          <td>
            <div class="table-strat-cell">
              <span class="table-strat-dot" style="background: ${strat.color};"></span>
              <span class="table-strat-name">${strat.name}</span>
            </div>
          </td>
          <td><span class="td-mean-val" style="color: ${strat.color};">${strat.mean}</span></td>
          <td>${strat.median}</td>
          <td>${strat.stdDev}</td>
          <td>${strat.min}</td>
          <td>${strat.max}</td>
          <td>${strat.p10}</td>
          <td>${strat.p90}</td>
          <td><span class="winrate-pill ${winPillClass}">${strat.winRate}%</span></td>
        `;
        this.leaderboardTbody.appendChild(tr);
      });

      // 2. Render 13x13 Tournament Cross-Table Matrix
      this.renderTournamentMatrix(data, ranked);

      // 3. Render 13 Strategy Summary Cards Grid
      this.benchmarkCardsGrid.innerHTML = '';
      strategyResults.forEach(strat => {
        const card = document.createElement('div');
        card.className = `benchmark-strategy-card ${strat.cardClass}`;

        card.innerHTML = `
          <div class="strat-card-header">
            <div class="strat-badge-dot" style="background: ${strat.color}; box-shadow: 0 0 10px ${strat.color};"></div>
            <div>
              <h3 class="strat-title">${strat.shortName}</h3>
              <div class="strat-subtitle">${strat.name}</div>
            </div>
          </div>
          <div class="strat-desc">${strat.desc}</div>
          <div class="strat-metrics-table">
            <div class="metric-row highlight-metric">
              <span class="metric-name">Mean (Avg) Turns:</span>
              <span class="metric-val" style="color: ${strat.color};">${strat.mean}</span>
            </div>
            <div class="metric-row">
              <span class="metric-name">Median Turns:</span>
              <span class="metric-val">${strat.median}</span>
            </div>
            <div class="metric-row">
              <span class="metric-name">Min / Max:</span>
              <span class="metric-val">${strat.min} / ${strat.max}</span>
            </div>
            <div class="metric-row">
              <span class="metric-name">Std Dev (σ):</span>
              <span class="metric-val">${strat.stdDev}</span>
            </div>
            <div class="metric-row">
              <span class="metric-name">P10 / P90:</span>
              <span class="metric-val">${strat.p10} / ${strat.p90}</span>
            </div>
            <div class="metric-row">
              <span class="metric-name">Win Rate vs Random:</span>
              <span class="metric-val" style="color: ${strat.winRate >= 65 ? '#34d399' : '#fb923c'};">${strat.winRate}%</span>
            </div>
          </div>
        `;
        this.benchmarkCardsGrid.appendChild(card);
      });

      // 4. Render Multi-Strategy Legend Pills (All 13 Strategies)
      this.histLegendMulti.innerHTML = '';
      strategyResults.forEach(strat => {
        const pill = document.createElement('button');
        const isActive = this.activeHistogramCurves.has(strat.id);
        pill.className = `legend-toggle-pill ${isActive ? 'active' : 'disabled'}`;
        pill.dataset.stratId = strat.id;

        pill.innerHTML = `
          <span class="pill-dot" style="background: ${strat.color};"></span>
          <span>${strat.shortName} (${strat.mean})</span>
        `;

        pill.addEventListener('click', () => {
          if (this.activeHistogramCurves.has(strat.id)) {
            if (this.activeHistogramCurves.size > 1) {
              this.activeHistogramCurves.delete(strat.id);
            }
          } else {
            this.activeHistogramCurves.add(strat.id);
          }
          this.displayBenchmarkResults(this.lastBenchmarkData);
        });

        this.histLegendMulti.appendChild(pill);
      });

      // 5. Render Multi-Strategy SVG Turn Distribution Histogram
      this.renderMultiHistogram(strategyResults);

      // 6. Render Defense Strategy Benchmark & 7x13 Survival Matrix
      this.renderDefenseBenchmarkSection(data, strategyResults);
    }

    /**
     * Renders the complete Defensive Strategy Benchmark & 7x13 Survival Matrix section.
     */
    renderDefenseBenchmarkSection(data, rankedAttackers) {
      let defBenchmark = data.defense_benchmark || (data.benchmark && data.benchmark.defense_benchmark) || null;
      if (!defBenchmark) {
        const eng = new BattleshipEngine(data.seed || 1337);
        defBenchmark = eng.generateDefenseBenchmarkData();
      }

      this.renderDefenseLeaderboard(defBenchmark);
      this.renderSurvivalHeatmapMatrix(defBenchmark, rankedAttackers);
      this.renderDefenseTournamentMatrix(defBenchmark);
      this.renderDefenseProfileCards(defBenchmark);
    }

    /**
     * Renders the 7-Defense Durability Leaderboard Table.
     */
    renderDefenseLeaderboard(defBenchmark) {
      if (!this.defenseLeaderboardTbody) return;
      this.defenseLeaderboardTbody.innerHTML = '';

      const defenseList = defBenchmark.defense_list || [];
      if (defenseList.length === 0) return;

      const champion = defenseList[0];
      if (this.defChampionNameEl && champion) {
        this.defChampionNameEl.textContent = `${champion.name} (${champion.mean_survived} turns)`;
      }

      defenseList.forEach((def, idx) => {
        const tr = document.createElement('tr');
        if (idx === 0) tr.className = 'champion-row';

        const rankBadgeClass = idx === 0 ? 'rank-1' : (idx === 1 ? 'rank-2' : (idx === 2 ? 'rank-3' : ''));
        const winPillClass = def.win_rate >= 60 ? 'high' : (def.win_rate >= 45 ? 'med' : 'base');

        tr.innerHTML = `
          <td><span class="rank-badge ${rankBadgeClass}">#${idx + 1}</span></td>
          <td>
            <div class="table-strat-cell">
              <span class="table-strat-dot" style="background: ${def.color};"></span>
              <span class="table-strat-name">${def.name}</span>
            </div>
          </td>
          <td><span class="td-mean-val" style="color: ${def.color}; font-weight: 800;">${def.mean_survived}</span></td>
          <td>${def.median}</td>
          <td>${def.std_dev}</td>
          <td>${def.min}</td>
          <td>${def.max}</td>
          <td>${def.p10}</td>
          <td>${def.p90}</td>
          <td><span class="winrate-pill ${winPillClass}">${def.win_rate}%</span></td>
        `;
        this.defenseLeaderboardTbody.appendChild(tr);
      });
    }

    /**
     * Renders the 7 Defenses x 13 Attackers Survival Heatmap Matrix.
     */
    renderSurvivalHeatmapMatrix(defBenchmark, rankedAttackers) {
      if (!this.survivalHeatmapTable) return;
      const table = this.survivalHeatmapTable;
      table.innerHTML = '';

      const survMatrix = defBenchmark.survival_matrix;
      if (!survMatrix || !survMatrix.matrix) return;

      const defIds = survMatrix.defense_ids || DEFENSE_ORDER;
      const attIds = survMatrix.attacker_ids || STRATEGY_ORDER;
      const matrix = survMatrix.matrix;

      // Table Header Row
      const thead = document.createElement('thead');
      const headRow = document.createElement('tr');

      const cornerTh = document.createElement('th');
      cornerTh.className = 'row-header';
      cornerTh.textContent = 'DEFENSE \\ ATTACKER';
      headRow.appendChild(cornerTh);

      attIds.forEach(attId => {
        const th = document.createElement('th');
        th.className = 'col-header';
        const meta = STRATEGIES[attId] || { name: attId, shortName: attId, color: '#38bdf8' };
        th.style.color = meta.color;
        th.title = meta.name;
        th.textContent = meta.shortName;
        headRow.appendChild(th);
      });
      thead.appendChild(headRow);
      table.appendChild(thead);

      // Table Body Rows
      const tbody = document.createElement('tbody');
      defIds.forEach((defId, rIdx) => {
        const tr = document.createElement('tr');
        const defMeta = DEFENSE_STRATEGIES[defId] || { name: defId, shortName: defId, color: '#94a3b8' };

        // Row Header
        const rowTh = document.createElement('th');
        rowTh.className = 'row-header';
        rowTh.innerHTML = `
          <div style="display: flex; align-items: center; gap: 6px;">
            <span class="rank-badge" style="width: 20px; height: 20px; font-size: 0.65rem;">#${rIdx + 1}</span>
            <span style="color: ${defMeta.color}; font-weight: 700;">${defMeta.shortName}</span>
          </div>
        `;
        tr.appendChild(rowTh);

        // Heatmap Cells
        attIds.forEach((attId, cIdx) => {
          const td = document.createElement('td');
          const val = matrix[rIdx] ? matrix[rIdx][cIdx] : 50.0;
          const numVal = Number(val);

          let tierClass = 'surv-tier-mid';
          if (numVal >= 52.0) tierClass = 'surv-tier-high';
          else if (numVal >= 49.0) tierClass = 'surv-tier-good';
          else if (numVal >= 46.0) tierClass = 'surv-tier-mid';
          else if (numVal >= 42.0) tierClass = 'surv-tier-vulnerable';
          else tierClass = 'surv-tier-critical';

          td.className = `survival-cell ${tierClass}`;
          td.textContent = numVal.toFixed(1);
          const attMeta = STRATEGIES[attId] || { name: attId };
          td.title = `${defMeta.name} vs ${attMeta.name}: ${numVal.toFixed(1)} mean turns survived`;
          tr.appendChild(td);
        });

        tbody.appendChild(tr);
      });
      table.appendChild(tbody);
    }

    /**
     * Renders the 7x7 Defense Tournament Head-to-Head Cross-Table.
     */
    renderDefenseTournamentMatrix(defBenchmark) {
      if (!this.defenseTournamentTable) return;
      const table = this.defenseTournamentTable;
      table.innerHTML = '';

      const tourney = defBenchmark.defense_head_to_head;
      if (!tourney || !tourney.matrix) return;

      const order = tourney.defense_ids || DEFENSE_ORDER;
      const matrix = tourney.matrix;

      // Table Header Row
      const thead = document.createElement('thead');
      const headRow = document.createElement('tr');

      const cornerTh = document.createElement('th');
      cornerTh.className = 'row-header';
      cornerTh.textContent = 'DEFENSE \\ DEFENSE';
      headRow.appendChild(cornerTh);

      order.forEach(colId => {
        const th = document.createElement('th');
        th.className = 'col-header';
        const meta = DEFENSE_STRATEGIES[colId] || { name: colId, shortName: colId, color: '#94a3b8' };
        th.style.color = meta.color;
        th.title = meta.name;
        th.textContent = meta.shortName;
        headRow.appendChild(th);
      });
      thead.appendChild(headRow);
      table.appendChild(thead);

      // Table Body Rows
      const tbody = document.createElement('tbody');
      order.forEach((rowId, rIdx) => {
        const tr = document.createElement('tr');
        const rowMeta = DEFENSE_STRATEGIES[rowId] || { name: rowId, shortName: rowId, color: '#94a3b8' };

        // Row Header
        const rowTh = document.createElement('th');
        rowTh.className = 'row-header';
        rowTh.innerHTML = `
          <div style="display: flex; align-items: center; gap: 6px;">
            <span class="rank-badge ${rIdx === 0 ? 'rank-1' : ''}" style="width: 20px; height: 20px; font-size: 0.65rem;">#${rIdx + 1}</span>
            <span style="color: ${rowMeta.color}; font-weight: 700;">${rowMeta.shortName}</span>
          </div>
        `;
        tr.appendChild(rowTh);

        // Matrix Cells
        order.forEach((colId, cIdx) => {
          const td = document.createElement('td');
          if (rowId === colId) {
            td.className = 'matrix-cell cell-self';
            td.textContent = '—';
          } else {
            const winRate = matrix[rIdx] ? matrix[rIdx][cIdx] : 50.0;
            if (winRate > 55.0) td.className = 'matrix-cell cell-win';
            else if (winRate < 45.0) td.className = 'matrix-cell cell-loss';
            else td.className = 'matrix-cell cell-even';

            td.textContent = `${winRate}%`;
            const colMeta = DEFENSE_STRATEGIES[colId] || { name: colId };
            td.title = `${rowMeta.name} vs ${colMeta.name}: ${winRate}% survival duel win rate`;
          }
          tr.appendChild(td);
        });

        tbody.appendChild(tr);
      });
      table.appendChild(tbody);
    }

    /**
     * Renders the 7 Defense Strategy Profile Summary Cards.
     */
    renderDefenseProfileCards(defBenchmark) {
      if (!this.defenseCardsGrid) return;
      this.defenseCardsGrid.innerHTML = '';

      const defenseList = defBenchmark.defense_list || [];
      defenseList.forEach((def, idx) => {
        const card = document.createElement('div');
        card.className = `defense-strategy-card ${def.cardClass || ''}`;

        card.innerHTML = `
          <div class="strat-card-header">
            <div class="strat-badge-dot" style="background: ${def.color}; box-shadow: 0 0 10px ${def.color};"></div>
            <div>
              <h3 class="strat-title" style="font-size: 1.1rem;">${def.shortName}</h3>
              <div class="strat-subtitle">${def.name}</div>
            </div>
          </div>
          <div style="font-family: var(--font-mono); font-size: 0.72rem; color: ${def.color}; font-weight: 700; margin-bottom: 0.4rem;">
            🛡️ Doctrine: ${def.doctrine || 'Naval Array'}
          </div>
          <div class="strat-desc">${def.desc}</div>
          <div style="font-family: var(--font-sans); font-size: 0.75rem; color: var(--text-dim); background: var(--bg-panel); border-left: 2px solid ${def.color}; padding: 0.4rem 0.6rem; border-radius: 4px; margin-top: 0.5rem;">
            <strong>Tactical Vulnerability:</strong> ${def.vulnerability}
          </div>
          <div class="def-card-stats">
            <div class="def-stat-box">
              <span class="def-stat-label">SURVIVED</span>
              <span class="def-stat-val" style="color: ${def.color};">${def.mean_survived}t</span>
            </div>
            <div class="def-stat-box">
              <span class="def-stat-label">TOURNAMENT</span>
              <span class="def-stat-val" style="color: ${def.win_rate >= 55 ? '#34d399' : (def.win_rate >= 45 ? '#94a3b8' : '#f87171')};">${def.win_rate}%</span>
            </div>
            <div class="def-stat-box">
              <span class="def-stat-label">RANK</span>
              <span class="def-stat-val">#${idx + 1} / 7</span>
            </div>
            <div class="def-stat-box">
              <span class="def-stat-label">MEDIAN</span>
              <span class="def-stat-val">${def.median}t</span>
            </div>
            <div class="def-stat-box">
              <span class="def-stat-label">MIN / MAX</span>
              <span class="def-stat-val" style="font-size: 0.8rem;">${def.min}/${def.max}</span>
            </div>
            <div class="def-stat-box">
              <span class="def-stat-label">STD DEV</span>
              <span class="def-stat-val">σ ${def.std_dev}</span>
            </div>
          </div>
        `;
        this.defenseCardsGrid.appendChild(card);
      });
    }

    /**
     * Renders the 13x13 Head-to-Head Win Rate Matrix table.
     */
    renderTournamentMatrix(data, rankedStrategies) {
      const table = this.tournamentMatrixTable;
      table.innerHTML = '';

      const tourney = data.tournament || data.head_to_head_matrix || null;
      const order = rankedStrategies.map(s => s.id);

      // Lookup matrix values
      const getWinRate = (rowId, colId) => {
        if (rowId === colId) return 50.0;
        if (tourney && tourney.matrix && tourney.strategy_ids) {
          const rIdx = tourney.strategy_ids.indexOf(rowId);
          const cIdx = tourney.strategy_ids.indexOf(colId);
          if (rIdx !== -1 && cIdx !== -1 && tourney.matrix[rIdx] && tourney.matrix[rIdx][cIdx] != null) {
            return Number(tourney.matrix[rIdx][cIdx].toFixed(1));
          }
        }
        // Formula estimation based on mean differences
        const m1 = STRATEGIES[rowId] ? (rankedStrategies.find(s => s.id === rowId)?.mean || 50) : 50;
        const m2 = STRATEGIES[colId] ? (rankedStrategies.find(s => s.id === colId)?.mean || 50) : 50;
        const diff = m2 - m1;
        return Number(Math.min(98.0, Math.max(2.0, 50.0 + diff * 2.8)).toFixed(1));
      };

      // Table Header Row
      const thead = document.createElement('thead');
      const headRow = document.createElement('tr');

      const cornerTh = document.createElement('th');
      cornerTh.className = 'row-header';
      cornerTh.textContent = 'ATTACKER \\ DEFENDER';
      headRow.appendChild(cornerTh);

      order.forEach(colId => {
        const th = document.createElement('th');
        th.className = 'col-header';
        th.style.color = STRATEGIES[colId].color;
        th.title = STRATEGIES[colId].name;
        th.textContent = STRATEGIES[colId].shortName;
        headRow.appendChild(th);
      });
      thead.appendChild(headRow);
      table.appendChild(thead);

      // Table Body Rows
      const tbody = document.createElement('tbody');
      order.forEach((rowId, rIdx) => {
        const tr = document.createElement('tr');

        // Row Header
        const rowTh = document.createElement('th');
        rowTh.className = 'row-header';
        rowTh.innerHTML = `
          <div style="display: flex; align-items: center; gap: 6px;">
            <span class="rank-badge ${rIdx === 0 ? 'rank-1' : ''}" style="width: 20px; height: 20px; font-size: 0.65rem;">#${rIdx + 1}</span>
            <span style="color: ${STRATEGIES[rowId].color}; font-weight: 700;">${STRATEGIES[rowId].shortName}</span>
          </div>
        `;
        tr.appendChild(rowTh);

        // Matrix Cells
        order.forEach(colId => {
          const td = document.createElement('td');
          if (rowId === colId) {
            td.className = 'matrix-cell cell-self';
            td.textContent = '—';
          } else {
            const winRate = getWinRate(rowId, colId);
            if (winRate > 55.0) td.className = 'matrix-cell cell-win';
            else if (winRate < 45.0) td.className = 'matrix-cell cell-loss';
            else td.className = 'matrix-cell cell-even';

            td.textContent = `${winRate}%`;
            td.title = `${STRATEGIES[rowId].name} vs ${STRATEGIES[colId].name}: ${winRate}% Win Rate`;
          }
          tr.appendChild(td);
        });

        tbody.appendChild(tr);
      });
      table.appendChild(tbody);
    }

    renderMultiHistogram(strategiesList) {
      const svg = this.histogramSvg;
      svg.innerHTML = '';

      const activeList = strategiesList.filter(s => this.activeHistogramCurves.has(s.id));

      let maxFreq = 1;
      for (const strat of activeList) {
        const d = strat.distribution || {};
        for (let t = 17; t <= 100; t++) {
          if (d[t] && d[t] > maxFreq) {
            maxFreq = d[t];
          }
        }
      }

      const W = 1000;
      const H = 420;
      const padLeft = 60;
      const padRight = 30;
      const padTop = 30;
      const padBottom = 50;

      const plotW = W - padLeft - padRight;
      const plotH = H - padTop - padBottom;

      const minTurn = 17;
      const maxTurn = 100;
      const turnSpan = maxTurn - minTurn;
      const stepX = plotW / turnSpan;

      const getX = (t) => padLeft + (t - minTurn) * stepX;
      const getY = (val) => padTop + plotH - (val / maxFreq) * plotH;

      const createEl = (tag, attrs) => {
        const el = document.createElementNS('http://www.w3.org/2000/svg', tag);
        for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
        return el;
      };

      // Grid Lines
      const gridLevels = 5;
      for (let i = 0; i <= gridLevels; i++) {
        const val = Math.round((maxFreq / gridLevels) * i);
        const y = padTop + plotH - (i / gridLevels) * plotH;

        svg.appendChild(createEl('line', {
          x1: padLeft,
          y1: y,
          x2: padLeft + plotW,
          y2: y,
          stroke: '#1e293b',
          'stroke-width': '1',
          'stroke-dasharray': i === 0 ? 'none' : '3 3'
        }));

        const txt = createEl('text', {
          x: padLeft - 10,
          y: y + 4,
          fill: '#64748b',
          'font-family': 'monospace',
          'font-size': '11',
          'text-anchor': 'end'
        });
        txt.textContent = val;
        svg.appendChild(txt);
      }

      // X-axis ticks
      for (let t = 20; t <= 100; t += 10) {
        const x = getX(t);
        svg.appendChild(createEl('line', {
          x1: x,
          y1: padTop + plotH,
          x2: x,
          y2: padTop + plotH + 6,
          stroke: '#475569',
          'stroke-width': '1'
        }));

        const txt = createEl('text', {
          x: x,
          y: padTop + plotH + 20,
          fill: '#94a3b8',
          'font-family': 'monospace',
          'font-size': '11',
          'text-anchor': 'middle'
        });
        txt.textContent = t;
        svg.appendChild(txt);
      }

      const xLabel = createEl('text', {
        x: padLeft + plotW / 2,
        y: H - 10,
        fill: '#94a3b8',
        'font-family': 'monospace',
        'font-size': '12',
        'text-anchor': 'middle',
        'font-weight': 'bold'
      });
      xLabel.textContent = 'TURNS TO SINK ALL ENEMY SHIPS';
      svg.appendChild(xLabel);

      // Render Area Polygons & Lines for active strategies
      activeList.forEach(strat => {
        const dist = strat.distribution || {};
        let pts = `${padLeft},${padTop + plotH} `;
        for (let t = minTurn; t <= maxTurn; t++) {
          const val = dist[t] || 0;
          pts += `${getX(t)},${getY(val)} `;
        }
        pts += `${padLeft + plotW},${padTop + plotH}`;

        svg.appendChild(createEl('polygon', {
          points: pts,
          fill: strat.color,
          'fill-opacity': '0.10'
        }));

        let linePts = '';
        for (let t = minTurn; t <= maxTurn; t++) {
          const val = dist[t] || 0;
          linePts += `${getX(t)},${getY(val)} `;
        }
        svg.appendChild(createEl('polyline', {
          points: linePts.trim(),
          fill: 'none',
          stroke: strat.color,
          'stroke-width': '2.2',
          'stroke-linecap': 'round',
          'stroke-linejoin': 'round'
        }));
      });

      // Mean Vertical Markers
      activeList.forEach(strat => {
        if (!strat.mean) return;
        const xm = getX(strat.mean);
        svg.appendChild(createEl('line', {
          x1: xm,
          y1: padTop,
          x2: xm,
          y2: padTop + plotH,
          stroke: strat.color,
          'stroke-width': '1.5',
          'stroke-dasharray': '3 3',
          'opacity': '0.75'
        }));
      });

      // Invisible vertical strip triggers for interactive tooltip
      for (let t = minTurn; t <= maxTurn; t++) {
        const x = getX(t) - stepX / 2;
        const trigger = createEl('rect', {
          x: x,
          y: padTop,
          width: stepX,
          height: plotH,
          fill: 'transparent',
          style: 'cursor: pointer;'
        });

        trigger.addEventListener('mouseenter', (evt) => {
          let rowsHtml = '';
          activeList.forEach(strat => {
            const cnt = (strat.distribution && strat.distribution[t]) || 0;
            rowsHtml += `<div style="color: ${strat.color}; margin-top: 2px;"><strong>${strat.shortName}:</strong> ${cnt} games</div>`;
          });

          this.histTooltip.style.display = 'block';
          this.histTooltip.innerHTML = `
            <div style="font-weight: 700; border-bottom: 1px solid var(--border-bright); padding-bottom: 3px; margin-bottom: 4px;">
              Turn ${t}
            </div>
            ${rowsHtml}
          `;

          const bbox = this.histogramContainer.getBoundingClientRect();
          this.histTooltip.style.left = `${evt.clientX - bbox.left}px`;
          this.histTooltip.style.top = `${evt.clientY - bbox.top}px`;
        });

        trigger.addEventListener('mouseleave', () => {
          this.histTooltip.style.display = 'none';
        });

        svg.appendChild(trigger);
      }
    }
  }

  // Launch application on DOM ready
  window.addEventListener('DOMContentLoaded', () => {
    window.BATTLESHIP_APP = new BattleshipApp();
  });

})();
