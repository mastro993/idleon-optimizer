// Extracts the Monument (W5) data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-monument-data.mjs <path-to-idleon-research-optimizer>
//
// The Bravery calculator reads only the tables its formulas touch. Everything
// else stays in the upstream checkout; the JSON records where each row came
// from so a later section can extend the extractor instead of guessing.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-monument-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const game = await import(join(source, 'js/stats/data/game/customlists.js'));
const maps = await import(join(source, 'js/stats/data/game/custommaps.js'));
const fountain = await import(join(source, 'js/stats/systems/w5/fountain.js'));
const jar = await import(join(source, 'js/stats/systems/w5/jar.js'));
const minehead = await import(join(source, 'js/stats/data/w7/minehead.js'));
const summon = await import(join(source, 'js/stats/data/w7/summon.js'));

const holes = game.HolesInfo;
const number = (value) => Number(value) || 0;
const rows = (row, count) => (holes[row] || []).slice(0, count);

const payload = {
  holes: {
    // Paying Respect milestone hours and text, slots 0-23.
    milestoneHours: rows(30, 8).map(number),
    milestoneText: rows(31, 24),
    // Permanent reward text and the per-level effect in HolesInfo[37].
    bonusText: rows(32, 30),
    bonusPerLevel: rows(37, 30).map(number),
    // Measurement type and base text (e.g. "5TOT"), used by Bravery damage.
    measurementType: (holes[52] || []).map(number),
    measurementBase: holes[55] || [],
    // Bolaia study rate per level, used by the study bonus.
    bolaiaPerLevel: (holes[70] || []).map(number),
  },
  // CosmoBonus(0, 0) and CosmoBonus(1, 3): base × Holes level, floored.
  cosmo: {
    monument: number(game.CosmoUpgrades[0]?.[0]?.[0]),
    measurement: number(game.CosmoUpgrades[1]?.[3]?.[0]),
  },
  // Fountain_BonTOT(t, 13) per level for the three monument waters.
  fountainPerLevel: [0, 1, 2].map((water) => number(fountain.UPG_DATA[water]?.[13]?.[6])),
  // Minceraft Gem's bonus per collectible level (COLLECTIBLE_DATA[19]).
  collectible19: number(jar.COLLECTIBLE_DATA[19]?.[1]),
  // Legend talent points per level for the entries the monument formulas use.
  legendPerPoint: Object.fromEntries(
    [24, 27, 29].map((index) => [String(index), number(game.LegendTalents[index]?.[2])]),
  ),
  summon: {
    // Undefined game values serialise as null; upstream's Number() turns them
    // into 0 or NaN, which never matches a bonus type. Keep only real rows.
    normal: Object.fromEntries(
      Object.entries(summon.SUMMON_NORMAL_BONUS)
        .map(([name, [kind, value]]) => [name, [Number(kind) || 0, Number(value) || 0]])
        .filter(([, [kind]]) => kind > 0),
    ),
    endlessType: summon.SUMMON_ENDLESS_TYPE.map(number),
    endlessVal: summon.SUMMON_ENDLESS_VAL.map(number),
  },
  // ArcadeShopInfo rows 42 (Monument AFK), 51 (Emperor bonuses), 59 (Meritoc).
  arcade: Object.fromEntries(
    [42, 51, 59].map((index) => [String(index), game.ArcadeShopInfo[index]]),
  ),
  // CompassUpg rows 39/80 feed 55 through its multiplicative type.
  compass: Object.fromEntries(
    [39, 45, 55, 80].map((index) => [String(index), game.CompassUpg[index]]),
  ),
  mineheadBonusQty: minehead.MINEHEAD_BONUS_QTY.map(number),
  // Companion 27 (reindeer) doubles a maxed arcade upgrade.
  companions: {
    27: [number(game.CompanionDB[27]?.[2]), number(game.CompanionDB[27]?.[11])],
  },
  artifact32: number(game.ArtifactInfo[32]?.[3]),
  emperor: {
    val: (game.EmperorBon[1] || []).map(number),
    types: (game.EmperorBon[2] || []).map(number),
  },
  arcane48PerLevel: number(game.ArcaneUpg[48]?.[5]),
  // GODSHARD_SET is the summoning set that raises winner bonuses.
  godshardSet: maps.EquipmentSets.GODSHARD_SET,
  // Jelly reward 28 (Emperor bonuses) and 33 (Meritoc) from the game's
  // Research[47] values; the save decides whether each is unlocked.
  jellyRewards: (game.Research[47] || []).map(number),
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    holesRows: [30, 31, 32, 37, 52, 55, 70],
  },
};

const out = new URL('../crates/core/data/monument.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${payload.holes.milestoneHours.length} milestones, ${payload.holes.bonusPerLevel.length} bonus rows, ${Object.keys(payload.summon.normal).length} summon wins`,
);
