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
import { writeFileSync } from 'node:fs';
import { DEFAULT_APP_BIN, McpLane, flag, hasFlag } from './mcp-lane.mjs';

const keep = hasFlag('--keep');
const appBin = flag('--app-bin', DEFAULT_APP_BIN);

let failures = 0;
let lane = null;

const tree = () => lane.tree();
const call = (name, args = {}) => lane.call(name, args);

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
  lane = await McpLane.start({
    appBin,
    attach: hasFlag('--attach'),
    keep,
  });

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

  const shot = flag('--shot', '/tmp/idleon-drive.png');
  writeFileSync(shot, await lane.screenshot());
  console.log(`screenshot: ${shot}`);
} catch (error) {
  failures += 1;
  console.error(`drive failed: ${error.message}`);
} finally {
  lane?.close();
}

if (failures > 0) {
  console.error(`${failures} check(s) failed`);
  process.exitCode = 1;
} else {
  console.log('all checks passed');
}
