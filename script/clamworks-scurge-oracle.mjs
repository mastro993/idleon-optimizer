// Writes the upstream Clamworks Scurge figures — the Self Affliction comparison
// and the Optimized EXP allocation — into fixtures/clamworks-scurge.json, so
// `crates/clamworks/tests/clamworks_scurge.rs` compares the Rust port against the
// JS that defines the rules.
//
// The upstream module plans on a raw save object, so each case is the committed
// fixture save with its `Menagerie` row rewritten: the Scurge rank, the nine
// upgrade levels and the Scurge unlock. A Scurge run's EXP an hour upstream comes
// from the combat simulator; that is not built for Clam Work, so the fixture pins
// the per-kill values the app multiplies by its kills-per-hour stand-in.
//
// Usage: node script/clamworks-scurge-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/clamworks-scurge-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const SCURGE = await import(join(source, 'js/stats/systems/w7/clamworks-scurge.js'));

const MULTIKILL = 1234.5;
const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save, serverVars: {} });
const savedLevels = Array.from({ length: 9 }, (_, index) => Number(JSON.parse(save.Menagerie)[5][index]) || 0);

const round = (value) => Number(Number(value).toFixed(9));

/// The fixture save with a case's Menagerie row: rank, Scurge upgrade levels and
/// the unlock flag at `Menagerie[2][4]`.
function rawSave({ rank, levels = savedLevels, unlocked = true }) {
  const raw = JSON.parse(JSON.stringify(save));
  const menagerie = JSON.parse(raw.Menagerie);
  menagerie[0][8] = rank;
  menagerie[2][4] = unlocked ? 1 : 0;
  for (let index = 0; index < 9; index++) menagerie[5][index] = levels[index] ?? 0;
  raw.Menagerie = JSON.stringify(menagerie);
  return raw;
}

const cases = [
  { name: 'saved-affliction', mode: 'affliction', maxAffliction: 12, otherPlayers: 0, rank: 122 },
  { name: 'saved-upgrades', mode: 'upgrades', maxAffliction: 12, otherPlayers: 0, rank: 122 },
  {
    name: 'unspent-upgrades-swarming',
    mode: 'upgrades',
    maxAffliction: 12,
    otherPlayers: 3,
    rank: 700,
  },
  {
    name: 'affliction-spread',
    mode: 'affliction',
    maxAffliction: 12,
    otherPlayers: 0,
    rank: 30,
    levels: [10, 0, 5, 0, 0, 0, 0, 0, 0],
  },
  {
    name: 'affliction-locked',
    mode: 'affliction',
    maxAffliction: 12,
    otherPlayers: 0,
    rank: 5,
    levels: [0, 0, 0, 0, 0, 0, 0, 0, 0],
  },
  {
    name: 'upgrades-locked',
    mode: 'upgrades',
    maxAffliction: 12,
    otherPlayers: 0,
    rank: 5,
    levels: [0, 0, 0, 0, 0, 0, 0, 0, 0],
  },
  {
    name: 'unspent-affliction',
    mode: 'affliction',
    maxAffliction: 8,
    otherPlayers: 0,
    rank: 60,
    levels: [5, 0, 2, 0, 10, 0, 0, 0, 0],
  },
  {
    name: 'not-unlocked',
    mode: 'upgrades',
    maxAffliction: 12,
    otherPlayers: 0,
    rank: 122,
    unlocked: false,
  },
  {
    name: 'over-budget',
    mode: 'affliction',
    maxAffliction: 12,
    otherPlayers: 0,
    rank: 10,
    levels: [5, 0, 7, 0, 0, 0, 0, 0, 0],
  },
];

const levels = (menagerie) => {
  const row = menagerie?.[5] || [];
  return Array.from({ length: 9 }, (_, index) => Number(row[index]) || 0);
};

const written = cases.map((entry) => {
  const raw = rawSave(entry);
  const saved = levels(JSON.parse(raw.Menagerie));
  const base = {
    name: entry.name,
    mode: entry.mode,
    maxAffliction: entry.maxAffliction,
    otherPlayers: entry.otherPlayers,
    rank: entry.rank,
    unlocked: entry.unlocked !== false,
    levels: saved,
  };
  let plan;
  try {
    plan = SCURGE.buildClamworksScurgePlan(raw, {
      charIdx: 0,
      setup: { charIdx: 0 },
      seeds: [1],
      seconds: 300,
      mode: entry.mode,
      maxAffliction: entry.maxAffliction,
      otherPlayers: entry.otherPlayers,
    });
  } catch (error) {
    return { ...base, error: error.message };
  }
  return {
    ...base,
    available: plan.available,
    selfBudget: plan.selfBudget,
    maximum: plan.maximum,
    rows: plan.rows.map((row) => ({
      label: row.label,
      levels: row.levels,
      changes: row.changes.map((change) => ({
        index: change.index,
        name: change.name,
        from: change.from,
        to: change.to,
      })),
      pointsLeft: row.pointsLeft,
      expPerKill: round(row.expPerKill),
    })),
  };
});

const payload = {
  savedBy: 'script/clamworks-scurge-oracle.mjs',
  source: 'js/stats/systems/w7/clamworks-scurge.js',
  multikillTotal: MULTIKILL,
  cases: written,
};

writeFileSync(
  join(root, 'fixtures/clamworks-scurge.json'),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(
  `wrote ${written.length} cases, ${written.reduce((sum, c) => sum + (c.rows?.length || 0), 0)} rows`,
);
