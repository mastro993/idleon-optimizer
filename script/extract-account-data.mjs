// Extracts the account-side constants the research multiplier tree reads.
// Usage: node script/extract-account-data.mjs <path-to-idleon-research-optimizer>
//
// The research grid data lives in research.json; this file holds the lookups
// the save-derived bonus tree needs: event-shop encodings, shape/pet tables,
// equipment UQ stats and the minehead/jelly/RoG bonus tables.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-account-data.mjs <upstream-repo>');
  process.exit(1);
}

const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
const existing = (() => {
  try {
    return readFileSync(marker, 'utf8');
  } catch {
    return null;
  }
})();
if (!existing) writeFileSync(marker, JSON.stringify({ type: 'module' }));

const read = (path) => readFileSync(join(source, path), 'utf8');
const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const gameHelpers = await import(join(source, 'js/game-helpers.js'));
const encoding = await import(join(source, 'js/stats/data/common/encoding.js'));
const arcade = await import(join(source, 'js/stats/data/w2/arcade.js'));
const sushi = await import(join(source, 'js/stats/data/w7/sushi.js'));
const meritoc = await import(join(source, 'js/stats/data/w7/meritoc.js'));
const minehead = await import(join(source, 'js/stats/data/w7/minehead.js'));
const jelly = await import(join(source, 'js/stats/data/w7/jelly-operator.js'));
const menagerie = await import(join(source, 'js/stats/data/w7/menagerie.js'));
const research = await import(join(source, 'js/stats/data/w7/research.js'));
const cards = await import(join(source, 'js/stats/data/common/cards.js'));
const equipment = await import(join(source, 'js/stats/data/common/equipment.js'));
const gallery = await import(join(source, 'js/stats/data/w7/gallery.js'));
const nametag = await import(join(source, 'js/stats/data/common/nametag.js'));
const vault = await import(join(source, 'js/stats/data/common/vault.js'));
const legend = await import(join(source, 'js/stats/data/w7/legendTalent.js'));
const spelunk = await import(join(source, 'js/stats/data/w7/spelunking.js'));
const farming = await import(join(source, 'js/stats/data/w5/farming.js'));
const sailing = await import(join(source, 'js/stats/data/w5/sailing.js'));
const lab = await import(join(source, 'js/stats/data/w4/lab.js'));
const breeding = await import(join(source, 'js/stats/data/w4/breeding.js'));
const summon = await import(join(source, 'js/stats/data/w7/summon.js'));
const emperor = await import(join(source, 'js/stats/data/common/emperor.js'));
const arcane = await import(join(source, 'js/stats/data/common/arcane.js'));
const alchemy = await import(join(source, 'js/stats/data/w2/alchemy.js'));
const items = await import(join(source, 'js/stats/data/game/items.js'));
const customMaps = await import(join(source, 'js/stats/data/game/custommaps.js'));
const deathNote = await import(join(source, 'js/stats/data/w7/deathNote.js'));
const gaming = await import(join(source, 'js/stats/data/w4/gaming.js'));
const gameConstants = await import(join(source, 'js/stats/data/game-constants.js'));
const hole = await import(join(source, 'js/stats/data/w5/hole.js'));

// Equipment UQ stats: [uq1txt, uq1val, uq2txt, uq2val] for every item that has
// one. The full ITEMS table is 400 KB; this keeps the account file small.
const itemStats = {};
for (const [name, item] of Object.entries(items.ITEMS)) {
  const uq = [item.UQ1txt || '0', Number(item.UQ1val) || 0, item.UQ2txt || '0', Number(item.UQ2val) || 0];
  if (uq[0] !== '0' || uq[2] !== '0') itemStats[name] = uq;
}

// Compact gallery tables: id/name -> [[stat, value], ...].
const uqRows = (map) =>
  Object.fromEntries(Object.entries(map).map(([key, rows]) => [key, rows.map((row) => [row.stat, row.val])]));

/// The vial row whose effect key is `key`, as `{index, kind, x1, x2, name}`.
function vialParams(key) {
  const rows = lists.AlchemyDescription[4] || [];
  const index = rows.findIndex((row) => row && row[11] === key);
  if (index < 0) throw new Error(`the upstream vial table has no ${key} row`);
  const row = rows[index];
  return {
    index,
    kind: String(row[3]),
    x1: Number(row[1]),
    x2: Number(row[2]),
    name: row[0],
  };
}

const payload = {
  encoding: encoding.N2L,
  arcade: Object.fromEntries(
    [27, 35, 38, 51, 52, 54, 59, 62, 63, 67, 69, 72].map((idx) => [idx, arcade.arcadeShopParams(idx)])
  ),
  rogBonus: sushi.ROG_BONUS_QTY,
  mineheadBonus: minehead.MINEHEAD_BONUS_QTY,
  meritocBase: meritoc.MERITOC_BASE,
  arenaThresholds: menagerie.arenaCrucibleThresholds(),
  petArenaThresholds: Array.from({ length: 16 }, (_, idx) => Number(lists.RANDOlist[53]?.[idx]) || 0),
  godTypes: lists.GodsInfo.map((row) => Number(row[13]) || 0),
  jellyReward: lists.Research[47].map(Number),
  stickerBase: lists.Research[25].map(Number),
  dancingCoralBase: lists.Spelunky[24].map(Number),
  menagerieBoost: lists.Research[58].map(Number),
  cardBaseReq: cards.CARD_BASE_REQ,
  equipmentSets: Object.fromEntries(
    ['PREHISTORIC_SET', 'TROLL_SET', 'GODSHARD_SET', 'EMPEROR_SET', 'DEMENTIA_SET'].map((name) => [
      name,
      equipment.equipSetBonus(name),
    ])
  ),
  nametagTier: nametag.NAMETAG_TIER_SCALE,
  nametags: uqRows(gallery.NAMETAG_UQ),
  trophies: uqRows(gallery.TROPHY_UQ),
  hats: uqRows(gallery.HAT_UQ),
  capes: uqRows(gallery.CAPE_UQ),
  items: itemStats,
  etcStats: customMaps.IDforETCbonus,
  labBonus: lab.LAB_BONUS_BASE,
  labDynamic: lab.LAB_BONUS_DYNAMIC,
  jewels: lab.JEWEL_DESC,
  vaultPerLevel: lists.UpgradeVault.map((row) => Number(row[5]) || 0),
  legendPerPoint: lists.LegendTalents.map((row) => Number(row[2]) || 0),
  spelunkUpgPerLevel: lists.SpelunkUpg.map((row) => Number(row[4]) || 0),
  exotic: Object.fromEntries(
    lists.MarketExoticInfo.map((row, idx) => [
      idx,
      { base: Number(row[3]) || 0, decay: Number(row[4]) === 1, denom: 1000 },
    ])
  ),
  artifacts: lists.ArtifactInfo.map((row) => Number(row[3]) || 0),
  tome: lists.Tome.map((row) => [Number(row[1]), Number(row[2]), Number(row[3])]),
  deathNoteMobs: deathNote.DN_MOB_DATA.map((world) => world.map(([idx, req]) => [idx, req])),
  deathNoteMobNames: lists.DeathNoteMobs,
  dungeonRankThresholds: (lists.RANDOlist[29] || []).map(Number),
  petWorldPets: lists.PetStats.map((world) => world.map((pet) => String(pet[0]))),
  petShinyType: breeding.PET_SHINY_TYPE,
  shinyTypeToCat: breeding.SHINY_TYPE_TO_CAT,
  shinyBonusPerLevel: breeding.SHINY_BONUS_PER_LV,
  summonNormalBonus: summon.SUMMON_NORMAL_BONUS,
  summonEndlessType: summon.SUMMON_ENDLESS_TYPE,
  summonEndlessVal: summon.SUMMON_ENDLESS_VAL,
  emperorBonusType: lists.EmperorBon[2].map(Number),
  emperorBonusVal: lists.EmperorBon[1].map(Number),
  companionBonus: lists.CompanionDB.map((row) => Number(row[2]) || 0),
  companionEnhanced: lists.CompanionDB.map((row) => Number(row[11]) || 0),
  chipEffects: lists.ChipDesc.map((row) => String(row[10])),
  vaultNoMastery: Array.from(gameConstants.VAULT_NO_MASTERY),
  palette: Object.fromEntries(
    lists.GamingPalette.map((row, idx) => [
      idx,
      { coeff: Number(row[4]) || 0, decay: Number(row[5]) === 1, denom: 25 },
    ])
  ),
  paletteSuperbitPairs: [
    [49, 25], [51, 13], [52, 31], [54, 18], [58, 3], [61, 12],
  ],
  arcanePerLevel: Array.from({ length: lists.ArcaneUpg.length }, (_, idx) => arcane.arcanePerLevel(idx)),
  arcaneNoMulti: (await import(join(source, 'js/stats/data/game-constants.js'))).ARCANE_NO_MULTI ? Array.from((await import(join(source, 'js/stats/data/game-constants.js'))).ARCANE_NO_MULTI) : [],
  bubbles: {
    y13: alchemy.bubbleParams(3, 32),
    statueStart: alchemy.bubbleParams(3, 2),
    prismaEligible: Number(lists.AlchemyDescription[3][32][10]) === 0,
  },
  // Cooking Mastery per-upgrade bonus (RandoListo2[8]) and the Rift salt lick
  // that boosts it, both read by `cooking-mastery-calc.html`.
  cookingMasteryPerLv: lists.RandoListo2[8].slice(0, 6).map(Number),
  saltLicks: lists.SaltLicks.map((row) => Number(row[3]) || 0),
  // The Canteen Read vial, the one alchemy vial that boosts Cooking Mastery EXP.
  cookingMasteryVial: vialParams('7cookmastery'),
  // The Skinny 0 Cal vial's curve (`AlchemyDescription[4]`, effect
  // `StatueDouble`), as the statue deposit estimate reads it.
  vials: {
    statueDouble: vialParams('StatueDouble'),
  },
  // The Hole rows the Gambit point chain reads (`js/stats/systems/w5/hole.js`).
  hole: {
    monumentBonus: lists.HolesInfo[37].map((value) => Number(value) || 0),
    gambitBonus: lists.HolesInfo[71].map(String),
    jarBonusPerLevel: hole.HOLES_JAR_BONUS_PER_LV,
  },
};

// Unknown fields are dropped so the file stays diffable.
for (const key of Object.keys(payload)) {
  if (payload[key] === undefined) delete payload[key];
}

const out = new URL('../crates/account/data/account.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload));
console.log(
  `wrote ${Object.keys(itemStats).length} item stats, ${payload.tome.length} tome slots, ` +
    `${payload.rogBonus.length} RoG tiers`
);
