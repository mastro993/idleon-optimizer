// Extracts the Cglunko data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-cglunko-data.mjs <path-to-idleon-research-optimizer>
//
// The upgrade names, descriptions, per-level values and cost bases are the
// game's own `RandoListo2[11]`, `[12]`, `[13]` and `[14]` rows, which
// `js/stats/systems/w5/cglunko.js` reads as `_names`, `_descs`, `_perLv` and
// `_costB`; its `UPGRADE_CATEGORIES` table says which upgrades feed DR.
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
const descs = lists.RandoListo2[12];
const perLv = lists.RandoListo2[13];
const costBase = lists.RandoListo2[14];
const categories = cglunko.UPGRADE_CATEGORIES;
for (const [row, values] of Object.entries({ names, descs, perLv, costBase, categories })) {
  if (values.length !== cglunko.UPGRADE_COUNT) {
    throw new Error(
      `the ${row} row moved: ${values.length} values for ${cglunko.UPGRADE_COUNT} upgrades`
    );
  }
}

const upgrades = names.map((name, index) => ({
  name: String(name).replace(/_/g, ' '),
  desc: String(descs[index]).replace(/_/g, ' '),
  perLv: Number(perLv[index]),
  costBase: Number(costBase[index]),
  category: String(categories[index]),
}));

for (const upgrade of upgrades) {
  if (!Number.isFinite(upgrade.perLv)) {
    throw new Error(`upgrade ${upgrade.name} has no per-level value`);
  }
  if (!Number.isFinite(upgrade.costBase) || upgrade.costBase <= 0) {
    throw new Error(`upgrade ${upgrade.name} has no cost base`);
  }
  if (upgrade.category === 'undefined') {
    throw new Error(`upgrade ${upgrade.name} has no category`);
  }
}

const payload = {
  upgrades,
  generatedFrom: { repo: 'Corgan/idleon-research-optimizer' },
};

const out = new URL('../crates/core/data/cglunko.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(`wrote ${upgrades.length} upgrades`);
