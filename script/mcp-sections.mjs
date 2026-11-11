// Per-section verification: opens every registry section, asserts its stable
// root and shared header, drives one interaction and captures a screenshot.
//
// Sections come from `idleon-optimizer --list-sections`, so this script never
// hardcodes the registry. Dashboard cards open from the grid; the hidden tools
// open through `--section <id>` in their own lane.
//
// Usage: node script/mcp-sections.mjs [options]
//   --only A,B       check just these section ids
//   --parallel N     run N isolated lanes at once (each its own private window)
//   --shard K/N      run one lane's slice (used by --parallel)
//   --shots DIR      where to write screenshots and sidecars (default target/mcp-sections)
//   --baseline DIR   compare captures against DIR/<id>.png
//   --update-baseline  seed or refresh the baseline instead of failing on a mismatch
//   --tolerance N    per-channel delta treated as a match (default 2)
//   --no-screenshots  skip captures (parallel lanes do this: only the frontmost
//                     window can be captured reliably)
//   --keep           leave spawned windows running
//   --app-bin PATH   app binary to launch
import { execFileSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import { DEFAULT_APP_BIN, McpLane, flag, hasFlag } from './mcp-lane.mjs';
import { comparePng, decodePng } from './png.mjs';

const argv = process.argv.slice(2);
const appBin = flag('--app-bin', DEFAULT_APP_BIN);
const keep = hasFlag('--keep');
const shotsDir = flag('--shots', join(process.cwd(), 'target/mcp-sections'));
const baselineDir = flag('--baseline', null);
const updateBaseline = hasFlag('--update-baseline');
const noScreenshots = hasFlag('--no-screenshots');
const tolerance = Number(flag('--tolerance', 2));
const only = flag('--only', null)
  ?.split(',')
  .map((id) => id.trim())
  .filter(Boolean);
const shardArg = flag('--shard', null);
const parallel = Number(flag('--parallel', 1));
if (!Number.isFinite(parallel) || parallel < 1) throw new Error('--parallel expects a positive number');
if (!Number.isFinite(tolerance) || tolerance < 0) throw new Error('--tolerance expects a non-negative number');

const [shardIndex, shardCount] = (() => {
  if (!shardArg) return [0, 1];
  const match = /^(\d+)\/(\d+)$/.exec(shardArg);
  if (!match) throw new Error(`--shard expects K/N, got ${shardArg}`);
  const index = Number(match[1]) - 1;
  const count = Number(match[2]);
  if (index < 0 || index >= count) throw new Error(`--shard ${shardArg} is out of range`);
  return [index, count];
})();

/// A small flat `save.json` the endgame check imports: two characters and the
/// nine research rows its audit validates, so the stats table has values.
const endgameSave = () => {
  const sparse = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const research = JSON.stringify(Array.from({ length: 9 }, () => sparse(240, {})));
  const character = (level) => JSON.stringify(sparse(30, { 0: level, 20: 42 }));
  return JSON.stringify({
    Research: research,
    Lv0_0: character(100),
    Lv0_1: character(60),
    MoneyBANK: 100,
    Money_0: 5,
  });
};

/// A small flat `save.json` carrying `OptLacc[26] = 1`, so the shadow page
/// answers YES from a real import rather than starting state.
const shadowSave = () => {
  const sparse = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  return JSON.stringify({
    Research: JSON.stringify([
      sparse(240, { 50: 3, 52: 4 }),
      sparse(240, {}),
      sparse(80, { 0: 1 }),
      sparse(80, {}),
      sparse(80, {}),
      [],
      [],
    ]),
    Lv0_0: JSON.stringify(sparse(30, { 20: 42 })),
    OptLacc: JSON.stringify(sparse(30, { 26: 1 })),
  });
};

/// A small flat `save.json` carrying a cavern: three jar types bought and one
/// slot filling an Opal jar, so the jar calculator has something to read. Rows
/// stay as short as the cells the jar reads.
const jarSave = () => {
  const row = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const holes = Array.from({ length: 35 }, () => []);
  holes[9] = row(21, { 20: 500 });
  holes[11] = row(38, { 37: 2 });
  holes[25] = [1, 0, 0, 250];
  return JSON.stringify({ Research: '[]', Holes: JSON.stringify(holes) });
};

/// A small flat `save.json` carrying worship tower levels and a best-wave table,
/// so the simulator has every type unlocked and a run to play.
const worshipSave = () => {
  const tower = Array(30).fill(0);
  for (let index = 9; index <= 17; index += 1) tower[index] = 1;
  const best = Array(8).fill(0);
  best[0] = 3;
  return JSON.stringify({
    Research: '[]',
    Lv0_0: JSON.stringify(Array(30).fill(0)),
    Tower: JSON.stringify(tower),
    TotemInfo: JSON.stringify([best, Array(8).fill(0), Array(8).fill(0)]),
    Dream: JSON.stringify(Array(24).fill(0)),
    SaltLick: JSON.stringify(Array(25).fill(0)),
    Menagerie: JSON.stringify([[], [], [], []]),
  });
};

/// A project document as the app writes it: the research state, the
/// supplements, one config per section and the section the window was on.
const projectDocument = () =>
  JSON.stringify({
    format: 'idleon-optimizer-project',
    version: 1,
    section: 'killroy',
    research: {
      researchLevel: 123,
      researchExp: 0,
      gridLevels: Array(240).fill(0),
      occFound: Array(80).fill(0),
      insightLvs: Array(80).fill(0),
      insightProgress: Array(80).fill(0),
      magData: [],
      shapePositions: [],
    },
    supplements: { serverVarResXp: 1.5, extraPct: 10 },
    sections: { killroy: { weeks: 5 } },
  });

/// The upstream optimizer's state string: gzip+base64 behind `ROv2:`.
const upstreamState = () => {
  const state = {
    _format: 'research-optimizer-state',
    _version: 2,
    researchLevel: 140,
    cachedResearchExp: 12345,
    gridLevels: Array(240).fill(1),
    occFound: Array(80).fill(1),
    insightLvs: Array(80).fill(0),
    insightProgress: Array(80).fill(0),
    magData: [],
    shapePositions: [],
    serverVarResXP: 1.25,
  };
  return `ROv2:${gzipSync(JSON.stringify(state)).toString('base64')}`;
};

/// One section's config document, the shape the import dialog applies on its own.
const sectionConfig = (id, config) =>
  JSON.stringify({
    format: 'idleon-optimizer-section',
    version: 1,
    section: id,
    config,
  });

/// Section-specific interaction checks. Every ready section must own one; a
/// planned section's interaction is the back action of its empty state.
const INTERACTIONS = {
  killroy: async (lane) => {
    await lane.call('click_element', { id: 'killroy-import' });
    await waitForNode(lane, 'import-drop', 'killroy: the empty state opens the import');
    await lane.call('click_element', { id: 'import-cancel' });
    await waitFor(lane, (nodes) => !has(nodes, 'import-drop'), 'killroy: cancel closes the import');
  },
  jar: async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-drop', 'jar: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    // The paste box keeps whatever the last import typed into it.
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: jarSave() });
    await waitForNode(lane, 'import-format', 'jar: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'jar-overview', 'jar: the cavern opens after Apply');
    const summary = labelOf(await lane.tree(), 'jar-summary-line') ?? '';
    if (!summary.includes('3 / 10')) throw new Error(`jar: the summary reads ${summary}`);
    await waitForNode(lane, 'jar-missing', 'jar: the unbought types are flagged');
    await lane.call('click_element', { id: 'production' });
    await waitForNode(lane, 'jar-production', 'jar: the production tab opens');
    await lane.call('click_element', { id: 'overview' });
    await waitForNode(lane, 'jar-overview', 'jar: the overview tab reopens');
  },
  farming: async (lane) => {
    await lane.call('click_element', { id: 'plot-1' });
    await waitFor(
      lane,
      (nodes) => labelOf(nodes, 'plot-detail') === 'Plot 2',
      'farming: clicking a plot moves the detail'
    );
    await lane.call('click_element', { id: 'back-to-dashboard' });
    await waitForNode(lane, 'dashboard', 'farming: back returns to the dashboard');
    await lane.call('click_element', { id: 'dashboard-resume' });
    await waitFor(
      lane,
      (nodes) => labelOf(nodes, 'plot-detail') === 'Plot 2',
      'farming: the plot selection survives the dashboard round-trip'
    );
  },
  endgame: async (lane) => {
    await waitForNode(lane, 'endgame-verdict', 'endgame: the verdict is there before a save');
    const before = labelOf(await lane.tree(), 'endgame-verdict');
    if (before !== 'NO') throw new Error(`endgame: expected NO before a save, got ${before}`);
    const importSave = async (text, ready) => {
      await lane.call('click_element', { id: 'header-import' });
      await waitForNode(lane, 'import-input', 'endgame: the import dialog opens');
      await lane.call('click_element', { id: 'import-input' });
      await lane.call('keyboard', { keystroke: 'secondary-a' });
      await lane.call('type_text', { text });
      await waitForNode(lane, 'import-format', 'endgame: the save previews');
      await lane.call('click_element', { id: 'import-apply' });
      await waitForNode(lane, ready, `endgame: ${ready} after Apply`);
    };
    // A save whose research rows are incomplete imports, and the audit says why.
    await importSave('{"Research":[]}', 'endgame-stats-error');
    const failed = labelOf(await lane.tree(), 'endgame-stats-error') ?? '';
    if (!failed.includes('Unable to calculate account statistics')) {
      throw new Error(`endgame: the error panel reads ${failed}`);
    }
    await importSave(endgameSave(), 'endgame-stat-totalLevels');
    const nodes = await lane.tree();
    const levels = labelOf(nodes, 'endgame-stat-totalLevels') ?? '';
    if (!levels.includes('160')) throw new Error(`endgame: totalLevels reads ${levels}`);
    const after = labelOf(nodes, 'endgame-verdict');
    if (after !== 'NO') throw new Error(`endgame: expected NO after a save, got ${after}`);
  },
  shadow: async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-drop', 'shadow: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('type_text', { text: shadowSave() });
    await lane.call('click_element', { id: 'import-apply' });
    await waitFor(
      lane,
      (nodes) => labelOf(nodes, 'shadow-verdict') === 'Shadow ban verdict: YES',
      'shadow: a nonzero OptLacc[26] answers YES',
    );
    const detail = labelOf(await lane.tree(), 'shadow-detail') ?? '';
    check(
      'shadow: the save status and the flag are visible',
      detail.includes('Save imported') && detail.includes('OptLacc[26] = 1'),
      detail,
    );
  },
  pickle: async (lane) => {
    await lane.call('focus_element', { id: 'ely-text-input' });
    await lane.call('type_text', { text: '50000000' });
    await waitFor(
      lane,
      (nodes) =>
        !has(nodes, 'pickle-empty') &&
        (labelOf(nodes, 'pickle-row-slimeB') ?? '').includes('max pickles 125'),
      'pickle: DPS updates the table',
    );
    await lane.call('set_value', { id: 'thumb-0', value: '120' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'pickle-row-slimeB') ?? '').includes('max pickles 139'),
      'pickle: the kill-time budget updates the table',
    );
  },
  research: async (lane) => {
    await lane.call('click_element', { id: 'run' });
    await waitForNode(lane, 'run-optimize', 'research: the optimize tab opens');
    await lane.call('click_element', { id: 'rates' });
    await waitForNode(lane, 'formula-tree', 'research: the shared formula tree opens');
    await lane.call('click_element', { id: 'formula-search' });
    await lane.call('type_text', { text: 'takin' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'formula-match-count') ?? '').includes('matching'),
      'research: searching the formula tree highlights nodes',
    );
    await lane.call('click_element', { id: 'formula-collapse' });
    await lane.call('click_element', { id: 'grid' });
    await waitForNode(lane, 'cell-129', 'research: the grid tab reopens');
  },
  'coral-god-rank': async (lane) => {
    // Turning the assumption off makes the save-derived unlock state (or its
    // absence) visible, so the controls prove they are wired to the planner.
    await lane.call('click_element', { id: 'coral-assume-unlocked' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'coral-validation') ?? '').includes('unlock state'),
      'coral: the unlock state follows the assumption checkbox',
    );
    await lane.call('click_element', { id: 'coral-assume-unlocked' });
  },
  statue: async (lane) => {
    await lane.call('click_element', { id: 'statue-search' });
    await lane.call('type_text', { text: 'move' });
    await waitFor(
      lane,
      (nodes) => has(nodes, 'statue-row-1') && !has(nodes, 'statue-row-0'),
      'statue: search narrows the table',
    );
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('keyboard', { keystroke: 'backspace' });
    await waitForNode(lane, 'statue-row-0', 'statue: clearing search restores the table');

    await lane.call('focus_element', { id: 'statue-sort' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => nodes.find((node) => node.id === 'statue-sort')?.value?.value === 'Lowest level first',
      'statue: sort changes to lowest level first',
    );
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'up' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => nodes.find((node) => node.id === 'statue-sort')?.value?.value === 'Game order',
      'statue: sort returns to game order',
    );
  },
  sailing: async (lane) => {
    await waitForNode(lane, 'fleet-mode', 'sailing: the assignment controls render');
    await lane.call('click_element', { id: 'fleet-mode-saved' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'fleet-mode') ?? '').includes('Current save'),
      'sailing: the current-save pairings show',
    );
    await lane.call('click_element', { id: 'fleet-mode-optimized' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'fleet-mode') ?? '').includes('Optimized'),
      'sailing: the optimized pairings show',
    );
  },
  monument: async (lane) => {
    await lane.call('click_element', { id: 'sources' });
    await waitForNode(lane, 'monument-pane-sources', 'monument: the bonus sources tab opens');
    await lane.call('click_element', { id: 'bravery' });
    await waitForNode(lane, 'monument-pane-bravery', 'monument: the bravery tab reopens');
  },
  'worship-td': async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'worship-td: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: worshipSave() });
    await waitForNode(lane, 'import-format', 'worship-td: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'worship-map', 'worship-td: the map renders after Apply');
    await lane.call('click_element', { id: 'worship-tower-0' });
    await lane.call('click_element', { id: 'worship-slot-3-561' });
    await waitForNode(lane, 'worship-tower-3-561', 'worship-td: the tower stands on the map');
    await lane.call('click_element', { id: 'worship-speed-50' });
    await lane.call('click_element', { id: 'worship-play' });
    await waitFor(
      lane,
      (nodes) => /^Wave [1-9]/.test(labelOf(nodes, 'worship-wave') ?? ''),
      'worship-td: playback advances the wave',
    );
    await lane.call('click_element', { id: 'worship-restart' });
    await waitFor(
      lane,
      (nodes) =>
        labelOf(nodes, 'worship-wave') === 'Wave 0' && !has(nodes, 'worship-tower-3-561'),
      'worship-td: restart clears the run and the towers',
    );
    // No towers on the restarted run, so the base totem falls and the replay
    // it recorded loads by itself.
    await lane.call('click_element', { id: 'worship-speed-300' });
    await lane.call('click_element', { id: 'worship-play' });
    await waitForNode(
      lane,
      'worship-replay',
      'worship-td: the replay loads when the run ends',
      30000,
    );
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'worship-map') ?? '').includes('replay'),
      'worship-td: the map switches to the replay',
    );
    await lane.call('focus_element', { id: 'thumb-0' });
    await lane.call('keyboard', { keystroke: 'home' });
    await waitFor(
      lane,
      (nodes) => /Replay frame 0 of \d+/.test(labelOf(nodes, 'worship-replay') ?? ''),
      'worship-td: the scrubber seeks the start of the timeline',
    );
    await lane.call('focus_element', { id: 'thumb-0' });
    await lane.call('keyboard', { keystroke: 'end' });
    await waitFor(
      lane,
      (nodes) => /Replay frame (\d+) of \1 at/.test(labelOf(nodes, 'worship-replay') ?? ''),
      'worship-td: the scrubber reaches the last frame',
    );
    await lane.call('click_element', { id: 'worship-replay-speed-100' });
    await lane.call('click_element', { id: 'worship-replay-play' });
    await waitFor(
      lane,
      (nodes) => {
        const match = /Replay frame (\d+) of/.exec(labelOf(nodes, 'worship-replay') ?? '');
        return match !== null && Number(match[1]) > 0;
      },
      'worship-td: replay playback advances after play',
    );
    await lane.call('click_element', { id: 'worship-plan-tab' });
    await waitForNode(lane, 'worship-plan-body', 'worship-td: the plan pane opens');
    await lane.call('click_element', { id: 'worship-plan-simulate' });
    await waitForNode(lane, 'worship-plan-final', 'worship-td: the plan run reports final towers');
    // The search screens dozens of whole runs: it reports progress, and the
    // recommendation it keeps becomes the plan the Plan pane runs.
    await lane.call('click_element', { id: 'worship-optimizer-tab' });
    await waitForNode(lane, 'worship-optimizer-body', 'worship-td: the optimizer pane opens');
    await lane.call('click_element', { id: 'worship-optimize-run' });
    await waitForNode(
      lane,
      'worship-optimize-result',
      'worship-td: the search recommends a plan',
      120000,
    );
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'worship-optimize-verdict') ?? '').startsWith('Recommended plan clears wave'),
      'worship-td: the recommendation names the waves it clears',
    );
    await waitForNode(
      lane,
      'worship-optimize-baselines',
      'worship-td: the baselines are compared',
    );
    await lane.call('click_element', { id: 'worship-optimize-use' });
    await waitForNode(lane, 'worship-plan-body', 'worship-td: the recommendation opens the plan pane');
    await waitFor(
      lane,
      (nodes) => labelOf(nodes, 'worship-plan-optimized') === 'Optimizer plan',
      'worship-td: the plan pane adopts the recommendation',
    );
    // Adopting the plan is not an input change: the recommendation stays fresh.
    await lane.call('click_element', { id: 'worship-optimizer-tab' });
    await waitForNode(lane, 'worship-optimize-result', 'worship-td: the search reopens');
    check(
      'worship-td: the adopted recommendation is not stale',
      !has(await lane.tree(), 'worship-optimize-stale-badge'),
    );
    // The adopted plan runs through the same engine the search used.
    await lane.call('click_element', { id: 'worship-plan-tab' });
    await lane.call('click_element', { id: 'worship-plan-simulate' });
    await waitForNode(
      lane,
      'worship-plan-final',
      'worship-td: the recommended plan simulates in the plan pane',
      60000,
    );
    await lane.call('click_element', { id: 'worship-model-tab' });
    await waitForNode(lane, 'worship-assumptions', 'worship-td: the model assumptions list');
    await lane.call('click_element', { id: 'worship-simulator-tab' });
    await waitForNode(lane, 'worship-map', 'worship-td: the simulator tab returns');

  },
  clamworks: async (lane) => {
    await lane.call('click_element', { id: 'clamworks-multikill-more' });
    await lane.call('click_element', { id: 'upgrades' });
    await waitForNode(lane, 'clamworks-upgrades', 'clamworks: the upgrades tab opens');
    await lane.call('click_element', { id: 'overview' });
    await waitForNode(lane, 'clamworks-overview', 'clamworks: the overview tab reopens');
  },
};

const prefix = shardCount > 1 ? `[${shardIndex + 1}/${shardCount}] ` : '';
const log = (message) => console.log(`${prefix}${message}`);
let failures = 0;

const check = (label, ok, detail = '') => {
  if (ok) {
    log(`ok   ${label}`);
  } else {
    failures += 1;
    log(`FAIL ${label}${detail ? ` — ${detail}` : ''}`);
  }
};

const fail = (label, error) => {
  failures += 1;
  log(`FAIL ${label} — ${error instanceof Error ? error.message : error}`);
};

const has = (nodes, id) => nodes.some((node) => node.id === id);
const labelOf = (nodes, id) => nodes.find((node) => node.id === id)?.label ?? null;

const waitFor = async (lane, predicate, label, timeoutMs = 15000) => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate(await lane.tree())) {
      log(`ok   ${label}`);
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`${label} (timed out)`);
};

const waitForNode = (lane, id, label, timeoutMs) =>
  waitFor(lane, (nodes) => has(nodes, id), label, timeoutMs);

/// Type into the dashboard search and wait for the card it should list. A
/// window busy with the previous section's frame can drop the first keystrokes,
/// so this retries once instead of failing the section on a lost focus.
const searchFor = async (lane, text, cardId, label) => {
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    await lane.call('click_element', { id: 'dashboard-search' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text });
    try {
      await waitForNode(lane, cardId, label, 6000);
      return;
    } catch (error) {
      if (attempt === 2) throw error;
    }
  }
};

/// Open a card from the dashboard, then verify the section it routes to.
async function visitCard(lane, unit) {
  await waitForNode(lane, 'dashboard-search', 'the dashboard is ready');
  await searchFor(lane, unit.title, `card-${unit.id}`, `${unit.id}: search lists its card`);
  await lane.call('click_element', { id: `card-${unit.id}` });
  await visitSection(lane, unit);
  await waitForNode(lane, 'dashboard', `${unit.id}: the card is back on the dashboard`);
  const resume = labelOf(await lane.tree(), 'dashboard-resume') ?? '';
  check(
    `${unit.id}: Resume remembers the section`,
    resume.includes(unit.title),
    resume,
  );
}

/// Assert the route, shared header, interaction and screenshot of one section.
async function visitSection(lane, unit) {
  const root = `section-${unit.id}`;
  await waitForNode(lane, root, `${unit.id}: section root`);
  const nodes = await lane.tree();
  const missing = [
    'back-to-dashboard',
    'section-title',
    'header-import',
    'header-export',
    'header-save',
  ]
    .filter((id) => !has(nodes, id));
  check(`${unit.id}: shared header`, missing.length === 0, `missing ${missing.join(', ')}`);
  check(
    `${unit.id}: header names the section`,
    (labelOf(nodes, 'section-title') ?? '').includes(unit.title),
    String(labelOf(nodes, 'section-title')),
  );

  if (unit.status === 'Planned') {
    // The shared empty state is not in the semantic tree; its back action is
    // the stable, assertable control every planned section renders.
    check(`${unit.id}: planned empty state`, has(nodes, 'section-back'));
    await capture(lane, unit);
    await lane.call('click_element', { id: 'section-back' });
    await waitForNode(lane, 'dashboard', `${unit.id}: empty-state back opens the dashboard`);
    return;
  }

  const interaction = INTERACTIONS[unit.id];
  if (!interaction) {
    throw new Error(`no interaction check registered for ready section ${unit.id}`);
  }
  await interaction(lane);
  await capture(lane, unit);
  await lane.call('click_element', { id: 'back-to-dashboard' });
  await waitForNode(lane, 'dashboard', `${unit.id}: header back opens the dashboard`);
}

/// Save the capture and its sidecar, then optionally compare a baseline.
async function capture(lane, unit) {
  if (noScreenshots) {
    log(`skip ${unit.id}: capture skipped in a parallel lane`);
    return;
  }
  mkdirSync(shotsDir, { recursive: true });
  // Park the pointer so a hovered control does not make the capture depend on
  // where the previous card happened to leave the cursor.
  await lane.call('pointer_move', { x: 4, y: 4 });
  const png = await lane.screenshot(`section-${unit.id}`);
  const { width, height } = decodePng(png);
  check(`${unit.id}: screenshot ${width}x${height}`, width > 0 && height > 0);
  writeFileSync(join(shotsDir, `${unit.id}.png`), png);
  writeFileSync(
    join(shotsDir, `${unit.id}.json`),
    `${JSON.stringify(
      {
        id: unit.id,
        title: unit.title,
        group: unit.group,
        status: unit.status,
        dashboard: unit.dashboard,
        root: `section-${unit.id}`,
        screenshot: `${unit.id}.png`,
        width,
        height,
        sha256: createHash('sha256').update(png).digest('hex'),
      },
      null,
      2,
    )}\n`,
  );

  if (!baselineDir) return;
  mkdirSync(baselineDir, { recursive: true });
  const path = join(baselineDir, `${unit.id}.png`);
  const existed = existsSync(path);
  if (!existed || updateBaseline) {
    writeFileSync(path, png);
    check(`${unit.id}: baseline ${existed ? 'updated' : 'created'}`, true);
    return;
  }
  const diff = comparePng(png, readFileSync(path), { tolerance });
  const detail = `${(diff.ratio * 100).toFixed(2)}% pixels, max delta ${diff.maxDelta}, size delta ${diff.sizeDelta}`;
  if (diff.same) log(`ok   ${unit.id}: matches its baseline (${detail})`);
  else check(`${unit.id}: matches its baseline`, false, detail);
}

/// Open the shared import dialog. A click that lands while a previous lane's
/// window is still closing can be lost, so one retry keeps the check about the
/// import rather than about window focus.
async function openImport(lane, label) {
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    await lane.call('click_element', { id: 'header-import' });
    try {
      await waitForNode(lane, 'import-drop', `${label}: the import dialog opens`, 6000);
      return;
    } catch (error) {
      if (attempt === 2) throw error;
      await new Promise((resolve) => setTimeout(resolve, 300));
    }
  }
}

/// Paste `text` into the shared import dialog and wait for the preview to name
/// the document, then apply it.
async function importText(lane, text, format, label) {
  await openImport(lane, label);
  await lane.call('click_element', { id: 'import-input' });
  await lane.call('keyboard', { keystroke: 'secondary-a' });
  await lane.call('type_text', { text });
  await waitFor(
    lane,
    (nodes) => (labelOf(nodes, 'import-format') ?? '').includes(format),
    `${label}: the preview names the ${format}`,
  );
  await lane.call('click_element', { id: 'import-apply' });
}

/// Paste `text` that cannot be read and expect the dialog to name the failure,
/// offer no preview and keep Apply disabled.
async function importError(lane, text, expected, label) {
  await openImport(lane, label);
  await lane.call('click_element', { id: 'import-input' });
  await lane.call('keyboard', { keystroke: 'secondary-a' });
  await lane.call('type_text', { text });
  await waitFor(
    lane,
    (nodes) => (labelOf(nodes, 'import-status') ?? '').includes(expected),
    `${label}: the message names "${expected}"`,
  );
  const nodes = await lane.tree();
  check(`${label}: no preview is offered`, !has(nodes, 'import-format'));
  const apply = nodes.find((node) => node.id === 'import-apply');
  check(`${label}: Apply stays disabled`, !(apply?.actions ?? []).includes('click'));
  await lane.call('click_element', { id: 'import-cancel' });
  await waitFor(
    lane,
    (current) => !has(current, 'import-drop'),
    `${label}: cancel closes the dialog`,
  );
}

/// The project round trip: export opens over the window state, then the project
/// document, an ROv2 state string and a section config each apply and leave a
/// visible mark.
async function visitProjectRoundTrip() {
  let lane = null;
  try {
    lane = await McpLane.start({ appBin, keep });
    await waitForNode(lane, 'dashboard', 'project: the dashboard is ready');

    await lane.call('click_element', { id: 'header-export' });
    await waitForNode(lane, 'export-text', 'project: the export dialog opens');
    const summary = labelOf(await lane.tree(), 'export-summary') ?? '';
    check(
      'project: the export names its bytes and sections',
      summary.includes('bytes') && summary.includes('section config'),
      summary,
    );
    await lane.call('click_element', { id: 'export-copy' });
    await waitFor(lane, (nodes) => !has(nodes, 'export-text'), 'project: copy closes the export');

    await importText(lane, projectDocument(), 'project', 'project');
    await waitForNode(lane, 'section-killroy', 'project: applying lands on the saved section');
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'status-status') ?? '').includes('research level 123'),
      'project: the status names the applied research level',
    );
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'status-status') ?? '').includes('researchLevel: 100 → 123'),
      'project: the status logs the research level change',
    );
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-changes', 'project: the dialog lists what the import changed');
    const logged = (await lane.tree()).filter((node) => node.id?.startsWith('import-change'));
    check(
      'project: the change log names the field and both values',
      logged.some((node) => (node.label ?? '').includes('researchLevel: 100 → 123')),
      logged.map((node) => node.label).join(' | '),
    );
    await lane.call('click_element', { id: 'import-cancel' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'import-drop'),
      'project: the change log closes with the dialog',
    );

    await lane.call('click_element', { id: 'back-to-dashboard' });
    await waitForNode(lane, 'dashboard', 'project: back on the dashboard');
    await importText(lane, upstreamState(), 'upstream state', 'state');
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'status-status') ?? '').includes('research level 140'),
      'state: the decompressed state migrated to its research level',
    );

    await searchFor(lane, 'Research Grid', 'card-research', 'config: search lists the research card');
    await lane.call('click_element', { id: 'card-research' });
    await waitForNode(lane, 'section-research', 'config: the research section opens');
    await importText(lane, sectionConfig('research', { tab: 'run' }), 'section config', 'config');
    await waitForNode(lane, 'run-optimize', 'config: the tab came from the section config');

    // Each failure gets its own message and nothing is applied.
    await importError(lane, '{"Research": ', 'not valid JSON', 'malformed');
    await importError(
      lane,
      JSON.stringify({ format: 'some-other-tool', version: 1 }),
      'unknown format',
      'wrong format',
    );
    await importError(
      lane,
      JSON.stringify({ format: 'idleon-optimizer-project', version: 9 }),
      'unsupported version',
      'bad version',
    );
    await importError(
      lane,
      JSON.stringify({
        format: 'idleon-optimizer-project',
        version: 1,
        save: { not: 'a save' },
        research: { researchLevel: 130 },
      }),
      'no save fields found',
      'unreadable save',
    );
    await importError(
      lane,
      sectionConfig('killroy', { weeks: 'twelve' }),
      'killroy',
      'malformed config',
    );
    // The failed imports never replaced the log or the state they could not read.
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-changes', 'errors: the last applied log survives');
    const survived = (await lane.tree()).filter((node) => node.id?.startsWith('import-change'));
    check(
      'errors leave the applied log alone',
      survived.some((node) => (node.label ?? '').includes('research: config changed')),
      survived.map((node) => node.label).join(' | '),
    );
    await lane.call('click_element', { id: 'import-cancel' });
    await waitForNode(
      lane,
      'run-optimize',
      'errors leave the applied section config in place',
    );
  } catch (error) {
    fail('project round trip', error);
  } finally {
    lane?.close();
  }
}

/// The session file: one lane imports a save and opens a section, a second lane
/// starts from the same app-data directory and must land back there, and a
/// third must back up a corrupt file and start on a clean dashboard.
async function visitSessionPersistence() {
  const stateDir = mkdtempSync(join(tmpdir(), 'idleon-session-check-'));
  const sessionFile = join(stateDir, 'session.json');
  let lane = null;
  try {
    lane = await McpLane.start({ appBin, stateDir, keep: false });
    await waitForNode(lane, 'dashboard', 'session: a fresh app-data directory starts on the dashboard');
    await importText(lane, endgameSave(), 'save.json', 'session');
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'header-save') ?? '').startsWith('Save imported'),
      'session: the save loads',
    );
    await searchFor(lane, 'Research Grid', 'card-research', 'session: search finds the research card');
    await lane.call('click_element', { id: 'card-research' });
    await waitForNode(lane, 'section-research', 'session: a section opens');
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-recent', 'session: the import dialog lists recent imports');
    const recent = (await lane.tree()).filter((node) => node.id?.startsWith('recent-import'));
    check(
      'session: a recent import names its format and time',
      recent.some((node) => {
        const line = node.label ?? '';
        return line.includes('save.json') && /\d{1,2}:\d{2}/.test(line);
      }),
      recent.map((node) => node.label).join(' | '),
    );
    await lane.call('click_element', { id: 'import-cancel' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'import-drop'),
      'session: the import dialog closes',
    );
    // A tab switch is section config, not an import: only the debounced write
    // can carry it, so give that write time to land before the process closes.
    await lane.call('click_element', { id: 'run' });
    await waitForNode(lane, 'run-optimize', 'session: the section config changes');
    await new Promise((resolve) => setTimeout(resolve, 600));
    lane.close();
    lane = null;

    lane = await McpLane.start({ appBin, stateDir, keep: false });
    await waitForNode(lane, 'section-research', 'session: relaunch restores the last section');
    const restored = await lane.tree();
    const save = labelOf(restored, 'header-save') ?? '';
    check(
      'session: relaunch restores the imported save',
      save.startsWith('Save imported'),
      save,
    );
    check(
      'session: relaunch restores the section config',
      has(restored, 'run-optimize'),
      restored.map((node) => node.id).filter(Boolean).slice(0, 20).join(', '),
    );
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-recent', 'session: recent imports survive a relaunch');
    const persisted = (await lane.tree()).filter((node) => node.id?.startsWith('recent-import'));
    check(
      'session: the restored recent import keeps its format and time',
      persisted.some((node) => {
        const line = node.label ?? '';
        return line.includes('save.json') && /\d{1,2}:\d{2}/.test(line);
      }),
      persisted.map((node) => node.label).join(' | '),
    );
    await lane.call('click_element', { id: 'import-cancel' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'import-drop'),
      'session: the restored import dialog closes',
    );
    lane.close();
    lane = null;

    writeFileSync(sessionFile, '{not a session');
    lane = await McpLane.start({ appBin, stateDir, keep: false });
    await waitForNode(lane, 'dashboard', 'session: a corrupt file starts clean on the dashboard');
    const nodes = await lane.tree();
    const status = labelOf(nodes, 'status-status') ?? '';
    check('session: the corrupt file is reported', status.includes('corrupt'), status);
    const saveAfterCorrupt = labelOf(nodes, 'header-save') ?? '';
    check(
      'session: a corrupt file leaves no save behind',
      saveAfterCorrupt === 'No save',
      saveAfterCorrupt,
    );
    const backups = readdirSync(stateDir).filter((name) => name.startsWith('session.corrupt-'));
    check('session: the corrupt file is backed up', backups.length === 1, backups.join(', '));
  } catch (error) {
    fail('session persistence', error);
  } finally {
    lane?.close();
  }
}

async function visitHidden(unit) {
  let lane = null;
  try {
    lane = await McpLane.start({ appBin, keep, section: unit.id });
    await waitForNode(lane, `section-${unit.id}`, `${unit.id}: --section opens its root`);
    await visitSection(lane, unit);
  } catch (error) {
    fail(`${unit.id}: hidden tool`, error);
  } finally {
    lane?.close();
  }
}

async function visitCards(units) {
  let lane = null;
  const ensureLane = async () => {
    lane ??= await McpLane.start({ appBin, keep });
    return lane;
  };
  for (const unit of units) {
    try {
      await visitCard(await ensureLane(), unit);
    } catch (error) {
      fail(`${unit.id}: card`, error);
      // A failed route can leave the window anywhere; start a clean lane.
      lane?.close();
      lane = null;
    }
  }
  lane?.close();
}

/// Fork N isolated lanes and wait for all of them. Each child runs its own
/// shard, so private endpoint directories prove the lanes cannot see each other.
async function runParallel(count) {
  const forwarded = [];
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--parallel') {
      index += 1;
      continue;
    }
    forwarded.push(argv[index]);
  }
  const runs = Array.from({ length: count }, (_, lane) => new Promise((resolve) => {
    const args = [process.argv[1], ...forwarded, '--shard', `${lane + 1}/${count}`];
    // Native window capture needs the window on screen; only one lane can be
    // frontmost, so parallel shards prove isolation and interactions instead.
    if (!forwarded.includes('--no-screenshots')) args.push('--no-screenshots');
    const child = spawn(process.execPath, args, { stdio: 'inherit' });
    child.on('exit', (code) => resolve(code ?? 1));
  }));
  const codes = await Promise.all(runs);
  failures += codes.filter((code) => code !== 0).length;
  if (codes.some((code) => code !== 0)) process.exitCode = 1;
}

try {
  if (parallel > 1 && !shardArg) {
    await runParallel(parallel);
  } else {
    if (!existsSync(appBin)) {
      throw new Error(`no app binary at ${appBin}; run cargo build -p idleon-optimizer first`);
    }
    const registry = JSON.parse(execFileSync(appBin, ['--list-sections'], { encoding: 'utf8' }));
    const selected = registry.filter((unit) => !only || only.includes(unit.id));
    const unknown = (only ?? []).filter((id) => !registry.some((unit) => unit.id === id));
    if (unknown.length > 0) throw new Error(`unknown section ids: ${unknown.join(', ')}`);
    const units = selected.filter((_, index) => index % shardCount === shardIndex);
    const cards = units.filter((unit) => unit.dashboard);
    const hidden = units.filter((unit) => !unit.dashboard);
    log(`${units.length} sections (${cards.length} cards, ${hidden.length} hidden tools)`);
    await visitCards(cards);
    for (const unit of hidden) await visitHidden(unit);
    // One window proves the whole round trip; a shard's other lanes are busy.
    if (!only && shardIndex === 0) {
      await visitProjectRoundTrip();
      await visitSessionPersistence();
    }
  }
} catch (error) {
  failures += 1;
  log(`sections failed: ${error.message}`);
}

if (failures > 0) {
  log(`${failures} check(s) failed`);
  process.exitCode = 1;
} else {
  log('all sections passed');
}
