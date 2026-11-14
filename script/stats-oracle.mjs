// Regenerates fixtures/stats.json from the upstream All Stats calculator, so
// the Rust stat engine is compared with upstream's own tree rather than
// hand-picked numbers.
// Usage: node script/stats-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/stats-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const { createStatContext } = await import(join(source, 'js/stats/stat-context.js'));
const { saveData } = await import(join(source, 'js/state.js'));
const data = await import(join(source, 'js/save/data.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save });

// The tree's own shape is the checklist: `computeTotalStat` returns the six
// top-level pools and one named child per term inside each. Three levels keep
// every term while dropping the per-item lists the gallery and equipment pools
// carry one level lower.
const DEPTH = 2;
// Pools whose own terms are the checklist, one level past the default depth.
const DETAIL_POOLS = ['All Stats Percentage'];
// The gallery and equipment pools list every item; their totals are the terms
// the port is compared on, so only those are serialized.
const LIST_PREFIXES = [
  'R0S', 'R1S', 'Trophy', 'Nametag', 'EquipmentHats', 'EquipmentCape',
  'Cape Vine Base', 'Premhat Base', 'Hatrack cached base',
];
const serialize = (node, depth = 0) => {
  const out = { name: String(node.name ?? ''), value: Number(node.val) };
  if (node.fmt) out.format = String(node.fmt);
  if (node.note) out.note = String(node.note);
  const list = LIST_PREFIXES.some((prefix) => out.name.startsWith(prefix));
  const deep = depth < DEPTH || DETAIL_POOLS.includes(out.name);
  if (!list && deep && Array.isArray(node.children) && node.children.length) {
    out.children = node.children.map((child) => serialize(child, depth + 1));
  }
  return out;
};

const stats = ['STR', 'AGI', 'WIS', 'LUK'];
const characters = Number(data.numCharacters) || 0;
const cases = [];
for (let character = 0; character < characters; character += 1) {
  const map = Number(data.currentMapData?.[character]) || 0;
  const ctx = createStatContext({ charIdx: character, mapIdx: map, saveData, mapBon: data.mapBonData });
  const row = { character, map, class: Number(data.charClassData?.[character]) || 0, stats: {} };
  for (const stat of stats) {
    const result = ctx.resolve(`total-${stat.toLowerCase()}`);
    row.stats[stat] = serialize(result);
  }
  cases.push(row);
}

writeFileSync(
  join(root, 'fixtures/stats.json'),
  `${JSON.stringify(
    {
      source: 'fixtures/data.json through js/save/loader.js loadSaveData',
      descriptor: 'js/stats/defs/base-stats.js',
      context: 'js/stats/stat-context.js createStatContext',
      treeDepth: DEPTH,
      cases,
    },
    null,
    2
  )}\n`
);
console.log(`wrote ${cases.length} character stat trees`);
