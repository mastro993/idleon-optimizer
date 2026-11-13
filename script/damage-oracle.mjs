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

// `damage-calc.html` builds the three DDL cards from the descriptor's children:
// the title, the `note` under it and the `_debug.preSoftcap` value the card's
// second line prints. The mastery tree is the `Mastery (min/max ratio)` child
// and its four terms; its formula's base and penalty come from the descriptor's
// own expansion (`formulaDetails`).
const recordLayer = (children, prefix, preSoftcapKey = 'preSoftcap') => {
  const child = children.find((node) => String(node.name).startsWith(prefix));
  if (!child) throw new Error(`the descriptor left out ${prefix}`);
  return {
    name: String(child.name),
    value: Number(child.val),
    note: String(child.note ?? ''),
    format: String(child.fmt ?? 'raw'),
    // DDL0's `_debug.preSoftcap` is the with-softcap value; `preSoftcapRaw` is
    // the one before the chain (the page labels it 'Base Damage Before Softcaps
    // and Food'), so the card's note compares like the other two layers.
    preSoftcap: Number(child._debug?.[preSoftcapKey] ?? 0),
  };
};

const findNode = (node, name) => {
  if (!node) return null;
  if (String(node.name).startsWith(name)) return node;
  for (const child of node.children ?? []) {
    const found = findNode(child, name);
    if (found) return found;
  }
  return null;
};

const resolveExpanded = (character, map) =>
  createStatContext({
    charIdx: character,
    mapIdx: map,
    saveData,
    mapBon: data.mapBonData,
    layer1: { formulaDetails: true },
  }).resolve('damage');

const recordMastery = (children, expanded) => {
  const node = children.find((child) => String(child.name).startsWith('Mastery'));
  if (!node) throw new Error('the descriptor left out the mastery node');
  const minDamage = (expanded.children ?? []).find((child) =>
    String(child.name).startsWith('Min Damage')
  );
  const detail = findNode(minDamage, 'Mastery (min/max ratio)');
  if (!detail) throw new Error('the expanded damage tree left out the mastery node');
  const base = findNode(detail, 'Base Mastery minus talent');
  const penalty = findNode(detail, 'Talent Mastery penalty');
  const cap = (detail.children ?? []).find((child) => Number(child.val) === 0.8);
  if (!base || !penalty || !cap) throw new Error('the mastery detail tree changed shape');
  return {
    name: String(node.name),
    value: Number(node.val),
    cap: Number(cap.val),
    base: Number((base.children ?? [])[0]?.val),
    penalty: Number(penalty.val),
    terms: (node.children ?? []).map((child) => ({
      name: String(child.name),
      value: Number(child.val),
      format: String(child.fmt ?? 'raw'),
    })),
  };
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
    partial: Boolean(result.partial),
    reason: String(result.reason ?? ''),
    max: layer(children, 'Max Damage'),
    min: layer(children, 'Min Damage'),
    mastery: recordMastery(children, resolveExpanded(character, map)),
    layers: {
      base: recordLayer(children, 'Base Damage', 'preSoftcapRaw'),
      percent: recordLayer(children, 'Percentage Damage Multiplier'),
      multiplier: recordLayer(children, 'Multiplicative Damage Bonuses'),
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

// `data.json` carries neither an `it.json` companion roster nor `serverVars`,
// so the descriptor flags the same two metadata fields the page lists.
const missing = [
  ...new Set(cases.map((item) => item.reason).filter(Boolean)),
];
if (missing.length > 1) throw new Error('the cases disagree about the missing metadata');

// The same save with both metadata fields present clears the flag, so the port
// is compared on both sides of the rule and not only on the missing case.
loadSaveData({ data: save, companion: { l: [] }, serverVars: { voteCategories: [0] } });
const completeMap = Number(data.currentMapData?.[0]) || 0;
const complete = resolve(0, completeMap);
if (complete.unavailable) throw new Error('the complete-metadata case came back unavailable');
if (complete.partial || complete.reason) {
  throw new Error(`a complete save still flags metadata: ${complete.reason}`);
}
loadSaveData({ data: save });

// The map selector lists every `MapDispName` entry in index order; the page's
// `populateMapSel` marks the dungeon and current rows in the label.
const maps = [...Array(MapDispName.length).keys()]
  .filter((map) => Boolean(MapDispName[map]))
  .map((map) => ({
    map,
    name: String(MapDispName[map]).replace(/_/g, ' '),
  }));

writeFileSync(
  join(root, 'fixtures/damage.json'),
  `${JSON.stringify(
    {
      source: 'fixtures/data.json through js/save/loader.js loadSaveData',
      descriptor: 'js/stats/defs/damage.js',
      context: 'js/stats/stat-context.js createStatContext',
      metadata: {
        missing: missing[0] ?? '',
        complete: { character: 0, map: completeMap, partial: false },
      },
      cases,
      dungeons: dungeonCases,
      maps,
    },
    null,
    2
  )}\n`
);
console.log(
  `wrote ${cases.length} character summaries, ${dungeonCases.length} dungeons and ${maps.length} map rows`
);
