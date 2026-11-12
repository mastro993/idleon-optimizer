// Extracts the Sneaking (W6) data the Rust app ships from the upstream JS repo.
// Usage: node script/extract-sneaking-data.mjs <path-to-idleon-research-optimizer>
//
// The loadout and inventory workspaces read the ninja equipment catalog
// (`NjEQ`), the Ninja Knowledge bonuses (`NinjaUpg`), the per-floor and
// per-twin `NinjaInfo` rows the detection, door and untie formulas use, the
// emporium letters that carry the Charmed perk and the legend-talent points
// per level. Everything else stays in the upstream checkout.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-sneaking-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const maps = await import(join(source, 'js/stats/data/game/custommaps.js'));
const hardcoded = await import(join(source, 'js/stats/data/game/hardcoded.js'));
const math = await import(join(source, 'js/stats/systems/w6/sneaking-math.js'));

const number = (value) => Number(value) || 0;
const rows = (row, count) => (row || []).slice(0, count);

// NjEQ rows: [type, bonusType, name, stat/desc, desc, max]. Hats keep their
// flavour text in slot 3, weapons their damage and charms their base/max.
const items = {};
for (const [key, row] of Object.entries(maps.NjEQ)) {
  items[key] = {
    type: number(row[0]),
    bonusType: number(row[1]),
    name: String(row[2] || ''),
    stat: String(row[3] ?? ''),
    desc: String(row[4] ?? ''),
    max: number(row[5]),
  };
}

// NinjaUpg rows: name at 4, bonus per level at 5, description at 6.
const upgrades = lists.NinjaUpg.map((row) => ({
  name: String(row?.[4] || ''),
  desc: String(row?.[6] || ''),
  perLevel: number(row?.[5]),
}));

const payload = {
  items,
  upgrades,
  info: {
    // Door HP per floor, the mastery-scaled base the door formulas read.
    doorHp: rows(lists.NinjaInfo[3], 12).map(number),
    // Captive floor per twin: which floor each twin's captive waits on.
    captiveFloor: rows(lists.NinjaInfo[7], 12).map(number),
    // Untie requirement per twin.
    untieRequired: rows(lists.NinjaInfo[8], 12).map(number),
    // Detection base per floor, before the mastery scale.
    detectionBase: rows(lists.NinjaInfo[9], 12).map(number),
  },
  // The game's per-level mastery scales: NinjaInfo rows the formulas rescale.
  scales: {
    detection: number(lists.NinjaInfo[9]?.[11]),
    doorHp: number(lists.NinjaInfo[3]?.[10]),
    jade: number(lists.NinjaInfo[10]?.[11]),
  },
  charmBonusTypes: Object.fromEntries(
    Object.entries(math.CHARM_BONUS_TYPES).map(([index, label]) => [index, label]),
  ),
  // The emporium letters: `emporiumBonus(t, Ninja[102][9])` tests the string.
  emporiumLetters: String(hardcoded.N2L),
  legendTalentPerPoint: lists.LegendTalents.map((row) => number(row?.[2])),
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/stats/data/game/customlists.js',
      'js/stats/data/game/custommaps.js',
      'js/stats/data/game/hardcoded.js',
      'js/stats/systems/w6/sneaking-math.js',
    ],
  },
};

const out = new URL('../crates/core/data/sneaking.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${Object.keys(items).length} items, ${upgrades.length} ninja upgrades, ${payload.legendTalentPerPoint.length} legend talents`,
);
