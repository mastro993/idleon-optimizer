// Regenerates fixtures/sneaking.json from the upstream Sneaking calculator, so
// the Rust loadout port is tested against the source of truth rather than
// hand-picked numbers.
// Usage: node script/sneaking-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/sneaking-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const math = await import(join(source, 'js/stats/systems/w6/sneaking-math.js'));
const stealth = await import(join(source, 'js/stats/systems/w6/sneaking-stealth.js'));
const jade = await import(join(source, 'js/stats/systems/w6/sneaking-jade.js'));
const exp = await import(join(source, 'js/stats/systems/w6/sneaking-exp.js'));
const optimizer = await import(join(source, 'js/stats/systems/w6/sneaking-optimizer.js'));
const talents = await import(join(source, 'js/stats/systems/common/talent.js'));
const spelunking = await import(join(source, 'js/stats/systems/w7/spelunking.js'));
const maps = await import(join(source, 'js/stats/data/game/custommaps.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save, serverVars: {} });
const S = state.saveData;
const nd = S.ninjaData || [];
const spelunk = S.spelunkData || [];
const ola = S.olaData || [];
if (!nd[103]) throw new Error('the fixture has no Ninja knowledge row');

// The formulas overflow at high mastery on the last floors; the fixture records
// the infinite result as the string "inf" so the Rust test can compare it.
const num = (value) => (Number.isFinite(Number(value)) ? Number(value) : 'inf');
const players = Math.min(
  12,
  (S.charNames || []).length || (S.lv0AllData || []).length || 12,
);
const mastery = Number(ola[231]) || 0;
const masteryMax = Number(ola[232]) || 0;
const knowledge = nd[103];
const omni = S.menagerieData?.[7] || [];
const gold = stealth.goldInventoryBonusDetails(nd, ola, spelunk, S, 0);
const twins = math.parseTwinData(nd);

// The account factors the breakdown trees show. The Rust test compares the
// port's modelled terms against these; the tree roots are recorded for
// reference because a few account systems are still explicit inputs.
const factorMap = (node) =>
  Object.fromEntries((node.children || []).map((child) => [child.name, child.val]));
const allFactors = (node) => {
  const map = { [node.name]: num(node.val) };
  for (const child of node.children || []) Object.assign(map, allFactors(child));
  return map;
};
const summarise = (node) => ({
  name: node.name,
  value: num(node.val),
  children: node.children ? node.children.map(summarise) : [],
});
const floors = Array.from({ length: players }, (_, twin) =>
  Number(nd[twin] && nd[twin][0]) || 0,
);
const twinName = (twin) => (S.charNames || [])[twin] || 'Twin ' + (twin + 1);
const masteryLevel = Number(ola[231]) || 0;
const stealthTree = (twin) =>
  stealth.buildStealthBreakdown(twin, floors[twin], floors, S, 0);
const jadeTree = (twin) => {
  const action = stealth.computeActionSpeed(twin, S, 0);
  const detection = math.detectionChance(
    stealth.computeTwinStealth(twin, floors[twin], floors, S, 0).stealth,
    floors[twin],
    masteryLevel,
  );
  const ko = math.koDuration(S.ninjaData[103] || [], 0, S.menagerieData?.[7]);
  const dodge = stealth.twinCharmBonuses(
    twin,
    nd,
    S.spelunkData || [],
    stealth.goldInventoryBonusDetails(nd, ola, S.spelunkData || [], S, 0).bonuses,
  )[0] || 0;
  return jade.buildJadeBreakdown(twin, S, {
    activeCharIdx: 0,
    floor: floors[twin],
    mastery: masteryLevel,
    twinFloors: floors,
    detection,
    actionSpeed: action,
    koTime: ko,
    dodgePct: dodge,
  });
};
const expTree = (twin) =>
  exp.buildSneakingExpBreakdown(twin, S, {
    activeCharIdx: 0,
    floor: floors[twin],
    mastery: masteryLevel,
    playerCount: players,
  });
const doorTree = (twin) => {
  const action = stealth.computeActionSpeed(twin, S, 0);
  const detection = math.detectionChance(
    stealth.computeTwinStealth(twin, floors[twin], floors, S, 0).stealth,
    floors[twin],
    masteryLevel,
  );
  const ko = math.koDuration(S.ninjaData[103] || [], 0, S.menagerieData?.[7]);
  const dodge = stealth.twinCharmBonuses(
    twin,
    nd,
    S.spelunkData || [],
    stealth.goldInventoryBonusDetails(nd, ola, S.spelunkData || [], S, 0).bonuses,
  )[0] || 0;
  return jade.buildDoorRateBreakdown(twin, S, {
    activeCharIdx: 0,
    actionSpeed: action,
    detection,
    koTime: ko,
    dodgePct: dodge,
    doorActive: false,
  });
};

const equip = (twin) =>
  math.parseTwinEquip(nd, twin).map((item) =>
    item ? { key: item.name, level: item.level } : { key: 'Blank', level: 0 },
  );

const payload = {
  // The saved roster: placement, untie state and the four equipment rows.
  twins: Array.from({ length: players }, (_, twin) => ({
    index: twin,
    name: (S.charNames || [])[twin] || 'Twin ' + (twin + 1),
    floor: twins[twin] ? twins[twin].floor : 0,
    untied: math.isTwinUntied(twin, nd),
    untieProgress: twins[twin] ? twins[twin].untieProgress : 0,
    level: Number(S.lv0AllData?.[twin]?.[17]) || 0,
    equip: equip(twin),
  })),
  // parseInventoryCharms: the visible 39 rows, in order, blanks left out.
  inventory: math.parseInventoryCharms(nd).map((item) => ({
    row: item.slot,
    key: item.key,
    level: item.level,
    stat: item.stat,
    bonusType: item.bonusType,
    isGold: item.isGold,
  })),
  // The inputs the formulas take from the save, so the Rust tests call the pure
  // functions with the same arrays instead of re-reading the save.
  inputs: {
    knowledge,
    omni,
    mastery,
    masteryMax,
    charmed: String(nd[102]?.[9] || '').includes('c'),
    talent432: talents.maxTalentBonus(432, 0, S),
    legend6: spelunking.legendPTSbonus(6, S),
    gemstones: Array.from({ length: 8 }, (_, index) => stealth.gemstoneBonus(index, ola, S, 0)),
  },
  gold: {
    bonuses: gold.bonuses,
    selected: Object.fromEntries(
      Object.entries(gold.selected).map(([type, pick]) => [
        type,
        { row: pick.slot, key: pick.key, raw: pick.raw, value: pick.value },
      ]),
    ),
  },
  charmMaps: Array.from({ length: players }, (_, twin) =>
    stealth.twinCharmBonuses(twin, nd, spelunk, gold.bonuses),
  ),
  actionSpeeds: Array.from({ length: players }, (_, twin) => stealth.computeActionSpeed(twin, S, 0)),
  // The pure functions, with explicit inputs.
  functions: {
    ninjaBonus: [0, 1, 4, 13, 15, 21, 23, 24, 99].map((index) => ({
      index,
      value: math.nkBonus(index, knowledge, omni),
    })),
    actionSpeed: [-50, 0, 120, 5000].map((equipSpeed) => ({
      equipSpeed,
      value: math.actionSpeed(knowledge, equipSpeed, omni),
    })),
    koDuration: [-50, 0, 120, 5000].map((equipKo) => ({
      equipKo,
      value: math.koDuration(knowledge, equipKo, omni),
    })),
    detectionBase: Array.from({ length: 12 }, (_, floor) =>
      [0, mastery].map((level) => ({
        floor,
        mastery: level,
        value: num(math.floorDetectionBase(floor, level)),
      })),
    ).flat(),
    detection: [
      [0, 4],
      [1e3, 4],
      [1e6, 4],
      [1e9, 4],
      [1e52, 4],
      [5, 11],
    ].map(([value, floor]) => ({
      stealth: value,
      floor,
      mastery,
      chance: num(math.detectionChance(value, floor, mastery)),
    })),
    stealthValue: [1, 100, 12345].flatMap((sneakLevel) =>
      [1, 2.5].flatMap((multi) =>
        [0, 120].map((funeralPct) => ({
          sneakLevel,
          multi,
          funeralPct,
          value: num(
            math.stealthValue(sneakLevel, knowledge, multi, {
              omniLevels: omni,
              funeralPct,
            }),
          ),
        })),
      ),
    ),
    doorHp: Array.from({ length: 12 }, (_, floor) =>
      [0, mastery].map((level) => ({
        floor,
        mastery: level,
        value: num(math.doorHP(floor, level)),
      })),
    ).flat(),
    stealthForDetection: Array.from({ length: 12 }, (_, floor) =>
      [0, 0.05, 0.3, 0.99].map((target) => ({
        floor,
        target,
        mastery,
        value: num(math.stealthForDetection(floor, target, mastery)),
      })),
    ).flat(),
    // Every catalog item at five levels: charmStat for charms and
    // weaponItemStat for weapons, zero for hats.
    items: Object.keys(maps.NjEQ).flatMap((key) =>
      [0, 7, 50, 500, 5000].map((level) => ({
        key,
        level,
        charm: math.charmStat(key, level),
        weapon: stealth.weaponItemStat(key, level),
      })),
    ),
    // canEquipCharmPair across the catalog's charm keys.
    pairs: (() => {
      const charms = Object.keys(maps.NjEQ).filter((key) => Number(maps.NjEQ[key][0]) === 2);
      const pairs = [];
      for (const first of ['NjTr4', 'NjTr7', 'NjTr16', 'Blank']) {
        for (const second of ['NjTr4', 'NjTr7', 'NjTr16', 'Blank']) {
          for (const charmed of [false, true]) {
            pairs.push({
              first,
              second,
              charmed,
              allowed: optimizer.canEquipCharmPair(
                { key: first, _empty: first === 'Blank' },
                { key: second, _empty: second === 'Blank' },
                charmed,
              ),
            });
          }
        }
      }
      if (charms.length === 0) throw new Error('the catalog has no charms');
      return pairs;
    })(),
  },
  breakdowns: {
    stealthFactors: Object.fromEntries(
      Object.entries(factorMap(stealthTree(0).children[1])).map(([name, value]) => [name, num(value)]),
    ),
    jadeFactors: Object.fromEntries(
      Object.entries(allFactors(jadeTree(0).children[0].children[1])).map(([name, value]) => [name, num(value)]),
    ),
    expFactors: Object.fromEntries(
      Object.entries(factorMap(expTree(0))).map(([name, value]) => [name, num(value)]),
    ),
    roots: {
      stealth: Array.from({ length: players }, (_, twin) => num(stealthTree(twin).val)),
      jade: Array.from({ length: players }, (_, twin) => num(jadeTree(twin).val)),
      exp: Array.from({ length: players }, (_, twin) => num(expTree(twin).val)),
      door: Array.from({ length: players }, (_, twin) => num(doorTree(twin).val)),
    },
    trees: {
      stealth: summarise(stealthTree(0)),
      jade: summarise(jadeTree(0)),
      exp: summarise(expTree(0)),
      door: summarise(doorTree(0)),
    },
  },
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/save/loader.js',
      'js/stats/systems/w6/sneaking-math.js',
      'js/stats/systems/w6/sneaking-stealth.js',
      'js/stats/systems/w6/sneaking-optimizer.js',
    ],
  },
};

const out = new URL('../fixtures/sneaking.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${payload.twins.length} twins, ${payload.inventory.length} inventory items, ${payload.functions.items.length} item stats`,
);
