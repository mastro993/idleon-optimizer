// Extracts the farming data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-farming-data.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-farming-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const farming = await import(join(source, 'js/stats/data/w5/farming.js'));
const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const planner = readFileSync(
  join(source, 'js/stats/systems/w6/farming-planner.js'),
  'utf8'
);
const system = readFileSync(join(source, 'js/stats/systems/w6/farming.js'), 'utf8');
const hardcoded = readFileSync(
  join(source, 'js/stats/data/game/hardcoded.js'),
  'utf8'
);

const seeds = Array.from({ length: farming.seedFamilyCount() }, (_, id) =>
  farming.seedFamily(id)
);
const market = [0, 1].flatMap((page) =>
  Array.from({ length: 8 }, (_, id) => farming.marketUpgrade(page, id))
);
const exotics = Array.from({ length: farming.exoticUpgradeCount() }, (_, id) =>
  farming.exoticUpgrade(id)
);

const rankBlock = planner.match(/var RANK_NAMES = \[([\s\S]*?)\];/)?.[1];
if (!rankBlock) throw new Error('RANK_NAMES moved');
const rankNames = [...rankBlock.matchAll(/'([^']*)'/g)].map((match) => match[1]);
const unlockBlock = planner.match(/var RANK_UNLOCKS = \[([^\]]*)\]/)?.[1];
if (!unlockBlock) throw new Error('RANK_UNLOCKS moved');
const rankUnlocks = unlockBlock.split(',').map((value) => Number(value.trim()));
const thresholdBlock = planner.match(/var GMO_THRESHOLDS = \[([^\]]*)\]/)?.[1];
if (!thresholdBlock) throw new Error('GMO_THRESHOLDS moved');
const gmoThresholds = thresholdBlock.split(',').map((value) => Number(value.trim()));
// The Crop Scientist gates index the emporium encoding: `Ninja[102][9]` holds
// one character per owned emporium item, and `N2L` maps a gate index to it.
const emporiumEncoding = hardcoded.match(/export var N2L = "([^"]*)"/)?.[1];
if (!emporiumEncoding) throw new Error('N2L moved');
const gateBlock = system.match(/var gates = \{([^}]*)\}/)?.[1];
if (!gateBlock) throw new Error('Crop Scientist gates moved');
const cropScientistGates = gateBlock
  .split(',')
  .map((pair) => Number(pair.split(':')[1].trim()));

if (rankNames.length !== 20) throw new Error('rank names moved');
if (rankUnlocks.length !== 20) throw new Error('rank unlocks moved');
if (gmoThresholds.length !== 5) throw new Error('GMO thresholds moved');
if (lists.NinjaInfo[36]?.length !== 20) throw new Error('Ninja Rank bases moved');
if (seeds.length !== 7) throw new Error('seed families moved');
if (market.length !== 16) throw new Error('market rows moved');
if (exotics.length !== 80) throw new Error('exotic rows moved');
if (cropScientistGates.length !== 10) throw new Error('Crop Scientist gates moved');
if (emporiumEncoding.length < 100) throw new Error('emporium encoding moved');
// The upstream indexes the encoding by UTF-16 unit, so a character outside the
// basic plane would shift every gate.
if ([...emporiumEncoding].length !== emporiumEncoding.length) {
  throw new Error('the emporium encoding has astral characters');
}

const payload = {
  seeds,
  market,
  exotics,
  rankNames,
  rankUnlocks,
  ninjaRankBases: lists.NinjaInfo[36].map(Number),
  gmoThresholds,
  emporiumEncoding,
  cropScientistGates,
  // Vault 79 (`Properly Funded Research`) is in the game's no-mastery list, so
  // its Crop Scientist bonus is the saved level times this per-level value.
  vaultCropScientistPerLevel: Number(lists.UpgradeVault[79][5]),
  generatedFrom: { repo: 'Corgan/idleon-research-optimizer' },
};

const out = new URL('../crates/farming/data/farming.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${seeds.length} seed families, ${market.length} market rows, ${exotics.length} exotics`
);
console.log(
  `Crop Scientist: ${cropScientistGates.length} emporium gates, vault 79 per level ${payload.vaultCropScientistPerLevel}`
);
