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
/// and every upgrade unlocked, so the comparison has rows to run. The saved
/// buffs carry one timed buff and one permanent form toggle, and character 2's
/// bag has no blank slot or skull stack so the skull limit has something to say.
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
    BuffsActive_0: [
      { 0: 460, 1: 1891, 2: 3360, length: 3 },
      { 0: 420, 1: 5, 2: 5, length: 3 },
    ],
    BuffsActive_1: [{ 0: 94, 1: 6683, 2: 8220, length: 3 }],
    InventoryOrder_0: ['Quest25', 'Blank', 'Blank'],
    InventoryOrder_1: ['Pearl4'],
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

/// A small flat save for the Farming planner: six planted plots across the
/// families, one evolution-blocked, Land Rank ranks that unlock the allocation's
/// candidate rows and leave points unspent, night market 5 unlocking Land Rank
/// XP, and twelve discovered crops so the assignment has candidates.
const farmingSave = () => {
  const row = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const plots = Array.from({ length: 36 }, () => [-1, 0, 0, 0, 0, 0, 0]);
  plots[0] = [0, 1000, 2, 0, 1, 0, 0];
  plots[1] = [1, 0, 4, 0, 3, 1, 0];
  plots[2] = [3, 12000, 5, 0, 2, 0, 0];
  plots[3] = [6, 0, 0, 0, 1, 4, 0];
  plots[4] = [2, 0, 1, 1, 1, 2, 0];
  plots[5] = [5, 0, 60, 0, 4, 0, 0];
  const ranks = row(36, {
    0: 10, 1: 20, 2: 30, 3: 40, 4: 50, 5: 60, 6: 70,
    7: 80, 8: 90, 9: 100, 10: 110, 11: 120,
  });
  return JSON.stringify({
    Research: '[]',
    FarmPlot: JSON.stringify(plots),
    FarmRank: JSON.stringify([ranks, row(36), row(20)]),
    FarmUpg: JSON.stringify(row(100, { 15: 1 })),
    FarmCrop: JSON.stringify({ 0: 150000, 1: 3000, 2: 1200, 3: 500, 21: 2500, 66: 11000, 230: 5 }),
    Lv0_0: JSON.stringify(row(24, { 16: 60 })),
  });
};

/// A small flat save carrying the three Monument rows Bravery reads: 600 saved
/// training hours and two unlocked milestones, so the run-state odds have a
/// damage range for a boost stack to move.
const monumentSave = () => {
  const zeros = (length) => Array(length).fill(0);
  const holes = Array.from({ length: 33 }, () => []);
  holes[7] = zeros(4);
  holes[11] = zeros(70);
  holes[13] = zeros(100);
  holes[14] = [600, 2];
  holes[15] = zeros(30);
  holes[22] = zeros(20);
  holes[24] = zeros(40);
  holes[26] = zeros(30);
  return JSON.stringify({ Research: '[]', Holes: JSON.stringify(holes) });
};

/// A small flat `save.json` with three characters, one of them away from Cooking,
/// a Prayer 2 charge and a Post Office 19c roll, so the ladle rates have inputs,
/// plus two kitchens, four meal levels and a Blue Jellyfish grant so the reset
/// plan has a layout to draw.
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
    Cooking: [
      [1, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      [2, 1, 0, 0, 0, 0, 10, 0, 0, 0, 0],
    ],
    Meals: [[3, 2, 1, 1], [], [140, 0, 0, 0], []],
    Menagerie: [[0, 0, 0, 0, 0, 9], [], [0, 0, 0, 0, 0, 1], []],
    BundlesReceived: { bun_s: 1 },
  });
};

/// A small flat `save.json` carrying a cog board and shelf, so the optimizer has
/// a player to surround, one mis-placed surround and a stronger shelf cog to
/// place. Construction level 42 comes from `Lv0_0[8]`.
const cogSave = () => {
  const order = Array(252).fill('Blank');
  order[14] = 'Player_Ana';
  order[13] = 'Cog3A00';
  order[15] = 'CogCry1';
  order[89] = 'Cog3cr';
  order[90] = 'Cog3le';
  order[108] = 'CogCry2';
  order[109] = 'Cog3ri';
  const map = {
    14: { a: 100000, b: 1000, c: 5000 },
    13: { d: 10, a: 100 },
    15: { d: 5 },
    89: { h: 'around', f: 80, d: 30 },
    90: { h: 'left', f: 20, d: 20 },
    108: { d: 200 },
    109: { h: 'right', f: 60, d: 40 },
  };
  const level = Array(30).fill(0);
  level[8] = 42;
  return JSON.stringify({
    Research: '[]',
    Lv0_0: JSON.stringify(level),
    CogO: JSON.stringify(order),
    CogM: JSON.stringify(map),

  });
};

/// A small flat `save.json` carrying the Cove's option rows: a few upgrade
/// levels, five blue digits, 1K Glunko kills and a 100x normal drop rate, so the
/// overview's DR tree multiplies out to 2.57712. `rows: false` shortens the row
/// past index 630, which must read zero and warn.
const cglunkoSave = ({ rows = true } = {}) => {
  const options = Array(rows ? 669 : 30).fill(0);
  if (rows) {
    options[630] = 5; // Opal Find
    options[631] = 2; // Slop Drop: 15 per level.
    options[632] = 5; // Reslime: 2 per level.
    options[633] = 5; // Droplier: 8 per level.
    options[634] = 2; // Killionaire: 3 per level.
    options[636] = 3; // Big Hit Multkill: 1 per level.
    options[638] = 4; // Sleepy Drops: 1 per level.
    options[640] = 4; // Blue Collector: 1 per level.
    options[641] = 4; // Researchy: 1 per level.
    options[644] = 3; // Cavernicus: 10 per level.
    options[645] = 2; // Multikill: 100 per level.
    options[649] = 6; // DeEmGe: 1 per level.
    options[650] = 2; // Grandioso: 5 per level.
    options[654] = 1000; // Blue shape 1: three digits.
    options[655] = 100; // Blue shape 2: two digits.
    options[668] = 1000; // Glunko kills: three digits.
    options[200] = 100; // Normal DR: two digits.
  }
  return JSON.stringify({
    Research: '[]',
    Lv0_0: JSON.stringify(Array(30).fill(0)),
    OptLacc: JSON.stringify(options),
  });
};
/// A small flat `save.json` carrying the Clamworks rows: a pearl balance, nine
/// upgrade levels, a worker-class level and a Scurge level, so the optimizer has
/// a budget and the promotion tab has progress.
const clamworksSave = () => {
  const options = Array(470).fill(0);
  options[454] = 5_000_000; // pearl balance
  [120, 12, 30, 20, 40, 2, 8, 0, 0].forEach((level, index) => {
    options[455 + index] = level;
  });
  options[464] = 4; // worker-class level
  return JSON.stringify({
    Research: '[]',
    Lv0_0: JSON.stringify(Array(30).fill(0)),
    OptLacc: JSON.stringify(options),
    Menagerie: JSON.stringify([
      [0, 0, 0, 0, 0, 0, 0, 0, 40],
      [0],
      [0],
      [0],
      [0],
      [0, 0, 0, 0, 0, 0],
    ]),
  });
};

/// A small flat `save.json` carrying two cauldrons of bubbles: one already
/// Prisma-ed, one ready to add Prisma to, one unpurchased and one large bubble,
/// so every status the plan knows is on screen. Kazam carries the STARTUE EXP
/// bubble, the one effect with a downstream model in this build.
const prismaSave = () => {
  const options = Array(385).fill(0);
  options[383] = 120; // fragments
  options[384] = '_0,'; // Roid Ragin already carries Prisma
  return JSON.stringify({
    Research: '[]',
    Lv0_0: JSON.stringify([100]),
    CurrentMap_0: 5,
    CauldronInfo: JSON.stringify([[10, 0, 25, 5], [40], [7], [12, 0, 25]]),
    OptLacc: JSON.stringify(options),
  });
};

/// A small flat `save.json` carrying the rows the Cooking Mastery tab reads: a
/// mastery rank of 18 with three upgrades bought, a levelled Divorce Cake and
/// ribbon ranks, so the section ranks the upgrades and recommends one.
const masterySave = () => {
  const row = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const ribbons = Array(102).fill(0);
  for (let index = 28; index < 40; index += 1) ribbons[index] = 8;
  return JSON.stringify({
    Research: '[]',
    Lv0_0: JSON.stringify(row(30, { 0: 200 })),
    CookMaster: JSON.stringify([[], [18, 0, 0, 1000], [7, 3, 0, 0, 0, 0], [0, 0]]),
    Meals: JSON.stringify([row(90, { 73: 150 })]),
    Ribbon: JSON.stringify(ribbons),
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
///
/// `trench: true` adds two characters saved at the Trench and the cavern rows
/// the depth projection reads (the Trench's cleared layers and progress, the
/// jar collectible, the Bolaia studies and Legend 29).
const fishingSave = ({ toolkit = true, trench = false, roster = 1 } = {}) => {
  const levels = Array(30).fill(0);
  levels[4] = 100;
  const tools = [];
  tools[1] = { 2: 'FishingRod13' };
  const save = {
    charNames: ['Ana', 'Bo', 'Cy'].slice(0, roster),
    Research: '[]',
    Lv0_0: JSON.stringify(levels),
    EquipOrder_0: JSON.stringify(tools),
    PVFishingSpotIndex_0: 1,
    CurrentMap_0: 61,
    AFKtarget_0: 'FishMed',
  };
  if (toolkit) save.PVFishingToolkit_0 = JSON.stringify([8, 4]);
  for (let ci = 1; ci < roster; ci += 1) {
    save[`Lv0_${ci}`] = JSON.stringify(levels);
    save[`EquipOrder_${ci}`] = JSON.stringify(tools);
    save[`PVFishingToolkit_${ci}`] = JSON.stringify([2, 2]);
    save[`AFKtarget_${ci}`] = 'MotherlodeFISH';
  }
  if (trench) {
    const holes = Array.from({ length: 27 }, () => []);
    // Holes[11]: per-cavern progress and cleared layers ([progress, layers]),
    // then the Trench's own progress and layers.
    holes[11] = [0, 3, 0, 2, 0, 1, 900, 2];
    // Holes[24][5]: the jar's Amethyst Heartstone level.
    holes[24] = [0, 0, 0, 0, 0, 4];
    // Holes[26]: the four Bolaia layer studies.
    holes[26] = [0, 5, 0, 0, 0, 0, 0, 4, 0, 0, 0, 3, 0, 0, 0, 0, 2];
    save.Holes = JSON.stringify(holes);
    const spelunk = Array.from({ length: 19 }, () => []);
    spelunk[18] = Array.from({ length: 50 }, () => 0);
    spelunk[18][29] = 2;
    save.Spelunk = JSON.stringify(spelunk);
  }
  return JSON.stringify(save);
};

/// A small flat `save.json` with one character standing inside a dungeon, so
/// the drop-rate page's runtime panel appears on its saved map.
const dropRateSave = () => {
  const kla = Array.from({ length: 60 }, () => [0]);
  return JSON.stringify({
    charNames: ['Ana'],
    Research: '[]',
    Lv0_0: JSON.stringify(Array(30).fill(0)),
    CharacterClass_0: 1,
    CurrentMap_0: 39,
    KLA_0: JSON.stringify(kla),
  });
};

/// A small flat `save.json` with two named characters standing on different
/// maps, so the stats context's character and map selectors have save defaults
/// to follow and a portal to read. The carry rows give the capacity tab a
/// formula tree to render.
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
    MaxCarryCap_0: JSON.stringify({
      Mining: 35000,
      Chopping: 35000,
      Fishing: 35000,
      Bugs: 35000,
      Foods: 35000,
      Critters: 35000,
      Souls: 35000,
      bCraft: 35000,
    }),
  });
};

/// A small flat `save.json` with one character's statue rows, a STARTUE EXP
/// bubble at level 200, a Skinny 0 Cal vial at level 20 and one Royal Guardian
/// Flair level, so the bubble, the deposit estimate and the time estimate all
/// have save state to read.
const statueSave = () => {
  const sparse = (length, filled = {}) =>
    Array.from({ length }, (_, index) => filled[index] ?? 0);
  const cauldron = Array.from({ length: 5 }, () => []);
  cauldron[3] = sparse(46, { 2: 200 });
  cauldron[4] = sparse(60, { 6: 20 });
  const royal = Array.from({ length: 23 }, () => []);
  royal[22] = sparse(32, { 0: 1 });
  return JSON.stringify({
    charNames: ['Ana'],
    Research: '[]',
    Lv0_0: JSON.stringify(sparse(30, { 20: 42 })),
    StatueLevels_0: JSON.stringify(
      Array.from({ length: 32 }, (_, index) => [100 + index, 0]),
    ),
    CauldronInfo: JSON.stringify(cauldron),
    OptLacc: JSON.stringify(sparse(70, { 69: 2 })),
    StuG: JSON.stringify(sparse(32, { 0: 2, 1: 3 })),
    RoyalG: JSON.stringify(royal),
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

/// A small flat save the sailing tabs read: three boats, three captains, every
/// chest-slot source and an artifact that feeds the chest rarity scale. It stays
/// well under the bridge's type-text limit and makes the pile simulation's
/// tier mix and capacity non-trivial.
const sailingSave = () => {
  const row = (length, filled = {}) =>
    Array.from({ length }, (_, index) => filled[index] ?? 0);
  const research = Array.from({ length: 48 }, () => []);
  research[0] = row(240);
  const sailing = [new Array(20).fill(-1), row(31, { 0: 5000, 1: 250 }), [2, 2], row(41)];
  sailing[3][0] = 1;
  sailing[3][1] = 6;
  sailing[3][9] = 2;
  sailing[3][19] = 2;
  const tasks = Array.from({ length: 6 }, () => []);
  tasks[4] = [0, 0, 2];
  return JSON.stringify({
    Research: JSON.stringify(research),
    Lv0_0: JSON.stringify(row(21, { 0: 100, 13: 10 })),
    OptLacc: JSON.stringify(row(128)),
    Boats: JSON.stringify([
      [1, 0, 0, 100, 0, 100, 0],
      [0, 0, 0, 80, 0, 80, 0],
      [2, 0, 0, 60, 0, 60, 0],
    ]),
    Captains: JSON.stringify([
      [3, 0, 1, 10, 250, 20, 10],
      [6, 3, 4, 20, 1000, 30, 15],
      [2, 1, 0, 5, 0, 10, 10],
    ]),
    GemItemsPurchased: JSON.stringify(row(140, { 129: 4 })),
    TaskZZ2: JSON.stringify(tasks),
    AchieveReg: JSON.stringify(row(400, { 287: -1 })),
    SailChests: JSON.stringify([]),
    Sailing: JSON.stringify(sailing),
  });
};

/// A small flat save with a Divinity budget, a God Rank, offer tier 1, Coral Kid
/// levels and the account rows the planner reads, so a run has actions to list.
const coralSave = () => {
  const sparse = (length, filled = {}) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const research = Array.from({ length: 9 }, () => sparse(240));
  research[7][4] = 5;
  return JSON.stringify({
    Research: JSON.stringify(research),
    Lv0_0: JSON.stringify(sparse(30, { 14: 2000, 20: 42 })),
    Divinity: JSON.stringify(sparse(30, { 24: 1e12, 25: 12, 26: 1 })),
    OptLacc: JSON.stringify(
      sparse(700, { 427: 20, 428: 10, 429: 10, 430: 10, 431: 10, 432: 10, 616: 5, 617: 5 }),
    ),
    UpgVault: JSON.stringify(sparse(80, { 76: 40 })),
  });
};

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

/// A small flat `save.json` carrying a cavern: a level-10 fountain with the
/// upgrade levels that unlock six currencies, a coin stack and a duck bar, so
/// the overview has rates, stacks and a bonus warning to read.
/// The committed synthetic account: it carries the full `Ninja` rows, so the
/// loadout, the inventory and the saved floor assignments all have data.
const sneakingSave = () => readFileSync(join(process.cwd(), 'fixtures/data.json'), 'utf8');

const fountainSave = () => {
  const row = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const holes = Array.from({ length: 35 }, () => []);
  holes[7] = row(30, { 15: 10 });
  holes[9] = row(36, { 30: 1e6, 31: 1e6, 32: 1e9 });
  holes[11] = row(100, { 81: 5000, 82: 0 });
  holes[13] = row(150, { 94: 1, 95: 1, 97: 1 });
  holes[17] = row(12, { 6: 100 });
  holes[21] = row(15, { 7: 10 });
  holes[22] = row(40, { 16: 10 });
  holes[26] = row(30, { 15: 10 });
  holes[29] = [[0, 1, 2, 3, 4, 5]];
  holes[30] = row(30, {});
  const upgrades = row(20, {
    0: 1, 1: 10, 2: 10, 3: 10, 4: 10, 5: 10,
    6: 10, 7: 10, 8: 10, 9: 10, 10: 10, 11: 10, 12: 10,
  });
  holes[31] = [
    upgrades,
    row(20, { 10: 10, 12: 10 }),
    row(20, { 10: 1, 12: 1 }),
  ];
  holes[32] = [row(20), row(20), row(20)];
  holes[33] = [0, 0, 45000];
  holes[34] = [1];
  return JSON.stringify({ Research: '[]', Holes: JSON.stringify(holes) });
};

/// A small flat `save.json` carrying the Spelunking rows: four unlocked caves,
/// ten characters at 100 stamina, four belt slots, five duplicates and two
/// memory slots, so the party, belt and depth estimate all render.
const spelunkSave = () => {
  const row = (length, filled = {}) =>
    Array.from({ length }, (_, index) => filled[index] ?? 0);
  const rows = Array.from({ length: 58 }, () => []);
  rows[0] = row(15, { 0: 1, 1: 1, 2: 1, 3: 1 });
  rows[1] = row(15, { 0: 42, 1: 12 });
  rows[3] = row(12, Object.fromEntries(Array.from({ length: 12 }, (_, id) => [id, 100])));
  rows[4] = row(30, { 10: 1 });
  rows[5] = row(68, { 12: 1, 14: 1, 15: 1, 16: 1, 17: 1, 23: 12, 24: 4, 25: 5, 53: 1, 57: 2 });
  rows[6] = ['Smol_Pebbles', 'Smooth_Rocks'];
  rows[7] = [0, 2, 7, -1, -1];
  rows[47] = row(15, { 0: 9 });
  rows[48] = [7];
  const lv0 = row(30, { 0: 500, 19: 10 });
  const save = { Research: '[]', Spelunk: JSON.stringify(rows) };
  save.OptionsListAccount = JSON.stringify(row(701, { 478: 15, 538: 1 }));
  save.GemItemsPurchased = JSON.stringify(row(44, { 43: 4 }));
  for (let id = 0; id < 10; id += 1) {
    save[`Lv0_${id}`] = JSON.stringify(lv0);
    save[`CharacterClass_${id}`] = String(10 + id);
  }
  return JSON.stringify(save);
};

/// A small flat `save.json` carrying a Jelly Operator board: eight cells
/// unlocked, the starting slots open, two Plasmids on the saved board, three
/// attempts banked and no account multipliers, so the estimate has a board to
/// run and the palette has every cell type to place.
const jellySave = () => {
  const row = (length, filled = {}) =>
    Array.from({ length }, (_, index) => filled[index] ?? 0);
  const research = Array.from({ length: 19 }, () => []);
  research[7] = row(15, { 9: 0, 10: 3, 11: 1000, 12: 500, 13: 0, 14: 2000 });
  research[14] = Array.from({ length: 180 }, () => -1);
  research[14][77] = 1;
  research[14][95] = 1;
  research[15] = row(9);
  research[16] = row(9);
  research[17] = row(40, { 0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 1, 7: 1 });
  research[18] = [];
  return JSON.stringify({
    Research: JSON.stringify(research),
    Lv0_0: JSON.stringify(row(30)),
  });
};

/// A small flat `save.json` carrying the Royal Guardian rows: one built W1
/// outpost with three guards and its links, a second outpost the militia is
/// clearing, and the Armory levels an overview reads. The Spelunk, Research and
/// SL rows give the clearing rate its cross-system factors.
const royalSave = () => {
  const row = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const armory = row(83, { 21: 20, 23: 10 });
  const g = Array.from({ length: 24 }, () => []);
  g[2] = armory;
  g[6] = row(4, { 0: 4 });
  g[7] = row(4, { 0: 2 });
  g[23] = row(10, { 3: 4, 8: 1 });
  const maps = Array.from({ length: 200 }, () => []);
  // A built Normal outpost on map 1: barracks, logistics, education, five rank
  // bars, two links, the type, the packed units and the glorified flag.
  maps[1] = [3, 20, 4, 5000, 900, 400, 200, 0, 7, -1, 0, 444111111, 0];
  // Map 2 has progress saved and a militia assignment, so it is clearing.
  maps[2] = [120];
  const research = row(19, {}).map(() => []);
  research[7] = row(20, { 9: 5 });
  research[14] = row(9, {});
  research[17] = row(10, {});
  return JSON.stringify({
    Research: JSON.stringify(research),
    RoyalG: JSON.stringify(g),
    RoyalMaps: JSON.stringify(maps),
    Spelunk: JSON.stringify(Array.from({ length: 55 }, () => [])),
    CharacterClass_0: 16,
    Lv0_0: JSON.stringify(row(30, { 0: 1200, 20: 40 })),
    SL_0: JSON.stringify({ 231: 300 }),
  });
};

/// A small flat `save.json` carrying three villagers and the `Holes` rows their
/// EXP rates read: levels, invested opals, stars, cosmo levels, a schematic, a
/// bell, two measurements, a monument slot and a fountain upgrade.
const opalSave = () => {
  const row = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const holes = Array.from({ length: 33 }, () => []);
  holes[1] = [5, 3, 0, 2, 0];
  holes[3] = [100, 40, 0, 15, 0];
  holes[4] = row(12, { 0: 4 });
  holes[5] = row(12, { 0: 5, 1: 4, 2: 3, 3: 4, 5: 4 });
  holes[11] = row(100, { 26: 6, 28: 1e6 });
  holes[13] = row(120, { 0: 1, 1: 1, 2: 1 });
  holes[15] = row(30, { 3: 500 });
  holes[17] = row(12, { 1: 100 });
  holes[22] = row(20, { 0: 10, 7: 10 });
  holes[31] = [row(20, { 13: 10, 14: 10 })];
  holes[32] = [row(20, {})];
  return JSON.stringify({ Research: '[]', Holes: JSON.stringify(holes) });
};

/// Section-specific interaction checks. Every ready section must own one; a
/// planned section's interaction is the back action of its empty state.
const INTERACTIONS = {
  spelunking: async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'spelunking: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: spelunkSave() });
    await waitForNode(lane, 'import-format', 'spelunking: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'spelunk-controls', 'spelunking: the controls render after Apply');
    const summary = labelOf(await lane.tree(), 'spelunk-depth-summary') ?? '';
    if (!summary.includes('Pebble Cove') || !summary.includes('Party stamina')) {
      throw new Error(`spelunking: the depth summary reads ${summary}`);
    }
    await waitForNode(lane, 'spelunk-depth-risk', 'spelunking: the exit chart renders');
    await waitForNode(lane, 'spelunk-depth-stamina', 'spelunking: the stamina chart renders');
    await waitForNode(lane, 'spelunk-belt', 'spelunking: the saved belt renders');
    await lane.call('click_element', { id: 'spelunk-preset-amber' });
    await waitFor(
      lane,
      (nodes) => {
        const slot = nodes.find((node) => node.id === 'spelunk-elixir-0');
        return (slot?.value?.value ?? '') === 'Amber boost';
      },
      'spelunking: the Amber preset fills the first belt slot',
    );
    const stamina = valueOf(await lane.tree(), 'spelunk-stamina') ?? '';
    if (stamina !== 'Saved stamina') {
      throw new Error(`spelunking: the starting stamina reads ${stamina}`);
    }
  },
  jelly: async (lane) => {
    await lane.call('click_element', { id: 'jelly-import' });
    await waitForNode(lane, 'import-drop', 'jelly: the empty state opens the import');
    await lane.call('click_element', { id: 'import-cancel' });
    await waitFor(lane, (nodes) => !has(nodes, 'import-drop'), 'jelly: cancel closes the import');
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'jelly: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: jellySave() });
    await waitForNode(lane, 'import-format', 'jelly: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'jelly-overview', 'jelly: the overview opens after Apply');
    await waitForNode(lane, 'jelly-snapshot', 'jelly: the account snapshot renders');
    // The layout editor places every cell type; a Plasmid sits on the saved board.
    await lane.call('click_element', { id: 'jelly-tab-layout' });
    await waitForNode(lane, 'jelly-board', 'jelly: the board renders');
    await waitForNode(lane, 'jelly-palette-7', 'jelly: the palette lists every cell type');
    await lane.call('click_element', { id: 'jelly-palette-0' });
    await lane.call('click_element', { id: 'jelly-slot-96' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'jelly-message') ?? '').includes('Placed Amoeba at slot 96'),
      'jelly: the palette places the picked cell',
    );
    // The estimate runs off the UI thread and stores its aggregate.
    await lane.call('click_element', { id: 'jelly-tab-overview' });
    await waitForNode(lane, 'jelly-overview', 'jelly: the overview reopens');
    await lane.call('click_element', { id: 'jelly-run-estimate' });
    await waitForNode(lane, 'jelly-estimate-result', 'jelly: the estimate stores a result', 120000);
    await waitForNode(lane, 'jelly-contribution', 'jelly: the cell contribution renders');
    await waitForNode(lane, 'jelly-obstruction-outlook', 'jelly: the outlook renders');
  },
  'royal-guardian': async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-drop', 'royal-guardian: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: royalSave() });
    await waitForNode(lane, 'import-format', 'royal-guardian: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'royal-overview', 'royal-guardian: the overview opens after Apply');
    const world = labelOf(await lane.tree(), 'royal-world-card-0') ?? '';
    if (!world.includes('1 / 22')) throw new Error(`royal-guardian: W1 reads ${world}`);
    // The range panel explains the world's first built outpost.
    const panel = labelOf(await lane.tree(), 'royal-range-target') ?? '';
    if (!panel.includes('Resource ')) throw new Error(`royal-guardian: the range panel reads ${panel}`);
    await waitForNode(lane, 'royal-world-1', 'royal-guardian: the world rail lists W2');
    await lane.call('click_element', { id: 'royal-world-1' });
    await waitForNode(lane, 'royal-clear-51', 'royal-guardian: the W2 tab lists its outposts');
    await lane.call('click_element', { id: 'royal-world-0' });
    await waitForNode(lane, 'royal-clear-2', 'royal-guardian: the W1 tab lists its clearing');
  },
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
    // The saved buffs are the save's own rows: a timed buff starts dropped and a
    // form toggle starts kept until the option changes.
    await waitForNode(lane, 'killroy-buffs', 'killroy: the saved buffs render');
    const dropped = labelOf(await lane.tree(), 'killroy-buff-0-460') ?? '';
    if (!dropped.includes('1,891') || !dropped.includes('3,360') || !dropped.includes('dropped')) {
      throw new Error(`killroy: the timed buff reads ${dropped}`);
    }
    const form = labelOf(await lane.tree(), 'killroy-buff-0-420') ?? '';
    if (!form.includes('kept') || !has(await lane.tree(), 'killroy-form-0-420')) {
      throw new Error(`killroy: the form toggle reads ${form}`);
    }
    await lane.call('click_element', { id: 'killroy-keep-buffs' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'killroy-buff-0-460') ?? '').includes('kept'),
      'killroy: keeping saved buffs keeps the timed row',
    );
    await lane.call('click_element', { id: 'killroy-unlimited' });
    await lane.call('click_element', { id: 'upgrades' });
    await waitForNode(lane, 'killroy-upgrades', 'killroy: the upgrade list opens');
    // Cancel: twenty trials over every configuration leave the run in flight.
    for (let press = 0; press < 17; press += 1) {
      await lane.call('click_element', { id: 'killroy-trials-up' });
    }
    await lane.call('click_element', { id: 'killroy-run' });
    await waitForNode(lane, 'killroy-run-progress', 'killroy: the run reports progress');
    await lane.call('click_element', { id: 'killroy-run-cancel' });
    await waitForNode(lane, 'killroy-cancelled', 'killroy: cancel stops the run');
    await lane.call('click_element', { id: 'killroy-run' });
    await waitForNode(lane, 'killroy-comparison', 'killroy: the comparison runs', 120000);
    await waitFor(
      lane,
      (nodes) => nodes.some((node) => node.id?.startsWith('killroy-best-')),
      'killroy: the best option is marked',
    );
    const details = labelOf(await lane.tree(), 'killroy-result-details') ?? '';
    if (!details.includes('Saved buffs kept') || !details.includes('Unlimited inventory')) {
      throw new Error(`killroy: the run details read ${details}`);
    }
    await lane.call('click_element', { id: 'killroy-export' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'status-status') ?? '').includes('Killroy results JSON'),
      'killroy: the results export copies JSON',
    );
    await lane.call('click_element', { id: 'baseline' });
    await waitForNode(lane, 'killroy-forecast', 'killroy: the baseline tab reopens');
  },
  prisma: async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'prisma: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: prismaSave() });
    await waitForNode(lane, 'import-format', 'prisma: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'prisma-summary', 'prisma: the plan lists the saved state after Apply');
    await lane.call('click_element', { id: 'prisma-run' });
    await waitForNode(lane, 'prisma-row-0-2', 'prisma: the run lists the comparable bubbles');
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'prisma-bonus-0-2') ?? '').includes('10 → 20'),
      'prisma: the ready bubble gains the Prisma multiplier'
    );
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'prisma-bonus-0-0') ?? '').includes('10 → 20'),
      'prisma: the already-Prisma row shows the benefit it retains'
    );
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'prisma-change-0-2') ?? '').includes('No measured change'),
      'prisma: the bubble bonus stays an intermediate, not a downstream outcome'
    );
    await waitFor(
      lane,
      (nodes) => {
        const consumers = labelOf(nodes, 'prisma-coverage-consumers-TotalSTR') ?? '';
        const model = labelOf(nodes, 'prisma-coverage-model-TotalSTR') ?? '';
        return consumers.includes('TotalStats') && model.includes('Missing');
      },
      'prisma: coverage names the native consumers and the missing model'
    );
    const coverage = labelOf(await lane.tree(), 'prisma-coverage-count') ?? '';
    if (!coverage.includes('missing')) throw new Error(`prisma: coverage reads ${coverage}`);
    // Kazam carries the STARTUE EXP bubble: its row ranks by a real downstream
    // metric and the coverage table marks it modelled.
    await lane.call('click_element', { id: 'segment-3' });
    await waitForNode(lane, 'prisma-cauldron', 'prisma: the Kazam tab opens');
    await lane.call('click_element', { id: 'prisma-run' });
    await waitForNode(lane, 'prisma-row-3-2', 'prisma: the Kazam run lists the STARTUE EXP row');
    await waitFor(
      lane,
      (nodes) => {
        const change = labelOf(nodes, 'prisma-change-3-2') ?? '';
        const model = labelOf(nodes, 'prisma-coverage-model-StatueStartEXP') ?? '';
        return change.includes('Statue EXP retained on level-up')
          && change.includes('+7.35')
          && model.includes('Modelled')
          && model.includes('Statue EXP retained on level-up');
      },
      'prisma: the STARTUE EXP row shows its retained statue EXP and coverage marks it modelled'
    );
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
    await lane.call('click_element', { id: 'rupie' });
    await waitForNode(lane, 'jar-rupies', 'jar: the rupie value tab opens');
    await waitForNode(lane, 'jar-rupies-breakdown', 'jar: the rupie breakdown renders');
    const payout = labelOf(await lane.tree(), 'jar-kpi-rupies-break') ?? '';
    if (payout === '') throw new Error('jar: the rupie payout reads nothing');
    await lane.call('click_element', { id: 'jar-tier-toggle-2' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'jar-kpi-rupies-break') ?? '') !== payout,
      'jar: switching the tier changes the rupies a break pays',
    );
    await lane.call('click_element', { id: 'chances' });
    await waitForNode(lane, 'jar-chances', 'jar: the chances tab opens');
    await waitForNode(lane, 'jar-chance-opal', 'jar: the chance cards render');
    await lane.call('click_element', { id: 'overview' });
    await waitForNode(lane, 'jar-overview', 'jar: the overview tab reopens');
  },

  opal: async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-drop', 'opal: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: opalSave() });
    await waitForNode(lane, 'import-format', 'opal: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'opal-overview', 'opal: the villagers open after Apply');
    const current = labelOf(await lane.tree(), 'opal-current-line') ?? '';
    if (!current.includes('3 unlocked')) {
      throw new Error(`opal: the current state reads ${current}`);
    }
    await waitForNode(lane, 'opal-terms', 'opal: every bonus term renders');
    const total = labelOf(await lane.tree(), 'formula-node-root') ?? '';
    if (!total.includes('Total EXP/hr')) {
      throw new Error(`opal: the breakdown starts at the total, not ${total}`);
    }
    await lane.call('click_element', { id: 'opal-villager-0' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'formula-node-root') ?? '').includes('Explorer EXP/hr'),
      'opal: picking a villager readdresses the tree',
    );
    await lane.call('click_element', { id: 'opal-villager-all' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'formula-node-root') ?? '').includes('Total EXP/hr'),
      'opal: the total control readdresses the tree',
    );
  },
  sneaking: async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-drop', 'sneaking: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: sneakingSave() });
    await waitForNode(lane, 'import-format', 'sneaking: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'sneaking-loadout', 'sneaking: the loadout opens after Apply');
    await waitForNode(lane, 'sneaking-inventory', 'sneaking: the inventory renders');
    await waitForNode(lane, 'sneaking-saved-floors', 'sneaking: the saved floors render');
    const before = labelOf(await lane.tree(), 'sneak-slot-0-3') ?? '';
    await lane.call('click_element', { id: 'sneak-slot-0-3' });
    // Inventory row 24 holds a non-Gold Lotus Flower, a legal second charm.
    await lane.call('click_element', { id: 'sneak-inv-23' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'sneak-slot-0-3') ?? '') !== before,
      'sneaking: the inventory charm swaps into the slot'
    );
    await lane.call('click_element', { id: 'sneak-reset-loadout' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'sneak-slot-0-3') ?? '') === before,
      'sneaking: reset restores the saved charm'
    );
  },
  fountain: async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-drop', 'fountain: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: fountainSave() });
    await waitForNode(lane, 'import-format', 'fountain: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'fountain-overview', 'fountain: the cavern opens after Apply');
    const summary = labelOf(await lane.tree(), 'fountain-summary-line') ?? '';
    if (!summary.includes('Fountain level 10')) {
      throw new Error(`fountain: the summary reads ${summary}`);
    }
    await waitForNode(lane, 'fountain-income', 'fountain: the income table renders');
    await waitForNode(lane, 'fountain-missing', 'fountain: the missing bonuses are warned');
    await waitForNode(lane, 'fountain-levels', 'fountain: the level planner renders');
    await waitForNode(lane, 'fountain-stacks', 'fountain: the ground stack renders');
    // The Currency tab: values, the water multipliers, the desired simulator
    // and the unlock tracker. Picking a row switches the simulation.
    await lane.call('click_element', { id: 'fountain-tab-currency' });
    await waitForNode(lane, 'fountain-currency-values', 'fountain: the currency tab opens');
    await waitForNode(
      lane,
      'fountain-water-multipliers',
      'fountain: the per-water multipliers render'
    );
    await waitForNode(lane, 'fountain-desired', 'fountain: the desired simulator renders');
    await waitForNode(lane, 'fountain-unlocks', 'fountain: the unlock tracker renders');
    await lane.call('click_element', { id: 'fountain-desired-1' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'fountain-summary-line') ?? '').includes('simulating Silver'),
      'fountain: the desired currency switches'
    );
    // The Upgrades tab: the water strip, the rows and the cost curve.
    await lane.call('click_element', { id: 'fountain-tab-upgrades' });
    await waitForNode(lane, 'fountain-upgrade-table', 'fountain: the upgrades tab opens');
    await waitFor(
      lane,
      (nodes) => nodes.some((node) => node.id?.startsWith('fountain-cost-curve')),
      'fountain: the cost curve renders'
    );
    await lane.call('click_element', { id: 'fountain-water-2' });
    await waitForNode(lane, 'fountain-upgrade-2-11', 'fountain: the green water rows render');
    await lane.call('click_element', { id: 'fountain-upgrade-2-11' });
    await lane.call('click_element', { id: 'fountain-tab-overview' });
    await waitForNode(lane, 'fountain-overview', 'fountain: the overview reopens');
  },
  farming: async (lane) => {
    // The Overview tab leads: the priority signals, the Crop Scientist table
    // and the growth breakpoints from the save, then every plot's curve.
    await waitForNode(lane, 'farming-cards', 'farming: the overview cards render');
    await waitForNode(lane, 'farming-signals', 'farming: the priority signals render');
    await waitForNode(
      lane,
      'farming-crop-scientist-table',
      'farming: the Crop Scientist table renders'
    );
    await waitForNode(lane, 'farming-growth-table', 'farming: the growth breakpoints render');
    await waitFor(
      lane,
      (nodes) => {
        const labels = nodes
          .filter((node) => node.id?.startsWith('farming-curve-plot-'))
          .map((node) => node.label ?? '');
        return labels.length === 36 && labels.every((label) => label.includes('target OG'));
      },
      'farming: every plot has a curve row'
    );
    const curve = labelOf(await lane.tree(), 'farming-curve-title') ?? '';
    if (curve !== 'Plot 1 curve') throw new Error(`farming: the curve title reads ${curve}`);
    await lane.call('click_element', { id: 'farming-curve-plot-7' });
    await waitFor(
      lane,
      (nodes) => labelOf(nodes, 'farming-curve-title') === 'Plot 8 curve',
      'farming: clicking a curve row switches the curve'
    );
    await lane.call('click_element', { id: 'plots' });
    await waitForNode(lane, 'farming-field', 'farming: the full plot grid opens');
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

    // The assignment and the allocator need a real field.
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'farming: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: farmingSave() });
    await waitForNode(lane, 'import-format', 'farming: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'farming-assignment') ?? '') === 'Plot assignment: Land Rank XP' &&
        (labelOf(nodes, 'farming-assignment-summary') ?? '').includes('of 36 plots differ'),
      'farming: the loaded field is assigned for the objective'
    );

    // Switching the objective re-plans the assignment.
    await lane.call('focus_element', { id: 'farming-objective' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'farming-assignment') ?? '') === 'Plot assignment: Farming EXP',
      'farming: the objective selector re-plans the assignment'
    );

    // The allocator runs off the UI thread and reports its plan.
    await lane.call('click_element', { id: 'farming-run' });
    await waitFor(
      lane,
      (nodes) => {
        const gain = labelOf(nodes, 'farming-allocation-gain') ?? '';
        return gain.startsWith('Projected gain:') && has(nodes, 'farming-allocation-rows');
      },
      'farming: the allocation renders its plan'
    );
    const summary = labelOf(await lane.tree(), 'farming-allocation-summary') ?? '';
    if (!/\d+ of \d+ points assigned/.test(summary)) {
      throw new Error(`farming: the allocation summary reads ${summary}`);
    }
    // A different scope makes the stored allocation stale, not wrong.
    await lane.call('click_element', { id: 'farming-mode-reset' });
    await waitForNode(lane, 'farming-stale', 'farming: the scope change marks the result stale');
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
    // FI-02: every lure/line pair is ranked for the selected pool and depth.
    await waitForNode(lane, 'fishing-ranking', 'fishing: the all-combination ranking renders');
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'fishing-ranking-objective') ?? '') ===
        'Ranking by Fishing EXP · 225 combinations',
      'fishing: the ranking covers 225 combinations by EXP'
    );
    await lane.call('focus_element', { id: 'fishing-objective' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'fishing-ranking-objective') ?? '') ===
        'Ranking by Fish quantity · 225 combinations',
      'fishing: the objective selector re-ranks by fish quantity'
    );
    const ranked = await lane.tree();
    const best = labelOf(ranked, 'fishing-best-toolkit') ?? '';
    const match = /^Best lure \+ line: (.+ \+ .+)$/.exec(best);
    if (!match) throw new Error(`fishing: the best combination reads ${best}`);
    const toolkit = match[1];
    const top = labelOf(ranked, 'fishing-rank-1') ?? '';
    if (!top.endsWith('· best')) throw new Error(`fishing: rank 1 reads ${top}`);
    // A tie with the top row is marked on the row and explained under the table.
    const tied = (labelOf(ranked, 'fishing-rank-2') ?? '').endsWith('· tied for best');
    if (tied && !(labelOf(ranked, 'fishing-tie-note') ?? '').includes('share the top')) {
      throw new Error('fishing: a tied row has no tie explanation');
    }
    // The best row applies to the toolbar; a best that is already selected has
    // no button to press. The ranking sits below the fold, so scroll it into
    // view before the real pointer click.
    const useBest = ranked.find((node) => node.id === 'fishing-use-best');
    if (useBest) {
      const y = useBest.bounds?.y ?? 0;
      if (y > 600) {
        await lane.call('scroll', { x: 20, y: 500, delta_y: Math.ceil(y - 300) });
        await waitForNode(lane, 'fishing-use-best', 'fishing: the ranking scrolls into view');
      }
      await lane.call('click_element', { id: 'fishing-use-best' });
      await waitFor(
        lane,
        (nodes) => (labelOf(nodes, 'fishing-summary') ?? '').includes(toolkit),
        'fishing: Use best applies the rank-1 lure and line'
      );
    }
    // A save without PVFishingToolkit_0 defaults to Fly + Fishing Twine and says so.
    await importSave(
      fishingSave({ toolkit: false }),
      'fishing-toolkit-missing',
      'fishing'
    );
    // FI-03: the cavern rows give the Trench its saved depth, and the second
    // character, saved at the Trench, starts ticked and adds to the team ETA.
    await importSave(
      fishingSave({ trench: true, roster: 2 }),
      'fishing-trench',
      'fishing'
    );
    const trench = await lane.tree();
    const depth = labelOf(trench, 'fishing-trench-depth') ?? '';
    if (!/^Trench depth 3/.test(depth)) {
      throw new Error(`fishing: the Trench depth reads ${depth}`);
    }
    const helperRow = labelOf(trench, 'fishing-trench-helper-name-1') ?? '';
    if (!/^Bo: [\d.,KMBQ]+ fish\/hr/.test(helperRow)) {
      throw new Error(`fishing: the helper row reads ${helperRow}`);
    }
    const team = labelOf(trench, 'fishing-trench-team') ?? '';
    if (!team.startsWith('ETA with 1 helper(s)')) {
      throw new Error(`fishing: the team ETA reads ${team}`);
    }
    const scrollTo = async (id) => {
      const node = (await lane.tree()).find((entry) => entry.id === id);
      const y = node?.bounds?.y ?? 0;
      if (y > 600) {
        await lane.call('scroll', { x: 20, y: 500, delta_y: Math.ceil(y - 300) });
        await waitForNode(lane, id, `fishing: ${id} scrolls into view`);
      }
    };
    // An unticked helper drops out of the team, and ticking it back adds it.
    await scrollTo('fishing-trench-helper-1');
    await lane.call('click_element', { id: 'fishing-trench-helper-1' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'fishing-trench-team'),
      'fishing: unticking the helper drops the team ETA'
    );
    await lane.call('click_element', { id: 'fishing-trench-helper-1' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'fishing-trench-team') ?? '').startsWith('ETA with 1 helper'),
      'fishing: ticking the helper adds the team ETA back'
    );
    // The depth selector reprojects: 5 is the default, 10 is the next choice.
    await scrollTo('fishing-trench-depths');
    await lane.call('focus_element', { id: 'fishing-trench-depths' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'fishing-trench-projection-note') ?? '') ===
        '10 depths projected from the saved depth',
      'fishing: the projection reprojects ten depths'
    );
    // A roster with no other character shows the empty helper state.
    await importSave(
      fishingSave({ trench: true }),
      'fishing-trench-helpers-empty',
      'fishing'
    );
    const alone = await lane.tree();
    if (has(alone, 'fishing-trench-team')) {
      throw new Error('fishing: a one-character roster shows a team ETA');
    }
    // A save without Holes has no Trench to project and says so.
    await importSave(fishingSave(), 'fishing-trench-missing', 'fishing');
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
    // A mechanic override shows as a removable chip, and Reset clears it. The
    // mechanics tab runs before the slot edits: it needs no slot selection.
    await lane.call('click_element', { id: 'mechanics' });
    await waitForNode(lane, 'combat-override-dropRate', 'combat-sim: the mechanics tab opens');
    // The first Down opens the list on Saved; the second highlights Off.
    await lane.call('focus_element', { id: 'combat-override-dropRate-mode' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'combat-overrides-count') ?? '') === '1 active overrides' &&
        (labelOf(nodes, 'combat-override-chips') ?? '').includes('Drop rate off'),
      'combat-sim: the override is active and chipped',
    );
    await lane.call('click_element', { id: 'combat-overrides-reset' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'combat-overrides-count') ?? '') === '0 active overrides' &&
        !has(nodes, 'combat-override-chips'),
      'combat-sim: reset puts every mechanic back to the save',
    );
    await lane.call('click_element', { id: 'gear' });
    await waitForNode(lane, 'combat-slot-0-0', 'combat-sim: the equipment tab reopens');
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
  'cooking-mastery': async (lane) => {
    // The empty state is the import action: a save without the mastery rows
    // carries nothing for the tab to read.
    await waitForNode(lane, 'cooking-mastery-import', 'cooking-mastery: the empty state offers the import');
    await lane.call('click_element', { id: 'cooking-mastery-import' });
    await waitForNode(lane, 'import-input', 'cooking-mastery: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('type_text', { text: masterySave() });
    await waitForNode(lane, 'import-format', 'cooking-mastery: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'cooking-mastery-summary', 'cooking-mastery: the mastery figures load');
    const summary = labelOf(await lane.tree(), 'cooking-mastery-summary') ?? '';
    if (!summary.includes('Mastery level 18')) throw new Error(`cooking-mastery: the summary reads ${summary}`);
    await waitForNode(lane, 'cooking-mastery-upgrades', 'cooking-mastery: the upgrade comparison renders');
    await waitForNode(lane, 'cooking-mastery-formula-search', 'cooking-mastery: the breakdown tree is there');
    // A free point ranks SWEET first on this save: the cake level is the only
    // scaling term above zero.
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'cooking-mastery-recommendation') ?? '').includes('SWEET'),
      'cooking-mastery: the recommendation names the best upgrade'
    );
    // CK-02: the daily distribution spends every point the rank grants, and
    // SOUR is still locked at rank 18, so it takes none.
    await waitForNode(lane, 'cooking-mastery-distribution', 'cooking-mastery: the distribution plans the budget');
    const spent = labelOf(await lane.tree(), 'cooking-mastery-distribution') ?? '';
    if (!spent.includes('with 19 of 19 points')) {
      throw new Error(`cooking-mastery: the plan reads ${spent}`);
    }
    check(
      'cooking-mastery: the locked upgrade takes no points',
      (labelOf(await lane.tree(), 'cooking-mastery-distribution-row-4') ?? '').includes('→ 0'),
      labelOf(await lane.tree(), 'cooking-mastery-distribution-row-4') ?? ''
    );
    // Switching the goal to the next unlock plans for rank 25, where SOUR
    // joins: the plan holds the same points and now spends on it.
    await lane.call('focus_element', { id: 'cooking-mastery-goal' });
    // The first key opens the list, the second walks to the next goal, and
    // Enter picks it.
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'cooking-mastery-distribution') ?? '').includes('held until rank 25'),
      'cooking-mastery: the goal switch holds the points for the next unlock'
    );
    const planned = labelOf(await lane.tree(), 'cooking-mastery-distribution-row-4') ?? '';
    if (!/SOUR — Total Ribbons: 0 → [1-9]/.test(planned)) {
      throw new Error(`cooking-mastery: the next unlock plans SOUR as ${planned}`);
    }
    // The budget stepper spends less: the plan says how much and the rows drop.
    await lane.call('click_element', { id: 'cooking-mastery-budget-less' });
    await lane.call('click_element', { id: 'cooking-mastery-budget-less' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'cooking-mastery-distribution') ?? '').includes('with 17 of'),
      'cooking-mastery: the budget stepper re-plans'
    );
    await waitForNode(
      lane,
      'cooking-mastery-distribution-copy',
      'cooking-mastery: the plan offers a copy action'
    );
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

    // Shape Optimization: a built-in preset applies, a tier edit detaches it
    // without rewriting it, and the real optimizer fills the preview.
    await lane.call('click_element', { id: 'shapes' });
    await waitForNode(lane, 'shape-opt', 'research: the Shape Optimization tab opens');
    await waitForNode(lane, 'shape-zone-above', 'research: the tier zones render');
    await lane.call('click_element', { id: 'shape-preset-_minehead' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'shape-active') ?? '').includes('Minehead'),
      'research: the Minehead preset applies',
    );
    await lane.call('drag_element', {
      from_id: 'shape-tier-H10',
      to_id: 'shape-zone-above',
      steps: 12,
    });
    await waitFor(
      lane,
      (nodes) => {
        const active = labelOf(nodes, 'shape-active') ?? '';
        const square = nodes.find((node) => node.id === 'shape-tier-H10');
        return active.includes('no preset active') && (square?.label ?? '').includes('above tier');
      },
      'research: editing a tier detaches the built-in preset',
    );
    // The tier panel scrolls with the page, so save the preset while the rail
    // is still in view, then scroll the placement card up.
    await lane.call('click_element', { id: 'shape-preset-name' });
    await new Promise((resolve) => setTimeout(resolve, 250));
    await lane.call('type_text', { text: 'Minehead pin' });
    await new Promise((resolve) => setTimeout(resolve, 250));
    await lane.call('click_element', { id: 'shape-save-preset' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'shape-custom-preset-0') ?? '').includes('Minehead pin'),
      'research: the custom shape preset is saved',
    );
    await lane.call('scroll', { x: 720, y: 700, delta_y: 900 });
    await lane.call('click_element', { id: 'shape-optimize' });
    await waitForNode(lane, 'shape-preview', 'research: the optimizer previews a placement', 120000);
    await waitForNode(lane, 'shape-breakdown-order-0', 'research: the coverage breakdown lists the first shape');
    const shapeSummary = labelOf(await lane.tree(), 'shape-summary') ?? '';
    if (!shapeSummary.includes('shapes placed')) {
      throw new Error(`research: the shape summary reads ${shapeSummary}`);
    }
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

    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-drop', 'coral: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: coralSave() });
    await waitForNode(lane, 'import-format', 'coral: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'coral-plan', 'coral: the plan card opens after Apply');
    await lane.call('click_element', { id: 'coral-run' });
    await waitForNode(lane, 'coral-projection', 'coral: the run projects the coral state');
    await waitForNode(lane, 'coral-actions', 'coral: the run lists the plan actions');
    const summary = labelOf(await lane.tree(), 'coral-plan-summary') ?? '';
    if (!summary.includes('actions')) throw new Error(`coral: the plan summary reads ${summary}`);

    // The fit explains the offering model, and names the save fields it had to
    // read as zero; the current levels come straight from the imported save.
    await waitForNode(lane, 'coral-offering-fit', 'coral: the offering fit opens');
    const fit = labelOf(await lane.tree(), 'coral-fit-summary') ?? '';
    if (!fit.includes('success') || !fit.includes('attempt')) {
      throw new Error(`coral: the fit explanation reads ${fit}`);
    }
    await waitForNode(lane, 'coral-fit-missing', 'coral: the fit warns about the fields it lacks');
    await waitForNode(lane, 'coral-levels', 'coral: the current levels table opens');
    const levels = labelOf(await lane.tree(), 'coral-levels') ?? '';
    if (!levels.includes('20, 10')) throw new Error(`coral: the current levels read ${levels}`);
    if (has(await lane.tree(), 'coral-levels-missing')) {
      throw new Error('coral: the levels warn despite the save carrying them');
    }

    // An edited input keeps the plan on screen but marks it stale; putting the
    // input back marks it current again.
    await lane.call('click_element', { id: 'coral-assume-unlocked' });
    await waitForNode(lane, 'coral-stale', 'coral: an input change marks the plan stale');
    await lane.call('click_element', { id: 'coral-assume-unlocked' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'coral-stale'),
      'coral: restoring the input clears the stale marker',
    );
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

    // The section's toolbar ids nest under the toolbar, so the breakdown selector
    // is found by its suffix rather than a bare id. A combobox opens on Enter.
    const breakdownSelect = (await lane.tree()).find((node) =>
      (node.id ?? '').endsWith('.ladle-breakdown-character'),
    );
    await lane.call('focus_element', { id: breakdownSelect.id });
    await lane.call('keyboard', { keystroke: 'enter' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => nodes.some((node) => (node.label ?? '').startsWith('Breakdowns for Bo')),
      'ladle: the breakdown follows the character selector',
    );

    await lane.call('click_element', { id: 'exp' });
    await waitForNode(lane, 'ladle-exp-pane', 'ladle: the Cooking EXP tab opens');
    await lane.call('click_element', { id: 'ladle-split' });
    await waitForNode(lane, 'ladle-exp-table', 'ladle: the split keeps the table rendered');

    await lane.call('click_element', { id: 'kitchens' });
    await waitForNode(lane, 'kitchens-pane', 'ladle: the Kitchens & Jellyfish tab opens');
    await waitForNode(lane, 'kitchens-layout', 'ladle: the reset draws its kitchen layout');
    await lane.call('click_element', { id: 'kitchens-days-less' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'kitchens-day-30'),
      'ladle: the forecast follows the horizon',
    );
    await bringIntoView(lane, 'kitchens-show-layout');
    await lane.call('click_element', { id: 'kitchens-show-layout' });
    await waitForNode(lane, 'kitchens-pins', 'ladle: the kitchen pins open');
    await bringIntoView(lane, 'kitchens-day-header-1');
    await lane.call('click_element', { id: 'kitchens-day-header-1' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'kitchens-day-body-1'),
      'ladle: collapsing a reset hides its instructions',
    );
    await lane.call('click_element', { id: 'kitchens-day-header-1' });
    await waitForNode(lane, 'kitchens-day-body-1', 'ladle: expanding a reset shows its instructions');
    await waitForNode(lane, 'kitchens-day-copy-1', 'ladle: the reset instructions are copyable');

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

    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'statue: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: statueSave() });
    await waitForNode(lane, 'import-format', 'statue: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'kpi-retention') ?? '').includes('19.2%'),
      'statue: the STARTUE bubble sets the EXP retention',
    );
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'kpi-tier') ?? '').includes('1.2x'),
      'statue: the Onyx tier multiplies deposits',
    );

    const guaranteed = labelOf(await lane.tree(), 'statue-needed-0');
    await lane.call('focus_element', { id: 'statue-estimate' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => labelOf(nodes, 'statue-needed-0') !== guaranteed,
      'statue: the deposit estimate changes the statues needed',
    );

    await lane.call('click_element', { id: 'statue-rate-0' });
    await lane.call('type_text', { text: '1M' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'statue-rate-source') ?? '').includes('manual entries'),
      'statue: the rate source names the manual entries',
    );
    await waitFor(
      lane,
      (nodes) => {
        const time = labelOf(nodes, 'statue-time-0') ?? '';
        return time.includes('Time to target') && !time.includes('Add rate') && !time.endsWith('—');
      },
      'statue: the farming rate produces a time estimate',
    );

    // Set all targets rejects a value that is not a whole level, applies one
    // level to every row, and undo puts the replaced targets back.
    await lane.call('click_element', { id: 'statue-set-all' });
    await lane.call('type_text', { text: '12.5' });
    await lane.call('click_element', { id: 'statue-apply-all' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'statue-batch-error') ?? '').includes(
          'Enter a nonnegative whole target level.',
        ),
      'statue: a fractional set-all level is rejected',
    );
    await lane.call('click_element', { id: 'statue-set-all' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: '520' });
    await lane.call('click_element', { id: 'statue-apply-all' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'statue-batch-status') ?? '').includes('All 32 targets set to Lv 520') &&
        !has(nodes, 'statue-batch-error'),
      'statue: set all targets applies one level to every statue',
    );
    await waitForNode(lane, 'statue-undo-batch', 'statue: the batch action offers an undo');
    await lane.call('click_element', { id: 'statue-undo-batch' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'statue-batch-status') ?? '') === 'Undid set all targets.' &&
        !has(nodes, 'statue-undo-batch'),
      'statue: undo restores the targets the batch replaced',
    );
    await lane.call('click_element', { id: 'statue-next-level' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'statue-batch-status') ?? '').includes(
          'Targets follow each saved level plus one',
        ),
      'statue: next level for all clears the entered targets',
    );

    // The farming breakdown renders through the shared formula tree and
    // follows its statue selector.
    await waitForNode(lane, 'statue-breakdown-tree', 'statue: the farming breakdown renders');
    const breakdownRoot = labelOf(await lane.tree(), 'formula-node-root');
    await lane.call('focus_element', { id: 'statue-breakdown-statue' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'formula-node-root') ?? '') !== breakdownRoot &&
        (labelOf(nodes, 'formula-node-root') ?? '').includes('statues needed'),
      'statue: the farming breakdown follows the selected statue',
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

    // Every upstream category has a tab, rows and a summary count; clicking
    // each tab swaps the panel to that category.
    await waitForNode(lane, 'stats-category-summary', 'stats-all: the category summary renders');
    const summary = labelOf(await lane.tree(), 'stats-category-summary') ?? '';
    const categories = [
      ['combat', 'Combat'],
      ['stat', 'Stats'],
      ['multiplier', 'Multipliers'],
      ['rate', 'Rates'],
      ['capacity', 'Capacity'],
      ['construction', 'Construction'],
      ['research', 'Research'],
      ['currency', 'Currency'],
      ['progression', 'Progression'],
    ];
    for (const [key, label] of categories) {
      if (!summary.includes(label)) {
        throw new Error(`stats-all: ${label} is missing from the summary: ${summary}`);
      }
      await lane.call('click_element', { id: `stats-${key}-tab` });
      await waitFor(
        lane,
        (nodes) => (labelOf(nodes, 'stats-category-panel') ?? '').startsWith(`${label} breakdowns:`),
        `stats-all: the ${label} tab renders its rows`,
      );
    }

    // The capacity rows resolve against the save and show their formula tree.
    await lane.call('click_element', { id: 'stats-capacity-tab' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-descriptor-summary') ?? '').includes('Mining Carry Capacity') &&
        (labelOf(nodes, 'stats-descriptor-summary') ?? '').includes('35,000'),
      'stats-all: the capacity row resolves from the save',
    );
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'formula-node-root') ?? '').includes('Mining Carry Capacity'),
      'stats-all: the capacity row shows its formula tree',
    );

    // A combat row has no ported formula yet, so it warns with its source.
    await lane.call('click_element', { id: 'stats-combat-tab' });
    await lane.call('click_element', { id: 'stats-row-drop-rate' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'stats-descriptor-warning') ?? '').includes('drop-rate.js'),
      'stats-all: an unported row names its upstream source',
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
  'drop-rate': async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'drop-rate: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: dropRateSave() });
    await waitForNode(lane, 'import-format', 'drop-rate: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    // The save stands inside a dungeon, so the runtime panel appears and the
    // total waits for both live values.
    await waitForNode(
      lane,
      'drop-rate-dungeon-runtime',
      'drop-rate: the dungeon inputs appear on a dungeon map',
    );
    const note = labelOf(await lane.tree(), 'drop-rate-context-note') ?? '';
    if (!note.includes('live dungeon inputs apply')) {
      throw new Error(`drop-rate: the context note reads ${note}`);
    }
    const waiting = labelOf(await lane.tree(), 'drop-rate-total-value') ?? '';
    if (waiting !== 'Unavailable') {
      throw new Error(`drop-rate: the total without live values reads ${waiting}`);
    }
    await lane.call('click_element', { id: 'drop-rate-luk' });
    await lane.call('type_text', { text: '1200' });
    await lane.call('click_element', { id: 'drop-rate-drop' });
    await lane.call('type_text', { text: '35' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'drop-rate-total-value') ?? '') === '13.35x',
      'drop-rate: the total is the upstream dungeon formula',
    );
    // A normal map hides the runtime panel and names the missing engine.
    await lane.call('focus_element', { id: 'drop-rate-map' });
    for (let step = 0; step < 2; step++) await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'drop-rate-dungeon-runtime'),
      'drop-rate: leaving the dungeon hides the live inputs',
    );
    await waitFor(
      lane,
      (nodes) => {
        const label = labelOf(nodes, 'drop-rate-total-sub') ?? '';
        return label.includes('shared stat pipeline');
      },
      'drop-rate: a normal map names the engine this build lacks',
    );
  },
  sailing: async (lane) => {
    await waitForNode(lane, 'fleet-import', 'sailing: the empty state offers the import');
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-drop', 'sailing: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    // The paste box keeps whatever the last import typed into it.
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: sailingSave() });
    await waitForNode(lane, 'import-format', 'sailing: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'fleet-mode', 'sailing: the fleet assignment opens after Apply');
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
    await lane.call('click_element', { id: 'ships' });
    await waitForNode(lane, 'ships-table', 'sailing: the ship breakpoints open');
    await waitFor(
      lane,
      (nodes) => nodes.some((node) => node.id?.startsWith('ships-target-')),
      'sailing: the breakpoint rows render',
    );
    await lane.call('click_element', { id: 'ships-sort-eta' });
    await lane.call('click_element', { id: 'captains' });
    await waitForNode(lane, 'captains-missing', 'sailing: the captain flags open');
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'captains-missing') ?? '').length > 0,
      'sailing: the crew card names the missing captains',
    );
    await lane.call('click_element', { id: 'replacements' });
    await waitForNode(lane, 'replacements-strongest', 'sailing: the replacement matrix opens');
    await waitFor(
      lane,
      (nodes) => nodes.some((node) => node.id?.startsWith('replacements-cell-')),
      'sailing: the matrix cells render',
    );
    await lane.call('click_element', { id: 'odds' });
    await waitForNode(lane, 'odds-fleet-table', 'sailing: the chest outcomes open');
    await waitFor(
      lane,
      (nodes) => nodes.some((node) => node.id?.startsWith('odds-tier-')),
      'sailing: the fleet chest tier rows render',
    );
    await waitForNode(lane, 'odds-pile-table', 'sailing: the near-full pile transitions render');
    await lane.call('click_element', { id: 'artifacts' });
    await waitForNode(lane, 'artifacts-island-table', 'sailing: the artifact ranking opens');
    await waitFor(
      lane,
      (nodes) => nodes.some((node) => node.id?.startsWith('artifact-name-')),
      'sailing: the selected island artifacts render',
    );
    await lane.call('click_element', { id: 'simulation' });
    await waitForNode(lane, 'sailing-sim-panel', 'sailing: the pile simulation opens');
    await lane.call('click_element', { id: 'sailing-sim-run' });
    await waitForNode(
      lane,
      'sailing-sim-currency-table',
      'sailing: the pile simulation finishes with its currency spread',
    );
    await waitFor(
      lane,
      (nodes) => nodes.some((node) => node.id?.startsWith('sailing-sim-slot-')),
      'sailing: the pile position table renders',
    );
    await lane.call('click_element', { id: 'formula' });
    await waitForNode(lane, 'sailing-breakdown-loot', 'sailing: the formula breakdowns open');
    await waitForNode(lane, 'sailing-breakdown-chest', 'sailing: the chest tree renders');
    await lane.call('click_element', { id: 'fleet' });
    await waitForNode(lane, 'fleet-table', 'sailing: the fleet table reopens');
  },
  monument: async (lane) => {
    await lane.call('click_element', { id: 'sources' });
    await waitForNode(lane, 'monument-pane-sources', 'monument: the bonus sources tab opens');
    await lane.call('click_element', { id: 'bravery' });
    await waitForNode(lane, 'monument-pane-bravery', 'monument: the bravery tab reopens');
    // A save with the Monument rows fills the Bravery cards and the odds.
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'monument: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: monumentSave() });
    await waitForNode(lane, 'import-format', 'monument: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'monument-controls', 'monument: the run state opens after Apply');
    const damage = labelOf(await lane.tree(), 'monument-odds-damage') ?? '';
    if (!damage.includes('3.0 - 25.0')) {
      throw new Error(`monument: the roll damage reads ${damage}`);
    }
    // A damage boost multiplies the roll, so the odds card follows the stepper.
    await lane.call('click_element', { id: 'monument-boosts-up' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'monument-odds-damage') ?? '').includes('3.0 - 27.5'),
      'monument: a boost stack updates the odds',
    );
  },
  cog: async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'cog: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: cogSave() });
    await waitForNode(lane, 'import-format', 'cog: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'cog-board', 'cog: the board renders after Apply');
    const insights = labelOf(await lane.tree(), 'cog-insights') ?? '';
    check(
      'cog: the insights name the weakest and strongest cogs',
      insights.includes('Weakest') && insights.includes('Strongest'),
      insights,
    );
    const ranges = labelOf(await lane.tree(), 'cog-roll-ranges') ?? '';
    check(
      'cog: the roll ranges use the effective construction level',
      /^Roll ranges at level \d+/.test(ranges),
      ranges,
    );
    const shelf = labelOf(await lane.tree(), 'cog-shelf') ?? '';
    check('cog: the shelf is read-only', shelf.includes('read-only'), shelf);
    await lane.call('click_element', { id: 'cog-optimize' });
    await waitForNode(lane, 'cog-result', 'cog: optimize produces a result', 60000);
    const result = labelOf(await lane.tree(), 'cog-result') ?? '';
    check('cog: the result names its swaps', result.includes('swaps'), result);
    // A finished run opens the walkthrough at its first step: Next applies it,
    // Previous restores the prior board and Done keeps the optimized one.
    await waitForNode(lane, 'cog-swap-next', 'cog: the walkthrough opens');
    const firstStep = labelOf(await lane.tree(), 'cog-swap-progress') ?? '';
    await lane.call('click_element', { id: 'cog-swap-next' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'cog-swap-progress') ?? '') !== firstStep,
      'cog: Next applies the first swap',
    );
    const secondStep = labelOf(await lane.tree(), 'cog-swap-progress') ?? '';
    await lane.call('click_element', { id: 'cog-swap-prev' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'cog-swap-progress') ?? '') === firstStep,
      'cog: Previous restores the prior board',
    );
    check('cog: the second step differs from the first', secondStep !== firstStep);
    for (let step = 0; !has(await lane.tree(), 'cog-swap-done'); step += 1) {
      if (step > 200) throw new Error('cog: the walkthrough never reaches Done');
      const before = labelOf(await lane.tree(), 'cog-swap-progress') ?? '';
      await lane.call('click_element', { id: 'cog-swap-next' });
      await waitFor(
        lane,
        (nodes) => (labelOf(nodes, 'cog-swap-progress') ?? '') !== before,
        `cog: walkthrough step ${step + 2}`,
      );
    }
    await lane.call('click_element', { id: 'cog-swap-done' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'cog-walkthrough') && has(nodes, 'cog-result'),
      'cog: Done keeps the final board and closes the walkthrough',
    );
    await lane.call('click_element', { id: 'cog-reset' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'cog-result') && has(nodes, 'cog-board'),
      'cog: reset restores the save board and drops the result',
    );

    // Custom weights replace the single goal. Set build-only weights and check
    // the result names the weighted objective.
    await lane.call('click_element', { id: 'cog-weighted' });
    await waitForNode(lane, 'cog-weight-build', 'cog: custom weights show their fields');
    const typeWeight = async (id, value) => {
      await lane.call('click_element', { id });
      await lane.call('keyboard', { keystroke: 'secondary-a' });
      await lane.call('type_text', { text: value });
    };
    await typeWeight('cog-weight-conexp', '0');
    await typeWeight('cog-weight-flaggy', '0');
    await typeWeight('cog-weight-build', '100');
    const weightValue = (nodes, label) =>
      nodes.find((node) => node.role === 'text_input' && node.label === label)?.value?.value ?? '';
    await waitFor(
      lane,
      (nodes) => weightValue(nodes, 'Build weight') === '100',
      'cog: the build weight reads 100',
    );
    await lane.call('click_element', { id: 'cog-optimize' });
    await waitForNode(lane, 'cog-result', 'cog: a weighted optimize produces a result', 60000);
    const weighted = labelOf(await lane.tree(), 'cog-result') ?? '';
    check('cog: the result names the weighted objective', weighted.includes('Custom weights'), weighted);
    await lane.call('click_element', { id: 'cog-reset' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'cog-result') && has(nodes, 'cog-board'),
      'cog: reset drops the weighted result',
    );

    // All-zero weights are refused: the banner names the reason and Optimize
    // carries no click action.
    await typeWeight('cog-weight-build', '0');
    await waitForNode(lane, 'cog-weight-error', 'cog: all-zero weights are rejected');
    const error = labelOf(await lane.tree(), 'cog-weight-error') ?? '';
    check('cog: the error names the reason', error.includes('above zero'), error);
    const optimize = (await lane.tree()).find((node) => node.id === 'cog-optimize');
    check('cog: invalid weights disable Optimize', !(optimize?.actions ?? []).includes('click'));
    await typeWeight('cog-weight-build', '100');
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'cog-weight-error'),
      'cog: valid weights clear the error',
    );

    // The perfect board explains its layers.
    await lane.call('click_element', { id: 'cog-perfect' });
    await waitForNode(lane, 'cog-perfect-result', 'cog: the perfect board is built', 120000);
    await waitForNode(lane, 'cog-perfect-layers', 'cog: the perfect board lists its layers');
    const perfect = labelOf(await lane.tree(), 'cog-perfect-result') ?? '';
    check(
      'cog: the perfect board explains the player and its cogs',
      perfect.includes('player at') && perfect.includes('Yang'),
      perfect,
    );
    await lane.call('click_element', { id: 'cog-reset' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'cog-perfect-result') && has(nodes, 'cog-board'),
      'cog: reset drops the perfect board',
    );
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
  cglunko: async (lane) => {
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
    await importSave(cglunkoSave(), 'cglunko-overview', 'cglunko');
    const summary = labelOf(await lane.tree(), 'cglunko-summary') ?? '';
    if (!summary.includes('DR ×2.58') || !summary.includes('5 blue digits')) {
      throw new Error(`cglunko: the summary reads ${summary}`);
    }
    // The shared tree opens on its root; collapsing hides the factors and
    // expanding brings them back, with the additive group summing its three
    // drop-rate upgrades to +30%.
    await lane.call('click_element', { id: 'cglunko-formula-collapse' });
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'formula-node-root-0'),
      'cglunko: collapse closes the tree',
    );
    await lane.call('click_element', { id: 'cglunko-formula-expand' });
    await waitForNode(lane, 'formula-node-root-0-0', 'cglunko: expand reopens the additive group');
    const additive = labelOf(await lane.tree(), 'formula-node-root-0-0') ?? '';
    if (!additive.includes('+30%')) {
      throw new Error(`cglunko: the additive group reads ${additive}`);
    }
    // A save without the Cove rows reads zero and names the missing field.
    await importSave(cglunkoSave({ rows: false }), 'cglunko-missing', 'cglunko');
    const empty = labelOf(await lane.tree(), 'cglunko-summary') ?? '';
    if (!empty.includes('DR ×1.00')) throw new Error(`cglunko: the empty summary reads ${empty}`);
  },
  clamworks: async (lane) => {
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
    await importSave(clamworksSave(), 'clamworks-overview', 'clamworks');
    const summary = labelOf(await lane.tree(), 'clamworks-summary') ?? '';
    if (!summary.includes('5.00M')) {
      throw new Error(`clamworks: the imported balance reads ${summary}`);
    }
    await lane.call('click_element', { id: 'clamworks-multikill-more' });
    await lane.call('click_element', { id: 'upgrades' });
    await waitForNode(lane, 'clamworks-upgrades', 'clamworks: the upgrades tab opens');
    // The AFK claim tab: an entry outside the credited range is refused with the
    // upstream message, and a valid one shows the claim and its comparison.
    await lane.call('click_element', { id: 'claim' });
    await waitForNode(lane, 'clamworks-claim', 'clamworks: the claim tab opens');
    await lane.call('click_element', { id: 'clamworks-claim-hours' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: '60000' });
    await lane.call('click_element', { id: 'clamworks-claim-calculate' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'clamworks-claim-error') ?? '').includes(
          'Enter AFK hours between 0 and 55,555.',
        ),
      'clamworks: hours outside the credited range are refused',
    );
    await lane.call('click_element', { id: 'clamworks-claim-hours' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: '8' });
    await lane.call('click_element', { id: 'clamworks-claim-calculate' });
    await waitFor(
      lane,
      (nodes) =>
        !has(nodes, 'clamworks-claim-error') && has(nodes, 'clamworks-claim-pearls'),
      'clamworks: a valid entry shows the claim',
    );
    await waitForNode(
      lane,
      'clamworks-compare-baseline',
      'clamworks: the comparison lists its baseline',
    );
    // The optimizer spends the imported balance and the applied plan's levels
    // take over the section until the saved levels are shown again.
    await lane.call('click_element', { id: 'optimizer' });
    await waitForNode(lane, 'clamworks-optimizer', 'clamworks: the optimizer tab opens');
    await lane.call('click_element', { id: 'run-clamworks-plan' });
    await waitForNode(lane, 'clamworks-plan-result', 'clamworks: the plan lists its purchases');
    await lane.call('click_element', { id: 'apply-clamworks-plan' });
    await waitForNode(
      lane,
      'clamworks-projection',
      'clamworks: applying the plan previews its levels',
    );
    await lane.call('click_element', { id: 'promotion' });
    await waitForNode(lane, 'clamworks-promotion-pane', 'clamworks: the promotion tab opens');
    await waitFor(
      lane,
      (nodes) =>
        Array.from({ length: 12 }, (_, index) => index + 1).every((level) =>
          has(nodes, `clamworks-reward-${level}`),
        ),
      'clamworks: every worker-class reward is listed',
    );
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
const valueOf = (nodes, id) => nodes.find((node) => node.id === id)?.value?.value ?? null;

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

/// Scroll the pane until an element's bounds sit inside the viewport: a click
/// goes through coordinates, so an element scrolled past the edge cannot be hit.
const bringIntoView = async (lane, id) => {
  for (let attempt = 0; attempt < 8; attempt += 1) {
    const node = (await lane.tree()).find((candidate) => (candidate.id ?? '').endsWith(id));
    const y = node?.bounds?.y ?? 0;
    if (y > 160 && y < 700) return;
    await lane.call('scroll', { x: 600, y: 500, delta_y: y < 160 ? -300 : 300 });
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
};

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

/// The project round trip: export opens over the window state with both the
/// project document and the state string, then the project document, the
/// exported state string and a section config each apply and leave a visible
/// mark.
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

    // The state string half of the export: a hand-set supplement travels in the
    // ROv2 string and comes back with it, and a reset restores what the last
    // applied document set rather than the build's defaults.
    await lane.call('click_element', { id: 'rates' });
    await waitForNode(lane, 'supp-shapes', 'state: the supplements card opens');
    await lane.call('click_element', { id: 'supp-shapes-stepper-more' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'supp-shapes') ?? '').includes(': 1'),
      'state: the extra-shape supplement reads 1',
    );
    await lane.call('click_element', { id: 'supp-shapes-stepper-more' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'supp-shapes') ?? '').includes(': 2'),
      'state: the extra-shape supplement reads 2',
    );
    await lane.call('click_element', { id: 'header-export' });
    await waitForNode(
      lane,
      'export-state-text',
      'state: the export dialog carries the ROv2 string',
    );
    const encoded = labelOf(await lane.tree(), 'export-state-text') ?? '';
    check(
      'state: the export string is an ROv2 payload',
      encoded.startsWith('ROv2:'),
      encoded.slice(0, 32),
    );
    await lane.call('click_element', { id: 'export-copy-state' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'status-status') ?? '').includes('ROv2 state string'),
      'state: copy state closes the dialog and names the string',
    );
    await lane.call('click_element', { id: 'supp-reset' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'supp-shapes') ?? '').includes(': 0'),
      'state: reset drops the hand-set supplement',
    );
    await importText(lane, encoded, 'upstream state', 'state round trip');
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'supp-shapes') ?? '').includes(': 2'),
      'state: the exported state brings its supplements back',
    );
    await lane.call('click_element', { id: 'supp-reset' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'supp-shapes') ?? '').includes(': 2'),
      'state: reset keeps what the imported state established',
    );

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
    await importError(
      lane,
      'ROv2:not-base64!',
      'not base64',
      'bad state string',
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
