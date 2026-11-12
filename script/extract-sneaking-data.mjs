// Extracts the Sneaking (W6) data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-sneaking-data.mjs <path-to-idleon-research-optimizer>
//
// The loadout and inventory workspaces read the ninja equipment catalog
// (`NjEQ`), the Ninja Knowledge bonuses (`NinjaUpg`), the per-floor and
// per-twin `NinjaInfo` rows the detection, door and untie formulas use, the
// emporium letters that carry the Charmed perk and the legend-talent points
// per level. Everything else stays in the upstream checkout.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-sneaking-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const maps = await import(join(source, 'js/stats/data/game/custommaps.js'));
const hardcoded = await import(join(source, 'js/stats/data/game/hardcoded.js'));
const math = await import(join(source, 'js/stats/systems/w6/sneaking-math.js'));
const alchemy = await import(join(source, 'js/stats/data/w2/alchemy.js'));
const sigils = await import(join(source, 'js/stats/data/common/sigils.js'));
const guild = await import(join(source, 'js/stats/data/common/guild.js'));
const talent = await import(join(source, 'js/stats/data/common/talent.js'));
const stamp = await import(join(source, 'js/stats/data/w1/stamp.js'));
const itemsData = await import(join(source, 'js/stats/data/game/items.js'));

const number = (value) => Number(value) || 0;
const rows = (row, count) => (row || []).slice(0, count);

// NjEQ rows: [type, bonusType, name, stat/desc, desc, max]. Hats keep their
// flavour text in slot 3, weapons their damage and charms their base/max.
const items = {};
for (const [key, row] of Object.entries(maps.NjEQ)) {
  items[key] = {
    type: number(row[0]),
    bonusType: number(row[1]),
    name: String(row[2] || ''),
    stat: String(row[3] ?? ''),
    desc: String(row[4] ?? ''),
    max: number(row[5]),
  };
}

// NinjaUpg rows: name at 4, bonus per level at 5, description at 6.
const upgrades = lists.NinjaUpg.map((row) => ({
  name: String(row?.[4] || ''),
  desc: String(row?.[6] || ''),
  perLevel: number(row?.[5]),
}));

const payload = {
  items,
  upgrades,
  info: {
    // Door HP per floor, the mastery-scaled base the door formulas read.
    doorHp: rows(lists.NinjaInfo[3], 12).map(number),
    // Captive floor per twin: which floor each twin's captive waits on.
    captiveFloor: rows(lists.NinjaInfo[7], 12).map(number),
    // Untie requirement per twin.
    untieRequired: rows(lists.NinjaInfo[8], 12).map(number),
    // Detection base per floor, before the mastery scale.
    detectionBase: rows(lists.NinjaInfo[9], 12).map(number),
    // Jade and EXP base per floor, before the mastery scale.
    jadeBase: rows(lists.NinjaInfo[10], 12).map(number),
    expBase: rows(lists.NinjaInfo[11], 12).map(number),
    // `FarmRankUpgBonusAtLevel`: the per-level Land Rank bonus row.
    farmRankPerLevel: rows(lists.NinjaInfo[36], 20).map(number),
  },
  // The game's per-level mastery scales: NinjaInfo rows the formulas rescale.
  scales: {
    detection: number(lists.NinjaInfo[9]?.[11]),
    doorHp: number(lists.NinjaInfo[3]?.[10]),
    jade: number(lists.NinjaInfo[10]?.[11]),
    exp: number(lists.NinjaInfo[11]?.[11]),
  },
  charmBonusTypes: Object.fromEntries(
    Object.entries(math.CHARM_BONUS_TYPES).map(([index, label]) => [index, label]),
  ),
  // The emporium letters: `emporiumBonus(t, Ninja[102][9])` tests the string.
  emporiumLetters: String(hardcoded.N2L),
  legendTalentPerPoint: lists.LegendTalents.map((row) => number(row?.[2])),
  // The account-wide systems the breakdown trees read. Every table is the
  // upstream data row itself; the formulas stay in the Rust port.
  account: {
    // `HolesInfo[37]`: the monument bonus per slot (also in opal.json).
    monumentInfo: (lists.HolesInfo[37] || []).map(number),
    // `TaskShopDesc[5][3][11]`: the Jade task-shop bonus per level.
    taskShopJadeBase: number(lists.TaskShopDesc?.[5]?.[3]?.[11]),
    // `guildBonusParams(14)`: the formula and coefficients of the EXP guild bonus.
    guild14: guild.guildBonusParams(14),
    // `MSA_Bonus(idx)`: the gaming star milestone config.
    msa: [
      { gate: 0, gateIdx: 7, mult: 1, offset: 0 },
      { gate: 0, gateIdx: 13, mult: 13, offset: 0 },
      { gate: 0, gateIdx: 3, mult: 1, offset: 0 },
      { gate: 0, gateIdx: 20, mult: 50, offset: 0 },
      { gate: 0, gateIdx: 11, mult: 1.12, offset: 0 },
      { gate: 0, gateIdx: 16, mult: 1, offset: 0 },
      { gate: 1, gateIdx: 12, mult: 1.75, offset: 0 },
      { gate: 1, gateIdx: 13, mult: 2, offset: 0 },
      { gate: 1, gateIdx: 14, mult: 1.5, offset: 0 },
      { gate: 0, gateIdx: 27, mult: 2.5, offset: 300 },
      { gate: 0, gateIdx: 44, mult: 0.3, offset: 300 },
    ],
    // `sigilTiers(idx)`: the five tier values of each Summoning sigil.
    sigils: (lists.SigilDesc || []).map((row, idx) => {
      const tiers = sigils.sigilTiers(idx);
      return tiers ? tiers : null;
    }),
    // `AlchemyDescription[4]`: each vial's effect key and curve.
    vials: (lists.AlchemyDescription[4] || []).map((row, index) => row ? {
      index,
      effect: String(row[11] || ''),
      kind: String(row[3] || ''),
      x1: number(row[1]),
      x2: number(row[2]),
    } : null).filter(Boolean),
    // `MealINFO`: each meal's per-level bonus and effect key.
    meals: (lists.MealINFO || []).map((row, index) => row ? {
      index,
      name: String(row[0] || ''),
      perLevel: number(row[2]),
      effect: String(row[5] || ''),
    } : null).filter(Boolean),
    // `bubbleParams`: the bubbles the trees read.
    bubbles: {
      a10: alchemy.bubbleParams(2, 10),
      y8: alchemy.bubbleParams(3, 8),
      y9: alchemy.bubbleParams(3, 9),
    },
    // `TalentDescriptions`: the curves the account terms read.
    talents: {
      56: talent.talentParams(56),
      432: talent.talentParams(432),
      207: talent.talentParams(207),
      431: talent.talentParams(431),
      talent431tab2: talent.talentParams(431, 2),
    },
    // `NinjaInfo[38]`: the voting bonus values.
    votingBonuses: (lists.NinjaInfo[38] || []).map(number),
    // `StarSigns[i][0]`: the name each index unlocks under.
    starSignNames: (lists.StarSigns || []).map((row) => String(row?.[0] || '')),
    // The static `SIGN_BONUSES` table for the three effects the trees read.
    starSignBonuses: {
      Stealth: { 73: 12 },
      Jade: { 75: 10 },
      SneakExp: { 74: 15 },
    },
    // `buildStampTypeMap` for the two stamp effects the trees read.
    stampTypes: (() => {
      const map = {};
      for (const [key, item] of Object.entries(itemsData.ITEMS)) {
        if (!key.startsWith('Stamp') || key.length < 7) continue;
        const parts = String(item.desc_line1 || '').split(',');
        const type = parts[0];
        if (type !== 'JadeCoin' && type !== 'SneakExp') continue;
        const catLetter = key[5];
        const cat = catLetter === 'A' ? 0 : catLetter === 'B' ? 1 : 2;
        const idx = Number(item.ID) - cat * 1000;
        (map[type] ||= []).push({
          cat,
          idx,
          x1: number(parts[2]),
          x2: number(parts[3]),
          formula: String(parts[1] || ''),
        });
      }
      return map;
    })(),
    // `THRESHOLDS` from `computeSkillLvRanks`: a skill total to rank ladder.
    riftRanks: [150, 200, 300, 400, 500, 750, 1000],
    // `SIGN_BONUSES` and the Zenit constants the formulas read.
    zenithPerLevel: number(lists.ZenithMarket?.[2]?.[4] ?? 1),
  },
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/stats/data/game/customlists.js',
      'js/stats/data/game/custommaps.js',
      'js/stats/data/game/hardcoded.js',
      'js/stats/systems/w6/sneaking-math.js',
      'js/stats/data/w2/alchemy.js',
      'js/stats/data/common/sigils.js',
      'js/stats/data/common/guild.js',
      'js/stats/data/common/talent.js',
      'js/stats/data/w1/stamp.js',
      'js/stats/data/w5/hole.js',
    ],
  },
};

const out = new URL('../crates/sneaking/data/sneaking.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${Object.keys(items).length} items, ${upgrades.length} ninja upgrades, ${payload.legendTalentPerPoint.length} legend talents`,
);
