// Regenerates fixtures/cog-insights.json from the upstream Cog Board
// Optimizer, so the Rust insights and roll ranges are tested against the source
// of truth rather than hand-picked numbers.
//
// The insight and roll-range functions live inline in `cog-optimizer.html`
// rather than in an importable module, so they are extracted by their
// balanced-brace bodies and run with the upstream helpers in scope. Nothing is
// reimplemented here.
// Usage: node script/cog-insights-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/cog-insights-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const {
  BOARD_SIZE,
  boardFromSave,
  recoverRawPlayerStats,
  computeBoardTotals,
  scoreBoard,
  classifyCog,
  cogAffectsPlayer,
  slotToPos,
} = await import(join(source, 'js/cogs/cog-board.js'));
const { findMaxConLevel } = await import(join(source, 'js/cogs/cog-judge.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save, serverVars: {} });
const order = state.saveData.cogOrderData;
const map = state.saveData.cogMapData;
if (!Array.isArray(order) || !map) throw new Error('the fixture has no cog rows');

const page = readFileSync(join(source, 'cog-optimizer.html'), 'utf8');

/** A top-level function's own text from the page, braces balanced. */
function source_of(name) {
  const start = page.indexOf(`function ${name}(`);
  if (start < 0) throw new Error(`${name} is not in cog-optimizer.html`);
  let index = page.indexOf('{', start);
  let depth = 0;
  for (; index < page.length; index++) {
    if (page[index] === '{') depth++;
    else if (page[index] === '}' && --depth === 0) break;
  }
  return page.slice(start, index + 1);
}

/** A page function with its own parameters and the given helpers in scope. */
function page_function(name, ...helpers) {
  const text = source_of(name);
  // `function name(args) {` → the parameter list the wrapper needs.
  const args = text
    .slice(text.indexOf('(') + 1, text.indexOf(')'))
    .split(',')
    .map((arg) => arg.trim())
    .filter(Boolean);
  const body = text.slice(text.indexOf('{') + 1, text.lastIndexOf('}'));
  return new Function(...args, ...helpers, body);
}

const fmtBig = page_function('fmtBig');
const computeBoardInsights = page_function(
  'computeBoardInsights',
  'BOARD_SIZE',
  'scoreBoard',
  'computeBoardTotals',
  'classifyCog',
  'slotToPos',
  'cogAffectsPlayer',
  'fmtBig',
);
const rollRangeStats = page_function('_rollRangeStats');
// `_findConForPlus1` closes over the level it was rendered at.
const nextLevelFor = (conLevel) =>
  new Function(
    '_rollRangeStats',
    'conLv',
    `${source_of('_findConForPlus1')}
     return _findConForPlus1;`,
  )(rollRangeStats, conLevel);

const board = boardFromSave(order, map);
recoverRawPlayerStats(board);
const conLevel = findMaxConLevel(save, order, map);

const round = (value) => (Number.isFinite(value) ? Number(value.toPrecision(15)) : value);

const goals = {};
for (const goal of ['build', 'conexp', 'flaggy']) {
  const insights = computeBoardInsights(
    board,
    goal,
    BOARD_SIZE,
    scoreBoard,
    computeBoardTotals,
    classifyCog,
    slotToPos,
    cogAffectsPlayer,
    fmtBig,
  );
  if (!insights) throw new Error(`${goal}: the fixture board yields no insights`);
  goals[goal] = {
    baseScore: round(insights.baseScore),
    impacts: insights.impacts.map((impact) => ({
      slot: impact.slot,
      delta: round(impact.delta),
      pct: round(impact.pct),
    })),
    weakest: { slot: insights.weakest.slot, delta: round(insights.weakest.delta) },
    best: { slot: insights.best.slot, delta: round(insights.best.delta) },
    rollTargets: insights.rollTargets.map((target) => ({
      slot: target.slot,
      col: target.pos.col,
      row: target.pos.row,
      why: target.why,
      roll: target.roll,
      pct: round(target.pct),
      delta: round(target.delta),
    })),
  };
}

const goalBase = { build: 'perRollA', flaggy: 'perRollC', conexp: 'perRollD' };
const goalMin = { build: 'perRollMinA', flaggy: 'perRollMinC', conexp: 'perRollMinD' };
const rollRanges = {};
for (const goal of ['build', 'conexp', 'flaggy']) {
  const next = nextLevelFor(conLevel);
  rollRanges[goal] = [3, 4].map((tier) => {
    const stats = rollRangeStats(tier, conLevel);
    const maxStat = goal === 'build' ? stats.maxA : goal === 'flaggy' ? stats.maxC : stats.maxD;
    return {
      tier,
      rolls: stats.maxRolls,
      rawMagnitude: round(stats.rawMag),
      perRollMin: round(stats[goalMin[goal]]),
      perRollMax: round(stats[goalBase[goal]]),
      totalMax: round(maxStat),
      minSurround: stats.minSurr,
      maxSurround: stats.maxSurr,
      nextMinLevel: next(tier, goalMin[goal]).needed,
      nextMinValue: next(tier, goalMin[goal]).to,
      nextMaxLevel: next(tier, goalBase[goal]).needed,
      nextMaxValue: next(tier, goalBase[goal]).to,
    };
  });
}

const fixture = {
  note: 'Generated by script/cog-insights-oracle.mjs from the upstream Cog Board Optimizer.',
  constructionLevel: conLevel,
  goals,
  rollRanges,
};
writeFileSync(join(root, 'fixtures/cog-insights.json'), `${JSON.stringify(fixture, null, 2)}\n`);
console.log(
  `wrote fixtures/cog-insights.json · ${goals.build.impacts.length} impacts · level ${conLevel}`,
);
