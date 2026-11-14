// Regenerates fixtures/decision-tree.json from the upstream decision-tree
// stepper, so the Rust port is compared against the source of truth.
// Usage: node script/decision-tree-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/decision-tree-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const math = await import(join(source, 'js/stats/systems/w7/research-math.js'));
const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const { buildSaveContext, makeSimCtx } = await import(join(source, 'js/save/context.js'));
const dt = await import(join(source, 'js/dt/dt-sim.js'));
const dtState = await import(join(source, 'js/dt/dt-state.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save, serverVars: {} });
const saveCtx = buildSaveContext();

// The tree's root state, exactly as `dtBuildInitState` builds it.
const initState = dt.dtBuildInitState();

// The plain chain: step to the next research or insight event, then step from
// the state that event left, as the tree's "Save & Next" does.
const chain = [];
let state = initState;
let total = 0;
for (let step = 0; step < 12; step++) {
  const result = dt.dtStepSim(state);
  if (!result) break;
  total += result.time;
  chain.push({
    event: result.event,
    stepHours: result.time,
    totalHours: total,
    level: result.state.rLv,
    expHr: result.state.expHr,
    insightObs: result.insightObs ?? [],
    insightLvs: result.insightLvs ?? {},
  });
  state = result.state;
}

// A small synthetic state: the fixture save's context with a low grid and a
// pair of monocles, so the insight events arrive within hours instead of days.
// The Rust test rebuilds it from `synthetic.state` to step it with the port.
const OCC_COUNT = math.OCC_DATA?.length ?? saveCtx.occFound.length;
const synthGL = new Array(saveCtx.gridLevels.length).fill(0);
for (const [cell, level] of Object.entries({ 31: 1, 72: 1, 90: 1, 91: 1, 92: 1, 93: 1, 94: 1 })) {
  synthGL[Number(cell)] = level;
}
const synthSO = new Array(saveCtx.gridLevels.length).fill(-1);
const synthIL = new Array(OCC_COUNT).fill(0);
synthIL[2] = 3;
synthIL[5] = 1;
const synthIP = new Array(OCC_COUNT).fill(0);
const synthOCC = new Array(OCC_COUNT).fill(0);
for (let obs = 0; obs < 8; obs++) synthOCC[obs] = 1;
const synthMags = [
  { type: 2, slot: 3, x: 0, y: 0 },
  { type: 1, slot: 2, x: 0, y: 0 },
  { type: 1, slot: 5, x: 0, y: 0 },
  { type: 0, slot: 0, x: 0, y: 0 },
];
const synthCtx = makeSimCtx(synthGL);
const synthRExp = 0.995 * math.researchExpReq(60, saveCtx.serverVarResXP);
const syntheticState = {
  gl: synthGL,
  so: synthSO,
  md: synthMags,
  il: synthIL,
  ip: synthIP,
  occ: synthOCC,
  sp: [],
  rLv: 60,
  rExp: synthRExp,
  expHr: 0,
  saveCtx,
};
syntheticState.expHr = math.simTotalExpWith(
  synthGL, synthSO, synthMags, synthIL, synthOCC, 60, synthCtx
);

const synthChain = [];
let synthState = syntheticState;
let synthTotal = 0;
for (let step = 0; step < 8; step++) {
  const result = dt.dtStepSim(synthState);
  if (!result) break;
  synthTotal += result.time;
  synthChain.push({
    event: result.event,
    stepHours: result.time,
    totalHours: synthTotal,
    level: result.state.rLv,
    expHr: result.state.expHr,
    insightObs: result.insightObs ?? [],
    insightLvs: result.insightLvs ?? {},
  });
  synthState = result.state;
}

// The synthetic auto-insight chain: step through the insight level-ups,
// re-fitting the monocles at each, until the next research level-up.
dtState.dtNodes.length = 0;
const synthRoot = dt.dtCreateNode(null, 'start', 0, syntheticState);
const synthTail = dt.dtAdvanceAutoInsight(synthRoot);
const synthAutoNodes = synthTail
  ? dtState.dtNodes.map((node) => ({
      parent: node.parentId,
      event: node.event,
      stepHours: node.time,
      level: node.rLv,
      expHr: node.expHr,
      auto: !!node._autoInsightChild,
      timeline: (node.insightTimeline ?? []).map((entry) => ({
        time: entry.time,
        obs: entry.obs,
        prevIL: entry.prevIL,
        newIL: entry.newIL,
        monoTargets: entry.monoTargets,
        expHr: entry.expHr,
      })),
    }))
  : [];

// The auto-insight chain from the start node, which re-fits the monocles at
// every insight level-up it skips through.
dtState.dtNodes.length = 0;
const rootNode = dt.dtCreateNode(null, 'start', 0, dt.dtBuildInitState());
const autoTail = dt.dtAdvanceAutoInsight(rootNode);
const autoNodes = autoTail
  ? dtState.dtNodes.map((node) => ({
      parent: node.parentId,
      event: node.event,
      stepHours: node.time,
      level: node.rLv,
      expHr: node.expHr,
      auto: !!node._autoInsightChild,
      timeline: (node.insightTimeline ?? []).map((entry) => ({
        time: entry.time,
        obs: entry.obs,
        prevIL: entry.prevIL,
        newIL: entry.newIL,
        monoTargets: entry.monoTargets,
        expHr: entry.expHr,
      })),
    }))
  : [];

writeFileSync(
  join(root, 'fixtures/decision-tree.json'),
  `${JSON.stringify(
    {
      source: 'js/dt/dt-sim.js (dtBuildInitState, dtStepSim, dtAdvanceAutoInsight)',
      save: 'fixtures/data.json',
      start: {
        level: initState.rLv,
        exp: initState.rExp,
        expHr: initState.expHr,
        magnifiers: initState.md.length,
        shapes: initState.sp.length,
      },
      chain,
      synthetic: {
        state: {
          researchLevel: 60,
          researchExp: synthRExp,
          gridLevels: synthGL,
          shapeOverlay: synthSO,
          insightLevels: synthIL,
          insightProgress: synthIP,
          occFound: synthOCC,
          mags: synthMags.map(({ type, slot }) => ({ type, slot })),
          expHr: syntheticState.expHr,
        },
        chain: synthChain,
        autoInsight: {
          tailLevel: synthTail ? synthTail.rLv : null,
          nodes: synthAutoNodes,
        },
      },
      autoInsight: {
        tailLevel: autoTail ? autoTail.rLv : null,
        nodes: autoNodes,
      },
    },
    null,
    2
  )}\n`
);
console.log(
  `wrote fixtures/decision-tree.json: ${chain.length} steps, ` +
    `${autoNodes.length} auto-insight nodes`
);
