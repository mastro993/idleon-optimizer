// Writes the upstream comparison cell for paired runs, so
// `crates/combat/tests/compare.rs` compares the Rust port against the module.
//
// Source: `js/stats/systems/common/combat-sim-setup-compare.js`
// (`setupCompareCell`, over `_runValue`'s per-hour conversion). Each case lists
// the row's runs and the baseline's runs as `{seed, kills, timeMs}`; the script
// hands them to the upstream function in its own `{runs: {seed: {...}}}`
// shape and records its answer. A run with `timeMs: 0` is the upstream missing
// value (`_runValue` returns null), which the Rust port represents as NaN.
//
// Usage: node script/combat-compare-oracle.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/combat-compare-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const COMPARE = await import(
  join(source, 'js/stats/systems/common/combat-sim-setup-compare.js')
);

// Upstream reads a seed row as `{hours, kills, ...}`: `addSetupCompareRun`
// stores `hours = timeMs / 3.6e6` before `_runValue` divides the kill count by
// it, and a zero-length run reports no value.
const row = (entry) => ({
  hours: entry.timeMs / 3.6e6,
  kills: entry.kills,
});
const runs = (entries) => ({
  runs: Object.fromEntries(
    entries.map((entry) => [String(entry.seed), row(entry)]),
  ),
});

const seeds = (kills, timeMs) => kills.map((value, index) => ({ seed: index + 1, kills: value, timeMs }));

const cases = [
  {
    name: 'three_seeds',
    values: seeds([120, 150, 135], 300_000),
    baseline: seeds([100, 100, 100], 300_000),
  },
  {
    name: 'one_seed',
    values: seeds([90], 300_000),
    baseline: seeds([75], 300_000),
  },
  {
    name: 'zero_baseline',
    values: seeds([10, 20, 30], 300_000),
    baseline: seeds([0, 0, 0], 300_000),
  },
  {
    name: 'zero_delta_on_a_zero_baseline',
    values: seeds([0, 0, 0], 300_000),
    baseline: seeds([0, 0, 0], 300_000),
  },
  {
    name: 'a_missing_run',
    values: [
      { seed: 1, kills: 120, timeMs: 300_000 },
      { seed: 2, kills: 10, timeMs: 0 },
      { seed: 3, kills: 135, timeMs: 300_000 },
    ],
    baseline: seeds([100, 100, 100], 300_000),
  },
];

const payload = {
  source: 'js/stats/systems/common/combat-sim-setup-compare.js setupCompareCell',
  metric: 'kills',
  cases: cases.map((entry) => ({
    name: entry.name,
    values: entry.values,
    baseline: entry.baseline,
    expected: COMPARE.setupCompareCell(
      runs(entry.values),
      runs(entry.baseline),
      'kills',
    ),
  })),
};

writeFileSync(
  new URL('../fixtures/combat-compare.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(`wrote ${payload.cases.length} comparison cases`);
