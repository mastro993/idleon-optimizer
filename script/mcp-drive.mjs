// Drives the running app through gpui-mcp and asserts the flows that matter:
// selecting a square, adding a shape, running the optimizer, applying a result.
//
// Usage: node script/mcp-drive.mjs [--keep]
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';

const keep = process.argv.includes('--keep');
const child = spawn('gpui-mcp', [], { stdio: ['pipe', 'pipe', 'pipe'] });
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

const call = async (name, args = {}) => {
  const result = await request('tools/call', { name, arguments: args });
  const text = result?.content?.find((part) => part.type === 'text')?.text;
  if (result?.isError) throw new Error(`${name}: ${text}`);
  return { text, content: result?.content ?? [] };
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

  const apps = (await call('list_apps')).text;
  const running = JSON.parse(apps);
  const app = (Array.isArray(running) ? running : running.apps ?? []).find(
    (candidate) => (candidate.app_id ?? candidate.id) === 'idleon-optimizer',
  );
  if (!app) throw new Error('the app is not running; start ./target/debug/idleon-optimizer first');

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

  // Add a shape from the inspector.
  const before = nodes.filter((node) => /^shape-row/.test(node.id ?? '')).length;
  await call('click_element', { id: 'shape-add' });
  await waitFor(
    (current) => current.filter((node) => /^shape-row/.test(node.id ?? '')).length > before,
    'adding a shape lists it',
  );

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

  const { content } = await call('screenshot');
  const image = content.find((part) => part.type === 'image');
  if (image) {
    writeFileSync('/tmp/idleon-drive.png', Buffer.from(image.data, 'base64'));
    console.log('screenshot: /tmp/idleon-drive.png');
  }
} catch (error) {
  failures += 1;
  console.error(`drive failed: ${error.message}`);
} finally {
  if (!keep) child.kill();
}

if (failures > 0) {
  console.error(`${failures} check(s) failed`);
  process.exitCode = 1;
} else {
  console.log('all checks passed');
}
