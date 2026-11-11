// Regenerates fixtures/sailing-upgrades.json from the upstream sailing
// calculator, so the Rust port is tested against the source of truth rather
// than hand-picked numbers.
// Usage: node script/sailing-oracle.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/sailing-oracle.mjs <upstream-repo>');
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
  availableIslandCount,
  boatUpgradeBreakpoint,
  captainLevelTime,
  captainRollQuality,
  chestLootRolls,
  chestReplacementOutcome,
  fleetChestPayoutBreakdown,
  islandArtifactOdds,
  optimizeCaptainAssignment,
  overflowUpgradeChance,
  routeMetrics,
  sailingCurrencyMeta,
  unendingLootSearch,
} = await import(join(source, 'js/stats/systems/w5/sailing-chests.js'));
const { maxArtifactTier } = await import(join(source, 'js/stats/systems/w5/sailing.js'));
const { islandArtifactCount, islandArtifactOffset } = await import(
  join(source, 'js/stats/data/w5/sailing.js')
);

// One purpose-built account: three boats, three captains, no account bonuses,
// Unending Loot Search active so the replacement matrix exercises both paths.
// The Rust test imports the same string, so it stays the only copy.
const lv0 = new Array(21).fill(0);
lv0[0] = 100;
lv0[13] = 10;
const sailing = [new Array(20).fill(-1), new Array(31).fill(0), [2, 2], new Array(41).fill(0)];
sailing[1][0] = 5000;
sailing[1][1] = 250;
// Artifact 9's tier feeds the chest rarity scale; artifacts 0 and 1 exercise an
// upgradeable and a capped artifact on the first island.
sailing[3][9] = 2;
sailing[3][0] = 1;
sailing[3][1] = 6;
const save = {
  Research: JSON.stringify(
    Array.from({ length: 48 }, (_, index) => (index === 0 ? new Array(240).fill(0) : [])),
  ),
  Lv0_0: JSON.stringify(lv0),
  OptLacc: JSON.stringify(new Array(128).fill(0)),
  // Boat 0 and 1 hold each other's best captain, so the saved fleet is not the
  // optimum and the assignment matters.
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
  GemItemsPurchased: JSON.stringify(new Array(140).fill(0)),
  // Talent 325: Unending Loot Search. The loader turns the level into the
  // percentage the replacement matrix reads.
  SL_0: JSON.stringify({ 325: 120 }),
  Sailing: JSON.stringify(sailing),
};

loadSaveData({ data: save, serverVars: {} });
const saveData = state.saveData;
const options = { activeCharIdx: 0, minimumTier: 0 };
const talent = unendingLootSearch(saveData, 0);
if (!(talent > 0)) throw new Error('Unending Loot Search did not resolve from SL_0');

/** The upstream fleet plan for one island, as the levels tab builds it. */
function plan(island) {
  return optimizeCaptainAssignment(island, 'treasure', saveData, options);
}

const fleet = plan(0);
const strongest = fleet.reduce((best, route) => (route.loot.value > best.loot.value ? route : best), fleet[0]);

const breakpoints = [];
for (let boat = 0; boat < saveData.boatsData.length; boat++) {
  for (const stat of ['loot', 'speed']) {
    // Upstream targets the island that pays the upgrade's currency, and the
    // first island for gold.
    const type = stat === 'loot'
      ? (boat < 4 ? 0 : Math.min(30, 1 + 2 * (boat - 4)))
      : (boat < 2 ? boat : boat < 5 ? 1 + 2 * (boat - 2) : Math.min(30, 2 * (boat - 4)));
    const island = Math.max(0, sailingCurrencyMeta(type).islandIdx);
    const bp = boatUpgradeBreakpoint(boat, stat, plan(island), saveData, options);
    breakpoints.push({
      boat,
      stat,
      level: bp.level,
      interval: bp.interval,
      targetLevel: bp.targetLevel,
      levels: bp.levels,
      currentBase: bp.currentBase,
      nextBase: bp.nextBase,
      targetBase: bp.targetBase,
      nextBaseGainPct: bp.nextBaseGainPct,
      baseGainPct: bp.baseGainPct,
      thresholdMarginalPct: bp.thresholdMarginalPct,
      daveyUnlocked: bp.daveyUnlocked,
      daveyActive: bp.daveyActive,
      daveyLevel: bp.daveyLevel,
      type: bp.type,
      totalCost: bp.totalCost,
      balance: bp.balance,
      remaining: bp.remaining,
      initialRate: bp.initialRate,
      finalRate: bp.finalRate,
      hours: bp.hours,
      treasureGainPct: bp.treasureGainPct,
      chestGainPct: bp.chestGainPct,
      artifactGainPct: bp.artifactGainPct,
      speedCapped: bp.speedCapped,
    });
  }
}

const assigned = new Map();
for (const route of fleet) assigned.set(route.captainIdx, route);

const captains = [];
for (let captain = 0; captain < saveData.captainsData.length; captain++) {
  const entry = saveData.captainsData[captain];
  const route = assigned.get(captain);
  const row = {
    captain,
    level: Number(entry[3]) || 0,
    slots: [captainRollQuality(captain, 0, saveData), captainRollQuality(captain, 1, saveData)].map(
      (slot) => ({
        bonusType: slot.bonusType,
        roll: slot.roll,
        min: slot.min,
        max: slot.max,
        quality: slot.quality,
      }),
    ),
  };
  if (route) {
    const time = captainLevelTime(captain, route.boatIdx, 0, saveData, options);
    Object.assign(row, {
      boat: route.boatIdx,
      progress: time.progress,
      requirement: time.requirement,
      remaining: time.remaining,
      expPerTrip: time.expPerTrip,
      expPerHour: time.expPerHour,
      hours: time.hours,
    });
  }
  captains.push(row);
}

const replacement = [];
for (const route of fleet) {
  for (let tier = 0; tier < 5; tier++) {
    const outcome = chestReplacementOutcome(route, strongest, tier, saveData, 0);
    const threshold = outcome.threshold;
    replacement.push({
      boat: route.boatIdx,
      captain: route.captainIdx,
      tier,
      strongestBoat: strongest.boatIdx,
      baseValue: route.loot.value,
      strongestValue: strongest.loot.value,
      sameIsland: threshold.sameIsland,
      sameTierEligible: threshold.sameTierEligible,
      referenceValue: threshold.referenceValue,
      // Upstream spells "no tier" as -1; the fixture keeps JSON's null.
      requiredTier: threshold.requiredTier >= 0 ? threshold.requiredTier : null,
      replacementValue: threshold.replacementValue,
      margin: threshold.margin,
      maxValue: threshold.maxValue,
      shortfall: threshold.shortfall,
      exactSameTierChance: outcome.exactSameTierChance,
      ulsChance: outcome.ulsChance,
      sameTierUpgradeChance: outcome.sameTierUpgradeChance,
      naturalReplacementChance: outcome.naturalReplacementChance,
      miracleGenerationChance: outcome.miracleGenerationChance,
      miracleGenerationRatePerHour: outcome.miracleGenerationRatePerHour,
      totalChance: outcome.totalChance,
      totalRatePerHour: outcome.totalRatePerHour,
    });
  }
}

const payload = {
  source: 'purpose-built save in script/sailing-oracle.mjs',
  modules: [
    'js/stats/systems/w5/sailing-chests.js',
  ],
  save,
  unendingLootSearch: talent,
  breakpoints,
  captains,
  replacement,
};
writeFileSync(
  join(root, 'fixtures/sailing-upgrades.json'),
  `${JSON.stringify(payload, null, 2)}\n`,
);
console.log(
  `wrote ${breakpoints.length} breakpoints, ${captains.length} captains and ${replacement.length} replacement cells`,
);
// ==================== CHEST, ARTIFACT AND LOOT OUTCOMES ====================

// `aggregateFleetTiers` in sailing-calc.html: each boat's exact-tier chest rate
// weights its captain's tier odds.
const generatedPerHour = fleet.reduce((sum, row) => sum + row.generatedChestsPerHour, 0);
const fleetTiers = [];
for (let tier = 0; tier < 6; tier++) {
  let exact = 0;
  let cumulative = 0;
  for (const row of fleet) {
    exact += row.generatedChestsPerHour * row.rarity.tiers[tier].probability;
    cumulative += row.generatedChestsPerHour * row.rarity.tiers[tier].cumulative;
  }
  fleetTiers.push({
    tier,
    probability: generatedPerHour > 0 ? exact / generatedPerHour : 0,
    cumulative: generatedPerHour > 0 ? cumulative / generatedPerHour : 0,
    expectedPerHour: exact,
  });
}

const payoutRows = fleetTiers.map(({ tier }) => {
  const breakdown = fleetChestPayoutBreakdown(fleet, tier, saveData);
  return {
    tier,
    multiplier: breakdown.multiplier,
    payouts: breakdown.payouts,
    chestRate: breakdown.chestRate,
    averageStoredValue: breakdown.averageStoredValue,
    totalValuePerChest: breakdown.totalValuePerChest,
    currencies: Object.fromEntries(
      Object.entries(breakdown.currencies).map(([key, currency]) => [
        key,
        {
          chancePerPayout: currency.chancePerPayout,
          expectedPayoutsPerChest: currency.expectedPayoutsPerChest,
          expectedValuePerChest: currency.expectedValuePerChest,
          expectedValuePerHour: currency.expectedValuePerHour,
        },
      ]),
    ),
  };
});

// The near-full pile block in sailing-calc.html: tier four cannot promote to
// Miracle, so only the four ordinary transitions are listed.
const nearFull = [];
for (let tier = 0; tier < 4; tier++) {
  nearFull.push({
    tier,
    chance: overflowUpgradeChance(tier, saveData, 0),
    lootMultiplier: CHEST_LOOT_MULTIS[tier],
    nextLootMultiplier: CHEST_LOOT_MULTIS[tier + 1],
    valueRatio:
      (CHEST_LOOT_MULTIS[tier + 1] * chestLootRolls(tier + 1, saveData)) /
      (CHEST_LOOT_MULTIS[tier] * chestLootRolls(tier, saveData)),
    artifactMultiplier: CHEST_ARTIFACT_MULTIS[1],
  });
}

// `breakdownRoute` picks the selected boat's route from the active plan.
const breakdownRoute = fleet.find((row) => row.boatIdx === 0) || fleet[0];
const artifactIslands = [];
for (let island = 0; island < availableIslandCount(saveData); island++) {
  const row = routeMetrics(breakdownRoute.boatIdx, island, saveData, {
    ...options,
    captainIdx: breakdownRoute.captainIdx,
  });
  const offset = islandArtifactOffset(island);
  let incomplete = 0;
  for (let index = 0; index < islandArtifactCount(island); index++) {
    if (Number(saveData.sailingData?.[3]?.[offset + index]) < maxArtifactTier(saveData)) incomplete++;
  }
  artifactIslands.push({
    island,
    incomplete,
    tripSeconds: row.timing.tripSeconds,
    artifactChancePerTrip: row.artifactChancePerTrip,
    artifactFindsPerHour: row.artifactFindsPerHour,
  });
}
artifactIslands.sort((left, right) => right.artifactFindsPerHour - left.artifactFindsPerHour);

// `artifactDetail` in sailing-calc.html sums the kept tiers' exact chances.
const weighted = new Map();
for (const tier of breakdownRoute.tiers) {
  if (!tier.kept) continue;
  const odds = islandArtifactOdds(breakdownRoute.islandIdx, tier.artifactMultiplier, saveData, {
    serverVars: {},
  });
  for (const artifact of odds.artifacts) {
    const row = weighted.get(artifact.artifactIdx) || {
      artifact: artifact.artifactIdx,
      name: artifact.name,
      currentTier: artifact.currentTier,
      // Upstream spells "never" as Infinity; the fixture keeps JSON's null.
      denominator: Number.isFinite(artifact.denominator) ? artifact.denominator : null,
      unlocked: artifact.unlocked,
      chance: 0,
    };
    row.chance += tier.probability * artifact.exactChance;
    weighted.set(artifact.artifactIdx, row);
  }
}

const outcomes = {
  source: 'purpose-built save in script/sailing-oracle.mjs',
  modules: [
    'sailing-calc.html',
    'js/stats/systems/w5/sailing-chests.js',
    'js/stats/systems/w5/sailing.js',
  ],
  save,
  unendingLootSearch: talent,
  island: breakdownRoute.islandIdx,
  boat: breakdownRoute.boatIdx,
  captain: breakdownRoute.captainIdx,
  maxArtifactTier: maxArtifactTier(saveData),
  routes: fleet.map((row) => ({
    boat: row.boatIdx,
    captain: row.captainIdx,
    island: row.islandIdx,
    generatedChestsPerHour: row.generatedChestsPerHour,
    keptChestsPerHour: row.keptChestsPerHour,
    lootValue: row.loot.value,
    treasurePerHour: row.treasurePerHour,
    artifactFindsPerHour: row.artifactFindsPerHour,
    artifactChancePerTrip: row.artifactChancePerTrip,
    artifactBase: row.artifactBase.value,
    rarityScale: row.rarity.rarityScale,
    treasureShares: row.treasureShares,
  })),
  fleet: { generatedPerHour, tiers: fleetTiers },
  payouts: payoutRows,
  nearFull,
  islands: artifactIslands,
  artifacts: [...weighted.values()],
};

writeFileSync(
  join(root, 'fixtures/sailing-outcomes.json'),
  `${JSON.stringify(outcomes, null, 2)}\n`,
);
console.log(
  `wrote ${fleetTiers.length} fleet tiers, ${payoutRows.length} payout rows, ${nearFull.length} near-full rows, ${artifactIslands.length} islands and ${weighted.size} artifacts`,
);
