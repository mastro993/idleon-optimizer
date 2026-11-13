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

/// A small flat save at research level 42 with every square at level 1, no saved
/// shape overlay and one found observation holding a magnifier: enough for the
/// grid, shape and magnifier editors, whatever save the previous section left.
const researchSave = () => {
  const sparse = (length, filled = {}) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  return JSON.stringify({
    Research: JSON.stringify([
      Array(240).fill(1),
      Array(240).fill(-1),
      sparse(80, { 0: 1 }),
      sparse(80, {}),
      sparse(80, {}),
      [0, 0],
      [],
      // stateR7: floor 4 with currency and a best hit, so the Minehead tab has
      // its boss, balance and farm term.
      [0, 0, 0, 0, 4, 123456, 5000],
      // The thirty Depth Charge upgrade levels.
      sparse(30, { 0: 5, 1: 3, 2: 2, 6: 2, 8: 2, 9: 1, 10: 1, 16: 1, 19: 1 }),
    ]),
    // The Sushi Station rows: six slots cooking, their slot types, the upgrade
    // levels, the fireplace columns, the bank, twelve discovered tiers and
    // their knowledge levels.
    Sushi: JSON.stringify([
      sparse(120, { 0: 0, 1: 3, 2: 5, 3: 2, 4: 7, 5: 1 }).map((tier, slot) =>
        slot < 6 ? tier : -1
      ),
      sparse(120, { 0: 0, 1: 1, 2: 2, 3: 3, 4: 0, 5: 1 }).map((type, slot) =>
        slot < 6 ? type : -1
      ),
      sparse(46, {
        0: 5, 1: 2, 2: 2, 3: 1, 6: 2, 8: 3, 9: 2, 10: 1, 11: 1, 12: 1, 21: 1,
        30: 1, 37: 2, 40: 1, 41: 1,
      }),
      sparse(15, { 0: 0, 1: 1, 2: 3, 3: 0, 4: 2 }),
      sparse(20, { 1: 0, 2: 4, 3: 10000, 8: 2400 }),
      Array.from({ length: 12 }, (_, tier) => (tier === 0 ? 1 : 0)),
      [],
      Array.from({ length: 12 }, (_, tier) => (tier % 3 === 0 ? 3 : 1)),
    ]),
    Lv0_0: JSON.stringify(sparse(30, { 20: 42 })),
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
  // Real saves write fixed-width rows and a character row, so the account
  // decoders have something to read.
  const holes = Array.from({ length: 35 }, () => row(150, {}));
  // Three types bought; Green (index 2) holds the next type's price.
  holes[9] = row(150, { 20: 500, 22: 10000 });
  // Types owned, jars produced of the first three, and the banked doubler.
  holes[11] = row(150, { 37: 2, 40: 1234, 41: 567, 42: 89, 60: 100 });
  // Production line, page two and the requirement reduction.
  holes[13] = row(150, { 67: 1, 74: 1, 75: 1 });
  // A few found collectibles across both pages.
  holes[24] = row(150, { 0: 3, 1: 2, 3: 4, 17: 2, 28: 1, 36: 5 });
  holes[25] = row(150, { 0: 1, 3: 250 });
  return JSON.stringify({
    Research: JSON.stringify([row(32, {})]),
    Lv0_0: JSON.stringify(row(30, {})),
    Holes: JSON.stringify(holes),
  });
};

/// A small flat save for the Farming planner: six planted plots across the
/// families, one evolution-blocked, Land Rank ranks that unlock the allocation's
/// candidate rows and leave points unspent, night market 5 unlocking Land Rank
/// XP, and twelve discovered crops so the assignment has candidates.
const farmingSave = () => {
  const row = (length, filled = {}) =>
    Array.from({ length }, (_, index) => filled[index] ?? 0);
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
/// damage range for a boost stack to move. One Monument AFK arcade level makes
/// the additive training pool non-zero, so the source list has a total to prove.
const monumentSave = () => {
  const zeros = (length) => Array(length).fill(0);
  const holes = Array.from({ length: 33 }, () => []);
  holes[7] = zeros(4);
  holes[11] = zeros(70);
  // A day of saved reward charge so the Wisdom tab has a multiplier to show.
  holes[11][13] = 86400;
  holes[13] = zeros(100);
  holes[14] = [600, 2];
  holes[15] = zeros(30);
  holes[22] = zeros(20);
  holes[24] = zeros(40);
  holes[26] = zeros(30);
  const arcade = zeros(60);
  arcade[42] = 1;
  return JSON.stringify({
    Research: '[]',
    Holes: JSON.stringify(holes),
    ArcadeUpg: JSON.stringify(arcade),
  });
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
/// overview's DR tree multiplies out to 2.57712. Three levels of One Big Blue
/// focus the third blue stone at ×30, which the Stones tab recommends.
/// `rows: false` shortens the row past index 630, which must read zero and warn.
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
    options[639] = 3; // One Big Blue: 10 per level, so the focus is ×30.
    options[640] = 4; // Blue Collector: 1 per level.
    options[641] = 4; // Researchy: 1 per level.
    options[644] = 3; // Cavernicus: 10 per level.
    options[645] = 2; // Multikill: 100 per level.
    options[649] = 6; // DeEmGe: 1 per level.
    options[650] = 2; // Grandioso: 5 per level.
    options[654] = 1000; // Blue shape 1: three digits.
    options[655] = 100; // Blue shape 2: two digits.
    options[666] = 3; // Blue focus: the third stone.
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
    // Row 0 is the yellow points each meal holds; rows 1 and 2 are the rank and
    // the EXP upgrades. Turkey of Thank already holds two of them.
    CookMaster: JSON.stringify([
      row(90, { 0: 2 }),
      [18, 0, 0, 1000],
      [7, 3, 0, 0, 0, 0],
      [0, 0],
    ]),
    // Two Skill Efficiency meals and two Total Damage meals, so both goals are
    // offered and the select has a second option to switch to.
    Meals: JSON.stringify([row(90, { 0: 100, 6: 100, 27: 100, 42: 100, 73: 150 })]),
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
  const row = (length, filled = {}) =>
    Array.from({ length }, (_, index) => filled[index] ?? 0);
  // The capacity inputs sit at zero: the rows scale by them, so the saved value
  // stays the bag row's own 35,000 until the harness edits one.
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
    UpgVault: JSON.stringify(row(12)),
    GemItemsPurchased: JSON.stringify(row(59)),
    Guild: JSON.stringify([row(3)]),
    SL_0: JSON.stringify(row(100)),
    SM_0: JSON.stringify(row(100, { 78: 200 })),
    StampLv: JSON.stringify([row(2), row(23), row(2)]),
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
  const row = (length, filled = {}) =>
    Array.from({ length }, (_, index) => filled[index] ?? 0);
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
/// run and the palette has every cell type to place. The Feverizer, Stronkroid
/// and automatic revives are unlocked and the stage is deep enough that the
/// normal phase cannot clear it, so the simulator has its controls and a
/// critical timeline to render.
const jellySave = () => {
  const row = (length, filled = {}) =>
    Array.from({ length }, (_, index) => filled[index] ?? 0);
  const research = Array.from({ length: 19 }, () => []);
  research[7] = row(15, { 9: 32, 10: 3, 11: 1000, 12: 500, 13: 0, 14: 2000 });
  research[14] = Array.from({ length: 180 }, () => -1);
  research[14][77] = 1;
  research[14][95] = 1;
  research[15] = row(9);
  research[16] = row(9);
  research[17] = row(40, {
    0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 1, 7: 1,
    12: 1, 16: 3, 28: 1, 29: 10, 35: 2, 36: 1, 38: 1,
  });
  research[18] = [];
  return JSON.stringify({
    Research: JSON.stringify(research),
    Lv0_0: JSON.stringify(row(30)),
  });
};

/// A small flat `save.json` carrying the Royal Guardian rows: one built W1
/// outpost with three guards and its resource link, a second outpost the militia
/// is clearing, the Armory levels an overview reads, one graded node with saved
/// progress, a currency balance and two banked hours. The Spelunk, Research and
/// SL rows give the clearing rate its cross-system factors, and `TimeAway`
/// carries the daily reset countdown the drain projection reads.
const royalSave = () => {
  const row = (length, filled) => Array.from({ length }, (_, index) => filled[index] ?? 0);
  const armory = row(83, { 21: 20, 23: 10, 70: 1 });
  const g = Array.from({ length: 24 }, () => []);
  g[1] = row(80, { 0: 1234 });
  g[2] = armory;
  g[3] = row(20, { 0: 7200 });
  g[4] = row(80, { 7: 500 });
  g[5] = row(80, { 7: 4 });
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
    TimeAway: JSON.stringify({ GlobalTime: 1794296810.952, ShopRestock: 14400 }),
    CharacterClass_0: 16,
    Lv0_0: JSON.stringify(row(30, { 0: 1200, 20: 40 })),
    SL_0: JSON.stringify({ 231: 300 }),
  });
};

/// A small flat `save.json` carrying three villagers and the `Holes` rows their
/// EXP rates read: levels, invested opals, stars, cosmo levels, a schematic, a
/// bell, two measurements, a monument slot and a fountain upgrade. One `Lv0_0`
/// row keeps the account at one character: the Prisma plan runs on every import
/// and refuses a save whose character count is zero.
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
  return JSON.stringify({
    Research: '[]',
    Holes: JSON.stringify(holes),
    // The Prisma plan runs on every import and refuses a save with no character.
    Lv0_0: JSON.stringify(Array(30).fill(0)),
  });
};

/// A two-character coin save: targets with and without a monster row, the
/// highest recorded damage and the fields the section's cards read.
const coinSave = () =>
  JSON.stringify({
    Research: '[[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[]]',
    Holes: JSON.stringify(Array.from({ length: 34 }, () => [[], [], []])),
    charNames: ['Ana', 'Bo'],
    Lv0_0: '[1]',
    Lv0_1: '[1]',
    CharacterClass_0: 1,
    CharacterClass_1: 7,
    CurrentMap_0: 2,
    CurrentMap_1: 50,
    AFKtarget_0: 'frogG',
    AFKtarget_1: 'Nothing',
    TaskZZ0: '[[],[500]]',
  });

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
    await bringIntoView(lane, 'spelunk-preset-amber');
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
    // The Amber policy carries its own dump, finisher, reserve, harvest and rope
    // reserve limits; the run explains the phases it actually took.
    const policySelect = (await lane.tree()).find((node) =>
      (node.id ?? '').endsWith('spelunk-policy'),
    );
    if (!policySelect) throw new Error('spelunking: the policy select is missing');
    await lane.call('focus_element', { id: policySelect.id });
    await lane.call('keyboard', { keystroke: 'enter' });
    for (let n = 0; n < 5; n += 1) await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitForNode(lane, 'spelunk-amber-policy', 'spelunking: the Amber policy renders');
    // The seeded run drives the whole expedition through the shared task runner
    // and leaves its depth, loot and event log on the page.
    await bringIntoView(lane, 'spelunk-run-start');
    await lane.call('click_element', { id: 'spelunk-run-start' });
    await waitForNode(
      lane,
      'spelunk-run-depth',
      'spelunking: the expedition run reports a depth',
      60000,
    );
    await waitForNode(
      lane,
      'spelunk-run-event-0',
      'spelunking: the run log lists its first event',
      60000,
    );
    const run = labelOf(await lane.tree(), 'spelunk-run-summary') ?? '';
    if (!run.includes('depth ')) {
      throw new Error(`spelunking: the run summary reads ${run}`);
    }
    await waitForNode(
      lane,
      'spelunk-run-amber-phase',
      'spelunking: the Amber policy explains its phases',
      60000,
    );
    await waitForNode(
      lane,
      'spelunk-run-amber-transition-0',
      'spelunking: the Amber policy lists its phase changes',
      60000,
    );
    const phase = labelOf(await lane.tree(), 'spelunk-run-amber-phase') ?? '';
    if (!phase.includes('Amber policy')) {
      throw new Error(`spelunking: the Amber phase reads ${phase}`);
    }
  },
  'jelly-operator': async (lane) => {
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
    await lane.call('click_element', { id: 'jelly-slot-76' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'jelly-message') ?? '').includes('Placed Amoeba at slot 76'),
      'jelly: the palette places the picked cell',
    );
    // The estimate runs off the UI thread and stores its aggregate.
    await lane.call('click_element', { id: 'jelly-tab-overview' });
    await waitForNode(lane, 'jelly-overview', 'jelly: the overview reopens');
    await lane.call('click_element', { id: 'jelly-run-estimate' });
    await waitForNode(lane, 'jelly-estimate-result', 'jelly: the estimate stores a result', 120000);
    await waitForNode(lane, 'jelly-contribution', 'jelly: the cell contribution renders');
    await waitForNode(lane, 'jelly-obstruction-outlook', 'jelly: the outlook renders');
    // The simulator's sandbox clears, resets and runs one seeded operation.
    await lane.call('click_element', { id: 'jelly-tab-simulator' });
    await waitForNode(lane, 'jelly-sim-board', 'jelly: the simulator board renders');
    await waitForNode(lane, 'jelly-sim-stage', 'jelly: the stage select renders');
    await waitForNode(lane, 'jelly-sim-roid', 'jelly: the Stronkroid timing renders');
    await waitForNode(lane, 'jelly-sim-revive', 'jelly: the revive policy renders');
    await waitForNode(lane, 'jelly-sim-plots-toggle-0', 'jelly: the board sections render');
    await lane.call('click_element', { id: 'jelly-sim-clear' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'jelly-sim-message') ?? '').includes('Removed every cell'),
      'jelly: Clear cells empties the sandbox',
    );
    await lane.call('click_element', { id: 'jelly-sim-reset' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'jelly-sim-message') ?? '').includes('Sandbox reset'),
      'jelly: Reset from save restores the board',
    );
    await lane.call('click_element', { id: 'jelly-sim-run' });
    await waitForNode(lane, 'jelly-sim-result', 'jelly: the simulator stores a run', 120000);
    await waitForNode(lane, 'jelly-sim-contribution', 'jelly: the run contribution renders');
    await waitForNode(lane, 'jelly-sim-timeline', 'jelly: the critical timeline renders');
    // The section planner screens every section, then simulates the finalists.
    await lane.call('click_element', { id: 'jelly-tab-sections' });
    await waitForNode(lane, 'jelly-sections', 'jelly: the sections planner opens');
    await waitForNode(lane, 'jelly-sections-strategy', 'jelly: the strategy select renders');
    await waitForNode(lane, 'jelly-sections-objective', 'jelly: the objective select renders');
    await waitForNode(lane, 'jelly-sections-tokens', 'jelly: the token input renders');
    await waitForNode(lane, 'jelly-sections-quality', 'jelly: the quality select renders');
    await waitForNode(lane, 'jelly-sections-beam', 'jelly: the beam input renders');
    await waitForNode(lane, 'jelly-sections-seed', 'jelly: the seed input renders');
    await lane.call('click_element', { id: 'jelly-sections-run' });
    await waitForNode(lane, 'jelly-sections-result', 'jelly: the plan stores a result', 180000);
    await waitForNode(lane, 'jelly-sections-recommended', 'jelly: the recommendation renders');
    await waitForNode(lane, 'jelly-sections-steps', 'jelly: the purchase sequence renders');
    await waitForNode(lane, 'jelly-sections-final-board', 'jelly: the recommended board renders');
    await waitForNode(lane, 'jelly-sections-candidates', 'jelly: the ranked sequences render');
    await waitForNode(lane, 'jelly-sections-individual', 'jelly: every individual section renders');
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
    // The outpost map draws the built outpost, the node it drains and the line
    // between them, and clicking the node opens the resources pane on it.
    await lane.call('click_element', { id: 'royal-tab-outposts' });
    await waitForNode(lane, 'royal-map', 'royal-guardian: the outpost map renders');
    await waitForNode(
      lane,
      'royal-map-outpost-1',
      'royal-guardian: the map draws the built outpost',
    );
    await waitForNode(lane, 'royal-map-resource-7', 'royal-guardian: the map draws its node');
    await bringIntoView(lane, 'royal-map-resource-7');
    await lane.call('click_element', { id: 'royal-map-resource-7' });
    await waitForNode(
      lane,
      'royal-resource-detail',
      'royal-guardian: the node opens the resources pane',
    );
    const node = labelOf(await lane.tree(), 'royal-resource-detail') ?? '';
    if (!node.includes('Resource 8') || !node.includes('grade 4')) {
      throw new Error(`royal-guardian: the node inspector reads ${node}`);
    }
    // Two banked hours and four hours to the reset come from the save itself.
    const banked = labelOf(await lane.tree(), 'royal-kpi-banked-figure') ?? '';
    if (!banked.includes('2')) throw new Error(`royal-guardian: the banked hours read ${banked}`);
    const reset = labelOf(await lane.tree(), 'royal-kpi-reset-figure') ?? '';
    if (!reset.includes('4')) throw new Error(`royal-guardian: the reset card reads ${reset}`);
    // The currency pane opens on the slot the save banked.
    await lane.call('click_element', { id: 'royal-tab-currencies' });
    await waitForNode(
      lane,
      'royal-currency-summary',
      'royal-guardian: the currency pane renders',
    );
    const balance = labelOf(await lane.tree(), 'royal-kpi-currency-balance-figure') ?? '';
    if (!balance.includes('1,234')) {
      throw new Error(`royal-guardian: the currency balance reads ${balance}`);
    }
    // RG-03: the upgrade list prices the shelves and plans their next levels.
    await lane.call('click_element', { id: 'royal-tab-upgrades' });
    await waitForNode(lane, 'royal-upgrade-armory', 'royal-guardian: the upgrade list renders');
    await waitForNode(lane, 'royal-armory-table', 'royal-guardian: every shelf is listed');
    // The save's 31 Armory levels reach the first seven shelves.
    const shelves = labelOf(await lane.tree(), 'royal-kpi-armory-shelves-figure') ?? '';
    if (!shelves.includes('7')) throw new Error(`royal-guardian: the shelf KPI reads ${shelves}`);
    const plan = labelOf(await lane.tree(), 'royal-armory-plan') ?? '';
    if (!plan.includes('level')) throw new Error(`royal-guardian: the cost plan reads ${plan}`);
    await bringIntoView(lane, 'royal-armory-row-0');
    await lane.call('click_element', { id: 'royal-armory-row-0' });
    await waitForNode(lane, 'royal-armory-plan-table', 'royal-guardian: a shelf prices its next levels');
    // The Orblet list switches in and counts what the save owns.
    await lane.call('focus_element', { id: 'royal-upgrade-view' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitForNode(lane, 'royal-orblet-table', 'royal-guardian: the orblet list renders');
    const orblets = labelOf(await lane.tree(), 'royal-kpi-orblets-owned-figure') ?? '';
    if (!orblets.includes('0')) throw new Error(`royal-guardian: the orblet count reads ${orblets}`);
    // The breakdown tree opens on the collection rule and switches to range.
    await lane.call('click_element', { id: 'royal-tab-breakdowns' });
    await waitForNode(
      lane,
      'royal-breakdown-node-root',
      'royal-guardian: the breakdown tree renders'
    );
    const collection = labelOf(await lane.tree(), 'royal-breakdown-summary') ?? '';
    if (!collection.includes('Collection')) {
      throw new Error(`royal-guardian: the breakdown summary reads ${collection}`);
    }
    await lane.call('click_element', { id: 'royal-breakdown-range' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'royal-breakdown-summary') ?? '').includes('Range'),
      'royal-guardian: the range breakdown replaces the collection one'
    );
    // The forecast commits a menu-open window: no Armory 30, so only a window collects.
    await lane.call('click_element', { id: 'royal-tab-forecast' });
    await waitForNode(lane, 'royal-forecast-summary', 'royal-guardian: the forecast summary renders');
    const empty = labelOf(await lane.tree(), 'royal-kpi-forecast-collects-figure') ?? '';
    if (!empty.includes('0')) throw new Error(`royal-guardian: the collect count reads ${empty}`);
    await lane.call('click_element', { id: 'royal-forecast-window-start' });
    await lane.call('type_text', { text: '1' });
    await lane.call('click_element', { id: 'royal-forecast-window-end' });
    await lane.call('type_text', { text: '2' });
    await lane.call('click_element', { id: 'royal-forecast-add-window' });
    await waitForNode(lane, 'royal-forecast-window-0', 'royal-guardian: the window joins the agenda');
    await lane.call('click_element', { id: 'royal-forecast-collect' });
    await lane.call('type_text', { text: '0.5' });
    await lane.call('click_element', { id: 'royal-forecast-add-collect' });
    await waitForNode(lane, 'royal-forecast-collect-0', 'royal-guardian: the collect joins the agenda');
    const refills = labelOf(await lane.tree(), 'royal-kpi-forecast-refills-figure') ?? '';
    if (!refills.includes('Refills')) {
      throw new Error(`royal-guardian: the forecast refill card reads ${refills}`);
    }
    await lane.call('click_element', { id: 'royal-tab-overview' });
    await waitForNode(lane, 'royal-overview', 'royal-guardian: the overview reopens');
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
    await lane.call('click_element', { id: 'collectibles' });
    await waitForNode(lane, 'jar-collectibles', 'jar: the collectibles tab opens');
    await waitForNode(lane, 'jar-collectible-grid', 'jar: the collectible tiles render');
    await waitForNode(lane, 'jar-collectible-priority', 'jar: the enchant priority renders');
    await lane.call('click_element', { id: 'upgrades' });
    await waitForNode(lane, 'jar-upgrades', 'jar: the upgrades tab opens');
    await waitForNode(lane, 'jar-schematic-groups', 'jar: the schematics render');
    await waitForNode(lane, 'jar-type-plan', 'jar: the jar type plan renders');
    await waitForNode(lane, 'jar-gilded-tiers', 'jar: the gilded tiers render');
    const affordable = labelOf(await lane.tree(), 'jar-upgrade-status') ?? '';
    if (!affordable.includes('Yes')) {
      throw new Error(`jar: the upgrade plan does not budget the next type (${affordable})`);
    }
    await lane.call('click_element', { id: 'nexttier' });
    await waitForNode(lane, 'jar-next-tier', 'jar: the next tier tab opens');
    await waitForNode(lane, 'jar-next-milestones', 'jar: the milestone ranking renders');
    const maxTier = labelOf(await lane.tree(), 'jar-next-projections') ?? '';
    if (!maxTier.includes('Max tier')) {
      throw new Error(`jar: the max tier is not labelled (${maxTier})`);
    }
    await waitForNode(lane, 'jar-next-recommendation', 'jar: the next tier recommendation renders');
    const recommendation = labelOf(await lane.tree(), 'jar-next-recommendation') ?? '';
    if (!recommendation.includes('fastest')) {
      throw new Error(`jar: the next tier recommendation is not actionable (${recommendation})`);
    }
    // JR-04: the enchant simulator runs off the UI thread and repeats exactly
    // for the same seed, and the history records the applied save and clears.
    await lane.call('click_element', { id: 'enchantsim' });
    await waitForNode(lane, 'jar-enchant-sim', 'jar: the enchant simulator opens');
    await waitForNode(lane, 'jar-sim-controls', 'jar: the simulator controls render');
    await waitForNode(lane, 'jar-sim-target-0', 'jar: the target grid lists the found collectibles');
    await scrollTo(lane, 'jar-sim-run');
    await lane.call('click_element', { id: 'jar-sim-run' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'jar-sim-result-line') ?? '').includes('seed 1'),
      'jar: the simulation reports its seed and average'
    );
    const first = labelOf(await lane.tree(), 'jar-sim-result-line') ?? '';
    await waitForNode(lane, 'jar-sim-spread', 'jar: the spread table renders');
    await waitForNode(lane, 'jar-sim-odds', 'jar: the enchant odds table renders');
    const odds = labelOf(await lane.tree(), 'jar-sim-odds') ?? '';
    if (!odds.includes('average enchant levels')) {
      throw new Error(`jar: the odds table names no simulated average (${odds})`);
    }
    await scrollTo(lane, 'jar-sim-run');
    await lane.call('click_element', { id: 'jar-sim-run' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'jar-sim-result-line') ?? '') === first,
      'jar: the same seed reproduces the same spread'
    );
    await lane.call('click_element', { id: 'enchanthistory' });
    await waitForNode(lane, 'jar-enchant-history', 'jar: the enchant history opens');
    const recorded = labelOf(await lane.tree(), 'jar-history-snapshots') ?? '';
    if (!/Snapshots: [1-9]/.test(recorded)) {
      throw new Error(`jar: the imported save left no snapshot (${recorded})`);
    }
    await waitForNode(lane, 'jar-history-count', 'jar: the history counts its snapshots');
    await waitForNode(lane, 'jar-history-spread-0', 'jar: the current spread renders');
    await scrollTo(lane, 'jar-history-clear');
    await lane.call('click_element', { id: 'jar-history-clear' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'jar-history-snapshots') ?? '').includes('0'),
      'jar: clearing the history empties it'
    );
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
    const currentTotal = rateOf(labelOf(await lane.tree(), 'opal-kpi-total'));
    const total = labelOf(await lane.tree(), 'opal-formula-node-root') ?? '';
    if (!total.includes('Total EXP/hr')) {
      throw new Error(`opal: the breakdown starts at the total, not ${total}`);
    }
    await lane.call('click_element', { id: 'opal-villager-0' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'opal-formula-node-root') ?? '').includes('Explorer EXP/hr'),
      'opal: picking a villager readdresses the tree',
    );
    await lane.call('click_element', { id: 'opal-villager-all' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'opal-formula-node-root') ?? '').includes('Total EXP/hr'),
      'opal: the total control readdresses the tree',
    );
    // The optimizer splits the saved 155 opals and the copied plan names them.
    await lane.call('click_element', { id: 'opal-tab-optimizer' });
    await waitForNode(lane, 'opal-optimizer', 'opal: the optimizer tab opens');
    await lane.call('click_element', { id: 'opal-run' });
    await waitForNode(lane, 'opal-plan-result', 'opal: the plan lists its distribution');
    const plan = labelOf(await lane.tree(), 'opal-plan-summary') ?? '';
    if (!plan.includes('155 opals')) {
      throw new Error(`opal: the plan reads ${plan}`);
    }
    await lane.call('click_element', { id: 'opal-plan-copy' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'status-status') ?? '').includes('Copied the opal plan'),
      'opal: the plan copies to the clipboard',
    );
    // The manual simulator opens on the saved split: 155 opals, no warning. It
    // takes whole, non-negative counts, so emptying the Measurer's 15 opals by
    // hand stops at zero and raises the page's total warning. The reset puts the
    // save's split back, and the total reads what the current state reports.
    await lane.call('click_element', { id: 'opal-tab-manual' });
    await waitForNode(lane, 'opal-manual-summary', 'opal: the manual simulator opens');
    let nodes = await lane.tree();
    if (!(labelOf(nodes, 'opal-manual-summary') ?? '').includes('155.0 opals typed')) {
      throw new Error(`opal: the manual split starts at ${labelOf(nodes, 'opal-manual-summary')}`);
    }
    if (has(nodes, 'opal-manual-warning')) {
      throw new Error('opal: the saved split is already balanced');
    }
    // The Measurer holds 15 opals: fifteen presses empty it, and a sixteenth
    // is refused because the count never goes below zero.
    for (let press = 0; press < 15; press += 1) {
      await lane.call('click_element', { id: 'opal-manual-3-down' });
    }
    let refused = false;
    try {
      await lane.call('click_element', { id: 'opal-manual-3-down' });
    } catch {
      refused = true;
    }
    if (!refused) throw new Error('opal: the count still steps below zero');
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'opal-manual-summary') ?? '').includes('140.0 opals typed') &&
        (labelOf(nodes, 'opal-manual-row-3') ?? '').includes('Measurer: 0.0 opals') &&
        has(nodes, 'opal-manual-warning'),
      'opal: the typed split stops at zero and warns about the total',
    );
    await lane.call('click_element', { id: 'opal-manual-reset' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'opal-manual-summary') ?? '').includes('155.0 opals typed') &&
        (labelOf(nodes, 'opal-manual-row-3') ?? '').includes('Measurer: 15.0 opals') &&
        !has(nodes, 'opal-manual-warning'),
      'opal: the reset restores the saved split',
    );
    // On the save's own split the simulator reads the current state's total, so
    // an equal distribution matches the figure the section reports.
    const manualTotal = rateOf(labelOf(await lane.tree(), 'opal-manual-total'));
    if (!manualTotal || manualTotal !== currentTotal) {
      throw new Error(`opal: the manual total reads ${manualTotal}, current ${currentTotal}`);
    }
    await lane.call('click_element', { id: 'opal-tab-current' });
    await waitForNode(lane, 'opal-overview', 'opal: the current tab reopens');
  },
  sneaking: async (lane) => {
    await openImport(lane, 'sneaking');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await typeInChunks(lane, sneakingSave());
    await waitForNode(lane, 'import-format', 'sneaking: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitForNode(lane, 'sneaking-loadout', 'sneaking: the loadout opens after Apply');
    await waitForNode(lane, 'sneaking-inventory', 'sneaking: the inventory renders');
    await waitForNode(lane, 'sneaking-saved-floors', 'sneaking: the saved floors render');
    const before = labelOf(await lane.tree(), 'sneak-slot-0-3') ?? '';
    await scrollTo(lane, 'sneak-slot-0-3');
    await lane.call('click_element', { id: 'sneak-slot-0-3' });
    // Inventory row 24 holds a non-Gold Lotus Flower, a legal second charm. The
    // grid sits below the fold, so the wheel has to reach it before the click.
    await scrollTo(lane, 'sneak-inv-23');
    await lane.call('click_element', { id: 'sneak-inv-23' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'sneak-slot-0-3') ?? '') !== before,
      'sneaking: the inventory charm swaps into the slot'
    );
    await scrollTo(lane, 'sneak-reset-loadout');
    await lane.call('click_element', { id: 'sneak-reset-loadout' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'sneak-slot-0-3') ?? '') === before,
      'sneaking: reset restores the saved charm'
    );
    // The Breakdowns workspace: every formula tab renders through the shared
    // tree, and the twin selector keeps the root on the chosen twin.
    await lane.call('click_element', { id: 'sneaking-tab-breakdowns' });
    await waitForNode(lane, 'sneaking-formula-node-root', 'sneaking: the breakdown tree renders');
    for (const [tab, root] of [
      ['jade', 'Jade per Hour'],
      ['exp', 'Sneaking EXP per Successful Action'],
      ['door', 'Door Damage per Hour'],
      ['runtime', 'Runtime'],
      ['stealth', 'Stealth'],
    ]) {
      await lane.call('click_element', { id: `sneak-formula-${tab}` });
      await waitFor(
        lane,
        (nodes) => (labelOf(nodes, 'sneaking-formula-node-root') ?? '').includes(root),
        `sneaking: the ${tab} tree renders`
      );
    }
    // SN-03: the charm search runs off the UI thread, ranks the placement and
    // only writes the loadout through Apply.
    await lane.call('click_element', { id: 'sneaking-tab-optimizer' });
    await waitForNode(lane, 'sneaking-optimizer', 'sneaking: the optimizer pane renders');
    await scrollTo(lane, 'sneak-optimize-run');
    await lane.call('click_element', { id: 'sneak-optimize-run' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'sneaking-optimizer-score') ?? '').startsWith('Jade:'),
      'sneaking: the Jade search reports its result',
      30000
    );
    // The objective switch re-measures: EXP is the picker's second choice.
    await lane.call('focus_element', { id: 'sneak-optimize-objective' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await lane.call('click_element', { id: 'sneak-optimize-run' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'sneaking-optimizer-score') ?? '').startsWith('EXP:'),
      'sneaking: the objective switch re-measures',
      30000
    );
    const kept = labelOf(await lane.tree(), 'sneaking-optimizer-score');
    // A cancel only stops the run: the recommendation the window already shows
    // stays, and the loadout is untouched until Apply. Either click can lose a
    // race with a finished run, so both are advisory and the assertion below
    // reads the result.
    await lane.call('click_element', { id: 'sneak-optimize-run' }).catch(() => {});
    await lane.call('click_element', { id: 'sneak-optimize-cancel' }).catch(() => {});
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'sneaking-optimizer-score') ?? '') === kept,
      'sneaking: cancelling keeps the stored recommendation',
      30000
    );
    await scrollTo(lane, 'sneak-optimize-apply');
    await lane.call('click_element', { id: 'sneak-optimize-apply' });
    await lane.call('click_element', { id: 'sneaking-tab-loadout' });
    await waitFor(
      lane,
      (nodes) => has(nodes, 'sneak-reset-loadout'),
      'sneaking: Apply writes the recommendation into the loadout'
    );
    await scrollTo(lane, 'sneak-reset-loadout');
    await lane.call('click_element', { id: 'sneak-reset-loadout' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'sneak-slot-0-3') ?? '') === before,
      'sneaking: reset after Apply restores the saved charms'
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
    await scrollTo(lane, 'fountain-desired-1');
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
    await scrollTo(lane, 'fountain-water-2');
    await lane.call('click_element', { id: 'fountain-water-2' });
    await waitForNode(lane, 'fountain-upgrade-2-11', 'fountain: the green water rows render');
    await scrollTo(lane, 'fountain-upgrade-2-11');
    await lane.call('click_element', { id: 'fountain-upgrade-2-11' });
    // The Planner tab ranks the next level of every upgrade; switching the
    // goal re-ranks the table, and the optimizer spends the saved balances.
    await lane.call('click_element', { id: 'fountain-tab-planner' });
    await waitForNode(lane, 'fountain-planner-table', 'fountain: the planner ranks the upgrades');
    const topByCurrency = labelOf(await lane.tree(), 'fountain-planner-top') ?? '';
    await lane.call('focus_element', { id: 'fountain-planner-goal' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => {
        const top = labelOf(nodes, 'fountain-planner-top') ?? '';
        return top.startsWith('Top pick:') && top !== topByCurrency;
      },
      'fountain: the goal switch changes the ranking'
    );
    await waitForNode(lane, 'fountain-optimizer', 'fountain: the optimizer renders');
    await scrollTo(lane, 'run-fountain-plan');
    // The click starts the run; the window can stop drawing while the plan
    // render lands, so a frame-wait timeout here is not a lost click.
    try {
      await lane.call('click_element', { id: 'run-fountain-plan' });
    } catch (error) {
      if (!/frame wait timed out/i.test(error.message)) throw error;
    }
    await waitForNode(lane, 'fountain-plan-steps', 'fountain: the plan lists its purchases', 30000);
    await waitForNode(lane, 'fountain-plan-step-list', 'fountain: the plan lists every step');
    await lane.call('click_element', { id: 'fountain-tab-overview' });
    await waitForNode(lane, 'fountain-overview', 'fountain: the overview reopens');
  },
  farming: async (lane) => {
    // The cards below the fold scroll with the pane, and a semantic click lands
    // on the element's live bounds, so bring a row into view before pressing it.
    const clickInView = async (id, label) => {
      let previous = null;
      for (let attempt = 0; attempt < 12; attempt += 1) {
        const node = (await lane.tree()).find((entry) => entry.id === id);
        if (!node) throw new Error(`${label}: ${id} is not in the tree`);
        const y = node.bounds?.y ?? 0;
        if (y >= 110 && y <= 850) {
          await lane.call('click_element', { id });
          return;
        }
        // A row already at the pane's end cannot move further, so press it
        // where it is instead of looping.
        if (previous !== null && Math.abs(y - previous) < 1) {
          await lane.call('click_element', { id });
          return;
        }
        previous = y;
        await lane.call('scroll', { x: 700, y: 460, delta_y: Math.ceil(y - 420) });
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
      throw new Error(`${label}: ${id} did not scroll into view`);
    };

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
    await clickInView('farming-curve-plot-7', 'farming');
    await waitFor(
      lane,
      (nodes) => labelOf(nodes, 'farming-curve-title') === 'Plot 8 curve',
      'farming: clicking a curve row switches the curve'
    );
    await lane.call('click_element', { id: 'plots' });
    await waitForNode(lane, 'farming-field', 'farming: the full plot grid opens');
    await clickInView('plot-1', 'farming');
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

    // Switching the objective re-plans the assignment: the first Down opens
    // the list, the second moves the highlight and Enter commits it.
    await lane.call('focus_element', { id: 'farming-objective' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'farming-assignment') ?? '') === 'Plot assignment: Farming EXP',
      'farming: the objective selector re-plans the assignment'
    );

    // The allocator runs off the UI thread and reports its plan.
    await clickInView('farming-run', 'farming');
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
    await clickInView('farming-mode-reset', 'farming');
    await waitForNode(lane, 'farming-stale', 'farming: the scope change marks the result stale');

    // A named preset keeps the goal, the baseline and the crop target.
    await clickInView('farming-preset-name', 'farming');
    await lane.call('type_text', { text: 'Farming EXP reset' });
    await clickInView('farming-preset-save', 'farming');
    await waitForNode(lane, 'farming-preset-0', 'farming: a named preset is saved');

    // The comparison plans every upstream goal from the current baseline.
    await clickInView('farming-compare-run', 'farming');
    await waitFor(
      lane,
      (nodes) => {
        const row = labelOf(nodes, 'farming-compare-rankXp') ?? '';
        const evolution = labelOf(nodes, 'farming-compare-evolution') ?? '';
        return (
          has(nodes, 'farming-compare-rows') &&
          row.startsWith('Land Rank XP:') &&
          evolution.startsWith('Crop Evolution:')
        );
      },
      'farming: the comparison plans every goal'
    );

    // Applying the preset restores the goal it saved, not the current one.
    await lane.call('focus_element', { id: 'farming-objective' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'farming-assignment') ?? '') === 'Plot assignment: Crop Evolution',
      'farming: the objective selector moves past the saved goal'
    );
    await clickInView('farming-preset-apply-0', 'farming');
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'farming-assignment') ?? '') === 'Plot assignment: Farming EXP',
      'farming: applying the preset restores its goal'
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
    // CS-03: the Encounter tab runs a map and reports the run's kills, and the
    // mechanics it does not model are listed beside the options.
    await lane.call('click_element', { id: 'encounter' });
    await waitForNode(lane, 'combat-encounter-run', 'combat-sim: the encounter tab opens');
    await waitForNode(
      lane,
      'combat-encounter-unsupported',
      'combat-sim: the mechanics this build leaves out are listed',
    );
    await lane.call('click_element', { id: 'combat-encounter-run' });
    await waitFor(
      lane,
      (nodes) => {
        const kills = /Kills ([\d.]+)/.exec(labelOf(nodes, 'combat-encounter-kills') ?? '');
        return (
          kills !== null &&
          Number(kills[1]) > 0 &&
          has(nodes, 'combat-encounter-by-type') &&
          !(labelOf(nodes, 'combat-encounter-damage') ?? '').endsWith(' 0.0')
        );
      },
      'combat-sim: the encounter run reports its kills and damage',
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
    // CK-03: the yellow point allocator spends the account's points across the
    // meals of one goal. Only the two goals with two leveled meals are offered,
    // and the save's 19 points are the whole budget.
    await lane.call('click_element', { id: 'cooking-mastery-tab-yellow' });
    await waitForNode(lane, 'cooking-mastery-yellow', 'cooking-mastery: the allocator plans the meals');
    const allocator = async () =>
      labelOf(await lane.tree(), 'cooking-mastery-yellow') ?? '';
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'cooking-mastery-yellow') ?? '').includes('19 of 19 yellow points'),
      'cooking-mastery: the allocator spends every point the account holds'
    );
    if (!(await allocator()).includes('Skill Efficiency')) {
      throw new Error(`cooking-mastery: the allocator starts on ${await allocator()}`);
    }
    await waitForNode(lane, 'cooking-mastery-yellow-rows', 'cooking-mastery: the allocation table renders');
    await waitForNode(
      lane,
      'cooking-mastery-yellow-character',
      'cooking-mastery: the character context is offered'
    );
    // The goal switch re-plans: Total Damage is the select's second option.
    await lane.call('focus_element', { id: 'cooking-mastery-yellow-goal' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'cooking-mastery-yellow') ?? '').includes('Total Damage'),
      'cooking-mastery: the goal switch re-plans the meals'
    );
    // Turkey of Thank is one of the two Total Damage meals, and it holds the
    // two yellow points the save already spent.
    await bringIntoView(lane, 'cooking-mastery-yellow-row-0');
    const turkey = labelOf(await lane.tree(), 'cooking-mastery-yellow-row-0') ?? '';
    if (!turkey.includes('2 points now')) {
      throw new Error(`cooking-mastery: the saved yellow points read ${turkey}`);
    }
    // The budget stepper spends less, and the plan says how much.
    await lane.call('click_element', { id: 'cooking-mastery-yellow-budget-less' });
    await lane.call('click_element', { id: 'cooking-mastery-yellow-budget-less' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'cooking-mastery-yellow') ?? '').includes('17 of 19 yellow points'),
      'cooking-mastery: the budget stepper re-plans the allocation'
    );
    await lane.call('click_element', { id: 'cooking-mastery-tab-exp' });
    await waitForNode(lane, 'cooking-mastery-summary', 'cooking-mastery: the EXP tab reopens');
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
    // The card lane carries the previous section's save, so start the research
    // checks from a purpose-built account.
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'research: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: researchSave() });
    await waitForNode(lane, 'import-format', 'research: the research save previews');
    await lane.call('click_element', { id: 'import-apply' });
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
    // The dashboard's cards own their own scroll areas, so the page scrolls
    // from its margin.
    await bringIntoView(lane, 'dash-cell-129', await wheelPoint(lane, 'dashboard-scroll'));
    await lane.call('click_element', { id: 'dash-cell-129' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'square-name') ?? '').includes('J6'),
      'research: the grid preview opens the editor on its square',
    );
    await lane.call('click_element', { id: 'dashboard' });
    await waitForNode(lane, 'research-dashboard', 'research: the dashboard tab reopens');
    await bringIntoView(lane, 'dash-obs-0', await wheelPoint(lane, 'dashboard-scroll'));
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

    // RES-13: every edit is undoable and redoable, the canvas handle turns a
    // shape, and the keyboard agrees with the pointer.
    await lane.call('click_element', { id: 'cell-129' });
    const level = labelOf(await lane.tree(), 'square-level') ?? '';
    await lane.call('click_element', { id: 'square-plus' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'square-level') ?? '') !== level,
      'research: the inspector raises the selected square',
    );
    await lane.call('click_element', { id: 'grid-undo' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'square-level') ?? '') === level,
      'research: undo takes the level change back',
    );
    await lane.call('click_element', { id: 'grid-redo' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'square-level') ?? '') !== level,
      'research: redo re-applies the level change',
    );
    await lane.call('click_element', { id: 'shape-add' });
    await waitForNode(lane, 'shape-rotate-handle', 'research: the new shape shows its rotation handle');
    // The handle turns on the pointer's angle around the shape, so it takes a
    // coordinate drag rather than a drag-and-drop handoff.
    const centreOf = (node) => ({
      x: node.bounds.x + node.bounds.width / 2,
      y: node.bounds.y + node.bounds.height / 2,
    });
    const handle = centreOf((await lane.tree()).find((node) => node.id === 'shape-rotate-handle'));
    const corner = centreOf((await lane.tree()).find((node) => node.id === 'cell-129'));
    await lane.call('pointer_drag', {
      from_x: handle.x,
      from_y: handle.y,
      to_x: corner.x,
      to_y: corner.y,
      steps: 12,
    });
    await waitFor(
      lane,
      (nodes) => !(labelOf(nodes, 'shape-rotate-handle') ?? '').includes('(0°)'),
      'research: dragging the canvas handle turns the shape',
    );
    const turned = labelOf(await lane.tree(), 'shape-rotate-handle') ?? '';
    await lane.call('keyboard', { keystroke: ']' });
    await waitFor(
      lane,
      (nodes) => labelOf(nodes, 'shape-rotate-handle') !== turned,
      'research: the ] key turns the picked shape',
    );
    await lane.call('keyboard', { keystroke: 'secondary-z' });
    await waitFor(
      lane,
      (nodes) => labelOf(nodes, 'shape-rotate-handle') === turned,
      'research: secondary-z undoes the keyboard turn',
    );

    // Magnifier assignment: pick an observation on Rates, then place and lift a
    // glass on it from the grid inspector.
    await lane.call('click_element', { id: 'rates' });
    await waitForNode(lane, 'obs-name-0', 'research: the rate table lists its observations');
    await lane.call('click_element', { id: 'obs-name-0' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'observations-selected') ?? '').startsWith('Selected:'),
      'research: clicking an observation selects it for the magnifier pool',
    );
    await lane.call('click_element', { id: 'grid' });
    await waitForNode(lane, 'mag-assign', 'research: the magnifier card edits the selected observation');
    // A disabled button keeps its node but loses its click action.
    const clickable = (nodes, id) =>
      ((nodes.find((node) => node.id === id)?.actions) ?? []).includes('click');
    if (clickable(await lane.tree(), 'mag-lift')) {
      await lane.call('click_element', { id: 'mag-lift' });
      await waitFor(
        lane,
        (nodes) => clickable(nodes, 'mag-add-Glass'),
        'research: lifting a magnifier frees one in the pool',
      );
    }
    const free = labelOf(await lane.tree(), 'mag-free-Glass') ?? '';
    await lane.call('click_element', { id: 'mag-add-Glass' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'mag-free-Glass') ?? '') !== free,
      'research: placing a glass takes it out of the pool',
    );
    await lane.call('click_element', { id: 'mag-lift' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'mag-free-Glass') ?? '') === free,
      'research: removing it puts the glass back in the pool',
    );

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
    await bringIntoView(lane, 'shape-preset-name', await wheelPoint(lane, 'shape-opt-scroll'));
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
    await bringIntoView(lane, 'shape-optimize', await wheelPoint(lane, 'shape-opt-scroll'));
    await lane.call('click_element', { id: 'shape-optimize' });
    await waitForNode(lane, 'shape-preview', 'research: the optimizer previews a placement', 120000);
    await waitForNode(lane, 'shape-breakdown-order-0', 'research: the coverage breakdown lists the first shape');
    const shapeSummary = labelOf(await lane.tree(), 'shape-summary') ?? '';
    if (!shapeSummary.includes('shapes placed')) {
      throw new Error(`research: the shape summary reads ${shapeSummary}`);
    }

    // RES-09: the Minehead tab's six subtabs render from the save. The subtab
    // buttons are shared `Button`s whose semantic id a render can rewrite
    // mid-frame, so they are clicked at their live centres by label.
    const subtab = async (label) => {
      for (let attempt = 0; attempt < 100; attempt += 1) {
        const node = (await lane.tree()).find(
          (candidate) => candidate.parent === 'minehead-tabs' && candidate.label === label
        );
        if (node) {
          return {
            x: node.bounds.x + node.bounds.width / 2,
            y: node.bounds.y + node.bounds.height / 2,
          };
        }
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      throw new Error(`research: no Minehead subtab labelled ${label}`);
    };
    await lane.call('click_element', { id: 'minehead' });
    await waitForNode(lane, 'minehead-dashboard', 'research: the Minehead dashboard opens');
    await waitForNode(lane, 'mh-upg-name-0', 'research: the Minehead upgrade table lists its first row');
    await waitForNode(lane, 'mh-reward-0', 'research: the Minehead boss rewards render');
    await lane.call('click_coordinates', await subtab('Currency'));
    await waitForNode(lane, 'minehead-currency-tree', 'research: the Minehead currency breakdown opens');
    await lane.call('click_coordinates', await subtab('Optimize'));
    await waitForNode(lane, 'mh-opt-run', 'research: the Minehead strategy search opens');
    await lane.call('click_coordinates', await subtab('Rank Upgrades'));
    await waitForNode(lane, 'mh-rank-run', 'research: the Minehead upgrade ranking opens');
    await lane.call('click_coordinates', await subtab('Upgrade Path'));
    await waitForNode(lane, 'mh-path-run', 'research: the Minehead upgrade path opens');
    await lane.call('click_coordinates', await subtab('Play'));
    await waitForNode(lane, 'mh-play-attack', 'research: the Minehead playfield opens');
    await waitForNode(lane, 'mh-tile-0', 'research: the playfield draws its first tile');
    await lane.call('click_element', { id: 'mh-play-insta' });
    await waitForNode(lane, 'mh-log-1', 'research: an insta-reveal logs its mine');
    await lane.call('click_element', { id: 'mh-play-new' });
    await waitForNode(lane, 'mh-play-attack', 'research: a new game keeps the playfield');

    // RES-10: the Sushi Station tab renders from the save. Its subtabs are the
    // same shared `Button`s the Minehead strip uses, so they are clicked at
    // their live centres by label too.
    const sushiSubtab = async (label) => {
      for (let attempt = 0; attempt < 100; attempt += 1) {
        const node = (await lane.tree()).find(
          (candidate) => candidate.parent === 'sushi-tabs' && candidate.label === label
        );
        if (node) {
          return {
            x: node.bounds.x + node.bounds.width / 2,
            y: node.bounds.y + node.bounds.height / 2,
          };
        }
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      throw new Error(`research: no Sushi subtab labelled ${label}`);
    };
    await lane.call('click_element', { id: 'sushi' });
    await waitForNode(lane, 'sushi-overview', 'research: the Sushi Station overview opens');
    await waitForNode(lane, 'su-slot-0', 'research: the station grid draws its first slot');
    await waitForNode(lane, 'su-tier-0', 'research: the tier map draws its first tier');
    await lane.call('click_coordinates', await sushiSubtab('Upgrades'));
    await waitForNode(lane, 'su-upg-name-0', 'research: the upgrade table lists its first row');
    const upgradeRows = async () =>
      (await lane.tree()).filter((node) => (node.id ?? '').startsWith('su-upg-name-')).length;
    const beforeSpoiler = await upgradeRows();
    await lane.call('click_element', { id: 'su-upg-spoiler' });
    await waitFor(
      lane,
      (nodes) =>
        nodes.filter((node) => (node.id ?? '').startsWith('su-upg-name-')).length > beforeSpoiler,
      'research: the spoiler toggle reveals the locked upgrades',
    );
    await lane.call('click_coordinates', await sushiSubtab('Currency/hr'));
    await waitForNode(lane, 'sushi-bucks-tree', 'research: the bucks breakdown opens');
    await waitForNode(lane, 'sushi-fuel-tree', 'research: the fuel generation tree renders');
    await waitForNode(lane, 'sushi-capacity-tree', 'research: the fuel capacity tree renders');
    await lane.call('click_coordinates', await sushiSubtab('Best Upgrade'));
    await waitForNode(lane, 'su-path-bucks', 'research: the affordable bucks path renders');
    await waitFor(
      lane,
      (nodes) => nodes.some((node) => (node.id ?? '').startsWith('su-best-bucks-')),
      'research: the bucks ranking lists its first upgrade',
    );
    await lane.call('click_coordinates', await sushiSubtab('Sushi'));
    await waitForNode(lane, 'su-tier-row-0', 'research: the sushi tier table opens');
    await waitForNode(lane, 'su-knowledge-0', 'research: the knowledge totals render');
    const tierRows = async () =>
      (await lane.tree()).filter((node) => (node.id ?? '').startsWith('su-tier-row-')).length;
    const beforeTiered = await tierRows();
    await lane.call('click_element', { id: 'su-rog-spoiler' });
    await waitFor(
      lane,
      (nodes) =>
        nodes.filter((node) => (node.id ?? '').startsWith('su-tier-row-')).length > beforeTiered,
      'research: the RoG spoiler reveals the locked tiers',
    );
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
    await waitForNode(
      lane,
      'kitchens-comparison-objective',
      'ladle: the forecast compares its scenarios'
    );
    await waitForNode(lane, 'kitchens-grant-chart', 'ladle: the comparison draws the grant trend');
    await waitForNode(lane, 'kitchens-outcomes', 'ladle: the meal outcomes table renders');
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

    // The target objective without a target refuses the forecast but keeps its
    // target inputs on screen, so the missing input can be fixed from the tab.
    await lane.call('focus_element', { id: 'kitchens-objective' });
    // The first Down opens the list, the second highlights the target objective.
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => has(nodes, 'kitchens-error') && has(nodes, 'kitchens-outcomes'),
      'ladle: the target objective asks for a target and keeps the inputs'
    );
    // Five presses take Turkey's target past its saved level, and the forecast
    // runs again under the objective the comparison now names.
    await bringIntoView(lane, 'kitchens-target-0');
    for (let press = 0; press < 5; press += 1) {
      await lane.call('click_element', { id: 'kitchens-target-0-up' });
    }
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'kitchens-comparison-objective') ?? '') === 'Selected meal targets',
      'ladle: the objective switch ranks the selected meal targets'
    );

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
    // A closed select opens on the first Down and starts on its current choice.
    await lane.call('focus_element', { id: 'statue-estimate' });
    await lane.call('keyboard', { keystroke: 'down' });
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
    const breakdownRoot = labelOf(await lane.tree(), 'statue-breakdown-node-root');
    await lane.call('focus_element', { id: 'statue-breakdown-statue' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'statue-breakdown-node-root') ?? '') !== breakdownRoot &&
        (labelOf(nodes, 'statue-breakdown-node-root') ?? '').includes('statues needed'),
      'statue: the farming breakdown follows the selected statue',
    );
  },
  damage: async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'damage: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: statsSave() });
    await waitForNode(lane, 'import-format', 'damage: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'damage-context-note') ?? '').startsWith(
          'Ana · Blunder Hills · The account stats engine (SA-02) is not built yet',
        ),
      'damage: the summary reports the missing account stats engine',
    );
    await waitForNode(lane, 'damage-game', 'damage: the summary cards render');
    await waitForNode(lane, 'damage-mastery-bar', 'damage: the mastery bar renders');

    // A character switch follows that character's current map.
    await lane.call('focus_element', { id: 'damage-character' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'damage-context-note') ?? '').startsWith('Bo · YumYum Grotto · '),
      'damage: the character selector switches to that character\'s current map',
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
    const count = (nodes) => Number(/(\d+) maps/.exec(labelOf(nodes, 'stats-map-count') ?? '')?.[1]);
    const accessible = count(await lane.tree());
    if (!(accessible > 0)) throw new Error(`stats-all: the accessible list reads ${accessible}`);

    // Character two follows its own current map. A closed select opens on the
    // first Down and starts on its current choice, so the second Down moves.
    await lane.call('focus_element', { id: 'stats-character' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-context-note') ?? '') ===
        'Bo · YumYum Grotto · Mining · Construction Active',
      'stats-all: the character selector switches to that character\'s current map',
    );

    // The map list level widens the map selector and refreshes the summary:
    // Accessible opens, then two rows down reaches All Valid Maps.
    await lane.call('focus_element', { id: 'stats-map-visibility' });
    await lane.call('keyboard', { keystroke: 'down' });
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
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await lane.call('focus_element', { id: 'stats-construction' });
    await lane.call('keyboard', { keystroke: 'down' });
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
      await bringIntoView(lane, `stats-${key}-tab`);
      await lane.call('click_element', { id: `stats-${key}-tab` });
      await waitFor(
        lane,
        (nodes) => (labelOf(nodes, 'stats-category-panel') ?? '').startsWith(`${label} breakdowns:`),
        `stats-all: the ${label} tab renders its rows`,
      );
    }

    // The capacity rows read the character's own saved bag row, so the check
    // below needs the character this save carries: Home opens on the first slot.
    await lane.call('focus_element', { id: 'stats-character' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'home' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-context-note') ?? '') ===
        'Ana · Blunder Hills · Choppin · Construction Inactive',
      'stats-all: the character selector returns to the character the save carries',
    );

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
      (nodes) =>
        (labelOf(nodes, 'stats-formula-node-root') ?? '').includes('Mining Carry Capacity'),
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

    // The formula explorer's ledger counts every category, so the region covers
    // the whole registry rather than the computed rows alone.
    await waitForNode(lane, 'stats-explorer-ledger', 'stats-all: the explorer ledger renders');
    const ledger = labelOf(await lane.tree(), 'stats-explorer-ledger') ?? '';
    for (const [, label] of categories) {
      if (!ledger.includes(label)) {
        throw new Error(`stats-all: the explorer ledger misses ${label}: ${ledger}`);
      }
    }

    // The breakdown card shows the same descriptor the explorer card will, so
    // the two values below come from one resolution rather than two.
    await bringIntoView(lane, 'stats-capacity-tab');
    await lane.call('click_element', { id: 'stats-capacity-tab' });

    // Search reaches the saved inputs behind the terms: the eight capacity
    // formulas read the vault row and nothing else does.
    await bringIntoView(lane, 'stats-explorer-search');
    await lane.call('click_element', { id: 'stats-explorer-search' });
    await lane.call('type_text', { text: 'vault' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-explorer-count') ?? '').startsWith('8 of 60') &&
        has(nodes, 'stats-formula-carry-capacity-mining') &&
        !has(nodes, 'stats-formula-drop-rate'),
      'stats-all: the explorer search reaches the saved inputs',
    );

    // Each formula links to the upstream definition it ports.
    const miningLink = (await lane.tree()).find(
      (node) => node.id === 'stats-formula-carry-capacity-mining-source',
    );
    if (!(miningLink?.label ?? '').includes('js/stats/defs/carry-capacity.js')) {
      throw new Error(
        `stats-all: the formula source link reads ${JSON.stringify(miningLink?.label)}`,
      );
    }

    // The status filter keeps the formulas this build computes.
    await bringIntoView(lane, 'stats-explorer-status');
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('keyboard', { keystroke: 'backspace' });
    await lane.call('focus_element', { id: 'stats-explorer-status' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'stats-explorer-count') ?? '').startsWith('10 of 60'),
      'stats-all: the status filter keeps the computed formulas',
    );

    // The category filter narrows the same list; the card shows the formula,
    // and its value is the breakdown's own.
    // The first Down opens the list on its current choice, so Capacity is five
    // rows down from All categories.
    await lane.call('focus_element', { id: 'stats-explorer-category' });
    for (let row = 0; row <= 5; row += 1) {
      await lane.call('keyboard', { keystroke: 'down' });
    }
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-explorer-count') ?? '').startsWith('8 of 60') &&
        has(nodes, 'stats-formula-carry-capacity-mining'),
      'stats-all: the category filter keeps one category',
    );
    await bringIntoView(lane, 'stats-formula-carry-capacity-mining');
    await lane.call('click_element', { id: 'stats-formula-carry-capacity-mining' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-formula-value') ?? '') ===
          (labelOf(nodes, 'stats-descriptor-summary') ?? '') &&
        (labelOf(nodes, 'stats-explorer-formula-node-root') ?? '').includes(
          'Mining Carry Capacity',
        ),
      'stats-all: the explorer card agrees with the breakdown and shows its tree',
    );

    // The game-input sandbox starts on the saved values, with nothing to list
    // as changed and nothing to reset.
    await bringIntoView(lane, 'stats-input-status');
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-input-status') ?? '') === 'Game-input sandbox: no edits.' &&
        (labelOf(nodes, 'stats-affected-count') ?? '') === 'Affected formulas (0)',
      'stats-all: the input ledger opens with no edits',
    );

    // The gem-shop capacity input doubles as the ledger's first edit: one
    // purchase takes the factor to x1.25, so every capacity row moves.
    await bringIntoView(lane, 'stats-input-gem-items-58');
    await lane.call('click_element', { id: 'stats-input-gem-items-58-up' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-input-status') ?? '') ===
          'Hypothetical: 1 edited game input.' &&
        (labelOf(nodes, 'stats-affected-count') ?? '') === 'Affected formulas (8)',
      'stats-all: a game input edit resolves a hypothetical save',
    );
    const changed = labelOf(await lane.tree(), 'stats-input-row-gem-items-58') ?? '';
    if (!changed.includes('Gem shop capacity · Purchases: 0 → 1')) {
      throw new Error(`stats-all: the changed input reads ${changed}`);
    }
    const affected = labelOf(await lane.tree(), 'stats-affected-carry-capacity-mining') ?? '';
    if (!affected.includes('35,000 → 43,750')) {
      throw new Error(`stats-all: the affected formula reads ${affected}`);
    }
    const marked = labelOf(await lane.tree(), 'stats-row-carry-capacity-mining') ?? '';
    if (!marked.includes('Saved: 35,000')) {
      throw new Error(`stats-all: the descriptor row keeps the saved value as ${marked}`);
    }
    await bringIntoView(lane, 'stats-descriptor-saved');
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'stats-descriptor-saved') ?? '').includes('Saved: 35,000'),
      'stats-all: the descriptor keeps the saved value beside the hypothetical one',
    );

    // An affected formula links back to its descriptor, tab included.
    await bringIntoView(lane, 'stats-affected-carry-capacity-mining');
    await lane.call('click_element', { id: 'stats-affected-carry-capacity-mining' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-category-panel') ?? '').startsWith('Capacity breakdowns:') &&
        (labelOf(nodes, 'stats-descriptor-summary') ?? '').includes('Mining Carry Capacity'),
      'stats-all: an affected formula selects its descriptor',
    );

    // Resetting one input restores its saved value, and resetting all of them
    // leaves the saved save exactly as it was.
    await bringIntoView(lane, 'stats-input-reset-gem-items-58');
    await lane.call('click_element', { id: 'stats-input-reset-gem-items-58' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-input-status') ?? '') === 'Game-input sandbox: no edits.' &&
        (labelOf(nodes, 'stats-descriptor-summary') ?? '').includes('35,000'),
      'stats-all: resetting an input restores the saved value',
    );
    await lane.call('click_element', { id: 'stats-input-gem-items-58-up' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'stats-affected-count') ?? '') === 'Affected formulas (8)',
      'stats-all: the second edit moves the rows again',
    );
    await lane.call('click_element', { id: 'stats-inputs-reset' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'stats-affected-count') ?? '') === 'Affected formulas (0)' &&
        (labelOf(nodes, 'stats-descriptor-summary') ?? '').includes('35,000'),
      'stats-all: reset all edits restores every saved value',
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
    const waiting = labelOf(await lane.tree(), 'drop-rate-total') ?? '';
    if (!waiting.endsWith('Unavailable')) {
      throw new Error(`drop-rate: the total without live values reads ${waiting}`);
    }
    // The tree renders upstream's unavailable branch until both values arrive.
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'drop-rate-formula-node-root-0') ?? '') ===
        'Dungeon Runtime Inputs Required: 0',
      'drop-rate: the tree asks for the two live run values',
    );
    await lane.call('click_element', { id: 'drop-rate-luk' });
    await lane.call('type_text', { text: '1200' });
    await lane.call('click_element', { id: 'drop-rate-drop' });
    await lane.call('type_text', { text: '35' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'drop-rate-total') ?? '').endsWith('13.35x'),
      'drop-rate: the total is the upstream dungeon formula',
    );
    // The formula tree expands that same number, term by term.
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'drop-rate-formula-node-root') ?? '') === 'Dungeon Drop Rate: ×13.35',
      'drop-rate: the formula tree roots on the total card number',
    );
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'drop-rate-formula-node-root-1') ?? '') === 'Percent to fraction: 12.35',
      'drop-rate: the tree expands the percent-to-fraction step',
    );
    // Searching by a term's source narrows the tree to the entries that name it.
    await bringIntoView(lane, 'drop-rate-formula-search');
    await lane.call('click_element', { id: 'drop-rate-formula-search' });
    await lane.call('type_text', { text: 'live dungeon' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'drop-rate-formula-match-count') ?? '') === '2 matching formula nodes',
      'drop-rate: searching the tree finds both live run inputs by source',
    );
    await lane.call('click_element', { id: 'drop-rate-formula-collapse' });
    // A normal map hides the runtime panel, names the missing engine and has no
    // tree to expand.
    await bringIntoView(lane, 'drop-rate-map');
    await lane.call('focus_element', { id: 'drop-rate-map' });
    // Maps 40 and 41 sit right after the saved map 39; the third step lands on a
    // normal map, so the runtime panel goes and the tree has nothing to expand.
    for (let step = 0; step < 3; step++) await lane.call('keyboard', { keystroke: 'down' });
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
    await waitFor(
      lane,
      (nodes) => {
        const gap = labelOf(nodes, 'drop-rate-formula-gap') ?? '';
        return gap.includes('shared stat pipeline') && !has(nodes, 'drop-rate-formula-node-root');
      },
      'drop-rate: a normal map has no tree to expand',
    );
  },
  'coin-multi': async (lane) => {
    await lane.call('click_element', { id: 'header-import' });
    await waitForNode(lane, 'import-input', 'coin-multi: the import dialog opens');
    await lane.call('click_element', { id: 'import-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: coinSave() });
    await waitForNode(lane, 'import-format', 'coin-multi: the save previews');
    await lane.call('click_element', { id: 'import-apply' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'coin-map') ?? '').includes('Froggy Fields') &&
        (labelOf(nodes, 'coin-target') ?? '').includes('Frog') &&
        (labelOf(nodes, 'coin-hp') ?? '').includes('120') &&
        (labelOf(nodes, 'coin-damage') ?? '').includes('500'),
      'coin-multi: the save cards read the map, target, HP and damage',
    );
    if (!(labelOf(await lane.tree(), 'coin-total') ?? '').includes('x')) {
      throw new Error('coin-multi: the total card has no multiplier');
    }
    if (!has(await lane.tree(), 'coin-partial')) {
      throw new Error('coin-multi: the partial badge has no explanation');
    }

    // Character two follows its own target, which the monster table lacks.
    await lane.call('focus_element', { id: 'coin-character' });
    await lane.call('keyboard', { keystroke: 'down' });
    await lane.call('keyboard', { keystroke: 'enter' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'coin-map') ?? '').includes('YumYum Grotto') &&
        (labelOf(nodes, 'coin-target') ?? '').includes('Unavailable'),
      'coin-multi: the character selector switches to that character\'s target',
    );

    // Zero damage puts the tier back to one, and the cards follow the field.
    await lane.call('click_element', { id: 'coin-damage-input' });
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('type_text', { text: '0' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'coin-damage') ?? '').includes('0') &&
        (labelOf(nodes, 'coin-overkill') ?? '').includes('1'),
      'coin-multi: the damage field drives the overkill tier',
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
    // The Wisdom tab reads its own rows: attempts, the saved charge and the
    // partial-save warning this flat save triggers.
    await lane.call('click_element', { id: 'wisdom' });
    await waitForNode(lane, 'monument-pane-wisdom', 'monument: the wisdom tab opens');
    const attempts = labelOf(await lane.tree(), 'monument-wisdom-attempts') ?? '';
    if (!attempts.includes('5.0')) {
      throw new Error(`monument: the wisdom attempts read ${attempts}`);
    }
    const charge = labelOf(await lane.tree(), 'monument-wisdom-charge-now') ?? '';
    if (!charge.includes('1.2x') || !charge.includes('story ready')) {
      throw new Error(`monument: the wisdom charge reads ${charge}`);
    }
    const warning = labelOf(await lane.tree(), 'monument-wisdom-notes') ?? '';
    check(
      'monument: the wisdom tab warns about the missing companion roster',
      warning.includes('Companion ownership is missing'),
      warning,
    );

    // The Justice tab: the worksheet's advisor, its undoable effects editor and
    // its case search.
    await lane.call('click_element', { id: 'justice' });
    await waitForNode(lane, 'monument-pane-justice', 'monument: the justice tab opens');
    await waitForNode(
      lane,
      'monument-justice-response-approve',
      'monument: the advisor lists the responses',
    );
    await waitForNode(
      lane,
      'monument-justice-recommended',
      'monument: the advisor suggests a response',
    );
    const justiceRequest = labelOf(await lane.tree(), 'monument-justice-request') ?? '';
    if (!justiceRequest.includes('Little Timmy')) {
      throw new Error(`monument: the justice request reads ${justiceRequest}`);
    }
    const heuristic = labelOf(await lane.tree(), 'monument-justice-heuristic') ?? '';
    if (!heuristic.includes('Survival-first heuristic')) {
      throw new Error(`monument: the advisor heuristic reads ${heuristic}`);
    }

    // The effects editor is undoable: a toggled deal reaches the delayed effects
    // and Undo takes it back. The worksheet is taller than the window, so each
    // control is scrolled into view first.
    await scrollTo(lane, 'monument-justice-effect-PermaPop');
    await lane.call('click_element', { id: 'monument-justice-effect-PermaPop' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'monument-justice-delayed') ?? '').includes('Permanent popularity floor'),
      'monument: the effects editor records a deal',
    );
    await scrollTo(lane, 'monument-justice-undo');
    await lane.call('click_element', { id: 'monument-justice-undo' });
    await waitFor(
      lane,
      (nodes) =>
        (labelOf(nodes, 'monument-justice-delayed') ?? '') ===
        'No active deals or investments recorded.',
      'monument: undo removes the recorded deal',
    );

    // The search narrows the case list by visitor or request wording.
    await scrollTo(lane, 'monument-justice-search');
    await lane.call('click_element', { id: 'monument-justice-search' });
    await lane.call('type_text', { text: 'Timmy' });
    await waitFor(
      lane,
      (nodes) => {
        const count = labelOf(nodes, 'monument-justice-match-count') ?? '';
        return count.includes('matching') && !count.startsWith('90');
      },
      'monument: the case search filters the list',
    );
    await lane.call('keyboard', { keystroke: 'secondary-a' });
    await lane.call('keyboard', { keystroke: 'backspace' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'monument-justice-match-count') ?? '') === '90 cases',
      'monument: clearing the search restores every case',
    );

    // Recording a response waits for the next case, and opening it reports what
    // the delayed effects paid.
    await scrollTo(lane, 'monument-justice-record-approve-0');
    await lane.call('click_element', { id: 'monument-justice-record-approve-0' });
    await waitForNode(
      lane,
      'monument-justice-next',
      'monument: a recorded response waits for the next case',
    );
    await scrollTo(lane, 'monument-justice-next');
    await lane.call('click_element', { id: 'monument-justice-next' });
    await waitFor(
      lane,
      (nodes) => (labelOf(nodes, 'monument-justice-notice') ?? '').includes('Next case opened'),
      'monument: opening the next case settles the delayed effects',
    );

    // MN-04: the bonus sources tab carries the permanent and charge summaries,
    // and its named training rows sum to the additive gain card.
    await lane.call('click_element', { id: 'sources' });
    await waitForNode(
      lane,
      'monument-permanent-bravery',
      'monument: the permanent reward summary renders',
    );
    const levels = labelOf(await lane.tree(), 'monument-permanent-bravery') ?? '';
    check(
      'monument: the permanent summary totals the saved levels',
      levels.includes('0.0 levels') && levels.includes('ten saved permanent reward levels'),
      levels,
    );
    const rewardMulti = labelOf(await lane.tree(), 'monument-reward-multi') ?? '';
    check(
      'monument: the reward-level multiplier is named',
      rewardMulti.includes('1.0x') && rewardMulti.includes('Companion bonus'),
      rewardMulti,
    );
    const nextCharge = labelOf(await lane.tree(), 'monument-charge-bravery') ?? '';
    check(
      'monument: the next-run charge reads the saved wait',
      nextCharge.includes('needs at least 1x') && nextCharge.includes('After another 24h: 1.2x'),
      nextCharge,
    );
    const gain = labelOf(await lane.tree(), 'monument-training-additive') ?? '';
    if (!gain.includes('+0.5%')) {
      throw new Error(`monument: the additive training gain reads ${gain}`);
    }
    await scrollTo(lane, 'monument-sources-total');
    const total = labelOf(await lane.tree(), 'monument-sources-total') ?? '';
    if (total !== '+0.5%') {
      throw new Error(`monument: the additive pool total reads ${total}`);
    }
    await waitForNode(
      lane,
      'monument-charge-justice',
      'monument: the charge summary covers every monument',
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
      (nodes) => !has(nodes, 'cglunko-formula-node-root-0'),
      'cglunko: collapse closes the tree',
    );
    await lane.call('click_element', { id: 'cglunko-formula-expand' });
    await waitForNode(lane, 'cglunko-formula-node-root-0-0', 'cglunko: expand reopens the additive group');
    const additive = labelOf(await lane.tree(), 'cglunko-formula-node-root-0-0') ?? '';
    if (!additive.includes('+30%')) {
      throw new Error(`cglunko: the additive group reads ${additive}`);
    }
    // The Upgrades tab lists the selected page's twelve upgrades with the stone
    // that funds each one.
    await lane.call('click_element', { id: 'upgrades' });
    await waitForNode(lane, 'cglunko-upgrades', 'cglunko: the upgrades tab opens');
    await waitForNode(lane, 'cglunko-upgrade-0', 'cglunko: the blue page lists Opal Find');
    if (has(await lane.tree(), 'cglunko-upgrade-12')) {
      throw new Error('cglunko: the blue page lists a purple upgrade');
    }
    // The page selector drives the list: purple shows upgrades 12..23 and the
    // priority table below switches to the purple drop-rate upgrades.
    await lane.call('click_element', { id: 'cglunko-page-toggle-1' });
    await waitForNode(lane, 'cglunko-upgrade-12', 'cglunko: purple shows Purp Collector');
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'cglunko-upgrade-0'),
      'cglunko: purple hides the blue page',
    );
    // The Optimizer tab ranks the selected page and spends its stones: blue
    // buys a level, and switching page marks that plan stale instead of
    // dropping it.
    await lane.call('click_element', { id: 'optimizer' });
    await waitForNode(lane, 'cglunko-optimizer', 'cglunko: the optimizer tab opens');
    await waitForNode(lane, 'cglunko-priority-12', 'cglunko: purple priority ranks Purp Collector first');
    if (has(await lane.tree(), 'cglunko-priority-10')) {
      throw new Error('cglunko: the purple priority lists a blue upgrade');
    }
    await lane.call('click_element', { id: 'cglunko-page-toggle-0' });
    await waitForNode(lane, 'cglunko-priority-10', 'cglunko: blue priority ranks Blue Collector');
    await lane.call('click_element', { id: 'run-cglunko-plan' });
    await waitForNode(lane, 'cglunko-plan-result', 'cglunko: the plan lists its purchases');
    await waitFor(
      lane,
      (nodes) => has(nodes, 'cglunko-plan-upgrade-1') || has(nodes, 'cglunko-plan-upgrade-3'),
      'cglunko: the plan buys a blue drop-rate upgrade',
    );
    await lane.call('click_element', { id: 'cglunko-page-toggle-1' });
    await waitForNode(lane, 'cglunko-plan-badge', 'cglunko: another page marks the plan stale');
    // The Stones tab renders the selected colour's six stones, the DR a digit
    // adds and the focus target the note explains: the save focuses Blue #3,
    // which is also the stone whose next digit is fewest focused kills away.
    await lane.call('click_element', { id: 'stones' });
    await waitForNode(lane, 'cglunko-stones', 'cglunko: the stones tab opens');
    await waitForNode(lane, 'cglunko-stone-6', 'cglunko: the purple page lists Purple #1');
    if (has(await lane.tree(), 'cglunko-stone-0')) {
      throw new Error('cglunko: the purple page lists a blue stone');
    }
    await lane.call('click_element', { id: 'cglunko-page-toggle-0' });
    await waitForNode(lane, 'cglunko-stone-0', 'cglunko: the blue page lists Blue #1');
    await waitFor(
      lane,
      (nodes) => !has(nodes, 'cglunko-stone-6'),
      'cglunko: the blue page hides the purple stones',
    );
    const impact = labelOf(await lane.tree(), 'cglunko-digit-impact') ?? '';
    if (!impact.includes('+4.0% per blue-shape digit') || !impact.includes('5 = +20.0% DR')) {
      throw new Error(`cglunko: the digit impact reads ${impact}`);
    }
    await waitForNode(lane, 'cglunko-stone-focus-2', 'cglunko: Blue #3 carries the saved focus');
    await waitForNode(
      lane,
      'cglunko-focus-recommend-2',
      'cglunko: Blue #3 is the recommended target',
    );
    const note = labelOf(await lane.tree(), 'cglunko-focus-note') ?? '';
    if (!note.includes('Blue #3') || !note.includes('×30.0 on blue')) {
      throw new Error(`cglunko: the focus note reads ${note}`);
    }
    await lane.call('click_element', { id: 'overview' });
    await waitForNode(lane, 'cglunko-overview', 'cglunko: the overview tab reopens');
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
/// The figure behind a `"<label>: <value>"` reading, without its caption.
const rateOf = (label) => (label ?? '').split(': ')[1] ?? '';
/// Scrolls a tall pane until the element sits inside the window, so a click
/// lands on it instead of on a clipped area.
const scrollTo = async (lane, id, { step = 400, attempts = 8 } = {}) => {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const node = (await lane.tree()).find((candidate) => candidate.id === id);
    if (!node) throw new Error(`no element ${id} to scroll to`);
    const y = node.bounds?.y ?? 0;
    if (y >= 120 && y <= 780) return;
    await lane.call('scroll', { x: 600, y: 520, delta_y: y > 780 ? step : -step });
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
};

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

/// Types a long text in pieces: the bridge caps one input command at 64 KiB, and
/// the synthetic account fixture is far larger than that.
const typeInChunks = async (lane, text, limit = 60 * 1024) => {
  let start = 0;
  while (start < text.length) {
    let end = Math.min(text.length, start + limit);
    while (end > start + 1 && Buffer.byteLength(text.slice(start, end), 'utf8') > limit) end -= 1;
    await lane.call('type_text', { text: text.slice(start, end) });
    start = end;
  }
};

/// A wheel point on a scrolling page, clear of the cards that own their own
/// scroll areas: the right margin of the scroll container.
const wheelPoint = async (lane, id) => {
  const node = (await lane.tree()).find((candidate) => (candidate.id ?? '').endsWith(id));
  const bounds = node?.bounds ?? { x: 0, width: 600 };
  return { x: bounds.x + bounds.width - 6, y: 300 };
};

/// Scroll the pane until an element's bounds sit inside the viewport: a click
/// goes through coordinates, so an element scrolled past the edge cannot be hit.
const bringIntoView = async (lane, id, { x = 600, y = 500 } = {}) => {
  for (let attempt = 0; attempt < 8; attempt += 1) {
    const node = (await lane.tree()).find((candidate) => (candidate.id ?? '').endsWith(id));
    const top = node?.bounds?.y ?? 0;
    if (top > 160 && top < 700) return;
    await lane.call('scroll', { x, y, delta_y: top < 160 ? -300 : 300 });
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
