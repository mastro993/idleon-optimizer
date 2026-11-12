// Extracts the fishing data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-fishing-data.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-fishing-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const fishing = await import(join(source, 'js/stats/data/w2/fishing.js'));
const items = await import(join(source, 'js/stats/data/game/items.js'));
const hole = await import(join(source, 'js/stats/data/w5/hole.js'));
const legend = await import(join(source, 'js/stats/data/w7/legendTalent.js'));
const jar = await import(join(source, 'js/stats/systems/w5/jar.js'));

const pools = fishing.FISHING_POOL_KEYS.map((key) => fishing.fishingPool(key));
const lures = fishing.fishingToolkitOptions(0);
const lines = fishing.fishingToolkitOptions(1);
const locations = fishing.fishingLocations();

const rods = Object.keys(items.ITEMS)
  .filter((key) => /^FishingRod\d+$/.test(key))
  .map((key) => ({
    key,
    name: items.ITEMS[key].displayName.replace(/\|/g, ' ').replace(/_/g, ' ').trim(),
    speed: Number(items.ITEMS[key].Speed) || 0,
  }))
  .sort((a, b) => Number(a.key.slice(10)) - Number(b.key.slice(10)));

if (pools.length !== 5) throw new Error('fish pools moved');
for (const pool of pools) {
  if (pool.fish.length !== 4) throw new Error(`pool ${pool.key} no longer has four fish`);
  if (pool.effReqs.length !== 3) throw new Error(`pool ${pool.key} no longer has three eff reqs`);
}
if (lures.length !== 15 || lines.length !== 15) throw new Error('toolkit options moved');
if (rods.length === 0) throw new Error('fishing rods moved');

// The Trench depth requirement (FI-03). Each skilling cavern's cleared layers
// scale one Bolaia study; the Trench is cavern 3. The jar's Amethyst Heartstone
// (collectible 5) is the third requirement reduction, and Legend 29 raises it.
const TRENCH_STUDIES = [1, 7, 11, 16];
const trench = {
  caverns: ['Motherlode', 'Hive', 'Evertree', 'Trench'],
  studies: TRENCH_STUDIES,
  bolaiaPerLevel: TRENCH_STUDIES.map((study) => hole.holesBolaiaPerLv(study)),
  legendPerPoint: legend.legendTalentPerPt(29),
  collectible5PerLevel: jar.COLLECTIBLE_DATA[5][1],
};
if (trench.bolaiaPerLevel.some((value) => !(value > 0))) throw new Error('Bolaia studies moved');
if (!(trench.legendPerPoint > 0)) throw new Error('Legend 29 moved');
if (!(trench.collectible5PerLevel > 0)) throw new Error('collectible 5 moved');

const payload = {
  pools,
  lures,
  lines,
  locations,
  rods,
  trench,
  generatedFrom: { repo: 'Corgan/idleon-research-optimizer' },
};

const out = new URL('../crates/fishing/data/fishing.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${pools.length} pools, ${lures.length} lures, ${lines.length} lines, ` +
    `${locations.length} locations, ${rods.length} rods, ${trench.studies.length} Trench studies`
);
