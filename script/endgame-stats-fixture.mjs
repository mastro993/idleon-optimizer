// Writes the upstream leaderboard audit for the committed synthetic save, so
// `crates/endgame/tests/endgame.rs` can compare the Rust port against the JS.
//
// Usage: node script/endgame-stats-fixture.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/endgame-stats-fixture.mjs <upstream-repo>');
  process.exit(1);
}

const { collectEndgameStats } = await import(join(source, 'js/endgame-stats.js'));
const save = JSON.parse(
  readFileSync(new URL('../fixtures/data.json', import.meta.url), 'utf8'),
);

const stats = collectEndgameStats(save).map(({ id, label, group, value, partial }) => ({
  id,
  label,
  group,
  value,
  partial: Boolean(partial),
}));

const payload = {
  generatedFrom: 'Corgan/idleon-research-optimizer js/endgame-stats.js',
  save: 'fixtures/data.json',
  stats,
};
writeFileSync(
  new URL('../fixtures/endgame-stats.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
const available = stats.filter((stat) => stat.value != null).length;
console.log(`wrote ${stats.length} metrics (${available} with a value)`);
