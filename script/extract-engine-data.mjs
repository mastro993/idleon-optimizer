// Extracts the static tables the shared stat engine reads: the item fields the
// weapon-power chain needs, star signs, alchemy bubbles and vials, sigil tiers,
// cards, post-office box rewards, guild bonuses, achievements, atoms, artifacts,
// the owl, golden food and the Cosmo/statue tables.
// Usage: node script/extract-engine-data.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-engine-data.mjs <upstream-repo>');
  process.exit(1);
}

const marker = join(source, 'package.json');
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
let markerType = null;
try {
  markerType = JSON.parse(spawnSync('cat', [marker]).stdout?.toString() || '{}').type;
} catch {
  markerType = null;
}
if (markerType !== 'module') writeFileSync(marker, JSON.stringify({ type: 'module' }));

const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const constants = await import(join(source, 'js/stats/data/game-constants.js'));
const hardcoded = await import(join(source, 'js/stats/data/game/hardcoded.js'));
const items = await import(join(source, 'js/stats/data/game/items.js'));
const customMaps = await import(join(source, 'js/stats/data/game/custommaps.js'));
const sigils = await import(join(source, 'js/stats/data/common/sigils.js'));
const cards = await import(join(source, 'js/stats/data/common/cards.js'));
const guild = await import(join(source, 'js/stats/data/common/guild.js'));
const goldenFood = await import(join(source, 'js/stats/data/common/goldenFood.js'));
const sailing = await import(join(source, 'js/stats/data/w5/sailing.js'));
const hole = await import(join(source, 'js/stats/data/w5/hole.js'));
const farming = await import(join(source, 'js/stats/data/w5/farming.js'));
const promotion = (await import(join(source, 'js/stats/data/game/customlists.js'))).ClassPromotionChoices;

// `ITEMS[name]` fields the damage chain reads: weapon power, the food effect
// list and the stamp description lines the type table is built from.
const itemFields = {};
for (const [name, item] of Object.entries(items.ITEMS)) {
  const row = {};
  if (Number(item.Weapon_Power)) row.wp = Number(item.Weapon_Power);
  for (const stat of ['STR', 'AGI', 'WIS', 'LUK', 'Defence']) {
    if (Number(item[stat])) row[stat] = Number(item[stat]);
  }
  if (item.Type) row.type = String(item.Type);
  if (item.Effect) row.effect = String(item.Effect);
  if (Number(item.Amount)) row.amount = Number(item.Amount);
  if (item.desc_line1) row.desc = String(item.desc_line1);
  if (Object.keys(row).length) itemFields[name] = row;
}

// `AlchemyDescription[c][i]`: [name, x1, x2, formula, ..., key at 11 (vials) or
// 15 (bubbles)]. `bubbleValByKey` walks the four cauldrons in order.
const bubbles = {};
const vials = {};
for (let cauldron = 0; cauldron < 4; cauldron += 1) {
  const rows = lists.AlchemyDescription[cauldron] || [];
  for (let index = 0; index < rows.length; index += 1) {
    const row = rows[index];
    if (!row) continue;
    if (row[15]) {
      bubbles[row[15]] = {
        name: String(row[0] ?? ''),
        cauldron,
        index,
        x1: Number(row[1]) || 0,
        x2: Number(row[2]) || 0,
        formula: String(row[3] ?? ''),
        // `BUBBLE_KEYS` in `js/stats/systems/common/stats.js`: the slab and
        // tome bubbles scale by discovered items and Tome points.
        slab: ['W4', 'A4', 'M4'].includes(row[15]),
        tome: ['W8', 'A9', 'M9'].includes(row[15]),
      };
    }
  }
}
// `bubbleParams(0, 18)` is the golden-food bubble the damage chain reads by
// its cauldron and index rather than by an effect key.
bubbles.O18 = (() => {
  const row = lists.AlchemyDescription[0][18];
  return {
    name: String(row[0] ?? ''),
    cauldron: 0,
    index: 18,
    x1: Number(row[1]) || 0,
    x2: Number(row[2]) || 0,
    formula: String(row[3] ?? ''),
  };
})();

for (const [index, row] of (lists.AlchemyDescription[4] || []).entries()) {
  if (!row || !row[11]) continue;
  const key = String(row[11]);
  (vials[key] ||= []).push({
    name: String(row[0] ?? ''),
    index,
    x1: Number(row[1]) || 0,
    x2: Number(row[2]) || 0,
    formula: String(row[3] ?? ''),
  });
}

// `BIG_BUBBLE_MULTIPLIERS` in `js/stats/systems/w2/alchemy.js`: the bubble each
// cauldron's slab and tome bubbles multiply by.
const bigBubbles = [
  { indices: [0, 2, 4, 7, 14], key: 'MultiOr' },
  { indices: [0, 6, 9, 12, 14], key: 'MultiGr' },
  { indices: [0, 2, 6, 12, 14], key: 'MultiPu' },
];

// `SIGN_BONUSES` in `js/stats/systems/common/starSign.js`: the hardcoded
// per-sign effect values (the description strings do not match the keys).
const starSignBonuses = {
  FightAFK: { 19: 2, 28: 6, 29: -6, 56: 4 },
  SkillAFK: { 20: 2, 25: 1, 29: -6, 55: 4 },
  SkillEXP: { 30: 3, 50: 6 },
  MainXP: { 2: 1, 24: 3, 52: 6 },
  WorshExp: { 46: 15 },
  Drop: { 14: 5, 76: 12 },
  PctDmg: { 0: 1, 32: 2, 51: 20, 53: 6, 54: 15, 70: 25 },
  WepPow: { 12: 2 },
  AccPct: { 13: 4, 35: 10 },
  CritChance: { 27: 4 },
  DivExp: { 62: 30 },
  MoveSpd: { 1: 2, 8: 4, 13: 2, 32: -3, 51: -12 },
  TotalHP: { 28: -80 },
  DefPct: { 12: 6, 28: -50, 32: 5, 36: 10 },
  FoodEffect: { 22: 15 },
  GFood: { 69: 20 },
  Jade: { 75: 10 },
  Stealth: { 73: 12 },
  SneakExp: { 74: 15 },
  ArtifactFind: { 61: 15 },
  SailingSpd: { 63: 20 },
  CropEvoPerFarmLv: { 65: 3 },
  FarmingEXP: { 66: 20 },
  OGChance: { 67: 15 },
  MobRespawn: { 26: 2, 49: 4 },
  pctCardDrop: { 41: 15 },
  CarryCap: { 11: 10, 25: 5, 29: 30 },
  MultiFish: { 6: 20 },
  CookSpd: { 58: 15 },
};

// `ACHIEVEMENT_TIER_VALUE` in `js/stats/systems/common/achievement.js` and the
// tiered `ACHIEVE_STATUS` table in hardcoded.js.
const achievements = {};
for (const index of Object.keys(hardcoded.ACHIEVE_STATUS)) {
  achievements[Number(index)] = Number(index);
}
const achievementTiers = Object.fromEntries(
  Object.entries(hardcoded.ACHIEVE_STATUS).flatMap(([tier, ids]) => ids.map((id) => [id, Number(tier)]))
);
for (const id of [4, 27, 37, 44, 107, 109, 117]) achievementTiers[id] = 5;
for (const id of [108]) achievementTiers[id] = 10;
for (const id of [99, 104, 112]) achievementTiers[id] = 20;

// Guild bonuses: `guildBonusParams(i)` until the table ends.
const guilds = [];
for (let index = 0; ; index += 1) {
  const row = guild.guildBonusParams(index);
  if (!row) break;
  guilds.push({ name: String(row.name ?? ''), x1: Number(row.x1) || 0, x2: Number(row.x2) || 0, formula: String(row.formula) });
}

// `cosmoUpgBase(tier, index)`: the Pocket Divinity rows.
const cosmo = [];
for (let tier = 0; tier < 5; tier += 1) {
  const row = [];
  for (let index = 0; index < 16; index += 1) {
    const value = hole.cosmoUpgBase(tier, index);
    if (!Number.isFinite(value)) break;
    row.push(value);
  }
  if (row.length) cosmo.push(row);
}

const data = {
  generatedFrom: 'Corgan/idleon-research-optimizer',
  items: itemFields,
  starSigns: (lists.StarSigns || []).map((row) => row.map((cell) => String(cell))),
  starSignBonuses,
  starSignNames: (lists.StarSigns || []).map((row) => String(row[0])),
  bubbles,
  bigBubbles,
  vials,
  sigils: Array.from({ length: 24 }, (_, index) => sigils.sigilTiers(index)),
  cards: {
    bonus: cards.CARD_BONUS,
    idForBonus: customMaps.IDforCardBonus,
    idForSet: customMaps.IDforCardSETbonus,
  },
  boxRewards: lists.PostOffUpgradeInfo,
  guilds,
  // `achieveStatus(idx, …)`: the rule is the completed flag plus this value.
  achievements: achievementTiers,
  achievementColumns: achievements,
  atoms: (lists.AtomInfo || []).map((row, index) => ({
    index,
    perLevel: Number(row[4]) || 0,
  })),
  artifacts: Array.from({ length: sailing.artifactCount() }, (_, index) => sailing.artifactBase(index)),
  owlBase: constants.OWL_BASE,
  companionBonus: constants.COMPANION_BONUS,
  goldFood: { info: goldenFood.GOLD_FOOD_INFO, slots: goldenFood.EMPORIUM_FOOD_SLOTS },
  meals: (lists.MealINFO || []).map((row, index) => ({
    index,
    name: String(row[0] ?? ''),
    perLevel: Number(row[2]) || 0,
    effect: String(row[5] ?? ''),
  })),
  cosmo,
  saltLicks: (lists.SaltLicks || []).map((row) => Number(row[3]) || 0),
  statues: (lists.StatueInfo || []).map((row) => Number(row[3]) || 0),
  // Every stamp item's `desc_line1` names its bonus type and curve.
  stamps: Object.entries(items.ITEMS).flatMap(([name, item]) => {
    const match = /^Stamp([ABC])(\d+)$/.exec(name);
    if (!match || !item.desc_line1) return [];
    const [kind, formula, x1, x2] = String(item.desc_line1).split(',');
    return [{
      item: name,
      kind,
      cat: 'ABC'.indexOf(match[1]),
      idx: Number(match[2]) - 1,
      formula: formula ?? '',
      x1: Number(x1) || 0,
      x2: Number(x2) || 0,
    }];
  }),
  farmRankPerLevel: farming.ninjaInfo(36),
  // `DungPassiveStats2[idx]`: the Flurbo shop's passive curves.
  dungPassive: (lists.DungPassiveStats2 || []).map((row) =>
    row ? {
      x1: Number(row[1]) || 0,
      x2: Number(row[2]) || 0,
      formula: String(row[3] ?? ''),
    } : null
  ),
  classParent: Object.fromEntries(
    promotion.flatMap((row, parent) =>
      row && row[0] !== 'Na' ? row.map((child) => [Number(child), parent]) : []
    )
  ),
};

writeFileSync(
  join(import.meta.dirname, '..', 'crates/stats-engine/data/engine.json'),
  `${JSON.stringify(data)}\n`
);
console.log(
  `wrote ${Object.keys(itemFields).length} items, ${Object.keys(bubbles).length} bubbles, ` +
    `${Object.keys(vials).length} vials and ${guilds.length} guild bonuses`
);
