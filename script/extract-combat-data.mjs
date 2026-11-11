// Extracts the equipment catalog and the class/talent names the combat
// loadout editor reads.
// Usage: node script/extract-combat-data.mjs <path-to-idleon-research-optimizer>
//
// Source: `js/stats/data/game/items.js` (ITEMS) and
// `js/stats/data/game/customlists.js` (ClassNames, TalentIconNames).
// Only the items an equipment slot can hold are kept — armor types, weapons
// (`typeGen: 'aWeapon'`) and tools — with the eight gear stats the setup
// editor shows plus the upgrade-slot count. Everything else (food, cards,
// obols, stones) stays out until a later combat issue needs it.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-combat-data.mjs <upstream-repo>');
  process.exit(1);
}

// The upstream tree is plain ES modules without a package.json; node needs the
// marker to import its `.js` files.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
const existing = (() => {
  try {
    return readFileSync(marker, 'utf8');
  } catch {
    return null;
  }
})();
if (!existing) writeFileSync(marker, JSON.stringify({ type: 'module' }));

const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const items = await import(join(source, 'js/stats/data/game/items.js'));

// `combat-sim-setup.js` ARMOR_TYPES and TOOL_TYPEGEN, flattened.
const ARMOR_TYPES = [
  ['HELMET', 'PREMIUM_HELMET'], [], ['SHIRT'], ['PENDANT'], ['PANTS'], ['RING'], ['SHOES'], ['RING'],
  ['PREMIUM_HELMET', 'HELMET'], ['KEYCHAIN'], ['TROPHY', 'REPLICA_TROPHY'], ['KEYCHAIN'], ['CAPE'],
  ['CHAT_RING'], ['NAMETAG', 'REPLICA_NAMETAG'], ['ATTIRE'],
];
const TOOL_TYPEGEN = ['aPick', 'aHatchet', 'aFishingRod', 'aBugNet', 'aTrap', 'aSkull', 'aDNAgun'];
const GEAR_STATS = ['Weapon_Power', 'STR', 'AGI', 'WIS', 'LUK', 'Defence', 'Reach', 'Speed'];
const ARMOR = new Set(ARMOR_TYPES.flat());
const TOOLS = new Set(TOOL_TYPEGEN);

const catalog = {};
for (const [key, item] of Object.entries(items.ITEMS)) {
  const type = String(item.Type || '');
  const typeGen = String(item.typeGen || '');
  const wearable = ARMOR.has(type) || TOOLS.has(typeGen) || typeGen === 'aWeapon';
  if (!wearable || type === 'ERROR') continue;
  const stats = {};
  for (const stat of GEAR_STATS) {
    const value = Number(item[stat]) || 0;
    if (value !== 0) stats[stat] = value;
  }
  const uq = {};
  for (const n of [1, 2]) {
    const text = String(item[`UQ${n}txt`] || '0');
    const value = Number(item[`UQ${n}val`]) || 0;
    if (text !== '0' || value !== 0) uq[`uq${n}`] = [text, value];
  }
  const entry = { name: String(item.displayName || key), type, typeGen };
  if (String(item.Class || 'ALL') !== 'ALL') entry.class = String(item.Class);
  if (Number(item.lvReqToEquip) > 0) entry.level = Number(item.lvReqToEquip);
  if (Number(item.Upgrade_Slots_Left) > 0) entry.slots = Number(item.Upgrade_Slots_Left);
  if (Object.keys(stats).length) entry.stats = stats;
  catalog[key] = Object.assign(entry, uq);
}

const payload = {
  classes: lists.ClassNames,
  talents: lists.TalentIconNames,
  items: catalog,
};

const out = new URL('../crates/core/data/combat.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload));
console.log(
  `wrote ${Object.keys(catalog).length} wearable items, ` +
    `${payload.classes.length} classes, ${payload.talents.length} talents`
);
