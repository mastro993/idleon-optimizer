// Extracts the Royal Guardian (W7) data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-royal-data.mjs <path-to-idleon-research-optimizer>
//
// RG-01 reads three static tables: the Armory bonus per level (the world unlocks
// and the outpost stat effects are armory levels × that bonus), the resource and
// map anchor positions (a range target is the distance to a node versus the
// outpost's reach), and the map rows that decide which outposts exist at all.
// RG-02 adds the node capacity column, the talent curves the collection rate
// reads beside talent 231, and the three single-row cross-system constants that
// rate multiplies in (the sushi RoG quantity, the Zenith Market step and the
// Arcade shop row). The generator also carries the Spelunk big-fish bases, so
// the section never hard-codes a number it cannot point at.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-royal-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const royal = await import(join(source, 'js/stats/systems/w7/royal-guardian.js'));
const data = await import(join(source, 'js/stats/data/w7/royal-guardian.js'));
const talents = await import(join(source, 'js/stats/data/common/talent.js'));
const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const maps = await import(join(source, 'js/stats/data/common/maps.js'));

const number = (value) => Number(value) || 0;

// `ArmoryUpg[idx][5]` is the bonus per level: an armory upgrade's effect is
// its saved level times that bonus, which is what opens worlds and scales the
// outpost stats. RG-03 extends the extractor with the cost columns.
// `OrbletMarket[idx][4]` is the same kind of per-level bonus.
const armoryBonusPerLevel = data.ARMORY_UPGRADES.map((row) => row.bonusPerLevel);
const orbletBonusPerLevel = data.ORBLET_MARKET.map((row) => row.bonusPerLevel);

// `MapDetails[idx][2]` is the map anchor the outpost range is measured from;
// `outpostKillRequirement(idx)` is the kill requirement the clearing values use:
// the source's RG_KillReq override, else the formula over `MapDetails[idx][0][0]`.
const mapRows = lists.MapDetails.map((row, index) => {
  if (!Array.isArray(row)) return null;
  const anchor = Array.isArray(row[2]) ? row[2] : [];
  return {
    name: maps.mapDisplayName(index),
    x: number(anchor[0]),
    y: number(anchor[1]),
    // `royalMapEligible` decides which maps carry an outpost at all.
    eligible: data.royalMapEligible(index),
    killRequirement: royal.outpostKillRequirement(index),
  };
});

// RoyalResources rows are [map x, map y, base capacity, currency slot].
const resources = data.ROYAL_RESOURCES.map((row) => ({
  x: row.x,
  y: row.y,
  baseCapacity: row.baseCapacity,
  currencySlot: row.currencySlot,
}));

// Spelunky[18] rows are "name,description,base,upgrade"; the big-fish bonus is
// `level / (100 + level) * base`.
const bigFishBase = (lists.Spelunky?.[18] || []).map((row) => number(String(row).split(',')[2]));

// The Royal talents the section reads: `talentParams(id)` reads
// `TalentDescriptions[id][1][0..2]`. 231 is the Xtra Clear Killz rate and 225,
// 226 and 230 are the collection-bonus talents the global rate multiplies in.
const talent = (id) => {
  const params = talents.talentParams(id);
  return { x1: params.x1, x2: params.x2, formula: params.formula };
};
const TALENTS = [225, 226, 230, 231];

// The three cross-system rows the global collection rate reads: the sushi RoG
// quantity at index 60, `ZenithMarket[10][4]` and `ArcadeShopInfo[70]`.
const arcade = await import(join(source, 'js/stats/data/w2/arcade.js'));
const sushi = await import(join(source, 'js/stats/data/w7/sushi.js'));
const sailing = await import(join(source, 'js/stats/data/w5/sailing.js'));
const [arcadeKind, arcadeBase, arcadeDenominator] = arcade.arcadeShopParams(70) ?? [];
const collection = {
  rogBonus60: number(sushi.ROG_BONUS_QTY[60]),
  zenithMarket10: sailing.zenithMarketPerLevel(10),
  arcade70: {
    kind: String(arcadeKind ?? ''),
    base: number(arcadeBase),
    denominator: number(arcadeDenominator),
  },
};

const payload = {
  armoryBonusPerLevel,
  orbletBonusPerLevel,
  resources,
  maps: mapRows,
  talents: Object.fromEntries(TALENTS.map((id) => [String(id), talent(id)])),
  collection,
  bigFishBase,
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'royal-guardian-calc.html',
      'js/stats/systems/w7/royal-guardian.js',
      'js/stats/data/w7/royal-guardian.js',
      'js/stats/data/common/maps.js',
      'js/stats/data/common/talent.js',
      'js/stats/data/game/customlists.js',
      'js/stats/data/w2/arcade.js',
      'js/stats/data/w5/sailing.js',
      'js/stats/data/w7/sushi.js',
    ],
    rows: {
      armory: 'ArmoryUpg',
      resources: 'RoyalResources[i][2..3]',
      maps: 'MapDetails[i][2]',
      talents: 'TalentDescriptions[225,226,230,231][1]',
      rogBonus60: 'Research[37][60]',
      zenithMarket10: 'ZenithMarket[10][4]',
      arcade70: 'ArcadeShopInfo[70][1..3]',
      bigFishBase: 'Spelunky[18]',
    },
  },
};

writeFileSync(
  new URL('../crates/core/data/royal-guardian.json', import.meta.url),
  `${JSON.stringify(payload)}\n`,
);
console.log(
  `wrote ${armoryBonusPerLevel.length} armory rows, ${resources.length} resources, ${mapRows.filter(Boolean).length} maps`,
);
