// Writes the upstream Royal Guardian figures for the committed synthetic save,
// so `crates/core/tests/royal_guardian.rs` compares the Rust port against the JS.
//
// Usage: node script/royal-oracle.mjs <path-to-idleon-research-optimizer>
//
// The clearing rate multiplies talent 231's *purchased* level by the Orblet,
// Spelunk and Jelly factors. Upstream raises that level with the all-talent
// bonus levels, which the Rust port does not decode yet, so the oracle reports
// the rate both ways: `xtraClearKillz` as upstream computes it, and
// `xtraClearKillzPurchasedOnly` with the all-talent levels left out. The test
// asserts the two agree on this save and names the gap when they do not.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/royal-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const R = await import(join(source, 'js/stats/systems/w7/royal-guardian.js'));
const data = await import(join(source, 'js/stats/data/w7/royal-guardian.js'));
const talents = await import(join(source, 'js/stats/data/common/talent.js'));
const talentSystem = await import(join(source, 'js/stats/systems/common/talent.js'));
const maps = await import(join(source, 'js/stats/data/common/maps.js'));
const spelunk = await import(join(source, 'js/stats/systems/w7/spelunking.js'));
const jelly = await import(join(source, 'js/stats/systems/w7/jelly-operator.js'));

const save = JSON.parse(
  readFileSync(new URL('../fixtures/data.json', import.meta.url), 'utf8'),
);
loader.loadSaveData({ data: save });
const S = state.saveData;

const WORLDS = 7;
const number = (value) => Number(value) || 0;

// The purchased-only talent, as the port reads it: the highest saved level
// across the characters through the talent's own curve. The loader decodes
// `SL_<n>` into `skillLvData`, so the raw save is the place to read the row.
const curve = talents.talentParams(231);
let purchasedLevel = 0;
for (const key of Object.keys(save).filter((name) => /^SL_\d+$/.test(name))) {
  const levels = JSON.parse(save[key]);
  purchasedLevel = Math.max(purchasedLevel, number(levels?.['231']));
}
const purchasedMulti = purchasedLevel > 0
  ? Math.max(1, 1 + curve.x1 * purchasedLevel / (purchasedLevel + curve.x2))
  : 0;
const upstreamDetail = talentSystem.maxTalentBonusDetail(
  231,
  R.royalGuardianCharacter(S).index,
  S,
);
const ext = { talent231Multi: purchasedMulti };

const counters = R.kingdomTotals(S);
const rows = R.outpostRows(S, ext);
const eligible = [0, 1, 2, 3, 4, 5, 6].flatMap((world) => R.worldOutpostRows(S, world));
const clearingTargets = eligible.filter(
  (row) => !row.built && R.outpostClearMetrics(S, row.mapIdx, ext).state === 'clearing',
);
const built = rows.map((row) => ({
  map: row.mapIdx,
  type: row.type,
  range: row.range,
  units: R.outpostTotalUnits(S, row.mapIdx),
  guards: R.totalUnitsByType(S, row.mapIdx)[2],
  purified: R.outpostIsPurified(S, row.mapIdx),
  glorified: R.outpostIsGlorified(S, row.mapIdx),
}));

const worlds = [];
for (let world = 0; world < WORLDS; world += 1) {
  const list = R.worldOutpostRows(S, world);
  const clearing = list.filter((row) => !row.built && R.outpostClearMetrics(S, row.mapIdx, ext).state === 'clearing');
  worlds.push({
    world,
    unlocked: R.royalWorldUnlocked(S, world),
    outposts: list.length,
    built: list.filter((row) => row.built).length,
    clearing: clearing.length,
  });
}

// The page lists the maps the world pages carry: a save row past the map
// catalog has no anchor, so the sections never show it.
const clearingRows = [];
for (const mapIdx of eligible.map((row) => row.mapIdx)) {
  const detail = R.outpostClearMetrics(S, mapIdx, ext);
  clearingRows.push({
    map: mapIdx,
    state: detail.state,
    // The page derives `active` from the state; `outpostClearMetrics` reports
    // `accruing` instead.
    active: detail.state === 'clearing',
    accruing: detail.accruing === true,
    progress: detail.progress,
    requirement: detail.requirement,
    remaining: detail.remaining,
    ratePerHour: detail.ratePerHour,
    assignments: detail.assignments,
    etaHours: Number.isFinite(detail.etaHours) ? detail.etaHours : null,
  });
}

const rangeTargets = [];
for (const row of rows) {
  const target = R.nextResourceRangeTarget(S, row.mapIdx, ext);
  rangeTargets.push({
    map: row.mapIdx,
    range: row.range,
    target: target
      ? {
          resource: target.resourceIdx,
          distance: target.distance,
          reach: row.range + 15,
          possible: target.plan.possible,
          currentLevel: target.plan.currentLevel,
          requiredRange: target.plan.requiredRange,
          targetLevel: target.plan.targetLevel,
          levelsNeeded: target.plan.levelsNeeded,
          reason: target.plan.reason ?? null,
        }
      : null,
  });
}

const payload = {
  counters: {
    builtOutposts: counters.outposts,
    outposts: worlds.reduce((sum, world) => sum + world.outposts, 0),
    worldsUnlocked: worlds.filter((world) => world.unlocked).length,
    armoryLevels: R.armoryTotalLevels(S),
    units: built.reduce((sum, row) => sum + row.units, 0),
    purified: built.filter((row) => row.purified).length,
    support: built.filter((row) => row.type === 1).length,
    savage: built.filter((row) => row.type === 2).length,
    militiaClearing: clearingTargets.length,
    militiaClearsPerHour: clearingTargets.reduce(
      (sum, row) => sum + R.militiaClearRate(S, row.mapIdx, ext),
      0,
    ),
  },
  talent: {
    purchasedLevel,
    purchasedMulti,
    // What upstream adds on top: the all-talent levels the port does not read.
    allTalentLevels: upstreamDetail.detail?.bonus ?? 0,
    xtraClearKillz: R.XtraClearKillz(S),
    xtraClearKillzPurchasedOnly: R.XtraClearKillz(S, ext),
  },
  // The two cross-system factors the clearing rate reads, beside the Orblet one
  // the save carries.
  crossSystem: {
    bigFish6: spelunk.computeBigFishBonus(6, S),
    jelly2: jelly.jellyCompletionBonus(2, S),
  },
  worlds,
  built,
  rangeTargets,
  clearing: clearingRows,
  // The map names the tables print, kept beside the numbers so a rename shows up.
  names: Object.fromEntries(
    [...new Set(clearingRows.map((row) => row.map))].map((map) => [map, maps.mapDisplayName(map)]),
  ),
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/stats/systems/w7/royal-guardian.js',
      'js/stats/data/w7/royal-guardian.js',
      'js/stats/data/common/talent.js',
    ],
    save: 'fixtures/data.json',
  },
};

writeFileSync(
  new URL('../fixtures/royal-guardian.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(
  `wrote ${clearingRows.length} clearing rows, ${built.length} built outposts, ${rangeTargets.length} range rows`,
);
