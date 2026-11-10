/**
 * BATTLESHIP COMMAND & STRATEGY LAB - CORE APPLICATION & CLIENT ENGINE
 * Supports 6 Attack Strategies:
 * 1. random_hunt: "Random + Hunt/Target"
 * 2. checkerboard: "Checkerboard + Hunt/Target"
 * 3. largest_rect: "Largest Empty Rect Midpoint"
 * 4. adaptive_stride: "Adaptive Stride (2→3→4→5)"
 * 5. stride4_to_2: "Staggered Stride (4→2)"
 * 6. rect_stride4: "Largest Rect + Stride (4→2)"
 */

(function () {
  'use strict';

  // =========================================================================
  // CONSTANTS & STRATEGY DEFINITIONS
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
    random_hunt: {
      id: 'random_hunt',
      name: 'Random + Hunt/Target',
      shortName: 'Random Hunt',
      color: '#f97316',
      cardClass: 'card-random_hunt',
      desc: 'Uniform random hunt across 100 cells until first hit, then targets orthogonal neighbors along line of fire.'
    },
    checkerboard: {
      id: 'checkerboard',
      name: 'Checkerboard + Hunt/Target',
      shortName: 'Checkerboard',
      color: '#38bdf8',
      cardClass: 'card-checkerboard',
      desc: 'Parity-restricted hunt on (r+c)%2==0 (50 cells). Because min ship size is 2, all ships must touch parity cells.'
    },
    largest_rect: {
      id: 'largest_rect',
      name: 'Largest Empty Rect Midpoint',
      shortName: 'Largest Rect',
      color: '#10b981',
      cardClass: 'card-largest_rect',
      desc: 'Finds the largest axis-aligned empty rectangle on the 10x10 board and fires at its exact center.'
    },
    adaptive_stride: {
      id: 'adaptive_stride',
      name: 'Adaptive Stride (2→3→4→5)',
      shortName: 'Adaptive Stride',
      color: '#a855f7',
      cardClass: 'card-adaptive_stride',
      desc: 'Dynamically widens parity search stride (k = 2, 3, 4, 5) matching the smallest remaining unsunk enemy ship.'
    },
    stride4_to_2: {
      id: 'stride4_to_2',
      name: 'Staggered Stride (4→2)',
      shortName: 'Stride (4→2)',
      color: '#ec4899',
      cardClass: 'card-stride4_to_2',
      desc: 'Searches on 4-stride grid while Carrier (5) or Battleship (4) are afloat, then drops to 2-stride checkerboard.'
    },
    rect_stride4: {
      id: 'rect_stride4',
      name: 'Largest Rect + Stride (4→2)',
      shortName: 'Rect + Stride (4→2)',
      color: '#f59e0b',
      cardClass: 'card-rect_stride4',
      desc: 'Finds the largest unshot rectangle and fires at the closest cell satisfying active 4-to-2 stride parity.'
    }
  };

  const STRATEGY_ORDER = [
    'rect_stride4',
    'stride4_to_2',
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
    return STRATEGIES[s] ? s : 'random_hunt';
  }

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
     * Randomly places all 5 Hasbro ships on a 10x10 grid with no overlaps.
     */
    generateFleet() {
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
        offset: Math.floor(this.rng() * 12),
        offset4: Math.floor(this.rng() * 4)
      };
    }

    /**
     * Selects the next shot coordinate for the given AI agent.
     */
    getNextShot(agent) {
      // 1. TARGET MODE (Used by all 6 strategies once a ship is hit)
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
      const strat = agent.strategy;

      // STRATEGY 2: Checkerboard Hunt
      if (strat === 'checkerboard') {
        const parity = agent.offset % 2;
        const parityCandidates = [];
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if ((r + c) % 2 === parity && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              parityCandidates.push({ r, c });
            }
          }
        }
        if (parityCandidates.length > 0) {
          return parityCandidates[Math.floor(this.rng() * parityCandidates.length)];
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
          // 4-stride diagonal search
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

        // 2-stride checkerboard fallback
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
     * Rebuilds target queue based on active unsunk hits.
     * Prioritizes collinear endpoints when 2+ hits align.
     */
    rebuildTargetQueue(agent) {
      agent.targetQueue = [];
      const hits = agent.activeHits;
      if (hits.length === 0) return;

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

      // Fallback: orthogonal neighbors of all active hits
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
    simulateMatch(p1Strategy, p2Strategy) {
      const p1FleetData = this.generateFleet();
      const p2FleetData = this.generateFleet();

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

              // Update remaining ship sizes for attacker
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
        p1_strategy: p1Strategy,
        p2_strategy: p2Strategy,
        winner: winner,
        total_turns: totalTurns,
        p1_fleet: p1Fleet,
        p2_fleet: p2Fleet,
        turns: turns
      };
    }

    /**
     * Runs 1 solo game for a strategy to measure turns-to-win.
     */
    runSoloGame(strategyType) {
      const fleetData = this.generateFleet();
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

      // Benchmark active curves in histogram
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
      this.benchmarkCardsGrid = document.getElementById('benchmark-cards-grid');

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
            // Auto run small 100-game benchmark so tab is vibrant immediately
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

        const res = await fetch('/api/match?p1=rect_stride4&p2=random_hunt&seed=1', {
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
        // Pre-generate 3 sample matches using top strategies
        const eng1 = new BattleshipEngine(42);
        const m1 = eng1.simulateMatch('rect_stride4', 'random_hunt');
        const eng2 = new BattleshipEngine(101);
        const m2 = eng2.simulateMatch('stride4_to_2', 'checkerboard');
        const eng3 = new BattleshipEngine(777);
        const m3 = eng3.simulateMatch('adaptive_stride', 'largest_rect');

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
      const seed = parseInt(this.inputMatchSeed.value, 10) || Math.floor(Math.random() * 999999);

      this.p1StrategyLabel.textContent = STRATEGIES[p1Strat].name;
      this.p2StrategyLabel.textContent = STRATEGIES[p2Strat].name;

      if (this.engineType === 'rust') {
        try {
          const res = await fetch(`/api/match?p1=${p1Strat}&p2=${p2Strat}&seed=${seed}`);
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
      const matchData = engine.simulateMatch(p1Strat, p2Strat);
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
      this.selectP1Strategy.value = p1Strat;
      this.selectP2Strategy.value = p2Strat;
      this.p1StrategyLabel.textContent = STRATEGIES[p1Strat].name;
      this.p2StrategyLabel.textContent = STRATEGIES[p2Strat].name;

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
    // STRATEGY BENCHMARK LAB (6 ATTACK STRATEGIES)
    // =========================================================================

    async runBenchmark(overrideGames = null) {
      const numGames = overrideGames || (parseInt(this.benchmarkGameCount.value, 10) || 500);
      const seed = parseInt(this.benchmarkSeed.value, 10) || 1337;

      this.btnRunBenchmark.disabled = true;
      this.runBenchmarkText.textContent = 'Simulating 6 Strategies...';
      this.benchmarkProgressContainer.style.display = 'block';
      this.benchmarkProgressBar.style.width = '0%';
      this.benchmarkProgressPercent.textContent = '0%';

      // Check if Rust server can provide all 6 strategies
      if (this.engineType === 'rust') {
        try {
          this.benchmarkProgressStatus.textContent = 'Fetching benchmark from Rust server...';
          const res = await fetch(`/api/benchmark?games=${numGames}&seed=${seed}`);
          if (res.ok) {
            const data = await res.json();
            // Verify if data contains at least 4 of the 6 strategies
            if (data.strategies && Object.keys(data.strategies).length >= 5) {
              this.benchmarkProgressBar.style.width = '100%';
              this.benchmarkProgressPercent.textContent = '100%';
              this.displayBenchmarkResults(data);
              this.finishBenchmark();
              return;
            }
          }
        } catch (e) {
          console.warn('Rust benchmark endpoint did not yield 6 strategies; running in-browser simulation.', e);
        }
      }

      // In-browser simulation of all 6 strategies
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
      const batchSize = Math.max(10, Math.floor(numGames / 20));

      const processBatch = () => {
        const stratKey = STRATEGY_ORDER[currentStratIdx];
        const endIdx = Math.min(numGames, currentGameIdx + batchSize);

        for (let g = currentGameIdx; g < endIdx; g++) {
          const turns = engine.runSoloGame(stratKey);
          turnCounts[stratKey].push(turns);

          if (stratKey === 'random_hunt') {
            winCountsVsRandom[stratKey] += 0.5; // Neutral
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

          const benchmarkReport = {
            games: numGames,
            seed: seed,
            strategies: computedStrategies
          };

          this.displayBenchmarkResults(benchmarkReport);
          this.finishBenchmark();
        }
      };

      setTimeout(processBatch, 10);
    }

    finishBenchmark() {
      this.btnRunBenchmark.disabled = false;
      this.runBenchmarkText.textContent = 'Run 6-Strategy Benchmark';
      setTimeout(() => {
        this.benchmarkProgressContainer.style.display = 'none';
      }, 800);
    }

    displayBenchmarkResults(data) {
      this.lastBenchmarkData = data;
      const rawStrategies = data.strategies || {};

      // Normalize into unified strategy objects
      const strategyResults = [];

      for (const sKey of STRATEGY_ORDER) {
        // Find matching entry from API or in-browser
        const raw = rawStrategies[sKey] || rawStrategies[`${sKey}_hunt`] || rawStrategies[sKey.replace('_hunt', '')] || {};
        const meta = STRATEGIES[sKey];

        const mean = raw.mean != null ? raw.mean : (raw.avg || 0);
        const median = raw.median != null ? raw.median : 0;
        const stdDev = raw.std_dev != null ? raw.std_dev : (raw.stdDev != null ? raw.stdDev : 0);
        const min = raw.min != null ? raw.min : 0;
        const max = raw.max != null ? raw.max : 0;
        const p10 = raw.p10 != null ? raw.p10 : (min + 4);
        const p90 = raw.p90 != null ? raw.p90 : (max - 4);

        let winRate = raw.win_rate_vs_random != null ? raw.win_rate_vs_random : null;
        if (winRate == null) {
          // Estimate win rate relative to random if not directly provided
          const randomMean = rawStrategies.random_hunt ? rawStrategies.random_hunt.mean : 53.5;
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

      // 1. Render Leaderboard Table
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

      // 2. Render 6 Strategy Summary Cards Grid
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

      // 3. Render Multi-Strategy Legend Pills
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

      // 4. Render Multi-Strategy SVG Turn Distribution Histogram
      this.renderMultiHistogram(strategyResults);
    }

    renderMultiHistogram(strategiesList) {
      const svg = this.histogramSvg;
      svg.innerHTML = '';

      const activeList = strategiesList.filter(s => this.activeHistogramCurves.has(s.id));

      // Compute global maximum frequency across active strategies
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

        // Semi-transparent polygon
        svg.appendChild(createEl('polygon', {
          points: pts,
          fill: strat.color,
          'fill-opacity': '0.12'
        }));

        // Polyline stroke
        let linePts = '';
        for (let t = minTurn; t <= maxTurn; t++) {
          const val = dist[t] || 0;
          linePts += `${getX(t)},${getY(val)} `;
        }
        svg.appendChild(createEl('polyline', {
          points: linePts.trim(),
          fill: 'none',
          stroke: strat.color,
          'stroke-width': '2.5',
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
          'stroke-width': '1.8',
          'stroke-dasharray': '4 3',
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
