// Frame-time harness: launches a private window with the fixture save, drives
// the UI through gpui-mcp, and reports the draw time of every frame each
// scenario produced (p50/p95/max, frames over 8.3 and 16.7 ms). The budget and
// the baseline it is checked against live in docs/ui-kit/performance.md.
//
// Every scenario is wrapped in `mark_frames` / `get_frame_report`, so only the
// frames the action caused are counted. Measure a release build: a debug build
// is several times slower and says nothing about the budget.
//
// Usage: node script/mcp-perf.mjs [options]
//   --release          launch target/release/idleon-optimizer (default: target/debug)
//   --app-bin PATH     launch this binary instead
//   --only A,B         run just these scenarios (startup always runs): idle,
//                      home-scroll, search, hover, sections, switch, sidebar,
//                      resize, run-away, or any section id (even one not in SECTIONS)
//   --json FILE        write the raw results
//   --compare FILE     print each scenario's delta against a previous --json
//   --check            exit non-zero when a scenario is over budget
//   --keep             leave the window running afterwards
import { execFile, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { cpus, release, tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import { DEFAULT_APP_BIN, McpLane, activate, flag, hasFlag } from './mcp-lane.mjs';

const appBin = flag(
  '--app-bin',
  hasFlag('--release') ? join(process.cwd(), 'target/release/idleon-optimizer') : DEFAULT_APP_BIN,
);
const only = flag('--only', null)?.split(',').map((entry) => entry.trim()).filter(Boolean) ?? null;
const jsonOut = flag('--json', null);
const compareWith = flag('--compare', null);
const enforce = hasFlag('--check');

/// Agreed with the owner on #337; docs/ui-kit/performance.md explains it.
const BUDGET = {
  frame_p95_ms: 8.3, // one vsync at 120 Hz
  frame_max_ms: 16.7, // never two vsyncs at 120 Hz, one at 60 Hz
  idle_frames: 0,
  first_frame_ms: 500,
};

/// Sections opened and scrolled, heaviest first in the baseline.
const SECTIONS = [
  'stats-all',
  'research',
  'spelunking',
  'royal-guardian',
  'jelly-operator',
  'worship-td',
  'combat-sim',
  'farming',
  'killroy',
];

/// Sections whose page scroll is not the largest scroll area on the right of
/// the sidebar: the wheel goes to this element's centre instead.
const SCROLL_TARGET = {
  'worship-td': 'worship-body',
  killroy: 'section-killroy',
  'combat-sim': 'combat-slot-scroll',
  'royal-guardian': 'royal-pane',
  spelunking: 'spelunking-pane',
  sneaking: 'sneaking-pane',
  fountain: 'fountain-pane',
  ladle: 'ladle-ladles-scroll',
  jar: 'jar-pane',
};

/// Cards the pointer crosses on Home.
const HOVER_CARDS = ['combat-sim', 'statue', 'fishing', 'prisma', 'killroy', 'cog', 'worship-td'].map(
  (id) => `card-${id}`,
);

/// `run-away` starts Worship TD's strategy search (about 3.5 s with the
/// fixture), then watches Home and `away` while it runs.
const RUN_AWAY = { section: 'worship-td', tab: 'worship-optimizer-tab', run: 'worship-optimize-run', away: 'killroy' };
/// The views a run elsewhere must not redraw: the place on screen.
const PLACE_VIEWS = ['Page', 'Pane<HomeReads>'];

/// The pair `switch` flips between.
const SWITCH_PAIR = ['research', 'royal-guardian'];

const STARTUP_RUNS = 3;
const RESIZE = { from: 1180, to: 1920, steps: 20, height: 900 };
const HOME_SIZE = { width: 1440, height: 900 };

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
/// A command's trimmed output, or null when it fails.
const read = (cmd, args) => {
  try {
    return execFileSync(cmd, args, { encoding: 'utf8' }).trim();
  } catch {
    return null;
  }
};
const round = (value) => (value == null ? null : Math.round(value * 10) / 10);
const percentile = (values, p) => {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];
};

let lane = null;
const results = [];
const errors = [];
const call = (name, args = {}) => lane.call(name, args);
const find = (nodes, id) => nodes.find((node) => node.id === id);

/// The tree, retrying a response the bridge cut short (a page mid-rebuild can
/// briefly exceed its limit).
const tree = async () => {
  for (let attempt = 1; ; attempt += 1) {
    try {
      return await lane.tree();
    } catch (error) {
      if (attempt >= 5 || !/truncated/.test(error.message)) throw error;
      await sleep(200);
    }
  }
};

const waitFor = async (predicate, label, timeoutMs = 20000) => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const nodes = await tree();
    if (predicate(nodes)) return nodes;
    await sleep(50);
  }
  throw new Error(`timed out waiting for ${label}`);
};
const waitForNode = (id, timeoutMs) => waitFor((nodes) => find(nodes, id), id, timeoutMs);

// --- window control (macOS System Events; the bridge has no resize tool) ---

const osascript = (script) => execFileSync('osascript', ['-e', script], { encoding: 'utf8' }).trim();
const ownWindow = (body) =>
  `tell application "System Events" to tell (first process whose unix id is ${lane.pid}) to ${body}`;
const setWindowSize = (width, height) => osascript(ownWindow(`set size of window 1 to {${width}, ${height}}`));
const windowSize = () => osascript(ownWindow('get size of window 1')).split(',').map((part) => Number(part.trim()));

/// Another lane activating itself occludes this window, and macOS then stops
/// drawing it. Sample the frontmost process while a scenario runs so a result
/// taken without focus is retried instead of reported. Resolves to the pid
/// that held focus instead, or null.
const execFileAsync = promisify(execFile);
const watchFocus = () => {
  let lost = null;
  let running = true;
  const loop = (async () => {
    while (running) {
      try {
        const { stdout } = await execFileAsync('osascript', [
          '-e',
          'tell application "System Events" to get unix id of first application process whose frontmost is true',
        ]);
        const front = Number(stdout.trim());
        if (front !== lane.pid) lost ??= front;
      } catch {
        // A failed sample says nothing about focus.
      }
      await sleep(100);
    }
  })();
  return async () => {
    running = false;
    await loop;
    return lost;
  };
};

// --- measurement ---

const summarize = (report) => {
  const frames = report.frames ?? [];
  const draws = frames.map((frame) => frame.draw_ms);
  const views = new Map();
  for (const view of report.views ?? []) {
    // `a::b::Pane<a::b::HomeReads>` → `Pane<HomeReads>`.
    const type = view.type_name.replace(/\b[a-z_][a-z0-9_]*::/g, '');
    const entry = views.get(type) ?? { type, rendered: 0, reused: 0 };
    entry.rendered += view.rendered;
    entry.reused += view.reused;
    views.set(type, entry);
  }
  return {
    frames: frames.length,
    truncated: Boolean(report.truncated),
    p50_ms: round(percentile(draws, 50)),
    p95_ms: round(percentile(draws, 95)),
    max_ms: round(draws.length ? Math.max(...draws) : null),
    prepaint_p50_ms: round(percentile(frames.map((frame) => frame.prepaint_ms), 50)),
    paint_p50_ms: round(percentile(frames.map((frame) => frame.root_paint_ms), 50)),
    over_8ms: draws.filter((draw) => draw > BUDGET.frame_p95_ms).length,
    over_16ms: draws.filter((draw) => draw > BUDGET.frame_max_ms).length,
    worst: [...frames]
      .sort((a, b) => b.draw_ms - a.draw_ms)
      .slice(0, 3)
      .map((frame) => ({
        draw_ms: round(frame.draw_ms),
        prepaint_ms: round(frame.prepaint_ms),
        paint_ms: round(frame.root_paint_ms),
        views: frame.views_rendered,
      })),
    views: [...views.values()].sort((a, b) => b.rendered - a.rendered).slice(0, 8),
    place_views: [...views.values()].filter((view) => PLACE_VIEWS.includes(view.type)),
  };
};

const overBudget = (result) => {
  const reasons = [];
  if (result.kind === 'idle' && result.frames > BUDGET.idle_frames) {
    reasons.push(`${result.frames} idle frames`);
  }
  if (result.kind === 'interaction') {
    if (result.p95_ms > BUDGET.frame_p95_ms) reasons.push(`p95 ${result.p95_ms} ms`);
    if (result.max_ms > BUDGET.frame_max_ms) reasons.push(`max ${result.max_ms} ms`);
  }
  if (result.kind === 'isolation') {
    for (const view of result.place_views.filter((entry) => entry.rendered)) {
      reasons.push(`${view.type} rendered ${view.rendered}x`);
    }
  }
  if (result.kind === 'startup' && result.first_frame_ms > BUDGET.first_frame_ms) {
    reasons.push(`first frame ${result.first_frame_ms} ms`);
  }
  return reasons;
};

/// Run `setup` (unmeasured), then mark, run `act`, let the UI settle and read
/// every frame drawn since the mark. A run that lost focus is retried twice.
const measure = async (name, { kind = 'interaction', note, setup, act, settle = 600 }) => {
  for (let attempt = 1; ; attempt += 1) {
    if (setup) await setup();
    activate(lane.pid);
    await sleep(300);
    await call('mark_frames');
    const focusLost = watchFocus();
    const started = Date.now();
    const extra = (await act()) ?? {};
    const actMs = Date.now() - started;
    await sleep(settle);
    const lost = await focusLost();
    const report = JSON.parse((await call('get_frame_report', { frame_limit: 512 })).text);
    if (lost && attempt < 3) {
      console.log(`     ${name}: pid ${lost} took focus, retrying`);
      continue;
    }
    const result = {
      name,
      kind,
      note,
      act_ms: actMs,
      attempts: attempt,
      focus_lost: lost !== null,
      ...summarize(report),
      ...extra,
    };
    result.over_budget = overBudget(result);
    results.push(result);
    printRow(result);
    return result;
  }
};

const printRow = (result) => {
  const cell = (value, width) => String(value ?? '–').padStart(width);
  const verdict = result.over_budget.length ? `over: ${result.over_budget.join(', ')}` : 'ok';
  const focus = result.focus_lost ? ' (focus lost)' : '';
  console.log(
    `${result.name.padEnd(26)}${cell(result.frames, 7)}${cell(result.p50_ms, 8)}${cell(result.p95_ms, 8)}` +
      `${cell(result.max_ms, 8)}${cell(result.over_16ms, 6)}  ${verdict}${focus}`,
  );
};

// --- navigation helpers ---

const onHome = (nodes) => find(nodes, 'dashboard-search') && !find(nodes, 'restoring');

const goHome = async () => {
  if (!onHome(await tree())) {
    await revealNav('nav-home');
    await call('click_element', { id: 'nav-home' });
  }
  await waitFor(onHome, 'Home');
  await sleep(300);
};

/// Scroll the sidebar until `nav-<id>` is inside it, so the measured click is
/// only the navigation.
const revealNav = async (id) => {
  let previous = null;
  for (let attempt = 0; attempt < 10; attempt += 1) {
    const nodes = await tree();
    const node = find(nodes, id);
    if (!node) throw new Error(`${id} is not in the sidebar`);
    const frame = find(nodes, 'app-sidebar').bounds;
    const { y, height } = node.bounds;
    if ((y >= frame.y && y + height <= frame.y + frame.height) || y === previous) return;
    previous = y;
    await call('scroll', {
      x: frame.x + frame.width / 2,
      y: frame.y + frame.height / 2,
      delta_y: Math.ceil(y - (frame.y + frame.height / 2)),
    });
    await sleep(200);
  }
  throw new Error(`${id} did not scroll into the sidebar`);
};

const openSection = async (id) => {
  await call('click_element', { id: `nav-${id}` });
  await waitForNode(`section-${id}`);
};

/// Where the wheel goes for a section: the named target's centre, or the right
/// edge of the largest scroll area in the workspace (the page's own scroll).
const scrollPoint = (nodes, id) => {
  const named = SCROLL_TARGET[id] && find(nodes, SCROLL_TARGET[id]);
  if (named) {
    const { x, y, width, height } = named.bounds;
    return { target: named.id, x: x + width / 2, y: y + Math.min(300, height / 2) };
  }
  const sidebar = find(nodes, 'app-sidebar')?.bounds;
  const left = sidebar ? sidebar.x + sidebar.width : 0;
  const page = nodes
    .filter((node) => /scroll/.test(node.id ?? '') && node.bounds && node.bounds.x >= left && node.bounds.height > 200)
    .sort((a, b) => b.bounds.width * b.bounds.height - a.bounds.width * a.bounds.height)[0];
  if (!page) return null;
  const { x, y, width, height } = page.bounds;
  return { target: page.id, x: x + width - 6, y: y + Math.min(300, height / 2) };
};

/// 15 wheel events of 80 px down, then 15 back up.
const wheelBurst = async ({ x, y }) => {
  for (const delta of [80, -80]) {
    for (let step = 0; step < 15; step += 1) await call('scroll', { x, y, delta_y: delta });
  }
};

// --- scenarios ---

/// Stop the lane's app and wait until it has exited.
const stopLane = async () => {
  const app = lane.appProcess;
  const exited = app ? once(app, 'exit') : null;
  lane.close();
  await exited;
};

/// Spawn → first frame and spawn → Home with the decoded session, for one
/// launch that restores `stateDir`. The first frame is read once the bridge
/// answers, so it is an upper bound when the window drew before that.
const launch = async (stateDir, keep) => {
  lane = await McpLane.start({ appBin, stateDir, keep });
  const since = () => round(performance.now() - lane.spawnedAt);
  const run = { discovered_ms: since(), first_frame_ms: null, session_ms: null };
  const deadline = Date.now() + 60000;
  while (run.session_ms === null && Date.now() < deadline) {
    if (run.first_frame_ms === null) {
      const report = JSON.parse((await call('get_frame_report', { since_frame_count: 0, frame_limit: 1 })).text);
      if ((report.frames ?? []).length) run.first_frame_ms = since();
    } else if (onHome(await tree())) {
      run.session_ms = since();
    }
    if (run.session_ms === null) await sleep(10);
  }
  if (run.session_ms === null) throw new Error('Home never replaced the loader');
  return run;
};

/// A user's launch restores the last session off the main thread behind the
/// loader. A first launch imports the fixture and saves that session; three
/// more restore it, and the median is reported. The last one stays up for the
/// other scenarios.
const startup = async () => {
  const stateDir = mkdtempSync(join(tmpdir(), 'idleon-perf-'));
  lane = await McpLane.start({ appBin, stateDir, importFile: join(process.cwd(), 'fixtures/data.json') });
  await waitFor(onHome, 'Home with the imported fixture', 60000);
  const saved = Date.now() + 10000;
  while (!readdirSync(stateDir).length && Date.now() < saved) await sleep(50);
  if (!readdirSync(stateDir).length) throw new Error('the import never saved a session to restore');
  await stopLane();

  const runs = [];
  for (let attempt = 0; attempt < STARTUP_RUNS; attempt += 1) {
    if (attempt) await stopLane();
    runs.push(await launch(stateDir, hasFlag('--keep')));
  }
  const median = (field) => percentile(runs.map((run) => run[field]), 50);
  const report = JSON.parse((await call('get_frame_report', { since_frame_count: 0, frame_limit: 512 })).text);
  const result = {
    name: 'startup',
    kind: 'startup',
    note: `median of ${STARTUP_RUNS} launches restoring a session that holds fixtures/data.json; frames of the last`,
    discovered_ms: median('discovered_ms'),
    first_frame_ms: median('first_frame_ms'),
    session_ms: median('session_ms'),
    runs,
    ...summarize(report),
  };
  result.over_budget = overBudget(result);
  results.push(result);
  console.log(
    `startup: bridge ${result.discovered_ms} ms, first frame ≤ ${result.first_frame_ms} ms, ` +
      `Home with the session ${result.session_ms} ms (median of ${STARTUP_RUNS})`,
  );
  printRow(result);
  activate(lane.pid);
  setWindowSize(HOME_SIZE.width, HOME_SIZE.height);
};

const idle = () =>
  measure('home-idle-3s', {
    kind: 'idle',
    note: 'Home, no input for 3 s',
    setup: async () => {
      await goHome();
      await call('pointer_move', { x: 4, y: 4 });
    },
    act: () => sleep(3000),
    settle: 0,
  });

/// Escape in the search field clears it and brings every card back.
const clearSearch = async () => {
  await call('click_element', { id: 'dashboard-search' });
  await call('keyboard', { keystroke: 'escape' });
  await waitForNode('card-combat-sim');
};

const search = async () => {
  const query = 'research';
  await measure('home-search-typing', {
    note: `focus the search field and type "${query}" one key at a time`,
    setup: async () => {
      await goHome();
      await clearSearch();
    },
    act: async () => {
      await call('click_element', { id: 'dashboard-search' });
      for (const key of query) await call('type_text', { text: key });
    },
  });
  await clearSearch();
};

const hover = () =>
  measure('home-hover', {
    note: 'pointer across 7 dashboard cards (hover_element is not supported on cards)',
    setup: goHome,
    act: async () => {
      const nodes = await tree();
      for (const id of HOVER_CARDS) {
        const bounds = find(nodes, id)?.bounds;
        if (bounds) await call('pointer_move', { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 });
      }
      await call('pointer_move', { x: 4, y: 4 });
    },
  });

const homeScroll = async () => {
  await goHome();
  const bounds = find(await tree(), 'dashboard-scroll').bounds;
  await measure('home-scroll', {
    note: '30 wheel events on dashboard-scroll',
    act: () => wheelBurst({ x: bounds.x + bounds.width - 6, y: bounds.y + 300 }),
  });
};

const section = async (id) => {
  await measure(`${id}-open`, {
    note: 'click the sidebar item until the section root shows, 1.5 s settle',
    setup: async () => {
      await goHome();
      await revealNav(`nav-${id}`);
    },
    act: async () => {
      const started = Date.now();
      await openSection(id);
      return { open_ms: Date.now() - started };
    },
    settle: 1500,
  });
  const point = scrollPoint(await tree(), id);
  if (!point) throw new Error(`${id}: no scroll area found`);
  await measure(`${id}-scroll`, {
    note: `30 wheel events on ${point.target}`,
    act: async () => {
      await wheelBurst(point);
      return { scroll_target: point.target };
    },
  });
};

const switchSections = async () => {
  const [a, b] = SWITCH_PAIR;
  await measure('switch-10x', {
    note: `${a} → ${b} → ${a}, 10 times, through the sidebar`,
    setup: async () => {
      await goHome();
      await revealNav(`nav-${a}`);
      await openSection(a);
      await revealNav(`nav-${b}`);
      await revealNav(`nav-${a}`);
    },
    act: async () => {
      for (let trip = 0; trip < 10; trip += 1) {
        await openSection(b);
        await openSection(a);
      }
    },
  });
};

/// Fold or unfold the sidebar, and wait for the toggle to say so. The toggle's
/// label names what the next click does.
const setFolded = async (folded) => {
  const done = (nodes) => find(nodes, 'sidebar-toggle')?.label === (folded ? 'Expand sidebar' : 'Collapse sidebar');
  if (!done(await tree())) await call('click_element', { id: 'sidebar-toggle' });
  await waitFor(done, folded ? 'the folded sidebar' : 'the expanded sidebar');
};

const sidebar = async () => {
  for (const folded of [true, false]) {
    await measure(folded ? 'sidebar-fold' : 'sidebar-unfold', {
      note: `Home: ${folded ? 'collapse to' : 'expand from'} the rail, 1 s settle`,
      setup: async () => {
        await goHome();
        await setFolded(!folded);
        await sleep(700);
      },
      act: () => setFolded(folded),
      settle: 1000,
    });
  }
};

const resize = async () => {
  const { from, to, steps, height } = RESIZE;
  const widths = Array.from({ length: steps + 1 }, (_, step) => Math.round(from + ((to - from) * step) / steps));
  await measure('window-resize', {
    note: `Home: ${steps} width steps ${from} → ${to} px and back, height ${height}`,
    setup: async () => {
      await goHome();
      // Start at the screen's left edge so the sweep has the most room.
      osascript(ownWindow('set position of window 1 to {0, 40}'));
      setWindowSize(from, height);
      await sleep(300);
    },
    act: async () => {
      let widest = 0;
      for (const width of [...widths, ...widths.slice(0, -1).reverse()]) {
        setWindowSize(width, height);
        widest = Math.max(widest, windowSize()[0]);
      }
      // macOS clamps a window to its screen; say how far the sweep really got.
      return { widest_px: widest };
    },
  });
  setWindowSize(HOME_SIZE.width, HOME_SIZE.height);
};

/// While a long run goes on in one section, Home or another section stays
/// still: only the sidebar's progress ring and the status line redraw (#363).
const runAway = async () => {
  const { section, tab, run, away: other } = RUN_AWAY;
  const ring = `nav-${section}-activity`;
  const running = (nodes) => /Running|runs/.test(find(nodes, ring)?.label ?? '');
  for (const away of [null, other]) {
    const result = await measure(`run-away-${away ?? 'home'}`, {
      kind: 'isolation',
      note: `${section} run in flight while ${away ?? 'Home'} is open; the place must not redraw`,
      setup: async () => {
        await waitFor((nodes) => !running(nodes), 'the previous run to end', 60000);
        await goHome();
        await revealNav(`nav-${section}`);
        await openSection(section);
        await call('click_element', { id: tab });
        await waitForNode(run);
        await call('click_element', { id: run });
        await waitFor(running, `the ${section} run to start`);
        if (away) {
          await revealNav(`nav-${away}`);
          await openSection(away);
        } else {
          await goHome();
        }
      },
      act: () => sleep(1000),
      settle: 0,
    });
    // A run that ended before the mark proves nothing.
    if (!result.views.some((view) => view.type === 'Pane<SidebarReads>' && view.rendered)) {
      throw new Error(`${result.name}: the run ended before the measurement`);
    }
  }
};

// --- run ---

const GROUPS = {
  idle,
  search,
  hover,
  'home-scroll': homeScroll,
  switch: switchSections,
  sidebar,
  resize,
  'run-away': runAway,
};
const wanted = (name) => !only || only.includes(name);
const sectionIds = only
  ? [
      ...new Set([
        ...(only.includes('sections') ? SECTIONS : []),
        ...only.filter((name) => !GROUPS[name] && name !== 'sections'),
      ]),
    ]
  : SECTIONS;

const run = async (label, scenario) => {
  try {
    await scenario();
  } catch (error) {
    console.log(`FAIL ${label}: ${error.message}`);
    errors.push({ scenario: label, error: error.message });
  }
};

const compare = (baseline) => {
  const before = new Map((baseline.results ?? []).map((result) => [result.name, result]));
  const delta = (now, then) => (now == null || then == null ? '' : ` (${now - then >= 0 ? '+' : ''}${round(now - then)})`);
  console.log(`\nAgainst ${compareWith} (${baseline.commit ?? 'unknown commit'}):`);
  for (const result of results) {
    const then = before.get(result.name);
    if (!then) {
      console.log(`${result.name.padEnd(26)} not in the baseline`);
      continue;
    }
    const fields = result.kind === 'startup'
      ? ['first_frame_ms', 'session_ms', 'frames']
      : ['frames', 'p50_ms', 'p95_ms', 'max_ms'];
    const cells = fields.map((field) => `${field} ${result[field]}${delta(result[field], then[field])}`);
    console.log(`${result.name.padEnd(26)} ${cells.join('  ')}`);
  }
};


const header = [['frames', 7], ['p50', 8], ['p95', 8], ['max', 8], ['>16.7', 6]];
console.log(`${'scenario'.padEnd(26)}${header.map(([title, width]) => title.padStart(width)).join('')}`);
try {
  await startup();
  if (wanted('idle')) await run('idle', idle);
  if (wanted('home-scroll')) await run('home-scroll', homeScroll);
  if (wanted('search')) await run('search', search);
  if (wanted('hover')) await run('hover', hover);
  for (const id of sectionIds) await run(id, () => section(id));
  if (wanted('switch')) await run('switch', switchSections);
  if (wanted('sidebar')) await run('sidebar', sidebar);
  if (wanted('resize')) await run('resize', resize);
  if (wanted('run-away')) await run('run-away', runAway);
} catch (error) {
  console.log(`FAIL startup: ${error.message}`);
  errors.push({ scenario: 'startup', error: error.message });
} finally {
  lane?.close();
}

const over = results.filter((result) => result.over_budget.length);
if (jsonOut) {
  const report = {
    date: new Date().toISOString().slice(0, 10),
    commit: read('git', ['rev-parse', '--short', 'HEAD']),
    binary: appBin.replace(`${process.cwd()}/`, ''),
    machine: {
      cpu: cpus()[0]?.model ?? null,
      os:
        process.platform === 'darwin'
          ? `macOS ${read('sw_vers', ['-productVersion'])}`
          : `${process.platform} ${release()}`,
    },
    budget: BUDGET,
    results,
    errors,
  };
  writeFileSync(jsonOut, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`\nwrote ${jsonOut}`);
}
if (compareWith) compare(JSON.parse(readFileSync(compareWith, 'utf8')));
console.log(`\n${results.length} scenarios, ${over.length} over budget, ${errors.length} failed`);
if (errors.length || (enforce && over.length)) process.exitCode = 1;
