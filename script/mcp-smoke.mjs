// Minimal MCP stdio client for gpui-mcp: lists running GPUI apps, prints the
// UI tree of ours, and optionally screenshots it. Keeps verification working
// outside an editor session.
//
// Usage: node script/mcp-smoke.mjs [--app idleon-optimizer] [--shot out.png]
//          [--spawn] [--keep] [--app-bin PATH]
//   --spawn  launch a private instance (own endpoint directory) and stop it after
import { spawn, execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const args = process.argv.slice(2);
const flag = (name, fallback) => {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : fallback;
};
const appId = flag('--app', 'idleon-optimizer');

/// macOS stops drawing an occluded window, and the bridge waits for frames, so
/// bring the app forward before driving it.
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

const shot = flag('--shot', null);
const keep = args.includes('--keep');
const spawnOwn = args.includes('--spawn');
const appBin = flag('--app-bin', join(process.cwd(), 'target/debug/idleon-optimizer'));

// A private endpoint directory keeps this instance invisible to every other
// lane's server, so concurrent runs cannot drive each other's window.
let endpointDir = null;
let appProcess = null;
if (spawnOwn) {
  if (!existsSync(appBin)) {
    console.error(`no app binary at ${appBin}; run cargo build -p idleon-optimizer first`);
    process.exit(1);
  }
  endpointDir = mkdtempSync(join(tmpdir(), 'idleon-smoke-'));
  appProcess = spawn(appBin, ['--endpoint-dir', endpointDir], { stdio: 'ignore' });
  const deadline = Date.now() + 30000;
  let published = false;
  while (Date.now() < deadline && !published) {
    published = readdirSync(endpointDir).some((name) => name.endsWith('.json'));
    if (!published) execFileSync('sleep', ['0.2']);
  }
  if (!published) {
    console.error('the app never published an endpoint');
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

function request(method, params) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, (message) => {
      if (message.error) reject(new Error(JSON.stringify(message.error)));
      else resolve(message.result);
    });
    child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
  });
}

function notify(method, params) {
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method, params })}\n`);
}

const callRaw = async (name, params = {}) => {
  const result = await request('tools/call', { name, arguments: params });
  const text = result?.content?.find((part) => part.type === 'text')?.text;
  if (result?.isError) throw new Error(`${name}: ${text}`);
  return { text, content: result?.content ?? [] };
};

const call = async (name, params = {}) => (await callRaw(name, params)).text;

try {
  await request('initialize', {
    protocolVersion: '2025-06-18',
    capabilities: {},
    clientInfo: { name: 'smoke', version: '0' },
  });
  notify('notifications/initialized', {});

  const catalog = await request('tools/list', {});
  console.log(`tools: ${catalog.tools.length}`);

  // The endpoint file lands before the bridge answers, so poll for discovery.
  let entry = [];
  let app = null;
  for (let attempt = 0; attempt < 40 && !app; attempt += 1) {
    const apps = JSON.parse(await call('list_apps'));
    entry = Array.isArray(apps) ? apps : (apps.apps ?? []);
    app = entry.find((candidate) => (candidate.app_id ?? candidate.appId ?? candidate.id) === appId);
    if (!app) execFileSync('sleep', ['0.25']);
  }
  if (!app) {
    console.error(`no running app ${appId}; got ${JSON.stringify(entry)}`);
    if (endpointDir) console.error(`endpoint dir ${endpointDir}: ${JSON.stringify(readdirSync(endpointDir))}`);
    process.exit(1);
  }
  activate(app.pid);
  console.log(`app: ${JSON.stringify(app)}`);

  const tree = JSON.parse(await call('get_ui_tree'));
  const nodes = Array.isArray(tree.nodes) ? tree.nodes : Object.values(tree.nodes ?? {});
  console.log(`nodes: ${nodes.length}`);
  for (const node of nodes.slice(0, 40)) {
    console.log(` - ${node.id ?? '?'} ${node.role ?? ''} ${JSON.stringify(node.label ?? node.text ?? '')}`);
  }
  if (tree.diagnostics?.length) console.log(`diagnostics: ${JSON.stringify(tree.diagnostics)}`);

  if (shot) {
    const { content } = await callRaw('screenshot', {});
    const image = content.find((part) => part.type === 'image');
    if (!image) throw new Error(`screenshot returned ${JSON.stringify(content).slice(0, 200)}`);
    writeFileSync(shot, Buffer.from(image.data, 'base64'));
    console.log(`screenshot: ${shot}`);
  }
} catch (error) {
  console.error(`smoke failed: ${error.message}`);
  process.exitCode = 1;
} finally {
  child.kill();
  stopApp();
}
