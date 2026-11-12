// Extracts the Minehead (Depth Charge) tables the Rust crate ships from the
// upstream JS repo. Usage: node script/extract-minehead-data.mjs <upstream-repo>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-minehead-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { MineheadUPG, Research } = await import(
  join(source, 'js/stats/data/game/customlists.js')
);
const data = await import(join(source, 'js/stats/data/w7/minehead.js'));
const systems = await import(join(source, 'js/stats/systems/w7/minehead.js'));

const clean = (text) => text.replace(/_/g, ' ').split('@')[0].trim();

const payload = {
  upgrades: MineheadUPG.map((upg) => ({
    name: clean(upg[0]),
    maxLv: Number(upg[1]),
    costExp: Number(upg[2]),
    bonus: Number(upg[3]),
    desc: clean(upg[5]),
  })),
  floorRewards: {
    qty: Research[20].map(Number),
    desc: Research[19].map(clean),
  },
  gridDims: Research[9].slice(),
  bossNames: Research[10].map((index) => Research[11][Number(index)] || 'Boss'),
  tileMultipliers: data.TILE_MULTIPLIERS,
  buttonRates: systems.BUTTON_RATES,
  generatedFrom: { repo: 'Corgan/idleon-research-optimizer' },
};

if (payload.upgrades.length !== 30) throw new Error('expected 30 Minehead upgrades');
if (payload.floorRewards.qty.length !== payload.floorRewards.desc.length) {
  throw new Error('floor reward quantities and descriptions disagree');
}

writeFileSync(
  new URL('../crates/minehead/data/minehead.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`
);
console.log(
  `wrote ${payload.upgrades.length} upgrades, ${payload.floorRewards.qty.length} floor rewards, ` +
    `${payload.gridDims.length} grid sizes and ${payload.bossNames.length} boss names`
);
