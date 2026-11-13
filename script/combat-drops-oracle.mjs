// Extracts the drop rows the combat simulator's card and obol result tabs read,
// and writes the cases the Rust port compares against upstream's own module.
// Usage: node script/combat-drops-oracle.mjs <path-to-idleon-research-optimizer>
//
// Sources: `js/stats/data/game/{custommaps,items,monsters}.js` for the
// `MonsterDrops`/`DropTables` rows, the card descriptions and the monster names,
// and `js/stats/systems/common/combat-sim-rewards.js` for the roll maths: the
// fixture calls the upstream `lootDropOddsReset`, `dropOdds`,
// `generateMonsterDrops` and `tryPickup` with a fresh seeded stream and records
// what they return.
//
// The port models the result tabs CS-05 ships: cards, obol fragments, kills by
// monster and the kills timeline. Every drop row that cannot produce a card or
// an obol (coins, equipment, stamps, recipes, …) is left out of the extracted
// tables, and the fixture runs the upstream functions over the same filtered
// rows, so the two sides roll the same stream and the outcomes are comparable.
//
// Two files come out of one checkout:
//   - `crates/combat/data/drops.json`: the filtered rows, the obol pages, the
//     card descriptions and the monster display names;
//   - `fixtures/combat-drops.json`: the upstream module's own results, which
//     `crates/combat/tests/drops.rs` compares the port against.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/combat-drops-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { MonsterDrops, DropTables } = await import(
  join(source, 'js/stats/data/game/custommaps.js')
);
const { ITEMS } = await import(join(source, 'js/stats/data/game/items.js'));
const { MONSTERS } = await import(join(source, 'js/stats/data/game/monsters.js'));
const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const core = await import(join(source, 'js/stats/systems/common/combat-sim-core.js'));
const world = await import(join(source, 'js/stats/systems/common/combat-sim-world.js'));
const rewards = await import(join(source, 'js/stats/systems/common/combat-sim-rewards.js'));

const root = new URL('..', import.meta.url);
const encounters = JSON.parse(
  readFileSync(new URL('crates/combat/data/encounters.json', root), 'utf8')
);
const save = JSON.parse(readFileSync(new URL('fixtures/data.json', root), 'utf8'));
loader.loadSaveData({ data: save });

// The rows the card and obol tabs read: card rows, obol rows and the drop-table
// references that can reach the obol rows `lootDropOddsReset` inserts.
const kept = (name) =>
  String(name).startsWith('Cards') ||
  String(name).startsWith('Obol') ||
  String(name).startsWith('DropTable');
// The kept rows as upstream's arrays, for the calls into its own module. A
// quest-gated row would need the per-character quest list, which this port does
// not read; the extractor refuses it rather than dropping the gate.
const keptRawRows = (rows) => {
  const out = (rows || []).filter((row) => kept(row[0]));
  for (const row of out) {
    if (row[3] !== 'N/A') throw new Error(`quest-gated drop row ${row[0]} needs the quest list`);
  }
  return out;
};
// The same rows as the plain objects the Rust port reads.
const keptRows = (rows) =>
  keptRawRows(rows).map((row) => ({
    name: String(row[0]),
    chance: Number(row[1]),
    qty: Number(row[2]),
  }));

// `cleanName` in combat-sim-calc.html: underscores and pipes become spaces.
const cleanName = (text) =>
  String(text || '')
    .replace(/[_|]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
const monName = (type) => cleanName(MONSTERS[type]?.Name) || type;

// Every monster type a supported map spawns, plus the types the Killroy rooms
// arm (`MapAFKtarget`) so a Killroy run can read the same rows.
const types = new Set();
for (const map of encounters.maps) for (const mob of map.mobs) types.add(mob.type);

// The drop tables the kept top-level rows reference, followed one level down.
const tables0 = new Map();
const addTable = (name) => {
  if (tables0.has(name) || !Array.isArray(DropTables[name])) return;
  const rows = keptRows(DropTables[name]);
  tables0.set(name, { rows });
  for (const row of rows) if (row.name.startsWith('DropTable')) addTable(row.name);
};
for (const type of types) {
  for (const row of keptRows(MonsterDrops[type])) {
    if (row.name.startsWith('DropTable')) addTable(row.name);
  }
}

// `lootDropOddsReset` (src 74357) splices 18 obol rows into nine drop tables,
// each at its table's second-to-last row. Replay that on a full copy, filter it
// and record where each inserted row lands, so the extracted tables keep the
// exact order the game reads and the runtime only applies the task multiplier.
const obolSplices = (() => {
  const full = Object.fromEntries(
    Object.entries(DropTables).map(([name, rows]) => [name, structuredClone(rows)])
  );
  const inserted = new Map();
  const add = (table, indexTable, name, chance) => {
    const list = full[table];
    if (!Array.isArray(list)) return;
    const at = Math.max(0, Math.min((full[indexTable]?.length ?? list.length) - 1, list.length));
    const row = [name, String(chance), '1', 'N/A'];
    inserted.set(row, { table, name, chance });
    list.splice(at, 0, row);
  };
  for (let e = 0; e < 12; e++) {
    let name, chance;
    if (e < 4) { name = 'ObolBronze' + e; chance = 0.0011; }
    else if (e < 8) { name = 'ObolSilver' + Math.round(e - 4); chance = 7e-4; }
    else if (e === 8) { name = 'ObolSilverMoney'; chance = 0.03; }
    else if (e === 9) { name = 'ObolSilverDamage'; chance = 0.031; }
    else if (e === 10) { name = 'ObolGold3'; chance = 0.015; }
    else { name = 'ObolSilverLuck'; chance = 0.025; }
    const table = e < 8 ? 'DropTable' + (e + 1) : e < 10 ? 'SuperDropTable1' : 'SuperDropTable2';
    const indexTable = e < 4 ? table : e < 8 ? 'DropTable' + (e + 2) : table;
    add(table, indexTable, name, chance);
  }
  for (let e = 0; e < 6; e++) {
    const name = e === 4 ? 'ObolBronzeKill' : e === 3 ? 'ObolBronzeEXP' : e === 0 ? 'ObolBronzeWorship'
      : e === 5 ? 'ObolBronzeDef' : e === 1 ? 'ObolBronzeTrapping' : 'ObolBronzeCons';
    const chance = e < 3 ? 9e-4 : 0.029;
    const table = e < 3 ? 'DropTable' + (e + 14) : e === 4 ? 'SuperDropTable3' : e === 3 ? 'SuperDropTable1' : 'SuperDropTable2';
    add(table, table, name, chance);
  }
  const splices = [];
  for (const [name, rows] of Object.entries(full)) {
    if (!tables0.has(name)) continue;
    let index = 0;
    for (const row of rows) {
      if (!kept(row[0])) continue;
      const entry = inserted.get(row);
      if (entry) splices.push({ table: name, index, name: entry.name, chance: entry.chance });
      index++;
    }
  }
  return splices;
})();

const monsters = {};
for (const type of [...types].sort()) {
  monsters[type] = {
    name: monName(type),
    kind: String(MONSTERS[type]?.Type || ''),
    rows: keptRows(MonsterDrops[type]),
  };
}

// Obol items: their page (by `typeGen`) and whether the auto-trash skips them.
const obols = {};
for (const [name, def] of Object.entries(ITEMS)) {
  if (!name.startsWith('Obol')) continue;
  const page =
    def?.typeGen === 'aObolCircle' ? 0
      : def?.typeGen === 'aObolSquare' ? 1
        : def?.typeGen === 'aObolHexagon' ? 2
          : 3;
  obols[name] = { page, hyper: name.includes('Hyper') };
}

// Cards: the ledger key (`desc_line1`) and the monster the card belongs to.
const cards = {};
for (const [type, rows] of Object.entries(MonsterDrops)) {
  for (const row of rows || []) {
    const name = String(row[0]);
    if (!name.startsWith('Cards') || cards[name]) continue;
    cards[name] = { key: String(ITEMS[name]?.desc_line1 ?? name), monster: type };
  }
}

writeFileSync(
  new URL('crates/combat/data/drops.json', root),
  JSON.stringify({
    source: 'js/stats/data/game/{custommaps,items,monsters}.js',
    monsters,
    tables: Object.fromEntries([...tables0].sort(([a], [b]) => a.localeCompare(b))),
    obolSplices,
    obols,
    cards,
  })
);

// ---------------------------------------------------------------------------
// Fixture: the upstream module's own results over the same filtered rows.
// ---------------------------------------------------------------------------
const standInPick = (sim) => {
  let first = null;
  for (const mon of sim.monsters) {
    if (mon.alive !== 1 || !(mon.hp > 0) || mon.recycled || mon.willDie !== 0) continue;
    if (mon.temp === 1) return mon;
    if (!first) first = mon;
  }
  return first;
};
const NEUTRAL = {
  respawnRescale: 1,
  monsterHpRescale: 1,
  respawnBonusPct: {},
  crystalSpawnRaw: 0,
  crystalSpawnCap: 0,
  guaranteedCrystalMobs: 0,
  giantMob: 0,
  prayer18: 0,
  chipCrys: 0,
};
const makeSim = (seed) => {
  const player = core.scriptedPlayer(world.defaultPlayerNode(1), {
    killEveryMs: 1_500,
    pick: standInPick,
  });
  const sim = core.createCombatSim({
    saveData: state.saveData,
    charIdx: 0,
    mapIdx: 1,
    seed,
    options: { rawSave: { data: save }, player, inputOverrides: NEUTRAL },
  });
  // A clean stream: the caller owns every draw from here on.
  sim.rng = core.createRng(seed);
  return sim;
};

// The inputs the fixture account carries, so the port's own save reader and the
// upstream input builder are compared on one account.
const inputs = makeSim(1).inputs.values;
const account = {
  dropRate: inputs.dropRate,
  cardChanceMulti: inputs.cardChanceMulti,
  talent29: inputs.talent29,
  bribe1: inputs.bribe1,
  bribe8: inputs.bribe8,
  tasksObol: inputs.tasksObol,
  obolAutoTrash: !!inputs.obolAutoTrash,
  obolAutoTrashRate: inputs.obolAutoTrashRate,
  obolFragmentBalance: inputs.obolFragmentBalance,
  obolInvOwn: inputs.obolInvOwn,
  cards0: inputs.cards0,
};

// 1. DropOdds: upstream `dropOdds` on a known stream.
const oddsCases = [];
{
  const rng = core.createRng(11);
  for (const [e, p, dr] of [
    [1, 0.002, 1],
    [1, 0.002, 4950.574185831907],
    [1, 0.0007, 12],
    [3, 0.25, 1],
    [1, 0.999, 1],
    [2, 0.6, 0.5],
    [1, 0.05, 0],
    [4, 0.11, 3.5],
  ]) {
    oddsCases.push({ e, p, dr, calls: rng.calls, result: rewards.dropOdds(rng, e, p, dr) });
  }
}

// 2. Reset: the filtered card and drop-table rows after `lootDropOddsReset`.
const resetCases = [];
for (const [type, cards0] of [
  ['mushG', { mushG: 0 }],
  ['frogG', { frogG: 3, mushG: 0 }],
  ['wolfA', {}],
]) {
  const sim = makeSim(3);
  sim.state.cards0 = { ...account.cards0, ...cards0 };
  sim.inputs.values = { ...sim.inputs.values, tasksObol: account.tasksObol };
  rewards.lootDropOddsReset(sim);
  const rows = {};
  const sampled = ['mushG', 'frogG', 'wolfA', 'frogP'].filter((name) => monsters[name]);
  for (const name of sampled) {
    rows[name] = keptRows(sim.rewardsState.monsterDrops[name]);
  }
  const tables = {};
  const sampledTables = [...tables0.keys()].filter((name) => name.startsWith('DropTable')).slice(0, 6);
  for (const name of sampledTables) {
    tables[name] = keptRows(sim.rewardsState.dropTables[name]);
  }
  resetCases.push({ cards0, monsters: rows, tables });
}

// 3. Generate: `generateMonsterDrops` over the filtered rows and a known seed.
const generateCases = [];
const obolMonster = Object.entries(monsters).find(([, monster]) =>
  monster.rows.some((row) => row.name.startsWith('Obol'))
)?.[0];
for (const [type, e, seed, calls] of [
  ['mushG', 1, 1, 6],
  ['mushG', 1, 42, 3],
  [obolMonster, 1, 7, 5],
  ['pincermin', 1, 9, 4],
  ['pincermin', 1, 3, 3],
].filter(([type]) => type)) {
  const sim = makeSim(seed);
  sim.rewardsState.monsterDrops = Object.fromEntries(
    Object.entries(sim.rewardsState.monsterDrops).map(([name, rows]) => [name, keptRawRows(rows)])
  );
  sim.rewardsState.dropTables = Object.fromEntries(
    Object.entries(sim.rewardsState.dropTables).map(([name, rows]) => [name, keptRawRows(rows)])
  );
  const rolls = [];
  for (let i = 0; i < calls; i++) {
    rolls.push(rewards.generateMonsterDrops(sim, e, type));
  }
  generateCases.push({ type, e, seed, rolls });
}

// 4. Pickup: `tryPickup` for cards and obols, on a scripted item sequence.
const pickupCases = [];
const pickup = (entries, { autoTrash, balance, rate, own, order, cards0 }) => {
  const sim = makeSim(5);
  const startCalls = sim.rng.calls;
  const v = sim.inputs.values;
  sim.inputs.values = {
    ...v,
    obolAutoTrash: autoTrash,
    obolAutoTrashRate: rate,
    obolFragmentBalance: balance,
  };
  sim.state.obol = { order: structuredClone(order), own: own.slice() };
  sim.state.obolFragments = balance;
  sim.state.cards0 = { ...account.cards0, ...cards0 };
  sim.rewardsState.ledger.obolFragments = 0;
  sim.rewardsState.ledger.obolsAutoTrashed = 0;
  sim.rewardsState.ledger.cards = {};
  rewards.lootDropOddsReset(sim);
  const picked = [];
  for (const [name, amount] of entries) {
    const item = {
      dropType: name,
      dropAmount: amount,
      collectedStatus: 0,
      recycled: false,
      maxCap: 0,
      node: 0,
      x: 0,
      width: 0,
      itemN: 0,
      itemM: 0,
      blockAutoLoot: 0,
      playerDropped: 0,
      dropTypeChecks: 0,
    };
    const taken = rewards.tryPickup(sim, item, { autoLoot: true });
    picked.push(taken);
  }
  return {
    picked,
    calls: sim.rng.calls - startCalls,
    fragments: sim.rewardsState.ledger.obolFragments,
    autoTrashed: sim.rewardsState.ledger.obolsAutoTrashed,
    balance: sim.state.obolFragments,
    cards: sim.rewardsState.ledger.cards,
    pages: sim.state.obol.order,
    cardRows: keptRows(sim.rewardsState.monsterDrops.mushG),
  };
};
const blankPages = () => [
  { 0: 'Blank', 1: 'Blank', 2: 'Blank', 3: 'Blank' },
  { 0: 'Blank', 1: 'Blank', 2: 'ObolSilverPop', 3: 'Blank' },
  {},
  { 0: 'Blank', 1: 'Blank', 2: 'Blank', 3: 'Blank' },
];
const push = (note, entries, options) =>
  pickupCases.push({ note, entries, options, out: pickup(entries, options) });
push(
  'auto-trash on: a full page rolls 0-3 fragments per obol',
  [
    ['ObolBronze0', 1],
    ['ObolBronze0', 1],
    ['ObolBronze0', 1],
    ['ObolBronze0', 1],
    ['ObolBronze0', 1],
    ['ObolBronze0', 1],
    ['ObolSilverDamage', 1],
    ['ObolSilverDamage', 1],
  ],
  { autoTrash: true, balance: 1_999_999_999, rate: 0.28, own: [1, 0, 0, 0], order: blankPages(), cards0: {} }
);
push(
  'auto-trash off: blank slots fill before anything else',
  [
    ['ObolSilverPop', 1],
    ['ObolSilverPop', 1],
    ['ObolBronze0', 1],
    ['ObolBronze0', 1],
  ],
  { autoTrash: false, balance: 0, rate: 0.28, own: [1, 1, 1, 1], order: blankPages(), cards0: {} }
);
push(
  'cards count, and a pickup below three resets the unowned x5 row',
  [
    ['CardsA0', 2],
    ['CardsA0', 3],
  ],
  { autoTrash: true, balance: 0, rate: 0.28, own: [1, 1, 1, 1], order: blankPages(), cards0: { mushG: 0 } }
);

const fixture = {
  source:
    'js/stats/systems/common/combat-sim-rewards.js through script/combat-drops-oracle.mjs',
  account,
  oddsCases,
  resetCases,
  generateCases,
  pickupCases,
};
writeFileSync(
  new URL('fixtures/combat-drops.json', root),
  JSON.stringify(fixture, null, 1)
);
console.log(
  `wrote ${Object.keys(monsters).length} monsters, ${tables0.size} tables, ` +
    `${oddsCases.length} odds, ${resetCases.length} reset, ${generateCases.length} ` +
    `generate and ${pickupCases.length} pickup cases`
);
