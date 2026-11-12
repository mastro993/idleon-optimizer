// Extracts the Royal Guardian (W7) data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-royal-data.mjs <path-to-idleon-research-optimizer>
//
// RG-01 reads three static tables: the Armory bonus per level (the world unlocks
// and the outpost stat effects are armory levels × that bonus), the resource and
// map anchor positions (a range target is the distance to a node versus the
// outpost's reach), and the map rows that decide which outposts exist at all.
// The generator also carries the talent-231 curve and the Spelunk big-fish bases
// the clearing rate multiplies in, so the section never hard-codes a number it
// cannot point at.
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
  currencySlot: row.currencySlot,
}));

// Spelunky[18] rows are "name,description,base,upgrade"; the big-fish bonus is
// `level / (100 + level) * base`.
const bigFishBase = (lists.Spelunky?.[18] || []).map((row) => number(String(row).split(',')[2]));

// The Xtra Clear Killz talent: `talentParams(231)` reads
// `TalentDescriptions[231][1][0..2]`, a `decayMulti` curve.
const talent = (id) => {
  const params = talents.talentParams(id);
  return { x1: params.x1, x2: params.x2, formula: params.formula };
};

const payload = {
  armoryBonusPerLevel,
  orbletBonusPerLevel,
  resources,
  maps: mapRows,
  talent231: talent(231),
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
    ],
    rows: {
      armory: 'ArmoryUpg',
      resources: 'RoyalResources',
      maps: 'MapDetails[i][2]',
      talent: 'TalentDescriptions[225..239][1]',
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
