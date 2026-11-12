// Regenerates fixtures/damage.json from the upstream calculator, so the Rust
// port is compared with the damage descriptor's own output rather than
// hand-picked numbers.
// Usage: node script/damage-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/damage-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const { createStatContext } = await import(join(source, 'js/stats/stat-context.js'));
const { saveData } = await import(join(source, 'js/state.js'));
const data = await import(join(source, 'js/save/data.js'));
const { isDungeonMap } = await import(join(source, 'js/stats/systems/w2/dungeon.js'));
const { MapDispName } = await import(join(source, 'js/stats/data/game/customlists.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save });

// `createStatContext` reads the same `saveData` and `mapBon` tables the page
// passes it; `damage-calc.html` builds them the same way.
const characters = data.numCharacters ?? 0;
const resolve = (character, map) =>
  createStatContext({
    charIdx: character,
    mapIdx: map,
    saveData,
    mapBon: data.mapBonData,
  }).resolve('damage');

const layer = (children, prefix) => {
  const child = children.find((node) => String(node.name).startsWith(prefix));
  return child ? Number(child.val) : null;
};

const cases = [];
for (let character = 0; character < characters; character += 1) {
  const map = Number(data.currentMapData?.[character]) || 0;
  const result = resolve(character, map);
  if (result.unavailable) throw new Error(`character ${character} came back unavailable`);
  const children = result.children ?? [];
  cases.push({
    character,
    map,
    class: Number(data.charClassData?.[character]) || 0,
    max: layer(children, 'Max Damage'),
    min: layer(children, 'Min Damage'),
    mastery: layer(children, 'Mastery'),
    layers: {
      base: layer(children, 'Base Damage'),
      percent: layer(children, 'Percentage Damage Multiplier'),
      multiplier: layer(children, 'Multiplicative Damage Bonuses'),
    },
  });
}

// Every dungeon map answers with the upstream reason instead of a summary.
const dungeonCases = [...Array(MapDispName.length).keys()]
  .filter((map) => isDungeonMap(map))
  .map((map) => {
    const result = resolve(0, map);
    if (!result.unavailable) throw new Error(`dungeon map ${map} resolved a summary`);
    return { map, reason: result.reason };
  });
if (dungeonCases.length === 0) throw new Error('the upstream dungeon set is empty');

writeFileSync(
  join(root, 'fixtures/damage.json'),
  `${JSON.stringify(
    {
      source: 'fixtures/data.json through js/save/loader.js loadSaveData',
      descriptor: 'js/stats/defs/damage.js',
      context: 'js/stats/stat-context.js createStatContext',
      cases,
      dungeons: dungeonCases,
    },
    null,
    2
  )}\n`
);
console.log(`wrote ${cases.length} character summaries and ${dungeonCases.length} dungeons`);
