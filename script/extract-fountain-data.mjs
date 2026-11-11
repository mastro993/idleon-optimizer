// Extracts the Fountain (W5) data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-fountain-data.mjs <path-to-idleon-research-optimizer>
//
// The fountain calculator reads the upgrade table, the water and currency names,
// and the handful of account rows its cross-system bonuses consume. Everything
// else stays in the upstream checkout; the JSON records where each row came
// from so FN-02 to FN-04 can extend the extractor instead of guessing.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-fountain-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const fountain = await import(join(source, 'js/stats/systems/w5/fountain.js'));
const game = await import(join(source, 'js/stats/data/game/customlists.js'));
const arcade = await import(join(source, 'js/stats/data/w2/arcade.js'));

const number = (value) => Number(value) || 0;
const strings = (row) => (row || []).map((value) => String(value));

const payload = {
  waterNames: fountain.WATER_NAMES,
  waterColors: fountain.WATER_COLORS,
  currencyNames: fountain.CURRENCY_NAMES,
  // The page's short labels, which the cost and planner columns print.
  currencyShort: ['Bronze', 'Silver', 'Gold', 'Dollar', 'Credit', 'T-Bill', 'Moolah', 'Shilling', 'Greane'],
  barFillTimes: [7200, 36000, 90000],
  // UPG_DATA: [name, prereq, xy, currency, baseCost, costScale, perLevel, desc].
  upgrades: fountain.UPG_DATA.map((water) =>
    water.map((row) => ({
      name: row[0],
      prereq: typeof row[1] === 'number' ? row[1] : -1,
      x: number(String(row[2]).split(',')[0]),
      y: number(String(row[2]).split(',')[1]),
      currency: number(row[3]),
      baseCost: number(row[4]),
      costScale: number(row[5]),
      perLevel: number(row[6]),
      description: row[7],
    })),
  ),
  holes: {
    // Measurement type and base text (e.g. "10TOT") for every hole upgrade.
    measurementType: (game.HolesInfo[52] || []).map(number),
    measurementBase: strings(game.HolesInfo[55]),
    // Bolaia study rate per level, read by the marble-per-fill bonus.
    bolaiaPerLevel: (game.HolesInfo[70] || []).map(number),
    // Bell coin bonus per level. HolesInfo[59] pairs a label with a value, so
    // the upstream's HI59_PERLV table is the odd entries only.
    bellPerLevel: Array.from({ length: 7 }, (_, index) => number(game.HolesInfo[59]?.[2 * index + 1])),
  },
  // CosmoBonus bases the value chain reads: Max Coin Stack (0,4) and the
  // measurement multiplier (1,3).
  cosmo: {
    maxStack: number(game.CosmoUpgrades[0]?.[4]?.[0]),
    measurement: number(game.CosmoUpgrades[1]?.[3]?.[0]),
  },
  // ArcadeShopInfo[68] "Fountain Speed": [formula, base, denominator].
  arcadeFountainSpeed: (() => {
    const params = arcade.arcadeShopParams(68);
    return { type: params[0], base: number(params[1]), denom: number(params[2]) };
  })(),
  // The lamp-wish bonus (Holes[21][7]) is 25 per level.
  lampPerLevel: 25,
  // Measurement type 8 counts cards in `Cards1` this many to the unit.
  cardsPerMeasurementUnit: 150,
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: ['js/stats/systems/w5/fountain.js', 'js/stats/data/game/customlists.js'],
    holesRows: [52, 55, 59, 70],
  },
};

const out = new URL('../crates/core/data/fountain.json', import.meta.url);
writeFileSync(out, `${JSON.stringify(payload, null, 1)}\n`);
console.log(
  `wrote ${payload.upgrades.length} waters x ${payload.upgrades[0]?.length} upgrades, ${payload.holes.measurementType.length} measurements`,
);
