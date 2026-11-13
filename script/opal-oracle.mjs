// Regenerates fixtures/opal.json from the upstream Opal Optimizer page, so the
// Rust port is tested against the page's own expressions.
//
// Usage: node script/opal-oracle.mjs <path-to-idleon-research-optimizer>
//
// `opal-calc.html` keeps every villager formula inline. This script reads the
// page's module script, drops its imports, and evaluates the remaining
// functions in a scope holding the upstream modules the imports named — so the
// oracle runs the page's own `buildExt`, `villagerExpRate` and `totalExpRate`
// instead of a retyped copy. Change the page and this fixture has to move with
// it.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/opal-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));

const html = readFileSync(join(source, 'opal-calc.html'), 'utf8');
const script = html.match(/<script type="module">\n([\s\S]*?)<\/script>/)[1];
const imports = [...script.matchAll(/^import \{([^}]+)\} from '([^']+)';$/gm)];
const names = [];
const values = [];
for (const [, list, path] of imports) {
  const module = await import(join(source, path));
  for (const entry of list.split(',')) {
    const name = entry.trim();
    names.push(name);
    values.push(module[name]);
  }
}

const body = script.replace(/^import .*$/gm, '').replace(/\ninit\(\);\s*$/, '\n');
const exported = [
  'buildExt',
  'villagerExpRate',
  'totalExpRate',
  'optimizeTotal',
  'optimizeForTarget',
  'bellBonus',
  'measBonus',
  'holeozDxp',
  'getVillagerLevel',
  'getVillagerOpals',
  'getVillagerStars',
  'getCosmoLevel',
  'VILLAGER_NAMES',
];
const factory = new Function(...names, `${body}\n; return { ${exported.join(', ')} };`);

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save });
values[names.indexOf('saveData')] = state.saveData;
const page = factory(...values);

const ext = page.buildExt();
const opals = Array.from({ length: 5 }, (_, v) => page.getVillagerOpals(v));
const levels = Array.from({ length: 5 }, (_, v) => page.getVillagerLevel(v));
const stars = Array.from({ length: 5 }, (_, v) => page.getVillagerStars(v));
const cosmo = {
  enthusiasm: Array.from({ length: allLength(1) }, (_, i) => page.getCosmoLevel(1, i)),
};
function allLength(tier) {
  const row = state.saveData.holesData?.[4 + tier];
  return Array.isArray(row) ? row.length : 0;
}

const totalOpals = opals.reduce((sum, count) => sum + count, 0);
// The page's own optimizer, so the port is held to at least the plans the
// upstream shapes find. Its greedy splits miss some plans, so the Rust port
// searching further is compared with an "at least" bound, not field for field.
const totalPlan = page.optimizeTotal(totalOpals, ext);
const optimizer = {
  total: { opals: totalPlan, rate: page.totalExpRate(totalPlan, ext) },
  targets: Array.from({ length: 5 }, (_, v) => {
    const plan = page.optimizeForTarget(v, totalOpals, ext);
    return { villager: v, opals: plan, rate: page.villagerExpRate(v, plan, ext) };
  }),
};

const b = ext._breakdown;
// `recalcSim`: one hand-typed distribution read against the save, with the
// page's own `vs Current` column (no share when a villager earned nothing).
const simulate = (counts) => {
  const typed = Array.from({ length: 5 }, (_, v) => Math.max(0, Number(counts[v]) || 0));
  const lowest = minOpals(typed, ext);
  const rates = Array.from({ length: 5 }, (_, v) => page.villagerExpRate(v, typed, ext));
  const currentRates = Array.from({ length: 5 }, (_, v) => page.villagerExpRate(v, opals, ext));
  const total = page.totalExpRate(typed, ext);
  const currentTotal = page.totalExpRate(opals, ext);
  return {
    opals: typed,
    totalOpals: typed.reduce((sum, count) => sum + count, 0),
    minOpals: lowest,
    equalSpread: ext.cosmoBonus_1_5 * Math.floor(lowest / 5),
    rates,
    total,
    changes: rates.map((rate, v) => (currentRates[v] > 0 ? ((rate - currentRates[v]) / currentRates[v]) * 100 : 0)),
    totalChange: currentTotal > 0 ? ((total - currentTotal) / currentTotal) * 100 : 0,
  };
};

const simulator = {
  distributions: [
    { label: 'the saved split', ...simulate(opals) },
    { label: 'a balanced swap', ...simulate([120, 24, 100, 35, 90]) },
    { label: "the page's own total plan", ...simulate(totalPlan) },
    { label: 'a total above the save', ...simulate([100, 24, 100, 35, 100]) },
    { label: 'nothing on the Explorer', ...simulate([0, 24, 100, 35, 210]) },
  ],
};
// `arcadeBonus` returns a `{val, children}` node for some indices; the page
// multiplies the value, so the fixture stores the number.
const num = (value) => (value && typeof value === 'object' ? Number(value.val) || 0 : Number(value) || 0);

const payload = {
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: ['opal-calc.html (inline module script)', 'JavaScript from the imported upstream systems'],
    save: 'fixtures/data.json loaded through js/save/loader.js',
  },
  villagers: Array.from({ length: 5 }, (_, v) => ({
    name: page.VILLAGER_NAMES[v],
    level: levels[v],
    opals: opals[v],
    stars: stars[v],
  })),
  cosmo: {
    tier1: cosmo.enthusiasm,
    scholars: state.saveData.holesData?.[13]?.filter((cell) => Number(cell) === 1).length || 0,
  },
  ext: {
    unlocked: ext.unlocked,
    numUnlocked: ext.numUnlocked,
    starMul: ext.starMul,
    cosmoBonus_1_0: ext.cosmoBonus_1_0,
    cosmoBonus_1_5: ext.cosmoBonus_1_5,
    additiveBase: ext.additiveBase,
    baseMulti: ext.baseMulti,
    holeozArr: ext.holeozArr,
  },
  terms: {
    basePlusUpg: b.basePlusUpg,
    multiChain: b.multiChain,
    exotic51: num(b.exotic51),
    arcade38: num(b.arcade38),
    grimoire29: b.grimoire29,
    arcane32: b.arcane32,
    mon03: b.mon03,
    mon13: b.mon13,
    mon23: b.mon23,
    meas7: b.meas7,
    meas0: b.meas0,
    bUpg48: b.bUpg48,
    cardCaveB: b.cardCaveB,
    bell1_0: b.bell1_0,
    summWin21: num(b.summWin21),
    vaultUpg73: b.vaultUpg73,
    cosmoBonus_1_1: b.cosmoBonus_1_1,
    cosSchBlt: b.cosSchBlt,
    cosmoBonus_1_2: b.cosmoBonus_1_2,
    compass59: b.compass59,
    legend12: b.legend12,
    pristine21: b.pristine21,
    comp13: num(b.comp13),
    statue28: b.statue28,
    jarSum: b.jarSum,
    evShop6: b.evShop6,
    loreEpi1: b.loreEpi1,
    bundleU: b.bundleU,
  },
  optimizer,
  simulator,
  current: {
    minOpals: minOpals(opals, ext),
    equalSpread: ext.cosmoBonus_1_5 * Math.floor(minOpals(opals, ext) / 5),
    totalOpals,
    rates: Array.from({ length: 5 }, (_, v) => page.villagerExpRate(v, opals, ext)),
    total: page.totalExpRate(opals, ext),
  },
};

function minOpals(counts, ext) {
  let min = Infinity;
  counts.forEach((count, index) => {
    if (ext.unlocked[index] && count < min) min = count;
  });
  return Number.isFinite(min) ? min : 0;
}

writeFileSync(new URL('../fixtures/opal.json', import.meta.url), `${JSON.stringify(payload, null, 1)}\n`);
console.log(
  `wrote ${payload.villagers.length} villagers, ${Object.keys(payload.terms).length} terms, ` +
    `total ${payload.current.total}`,
);
