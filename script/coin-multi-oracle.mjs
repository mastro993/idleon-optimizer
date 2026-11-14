// Regenerates fixtures/coin-multi.json from the upstream Coin Multiplier page,
// so the Rust port is tested against the source of truth rather than hand-picked
// numbers.
// Usage: node script/coin-multi-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/coin-multi-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const saveDataModule = await import(join(source, 'js/save/data.js'));
const { createStatContext } = await import(join(source, 'js/stats/stat-context.js'));
const { computeOverkillTier } = await import(join(source, 'js/stats/systems/common/overkill.js'));
const { MONSTERS } = await import(join(source, 'js/stats/data/game/monsters.js'));

/** Parse a save field that may be a JSON string. */
function parseKey(save, key) {
  const raw = save[key];
  if (typeof raw !== 'string') return raw;
  try {
    return JSON.parse(raw);
  } catch {
    return raw;
  }
}

/** The highest damage the save records: `TaskZZ0[1][0]`. */
function highestDamage(save) {
  const tasks = parseKey(save, 'TaskZZ0');
  return Number(tasks?.[1]?.[0]) || 0;
}

/** One character's upstream result, in the shape the Rust test reads. */
function evaluate(label, rawSave, charIdx) {
  loadSaveData({ data: rawSave, serverVars: {} });
  const saveData = state.saveData;
  const characters = saveData.lv0AllData?.length || 0;
  if (charIdx >= characters) throw new Error(`${label}: no character ${charIdx}`);
  const mapIdx = Number(saveDataModule.currentMapData[charIdx]) || 0;
  const target = String(saveDataModule.afkTargetData[charIdx] || '');
  const monster = target && MONSTERS[target];
  const monsterHP = monster ? Number(monster.MonsterHPTotal) || 0 : 0;
  const ctx = createStatContext({ charIdx, mapIdx, saveData });
  const damage = Number(ctx.resolve('damage').val) || 0;
  let overkill = computeOverkillTier(charIdx, ctx, {
    mapIdx,
    afkTarget: target || '__missing_saved_target__',
    monsterHP,
  });
  overkill = Object.assign({}, overkill, { afkTarget: target, maxDmg: damage });
  overkill.tier = Math.max(1, Math.min(51, Number(overkill.tier) || 1));
  ctx.overkillTier = overkill.tier;
  const tree = ctx.resolve('coin-multi');
  return {
    label,
    character: charIdx,
    map: mapIdx,
    target,
    monsterHp: monsterHP,
    highestDamage: highestDamage(rawSave),
    damage,
    overkill: {
      tier: overkill.tier,
      monsterHp: overkill.monsterHP,
      exponent: overkill.exponent,
      partial: Boolean(overkill.partial),
    },
    total: Number(tree.val) || 0,
    partial: Boolean(tree.partial),
    reason: String(tree.reason || ''),
    // The 23 multiplicative groups, in the order `coin-multi.js` multiplies them,
    // with the additive terms each group's own breakdown sums.
    groups: (tree.children || []).map((child) => ({
      name: String(child.name),
      value: Number(child.val) || 0,
      terms: (child.children || []).map((term) => ({
        name: String(term.name),
        value: typeof term.val === 'object' ? Number(term.val?.val) || 0 : Number(term.val) || 0,
      })),
    })),
  };
}

// A small save that activates only the groups this build ports, so the total
// itself is compared, not just the individual groups.
const SIMPLE_SAVE = {
  charNames: ['Ana'],
  Research: JSON.stringify(Array.from({ length: 18 }, () => [])),
  Holes: JSON.stringify(Array.from({ length: 34 }, () => [[], [], []])),
  Lv0_0: JSON.stringify([1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
  CharacterClass_0: 1,
  CurrentMap_0: 2,
  AFKtarget_0: 'frogG',
  // Companions 24, 45, 159 and 38 are owned.
  OptLacc: JSON.stringify(
    (() => {
      const ola = new Array(701).fill(0);
      ola[606] = '24,45,159,38';
      // Event Shop 9 and 20 owned: their encoding letters are at those indices.
      ola[311] = 'a,b,c,d,e,f,g,h,i,j';
      return ola;
    })(),
  ),
  // The coin descriptor reads `bundlesData.bun_y` directly (hasBundle's nested
  // `h` row is a different shape).
  BundlesReceived: JSON.stringify({ bun_y: 1 }),
  UpgVault: JSON.stringify(
    (() => {
      const vault = new Array(90).fill(0);
      vault[34] = 10;
      vault[37] = 10;
      return vault;
    })(),
  ),
  // One completed task and a few capped bubble levels for the vault kill tiers.
  TaskZZ3: JSON.stringify([[[1, 1]], [], [], []]),
  CauldronInfo: JSON.stringify([[5], [50], [120], [0]]),
};

const fixture = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
const full = { source: 'fixtures/data.json', characters: [] };
const count = (() => {
  loadSaveData({ data: fixture, serverVars: {} });
  return state.saveData.lv0AllData?.length || 0;
})();
for (let charIdx = 0; charIdx < count; charIdx += 1) {
  full.characters.push(evaluate('fixtures/data.json', fixture, charIdx));
}

const simple = {
  source: 'script/coin-multi-oracle.mjs · SIMPLE_SAVE',
  characters: [evaluate('simple', SIMPLE_SAVE, 0)],
};

writeFileSync(
  new URL('../fixtures/coin-multi.json', import.meta.url),
  `${JSON.stringify({ full, simple }, null, 2)}\n`,
);
console.log(
  `wrote ${full.characters.length} full-save characters and ${simple.characters.length} simple-save character`,
);
