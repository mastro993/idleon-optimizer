// Writes the upstream mechanics-override catalog and its resolved overrides, so
// the Rust port reads the same data and is tested against the same outcomes.
// Usage: node script/combat-overrides-oracle.mjs <path-to-idleon-research-optimizer>
//
// Sources: `js/stats/systems/common/combat-sim-toggles.js` (the catalog, its
// normalization, `buildToggleOverrides` and `describeToggleState`) and
// `js/stats/data/game/customlists.js` (`RANDOlist[112]`, the medallion types the
// `compass3` override forces on).
//
// Two files come out of one checkout, so they can never disagree:
//   - `crates/core/data/combat-overrides.json`: the catalog the app reads;
//   - `fixtures/combat-overrides.json`: the saved-input map and the resolved
//     states `crates/core/tests/combat_overrides.rs` compares the engine against.
// The entries map is synthetic: one number per input so the off, set and scale
// branches have a saved value to act on, and an object for `objscale`.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/combat-overrides-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const toggles = await import(
  join(source, 'js/stats/systems/common/combat-sim-toggles.js')
);
const lists = await import(join(source, 'js/stats/data/game/customlists.js'));

const medallions = lists.RANDOlist[112];

// Flags with their own keys write more than their first input; keep those maps
// in the catalog so the Rust engine does not hard-code them.
const fixed = (map) =>
  Object.entries(map).map(([key, value]) =>
    Array.isArray(value)
      ? { key, kind: 'list', items: value }
      : { key, kind: 'number', number: value }
  );

const catalog = toggles.COMBAT_TOGGLES.map((toggle) => ({
  id: toggle.id,
  group: toggle.group,
  kind: toggle.kind,
  label: toggle.label,
  desc: toggle.desc,
  keys: toggle.keys,
  neutral: toggle.neutral,
  on: toggle.on ? fixed(toggle.on) : [],
  off: toggle.off ? fixed(toggle.off) : [],
}));

// The saved values every case resolves against. Catalog inputs get a stable
// number (or an object for `objscale`), plus keys the catalog does not own so the
// advanced `raw:` ids are exercised.
const saved = {};
for (const [index, toggle] of toggles.COMBAT_TOGGLES.entries()) {
  const key = toggle.keys[0];
  saved[key] =
    toggle.kind === 'objscale' ? { mushG: 10, frogG: 2.5 } : (index % 5) + 1;
}
saved.filterSlots = 12;
saved.someOtherInput = 7;

const entries = Object.fromEntries(
  Object.entries(saved).map(([key, value]) => [key, { value, source: 'oracle' }])
);

// The formatter the page hands `describeToggleState` (`fmtOvr`): integers read
// as integers, other values keep four significant digits. The raw-input namer is
// upstream `rawInputName`'s generic fallback: camel-case keys split into words.
const fmt = (value) =>
  Number.isInteger(value) ? String(value) : String(+value.toPrecision(4));
const rawName = (key) =>
  key
    .replace(/([a-z])([A-Z0-9])/g, '$1 $2')
    .replace(/^./, (c) => c.toUpperCase());

const cases = [
  {
    name: 'off-set-scale',
    state: {
      dropRate: { mode: 'off' },
      damageMax: { mode: 'set', value: 1234 },
      killPerKill: { mode: 'scale', value: 2 },
      multikillTotal: { mode: 'off' },
      'raw:filterSlots': { mode: 'set', value: 9 },
      'raw:someOtherInput': { mode: 'scale', value: 0.5 },
    },
  },
  {
    name: 'flags-and-tris',
    state: {
      wraith: { mode: 'on' },
      arcanist: { mode: 'off' },
      compass3: { mode: 'on' },
      autoLoot: { mode: 'off' },
      overkillActive: { mode: 'on' },
    },
  },
  { name: 'object-scale', state: { respawnBonusPct: { mode: 'scale', value: 2 } } },
  { name: 'object-off', state: { respawnBonusPct: { mode: 'off' } } },
  {
    name: 'raw-scale-without-a-saved-input',
    state: { 'raw:notAnInput': { mode: 'scale', value: 3 } },
  },
  {
    name: 'invalid-rows-are-dropped',
    state: {
      nope: { mode: 'off' },
      dropRate: { mode: 'on' },
      monsterCash: { mode: 'set' },
      'raw:': { mode: 'set', value: 1 },
      extraMushKillz: { mode: 'peak' },
    },
  },
];

const resolved = cases.map((entry) => ({
  name: entry.name,
  state: entry.state,
  normalized: toggles.normalizeToggleState(entry.state),
  active: toggles.activeToggleCount(entry.state),
  needsBase: toggles.toggleNeedsBase(entry.state),
  overrides: toggles.buildToggleOverrides(entry.state, entries),
  described: toggles.describeToggleState(entry.state, fmt, rawName),
}));

const root = new URL('..', import.meta.url);
writeFileSync(
  new URL('crates/core/data/combat-overrides.json', root),
  `${JSON.stringify({ groups: toggles.TOGGLE_GROUPS, toggles: catalog }, null, 2)}\n`
);
writeFileSync(
  new URL('fixtures/combat-overrides.json', root),
  `${JSON.stringify(
    {
      source: [
        'js/stats/systems/common/combat-sim-toggles.js',
        'js/stats/data/game/customlists.js RANDOlist[112]',
      ],
      groups: toggles.TOGGLE_GROUPS,
      entries: saved,
      cases: resolved,
    },
    null,
    2
  )}\n`
);
console.log(
  `wrote ${catalog.length} mechanics, ${cases.length} cases and ${medallions.length} medallions`
);
