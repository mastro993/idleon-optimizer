// Regenerates fixtures/monument.json from the upstream Monument calculator, so
// the Rust port is tested against the source of truth rather than hand-picked
// numbers.
// Usage: node script/monument-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/monument-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const { monumentSnapshot, monumentMilestones, braverySwordOdds, monumentRewardCharge } =
  await import(join(source, 'js/stats/systems/w5/monument.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save, serverVars: {} });
const saveData = state.saveData;

// The extracted data tables only cover the fields the section reads, so the
// snapshot note about Summoning access stays out of the oracle: the Rust port
// narrows the save the same way.
const snapshot = monumentSnapshot(saveData, 0);
if (!snapshot.available) throw new Error('the fixture has no Monument rows');
const bravery = snapshot.monuments[0];

/** The saved Bravery snapshot, in the shape the Rust `Snapshot` serialises to. */
const saved = {
  available: true,
  notes: snapshot.notes,
  training: {
    sources: snapshot.training.sources.map((source) => ({
      name: source.name,
      value: source.val,
    })),
    pct: snapshot.training.pct,
    multi: snapshot.training.multi,
    rate: snapshot.training.rate,
    perDay: snapshot.training.perDay,
  },
  stats: bravery.stats,
  milestones: monumentMilestones(saveData, 0, snapshot.training.rate),
  bonuses: bravery.bonuses,
};

/// One run state each, so boost and rethrow changes are pinned and not just the
/// saved cards.
const RUN_STATES = [
  { label: 'fresh story', wave: 0, runtime: {} },
  { label: 'boosted stacks', wave: 100, runtime: { boosts: 10 } },
  { label: 'no rethrows left', wave: 1000, runtime: { rerolls: 0 } },
  { label: 'boosted mid story', wave: 7, runtime: { boosts: 4, rerolls: 0 } },
  { label: 'late wave, many boosts', wave: 999, runtime: { boosts: 200, rerolls: 5 } },
  { label: 'last-sword HP entered', wave: 52, runtime: {}, remainingHp: 150000000 },
  {
    label: 'boosted with last-sword HP',
    wave: 50,
    runtime: { boosts: 3 },
    remainingHp: 100000,
  },
];

const odds = RUN_STATES.map(({ label, wave, runtime, remainingHp = null }) => {
  const result = braverySwordOdds(saveData, { wave, ...runtime, remainingHP: remainingHp });
  const { hours, milestones, opalChance, minDmg, maxDmg, rerolls, maxRerolls, boosts, swords,
    retellings, rightmost, autoKill } = result;
  return {
    label,
    wave,
    runtime,
    remainingHp,
    // The Rust `Odds` nests the run stats; the upstream result spreads them.
    result: {
      stats: { hours, milestones, opalChance, minDmg, maxDmg, rerolls, maxRerolls, boosts, swords,
        retellings, rightmost, autoKill },
      wave: result.wave,
      hp: result.hp,
      automatic: result.automatic,
      kill: result.kill,
      boostChance: result.boostChance,
      lastSwordChance: result.lastSwordChance,
      nextDamageFactor: result.nextDamageFactor,
    },
  };
});

/** The saved Wisdom snapshot, in the shape the Rust `WisdomSnapshot` serialises to. */
const wisdom = {
  available: true,
  notes: snapshot.notes,
  training: saved.training,
  stats: snapshot.monuments[2].stats,
  milestones: monumentMilestones(saveData, 2, snapshot.training.rate),
  bonuses: snapshot.monuments[2].bonuses,
  charge: snapshot.monuments[2].charge,
  tomorrow: snapshot.monuments[2].tomorrow,
};

/// The reward charge for every monument, plus a preview of a longer wait, so the
/// shared formula is pinned on all three branches.
const charges = snapshot.monuments.map((monument, index) => ({
  monument: index,
  charge: monument.charge,
  tomorrow: monument.tomorrow,
  extraWait: monumentRewardCharge(saveData, index, 0, 3600),
}));

/** Rejected reward-charge inputs and the upstream message each one prints. */
const chargeErrors = [
  { label: 'unknown monument', monument: 3, extraSeconds: 0 },
  { label: 'negative extra wait', monument: 0, extraSeconds: -1 },
].map((entry) => {
  try {
    monumentRewardCharge(saveData, entry.monument, 0, entry.extraSeconds);
  } catch (error) {
    return { ...entry, message: error.message };
  }
  throw new Error(`${entry.label}: the upstream calculator accepted the input`);
});

/** Rejected inputs and the upstream message each one prints. */
const errors = [
  { label: 'too many enemies', wave: 1001, runtime: {}, remainingHp: null },
  { label: 'rethrows over the maximum', wave: 0, runtime: { rerolls: 16 }, remainingHp: null },
  // Both invalid: the run stats are read before the HP, so the rethrow error wins.
  { label: 'too many rethrows and no HP', wave: 0, runtime: { rerolls: 16 }, remainingHp: 0 },
  { label: 'non-positive HP', wave: 0, runtime: {}, remainingHp: 0 },
].map((entry) => {
  try {
    braverySwordOdds(saveData, {
      wave: entry.wave,
      ...entry.runtime,
      remainingHP: entry.remainingHp,
    });
  } catch (error) {
    return { ...entry, message: error.message };
  }
  throw new Error(`${entry.label}: the upstream calculator accepted the input`);
});

writeFileSync(
  join(root, 'fixtures/monument.json'),
  `${JSON.stringify(
    {
      source: 'fixtures/data.json',
      calculator: 'js/stats/systems/w5/monument.js',
      character: 0,
      snapshot: saved,
      wisdom,
      charges,
      chargeErrors,
      odds,
      errors,
    },
    null,
    2
  )}\n`
);
console.log(
  `wrote the Bravery and Wisdom snapshots, ${charges.length} reward charges, ${odds.length} run states and ${errors.length} rejected inputs to fixtures/monument.json`
);
