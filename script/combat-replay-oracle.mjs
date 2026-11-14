// Extracts the replay tables the combat simulator reads and writes the frames
// and the Killroy handoff the Rust port must reproduce.
// Usage: node script/combat-replay-oracle.mjs <path-to-idleon-research-optimizer>
//
// Sources: `js/stats/systems/common/combat-sim-replay.js` (the delta codec: the
// encoder, `packReplayFrames` and the store), `js/ui/killroy-replay.js` (the
// Killroy handoff: capture, store, read and its validation),
// `js/stats/systems/common/combat-sim-killroy.js` for the room map ids, and
// `js/stats/systems/common/combat-sim-setup.js` for `characterOptions`.
//
// Two files come out of one checkout, so they cannot disagree:
//   - `crates/combat/data/replay.json`: the room map ids a Killroy replay may
//     name, so validation can reject a map that is not a Killroy room;
//   - `fixtures/combat-replay.json`: a scripted snapshot series through the
//     module's own encoder, the frames it produces, the dictionary batches and
//     the store's materialized frames, plus the Killroy handoff's capture and
//     its accept/reject cases. `crates/combat/tests/replay.rs` compares the port
//     against them: the bytes must match exactly, because the port writes the
//     frames the store reads.
//
// Snapshot object keys are sorted before they reach the encoder. JavaScript
// keeps an object's insertion order and Rust's `serde_json` map is ordered, so
// sorting makes both sides visit fields in the same order and the encoded bytes
// comparable.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/combat-replay-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const replay = await import(
  join(source, 'js/stats/systems/common/combat-sim-replay.js')
);
const killroyReplay = await import(join(source, 'js/ui/killroy-replay.js'));
const killroy = await import(
  join(source, 'js/stats/systems/common/combat-sim-killroy.js')
);
const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));

const root = new URL('..', import.meta.url);

// ---------------------------------------------------------------------------
// Data: the maps a Killroy replay may name.
// ---------------------------------------------------------------------------
const roomMaps = killroy.killroyMapIds();
writeFileSync(
  new URL('crates/combat/data/replay.json', root),
  JSON.stringify({
    source:
      'js/stats/systems/common/combat-sim-killroy.js + js/ui/killroy-replay.js',
    keyframeEvery: 300,
    snapshotMs: 100,
    killroyMaps: roomMaps,
  })
);

// ---------------------------------------------------------------------------
// Fixture: the codec's own bytes and frames.
// ---------------------------------------------------------------------------
const canonical = (value) => {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    const out = {};
    for (const key of Object.keys(value).sort()) out[key] = canonical(value[key]);
    return out;
  }
  return value;
};

// A snapshot series covering every encoding path: keyframes and deltas,
// quantized positions, small and large integers, negative zero, floats,
// booleans, nulls, interned strings, plain and nested objects, field deletes,
// uid lists with template reuse, a list whose uid prefix changes, a list that
// empties, and an array that stops being a uid list.
const SERIES = [
  {
    t: 0,
    player: { x: 0, y: 0, vx: 0, vy: 0, anim: '0', hp: 100.5, alive: true, equip: null },
    monsters: [{ uid: 'm1', id: 1, type: 'mushG', x: 100, y: 300, hp: 50, alive: 1 }],
    projectiles: [{ uid: 'b1', x: 110.25, y: 302.75, anim: '1' }],
    items: [],
    den: { active: 1, wave: 1, fear: 0.5, score: 12 },
    tags: ['a', 'b'],
    note: 'spawn',
    flag: false,
    big: 4294967296,
    safe: 9007000000000000,
    neg: -9007199254740991,
    frac: 0.25,
    empty: null,
  },
  {
    t: 100,
    player: { x: 3.4, y: 0, vx: -0.2, vy: 0, anim: '1', hp: 100.5, alive: true, equip: { helmet: 'B', weapon: 'C' } },
    monsters: [
      { uid: 'm1', id: 1, type: 'mushG', x: 103.4, y: 300, hp: 40.25, alive: 1 },
      { uid: 'm2', id: 2, type: 'mushG', x: 140.5, y: 300, hp: 50, alive: 1 },
    ],
    projectiles: [],
    items: [{ uid: 'i1', type: 'COIN', qty: 3, x: 12.5, y: 20, status: 0 }],
    den: { active: 1, wave: 1, fear: 0.5, score: 12, chain: 0 },
    tags: ['a', 'b'],
    note: 'kill',
    flag: true,
    big: 4294967296,
    safe: 9007000000000000,
    neg: -9007199254740991,
    frac: 0.5,
    empty: null,
  },
  {
    t: 200,
    player: { x: 6.8, y: 0, vx: -0.2, vy: 0, anim: '1', hp: 99, alive: true, equip: { helmet: 'B', weapon: 'C' } },
    monsters: [{ uid: 'm2', id: 2, type: 'mushG', x: 140.5, y: 300, hp: 20, alive: 1 }],
    projectiles: [{ uid: 'b2', x: 150, y: 305, anim: '10' }],
    items: [
      { uid: 'i1', type: 'COIN', qty: 3, x: 12.5, y: 20, status: 1 },
      { uid: 'i2', type: 'COIN', qty: 4, x: 12.5, y: 20, status: 0 },
      { uid: 'i3', type: 'COIN', qty: 5, x: 12.5, y: 20, status: 0 },
    ],
    den: { active: 1, wave: 2, fear: 0.75, score: 30 },
    tags: ['b'],
    note: 'spawn',
    flag: false,
    big: 1,
    safe: 42,
    neg: -1,
    frac: 2.5,
    empty: null,
  },
  {
    t: 300,
    player: { x: 10.2, y: 0, vx: 0, vy: 0, anim: '2_', hp: 99, alive: true, equip: { helmet: 'B', weapon: 'C' } },
    monsters: [],
    projectiles: [
      { uid: 'b2', x: 160, y: 305, anim: '10' },
      { uid: 'b3', x: 170.5, y: 305, anim: '10' },
    ],
    items: [],
    den: { active: 0.5, wave: 2, fear: 49.5, score: 30 },
    tags: [],
    note: 'end',
    flag: false,
    big: 1,
    safe: 42,
    neg: -1,
    frac: 2.5,
    empty: null,
  },
  {
    t: 400,
    player: { x: 10.2, y: 0, vx: 0, vy: 0, anim: '2_', hp: 99, alive: false, equip: { helmet: 'B', weapon: 'C' } },
    monsters: [{ uid: 'm3', id: 3, type: 'mushW', x: 60, y: 320, hp: 70, alive: 1 }],
    projectiles: [],
    items: [{ uid: 'i9', type: 'COIN', qty: 100, x: 0, y: 0, status: 2 }],
    tags: ['a', 'b', 'c'],
    note: 'end',
    flag: true,
    big: 1,
    safe: 42,
    neg: -1,
    frac: 2.5,
    empty: null,
  },
  {
    t: 500,
    player: { x: 10.2, y: -2.5, vx: 0, vy: 0, anim: '2_', hp: 99, alive: true, equip: { helmet: 'B', weapon: 'C' } },
    monsters: [
      { uid: 'm3', id: 3, type: 'mushW', x: 60, y: 320, hp: 70, alive: 1 },
      { uid: 'm4', id: 4, type: 'mushW', x: 61, y: 320, hp: 70, alive: 1 },
      { uid: 'm5', id: 5, type: 'mushW', x: 62, y: 320, hp: 70, alive: 1 },
    ],
    projectiles: [{ uid: 'i1189', x: 0, y: 0, anim: 'WT8' }],
    items: [],
    den: { active: 0, wave: 2, fear: 50, score: 30 },
    note: 'respawn',
    flag: false,
    big: 1,
    safe: 42,
    neg: -1,
    frac: 2.5,
    empty: null,
  },
  {
    t: 600,
    player: { x: 10.2, y: -2.5, vx: 0, vy: 0, anim: '2_', hp: 99, alive: true, equip: { helmet: 'B', weapon: 'C' } },
    monsters: [
      { uid: 'm3', id: 3, type: 'mushW', x: 60, y: 320, hp: 70, alive: 1 },
      { uid: 'm4', id: 4, type: 'mushW', x: 61, y: 320, hp: 70, alive: 1 },
      { uid: 'm5', id: 5, type: 'mushW', x: 62, y: 320, hp: 70, alive: 1 },
      { uid: 'm6', id: 6, type: 'mushW', x: 63, y: 320, hp: 70, alive: 1 },
    ],
    projectiles: [
      { uid: 'i1189', x: 0, y: 0, anim: 'WT8' },
      { uid: 'i1190', x: 1, y: 0, anim: 'WT8' },
    ],
    items: [],
    tags: ['z'],
    note: 'respawn',
    flag: false,
    big: 1,
    safe: 42,
    neg: -1,
    frac: 2.5,
    empty: null,
  },
  {
    t: 700,
    player: { x: 10.2, y: -2.5, vx: 0, vy: 0, anim: '2_', hp: 99, alive: true, equip: null },
    monsters: [
      { uid: 'm3', id: 3, type: 'mushW', x: 60, y: 320, hp: 70, alive: 1 },
      { uid: 'm6', id: 6, type: 'mushW', x: 63, y: 320, hp: 70, alive: 1 },
    ],
    projectiles: [{ uid: 'b7', x: 2, y: 0, anim: 'WT8' }],
    items: [],
    tags: [],
    note: 'spawn',
    flag: false,
    big: 1,
    safe: 42,
    neg: -1,
    frac: 2.5,
    empty: null,
  },
];

const KEYFRAME_EVERY = 3;
const BATCH = 3;
const snapshots = SERIES.map(canonical);
const encoder = replay.createReplayEncoder({ keyframeEvery: KEYFRAME_EVERY });
const store = replay.createReplayStore({ cacheSize: 64, materializeWindow: 8 });
const frames = [];
const batches = [];
let batch = [];
const flush = () => {
  if (!batch.length) return;
  const keys = encoder.newKeys();
  const packed = replay.packReplayFrames(batch);
  store.append({ keysFrom: keys.keysFrom, keys: keys.keys, buf: packed.buf, lens: packed.lens });
  batches.push({
    keysFrom: keys.keysFrom,
    keys: keys.keys,
    frames: batch.map((bytes) => Buffer.from(bytes).toString('base64')),
  });
  batch = [];
};
for (const snapshot of snapshots) {
  const bytes = encoder.push(snapshot);
  frames.push(Buffer.from(bytes).toString('base64'));
  batch.push(bytes);
  if (batch.length >= BATCH) flush();
}
flush();
const decoded = [];
for (let index = 0; index < frames.length; index++) {
  decoded.push(canonical(store.get(index)));
}

const codec = {
  keyframeEvery: KEYFRAME_EVERY,
  batch: BATCH,
  dictionary: encoder.keys,
  bytes: encoder.bytes,
  frames,
  batches,
  decoded,
};

// ---------------------------------------------------------------------------
// Fixture: the Killroy handoff's capture and its validation.
// ---------------------------------------------------------------------------
// A small purpose-built save: the handoff only counts characters, so the
// fixture does not need the full synthetic account.
const save = {
  Research: JSON.stringify(Array.from({ length: 9 }, () => [])),
  charNames: ['Ana', 'Bo'],
  Lv0_0: JSON.stringify([120]),
  CharacterClass_0: 7,
  Lv0_1: JSON.stringify([80]),
  CharacterClass_1: 1,
};
loader.loadSaveData({ data: save });
const mem = new Map();
const memStore = {
  setItem: (key, value) => mem.set(key, value),
  getItem: (key) => (mem.has(key) ? mem.get(key) : null),
  removeItem: (key) => mem.delete(key),
};
const baseJob = {
  setup: { charIdx: 0, patches: [] },
  charIdx: 0,
  mapIdx: roomMaps[0],
  seconds: 300,
  seed: 20260101,
  inputOverrides: { useSavedBuffs: true, unlimitedInventory: false },
};
const captured = killroyReplay.captureKillroyReplay(save, baseJob, 'Room practice');
const link = killroyReplay.storeKillroyReplay(memStore, 'token', captured);
const read = killroyReplay.readKillroyReplay(
  memStore,
  link.slice(link.indexOf('#'))
);
const readBack =
  JSON.stringify(canonical(read)) === JSON.stringify(canonical(captured));

const invalid = (payload) => {
  memStore.setItem('killroy-replay:bad', JSON.stringify(payload));
  try {
    killroyReplay.readKillroyReplay(memStore, '#killroy-replay=bad');
    return null;
  } catch (error) {
    return String(error && error.message ? error.message : error);
  }
};
const withPayload = (over) => Object.assign({}, captured, over);
const killroyCases = [
  { note: 'the captured payload', payload: captured, error: null },
  { note: 'a version this build does not read', payload: withPayload({ version: 2 }) },
  { note: 'no save to replay', payload: withPayload({ raw: null }) },
  { note: 'a label that is not text', payload: withPayload({ label: 7 }) },
  { note: 'a character the save does not carry', payload: withPayload({ charIdx: 99 }) },
  { note: 'a fractional character index', payload: withPayload({ charIdx: 0.5 }) },
  { note: 'a map that is not a Killroy room', payload: withPayload({ mapIdx: 3 }) },
  { note: 'a run shorter than a second', payload: withPayload({ seconds: 0 }) },
  { note: 'a run longer than a day', payload: withPayload({ seconds: 86401 }) },
  { note: 'a seed outside the game range', payload: withPayload({ seed: 0 }) },
  { note: 'a seed past 32 bits', payload: withPayload({ seed: 4294967296 }) },
  { note: 'a fractional seed', payload: withPayload({ seed: 1.5 }) },
];
for (const testCase of killroyCases) {
  testCase.error = invalid(testCase.payload);
}

const fixture = {
  source:
    'js/stats/systems/common/combat-sim-replay.js and js/ui/killroy-replay.js through script/combat-replay-oracle.mjs',
  codec,
  snapshots,
  killroy: {
    roomMaps,
    captured: canonical(captured),
    readBack,
    cases: killroyCases.map((testCase) => ({
      note: testCase.note,
      payload: canonical(testCase.payload),
      error: testCase.error,
    })),
  },
};
writeFileSync(
  new URL('fixtures/combat-replay.json', root),
  JSON.stringify(fixture, null, 1)
);
console.log(
  `wrote ${snapshots.length} snapshots, ${frames.length} frames (${encoder.bytes} bytes), ` +
    `${killroyCases.length} Killroy cases and ${roomMaps.length} room maps`
);
