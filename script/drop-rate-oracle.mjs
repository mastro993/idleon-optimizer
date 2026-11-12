// Regenerates fixtures/drop-rate.json from the upstream calculator, so the Rust
// port is tested against the source of truth rather than hand-picked numbers.
// Usage: node script/drop-rate-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/drop-rate-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const { saveData } = await import(join(source, 'js/state.js'));
const { mapBonData } = await import(join(source, 'js/save/data.js'));
const { createStatContext } = await import(join(source, 'js/stats/stat-context.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
// The fixture is the save object itself, which `loadSaveData` accepts directly.
loadSaveData({ data: save });

// The live dungeon values a run shows: the fixture carries no run state, so the
// oracle sets them the way `drop-rate-calc.html` reads its two inputs.
const cases = [
  { map: 39, luk: 0, dropRate: 0 },
  { map: 39, luk: 1200, dropRate: 35 },
  { map: 40, luk: 25_000, dropRate: 750.5 },
  { map: 70, luk: 3.5, dropRate: 0.25 },
  { map: 71, luk: 999_999, dropRate: 12_345 },
  { map: 118, luk: 12_345.5, dropRate: 678.25 },
  { map: 119, luk: 1, dropRate: 99 },
];

function resolve(map, luk, dropRate) {
  const ctx = createStatContext({
    charIdx: 0,
    mapIdx: map,
    saveData,
    mapBon: mapBonData,
    layer1: { dungeonLuk: luk, dungeonDropRarity: dropRate },
  });
  return ctx.resolve('drop-rate');
}

const solved = cases.map(({ map, luk, dropRate }) => {
  const tree = resolve(map, luk, dropRate);
  if (tree.unavailable) throw new Error(`map ${map} came back unavailable`);
  return { map, luk, dropRate, total: tree.val };
});

const missing = resolve(39, NaN, NaN);
if (!missing.unavailable) throw new Error('a dungeon map without live values must be unavailable');

// Every other map needs the descriptor's whole pool chain. The reference total
// is recorded so the port that builds the account stats engine can check
// itself; the current Rust build reports the gap instead of this number.
const character = 0;
const map = Number(saveData.currentMapData?.[character]) || 0;
const reference = resolve(map, NaN, NaN);

writeFileSync(
  join(root, 'fixtures/drop-rate.json'),
  `${JSON.stringify(
    {
      source: 'fixtures/data.json through js/save/loader.js loadSaveData',
      descriptor: 'js/stats/defs/drop-rate.js',
      formula: 'js/stats/systems/w2/dungeon.js computeDungeonDropRate',
      cases: solved,
      missingInputs: { map: 39, reason: missing.reason },
      nonDungeon: {
        note: 'Needs the account stats engine (SA-02); recorded as the reference total.',
        character,
        map,
        total: reference.val,
      },
    },
    null,
    2
  )}\n`
);
console.log(`wrote ${solved.length} dungeon totals to fixtures/drop-rate.json`);
