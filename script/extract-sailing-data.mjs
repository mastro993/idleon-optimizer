// Extracts the sailing tables the Rust app ships from the upstream JS repo.
// Usage: node script/extract-sailing-data.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-sailing-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const customlists = await import(
  join(source, 'js/stats/data/game/customlists.js')
);

const clean = (text) =>
  String(text ?? '')
    .replace(/[+{%}]/g, '')
    .replace(/_/g, ' ')
    .trim();

const artifacts = customlists.ArtifactInfo.map((row, id) => ({
  id,
  name: clean(row[0]) || `Artifact ${id}`,
  // The upstream helper reads `[2]` as the tier-zero denominator; zero means
  // "never", which JSON cannot hold as Infinity.
  baseOdds: Number(row[2]) || null,
  baseBonus: Number(row[3]) || 0,
  // Tiers 2-7 store their value at `[2 * tier + 2]`.
  tierBonuses: [2, 3, 4, 5, 6, 7].map((tier) => Number(row[2 * tier + 2]) || 0),
}));

const islands = customlists.IslandInfo.map((row, id) => ({
  id,
  distance: Number(row[1]) || 0,
  cloudRequired: Number(row[5]) || 0,
  captainExp: Number(row[8]) || 0,
  artifactCount: Number(customlists.IslandInfobox[id]?.[2]) || 0,
}));

const captainBonuses = customlists.CaptainBonuses.map((row, id) => ({
  id,
  name: clean(row[3]) || `Bonus ${id}`,
}));

const payload = { artifacts, islands, captainBonuses };
writeFileSync(
  new URL('../crates/core/data/sailing.json', import.meta.url),
  `${JSON.stringify(payload, null, 2)}\n`,
);
console.log(
  `wrote ${artifacts.length} artifacts, ${islands.length} islands, ${captainBonuses.length} captain bonuses`,
);
