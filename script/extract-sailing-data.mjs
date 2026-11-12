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

// The doubled/tripled/... descriptions `computeArtifactBonus` reads at
// `[2 * tier + 1]`; only the ones that spell out the multiplier scale the
// artifact's main bonus, so the others stay at 0.
const TIER_MULTIPLIER_TEXT = {
  2: "The_artifact's_main_bonus_is_doubled!",
  3: "The_artifact's_main_bonus_is_tripled!",
  4: "The_artifact's_main_bonus_is_quadrupled!",
  5: "The_artifact's_main_bonus_is_quintupled!",
  6: "The_artifact's_main_bonus_is_sixtupled!",
  7: "The_artifact's_main_bonus_is_seventupled!",
};

const artifacts = customlists.ArtifactInfo.map((row, id) => ({
  id,
  name: clean(row[0]) || `Artifact ${id}`,
  // The upstream helper reads `[2]` as the tier-zero denominator; zero means
  // "never", which JSON cannot hold as Infinity.
  baseOdds: Number(row[2]) || null,
  baseBonus: Number(row[3]) || 0,
  // Tiers 2-7 store their value at `[2 * tier + 2]`.
  tierBonuses: [2, 3, 4, 5, 6, 7].map((tier) => Number(row[2 * tier + 2]) || 0),
  // Tiers 2-7 double, triple, ... the main bonus when their description says
  // so; 0 means the tier keeps the base bonus.
  mainBonusTierMultipliers: [2, 3, 4, 5, 6, 7].map((tier) =>
    row[2 * tier + 1] === TIER_MULTIPLIER_TEXT[tier] ? tier : 0,
  ),
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
