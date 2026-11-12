// Extracts the Killroy encounter pools and upgrade list the Rust app ships from
// the upstream JS repo, matching js/stats/data/w2/killroy.js.
// Usage: node script/extract-killroy-data.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-killroy-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const killroy = await import(join(source, 'js/stats/data/w2/killroy.js'));

// The pools `killroyWeek` can pick: the world's base slot, its alternatives, the
// World 5 fallback and the Summoning unlock. Room order is the roll lookup.
const POOLS = [68, 69, 70, 71, 72, 99];

const pools = Object.fromEntries(
  POOLS.map((pool) => {
    const rooms = lists.RANDOlist[pool];
    if (!Array.isArray(rooms)) throw new Error(`unknown Killroy pool ${pool}`);
    return [
      String(pool),
      rooms.map((room) => {
        const mapIdx = lists.MapName.indexOf(room);
        if (mapIdx < 0) throw new Error(`unknown Killroy room ${room}`);
        return {
          mapIdx,
          name: String(lists.MapDispName[mapIdx]).replace(/_/g, ' '),
          monster: lists.MapAFKtarget[mapIdx],
        };
      }),
    ];
  })
);

// The six upgrades the comparison offers, with the effect each one buys.
const upgrades = killroy.KILLROY_UPGRADES.map(({ name, effect }) => ({ name, effect }));

const data = { families: killroy.KILLROY_FAMILIES, upgrades, pools };
writeFileSync(
  join(import.meta.dirname, '../crates/killroy/data/killroy.json'),
  `${JSON.stringify(data, null, 2)}\n`
);
console.log(`wrote ${POOLS.length} pools to crates/killroy/data/killroy.json`);
