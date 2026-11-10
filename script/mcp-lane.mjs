// Shared GPUI-MCP lane: one private app window and one stdio bridge.
//
// Every harness that needs its own window goes through `McpLane.start`: the app
// takes a private endpoint directory, the server is scoped to the same one, and
// concurrent lanes cannot see or drive each other. `--attach` drops the private
// directory and talks to a window started by hand.
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export const DEFAULT_APP_ID = 'idleon-optimizer';
export const DEFAULT_APP_BIN = join(process.cwd(), 'target/debug/idleon-optimizer');

const argv = process.argv.slice(2);

/// The value of `--flag value`.
export const flag = (name, fallback, args = argv) => {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : fallback;
};

/// Whether `--flag` is present.
export const hasFlag = (name, args = argv) => args.includes(name);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/// macOS stops drawing an occluded window, and the bridge waits for frames, so
/// bring this instance forward before driving it.
export const activate = (pid) => {
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

export class McpLane {
  /// Launch a private app window (unless attached) and connect a bridge to it.
  static async start({
    appBin = DEFAULT_APP_BIN,
    appId = DEFAULT_APP_ID,
    attach = false,
    keep = false,
    section = null,
    save = null,
    serverBin = 'gpui-mcp',
  } = {}) {
    let endpointDir = null;
    let appProcess = null;
    if (!attach) {
      if (!existsSync(appBin)) {
        throw new Error(`no app binary at ${appBin}; run cargo build -p idleon-optimizer first`);
      }
      endpointDir = mkdtempSync(join(tmpdir(), 'idleon-lane-'));
      const appArgs = ['--endpoint-dir', endpointDir];
      if (section) appArgs.push('--section', section);
      if (save) appArgs.push('--save', save);
      appProcess = spawn(appBin, appArgs, { stdio: 'ignore' });
      const deadline = Date.now() + 30000;
      let published = false;
      while (Date.now() < deadline && !published) {
        published = readdirSync(endpointDir).some((name) => name.endsWith('.json'));
        if (!published) await sleep(200);
      }
      if (!published) {
        appProcess.kill();
        throw new Error('the app never published an endpoint; is it crashing at startup?');
      }
    }
    const child = spawn(serverBin, endpointDir ? ['--endpoint-dir', endpointDir] : [], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    const lane = new McpLane({ appId, endpointDir, appProcess, child, keep });
    lane.#wire();
    try {
      await lane.#initialize();
      lane.app = await lane.#discover();
      lane.pid = appProcess?.pid ?? lane.app.pid;
      activate(lane.pid);
      return lane;
    } catch (error) {
      lane.close();
      throw error;
    }
  }

  constructor({ appId, endpointDir, appProcess, child, keep }) {
    this.appId = appId;
    this.endpointDir = endpointDir;
    this.appProcess = appProcess;
    this.child = child;
    this.keep = keep;
    this.app = null;
    this.pid = null;
    this.nextId = 1;
    this.buffer = '';
    this.pending = new Map();
  }

  #wire() {
    this.child.stdout.on('data', (chunk) => {
      this.buffer += chunk.toString();
      let index;
      while ((index = this.buffer.indexOf('\n')) >= 0) {
        const line = this.buffer.slice(0, index).trim();
        this.buffer = this.buffer.slice(index + 1);
        if (!line) continue;
        const message = JSON.parse(line);
        const entry = this.pending.get(message.id);
        if (!entry) continue;
        this.pending.delete(message.id);
        if (message.error) entry.reject(new Error(JSON.stringify(message.error)));
        else entry.resolve(message.result);
      }
    });
    this.child.stderr.on('data', (chunk) => process.stderr.write(chunk));
    this.child.on('error', (error) => {
      for (const entry of this.pending.values()) entry.reject(error);
      this.pending.clear();
    });
    this.child.on('exit', (code) => {
      const error = new Error(`gpui-mcp exited with code ${code}`);
      for (const entry of this.pending.values()) entry.reject(error);
      this.pending.clear();
    });
  }

  async #initialize() {
    await this.request('initialize', {
      protocolVersion: '2025-06-18',
      capabilities: {},
      clientInfo: { name: 'idleon-harness', version: '0' },
    });
    this.notify('notifications/initialized', {});
  }

  /// The endpoint file lands before the bridge answers, so poll for discovery.
  async #discover() {
    let entry = [];
    for (let attempt = 0; attempt < 40; attempt += 1) {
      const running = JSON.parse((await this.call('list_apps')).text);
      entry = Array.isArray(running) ? running : (running.apps ?? []);
      const app = entry.find(
        (candidate) => (candidate.app_id ?? candidate.appId ?? candidate.id) === this.appId,
      );
      if (app) return app;
      await sleep(250);
    }
    const where = this.endpointDir
      ? `endpoint dir ${this.endpointDir}: ${JSON.stringify(readdirSync(this.endpointDir))}`
      : 'shared discovery directory';
    throw new Error(`no running app ${this.appId}; got ${JSON.stringify(entry)} (${where})`);
  }

  request(method, params) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
    });
  }

  notify(method, params) {
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method, params })}\n`);
  }

  /// Call a tool, retrying once after reactivating an occluded window. Returns
  /// the text and structured content so callers can read images as well.
  async call(name, args = {}) {
    for (let attempt = 1; ; attempt += 1) {
      const result = await this.request('tools/call', { name, arguments: args });
      const text = result?.content?.find((part) => part.type === 'text')?.text;
      if (result?.isError && /frame wait timed out/i.test(text ?? '') && attempt === 1) {
        activate(this.pid);
        await sleep(250);
        continue;
      }
      if (result?.isError) throw new Error(`${name}: ${text}`);
      return { text, content: result?.content ?? [] };
    }
  }

  /// The current semantic tree as a flat node list.
  async tree() {
    const { text } = await this.call('get_ui_tree');
    const parsed = JSON.parse(text);
    return Object.values(parsed.nodes ?? {});
  }

  /// A PNG capture of the whole window or one element. Native capture stalls
  /// when macOS stops drawing an occluded window, so bring the app forward and
  /// retry a deadline twice.
  async screenshot(elementId = null) {
    activate(this.pid);
    await sleep(120);
    for (let attempt = 1; ; attempt += 1) {
      try {
        const { content } = elementId
          ? await this.call('screenshot_element', { id: elementId })
          : await this.call('screenshot');
        const image = content.find((part) => part.type === 'image');
        if (!image) throw new Error(`screenshot returned ${JSON.stringify(content).slice(0, 200)}`);
        return Buffer.from(image.data, 'base64');
      } catch (error) {
        if (attempt > 2 || !/(deadline|frame wait timed out)/i.test(error.message)) throw error;
        activate(this.pid);
        await sleep(500);
      }
    }
  }

  /// Stop the bridge, and the app it started unless `--keep` asks to leave it up.
  close() {
    if (!this.child.killed) this.child.kill();
    if (this.appProcess && !this.keep) {
      this.appProcess.kill();
      this.appProcess = null;
    }
  }
}
