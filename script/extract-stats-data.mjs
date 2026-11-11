// Extracts the map and class tables the stats-all calculation context reads.
// Usage: node script/extract-stats-data.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-stats-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const custom = await import(join(source, 'js/stats/data/game/custommaps.js'));
const monsters = await import(join(source, 'js/stats/data/game/monsters.js'));
const dungeon = await import(join(source, 'js/stats/systems/w2/dungeon.js'));

const colosseum = new Set(lists.ColosseumInfo.map((row) => Number(row[2])));
const mapCount = Math.max(lists.MapDispName.length, lists.MapName.length);
const maps = [];
for (let idx = 0; idx < mapCount; idx += 1) {
  const target = String(lists.MapAFKtarget[idx] ?? '');
  maps.push({
    displayName: String(lists.MapDispName[idx] ?? ''),
    internalName: String(lists.MapName[idx] ?? ''),
    afkTarget: target,
    targetName: String(monsters.MONSTERS[target]?.Name ?? ''),
    afkType: String(monsters.MONSTERS[target]?.AFKtype ?? ''),
    sideTargets: Array.isArray(lists.MapAFKtargetSide[idx])
      ? lists.MapAFKtargetSide[idx].map(String)
      : [],
    killReq: Number(lists.MapDetails[idx]?.[0]?.[0]) || 0,
    multicharReq: Number(custom.MapMulticharReq[idx]) || 0,
    colosseum: colosseum.has(idx),
    dungeon: dungeon.DUNGEON_MAPS.has(idx),
    // `portal i` takes `transitions[i + 1]`, upstream's own indexing.
    transitions: (Array.isArray(lists.SceneTransitions[idx]) ? lists.SceneTransitions[idx] : [])
      .filter((row) => Array.isArray(row))
      .map((row) => [Number(row[0]) || 0, String(row[1] ?? '')]),
  });
}

const payload = { maps, classes: lists.ClassNames.map(String) };
writeFileSync(
  new URL('../crates/core/data/stats.json', import.meta.url),
  `${JSON.stringify(payload)}\n`,
);
console.log(`wrote ${maps.length} maps and ${payload.classes.length} class names`);
