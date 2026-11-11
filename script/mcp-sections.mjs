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
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
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
  monument: async (lane) => {
    await lane.call('click_element', { id: 'sources' });
    await waitForNode(lane, 'monument-pane-sources', 'monument: the bonus sources tab opens');
    await lane.call('click_element', { id: 'bravery' });
    await waitForNode(lane, 'monument-pane-bravery', 'monument: the bravery tab reopens');
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

const waitForNode = (lane, id, label) =>
  waitFor(lane, (nodes) => has(nodes, id), label);

/// Open a card from the dashboard, then verify the section it routes to.
async function visitCard(lane, unit) {
  await waitForNode(lane, 'dashboard-search', 'the dashboard is ready');
  await lane.call('click_element', { id: 'dashboard-search' });
  await lane.call('keyboard', { keystroke: 'escape' });
  await lane.call('type_text', { text: unit.title });
  await waitForNode(lane, `card-${unit.id}`, `${unit.id}: search lists its card`);
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
  const missing = ['back-to-dashboard', 'section-title', 'header-import', 'header-save']
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
