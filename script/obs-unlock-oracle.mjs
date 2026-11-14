// Regenerates fixtures/obs-unlock.json from the upstream observation-unlock
// maths, so the Rust port is compared against the source of truth rather than
// hand-picked numbers. The state is the committed `fixtures/data.json` account:
// its research rows, magnifier pool and save context go through the upstream
// loader, so the fixture carries what the calculator's own page would show.
// Usage: node script/obs-unlock-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/obs-unlock-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const { buildSaveContext } = await import(join(source, 'js/save/context.js'));
const { computeObsUnlockPriority } = await import(join(source, 'js/analysis.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save, serverVars: {} });
const saveCtx = buildSaveContext();

// The same `state` the page posts to its worker (`js/workers/sim-worker.js`).
const state = {
  gl: saveCtx.gridLevels,
  so: saveCtx.shapeOverlay,
  il: saveCtx.insightLvs,
  occ: saveCtx.occFound,
  rLv: saveCtx.researchLevel,
  mMax: saveCtx.magMaxPerSlot,
  mOwned: saveCtx.magnifiersOwned,
  md: saveCtx.magData,
  ip: saveCtx.insightProgress,
  failedRolls: saveCtx.cachedFailedRolls,
};

const result = await computeObsUnlockPriority(
  () => {},
  state,
  saveCtx
);

// The mastery terms the roll bounds read, so a divergence points at the decode.
const mastery = saveCtx.researchMastery;
const masteryTotal = mastery?.unlocked
  ? mastery.levels.reduce((sum, level) => sum + level, 0) + (state.rLv - mastery.level)
  : 0;

writeFileSync(
  join(root, 'fixtures/obs-unlock.json'),
  `${JSON.stringify(
    {
      source: 'js/analysis.js (computeObsUnlockPriority), js/save/context.js',
      save: 'fixtures/data.json',
      state: {
        researchLevel: state.rLv,
        magnifiersOwned: state.mOwned,
        magMaxPerSlot: state.mMax,
        found: state.occ.filter((flag) => flag >= 1).length,
        masteryTotal,
      },
      summary: {
        currentTotal: result.currentTotal,
        minRoll: result.minRoll,
        maxRoll: result.maxRoll,
        rollsPerDay: result.rollsPerDay,
        rollsRemaining: result.rollsRemaining,
        failedRolls: result.failedRolls,
        smartEyeLv: result.smartEyeLv,
        smartEyeCap: result.smartEyeCap,
      },
      results: result.results.map((row) => ({
        idx: row.idx,
        name: row.name,
        rollThreshold: row.rollThreshold,
        requiredRLv: row.requiredRLv,
        canUseNow: row.canUseNow,
        pUnlockToday: row.pUnlockToday,
        expectedDays: row.expectedDays,
        expGain: row.expGain,
        newRate: row.newRate,
        score: row.score,
      })),
    },
    null,
    2
  )}\n`
);
console.log(
  `wrote fixtures/obs-unlock.json: ${result.results.length} undiscovered observations, ` +
    `rolls ${result.minRoll}–${result.maxRoll}, ${result.rollsPerDay}/day`
);
