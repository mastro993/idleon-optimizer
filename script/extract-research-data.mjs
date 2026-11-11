// Extracts the research-grid data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-research-data.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-research-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const mod = await import(join(source, 'js/stats/data/w7/research.js'));
const game = await import(join(source, 'js/stats/data/game/customlists.js'));

const nodes = mod.GRID_INDICES.map((id) => ({
  id,
  name: mod.RES_GRID_RAW[id][0],
  maxLv: mod.RES_GRID_RAW[id][1],
  bonusPerLv: mod.RES_GRID_RAW[id][2],
  desc: mod.RES_GRID_RAW[id][3] ?? '',
  goal: mod.NODE_GOAL?.[id] ?? null,
}));

const shapes = mod.SHAPE_NAMES.map((name, i) => ({
  id: i,
  name,
  bonusPct: mod.SHAPE_BONUS_PCT[i],
  vertices: mod.SHAPE_VERTICES[i],
  dims: mod.SHAPE_DIMS[i],
  color: mod.SHAPE_COLORS[i],
}));

const observations = mod.OCC_DATA.map((o, i) => ({ id: i, name: o.name, roll: o.roll, rollReq: o.rollReq }));

const presetsSource = await import('node:fs').then((fs) =>
  fs.readFileSync(join(source, 'js/optimizers/shape-presets.js'), 'utf8')
);

// The presets are a literal array; read its static `data` fields rather than
// running the save-dependent `build` functions.
const presetRows = [];
for (const match of presetsSource.matchAll(
  /\{ id: '([^']+)', name: '([^']+)', data: '([^']*)'/g,
)) {
  const [, id, name, data] = match;
  presetRows.push({ id, name, data });
}

const coordToId = new Map();
for (const node of nodes) {
  const col = node.id % mod.GRID_COLS;
  const row = Math.floor(node.id / mod.GRID_COLS);
  coordToId.set(String.fromCharCode(65 + col) + (mod.GRID_ROWS - row), node.id);
}

const presets = presetRows.map(({ id, name, data }) => ({
  id,
  name,
  data,
  targets: data
    .replaceAll('|', '')
    .split(',')
    .map((coord) => coord.trim())
    .filter(Boolean)
    .map((coord) => coordToId.get(coord))
    .filter((value) => value !== undefined),
}));

const payload = {
  grid: { cols: mod.GRID_COLS, rows: mod.GRID_ROWS },
  nodes,
  shapes,
  observations,
  presets,
  generatedFrom: { repo: 'Corgan/idleon-research-optimizer', game: game.Research?.length ?? 0 },
};

const out = new URL('../crates/core/data/research.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(`wrote ${nodes.length} nodes, ${shapes.length} shapes, ${observations.length} observations`);

// Clamworks reads four Spelunky rows: the upgrade descriptions (27), per-level
// values (28), cost bases (29) and the promotion reward descriptions (30). The
// names are literals in `js/stats/systems/w7/clamworks.js`.
const upgradeNames = ['PEARL VALUE', 'CLAM COMRADES', 'LUCKY DAY', 'MULTI-SCALPING',
  'FRUGALITY', 'PURE PEARLS', 'ENCYSTATION UP', 'SHINIER PEARLS', 'ANTI INFLATION'];
const scurgeNames = ['RECKONING', 'PEARL INFECTION', 'SELF AFFLICTION', 'COST DISSOLVING',
  'EVEN BETTER', 'CROSS CULTING', 'GRAVESTART', 'SWARMING', 'TRUE SCURGE'];
const scurgeUnlocks = [0, 5, 10, 25, 50, 100, 250, 500, 1000];

const clamworks = {
  upgrades: upgradeNames.map((name, i) => ({
    name,
    desc: game.Spelunky[27][i],
    perLv: Number(game.Spelunky[28][i]),
    costBase: Number(game.Spelunky[29][i]),
  })),
  scurge: scurgeNames.map((name, i) => ({
    name,
    desc: game.Spelunky[27][i + 9],
    perLv: Number(game.Spelunky[28][i + 9]),
    unlockLevel: scurgeUnlocks[i],
  })),
  promotion: game.Spelunky[30],
  generatedFrom: { repo: 'Corgan/idleon-research-optimizer' },
};

writeFileSync(
  new URL('../crates/core/data/clamworks.json', import.meta.url),
  JSON.stringify(clamworks, null, 1)
);
console.log(`wrote ${clamworks.upgrades.length} Clamworks upgrades and ${clamworks.scurge.length} Scurge upgrades`);
