// Regenerates fixtures/sailing-simulation.json from the upstream sailing pile
// simulator, so the Rust port is tested against the source of truth rather than
// hand-picked samples.
//
// Upstream draws from `Math.random`. This script replaces it with the same
// seeded xorshift64* stream the Rust port uses, so the upstream simulator's own
// output is deterministic and the two can be compared field for field.
// Usage: node script/sailing-simulation-oracle.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/sailing-simulation-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const {
  CHEST_ARTIFACT_MULTIS,
  CHEST_LOOT_MULTIS,
  CHEST_TIER_NAMES,
  chestLootRolls,
  maxChestSlots,
  optimizeCaptainAssignment,
  overflowUpgradeChance,
  routeMetrics,
  unendingLootSearch,
} = await import(join(source, 'js/stats/systems/w5/sailing-chests.js'));
const { runSailingPileSimulation } = await import(
  join(source, 'js/stats/systems/w5/sailing-sim.js')
);

// A three-boat account with every capacity source: the gem purchase, Artifact
// 19 at tier two, the task reward and achievement 287. Unending Loot Search is
// active so the near-full scan runs, and boats 0 and 1 hold each other's best
// captain so the saved and optimized pairings differ.
const lv0 = new Array(21).fill(0);
lv0[0] = 100;
lv0[13] = 10;
const sailing = [new Array(20).fill(-1), new Array(31).fill(0), [2, 2], new Array(41).fill(0)];
sailing[1][0] = 5000;
sailing[1][1] = 250;
sailing[3][9] = 2;
sailing[3][19] = 2;
sailing[3][0] = 1;
sailing[3][1] = 6;
const tasks = Array.from({ length: 6 }, () => []);
tasks[4] = [0, 0, 2];
const achievements = new Array(400).fill(0);
achievements[287] = -1;
const save = {
  Research: JSON.stringify(
    Array.from({ length: 48 }, (_, index) => (index === 0 ? new Array(240).fill(0) : [])),
  ),
  Lv0_0: JSON.stringify(lv0),
  OptLacc: JSON.stringify(new Array(128).fill(0)),
  Boats: JSON.stringify([
    [1, 0, 0, 100, 0, 100, 0],
    [0, 0, 0, 80, 0, 80, 0],
    [2, 0, 0, 60, 0, 60, 0],
  ]),
  Captains: JSON.stringify([
    [3, 0, 1, 10, 250, 20, 10],
    [6, 3, 4, 20, 1000, 30, 15],
    [2, 1, 0, 5, 0, 10, 10],
  ]),
  GemItemsPurchased: JSON.stringify(new Array(140).fill(0).map((_, index) => (index === 129 ? 4 : 0))),
  TaskZZ2: JSON.stringify(tasks),
  AchieveReg: JSON.stringify(achievements),
  SailChests: JSON.stringify([]),
  SL_0: JSON.stringify({ 325: 120 }),
  Sailing: JSON.stringify(sailing),
};

/** The upstream xorshift64* stream, matching `crate::sailing::sim`'s `Rng`. */
function seededRandom(seed) {
  let value = BigInt(seed) || 0x9e3779b97f4a7c15n;
  const mask = (1n << 64n) - 1n;
  return () => {
    let next = value;
    next ^= next >> 12n;
    next ^= next << 25n;
    next &= mask;
    next ^= next >> 27n;
    value = next;
    const rolled = (next * 0x2545f4914f6cdd1dn) & mask;
    return Number(rolled >> 11n) / 9007199254740992;
  };
}

/** The current-save pairings, as `sailing-calc.html`'s `savedFleet` builds them. */
function savedFleet(island, saveData, options) {
  const fleet = [];
  const boatCount = Math.min(
    saveData.boatsData?.length || 0,
    1 + Number(saveData.sailingData?.[2]?.[1] || 0),
  );
  const captainCount = Math.min(
    saveData.captainsData?.length || 0,
    1 + Number(saveData.sailingData?.[2]?.[0] || 0),
  );
  for (let boat = 0; boat < boatCount; boat++) {
    const captain = Math.round(Number(saveData.boatsData?.[boat]?.[0]));
    if (captain >= 0 && captain < captainCount) {
      fleet.push(routeMetrics(boat, island, saveData, { ...options, captainIdx: captain }));
    }
  }
  return fleet;
}

/** The upstream `buildSailingSimulationConfig`, as the simulation tab builds it. */
function buildConfig(fleet, saveData, case_) {
  const options = { activeCharIdx: 0, minimumTier: case_.minimumTier };
  return {
    routes: fleet.map((route) => ({
      boatIdx: route.boatIdx,
      captainIdx: route.captainIdx,
      tripsPerHour: route.generatedChestsPerHour,
      lootValue: route.loot.value,
      artifactMultiplier: route.artifactBase.value,
      islandIdx: case_.island,
      tierProbabilities: route.rarity.tiers.map((tier) => tier.probability),
    })),
    hours: case_.hours,
    trials: case_.trials,
    capacity: maxChestSlots(saveData, 0),
    minimumTier: case_.minimumTier,
    unendingTalent: unendingLootSearch(saveData, 0),
    lootMultipliers: CHEST_LOOT_MULTIS.slice(),
    artifactMultipliers: CHEST_ARTIFACT_MULTIS.slice(),
    upgradeChances: CHEST_TIER_NAMES.map((_, tier) =>
      overflowUpgradeChance(tier, saveData, 0),
    ),
    payoutsByTier: CHEST_TIER_NAMES.map((_, tier) => chestLootRolls(tier, saveData)),
    currencyShares: fleet[0]?.treasureShares || { gold: 1, primary: 0, rare: 0 },
    progressBatch: 50,
  };
}

const cases = [
  {
    name: 'optimized treasure fleet, a day of arrivals',
    mode: 'optimized',
    goal: 'treasure',
    island: 0,
    hours: 24,
    trials: 300,
    seed: 12345,
    minimumTier: 0,
  },
  {
    name: 'current-save artifact fleet, a tier-three filter',
    mode: 'saved',
    goal: 'artifact',
    island: 3,
    hours: 12,
    trials: 250,
    seed: 777,
    minimumTier: 3,
  },
];

const payload = {
  source: 'purpose-built save in script/sailing-simulation-oracle.mjs',
  modules: [
    'js/stats/systems/w5/sailing-sim.js',
    'js/stats/systems/w5/sailing-chests.js',
  ],
  save,
  cases: [],
};

for (const case_ of cases) {
  loadSaveData({ data: structuredClone(save), serverVars: {} });
  const saveData = state.saveData;
  const options = { activeCharIdx: 0, minimumTier: case_.minimumTier };
  const fleet =
    case_.mode === 'saved'
      ? savedFleet(case_.island, saveData, options)
      : optimizeCaptainAssignment(case_.island, case_.goal, saveData, options);
  if (!fleet.length) throw new Error(`${case_.name}: the fleet is empty`);
  const config = buildConfig(fleet, saveData, case_);
  Math.random = seededRandom(case_.seed);
  const result = runSailingPileSimulation(config, () => {});
  if (!result) throw new Error(`${case_.name}: the simulation returned nothing`);
  payload.cases.push({
    name: case_.name,
    mode: case_.mode,
    goal: case_.goal,
    island: case_.island,
    hours: case_.hours,
    trials: case_.trials,
    seed: case_.seed,
    config: {
      routes: config.routes,
      hours: config.hours,
      trials: config.trials,
      seed: case_.seed,
      capacity: config.capacity,
      minimumTier: config.minimumTier,
      unendingLootSearch: config.unendingTalent,
      lootMultipliers: config.lootMultipliers,
      artifactMultipliers: config.artifactMultipliers,
      upgradeChances: config.upgradeChances,
      payoutsByTier: config.payoutsByTier,
      currencyShares: config.currencyShares,
    },
    result,
  });
}

writeFileSync(
  join(root, 'fixtures/sailing-simulation.json'),
  `${JSON.stringify(payload, null, 2)}\n`,
);
console.log(`wrote ${payload.cases.length} simulation cases`);
