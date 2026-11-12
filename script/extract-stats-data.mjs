// Extracts the static tables the stats-all calculation context and its
// category breakdowns read: maps, classes, and the carry-capacity systems
// (guild, bribe, prayer, shrine, talent, stamp, star sign, vault, pristine
// charm, companion).
// Usage: node script/extract-stats-data.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-stats-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const custom = await import(join(source, 'js/stats/data/game/custommaps.js'));
const monsters = await import(join(source, 'js/stats/data/game/monsters.js'));
const dungeon = await import(join(source, 'js/stats/systems/w2/dungeon.js'));
const items = await import(join(source, 'js/stats/data/game/items.js'));
const compass = await import(join(source, 'js/stats/data/common/compass.js'));
const equipment = await import(join(source, 'js/stats/data/common/equipment.js'));
const sushi = await import(join(source, 'js/stats/data/w7/sushi.js'));
const breeding = await import(join(source, 'js/stats/data/w4/breeding.js'));
const encoding = await import(join(source, 'js/stats/data/common/encoding.js'));
const farmingData = await import(join(source, 'js/stats/data/w5/farming.js'));
const exotic49 = farmingData.exoticParams(49);
const constants = await import(join(source, 'js/stats/data/game-constants.js'));
const gaming = await import(join(source, 'js/stats/data/w4/gaming.js'));
const palette23 = gaming.paletteParams(23);
const { readFileSync } = await import('node:fs');

const colosseum = new Set(lists.ColosseumInfo.map((row) => Number(row[2])));
const mapCount = Math.max(lists.MapDispName.length, lists.MapName.length);
const maps = [];
for (let idx = 0; idx < mapCount; idx += 1) {
  const target = String(lists.MapAFKtarget[idx] ?? '');
  maps.push({
    displayName: String(lists.MapDispName[idx] ?? ''),
    internalName: String(lists.MapName[idx] ?? ''),
    afkTarget: target,
    targetName: String(monsters.MONSTERS[target]?.Name ?? ''),
    afkType: String(monsters.MONSTERS[target]?.AFKtype ?? ''),
    sideTargets: Array.isArray(lists.MapAFKtargetSide[idx])
      ? lists.MapAFKtargetSide[idx].map(String)
      : [],
    killReq: Number(lists.MapDetails[idx]?.[0]?.[0]) || 0,
    multicharReq: Number(custom.MapMulticharReq[idx]) || 0,
    colosseum: colosseum.has(idx),
    dungeon: dungeon.DUNGEON_MAPS.has(idx),
    // `portal i` takes `transitions[i + 1]`, upstream's own indexing.
    transitions: (Array.isArray(lists.SceneTransitions[idx]) ? lists.SceneTransitions[idx] : [])
      .filter((row) => Array.isArray(row))
      .map((row) => [Number(row[0]) || 0, String(row[1] ?? '')]),
  });
}

// Carry capacity reads these systems through upstream's own resolvers. Each
// table keeps its row order, so a save index reads the row it names upstream.
const guilds = lists.GuildBonuses.map((row) => ({
  x1: Number(row[4]) || 0,
  x2: Number(row[5]) || 0,
  formula: String(row[6] ?? ''),
}));
const bribes = lists.BribeDescriptions.map((row) => Number(row[5]) || 0);
const prayers = lists.PrayerInfo.map((row) => [Number(row[3]) || 0, Number(row[4]) || 0]);
const shrines = lists.ShrineInfo.map((row) => [Number(row[2]) || 0, Number(row[3]) || 0]);
const talents = {};
for (const id of [78, 634]) {
  const curve = lists.TalentDescriptions[id]?.[1] ?? [];
  talents[id] = {
    formula: String(curve[2] ?? ''),
    x1: Number(curve[0]) || 0,
    x2: Number(curve[1]) || 0,
  };
}
const pristineCharms = {};
for (const [key, row] of Object.entries(custom.NjEQ ?? {})) {
  const match = /^NjTrP(\d+)$/.exec(key);
  if (match) pristineCharms[Number(match[1])] = Number(row?.[3]) || 0;
}
// `buildStampTypeMap` in js/stats/systems/w1/stamp.js: stamp items carry
// `type,formula,x1,x2,overlevelStep,...,skillIndex` in their first description.
const stamps = [];
for (const [key, item] of Object.entries(items.ITEMS)) {
  if (!key.startsWith('Stamp') || key.length < 7) continue;
  const cat = { A: 0, B: 1, C: 2 }[key[5]];
  if (cat == null) continue;
  const parts = String(item.desc_line1 ?? '').split(',');
  stamps.push({
    item: key,
    type: parts[0],
    cat,
    idx: Number(item.ID) - cat * 1000,
    formula: String(parts[1] ?? ''),
    x1: Number(parts[2]) || 0,
    x2: Number(parts[3]) || 0,
    overlevelStep: Number(parts[4]) || 0,
    skillIdx: Number(parts[10]) || 0,
  });
}

function carryStarSigns() {
  const text = readFileSync(
    join(source, 'js/stats/systems/common/starSign.js'),
    'utf8',
  );
  const line = /CarryCap:\s*\{([^}]*)\}/.exec(text);
  if (!line) throw new Error('starSign.js no longer defines SIGN_BONUSES.CarryCap');
  return line[1]
    .split(',')
    .filter((entry) => entry.trim() !== '')
    .map((entry) => {
      const [sign, value] = entry.split(':').map((part) => Number(part.trim()));
      return [sign, value];
    });
}

const payload = {
  maps,
  classes: lists.ClassNames.map(String),
  capacity: {
    numberLetter: String(encoding.N2L),
    boss3bCardPct: Number(constants.BOSS3B_CARD_PCT) || 0,
    guilds,
    bribes,
    prayers,
    shrines,
    talents,
    pristineCharms,
    stamps,
    compassPerLevel: lists.CompassUpg.map((_, index) => Number(compass.compassUpgPerLevel(index)) || 0),
    emperorSet: Number(equipment.equipSetBonus('EMPEROR_SET')) || 0,
    rogBonus: Array.from({ length: 60 }, (_, index) =>
      Number(sushi.ROG_BONUS_QTY[index]) || 0,
    ),
    legendPerPoint: lists.LegendTalents.map((row) => Number(row[2]) || 0),
    palette23: {
      coeff: Number(palette23?.coeff) || 0,
      denom: Number(palette23?.denom) || 0,
      decay: Boolean(palette23?.isDecay),
    },
    jellyReward50: Number(lists.Research?.[47]?.[50]) || 0,
    exotic49: { base: Number(exotic49?.base) || 0, decay: exotic49?.type === 'decay' },
    petShinyType: breeding.PET_SHINY_TYPE.map((row) => row.map((kind) => Number(kind) || 0)),
    shinyTypeToCat: breeding.SHINY_TYPE_TO_CAT.map(Number),
    shinyBonusPerLevel: breeding.SHINY_BONUS_PER_LV.map(Number),
    starSignNames: lists.StarSigns.map((row) => String(row[0] ?? '')),
    // `SIGN_BONUSES` in js/stats/systems/common/starSign.js is module-local, so
    // the table is read from its source instead of imported.
    carryStarSigns: carryStarSigns(),
  },
};
writeFileSync(
  new URL('../crates/core/data/stats.json', import.meta.url),
  `${JSON.stringify(payload)}\n`,
);
console.log(
  `wrote ${maps.length} maps, ${payload.classes.length} class names and ${stamps.length} stamps`,
);
