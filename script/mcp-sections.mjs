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
/// A small flat save with a wrapped Killroy rotation, two eligible characters
/// and every upgrade unlocked, so the comparison has rows to run.
const killroySave = () => {
  const sparse = (length, filled = {}) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const portals = () => sparse(251, { 100: [0], 150: [0], 200: [0], 250: [0] });
  return JSON.stringify({
    Research: '[]',
    Lv0_0: JSON.stringify(sparse(30, { 20: 42 })),
    Lv0_1: JSON.stringify(sparse(30, { 20: 30 })),
    CharacterClass_0: 10,
    CharacterClass_1: 31,
    TimeAway: JSON.stringify({ GlobalTime: 1794296810.952, ShopRestock: 62019.836 }),
    OptLacc: sparse(228, {
      39: 5, 106: 155, 107: 11, 108: 281, 109: 28, 110: 1, 111: 2, 112: 400, 113: 0, 114: 137, 227: 1,
    }),
    Dream: JSON.stringify(sparse(24, { 4: 3 })),
    KLA_0: portals(),
    KLA_1: portals(),
    Summon: sparse(4, { 3: sparse(3, { 2: 5 }) }),
    KRbest: { ratB: 2240, thermostat: 2356 },
    serverVars: { KillroySwap: 7 },
  });
};

const jarSave = () => {
  const row = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const holes = Array.from({ length: 35 }, () => []);
  holes[9] = row(21, { 20: 500 });
  holes[11] = row(38, { 37: 2 });
  holes[25] = [1, 0, 0, 250];
  return JSON.stringify({ Research: '[]', Holes: JSON.stringify(holes) });
};

/// A small flat `save.json` with three characters, one of them away from Cooking,
/// a Prayer 2 charge and a Post Office 19c roll, so the ladle rates have inputs.
const ladleSave = () => {
  const row = (index, value) => {
    const cells = Array(30).fill(0);
    cells[index] = value;
    return cells;
  };
  const postOffice = Array(36).fill(0);
  postOffice[19] = 400;
  return JSON.stringify({
    Research: '[]',
    charNames: ['Ana', 'Bo', 'Cy'],
    Lv0_0: row(10, 42),
    Lv0_1: row(10, 30),
    Lv0_2: row(10, 60),
    Exp0_0: row(10, 123456789),
    Exp0_1: row(10, 0),
    Exp0_2: row(10, 5e9),
    AFKtarget_0: 'Cooking',
    AFKtarget_1: 'Fighting',
    AFKtarget_2: 'Cooking',
    PTimeAway_0: 986.4,
    TimeAway: JSON.stringify({ GlobalTime: 1000000, ShopRestock: 27000, Player: 986400 }),
    POu_0: postOffice,
    POu_1: postOffice,
    POu_2: postOffice,
    PrayOwned: [0, 0, 25],
    Prayers_2: [2],
    ChestOrder: ['Ladle'],
    ChestQuantity: [120],
    InventoryOrder_0: ['ExpBalloon1'],
    ItemQTY_0: [7],
  });
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

/// A small flat `save.json` with a levelled warrior, an equipped helmet and a
/// weapon slot holding a key the item catalog does not know, so the loadout
/// editor loads saved defaults, its totals move and its flags fire.
const combatSave = () => {
  const armor = Array(16).fill('');
  armor[0] = 'EquipmentHats1';
  armor[1] = 'NotAnItem';
  return JSON.stringify({
    Research: '[]',
    charNames: ['Ana'],
    Lv0_0: JSON.stringify([120]),
    CharacterClass_0: 7,
    EquipOrder_0: JSON.stringify([armor, Array(7).fill(''), []]),
    EMm0_0: JSON.stringify({ 0: { STR: 2, Defence: 4 }, 1: { Weapon_Power: 3 } }),
    EMm1_0: '{}',
    SL_0: JSON.stringify({ 0: 5, 3: 2 }),
    SM_0: JSON.stringify({ 0: 20, 3: 10 }),
  });
};

/// A small flat `save.json` carrying one fishing character on the Deepwater
/// Docks spot, with a saved toolkit unless `toolkit: false`, so the fishing
/// section can open on a real pool and flag a missing toolkit key.
const fishingSave = ({ toolkit = true } = {}) => {
  const levels = Array(30).fill(0);
  levels[4] = 100;
  const tools = [];
  tools[1] = { 2: 'FishingRod13' };
  const save = {
    Research: '[]',
    Lv0_0: JSON.stringify(levels),
    EquipOrder_0: JSON.stringify(tools),
    PVFishingSpotIndex_0: 1,
    CurrentMap_0: 61,
    AFKtarget_0: 'FishMed',
  };
  if (toolkit) save.PVFishingToolkit_0 = JSON.stringify([8, 4]);
  return JSON.stringify(save);
};

/// A small flat `save.json` with two named characters standing on different
/// maps, so the stats context's character and map selectors have save defaults
/// to follow and a portal to read.
const statsSave = () => {
  const kla = Array.from({ length: 60 }, () => [0]);
  return JSON.stringify({
    charNames: ['Ana', 'Bo'],
    Research: '[]',
    Lv0_0: JSON.stringify(Array(30).fill(0)),
    Lv0_1: JSON.stringify(Array(30).fill(0)),
    CharacterClass_0: 1,
    CharacterClass_1: 7,
    CurrentMap_0: 0,
    CurrentMap_1: 50,
    KLA_0: JSON.stringify(kla),
    KLA_1: JSON.stringify(kla),
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
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-drop', 'killroy: the header import opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: killroySave() });
    await waitForNode(lane, 'import-format', 'killroy: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'killroy-forecast', 'killroy: the forecast opens after Apply');
    await lane.call('click_element', { id: 'upgrades' });
    await waitForNode(lane, 'killroy-upgrades', 'killroy: the upgrade list opens');
    await lane.call('click_element', { id: 'killroy-run' });
    await waitForNode(lane, 'killroy-comparison', 'killroy: the comparison runs');
    await waitFor(
      lane,
      (nodes) => nodes.some((node) => node.id?.startsWith('killroy-best-')),
      'killroy: the best option is marked',
    );
    await lane.call('click_element', { id: 'baseline' });
    await waitForNode(lane, 'killroy-forecast', 'killroy: the baseline tab reopens');
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
  fishing: async (lane) => {
    const importSave = async (text, ready, message) => {
      await lane.call('click_element', { id: 'header-import' });
      await waitForNode(lane, 'import-input', `${message}: the import dialog opens`);
      await lane.call('click_element', { id: 'import-input' });
      await lane.call('keyboard', { keystroke: 'secondary-a' });
      await lane.call('type_text', { text });
      await waitForNode(lane, 'import-format', `${message}: the save previews`);
      await lane.call('click_element', { id: 'import-apply' });
      await waitForNode(lane, ready, `${message}: ${ready} after Apply`);
    };
    await importSave(fishingSave(), 'fishing-summary', 'fishing');
    const saved = labelOf(await lane.tree(), 'fishing-summary') ?? '';
    if (!saved.includes('Deepwater Docks')) {
      throw new Error(`fishing: the saved spot reads ${saved}`);
    }
    await lane.call('click_element', { id: 'fishing-location-54-0' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'fishing-summary') ?? '').includes('Salty Shores'),
      'fishing: clicking a location switches the pool'
    );
    const detail = labelOf(await lane.tree(), 'fishing-detail') ?? '';
    if (!detail.includes('Small Fish')) throw new Error(`fishing: the split reads ${detail}`);
    // A save without PVFishingToolkit_0 defaults to Fly + Fishing Twine and says so.
    await importSave(
      fishingSave({ toolkit: false }),
      'fishing-toolkit-missing',
      'fishing'
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
  'combat-sim': async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'combat-sim: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: combatSave() });
    await waitForNode(lane, 'import-format', 'combat-sim: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'combat-slot-0-0', 'combat-sim: the loadout loads');
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'combat-slot-0-0') ?? '').includes('Farmer Brim') &&
        (labelOf(nodes, 'combat-issues') ?? '').includes('NotAnItem'),
      'combat-sim: saved gear loads and the unknown item is flagged',
    );
    // Pick the weapon slot, then the first weapon the picker offers.
    await lane.call('click_element', { id: 'combat-slot-0-1' });
    await waitForNode(lane, 'combat-item-select', 'combat-sim: the slot detail opens');
    await lane.call('focus_element', { id: 'combat-item-select' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => {
        const power = /Weapon Power (\d+)/.exec(labelOf(nodes, 'combat-summary') ?? '');
        return (
          !(labelOf(nodes, 'combat-slot-0-1') ?? '').includes('NotAnItem') &&
          power !== null &&
          Number(power[1]) > 0
        );
      },
      'combat-sim: picking an item updates the totals and clears the flag',
    );
    // A talent edit moves the summary on the next frame.
    await lane.call('click_element', { id: 'talents' });
    await waitForNode(lane, 'combat-talent-0', 'combat-sim: the talent table opens');
    await lane.call('click_element', { id: 'combat-talent-0-more' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'combat-summary') ?? '').includes('6 talent points'),
      'combat-sim: a talent edit updates the summed points',
    );
    // The working loadout saves as a named preset.
    await lane.call('click_element', { id: 'combat-preset-name' });
    await lane.call('type_text', { text: 'Bossing' });
    await lane.call('click_element', { id: 'combat-preset-save' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'combat-preset-0') ?? '').includes('Bossing'),
      'combat-sim: the preset is saved',
    );
    await lane.call('click_element', { id: 'gear' });
    await waitForNode(lane, 'combat-slot-0-0', 'combat-sim: the equipment tab reopens');
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
    await waitForNode(lane, 'research-dashboard', 'research: the dashboard tab opens first');
    for (const id of [
      'dash-kpis',
      'dash-grid',
      'dash-observations',
      'dash-exp-formula',
      'dash-afk-formula',
      'dash-insight-formula',
    ]) {
      await waitForNode(lane, id, `research: the dashboard renders ${id}`);
    }
    await lane.call('click_element', { id: 'dash-cell-129' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'square-name') ?? '').includes('J6'),
      'research: the grid preview opens the editor on its square',
    );
    await lane.call('click_element', { id: 'dashboard' });
    await waitForNode(lane, 'research-dashboard', 'research: the dashboard tab reopens');
    await lane.call('click_element', { id: 'dash-obs-0' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'observations-selected') ?? '').startsWith('Selected:'),
      'research: the observation preview opens the rates editor on its observation',
    );
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
  ladle: async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'ladle: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: ladleSave() });
    await waitForNode(lane, 'import-format', 'ladle: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'ladle-section', 'ladle: the section opens after Apply');
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'ladle-characters') ?? '').includes('2 checked'),
      'ladle: the two Cooking characters are counted',
    );

    await lane.call('focus_element', { id: 'ladle-breakdown-character' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => labelOf(nodes, 'ladle-breakdown-character') === 'Breakdowns for Bo',
      'ladle: the breakdown follows the character selector',
    );

    await lane.call('click_element', { id: 'exp' });
    await waitForNode(lane, 'ladle-exp-pane', 'ladle: the Cooking EXP tab opens');
    await lane.call('click_element', { id: 'ladle-split' });
    await waitForNode(lane, 'ladle-exp-table', 'ladle: the split keeps the table rendered');
    await lane.call('click_element', { id: 'ladles' });
    await waitForNode(lane, 'ladle-breakdowns', 'ladle: the Ladles tab reopens');
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
  'stats-all': async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'stats-all: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: statsSave() });
    await waitForNode(lane, 'import-format', 'stats-all: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-context-note') ?? '') ===
        'Ana · Blunder Hills · Mining · Construction Active',
      'stats-all: the save defaults open on character one and its current map',
    );
    const count = (nodes) => Number(/^(\d+) maps/.exec(labelOf(nodes, 'stats-map-count') ?? '')?.[1]);
    const accessible = count(await lane.tree());
    if (!(accessible > 0)) throw new Error(`stats-all: the accessible list reads ${accessible}`);

    // Character two follows its own current map.
    await lane.call('focus_element', { id: 'stats-character' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-context-note') ?? '') ===
        'Bo · YumYum Grotto · Mining · Construction Active',
      'stats-all: the character selector switches to that character\'s current map',
    );

    // The map list level widens the map selector and refreshes the summary.
    await lane.call('focus_element', { id: 'stats-map-visibility' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => {
        const label = labelOf(nodes, 'stats-map-count') ?? '';
        return label.includes('All Valid Maps') && count(nodes) > accessible;
      },
      'stats-all: the map list level refreshes the map summary',
    );

    // Skill and construction mode follow their selectors.
    await lane.call('focus_element', { id: 'stats-skill' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await lane.call('focus_element', { id: 'stats-construction' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-context-note') ?? '') ===
        'Bo · YumYum Grotto · Choppin · Construction Inactive',
      'stats-all: the skill and construction selectors refresh the context',
    );

    // A project document restores the whole context.
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'stats-all: the import dialog reopens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', {
      text: JSON.stringify({
        format: 'idleon-optimizer-project',
        version: 1,
        section: 'stats-all',
        research: {
          researchLevel: 100,
          gridLevels: Array(240).fill(0),
          occFound: Array(80).fill(0),
          insightLvs: Array(80).fill(0),
          insightProgress: Array(80).fill(0),
          magData: [],
          shapePositions: [],
        },
        sections: {
          'stats-all': {
            character: 1,
            map: 50,
            mapVisibility: 'all',
            skill: 'spelunking',
            construction: 'inactive',
          },
        },
      }),
    });
    await waitForNode(lane, 'import-format', 'stats-all: the project previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-context-note') ?? '') ===
        'Bo · YumYum Grotto · Spelunking · Construction Inactive',
      'stats-all: the project document restores the calculation context',
    );
    const restored = await lane.tree();
    if (!(labelOf(restored, 'stats-map-count') ?? '').includes('All Valid Maps')) {
      throw new Error('stats-all: the restored map list level is not All');
    }
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
