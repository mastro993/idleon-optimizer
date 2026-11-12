// Extracts the Jar Cavern (W5) data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-jar-data.mjs <path-to-idleon-research-optimizer>
//
// The jar calculator reads only the tables its formulas touch. Everything else
// stays in the upstream checkout; the JSON records where each row came from so
// a later section can extend the extractor instead of guessing.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-jar-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const game = await import(join(source, 'js/stats/data/game/customlists.js'));
const fountain = await import(join(source, 'js/stats/systems/w5/fountain.js'));
const jar = await import(join(source, 'js/stats/systems/w5/jar.js'));
const gaming = await import(join(source, 'js/stats/data/w4/gaming.js'));
const encoding = await import(join(source, 'js/stats/data/common/encoding.js'));

const holes = game.HolesInfo;
const number = (value) => Number(value) || 0;
const cosmo = (tier, index) => number(game.CosmoUpgrades[tier]?.[index]?.[0]);
const palette = gaming.paletteParams(2);

const payload = {
  // CosmoBonus bases: `cosmoUpgBase(tier, index)` for the three upgrades the
  // jar reads — the monument chain (0, 0), the extra rupie chance (0, 3) and
  // the measurement scale (1, 3).
  cosmo: {
    monument: cosmo(0, 0),
    extraRupie: cosmo(0, 3),
    measurement: cosmo(1, 3),
  },
  // Legend talent points per level for the two talents the jar reads.
  legendPerPoint: Object.fromEntries(
    [10, 29].map((index) => [String(index), number(game.LegendTalents[index]?.[2])]),
  ),
  // `paletteParams(2)`: the Sailing/Gaming palette that grants collectible
  // chance. Linear (not decay) with a per-level coefficient.
  palette2: { coeff: number(palette?.coeff), isDecay: Boolean(palette?.isDecay) },
  // `UPG_DATA[water][upgrade][6]`, the Fountain bonus per level for the monument
  // boost (13) and the Belt/Spelunk total (15) at water 2.
  fountainPerLevel: {
    monument: number(fountain.UPG_DATA[2]?.[13]?.[6]),
    total: number(fountain.UPG_DATA[2]?.[15]?.[6]),
  },
  // HolesInfo[37]: the per-level value of each monument bonus the ROG chain reads.
  monumentBonusPerLevel: (holes[37] || []).map(number),
  // HolesInfo[52] and [55]: each measurement's input type and its base text
  // (some rows scale as `level / (100 + level)`, the `TOT` ones).
  measurementType: (holes[52] || []).map(number),
  measurementBase: holes[55] || [],
  // SuperBit 32's encoding key, matched against `Gaming[12]`.
  superBit32: encoding.N2L[32],
  // `COLLECTIBLE_DATA`: one row per collectible, with the effect text the
  // collectibles tab formats at the saved bonus.
  collectibles: jar.COLLECTIBLE_DATA.map(([name, perLevel, category, effect]) => ({
    name,
    perLevel: number(perLevel),
    category,
    effect,
  })),
  // `CATEGORY_LABELS` and `CATEGORY_COLORS`, keyed by collectible category.
  categoryLabels: jar.CATEGORY_LABELS,
  categoryColors: jar.CATEGORY_COLORS,
  // `B_UPG_DATA`: [index, name, effect, description, currency, cost].
  schematics: jar.B_UPG_DATA.map(([index, name, effect, description, currency, cost]) => ({
    index,
    name,
    effect: number(effect),
    description,
    currency: number(currency),
    cost: number(cost),
  })),
  // `B_UPG_GROUPS`: the schematic groups in display order.
  schematicGroups: Object.entries(jar.B_UPG_GROUPS).map(([name, indices]) => ({
    name,
    indices,
  })),
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/stats/data/game/customlists.js',
      'js/stats/systems/w5/fountain.js',
      'js/stats/systems/w5/jar.js',
      'js/stats/data/w4/gaming.js',
      'js/stats/data/common/encoding.js',
    ],
    holesRows: [37, 52, 55],
  },
};

const out = new URL('../crates/jar/data/jar.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${payload.measurementBase.length} measurement bases, ${payload.monumentBonusPerLevel.length} monument bonuses, ${payload.collectibles.length} collectibles, ${payload.schematics.length} schematics`,
);
