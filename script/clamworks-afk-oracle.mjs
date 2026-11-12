// Writes the upstream Clamworks AFK claim and comparison figures for the
// committed synthetic save, so `crates/core/tests/clamworks_afk.rs` compares the
// Rust port against the JS.
//
// The upstream page runs the claim in a combat worker; the pearl half of that
// result is `clamworksAfkSummary` over the map's pearl drop rows
// (`afkMonsterDropOutcomes`), which is what this script calls directly. The
// claim's combat stand-ins — kills per hour and the drop-rate stat — are passed
// in, exactly as the app passes them in place of the combat simulator.
//
// Usage: node script/clamworks-afk-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/clamworks-afk-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const saveDataModule = await import(join(source, 'js/save/data.js'));
const CW = await import(join(source, 'js/stats/systems/w7/clamworks.js'));
const AFK = await import(join(source, 'js/stats/systems/w7/clamworks-afk.js'));
const AFK_COMMON = await import(join(source, 'js/stats/systems/common/combat-afk.js'));
const PLAN = await import(join(source, 'js/stats/systems/w7/clamworks-combat.js'));
const { createStatContext } = await import(join(source, 'js/stats/stat-context.js'));
const { MonsterDrops } = await import(join(source, 'js/stats/data/game/custommaps.js'));

const save = JSON.parse(
  readFileSync(new URL('../fixtures/data.json', import.meta.url), 'utf8'),
);
loader.loadSaveData({ data: save });
const saveData = state.saveData;

const MULTIKILL = 1234.5;
const MAP = 'w7a6';

// The two combat simulator stand-ins the app also takes as inputs.
const values = (dropRate) => ({
  dropRate,
  multikillTotal: MULTIKILL,
  overkillActive: 0,
  printerFilter: [],
  filterSlots: 0,
  cards0: {},
  monsterCash: 7_000_000,
  questComplete: {},
});

// A fresh copy of the map's drop rows per claim: the Clamworks initialiser
// rewrites the pearl rows in place.
const tables = (phase) => {
  const copy = { ldorRuns: 0, monsterDrops: JSON.parse(JSON.stringify(MonsterDrops)) };
  AFK.initializeClamworksAfkDrops(copy, phase);
  return copy;
};

const ctx = createStatContext({ saveData, charIdx: 0, mapIdx: 306, layer1: { afkClaim: true } });

/// One claim: the drops of the initial table plus the summary over them.
function claim({ hours, killsPerHour, dropRate, phase }) {
  const batch = Math.floor(hours * killsPerHour);
  // The native claim needs more than two minutes away; short ones roll nothing.
  const eligible = hours * 3600 > 120;
  const drops = eligible ? AFK_COMMON.afkMonsterDropOutcomes(batch, MAP, values(dropRate), tables(phase)) : [];
  // Replacements always read the settled table, whatever the claim started from.
  const rerolls = eligible ? AFK_COMMON.afkMonsterDropOutcomes(batch, MAP, values(dropRate), tables('settled')) : [];
  const summary = AFK.clamworksAfkSummary(ctx, {
    values: values(dropRate),
    drops,
    rerollDrops: rerolls,
    food: null,
    eligible,
    phase,
  });
  return {
    batch,
    rolls: eligible ? batch : 0,
    summary,
    row: (drop) => ({
      expected: drop.expected,
      chance: drop.chance,
      minimum: drop.minimum,
      guaranteed: drop.guaranteed,
      pearls: drop.expected * (drop.key === 'ClamPearl1' ? summary.pureMultiplier : 1)
        * summary.value * (1 + 9 * summary.luckyChance),
    }),
  };
}

const claims = [
  { hours: 8, killsPerHour: 2000, dropRate: 2, phase: 'entry' },
  { hours: 8, killsPerHour: 2000, dropRate: 2, phase: 'settled' },
  { hours: 0.5, killsPerHour: 1400, dropRate: 2, phase: 'settled' },
  { hours: 0.03, killsPerHour: 2000, dropRate: 2, phase: 'settled' },
].map((inputs) => {
  const { batch, rolls, summary, row } = claim(inputs);
  return {
    ...inputs,
    batch,
    rolls,
    regular: row(summary.regular),
    pure: row(summary.pure),
    value: summary.value,
    luckyChance: summary.luckyChance,
    pureMultiplier: summary.pureMultiplier,
    // Upstream's payout over the initial claim, before any reroll.
    expectedPearls: summary.expectedPearls,
  };
});

// PURE PEARLS unbought: the row is gone in the fresh table and its rate is zero
// once the table settles. The live options list is what the module reads.
const pureLevel = saveDataModule.optionsListData[460];
saveDataModule.optionsListData[460] = 0;
const pureLocked = ['entry', 'settled'].map((phase) => {
  const { summary, row } = claim({ hours: 0.5, killsPerHour: 1400, dropRate: 2, phase });
  return { phase, pure: row(summary.pure), expectedPearls: summary.expectedPearls };
});
saveDataModule.optionsListData[460] = pureLevel;

// The comparison: every unlocked +1 the upstream plan offers, each claim at the
// same hours and stand-ins, summarised by the upstream `summarizeClamworksAfk`.
const COMPARE = { hours: 8, killsPerHour: 2000, dropRate: 2, phase: 'entry' };
const plan = PLAN.buildClamworksAfkPlan(save, {
  charIdx: 0,
  setup: null,
  hours: COMPARE.hours,
  phase: COMPARE.phase,
});
const records = plan.rows.map((entry) => {
  if (entry.index == null) {
    const { summary } = claim(COMPARE);
    return { key: entry.key, result: { effectiveHours: COMPARE.hours, clamworks: summary, warnings: [] } };
  }
  // The row's own upgrade level, patched into the live options list the module reads.
  const at = 455 + entry.index;
  const level = saveDataModule.optionsListData[at];
  saveDataModule.optionsListData[at] = entry.to;
  const { summary } = claim(COMPARE);
  saveDataModule.optionsListData[at] = level;
  return { key: entry.key, result: { effectiveHours: COMPARE.hours, clamworks: summary, warnings: [] } };
});
const rows = PLAN.summarizeClamworksAfk(plan, records).map((row) => ({
  key: row.key,
  label: row.label,
  index: row.index ?? null,
  from: row.from ?? null,
  to: row.to ?? null,
  cost: row.cost,
  affordable: row.affordable ?? null,
  expectedPearls: row.expectedPearls,
  pureChance: row.pureChance,
  gain: row.gain,
  paybackClaims: row.paybackClaims,
  paybackHours: row.paybackHours,
}));

const payload = {
  source: 'js/stats/systems/w7/clamworks-afk.js, js/stats/systems/common/combat-afk.js',
  map: MAP,
  multikillTotal: MULTIKILL,
  pearlDropRate: CW.pearlDropRate(),
  blackPearlDropRate: CW.blackPearlDropRate(),
  claims,
  pureLocked,
  compare: { ...COMPARE, planWarnings: plan.warnings, rows },
};

writeFileSync(
  new URL('../fixtures/clamworks-afk.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(`wrote ${claims.length} claims, ${pureLocked.length} pure-locked cases and ${rows.length} compare rows`);
