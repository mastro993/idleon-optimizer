// Extracts the Cglunko data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-cglunko-data.mjs <path-to-idleon-research-optimizer>
//
// The upgrade names and their per-level values are the game's own
// `RandoListo2[11]` and `RandoListo2[13]` rows, which
// `js/stats/systems/w5/cglunko.js` reads as `_names` and `_perLv`.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-cglunko-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const cglunko = await import(join(source, 'js/stats/systems/w5/cglunko.js'));

const names = lists.RandoListo2[11];
const perLv = lists.RandoListo2[13];
if (names.length !== cglunko.UPGRADE_COUNT || perLv.length !== cglunko.UPGRADE_COUNT) {
  throw new Error(
    `the upgrade rows moved: ${names.length} names and ${perLv.length} values for ${cglunko.UPGRADE_COUNT} upgrades`
  );
}

const upgrades = names.map((name, index) => ({
  name: String(name).replace(/_/g, ' '),
  perLv: Number(perLv[index]),
}));

for (const upgrade of upgrades) {
  if (!Number.isFinite(upgrade.perLv)) {
    throw new Error(`upgrade ${upgrade.name} has no per-level value`);
  }
}

const payload = {
  upgrades,
  generatedFrom: { repo: 'Corgan/idleon-research-optimizer' },
};

const out = new URL('../crates/core/data/cglunko.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(`wrote ${upgrades.length} upgrades`);
