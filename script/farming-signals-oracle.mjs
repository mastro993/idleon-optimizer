// Regenerates fixtures/farming-signals.json from the upstream Farming Planner, so the
// Rust port is tested against the upstream calculator's own output.
//
// Usage: node script/farming-signals-oracle.mjs <path-to-idleon-research-optimizer>
//
// The generation command in the repo notes clones the upstream JS and runs this
// against that checkout. The fixture carries its own purpose-built save: the
// upstream account snapshot reads a dozen systems this port deliberately does
// not apply, and with those systems absent from the save they fall back to
// their neutral values, so the two calculators agree term for term. Feeding the
// full `fixtures/data.json` account instead would compare the port against
// systems it does not claim to model.
import { spawnSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/farming-signals-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));
const load = (path) => import(join(source, path));

const { loadSaveData } = await load('js/save/loader.js');
const { saveData } = await load('js/state.js');
const planner = await load('js/stats/systems/w6/farming-planner.js');
const { computeCropSC } = await load('js/stats/systems/w6/farming.js');
const { seedFamilyCount } = await load('js/stats/data/w5/farming.js');

// ===== The purpose-built save =====
// One plot per seed family plus an empty field cell, a saved overgrowth and
// evolution depth, Land Rank points, market levels, the Crop Scientist account
// terms (emporium encoding `AxyzwBCLNR` = items 27, 24, 25, 26, 23, 28, 29,
// 38, 40 and 44) and the
// organic labels exotic bonuses read.
const numbers = (count, value) => Array.from({ length: count }, () => value);
const plots = Array.from({ length: 36 }, (_, index) => [
  index === 35 ? -1 : index % 7,
  3000 * (index % 5),
  index % 3,
  index % 11 === 4 ? 1 : 0,
  index % 4,
  index % 3,
  120 + index,
]);
const save = {
  FarmPlot: JSON.stringify(plots),
  FarmRank: JSON.stringify([
    Array.from({ length: 36 }, (_, index) => index % 3),
    Array.from({ length: 36 }, (_, index) => 40 * index),
    Array.from({ length: 20 }, (_, index) => index % 3),
  ]),
  FarmUpg: JSON.stringify(
    Array.from({ length: 100 }, (_, index) =>
      index === 13 ? 500 : index === 11 ? 0 : index < 3 ? 12 : index % 4
    )
  ),
  FarmCrop: JSON.stringify(
    Object.fromEntries(
      Array.from({ length: 260 }, (_, crop) => [crop, 300 + 1000 * crop])
    )
  ),
  Lv0_0: JSON.stringify(Array.from({ length: 25 }, (_, index) => (index === 16 ? 75 : index === 20 ? 30 : 1))),
  Ninja: JSON.stringify(
    Array.from({ length: 110 }, (_, row) =>
      row === 102 ? Array.from({ length: 12 }, (_, cell) => (cell === 9 ? 'AxyzwBCLNR' : 0)) : []
    )
  ),
  Grimoire: JSON.stringify(Array.from({ length: 40 }, (_, index) => (index === 22 ? 12 : index === 36 ? 30 : 0))),
  UpgVault: JSON.stringify(Array.from({ length: 100 }, (_, index) => (index === 79 ? 18 : index === 89 ? 20 : 0))),
  Spelunk: JSON.stringify(Array.from({ length: 51 }, (_, index) => (index === 50 ? [3, 36] : 0))),
  GemItemsPurchased: JSON.stringify(Array.from({ length: 200 }, (_, index) => (index === 139 ? 2 : 0))),
  Research: JSON.stringify(Array.from({ length: 9 }, () => [])),
};

loadSaveData({ data: save, serverVars: {} });
if (saveData.farmCropCount === 0) throw new Error('the loader read no crops from the save');

// ===== The upstream calculator =====
const snapshot = planner.farmingAccountSnapshot(saveData, { activeCharIdx: 0 });
const diagnostics = planner.farmingDiagnostics(saveData, snapshot);
const breakpoints = planner.growthBreakpointRows(saveData, snapshot);
const metrics = planner.allPlotMetrics(saveData, snapshot);
const curves = Array.from({ length: 36 }, (_, plot) =>
  planner.optimalOvergrowthTarget(plot, saveData, snapshot)
);

/** `Infinity` has no JSON form; `null` is the fixture's word for it. */
const finite = (value) => (Number.isFinite(Number(value)) ? Number(value) : null);

const payload = {
  source: `the upstream js/stats/systems/w6/farming-planner.js and farming.js at ${'Corgan/idleon-research-optimizer'}`,
  save,
  snapshot: {
    farmLevel: snapshot.farmLevel,
    growthRate: snapshot.growthRate,
    nextCropCommon: snapshot.nextCropCommon,
    overgrowthCommon: snapshot.overgrowthCommon,
    farmingExp: snapshot.farmingExp,
    beanMultiplier: snapshot.beanMultiplier,
    cropValueCap: snapshot.cropValueCap,
    productBonus: snapshot.productBonus,
    expectedVines: snapshot.expectedVines,
    plotRankDisplay: snapshot.plotRankDisplay,
    voting29: snapshot.voting29,
    cropCounts: snapshot.cropCounts,
    rank: {
      bonuses: snapshot.rank.bonuses,
      evolution: snapshot.rank.evolution,
      cropValue: snapshot.rank.cropValue,
      rankXp: snapshot.rank.rankXp,
      overgrowth: snapshot.rank.overgrowth,
      farmingExp: snapshot.rank.farmingExp,
    },
  },
  diagnostics: {
    cropCount: diagnostics.cropCount,
    cropValueCap: diagnostics.cropValueCap,
    cappedPlots: diagnostics.cappedPlots,
    allPlotsCapped: diagnostics.allPlotsCapped,
    ordinaryGrowthAtActiveCap: diagnostics.ordinaryGrowthAtActiveCap,
    medalPlotCount: diagnostics.medalPlotCount,
    guaranteedEvolutionPlots: diagnostics.guaranteedEvolutionPlots,
    unspentRankPoints: diagnostics.unspentRankPoints,
    fifthColumnMax: diagnostics.fifthColumnMax,
    gmoCounts: diagnostics.gmoCounts,
  },
  growthBreakpoints: breakpoints.map((row) => ({
    familyId: row.familyId,
    name: row.name,
    requirement: row.requirement,
    rawSeconds: finite(row.rawSeconds),
    activeSeconds: finite(row.activeSeconds),
    oneSecondRate: finite(row.oneSecondRate),
    atActiveCap: row.atActiveCap,
    medalFixed: row.medalFixed,
  })),
  plots: metrics.map((row) => ({
    plotIdx: row.plotIdx,
    cropId: row.cropId,
    growthRequirement: row.growthRequirement,
    growthSeconds: finite(row.growthSeconds),
    nextCropChance: row.nextCropChance,
    nextOvergrowthChance: row.nextOvergrowthChance,
    farmingExp: row.farmingExp,
    rankXp: row.rankXp,
    rankRequirement: row.rankRequirement,
    cropValue: {
      base: row.cropValue.base,
      raw: row.cropValue.raw,
      cap: row.cropValue.cap,
      value: row.cropValue.value,
      capped: row.cropValue.capped,
      expected: row.cropValue.expected,
      expectedHarvest: row.cropValue.expectedHarvest,
    },
  })),
  curves: curves.map((curve) => ({
    plotIdx: curve.plotIdx,
    currentOg: curve.currentOg,
    cycleSeconds: finite(curve.cycleSeconds),
    bestTargetOg: curve.best.targetOg,
    rows: curve.rows.map((row) => [
      row.targetOg,
      row.rewardMultiplier,
      finite(row.repeatSeconds),
      finite(row.longRunRate),
      finite(row.etaFromCurrent),
    ]),
  })),
  cropScientist: Array.from({ length: 10 }, (_, index) => ({
    id: index,
    value: computeCropSC(index, saveData),
  })),
};

if (payload.growthBreakpoints.length !== seedFamilyCount()) {
  throw new Error('the upstream growth breakpoints changed shape');
}

writeFileSync(join(root, 'fixtures/farming-signals.json'), `${JSON.stringify(payload, null, 2)}\n`);
console.log(
  `wrote fixtures/farming-signals.json: ${payload.plots.length} plots, ` +
    `${payload.curves.length} curves, ${payload.cropScientist.length} Crop Scientist rows`
);
