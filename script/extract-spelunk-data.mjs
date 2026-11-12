// Extracts the Spelunking (W7) data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-spelunk-data.mjs <path-to-idleon-research-optimizer>
//
// The section reads the cave and rock tables, the tool and elixir rows, the
// shop's per-level bonus column, and the elixir names/presets the upstream page
// hardcodes. Everything else stays in the upstream checkout.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-spelunk-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const game = await import(join(source, 'js/stats/data/game/customlists.js'));
const spelunking = await import(join(source, 'js/stats/data/w7/spelunking.js'));
const expedition = await import(join(source, 'js/stats/systems/w7/spelunk-expedition.js'));

// `js/ui/spelunking.js` keeps the display names and the built-in presets
// beside the markup. They are upstream constants, not save data, so they are
// extracted here instead of being retyped in Rust.
const page = readFileSync(join(source, 'js/ui/spelunking.js'), 'utf8');
const namesMatch = /const elixirNames = \[([\s\S]*?)\];/.exec(page);
if (!namesMatch) throw new Error('elixirNames not found in js/ui/spelunking.js');
const elixirNames = Function(`return [${namesMatch[1]}]`)();
const presetsMatch = /const elixirPresets = \[([\s\S]*?)\];\nconst policyNames/.exec(page);
if (!presetsMatch) throw new Error('elixirPresets not found in js/ui/spelunking.js');
const elixirPresets = Function(`return [${presetsMatch[1]}]`)();

const number = (value) => Number(value) || 0;

const payload = {
  caves: spelunking.spelunkCaves(),
  tools: spelunking.spelunkTools(),
  elixirs: spelunking.spelunkElixirs().map((elixir) => ({
    ...elixir,
    name: elixirNames[elixir.id] ?? `Elixir ${elixir.id}`,
  })),
  elixirPresets,
  policies: expedition.SPELUNK_POLICIES,
  // SpelunkUpg[idx][4]: the per-level value `shopUpgBonus` multiplies.
  upgradePerLevel: Array.from(
    { length: game.SpelunkUpg.length },
    (_, index) => number(game.SpelunkUpg[index]?.[4]),
  ),
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    caves: 'SpelunkRocks / Spelunky[0,6,7,8,9]',
    tools: 'Spelunky[10..13]',
    elixirs: 'Spelunky[3,16,17]',
    shop: 'SpelunkUpg[idx][4]',
    ui: 'js/ui/spelunking.js elixirNames/elixirPresets',
  },
};

const out = new URL('../crates/core/data/spelunking.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${payload.caves.length} caves, ${payload.tools.length} tools, ${payload.elixirs.length} elixirs, ${payload.upgradePerLevel.length} shop rows`,
);
