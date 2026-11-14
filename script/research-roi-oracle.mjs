// Regenerates fixtures/research-roi.json from the upstream insight maths, so
// the Rust port is compared against the source of truth rather than hand-picked
// numbers. The state below is synthetic: a level-80 grid with a handful of
// unified nodes, nine found observations and a mixed magnifier pool.
// Usage: node script/research-roi-oracle.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/research-roi-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const math = await import(join(source, 'js/stats/systems/w7/research-math.js'));
const game = await import(join(source, 'js/stats/data/w7/research.js'));
const monos = await import(join(source, 'js/optimizers/monos.js'));
const context = await import(join(source, 'js/save/context.js'));

const GRID_SIZE = game.GRID_SIZE;
const OCC_COUNT = game.OCC_DATA.length;

/// The state both sides read. `context` is a `SimContext` document: the Rust
/// test deserializes it as is, and the upstream context below is built from the
/// same fields (plus the `abm` the upstream derives, which is written back here).
const state = {
  researchLevel: 80,
  gridLevels: { 31: 2, 50: 1, 52: 2, 70: 2, 72: 2, 90: 1, 91: 2, 92: 1, 93: 2, 94: 1, 110: 1, 112: 1, 125: 2 },
  shapeOverlay: { 70: 0, 90: 2, 110: 5 },
  occFound: 9,
  insightLevels: [3, 1, 0, 4, 2, 5, 1, 0, 0],
  insightProgress: [500, 120, 0, 900, 50, 2000, 10, 0, 0],
  mags: [
    { type: 0, slot: 0 },
    { type: 0, slot: 1 },
    { type: 0, slot: 3 },
    { type: 0, slot: 3 },
    { type: 2, slot: 3 },
    { type: 1, slot: 0 },
    { type: 1, slot: 5 },
  ],
  targets: [3, 6],
};

const gl = new Array(GRID_SIZE).fill(0);
for (const [cell, level] of Object.entries(state.gridLevels)) gl[Number(cell)] = level;
const so = new Array(GRID_SIZE).fill(-1);
for (const [cell, shape] of Object.entries(state.shapeOverlay)) so[Number(cell)] = shape;
const il = new Array(OCC_COUNT).fill(0);
state.insightLevels.forEach((level, obs) => (il[obs] = level));
const ip = new Array(OCC_COUNT).fill(0);
state.insightProgress.forEach((progress, obs) => (ip[obs] = progress));
const occ = new Array(OCC_COUNT).fill(0);
for (let obs = 0; obs < state.occFound; obs++) occ[obs] = 1;
const mags = state.mags.map((mag) => ({ ...mag, x: 0, y: 0 }));

// The upstream context reads a save context; every field here maps one to one
// onto the Rust `SimContext` field of the same name.
const contextDoc = {
  researchLevel: state.researchLevel,
  serverVarResXp: 1.02,
  allBonusMulti: 1,
  extraPct: 12,
  companion52: 1.15,
  killroy5: 1.2,
  royalResearch: 1.1,
  arcade63: 5,
  clamWork9: 1,
  dream14: 2,
  cglunko11: 3,
  fountain216: 4,
  menagerieB3: 3,
  researchEquipmentMulti: 1.3,
  jellyResearchMulti: 1.05,
  jellyInsightPct: 10,
  jellyKaleiPct: 8,
  masteryAdditivePct: 0,
  masteryExpMulti: 1,
  rog: [4, 0, 0, 0, 0, 0, 0, 0, 1],
  emporium: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
};

const saveCtx = {
  companionHas55: false,
  cachedComp0DivOk: false,
  cbGridAll: 0,
  rog: contextDoc.rog,
  companion55Value: 0,
  comp52TrueMulti: contextDoc.companion52,
  cachedStickerFixed: 0,
  cachedBoonyCount: 0,
  cachedEvShop37: 0,
  cachedExtPctExSticker: contextDoc.extraPct,
  companionHas54: false,
  companionHas153: false,
  evShop: [],
  sb: [],
  emp: contextDoc.emporium,
  mhq: [],
  cachedSpelunkyUpg7: 0,
  serverVarResXP: contextDoc.serverVarResXp,
  button0: 0,
  btnBaseNoGrid: 0,
  killroy5: contextDoc.killroy5,
  dream14: contextDoc.dream14,
  cglunko11: contextDoc.cglunko11,
  fountain2_16: contextDoc.fountain216,
  companion54Level2: 0,
  royalResearch: contextDoc.royalResearch,
  stickerMultiplicative: false,
  arcade63: contextDoc.arcade63,
  clamWork9: contextDoc.clamWork9,
  menagerieB3: contextDoc.menagerieB3,
  researchEquipmentMulti: contextDoc.researchEquipmentMulti,
  bundleBanM: false,
  jellyResearchMulti: contextDoc.jellyResearchMulti,
  jellyInsightPct: contextDoc.jellyInsightPct,
  jellyKaleiPct: contextDoc.jellyKaleiPct,
  researchMastery: { unlocked: false, level: 0, levels: [] },
};
const ctx = context.makeSimCtx(gl, saveCtx);
// The all-bonus multiplier is derived upstream; the fixture carries its value so
// the Rust context reads exactly what the oracle did.
contextDoc.allBonusMulti = ctx.abm;

// `insightExpRate` ignores the insight level, so one value per usable observation.
const insightRates = math
  .getAvailableSlots(state.researchLevel, occ)
  .map((obs) => [obs, math.insightExpRate(obs, mags, il, gl, so, ctx)]);

const insightReq = [];
for (let obs = 0; obs < OCC_COUNT; obs++) {
  for (let level = 0; level <= 5; level++) {
    insightReq.push([obs, level, math.insightExpReqAt(obs, level)]);
  }
}

const levelled = il.slice();
levelled[6] = (levelled[6] || 0) + 1;
const expRates = {
  base: math.simTotalExpWith(gl, so, mags, il, occ, state.researchLevel, ctx),
  afterInsight: math.simTotalExpWith(gl, so, mags, levelled, occ, state.researchLevel, ctx),
  bare: math.simTotalExpWith(gl, so, mags, il, occ, state.researchLevel, {
    ...ctx,
    abm: 1,
  }),
};

const concentrated = [];
for (const target of state.targets) {
  const layout = monos.buildConcentratedLayout(
    { md: mags, mMax: math.magMaxForLevel(state.researchLevel), gl, so, il, occ, rLv: state.researchLevel },
    target,
    ctx
  );
  concentrated.push({
    target,
    layout: layout.map((mag) => ({ type: mag.type, slot: mag.slot })),
    insightRate: math.insightExpRate(target, layout, il, gl, so, ctx),
    expRate: math.simTotalExpWith(gl, so, layout, il, occ, state.researchLevel, ctx),
  });
}

writeFileSync(
  join(root, 'fixtures/research-roi.json'),
  `${JSON.stringify(
    {
      source: 'js/optimizers/monos.js, js/stats/systems/w7/research-math.js',
      state: { ...state, gridLevels: gl, shapeOverlay: so },
      context: contextDoc,
      magCap: math.magMaxForLevel(state.researchLevel),
      availableSlots: math.getAvailableSlots(state.researchLevel, occ),
      insightRates,
      insightReq,
      expRates,
      concentrated,
    },
    null,
    2
  )}\n`
);
console.log(
  `wrote fixtures/research-roi.json: ${insightRates.length} insight rates, ${concentrated.length} concentrated layouts`
);
