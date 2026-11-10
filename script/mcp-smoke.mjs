// Minimal MCP stdio client for gpui-mcp: lists running GPUI apps, prints the
// UI tree of ours, and optionally screenshots it. Keeps verification working
// outside an editor session.
//
// Usage: node script/mcp-smoke.mjs [--app idleon-optimizer] [--shot out.png]
//          [--spawn] [--keep] [--app-bin PATH] [--section ID]
//   --spawn    launch a private instance (own endpoint directory) and stop it after
//   --section  start the app on a registry id (or an unknown one to see the empty state)
import { writeFileSync } from 'node:fs';
import { DEFAULT_APP_BIN, DEFAULT_APP_ID, McpLane, flag, hasFlag } from './mcp-lane.mjs';

const appId = flag('--app', DEFAULT_APP_ID);
const shot = flag('--shot', null);
const section = flag('--section', null);
const keep = hasFlag('--keep');
const appBin = flag('--app-bin', DEFAULT_APP_BIN);

let lane = null;
try {
  lane = await McpLane.start({
    appBin,
    appId,
    attach: !hasFlag('--spawn'),
    keep,
    section,
  });

  const catalog = await lane.request('tools/list', {});
  console.log(`tools: ${catalog.tools.length}`);
  console.log(`app: ${JSON.stringify(lane.app)}`);

  // The endpoint answers before the first frame; wait for a rendered tree.
  let nodes = [];
  for (let attempt = 0; attempt < 50 && nodes.length === 0; attempt += 1) {
    nodes = await lane.tree();
    if (nodes.length === 0) await new Promise((resolve) => setTimeout(resolve, 200));
  }
  console.log(`nodes: ${nodes.length}`);
  for (const node of nodes.slice(0, 40)) {
    console.log(` - ${node.id ?? '?'} ${node.role ?? ''} ${JSON.stringify(node.label ?? node.text ?? '')}`);
  }

  // `--section` proves the route: a known id lands on its own section root, an
  // unknown one on the missing state, and the latter always renders an empty state.
  if (section) {
    const known = nodes.some((node) => node.id === `section-${section}`);
    const root = nodes.find(
      (node) => node.id === (known ? `section-${section}` : 'section-unknown'),
    );
    if (!root) {
      throw new Error(
        `no section root for ${section}; got ${nodes.map((node) => node.id).slice(0, 12).join(', ')}`,
      );
    }
    const empty = nodes.find(
      (node) => node.id === 'section-back' || node.id === 'section-missing-back',
    );
    if (!known && !empty) throw new Error(`section ${section} rendered no empty state`);
    // Every known section renders the shared no-save state until a save loads.
    const saveLabel = nodes.find((node) => node.id === 'header-save')?.label ?? '';
    if (known && saveLabel === 'No save' && !nodes.some((node) => node.id === 'no-save-state')) {
      throw new Error(`section ${section} rendered no no-save state`);
    }
    console.log(`section: ${root.id}${empty ? ` (${empty.id})` : ''}`);
  }

  if (shot) {
    writeFileSync(shot, await lane.screenshot());
    console.log(`screenshot: ${shot}`);
  }
} catch (error) {
  console.error(`smoke failed: ${error.message}`);
  process.exitCode = 1;
} finally {
  lane?.close();
}
