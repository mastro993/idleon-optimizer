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

  // Start on the grid tab whatever the last run left behind.
  if (has(await tree(), 'grid')) {
    await call('click_element', { id: 'grid' });
  }
  await waitFor((current) => has(current, 'cell-129'), 'grid tab opens');

  let nodes = await tree();
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

  // Save import: a paste that cannot parse reports an error and keeps the run.
  // Idempotent: the panel may still be open from an earlier run.
  if (!has(await tree(), 'save-import') && has(await tree(), 'save-toggle')) {
    await call('click_element', { id: 'save-toggle' });
  }
  await waitFor((current) => has(current, 'save-import'), 'save panel opens');
  await call('click_element', { id: 'save-input' });
  await call('type_text', { text: '{"not":"a save"}' });
  await call('click_element', { id: 'save-import' });
  await waitFor(
    (current) => {
      const status = labelOf(current, 'save-status') ?? '';
      return status.length > 0 && !status.includes('Nothing imported yet');
    },
    'an unreadable save reports its error',
  );

  if (has(await tree(), 'grid')) {
    await call('click_element', { id: 'grid' });
  }
  await waitFor((current) => has(current, 'cell-129'), 'the run survives a failed import');

  const { content } = await call('screenshot');
  const image = content.find((part) => part.type === 'image');
  if (image) {
    const shot = flag('--shot', '/tmp/idleon-drive.png');
    writeFileSync(shot, Buffer.from(image.data, 'base64'));
    console.log(`screenshot: ${shot}`);
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
