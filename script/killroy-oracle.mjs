// Regenerates fixtures/killroy.json from the upstream planner, so the Rust port
// is tested against the source of truth rather than hand-picked numbers.
// Usage: node script/killroy-oracle.mjs <path-to-idleon-research-optimizer> [weeks]
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/killroy-oracle.mjs <upstream-repo> [weeks]');
  process.exit(1);
}
const weeks = Number(process.argv[3] ?? 2);
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const planner = await import(
  join(source, 'js/stats/systems/common/killroy-planner.js')
);
const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
// `serverVars.KillroySwap` is server state, so a real forecast needs a wrapped
// save; the synthetic fixture is the account half only.
const raw = { data: save, serverVars: { KillroySwap: 7 } };
const forecast = planner.killroyForecast(raw, { weeks });

// The Rust forecast covers the fields the section renders; class labels are not
// part of it, so drop the planner's display-only `className`.
for (const week of forecast.weeks) {
  for (const character of week.characters) delete character.className;
}

writeFileSync(
  join(root, 'fixtures/killroy.json'),
  `${JSON.stringify(
    {
      source: 'fixtures/data.json wrapped as it.json with serverVars.KillroySwap = 7',
      planner: 'js/stats/systems/common/killroy-planner.js',
      weeks,
      forecast,
    },
    null,
    2
  )}\n`
);
console.log(`wrote ${forecast.weeks.length} weeks to fixtures/killroy.json`);
