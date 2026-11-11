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

if (rankNames.length !== 20) throw new Error('rank names moved');
if (rankUnlocks.length !== 20) throw new Error('rank unlocks moved');
if (gmoThresholds.length !== 5) throw new Error('GMO thresholds moved');
if (lists.NinjaInfo[36]?.length !== 20) throw new Error('Ninja Rank bases moved');
if (seeds.length !== 7) throw new Error('seed families moved');
if (market.length !== 16) throw new Error('market rows moved');
if (exotics.length !== 80) throw new Error('exotic rows moved');

const payload = {
  seeds,
  market,
  exotics,
  rankNames,
  rankUnlocks,
  ninjaRankBases: lists.NinjaInfo[36].map(Number),
  gmoThresholds,
  generatedFrom: { repo: 'Corgan/idleon-research-optimizer' },
};

const out = new URL('../crates/core/data/farming.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${seeds.length} seed families, ${market.length} market rows, ${exotics.length} exotics`
);
