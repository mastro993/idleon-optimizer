// Regenerates crates/clamworks/tests/clamworks-oracle.json from the upstream
// Clamworks calculator, so the promotion tracker, the worker-class rewards and
// the budgeted upgrade plan are compared with the source of truth instead of
// hand-picked numbers.
//
// The plan is driven by the upstream functions themselves: `rankUpgrades` picks
// the purchase and `upgNextCost` prices it at the levels the plan has reached,
// which the oracle writes back into the save data before the next step.
//
// Usage: node script/clamworks-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/clamworks-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const saveData = await import(join(source, 'js/save/data.js'));
const CW = await import(join(source, 'js/stats/systems/w7/clamworks.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save, serverVars: {} });

/** The saved options list, with every Clamworks entry read as a number. */
const saved = saveData.optionsListData.map((value) => Number(value) || 0);
const round = (value) => Number(Number(value).toFixed(9));

/** Point the upstream module's `_ola` reads at `levels`, then restore nothing. */
const useLevels = (levels) => saveData.assignSaveData({ optionsListData: levels });

const MAX_STEPS = 500;

/** The plan the upstream rank and cost functions buy for `budget`. */
function plan(budget, multikillTotal) {
  const levels = saved.slice();
  const steps = [];
  let spent = 0;
  let capped = false;
  const before = {
    pearlValue: CW.pearlValue({ multikillTotal }),
    pearlsPerKill: CW.pearlsPerKill({ multikillTotal }),
  };
  for (;;) {
    if (steps.length >= MAX_STEPS) {
      capped = true;
      break;
    }
    const remaining = budget - spent;
    if (remaining <= 0) break;
    useLevels(levels);
    const rank = CW.rankUpgrades({ multikillTotal }).find(
      (row) => row.ppkGain > 0 && row.nextCost <= remaining,
    );
    if (!rank) break;
    levels[455 + rank.idx] = rank.level + 1;
    spent += rank.nextCost;
    steps.push({
      index: rank.idx,
      fromLevel: rank.level,
      cost: round(rank.nextCost),
      gain: round(rank.ppkGain),
    });
  }
  useLevels(levels);
  const after = {
    pearlValue: CW.pearlValue({ multikillTotal }),
    pearlsPerKill: CW.pearlsPerKill({ multikillTotal }),
  };
  const endUpgrades = Array.from({ length: CW.UPGRADE_COUNT }, (_, i) => levels[455 + i]);
  useLevels(saved);
  return {
    budget,
    multikillTotal,
    capped,
    spent: round(spent),
    steps,
    endUpgrades,
    pearlsPerKillBefore: round(before.pearlsPerKill),
    pearlsPerKillAfter: round(after.pearlsPerKill),
    pearlValueBefore: round(before.pearlValue),
    pearlValueAfter: round(after.pearlValue),
  };
}

useLevels(saved);
const promotion = {
  level: CW.promotionLevel(),
  balance: round(CW.pearlBalance()),
  chance: CW.promotionChance(),
  cost: CW.promotionCost(),
  expectedAttempts: Math.ceil(1 / CW.promotionChance()),
  expectedCost: CW.expectedPearlsPerPromotion(),
  clamHpAfter: 1e16 * Math.pow(30, CW.promotionLevel() + 1),
  rewards: CW.PROMO_BONUS_DESCS.map((description, index) => ({
    level: index + 1,
    description: description.replace(/_/g, ' '),
    unlocked: CW.clamWorkBonus(index) === 1,
  })),
};

const fixture = {
  savedBy: 'script/clamworks-oracle.mjs',
  source: 'js/stats/systems/w7/clamworks.js + clamworks-calc.html (promotion tab)',
  pearlBalance: round(CW.pearlBalance()),
  promotionLevel: CW.promotionLevel(),
  upgrades: Array.from({ length: CW.UPGRADE_COUNT }, (_, i) => CW.upgLevel(i)),
  promotion,
  plans: [
    plan(round(CW.pearlBalance()), 1234.5),
    plan(1_000_000, 1234.5),
    plan(5_000_000, 0),
  ],
};

writeFileSync(
  join(root, 'crates/clamworks/tests/clamworks-oracle.json'),
  `${JSON.stringify(fixture, null, 1)}\n`,
);
console.log(
  `wrote ${fixture.plans.length} Clamworks plans (${fixture.plans
    .map((entry) => `${entry.steps.length} steps`)
    .join(', ')}) and ${promotion.rewards.length} rewards`,
);
