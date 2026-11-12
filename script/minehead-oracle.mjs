// Regenerates crates/minehead/tests/fixtures/upstream.json from the upstream
// Depth Charge calculator, so the Rust port is tested against the source of
// truth rather than hand-picked numbers.
// Usage: node script/minehead-oracle.mjs <path-to-idleon-research-optimizer>
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/minehead-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const systems = await import(join(source, 'js/stats/systems/w7/minehead.js'));
const sim = await import(join(source, 'js/minehead/sim.js'));
const inferrer = await import(join(source, 'js/minehead/strategy-inferrer.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
const research = JSON.parse(save.Research);
const upgLevels = research[8].map(Number);
const stateR7 = research[7].map(Number);
const floor = stateR7[4] || 0;

// Formula table: the level rows the dashboard renders, the upgrade costs and
// the damage pipeline, evaluated over the fixture's own upgrade levels.
const levels = upgLevels.slice(0, 30);
const qtyLevels = levels.slice();
const formulas = {
  upgLvReq: levels.map((_, t) => systems.upgLvReq(t)),
  upgradeQTY: levels.map((lv, t) => systems.upgradeQTY(t, lv)),
  qty: levels.map((lv, t) => systems.upgradeQTY(t, lv, 2)),
  costs: levels.map((lv, t) => systems.upgCost(t, lv, levels[26] || 0, 1.25, 12)),
  canBuy: levels.map((lv, t) =>
    systems.canBuyUpg(t, lv, 5e9, levels[26] || 0, 1.25, 12)
  ),
  gridDims: Array.from({ length: 18 }, (_, lv) => systems.gridDims(lv)),
  goldTiles: systems.goldTilesTotal(qtyLevels, 3),
  blocks: systems.blocksTotal(qtyLevels),
  flags: systems.flagsTotal(qtyLevels),
  instas: systems.instaRevealsTotal(qtyLevels),
  dailyTries: systems.dailyTries(4),
  maxHP: systems.maxHPYou(qtyLevels),
  floors: Array.from({ length: 18 }, (_, f) => ({
    hp: systems.floorHP(f, 1.15),
    mines: systems.minesOnFloor(f, 1, 17),
  })),
  baseDamage: systems.baseDMG(qtyLevels, 37.5, 3, 12, 8, 25),
  perTilePct: systems.bonusDMGperTilePCT(qtyLevels, 21.5),
  bluecrownMulti: systems.bluecrownMulti(qtyLevels),
  bluecrownOdds: systems.bluecrownOdds(qtyLevels),
  jackpotOdds: systems.jackpotOdds(qtyLevels),
  jackpotTiles: systems.jackpotTiles(qtyLevels),
  outgoing: [0, 2, 5, 9, 12, 14, 19, 20, 23, 30, 41].map((value) =>
    systems.currentOutgoingDMG(
      [value], 1, false, qtyLevels, 37.5, 21.5, 10, 3, 12, 8, 25
    )
  ),
  wiggleMax: systems.wiggleMaxPerGame(4),
  glimbo: Array.from({ length: 6 }, (_, lv) => systems.glimboCost(3, lv, 1.4, 1)),
};

// Currency/hr over the fixture's account terms. The Rust crate decodes these
// from the save; here both sides are pinned to the same numbers so the port's
// arithmetic is what is compared.
const SHAPE_GRID_LEVELS = { 129: 40, 148: 12, 147: 7, 166: 5, 125: 3, 167: 22, 146: 9 };
const currency = systems.currencyPerHour({
  gridBonus129: 100,
  gridBonus148: 25,
  gridBonus147: 10,
  gridBonus166: 6,
  comp143: 1.5,
  comp143Level2: 1,
  bonusQTY6: 40,
  atom13: 2,
  mealMineCurr: 12,
  arcade62: 30,
  rogBonus12: 8,
  buttonBonus1: 5,
  eventShop44: 1,
  royalCurrencyMulti: 1.25,
  jellyCurrency8: 6,
  bundleJ: 1,
  taskCurrencyLevel: 2,
  dancingCoral5: 15,
  legend40: 25,
  doublers: [1, 22],
  upgLevels,
  highestDmg: stateR7[6] || 1,
});
// The shape ranking needs a shaped-able grid and the button presses, so it is
// pinned on a documented synthetic account rather than the fixture's absent
// grid levels.
const shapeSave = {
  ...save,
  allBonusMulti: 1,
  mineheadUpgLevels: upgLevels,
  gridLevels: Array.from({ length: 240 }, (_, i) => SHAPE_GRID_LEVELS[i] || 0),
  shapeOverlay: [],
  olaData: Array.from({ length: 600 }, (_, i) => (i === 594 ? 30 : 0)),
};
const shapeOpts = { tilesPerTurn: 5, mealMineCurr: 12 };
const shapeValues = systems.mineheadGridShapeValues(shapeSave, 30, shapeOpts);
const priority = systems.mineheadShapePriority(shapeSave, 30, shapeOpts);

// Seeded Monte Carlo: the fixture's levels on the saved floor and one boss
// ahead, with the default EV strategy. The Rust port reproduces these numbers
// trial-for-trial.
const gameOpts = {
  floor,
  upgLevels,
  gridBonus167: 37.5,
  gridBonus146: 21.5,
  gridBonus166_1: 4,
  wepPowDmgPCT: 10,
  sailing38: 3,
  dancingCoral5: 12,
  jellyDamage9: 8,
  jellyGoldTiles45: 3,
  svarHP: 1.15,
  mineReduction: 1,
  legend40: 25,
  legend40Pts: 17,
  doublers: [],
};
const monteCarlo = [floor, floor + 1].map((target) => ({
  floor: target,
  trials: 200,
  seed: 42 + target,
  result: sim.monteCarloFloor({ ...gameOpts, floor: target, nTrials: 200, seed: 42 + target }),
}));

// The optimizer over a small grid, so the search itself is compared.
const optimized = sim.optimizeStrategy({
  floor,
  upgLevels,
  screenTrials: 20,
  finalTrials: 30,
  topN: 2,
  seed: 7,
  grid: {
    evMultiplier: [0.8, 1.3],
    minReveal: [0, 1],
    mineCapPct: [1.0],
    goldenMinePct: [1.0],
    hpThreshold: [1.0],
    blockAggro: [true],
    commitMin: [0],
    lifeAggro: [1.0],
    turn1EvMul: [0],
    crownChase: [0],
    instaMode: [0],
  },
  gameOpts: { ...gameOpts },
});

// The Play decisions the inferrer is compared on: contexts the EV strategy
// scores, with the strategy's own answer recorded as the player's choice.
const _playStrat = sim.tunableStrategy({});
const decisions = [];
for (let i = 0; i < 40; i++) {
  const ctx = {
    safeRevealedThisTurn: i % 5,
    minesTotal: 12,
    minesRemaining: 6 - (i % 5),
    unrevealedCount: 30 - i,
    livesLeft: 3,
    totalDmgSoFar: i * 100,
    bossHP: 12_000,
    turnsPlayed: 1 + (i % 4),
    blocksLeft: 2,
    goldensOnGrid: i % 3,
    currentTurnDmg: i * 40,
    remainingHP: 12_000 - i * 100,
    perTilePct: 21.5,
    tileCount: i % 6,
    crownProgress: i % 4,
    lastTurnMineHit: i % 7 === 0,
  };
  decisions.push({
    ...ctx,
    choice: _playStrat(ctx),
    spatial: {
      idx: i,
      r: Math.floor(i / 10),
      c: i % 10,
      cols: 10,
      rows: 5,
      isCorner: i === 0,
      isEdge: i < 10,
      isCenter: i >= 10,
      adjMines: i % 3,
      adjSafe: i % 4,
      adjUnrevealed: 8,
      clickOrder: i % 5,
    },
  });
}
const inferred = inferrer.inferStrategy(decisions);

const payload = {
  source: 'fixtures/data.json Research rows 7 and 8',
  formulas,
  currency,
  shapeValues,
  priority,
  monteCarlo,
  optimized: {
    totalCombos: optimized.totalCombos,
    bestParams: optimized.bestParams,
    bestResult: optimized.bestResult,
    topResults: optimized.topResults,
  },
  decisions,
  inferred: {
    params: inferred.params,
    agreement: inferred.agreement,
    totalDecisions: inferred.totalDecisions,
    profile: inferred.profile,
    spatial: inferred.spatial,
  },
};

const out = join(root, 'crates/minehead/tests/fixtures');
mkdirSync(out, { recursive: true });
writeFileSync(join(out, 'upstream.json'), `${JSON.stringify(payload, null, 1)}\n`);
console.log(
  `wrote formulas, ${monteCarlo.length} Monte Carlo runs, ` +
    `${optimized.totalCombos} optimized combos`
);
