// Writes the upstream Coral God Rank planner figures for the committed synthetic
// save, so `crates/core/tests/coral_plan.rs` compares the Rust port against the
// JS beam search and offering model.
//
// Usage: node script/coral-god-rank-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/coral-god-rank-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const C = await import(join(source, 'js/stats/systems/w5/coral-god-rank.js'));

const save = JSON.parse(
  readFileSync(new URL('../fixtures/data.json', import.meta.url), 'utf8'),
);
// The app resolves a missing Y4 and DivCostAfter3 to 0, exactly as the upstream
// page's `getDefaultInputs` does; the save carries neither.
loader.loadSaveData({ data: save });
const saveData = state.saveData;
const base = C.coralGodRankInputs(saveData, { assumeUnlocked: true });
// The raw save stores each character row as a bare comma list; index 14 is the
// character's Divinity level, which the unlock table sums across all players.
const row = (key) => {
  const value = save[key];
  if (Array.isArray(value)) return value;
  try {
    return JSON.parse(value);
  } catch {
    return String(value).split(',').map(Number);
  }
};
const totalDivinityLevels = Object.keys(save)
  .filter((key) => /^Lv0_\d+$/.test(key))
  .reduce((total, key) => total + Number(row(key)[14] || 0), 0);

/// Singularity upgrades 6-11 are the second Coral Kid page in `OptLacc`.
const singularityLevels = [6, 7, 8, 9, 10, 11].map((index) =>
  C.coralKidLevel(index, saveData),
);

/// The section's defaults, in the same shape as `idleon_core::coral::Inputs`.
const inputs = {
  goal: 'balanced',
  budget: base.budget || 0,
  maxLevels: 25,
  beamSize: 100,
  godRank: base.godRank || 0,
  y4: base.y4 == null ? 0 : base.y4,
  divCostAfter3: base.divCostAfter3 == null ? 0 : base.divCostAfter3,
  reefUpgradeCount: base.reefUpgradeCount || 0,
  mineFloor: base.mineFloor == null ? 0 : base.mineFloor,
  vault76Bonus: base.vault76Bonus == null ? 0 : base.vault76Bonus,
  tier: base.tier || 0,
  planningConfidence: 0.9,
  assumeUnlocked: true,
  levels: base.levels,
  singularityLevels,
  totalDivinityLevels,
};

/// A crafted small plan where a wide beam must beat a one-state beam.
const narrowProbe = {
  goal: 'balanced',
  budget: 1e9,
  maxLevels: 8,
  beamSize: 100,
  godRank: 31,
  y4: 0,
  divCostAfter3: 0,
  reefUpgradeCount: 10,
  mineFloor: 0,
  vault76Bonus: 0,
  tier: 0,
  planningConfidence: 0.9,
  assumeUnlocked: true,
  levels: [10, 10, 10, 10, 10, 10],
  // The planner reads Singularity levels from the save, not from options.
  singularityLevels,
  totalDivinityLevels: 10_000,
};

const metrics = (plan) => ({
  levels: plan.levels,
  godRank: plan.godRank,
  divinityPoints: plan.divinityPoints,
  divinityExp: plan.divinityExp,
  classExp: plan.classExp,
  minorLinks: plan.minorLinks,
  blessingCap: plan.blessingCap,
  dailyReefCoral: plan.dailyReefCoral,
});

const actions = (plan) =>
  plan.actions.map((action) =>
    action.type === 'coral-kid'
      ? { type: 'coral-kid', index: action.index, level: action.level, cost: action.cost }
      : {
          type: 'god-offering',
          fromRank: action.fromRank,
          toRank: action.toRank,
          attempts: action.attempts,
          expectedSpend: action.expectedSpend,
          maxPaidSpend: action.maxPaidSpend,
          confidence: action.confidence,
          wholePlanConfidence: action.wholePlanConfidence,
        },
  );

/// One case: the options the port receives and the plan upstream returns.
const run = (name, options) => {
  const plan = C.optimizeCoralGodRank(saveData, options.goal, options);
  return {
    name,
    inputs: options,
    plan: {
      goal: plan.goal,
      current: metrics(plan.current),
      final: metrics(plan.final),
      actions: actions(plan),
      budget: plan.budget,
      spent: plan.spent,
      remaining: plan.remaining,
      partial: plan.partial,
      unlocks: plan.unlocks,
      wholePlanConfidence: plan.metadata.wholePlanConfidence,
      dominancePruned: plan.metadata.dominancePruned,
    },
  };
};

const offeringRows = [
  { godRank: 0, y4: 0 },
  { godRank: 1, y4: 0 },
  { godRank: 5, y4: 0 },
  { godRank: 10, y4: 0 },
  { godRank: 31, y4: 0 },
  { godRank: 32, y4: 0 },
  { godRank: 38, y4: 0 },
  { godRank: 99, y4: 0 },
  { godRank: 0, y4: 20 },
  { godRank: 31, y4: 20 },
  { godRank: 31, y4: 100 },
].map(({ godRank, y4 }) => {
  const stats = C.offeringStats(0, saveData, {
    godRank,
    divCostAfter3: inputs.divCostAfter3,
    y4,
    tier: inputs.tier,
  });
  return {
    godRank,
    y4,
    tier: inputs.tier,
    odds: stats.successChancePerAttempt,
    cost: stats.paidCost,
    freeChance: stats.freeCostChance,
    expectedSpendForOneSuccess: stats.expectedSpendForOneSuccess,
    confidence: stats.confidence,
  };
});

const fixture = {
  source:
    'Corgan/idleon-research-optimizer js/stats/systems/w5/coral-god-rank.js',
  cases: [
    run('fixture', { ...inputs }),
    run('fixture-narrow-beam', { ...inputs, beamSize: 1 }),
    run('narrow-probe-wide', { ...narrowProbe }),
    run('narrow-probe-narrow', { ...narrowProbe, beamSize: 1 }),
  ],
  offering: offeringRows,
};

writeFileSync(
  new URL('../fixtures/coral-plan.json', import.meta.url),
  `${JSON.stringify(fixture, null, 2)}\n`,
);
console.log(
  `wrote fixtures/coral-plan.json: ${fixture.cases.length} cases, ${offeringRows.length} offering rows`,
);
