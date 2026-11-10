/**
 * BATTLESHIP COMMAND & STRATEGY LAB - CORE APPLICATION & CLIENT ENGINE
 * Dual-Mode: Full in-browser simulation engine + Rust HTTP API connector.
 */

(function () {
  'use strict';

  // =========================================================================
  // CONSTANTS & HASBRO RULES
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
      return {
        strategy: strategyType, // 'checkerboard' or 'random'
        mode: 'HUNT',           // 'HUNT' or 'TARGET'
        activeHits: [],         // Array of {r, c} hits belonging to unsunk ships
        targetQueue: [],        // Prioritized {r, c} coordinates to shoot next
        targetGrid: Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(CELL_STATE.UNKNOWN)),
        shotsCount: 0,
        hitsCount: 0,
        missesCount: 0,
        shipsSunkCount: 0
      };
    }

    /**
     * Selects the next shot coordinate for the given AI agent.
     */
    getNextShot(agent) {
      // 1. If in TARGET mode, try queue first
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

      if (agent.strategy === 'checkerboard') {
        // Collect all unshot parity cells (r + c) % 2 === 0
        const parityCandidates = [];
        for (let r = 0; r < GRID_SIZE; r++) {
          for (let c = 0; c < GRID_SIZE; c++) {
            if ((r + c) % 2 === 0 && agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
              parityCandidates.push({ r, c });
            }
          }
        }

        if (parityCandidates.length > 0) {
          const idx = Math.floor(this.rng() * parityCandidates.length);
          return parityCandidates[idx];
        }
      }

      // Random Hunt or fallback if parity cells exhausted
      const anyCandidates = [];
      for (let r = 0; r < GRID_SIZE; r++) {
        for (let c = 0; c < GRID_SIZE; c++) {
          if (agent.targetGrid[r][c] === CELL_STATE.UNKNOWN) {
            anyCandidates.push({ r, c });
          }
        }
      }

      if (anyCandidates.length > 0) {
        const idx = Math.floor(this.rng() * anyCandidates.length);
        return anyCandidates[idx];
      }

      return null; // All 100 cells shot
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
        // Add 4 orthogonal neighbors
        const { r, c } = hits[0];
        const deltas = [[-1, 0], [1, 0], [0, -1], [0, 1]];
        // Shuffle deltas for variety
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

      // Check if hits are collinear
      const sameRow = hits.every(h => h.r === hits[0].r);
      const sameCol = hits.every(h => h.c === hits[0].c);

      if (sameRow) {
        const row = hits[0].r;
        const cols = hits.map(h => h.c).sort((a, b) => a - b);
        const minCol = cols[0];
        const maxCol = cols[cols.length - 1];

        // Endpoints
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

        // Snapshot current AI mode & target queue BEFORE the shot
        const aiModeBefore = attacker.mode;
        const targetQueueSnapshot = attacker.targetQueue.map(c => ({ ...c }));

        const shot = this.getNextShot(attacker);
        if (!shot) break; // Grid full

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

          // Locate ship in defender's fleet
          hitShip = defenderFleet.find(s => s.id === targetShipId);
          if (hitShip) {
            hitShip.hits++;
            if (hitShip.hits >= hitShip.size) {
              result = 'SUNK';
              hitShip.sunk = true;
              sunkShip = hitShip.name;
              sunkCells = hitShip.cells.map(c => ({ ...c }));
              attacker.shipsSunkCount++;

              // Mark cells as SUNK on attacker's target grid
              for (const sc of hitShip.cells) {
                attacker.targetGrid[sc.r][sc.c] = CELL_STATE.SUNK;
              }

              // Remove sunk ship cells from active hits
              const sunkSet = new Set(hitShip.cells.map(c => `${c.r},${c.c}`));
              attacker.activeHits = attacker.activeHits.filter(h => !sunkSet.has(`${h.r},${h.c}`));

              // If activeHits is empty, reset to HUNT
              if (attacker.activeHits.length === 0) {
                attacker.mode = 'HUNT';
                attacker.targetQueue = [];
              } else {
                // Another hit ship was partially hit during targeting; rebuild queue
                this.rebuildTargetQueue(attacker);
              }
            } else {
              // Standard hit: rebuild/prioritize target queue
              this.rebuildTargetQueue(attacker);
            }
          }
        } else {
          result = 'MISS';
          attacker.missesCount++;
          attacker.targetGrid[shot.r][shot.c] = CELL_STATE.MISS;
        }

        // Record turn step
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

        // Check victory
        if (defenderFleet.every(s => s.sunk)) {
          winner = activePlayer;
          break;
        }

        // Toggle player
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
     * Runs N solo games for a strategy to measure turns-to-win distribution.
     */
    runSoloBenchmark(strategyType, numGames) {
      const turnCounts = [];

      for (let g = 0; g < numGames; g++) {
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
        turnCounts.push(turns);
      }

      return turnCounts;
    }

    /**
     * Runs head-to-head duels between Strategy 1 and Strategy 2.
     */
    runHeadToHead(numGames) {
      let p1Wins = 0; // Checkerboard
      let p2Wins = 0; // Random

      for (let g = 0; g < numGames; g++) {
        // P1 uses checkerboard, P2 uses random
        const match = this.simulateMatch('checkerboard', 'random');
        if (match.winner === 1) p1Wins++;
        else if (match.winner === 2) p2Wins++;
      }

      return {
        checkerboardWins: p1Wins,
        randomWins: p2Wins,
        games: numGames,
        checkerboardWinRate: ((p1Wins / numGames) * 100).toFixed(1),
        randomWinRate: ((p2Wins / numGames) * 100).toFixed(1)
      };
    }
  }

  // Math Statistics Utilities
  function computeStats(values) {
    if (!values || values.length === 0) {
      return { mean: 0, median: 0, min: 0, max: 0, stdDev: 0, p90: 0, distribution: {} };
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

    const p90Idx = Math.floor(n * 0.9);
    const p90 = sorted[Math.min(p90Idx, n - 1)];

    // Distribution map from 17 to 100
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
      this.currentTurnIndex = 0; // 0 is initial placement, 1..N are turns
      this.isPlaying = false;
      this.playbackSpeed = 1.0;
      this.playTimer = null;

      // View settings
      this.fogOfWar = true;
      this.showParity = true;
      this.showQueue = true;
      this.logFilterHitsOnly = false;

      // Engine status
      this.engineType = 'detecting'; // 'rust' or 'in-browser'

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

      // Benchmark Controls & Outputs
      this.benchmarkGameCount = document.getElementById('benchmark-game-count');
      this.benchmarkSeed = document.getElementById('benchmark-seed');
      this.btnRunBenchmark = document.getElementById('btn-run-benchmark');
      this.runBenchmarkText = document.getElementById('run-benchmark-text');
      this.benchmarkProgressContainer = document.getElementById('benchmark-progress-container');
      this.benchmarkProgressBar = document.getElementById('benchmark-progress-bar');
      this.benchmarkProgressStatus = document.getElementById('benchmark-progress-status');
      this.benchmarkProgressPercent = document.getElementById('benchmark-progress-percent');

      // Metric elements
      this.metricStrat1Mean = document.getElementById('metric-strat1-mean');
      this.metricStrat1Median = document.getElementById('metric-strat1-median');
      this.metricStrat1Min = document.getElementById('metric-strat1-min');
      this.metricStrat1Max = document.getElementById('metric-strat1-max');
      this.metricStrat1Std = document.getElementById('metric-strat1-std');
      this.metricStrat1P90 = document.getElementById('metric-strat1-p90');

      this.metricStrat2Mean = document.getElementById('metric-strat2-mean');
      this.metricStrat2Median = document.getElementById('metric-strat2-median');
      this.metricStrat2Min = document.getElementById('metric-strat2-min');
      this.metricStrat2Max = document.getElementById('metric-strat2-max');
      this.metricStrat2Std = document.getElementById('metric-strat2-std');
      this.metricStrat2P90 = document.getElementById('metric-strat2-p90');

      this.h2hS2Winrate = document.getElementById('h2h-s2-winrate');
      this.h2hS2Wincount = document.getElementById('h2h-s2-wincount');
      this.h2hS1Winrate = document.getElementById('h2h-s1-winrate');
      this.h2hS1Wincount = document.getElementById('h2h-s1-wincount');
      this.h2hBarS2 = document.getElementById('h2h-bar-s2');
      this.h2hBarS1 = document.getElementById('h2h-bar-s1');
      this.h2hSpeedupCallout = document.getElementById('h2h-speedup-callout');

      // Histogram
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

    /**
     * Initializes the 10x10 DOM grids with coordinate headers (A-J rows, 1-10 columns).
     */
    setupGrids() {
      const buildGrid = (container, playerNum) => {
        container.innerHTML = '';
        container.classList.add('show-parity', 'show-queue');

        // Top-left blank corner
        const corner = document.createElement('div');
        corner.className = 'grid-label';
        container.appendChild(corner);

        // Column labels 1 - 10
        for (let c = 1; c <= GRID_SIZE; c++) {
          const colLabel = document.createElement('div');
          colLabel.className = 'grid-label col-label';
          colLabel.textContent = c;
          container.appendChild(colLabel);
        }

        // Rows A - J + 10 cells per row
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
      // Tab Navigation
      this.tabBtnReplay.addEventListener('click', () => this.switchTab('replay'));
      this.tabBtnBenchmark.addEventListener('click', () => this.switchTab('benchmark'));

      // New Match & Seed
      this.btnNewMatch.addEventListener('click', () => this.loadNewMatch());
      this.btnRandomSeed.addEventListener('click', () => {
        this.inputMatchSeed.value = Math.floor(Math.random() * 999999) + 1;
        this.loadNewMatch();
      });

      // Toggles
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

      // Playback Controls
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

      // Speed selection
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

      // Battle Log Filters
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

      // Benchmark Run
      this.btnRunBenchmark.addEventListener('click', () => this.runBenchmark());

      // Modal
      this.modalBtnDismiss.addEventListener('click', () => {
        this.winnerModal.style.display = 'none';
      });
      this.modalBtnReplay.addEventListener('click', () => {
        this.winnerModal.style.display = 'none';
        this.jumpToTurn(0);
        this.play();
      });

      // Preset selector
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
        // If benchmark not run yet, render blank or sample benchmark
        if (window.BATTLESHIP_SAMPLE_DATA && window.BATTLESHIP_SAMPLE_DATA.benchmark) {
          this.displayBenchmarkResults(window.BATTLESHIP_SAMPLE_DATA.benchmark);
        }
      }
    }

    /**
     * Detects if Rust HTTP Server is active at `/api/health` or `/api/match`.
     */
    async detectEngine() {
      this.engineStatusText.textContent = 'Probing Engine...';
      try {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 1200);

        const res = await fetch('/api/match?p1=checkerboard&p2=random&seed=1', {
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
        console.log('[Battleship Lab] Rust server not reachable; using autonomous in-browser engine.', err.message);
      }

      // Check for sample data
      this.checkSampleData();

      // Load initial match
      this.loadNewMatch();
    }

    checkSampleData() {
      // If sample data wasn't provided externally, generate default precomputed presets
      if (!window.BATTLESHIP_SAMPLE_DATA || !window.BATTLESHIP_SAMPLE_DATA.sample_matches) {
        const eng42 = new BattleshipEngine(42);
        const m1 = eng42.simulateMatch('checkerboard', 'random');
        const eng101 = new BattleshipEngine(101);
        const m2 = eng101.simulateMatch('random', 'checkerboard');
        const eng777 = new BattleshipEngine(777);
        const m3 = eng777.simulateMatch('checkerboard', 'checkerboard');

        window.BATTLESHIP_SAMPLE_DATA = {
          sample_matches: [m1, m2, m3]
        };
      }

      if (window.BATTLESHIP_SAMPLE_DATA && window.BATTLESHIP_SAMPLE_DATA.sample_matches && window.BATTLESHIP_SAMPLE_DATA.sample_matches.length > 0) {
        this.presetContainer.style.display = 'flex';
        this.selectPresetMatch.innerHTML = '<option value="">-- Choose Sample Match --</option>';
        window.BATTLESHIP_SAMPLE_DATA.sample_matches.forEach((m, idx) => {
          const opt = document.createElement('option');
          opt.value = idx;
          const p1 = m.p1_strategy === 'checkerboard' ? 'Checkerboard' : 'Random';
          const p2 = m.p2_strategy === 'checkerboard' ? 'Checkerboard' : 'Random';
          opt.textContent = `Sample #${idx + 1}: ${p1} vs ${p2} (Seed ${m.seed || idx + 1})`;
          this.selectPresetMatch.appendChild(opt);
        });
      }
    }

    /**
     * Loads or simulates a new match based on selected strategies and seed.
     */
    async loadNewMatch() {
      this.pause();
      const p1Strat = this.selectP1Strategy.value;
      const p2Strat = this.selectP2Strategy.value;
      const seed = parseInt(this.inputMatchSeed.value, 10) || Math.floor(Math.random() * 999999);

      this.p1StrategyLabel.textContent = p1Strat === 'checkerboard' ? 'Checkerboard + Hunt/Target' : 'Random + Hunt/Target';
      this.p2StrategyLabel.textContent = p2Strat === 'checkerboard' ? 'Checkerboard + Hunt/Target' : 'Random + Hunt/Target';

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

      // In-browser simulation fallback / default
      const engine = new BattleshipEngine(seed);
      const matchData = engine.simulateMatch(p1Strat, p2Strat);
      this.setMatch(matchData);
    }

    /**
     * Sets the loaded match data and initializes visual state.
     */
    setMatch(match) {
      this.currentMatch = match;
      this.currentTurnIndex = 0;

      // Update Scrubber
      this.timelineSlider.min = 0;
      this.timelineSlider.max = match.turns.length;
      this.timelineSlider.value = 0;
      this.scrubberCurrentTurn.textContent = 'Turn 0';
      this.scrubberMaxTurn.textContent = `Max: ${match.turns.length}`;

      // Build Fleet status panels
      this.renderFleetPanels();

      // Render Battle Log
      this.renderBattleLog();

      // Render initial board state (Turn 0)
      this.renderTurn(0);
    }

    /**
     * Renders the Fleet panels for Player 1 and Player 2.
     */
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

    /**
     * Reconstructs the game state at turn `turnIdx` and updates the entire UI.
     */
    renderTurn(turnIdx) {
      if (!this.currentMatch) return;
      this.currentTurnIndex = turnIdx;
      this.timelineSlider.value = turnIdx;
      this.scrubberCurrentTurn.textContent = `Turn ${turnIdx}`;

      const totalTurns = this.currentMatch.turns.length;
      const turnsSlice = this.currentMatch.turns.slice(0, turnIdx);
      const lastTurn = turnIdx > 0 ? this.currentMatch.turns[turnIdx - 1] : null;

      // Reset Board Arrays
      // p1TargetGrid: shots P1 has fired at P2 waters
      // p2TargetGrid: shots P2 has fired at P1 waters
      const p1GridState = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(CELL_STATE.UNKNOWN));
      const p2GridState = Array.from({ length: GRID_SIZE }, () => Array(GRID_SIZE).fill(CELL_STATE.UNKNOWN));

      // Sunk cells sets
      const p1SunkCells = new Set(); // cells of P2's fleet sunk by P1
      const p2SunkCells = new Set(); // cells of P1's fleet sunk by P2

      // Ship hits tracking
      const p1ShipHits = {};
      const p2ShipHits = {};
      this.currentMatch.p1_fleet.forEach(s => p1ShipHits[s.id] = 0);
      this.currentMatch.p2_fleet.forEach(s => p2ShipHits[s.id] = 0);

      // Replay all shots up to turnIdx
      for (const t of turnsSlice) {
        if (t.player === 1) {
          // P1 shooting at P2
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
          // P2 shooting at P1
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

      // Fleet placement lookup maps
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

      // Candidate queue cells for active player
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

      // Render Cells for Grid 1 (P1 targeting P2's fleet)
      this.updateGridDom(this.gridP1El, p1GridState, p2ShipMap, p1SunkCells, activeQueueP1, lastTurn && lastTurn.player === 1 ? lastTurn : null);

      // Render Cells for Grid 2 (P2 targeting P1's fleet)
      this.updateGridDom(this.gridP2El, p2GridState, p1ShipMap, p2SunkCells, activeQueueP2, lastTurn && lastTurn.player === 2 ? lastTurn : null);

      // Update Fleet Panel Pips and Badges
      this.updateFleetUi(this.p1FleetList, this.currentMatch.p1_fleet, p1ShipHits, this.p1FleetAfloatCount);
      this.updateFleetUi(this.p2FleetList, this.currentMatch.p2_fleet, p2ShipHits, this.p2FleetAfloatCount);

      // Update Telemetry & AI Mode
      this.updateTelemetry(lastTurn, turnIdx);

      // Update Banner
      this.updateBanner(lastTurn, turnIdx, totalTurns);

      // Highlight active turn in battle log
      this.highlightLogStep(turnIdx);

      // If at end of game and playing, pause and show victory modal
      if (turnIdx === totalTurns && totalTurns > 0) {
        this.pause();
        if (this.currentMatch.winner) {
          this.showWinnerModal();
        }
      }
    }

    /**
     * Updates DOM cells of a single radar grid.
     */
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

        // Reset state classes
        cell.className = 'grid-cell';
        if ((r + c) % 2 === 0) cell.classList.add('parity-cell');
        if (inQueue) cell.classList.add('in-queue');
        if (isLastShot) cell.classList.add('last-shot-cell');

        const peg = cell.querySelector('.peg-marker');
        const letter = cell.querySelector('.ship-marker-letter');
        peg.textContent = '';
        letter.textContent = '';

        // Ship presence (Visible if Fog of War is OFF, or if cell is HIT/SUNK)
        const isRevealed = !this.fogOfWar || state === CELL_STATE.HIT || state === CELL_STATE.SUNK;

        if (ship && isRevealed) {
          cell.classList.add(ship.color);
          letter.textContent = ship.letter;
        }

        // Hit / Miss / Sunk peg styling
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

    /**
     * Updates fleet hit pips and afloat badges.
     */
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

    /**
     * Updates live telemetry readouts.
     */
    updateTelemetry(lastTurn, turnIdx) {
      if (!lastTurn) {
        // Turn 0
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

      // Mode badges
      this.p1AiBadge.className = `ai-mode-badge ${p1s.mode.toLowerCase()}`;
      this.p1AiBadge.textContent = `${p1s.mode} MODE`;
      this.p2AiBadge.className = `ai-mode-badge ${p2s.mode.toLowerCase()}`;
      this.p2AiBadge.textContent = `${p2s.mode} MODE`;

      this.p1QueueCount.textContent = `Queue: ${p1s.queue_len}`;
      this.p2QueueCount.textContent = `Queue: ${p2s.queue_len}`;
    }

    /**
     * Updates top action banner.
     */
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

    /**
     * Populates the chronological battle log.
     */
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
      const stratName = m.winner === 1 ? m.p1_strategy : m.p2_strategy;
      const stratFormatted = stratName === 'checkerboard' ? 'Checkerboard + Hunt/Target' : 'Random + Hunt/Target';

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

    // Playback Engine
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
    // STRATEGY BENCHMARK LAB
    // =========================================================================

    async runBenchmark() {
      const numGames = parseInt(this.benchmarkGameCount.value, 10) || 500;
      const seed = parseInt(this.benchmarkSeed.value, 10) || 1337;

      this.btnRunBenchmark.disabled = true;
      this.runBenchmarkText.textContent = 'Simulating...';
      this.benchmarkProgressContainer.style.display = 'block';
      this.benchmarkProgressBar.style.width = '0%';
      this.benchmarkProgressPercent.textContent = '0%';

      // Check if Rust HTTP server benchmark is available
      if (this.engineType === 'rust') {
        try {
          this.benchmarkProgressStatus.textContent = 'Fetching benchmark from Rust server...';
          this.benchmarkProgressBar.style.width = '60%';
          const res = await fetch(`/api/benchmark?games=${numGames}&seed=${seed}`);
          if (res.ok) {
            const data = await res.json();
            this.benchmarkProgressBar.style.width = '100%';
            this.benchmarkProgressPercent.textContent = '100%';
            this.displayBenchmarkResults(data);
            this.finishBenchmark();
            return;
          }
        } catch (e) {
          console.warn('Rust benchmark endpoint failed; running in-browser fallback', e);
        }
      }

      // In-browser simulation with chunked execution for 60fps UI
      const engine = new BattleshipEngine(seed);
      const s1Turns = [];
      const s2Turns = [];
      let s2Wins = 0;
      let s1Wins = 0;

      const chunkSize = 25;
      let gamesProcessed = 0;

      const processBatch = () => {
        const batchEnd = Math.min(numGames, gamesProcessed + chunkSize);

        for (let g = gamesProcessed; g < batchEnd; g++) {
          // Solo benchmark game for Strategy 1 (Random)
          const r1 = engine.runSoloBenchmark('random', 1);
          s1Turns.push(r1[0]);

          // Solo benchmark game for Strategy 2 (Checkerboard)
          const r2 = engine.runSoloBenchmark('checkerboard', 1);
          s2Turns.push(r2[0]);

          // Head-to-Head Duel
          const duel = engine.simulateMatch('checkerboard', 'random');
          if (duel.winner === 1) s2Wins++;
          else if (duel.winner === 2) s1Wins++;
        }

        gamesProcessed = batchEnd;
        const pct = Math.floor((gamesProcessed / numGames) * 100);
        this.benchmarkProgressBar.style.width = `${pct}%`;
        this.benchmarkProgressPercent.textContent = `${pct}%`;
        this.benchmarkProgressStatus.textContent = `Simulated ${gamesProcessed} / ${numGames} matches...`;

        if (gamesProcessed < numGames) {
          setTimeout(processBatch, 4);
        } else {
          // Complete
          const strat1Stats = computeStats(s1Turns);
          const strat2Stats = computeStats(s2Turns);
          const benchmarkResult = {
            games: numGames,
            seed: seed,
            strategies: {
              random_hunt: strat1Stats,
              checkerboard_hunt: strat2Stats
            },
            head_to_head: {
              checkerboard_wins: s2Wins,
              random_wins: s1Wins,
              checkerboard_win_rate: Number(((s2Wins / numGames) * 100).toFixed(1)),
              random_win_rate: Number(((s1Wins / numGames) * 100).toFixed(1))
            }
          };

          this.displayBenchmarkResults(benchmarkResult);
          this.finishBenchmark();
        }
      };

      setTimeout(processBatch, 10);
    }

    finishBenchmark() {
      this.btnRunBenchmark.disabled = false;
      this.runBenchmarkText.textContent = 'Run Strategy Benchmark';
      setTimeout(() => {
        this.benchmarkProgressContainer.style.display = 'none';
      }, 800);
    }

    displayBenchmarkResults(data) {
      // Normalize data formats (flexible to Rust and JS representations)
      const s1 = data.strategies.random_hunt || data.strategies.random || {};
      const s2 = data.strategies.checkerboard_hunt || data.strategies.checkerboard || {};
      const h2h = data.head_to_head || {};

      // Strategy 1 (Random) metrics
      this.metricStrat1Mean.textContent = s1.mean != null ? `${s1.mean} turns` : '--';
      this.metricStrat1Median.textContent = s1.median != null ? `${s1.median} turns` : '--';
      this.metricStrat1Min.textContent = s1.min != null ? `${s1.min} turns` : '--';
      this.metricStrat1Max.textContent = s1.max != null ? `${s1.max} turns` : '--';
      this.metricStrat1Std.textContent = s1.std_dev != null ? s1.std_dev : (s1.stdDev != null ? s1.stdDev : '--');
      this.metricStrat1P90.textContent = s1.p90 != null ? `${s1.p90} turns` : '--';

      // Strategy 2 (Checkerboard) metrics
      this.metricStrat2Mean.textContent = s2.mean != null ? `${s2.mean} turns` : '--';
      this.metricStrat2Median.textContent = s2.median != null ? `${s2.median} turns` : '--';
      this.metricStrat2Min.textContent = s2.min != null ? `${s2.min} turns` : '--';
      this.metricStrat2Max.textContent = s2.max != null ? `${s2.max} turns` : '--';
      this.metricStrat2Std.textContent = s2.std_dev != null ? s2.std_dev : (s2.stdDev != null ? s2.stdDev : '--');
      this.metricStrat2P90.textContent = s2.p90 != null ? `${s2.p90} turns` : '--';

      // Head-to-Head
      const s2Rate = h2h.checkerboard_win_rate != null ? h2h.checkerboard_win_rate : (h2h.p1_win_rate || 50);
      const s1Rate = h2h.random_win_rate != null ? h2h.random_win_rate : (h2h.p2_win_rate || 50);
      const s2Wins = h2h.checkerboard_wins != null ? h2h.checkerboard_wins : (h2h.p1_wins || 0);
      const s1Wins = h2h.random_wins != null ? h2h.random_wins : (h2h.p2_wins || 0);

      this.h2hS2Winrate.textContent = `${s2Rate}%`;
      this.h2hS2Wincount.textContent = `${s2Wins} wins`;
      this.h2hS1Winrate.textContent = `${s1Rate}%`;
      this.h2hS1Wincount.textContent = `${s1Wins} wins`;

      this.h2hBarS2.style.width = `${s2Rate}%`;
      this.h2hBarS1.style.width = `${s1Rate}%`;

      const delta = (s1.mean - s2.mean).toFixed(1);
      this.h2hSpeedupCallout.innerHTML = `Checkerboard finishes on average <strong>${delta} turns faster</strong>, securing a decisive <strong>${s2Rate}% win rate</strong> in simultaneous head-to-head duels!`;

      // Render Distribution Histogram
      this.renderHistogram(s1, s2);
    }

    /**
     * Renders responsive SVG Turn-to-Win Distribution Histogram.
     */
    renderHistogram(s1, s2) {
      const svg = this.histogramSvg;
      svg.innerHTML = '';

      const d1 = s1.distribution || {};
      const d2 = s2.distribution || {};

      // Calculate max frequency
      let maxFreq = 1;
      for (let t = 17; t <= 100; t++) {
        if (d1[t] && d1[t] > maxFreq) maxFreq = d1[t];
        if (d2[t] && d2[t] > maxFreq) maxFreq = d2[t];
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
      const barStep = plotW / turnSpan;

      const getX = (t) => padLeft + (t - minTurn) * barStep;
      const getY = (val) => padTop + plotH - (val / maxFreq) * plotH;

      // Create SVG namespace helper
      const createEl = (tag, attrs) => {
        const el = document.createElementNS('http://www.w3.org/2000/svg', tag);
        for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
        return el;
      };

      // Background grid lines (horizontal)
      const gridLevels = 5;
      for (let i = 0; i <= gridLevels; i++) {
        const val = Math.round((maxFreq / gridLevels) * i);
        const y = padTop + plotH - (i / gridLevels) * plotH;

        // Line
        svg.appendChild(createEl('line', {
          x1: padLeft,
          y1: y,
          x2: padLeft + plotW,
          y2: y,
          stroke: '#1e293b',
          'stroke-width': '1',
          'stroke-dasharray': i === 0 ? 'none' : '3 3'
        }));

        // Y-axis label
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

      // X-axis ticks & labels every 10 turns (20, 30, 40, ..., 100)
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

      // X-axis label
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

      // Draw Distribution Bars / Polygons
      const makeAreaPoints = (dist) => {
        let pts = `${padLeft},${padTop + plotH} `;
        for (let t = minTurn; t <= maxTurn; t++) {
          const val = dist[t] || 0;
          pts += `${getX(t)},${getY(val)} `;
        }
        pts += `${padLeft + plotW},${padTop + plotH}`;
        return pts;
      };

      // Strategy 1 (Random, Orange) Area & Stroke
      const s1Points = makeAreaPoints(d1);
      svg.appendChild(createEl('polygon', {
        points: s1Points,
        fill: 'rgba(249, 115, 22, 0.28)'
      }));

      // Strategy 2 (Checkerboard, Cyan) Area & Stroke
      const s2Points = makeAreaPoints(d2);
      svg.appendChild(createEl('polygon', {
        points: s2Points,
        fill: 'rgba(56, 189, 248, 0.32)'
      }));

      // Individual bars with hover events
      const barW = Math.max(2, (barStep * 0.85));
      for (let t = minTurn; t <= maxTurn; t++) {
        const v1 = d1[t] || 0;
        const v2 = d2[t] || 0;
        const x = getX(t) - barW / 2;

        if (v1 > 0) {
          const y1 = getY(v1);
          const h1 = (padTop + plotH) - y1;
          const rect1 = createEl('rect', {
            x: x,
            y: y1,
            width: barW / 2,
            height: h1,
            fill: '#f97316',
            rx: '1',
            opacity: '0.85'
          });
          svg.appendChild(rect1);
        }

        if (v2 > 0) {
          const y2 = getY(v2);
          const h2 = (padTop + plotH) - y2;
          const rect2 = createEl('rect', {
            x: x + barW / 2,
            y: y2,
            width: barW / 2,
            height: h2,
            fill: '#38bdf8',
            rx: '1',
            opacity: '0.85'
          });
          svg.appendChild(rect2);
        }

        // Invisible overlay for clean hover tooltips
        const trigger = createEl('rect', {
          x: x,
          y: padTop,
          width: barW,
          height: plotH,
          fill: 'transparent',
          style: 'cursor: pointer;'
        });

        trigger.addEventListener('mouseenter', (evt) => {
          this.histTooltip.style.display = 'block';
          this.histTooltip.innerHTML = `
            <div><strong>Turn ${t}</strong></div>
            <div style="color: #f97316;">Random Hunt: ${v1} games</div>
            <div style="color: #38bdf8;">Checkerboard: ${v2} games</div>
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

      // Mean dashed vertical lines
      if (s1.mean) {
        const x1 = getX(s1.mean);
        svg.appendChild(createEl('line', {
          x1: x1,
          y1: padTop,
          x2: x1,
          y2: padTop + plotH,
          stroke: '#f97316',
          'stroke-width': '2',
          'stroke-dasharray': '5 4'
        }));
        const t1 = createEl('text', {
          x: x1,
          y: padTop - 8,
          fill: '#f97316',
          'font-family': 'monospace',
          'font-size': '11',
          'text-anchor': 'middle',
          'font-weight': 'bold'
        });
        t1.textContent = `μ1 = ${s1.mean}`;
        svg.appendChild(t1);
      }

      if (s2.mean) {
        const x2 = getX(s2.mean);
        svg.appendChild(createEl('line', {
          x1: x2,
          y1: padTop,
          x2: x2,
          y2: padTop + plotH,
          stroke: '#38bdf8',
          'stroke-width': '2',
          'stroke-dasharray': '5 4'
        }));
        const t2 = createEl('text', {
          x: x2,
          y: padTop - 8,
          fill: '#38bdf8',
          'font-family': 'monospace',
          'font-size': '11',
          'text-anchor': 'middle',
          'font-weight': 'bold'
        });
        t2.textContent = `μ2 = ${s2.mean}`;
        svg.appendChild(t2);
      }
    }
  }

  // Launch application on DOM ready
  window.addEventListener('DOMContentLoaded', () => {
    window.BATTLESHIP_APP = new BattleshipApp();
  });

})();
