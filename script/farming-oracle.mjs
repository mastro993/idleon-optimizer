// Regenerates fixtures/farming.json from the upstream Farming planner, so the
// Rust port is tested against the source of truth rather than hand-picked
// numbers.
// Usage: node script/farming-oracle.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/farming-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const planner = await import(join(source, 'js/stats/systems/w6/farming-planner.js'));

// One purpose-built account. Every plot in a different family, one evolution
// blocked, one with overgrowth; crop quantities cross all five GMO thresholds;
// the saved Land Rank levels leave points unspent; night market 5 unlocks Land
// Rank XP; and the exotic rows behind the rank bonus and the fifth column are
// bought, so the allocator's own terms are exercised.
const row = (length, entries) => {
  const row = new Array(length).fill(0);
  for (const [index, value] of entries) row[index] = value;
  return row;
};
const plots = new Array(36).fill(null).map(() => [-1, 0, 0, 0, 0, 0, 0]);
plots[0] = [0, 1000, 2, 0, 1, 0, 0];
plots[1] = [1, 0, 4, 0, 3, 1, 0];
plots[2] = [3, 12000, 5, 0, 2, 0, 0];
plots[3] = [6, 0, 0, 0, 1, 4, 0];
plots[4] = [2, 0, 1, 1, 1, 2, 0];
plots[5] = [5, 0, 60, 0, 4, 0, 0];
const rankRanks = row(36, [
  [0, 40], [1, 60], [2, 90], [3, 110], [4, 150], [5, 180],
  [6, 220], [7, 260], [8, 300], [9, 240], [10, 300], [11, 350],
]);
const rankLevels = [
  50, 30, 40, 25, 1, 30, 40, 35, 20, 1,
  45, 30, 25, 20, 1, 50, 40, 30, 35, 1,
];
const spelunk = new Array(60).fill(null).map(() => []);
spelunk[50] = [5];
const lv0 = row(24, [[16, 60]]);
const save = {
  Research: JSON.stringify(new Array(48).fill(null).map(() => [])),
  FarmPlot: JSON.stringify(plots),
  FarmRank: JSON.stringify([rankRanks, row(36, []), rankLevels]),
  FarmUpg: JSON.stringify(
    row(100, [
      [2, 10], [3, 4], [5, 2], [6, 3], [9, 5],
      [11, 3], [12, 4], [13, 2], [15, 1], [16, 3],
      [25, 1], [34, 2],
    ]),
  ),
  FarmCrop: JSON.stringify({
    0: 150000, 1: 3000, 2: 1200, 3: 500, 21: 2500, 25: 60,
    46: 1200, 66: 11000, 84: 250, 107: 100, 167: 45, 230: 5,
  }),
  Spelunk: JSON.stringify(spelunk),
  GemItemsPurchased: JSON.stringify(row(150, [[139, 2]])),
  Lv0_0: JSON.stringify(lv0),
};

loadSaveData({ data: save, serverVars: {} });
const saveData = state.saveData;
const snapshot = planner.farmingAccountSnapshot(saveData, { activeCharIdx: 0 });

/** One objective's assignment and allocation, as the calculator builds them. */
function plan(objective, mode = 'unspent') {
  const targetCropId = 240;
  const assignment = planner.plotAssignmentPlan(saveData, snapshot, objective, { targetCropId });
  const allocation = planner.optimizeLandRank(saveData, 0, objective, { mode, targetCropId });
  return {
    objective,
    mode,
    assignment: {
      targetCropId: assignment.targetCropId,
      rows: assignment.rows.map(({ plotIdx, currentCropId, currentFamilyId, targetCropId, targetFamilyId, targetEvolution, changeNeeded }) => ({
        plot: plotIdx,
        currentCropId,
        currentFamilyId: Number.isFinite(currentFamilyId) ? currentFamilyId : -1,
        targetCropId,
        targetFamilyId,
        targetEvolution,
        changeNeeded,
      })),
    },
    allocation: {
      pointsAvailable: allocation.pointsAvailable,
      pointsSpent: allocation.pointsSpent,
      pointsUnused: allocation.pointsUnused,
      initialScore: allocation.initialScore,
      finalScore: allocation.finalScore,
      relativeGain: allocation.relativeGain,
      initialLevels: allocation.initialLevels,
      levels: allocation.levels,
      changes: allocation.changes.map(({ id, before, after, delta }) => ({ id, before, after, delta })),
    },
  };
}

const objectives = ['rankXp', 'farmingExp', 'evolution', 'overgrowth', 'cropValue', 'beans', 'growth'];
if (!planner.optimizeLandRank) throw new Error('the upstream planner does not allocate Land Rank');

const fixture = {
  source: 'purpose-built save in script/farming-oracle.mjs',
  modules: [
    'farming-calc.html',
    'js/stats/data/w5/farming.js',
    'js/stats/systems/w6/farming-planner.js',
  ],
  save,
  snapshot: {
    growthRate: snapshot.growthRate,
    nextCropCommon: snapshot.nextCropCommon,
    overgrowthCommon: snapshot.overgrowthCommon,
    farmingExp: snapshot.farmingExp,
    cropValueCap: snapshot.cropValueCap,
    productBonus: snapshot.productBonus,
    expectedVines: snapshot.expectedVines,
    plotRankDisplay: snapshot.plotRankDisplay,
    cropCounts: snapshot.cropCounts,
    rankBonuses: snapshot.rank.bonuses,
    rankEvolution: snapshot.rank.evolution,
    rankCropValue: snapshot.rank.cropValue,
    rankXp: snapshot.rank.rankXp,
    rankOvergrowth: snapshot.rank.overgrowth,
    rankFarmingExp: snapshot.rank.farmingExp,
    totalLandRank: planner.totalLandRank(saveData),
    pointsLeft: planner.landRankPointsLeft(saveData),
    fifthColumnMax: planner.landRankFifthColumnMax(saveData),
  },
  plans: [...objectives.map((objective) => plan(objective)), plan('farmingExp', 'reset')],
};

writeFileSync(join(root, 'fixtures/farming.json'), `${JSON.stringify(fixture, null, 2)}\n`);
console.log(`wrote ${fixture.plans.length} plans to fixtures/farming.json`);
