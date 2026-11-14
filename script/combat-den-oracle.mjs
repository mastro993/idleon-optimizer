// Extracts the Den tables the combat results panel reads, and writes the cases
// the Rust port compares against upstream's own module.
// Usage: node script/combat-den-oracle.mjs <path-to-idleon-research-optimizer>
//
// Sources: `js/stats/data/game/monsters.js` for the hound definition,
// `js/stats/data/w5/hole.js` for the Bolaia per-level column, and
// `js/stats/systems/common/combat-sim-special.js` for the maths: the fixture
// drives `initSpecial`, the scene's `_denFrame`, the hound's 100 ms timer and
// `monsterKillPre` on a fabricated sim, so it exercises the module's own
// Den functions without the map's roster or reward stream.
//
// Two files come out of one checkout:
//   - `crates/combat/data/den.json`: the hound's base HP, the Bolaia column and
//     the amplifier names the panel shows;
//   - `fixtures/combat-den.json`: the Den snapshots at each ten-second mark of
//     the scripted attempt, which `crates/combat/tests/den.rs` compares the
//     port against.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/combat-den-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { MONSTERS } = await import(join(source, 'js/stats/data/game/monsters.js'));
const scenes = await import(join(source, 'js/stats/data/game/scenes.js'));
const { HolesInfo } = await import(join(source, 'js/stats/data/game/customlists.js'));
const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const core = await import(join(source, 'js/stats/systems/common/combat-sim-core.js'));
const world = await import(join(source, 'js/stats/systems/common/combat-sim-world.js'));
const special = await import(join(source, 'js/stats/systems/common/combat-sim-special.js'));

const root = new URL('..', import.meta.url);
const save = JSON.parse(readFileSync(new URL('fixtures/data.json', root), 'utf8'));
loader.loadSaveData({ data: save });

const cleanName = (text) =>
  String(text || '')
    .replace(/[_|]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();

// ---------------------------------------------------------------------------
// Data: the hound's base row, the Bolaia column and the amplifier names.
// ---------------------------------------------------------------------------
const hound = MONSTERS.caveA;
const ampNames = Array.from({ length: 8 }, (_, index) =>
  cleanName(lists.HolesInfo[57]?.[index] ?? HolesInfo[57]?.[index] ?? `Amplifier ${index}`)
);
writeFileSync(
  new URL('crates/combat/data/den.json', root),
  JSON.stringify({
    source: 'js/stats/data/game/{monsters,customlists}.js + js/stats/data/w5/hole.js',
    hound: { name: cleanName(hound.Name), hp: Number(hound.MonsterHPTotal) || 0, respawn: Number(hound.RespawnTime) || 0 },
    bolaia: (HolesInfo[70] || []).map((value) => Number(value) || 0),
    ampNames,
  })
);

// ---------------------------------------------------------------------------
// Fixture: the Den module's own snapshots on a fabricated sim.
// ---------------------------------------------------------------------------
// The scene geometry only places the hounds; the panel never reads their x/y.
const geometry = scenes.SCENE_GEOMETRY[216];
const sim = {
  saveData: state.saveData,
  charIdx: 0,
  scene: {
    map: 216,
    name: 'SceneEvents_135',
    nodes: geometry.nodes,
    defs: {},
    N: 0,
    onScreen: 0,
    partial: [],
  },
  inputs: { values: {}, entries: {} },
  state: { gi: [], TEMP: 0, respawn: {}, ola: {} },
  rng: core.createRng(1),
  monsters: [],
  actors: [],
  sceneUpdaters: [],
  buffs: {},
  timeMs: 0,
  player: { hp: 1, hpMax: 1, damageMax: 100, accuracy: 100, dropRate: 1 },
  options: { special: { cavern: 2 } },
  addActor(actor) {
    this.actors.push(actor);
    return actor;
  },
  emit() {},
  later(ms, fn) {
    const task = { id: nextId++, ms, fn, remaining: inPass ? ms - 10 : ms, repeat: false, actor: null };
    tasks.push(task);
    return task;
  },
  every(ms, fn, actor) {
    const task = { id: nextId++, ms, fn, remaining: inPass ? ms - 10 : ms, repeat: true, actor };
    tasks.push(task);
    return task;
  },
};
let tasks = [];
let nextId = 1;
// The inputs the Den reads: the saved cavern pick and its amplifier levels.
sim.inputs.values = {
  holeCavern: 2,
  respawnRescale: 1,
  monsterHpRescale: 1,
  monsterWidth: 60,
  monsterNumFrames: 28,
  charClass: Number(state.saveData?.CharacterClass_0) || 0,
};

// `_step` order: the scheduler's tasks decrement and fire in creation order,
// then the scene updaters run, then the clock advances.
let inPass = false;
const step = (scriptedKill) => {
  inPass = true;
  for (const task of tasks) if (!task.done) task.remaining -= 10;
  const due = tasks.filter((task) => task.remaining <= 0).sort((a, b) => a.id - b.id);
  for (const task of due) {
    if (task.remaining > 0) continue;
    if (task.actor && task.actor.recycled) {
      task.done = true;
      continue;
    }
    if (task.repeat) task.remaining += task.ms;
    else task.done = true;
    task.fn();
  }
  tasks = tasks.filter((task) => !task.done);
  inPass = false;
  for (const fn of sim.sceneUpdaters.slice()) fn(sim);
  sim.timeMs += 10;
};

// The stand-in's kill task, created before `initSpecial` like `player.init`'s.
let killTask = null;
const killLive = () => {
  if (!scriptedKill) return;
  const hound = sim.monsters.find((mon) => mon.type === 'caveA' && mon.hp > 0 && !mon.recycled);
  if (!hound) return;
  // `damageMonster` drops the HP through zero; `_killHandler` books the kill
  // through the reward gate, which is the Den's `monsterKillPre`.
  hound.hp = Math.min(hound.hp, 0) - 1;
  hound.alive = 0;
  special.monsterKillPre(sim, hound);
};
let scriptedKill = false;

const snapshot = () => {
  const summary = special.specialSummary(sim);
  return {
    den: summary.state.den,
    totals: summary.den,
  };
};

const CASES = [
  { note: 'the saved amplifiers at a 2 s kill interval', killEveryMs: 2000, seconds: 60, seed: 1 },
  { note: 'a slow stand-in lets the hounds reach their fear tokens', killEveryMs: 5000, seconds: 180, seed: 3 },
  { note: 'a one-second stand-in fills the score faster', killEveryMs: 1000, seconds: 120, seed: 7 },
];

const cases = [];
for (const testCase of CASES) {
  tasks = [];
  nextId = 1;
  sim.monsters = [];
  sim.actors = [];
  sim.sceneUpdaters = [];
  sim.state = { gi: [], TEMP: 0, respawn: {}, ola: {} };
  sim.timeMs = 0;
  sim.rng = core.createRng(testCase.seed);
  if (testCase.amps) sim.inputs.values.denAmps = testCase.amps;
  else delete sim.inputs.values.denAmps;
  scriptedKill = true;
  // The stand-in's kill tick is the first task `player.init` registers.
  tasks.push({ id: nextId++, ms: testCase.killEveryMs, remaining: testCase.killEveryMs, repeat: true, actor: null, fn: killLive });
  special.initSpecial(sim);
  const marks = [];
  const steps = Math.round((testCase.seconds * 1000) / 10);
  for (let pass = 0; pass < steps; pass++) {
    step();
    if ((pass + 1) % 1000 === 0) marks.push({ tMs: sim.timeMs, ...snapshot().den });
  }
  const end = snapshot();
  const holes = state.saveData.holesData || [];
  const resolvedAmps = testCase.amps || (holes[12] || []).slice(0, 8).map((value) => Number(value) || 0);
  if (!testCase.amps && resolvedAmps.length < 8) resolvedAmps.length = 8;
  cases.push({
    note: testCase.note,
    killEveryMs: testCase.killEveryMs,
    seconds: testCase.seconds,
    seed: testCase.seed,
    config: {
      amps: resolvedAmps,
      bUpg23: Number(holes[13]?.[23]) >= 1 ? 1 : 0,
      bolaiaLevel: Number(holes[26]?.[2]) || 0,
      bestScore: Number(holes[11]?.[8]) || 0,
      opalLevel: Number(holes[7]?.[2]) || 0,
      houndHp: Number(hound.MonsterHPTotal) || 0,
    },
    expected: {
      den: end.den,
      totals: end.totals,
      marks,
    },
  });
  console.log(`${testCase.note}: score ${end.den.score}, kills ${end.den.kills}, wave ${end.den.wave}, fear ${end.den.fear}, opals ${end.den.opals}`);
}

// The amplifier, score-multiplier and opal-requirement formulas on their own.
const AMP_CASES = [
  [0, 0, 0, 0, 0, 0, 0, 0],
  [3, 1, 4, 2, 0, 5, 7, 6],
  [10, 8, 2, 0, 9, 1, 3, 4],
];
const formulaCases = AMP_CASES.map((amps) => ({
  amps,
  debuff: amps.map((_, kind) => special.ampDebuff(amps, kind)),
  multi: [0, 1, 3].map((bUpg) => ({ bUpg, value: special.ampMulti(amps, bUpg) })),
}));
const opalCases = [0, 1, 7, 21, 45].map((level) => ({
  level,
  requirement: special.houndNextOpalScoreReq(level),
}));

const fixture = {
  source:
    'js/stats/systems/common/combat-sim-special.js through script/combat-den-oracle.mjs',
  hound: { name: cleanName(hound.Name), hp: Number(hound.MonsterHPTotal) || 0 },
  cases,
  formulaCases,
  opalCases,
};
writeFileSync(new URL('fixtures/combat-den.json', root), JSON.stringify(fixture, null, 1));
console.log(`wrote ${cases.length} Den cases`);
