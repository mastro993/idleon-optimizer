// Drives the app through gpui-mcp and asserts the flows that matter: selecting a
// square, dragging a shape, the keyboard, optimizing, applying a result, the
// supplements editor and the save importer.
//
// By default it launches its own window with a private endpoint directory, so
// several lanes can run this at once without fighting over one instance.
//
// Usage: node script/mcp-drive.mjs [--attach] [--keep] [--app-bin PATH]
//   --attach    drive a window you already started (shared discovery directory)
//   --keep      leave the spawned window running for a look
import { spawn, execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const argv = process.argv.slice(2);
const flag = (name, fallback) => {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : fallback;
};
const keep = argv.includes('--keep');
const attach = argv.includes('--attach');
const appBin = flag('--app-bin', join(process.cwd(), 'target/debug/idleon-optimizer'));

/// A small flat `save.json` the run pastes into the import dialog: two
/// characters at level 42 and 30, so the preview's numbers are assertable.
const harnessSave = () => {
  const sparse = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const character = (level) => JSON.stringify(sparse(30, { 20: level }));
  return JSON.stringify({
    Research: JSON.stringify([
      sparse(240, { 50: 3, 52: 4 }),
      sparse(240, {}),
      sparse(80, { 0: 1, 1: 1 }),
      sparse(80, {}),
      sparse(80, {}),
      [],
      [],
    ]),
    Lv0_0: character(42),
    Lv0_1: character(30),
  });
};

/// macOS stops drawing an occluded window, and the bridge waits for frames, so
/// bring this instance forward before driving it.
const activate = (pid) => {
  if (process.platform !== 'darwin' || !pid) return;
  try {
    execFileSync('osascript', [
      '-e',
      `tell application "System Events" to set frontmost of first process whose unix id is ${pid} to true`,
    ], { stdio: 'ignore' });
  } catch {
    // Activation is best effort; a visible window works without it.
  }
};

let endpointDir = null;
let appProcess = null;
if (!attach) {
  if (!existsSync(appBin)) {
    console.error(`no app binary at ${appBin}; run cargo build -p idleon-optimizer first`);
    process.exit(1);
  }
  endpointDir = mkdtempSync(join(tmpdir(), 'idleon-lane-'));
  appProcess = spawn(appBin, ['--endpoint-dir', endpointDir], { stdio: 'ignore' });
  const deadline = Date.now() + 30000;
  let published = false;
  while (Date.now() < deadline && !published) {
    published = readdirSync(endpointDir).some((name) => name.endsWith('.json'));
    if (!published) execFileSync('sleep', ['0.2']);
  }
  if (!published) {
    console.error('the app never published an endpoint; is it crashing at startup?');
    appProcess.kill();
    process.exit(1);
  }
}

const stopApp = () => {
  if (appProcess && !keep) {
    appProcess.kill();
    appProcess = null;
  }
};

const child = spawn(
  'gpui-mcp',
  endpointDir ? ['--endpoint-dir', endpointDir] : [],
  { stdio: ['pipe', 'pipe', 'pipe'] },
);
let nextId = 1;
const pending = new Map();
let buffer = '';
let failures = 0;

child.stdout.on('data', (chunk) => {
  buffer += chunk.toString();
  let index;
  while ((index = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, index).trim();
    buffer = buffer.slice(index + 1);
    if (!line) continue;
    const message = JSON.parse(line);
    const entry = pending.get(message.id);
    if (entry) {
      pending.delete(message.id);
      entry(message);
    }
  }
});
child.stderr.on('data', (chunk) => process.stderr.write(chunk));

const request = (method, params) => {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, (message) => {
      if (message.error) reject(new Error(JSON.stringify(message.error)));
      else resolve(message.result);
    });
    child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
  });
};

let appPid = null;

const call = async (name, args = {}) => {
  for (let attempt = 1; ; attempt += 1) {
    const result = await request('tools/call', { name, arguments: args });
    const text = result?.content?.find((part) => part.type === 'text')?.text;
    // macOS stops drawing an occluded window; the bridge waits for the frame the
    // action should cause. Bring the app forward and try once more.
    if (result?.isError && /frame wait timed out/i.test(text ?? '') && attempt === 1) {
      activate(appPid);
      await new Promise((resolve) => setTimeout(resolve, 250));
      continue;
    }
    if (result?.isError) throw new Error(`${name}: ${text}`);
    return { text, content: result?.content ?? [] };
  }
};

const tree = async () => {
  const { text } = await call('get_ui_tree');
  const parsed = JSON.parse(text);
  return Object.values(parsed.nodes ?? {});
};

const check = (label, condition, detail = '') => {
  if (condition) {
    console.log(`ok   ${label}`);
  } else {
    failures += 1;
    console.log(`FAIL ${label}${detail ? ` — ${detail}` : ''}`);
  }
};

const labelOf = (nodes, id) => nodes.find((node) => node.id === id)?.label ?? null;
const has = (nodes, id) => nodes.some((node) => node.id === id);
const textIn = (nodes, needle) =>
  nodes.some((node) => JSON.stringify(node.label ?? '').includes(needle));

const waitFor = async (predicate, label, timeoutMs = 15000) => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate(await tree())) {
      console.log(`ok   ${label}`);
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  failures += 1;
  console.log(`FAIL ${label} (timed out)`);
  return false;
};

try {
  await request('initialize', {
    protocolVersion: '2025-06-18',
    capabilities: {},
    clientInfo: { name: 'drive', version: '0' },
  });
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized', params: {} })}\n`);

  // The endpoint file lands before the bridge answers, so poll for discovery.
  let app = null;
  for (let attempt = 0; attempt < 40 && !app; attempt += 1) {
    const running = JSON.parse((await call('list_apps')).text);
    app = (Array.isArray(running) ? running : running.apps ?? []).find(
      (candidate) => (candidate.app_id ?? candidate.id) === 'idleon-optimizer',
    );
    if (!app) execFileSync('sleep', ['0.25']);
  }
  if (!app) throw new Error('the app is not running; start ./target/debug/idleon-optimizer first');
  appPid = appProcess?.pid ?? app.pid;
  activate(appPid);

  // The dashboard is the landing page: every upstream card is exposed.
  await waitFor((current) => has(current, 'card-research'), 'dashboard opens');
  let nodes = await tree();
  const cards = nodes.filter((node) => /^card-/.test(node.id ?? ''));
  check('all 25 dashboard cards are exposed', cards.length === 25, String(cards.length));
  check('cards carry their world badge', (labelOf(nodes, 'card-research') ?? '').includes('W7'));

  // Search filters by world, and Escape clears it.
  await call('click_element', { id: 'dashboard-search' });
  await call('type_text', { text: 'w5' });
  await waitFor(
    (current) => has(current, 'card-fountain') && !has(current, 'card-combat-sim'),
    'search filters by world',
  );
  await call('keyboard', { keystroke: 'escape' });
  await waitFor((current) => has(current, 'card-combat-sim'), 'Escape clears the search');

  // A planned card opens its section; Escape and the header's Back both return,
  // and the dashboard offers to resume the section it remembers.
  await call('click_element', { id: 'card-fishing' });
  await waitFor((current) => has(current, 'section-fishing'), 'a card opens its section');
  await call('keyboard', { keystroke: 'escape' });
  await waitFor(
    (current) => has(current, 'dashboard-resume') && !has(current, 'section-fishing'),
    'Escape returns to the dashboard',
  );
  await call('click_element', { id: 'dashboard-resume' });
  await waitFor((current) => has(current, 'section-fishing'), 'resume reopens the section');
  await call('click_element', { id: 'back-to-dashboard' });
  await waitFor((current) => has(current, 'card-fishing'), 'back returns to the dashboard');

  // The research card opens the one implemented section. Filter first so the
  // card is in view, as scrolling to it by hand would do.
  await call('click_element', { id: 'dashboard-search' });
  await call('type_text', { text: 'research' });
  await waitFor((current) => has(current, 'card-research'), 'search finds research');
  await call('click_element', { id: 'card-research' });
  await waitFor((current) => has(current, 'cell-129'), 'research card opens the grid');

  // The router keeps the section's own state and remembers the last section:
  // switch tabs, leave with the back shortcut, resume the same tab.
  await call('click_element', { id: 'run' });
  await waitFor((current) => has(current, 'run-optimize'), 'the optimize tab opens');
  await call('keyboard', { keystroke: 'secondary-left' });
  await waitFor(
    (current) => has(current, 'dashboard-resume') && !has(current, 'section-research'),
    'the back shortcut returns to the dashboard',
  );
  await call('click_element', { id: 'dashboard-resume' });
  await waitFor(
    (current) => has(current, 'run-optimize'),
    'resume restores the section and its open tab',
  );
  await call('click_element', { id: 'grid' });
  await waitFor((current) => has(current, 'cell-129'), 'the grid tab reopens');

  nodes = await tree();
  check('grid cells are exposed', nodes.filter((node) => /^cell-\d+$/.test(node.id ?? '')).length > 200);
  check('run control is exposed', has(nodes, 'run-optimize') || has(nodes, 'app-tabs'));

  // Select a square through the canvas overlay.
  const cell = 'cell-129';
  check('cell 129 exists', has(nodes, cell));
  await call('click_element', { id: cell });
  await waitFor((current) => has(current, 'square-name'), 'clicking a square selects it');
  nodes = await tree();
  check(
    'inspector names the square',
    (labelOf(nodes, 'square-name') ?? '').includes('J6'),
    String(labelOf(nodes, 'square-name')),
  );
  check('grid marks the selected cell', labelOf(nodes, cell) !== null);

  // Drag a shape by its polygon: pointer down on a covered square, move, release.
  // A disabled Ely button keeps `enabled: true` but carries no click action.
  const canClick = (node) => (node?.actions ?? []).includes('click');
  const shapeAdd = (await tree()).find((node) => node.id === 'shape-add');
  if (canClick(shapeAdd)) {
    await call('click_element', { id: 'shape-add' });
    await new Promise((resolve) => setTimeout(resolve, 400));
  }
  const summary = labelOf(await tree(), 'shape-summary-0');
  const anchor = /^([A-T])(\d+)/.exec(summary ?? '');
  if (anchor) {
    const cell = (12 - Number(anchor[2])) * 20 + (anchor[1].charCodeAt(0) - 65);
    const destination = cell % 20 < 18 ? cell + 2 : cell - 2;
    const from = (await tree()).find((node) => node.id === `cell-${cell}`);
    const to = (await tree()).find((node) => node.id === `cell-${destination}`);
    if (from && to) {
      const [sx, sy] = [from.bounds.x + from.bounds.width / 2, from.bounds.y + from.bounds.height / 2];
      const [tx, ty] = [to.bounds.x + to.bounds.width / 2, to.bounds.y + to.bounds.height / 2];
      await call('pointer_move', { x: sx, y: sy });
      await call('pointer_down', { x: sx, y: sy });
      for (let step = 1; step <= 8; step += 1) {
        await call('pointer_move', {
          x: sx + ((tx - sx) * step) / 8,
          y: sy + ((ty - sy) * step) / 8,
          held_button: 'left',
        });
      }
      await call('pointer_up', { x: tx, y: ty });
      await new Promise((resolve) => setTimeout(resolve, 400));
      const moved = labelOf(await tree(), 'shape-summary-0');
      check('dragging a shape moves its squares', moved !== summary, `${summary} -> ${moved}`);
    }
  }

  // Add a shape from the inspector.
  const before = nodes.filter((node) => /^shape-row/.test(node.id ?? '')).length;
  const canAdd = canClick((await tree()).find((node) => node.id === 'shape-add'));
  if (canAdd) {
    await call('click_element', { id: 'shape-add' });
    await waitFor(
      (current) => current.filter((node) => /^shape-row/.test(node.id ?? '')).length > before,
      'adding a shape lists it',
    );
  } else {
    check('shapes are capped, nothing to add', true);
  }

  // Switch to the Optimize tab and run.
  if (has(await tree(), 'run')) {
    await call('click_element', { id: 'run' });
  }
  await waitFor((current) => has(current, 'run-optimize'), 'optimize pane opens');
  await call('click_element', { id: 'run-optimize' });
  await waitFor((current) => has(current, 'apply-result'), 'optimizing produces a result', 60000);

  nodes = await tree();
  const gain = labelOf(nodes, 'result-gain') ?? '';
  check('result reports a gain', gain.includes('%'), gain);
  check('the optimizer never loses EXP', !gain.trim().startsWith('-'), gain);
  check('result reports the after rate', has(nodes, 'result-after'));

  await call('click_element', { id: 'apply-result' });
  await waitFor(
    (current) => !has(current, 'apply-result'),
    'applying the result clears it',
  );

  // End on the grid so the screenshot shows the optimized layout.
  if (has(await tree(), 'grid')) {
    await call('click_element', { id: 'grid' });
  }
  await waitFor((current) => has(current, 'cell-129'), 'grid tab reopens');

  // Keyboard on the focused grid: arrows move the selection, `+` levels it, Escape drops it.
  await call('click_element', { id: 'cell-129' });
  await waitFor(
    (current) => (labelOf(current, 'square-name') ?? '').includes('J6'),
    'J6 is selected for the keyboard',
  );
  await call('keyboard', { keystroke: 'right' });
  await waitFor(
    (current) => (labelOf(current, 'square-name') ?? '').includes('K6'),
    'the right arrow moves the selection',
  );
  const levelBefore = labelOf(await tree(), 'square-level') ?? '';
  await call('keyboard', { keystroke: '=' });
  await waitFor(
    (current) => (labelOf(current, 'square-level') ?? '') !== levelBefore,
    'plus raises the selected square',
  );
  await call('keyboard', { keystroke: 'escape' });
  await waitFor(
    (current) => !has(current, 'square-name'),
    'Escape clears the selection',
  );

  // Supplements: a stepper press changes the displayed rate immediately.
  if (has(await tree(), 'rates')) {
    await call('click_element', { id: 'rates' });
  }
  await waitFor((current) => has(current, 'supp-abm'), 'supplements card is open');
  nodes = await tree();
  check('every supplement is exposed', ['supp-svrxp', 'supp-abm', 'supp-extra-pct', 'supp-extra-multi', 'supp-points', 'supp-shapes', 'supp-mags'].every((id) => has(nodes, id)));
  const rateBefore = labelOf(nodes, 'kpi-row') ?? '';
  check('the rate readout is exposed', rateBefore.length > 0, rateBefore);
  await call('click_element', { id: 'supp-abm-stepper-more' });
  await waitFor(
    (current) => (labelOf(current, 'kpi-row') ?? '') !== rateBefore,
    'raising the all-bonus multiplier moves the rate',
  );
  await call('click_element', { id: 'supp-reset' });
  await waitFor(
    (current) => (labelOf(current, 'kpi-row') ?? '') === rateBefore,
    'reset restores the supplement',
  );

  // The shared import dialog: the header opens it from any section, the preview
  // names the format, level, characters and warnings, and Apply replaces the run.
  await call('click_element', { id: 'header-import' });
  await waitFor((current) => has(current, 'import-drop'), 'the header opens the import dialog');
  await call('click_element', { id: 'import-input' });
  await call('type_text', { text: harnessSave() });
  await waitFor(
    (current) => labelOf(current, 'import-format') === 'Format: save.json',
    'the preview names the detected format',
  );
  nodes = await tree();
  check('the preview names the research level', labelOf(nodes, 'import-level') === 'Research level: 42');
  check('the preview counts characters', labelOf(nodes, 'import-characters') === 'Characters: 2');
  check('the preview counts warnings', /^Warnings: \d+$/.test(labelOf(nodes, 'import-warnings') ?? ''));
  await call('click_element', { id: 'import-apply' });
  await waitFor(
    (current) =>
      !has(current, 'import-drop') &&
      (labelOf(current, 'status-status') ?? '').includes('research level 42'),
    'applying the save replaces the run',
  );
  check(
    'the header reports the loaded save',
    has(await tree(), 'header-save') && labelOf(await tree(), 'header-save') === 'Save imported',
  );
  const appliedRate = labelOf(await tree(), 'kpi-row') ?? '';
  check('the applied save drives the rate', appliedRate.length > 0, appliedRate);

  // A paste that cannot parse only replaces the message: no Apply, run untouched.
  await call('click_element', { id: 'header-import' });
  await waitFor((current) => has(current, 'import-drop'), 'the dialog reopens');
  await call('click_element', { id: 'import-input' });
  await call('keyboard', { keystroke: 'secondary-a' });
  await call('type_text', { text: '{"not":"a save"}' });
  await waitFor(
    (current) => (labelOf(current, 'import-status') ?? '').includes('no save fields'),
    'an unreadable save reports its error',
  );
  nodes = await tree();
  check('an unreadable save exposes no preview', !has(nodes, 'import-format'));
  const apply = nodes.find((node) => node.id === 'import-apply');
  check('an unreadable save cannot be applied', !(apply?.actions ?? []).includes('click'));
  await call('click_element', { id: 'import-cancel' });
  await waitFor(
    (current) =>
      !has(current, 'import-drop') && labelOf(current, 'kpi-row') === appliedRate,
    'a failed import leaves the run untouched',
  );

  if (has(await tree(), 'grid')) {
    await call('click_element', { id: 'grid' });
  }
  await waitFor((current) => has(current, 'cell-129'), 'the run survives a failed import');

  // macOS stops drawing an occluded window and the native capture gives up after
  // 15s, so a shot may need the window brought forward and one more try.
  const shot = flag('--shot', '/tmp/idleon-drive.png');
  let image = null;
  for (let attempt = 0; attempt < 3 && !image; attempt += 1) {
    try {
      const { content } = await call('screenshot');
      image = content.find((part) => part.type === 'image') ?? null;
    } catch (error) {
      console.error(`screenshot attempt ${attempt + 1}: ${error.message}`);
      activate(appPid);
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }
  if (image) {
    writeFileSync(shot, Buffer.from(image.data, 'base64'));
    console.log(`screenshot: ${shot}`);
  } else {
    // Evidence, not an assertion: a capture an occluded window never delivers
    // must not turn a passing drive into a failed gate.
    console.warn(`no screenshot: the native capture never returned (wanted ${shot})`);
  }
} catch (error) {
  failures += 1;
  console.error(`drive failed: ${error.message}`);
} finally {
  if (!keep) child.kill();
  stopApp();
}

if (failures > 0) {
  console.error(`${failures} check(s) failed`);
  process.exitCode = 1;
} else {
  console.log('all checks passed');
}
