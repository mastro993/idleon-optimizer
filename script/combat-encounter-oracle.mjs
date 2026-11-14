// Extracts the encounter tables the combat simulator's run reads and writes the
// outcomes the Rust port must reproduce.
// Usage: node script/combat-encounter-oracle.mjs <path-to-idleon-research-optimizer>
//
// Sources: `js/stats/data/game/scenes.js` (`SCENE_GEOMETRY`, `SCENE_SPAWNS`),
// `js/stats/data/game/monsters.js` (`MONSTERS`), `js/stats/data/game/customlists.js`
// (`MapDispName`, `MapDetails`, `NinjaInfo`) and, for the fixture, the upstream
// engine itself: `combat-sim-core.js` (`createCombatSim`, `scriptedPlayer`,
// `idlePlayer`), `combat-sim-world.js` (`defaultPlayerNode`, `numberOfEnemies`)
// and `js/save/loader.js`.
//
// Two files come out of one checkout, so they cannot disagree:
//   - `crates/combat/data/encounters.json`: the maps, their spawn roster and the
//     base monster rows the app runs;
//   - `fixtures/combat-encounter.json`: the outcomes the `standIn` and `idle`
//     players produce on the committed synthetic save, which
//     `crates/combat/tests/encounter.rs` compares the port against.
//
// The fixture runs at neutral monster tuning (`respawnRescale`,
// `monsterHpRescale` and `respawnBonusPct` cleared) so it does not depend on the
// upstream stat context, and with crystal, giant and plunder spawning off: the
// port models the spawn/respawn/kill loop only, and those temp spawns belong to
// the reward engine (CS-05). The engine's other reward draws do not change the
// roster the stand-in kills, so kills and damage stay comparable.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/combat-encounter-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const scenes = await import(join(source, 'js/stats/data/game/scenes.js'));
const { MAP_SCENES, MOB_SPRITES } = await import(
  join(source, 'js/stats/data/game/combat-sprites.js')
);
const { MONSTERS } = await import(join(source, 'js/stats/data/game/monsters.js'));
const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const core = await import(join(source, 'js/stats/systems/common/combat-sim-core.js'));
const world = await import(join(source, 'js/stats/systems/common/combat-sim-world.js'));

// Maps whose scene the port does not run: the rift (166), the cavern menus
// (216), Clam Work's saved mob count (306), the four maps whose monster HP a
// NinjaInfo row rewrites from the player's BoneJoePickle count, and any map
// whose scene script spawns a monster with no extracted def.
const EXCLUDED = {
  166: 'The Rift swaps its monster for a random rift mob; its kill tasks are not modelled.',
  216: 'The Hole needs the saved cavern pick and its node layout; the cavern menus are not modelled.',
  306: 'Clam Work spawns a monster count derived from the save, which this build does not read.',
};
for (const idx of Object.keys(lists.MapDispName).map(Number)) {
  const types = (scenes.SCENE_SPAWNS[idx]?.mobs || []).map((row) => row[0]);
  if (types.some((type) => (lists.NinjaInfo?.[30] || []).includes(type))) {
    EXCLUDED[idx] = 'BoneJoePickle rewrites this map\'s monster HP from the save, which this build does not read.';
  }
}

const maps = [];
for (const key of Object.keys(scenes.SCENE_SPAWNS)) {
  const idx = Number(key);
  const spawns = scenes.SCENE_SPAWNS[key];
  if (EXCLUDED[idx] || !spawns?.mobs?.length || !scenes.SCENE_GEOMETRY[key]) continue;
  const missing = spawns.mobs.filter(([type]) => !MONSTERS[type]).map(([type]) => type);
  if (missing.length) {
    EXCLUDED[idx] = `The scene spawns ${missing.join(', ')}, which has no extracted monster row.`;
    continue;
  }
  const geometry = scenes.SCENE_GEOMETRY[key];
  const art = MAP_SCENES[idx];
  const sprite = (type) => {
    const row = MOB_SPRITES[type];
    if (!row) return null;
    return {
      file: String(row.file ?? ''),
      across: Number(row.across) || 1,
      down: Number(row.down) || 1,
      frames: Number(row.frames) || 1,
      fw: Number(row.fw) || 50,
      fh: Number(row.fh) || 50,
      ox: Number(row.ox) || 0,
      oy: Number(row.oy) || 0,
      movingFrame: Number(row.movingFrame) || 0,
      deathFrame: Number(row.deathFrame) || 0,
    };
  };
  maps.push({
    index: idx,
    name: String(lists.MapDispName[idx] ?? ''),
    scene: String(spawns.scene ?? ''),
    // The replay draws the map's own background and platform art and stages
    // each monster on its spawn node; the run never moves them, so their x
    // within the platform is the platform's centre.
    art: {
      name: String(art?.sceneName ?? ''),
      width: Number(art?.w) || 960,
      height: Number(art?.h) || 540,
      bg: art?.bg ?? null,
      plat: art?.plat ?? null,
      sky: art?.sky ? (art.sky.color ? [String(art.sky.color)] : [String(art.sky.top), String(art.sky.bottom)]) : null,
      playerNode: world.defaultPlayerNode(idx),
      nodes: geometry.nodes.map((node) => ({
        x: Number(node.x) || 0,
        y: Number(node.y) || 0,
        length: Number(node.length) || 0,
      })),
    },
    mobs: spawns.mobs.map(([type, node]) => ({
      type,
      node: Number(node) || 0,
      hp: Number(MONSTERS[type].MonsterHPTotal) || 0,
      respawn: Number(MONSTERS[type].RespawnTime) || 0,
      height: Number(MONSTERS[type].HeightOfMonster) || 0,
      monType: String(MONSTERS[type].Type ?? ''),
      sprite: sprite(type),
    })),
  });
}
maps.sort((a, b) => a.index - b.index);

const data = {
  source: 'js/stats/data/game/{scenes,monsters,customlists}.js',
  excluded: Object.fromEntries(
    Object.entries(EXCLUDED).sort(([a], [b]) => Number(a) - Number(b))
  ),
  maps,
};
writeFileSync(
  new URL('../crates/combat/data/encounters.json', import.meta.url),
  JSON.stringify(data)
);

// ---------------------------------------------------------------------------
// Fixture: the upstream engine's own outcomes for the two players the port runs.
// ---------------------------------------------------------------------------
const save = JSON.parse(
  readFileSync(new URL('../fixtures/data.json', import.meta.url), 'utf8')
);
loader.loadSaveData({ data: save });

const standInPick = (sim) => {
  let first = null;
  for (const mon of sim.monsters) {
    if (mon.alive !== 1 || !(mon.hp > 0) || mon.recycled || mon.willDie !== 0) continue;
    if (mon.temp === 1) return mon;
    if (!first) first = mon;
  }
  return first;
};

// Neutral monster tuning and no temp spawns: the halves of the engine the port
// does not model, held at their neutral value so both sides see one roster.
const OVERRIDES = {
  respawnRescale: 1,
  monsterHpRescale: 1,
  respawnBonusPct: {},
  crystalSpawnRaw: 0,
  crystalSpawnCap: 0,
  guaranteedCrystalMobs: 0,
  giantMob: 0,
  prayer18: 0,
  chipCrys: 0,
};

const CASES = [
  { map: 1, player: 'standIn', killEveryMs: 700, seconds: 120 },
  { map: 2, player: 'standIn', killEveryMs: 100, seconds: 120 },
  { map: 6, player: 'standIn', killEveryMs: 500, seconds: 300 },
  { map: 60, player: 'standIn', killEveryMs: 250, seconds: 120 },
  { map: 119, player: 'standIn', killEveryMs: 400, seconds: 180 },
  { map: 1, player: 'idle', seconds: 60 },
];

const cases = [];
for (const testCase of CASES) {
  const { map, player: mode, seconds } = testCase;
  const spawns = scenes.SCENE_SPAWNS[map];
  const player =
    mode === 'idle'
      ? core.idlePlayer(world.defaultPlayerNode(map))
      : core.scriptedPlayer(world.defaultPlayerNode(map), {
          killEveryMs: testCase.killEveryMs,
          pick: standInPick,
        });
  const sim = core.createCombatSim({
    saveData: state.saveData,
    charIdx: 0,
    mapIdx: map,
    seed: 1,
    options: { rawSave: { data: save }, player, inputOverrides: OVERRIDES },
  });
  if (sim.unsupported) throw new Error(`map ${map} is unsupported upstream: ${sim.unsupported}`);
  const killsByType = {};
  let kills = 0;
  let damage = 0;
  sim.on('monsterKilled', (payload) => {
    kills += 1;
    killsByType[payload.type] = (killsByType[payload.type] || 0) + 1;
  });
  sim.on('damage', (payload) => {
    damage += Number(payload.amount) || 0;
  });
  const summary = sim.run(seconds);
  const respawn = sim.monsters.map(
    (mon) => sim.state.respawn[mon.id + sim.scene.N] ?? null
  );
  cases.push({
    map,
    scene: String(spawns.scene ?? ''),
    player: mode,
    killEveryMs: mode === 'idle' ? 0 : testCase.killEveryMs,
    seconds,
    expected: {
      simulatedMs: summary.timeMs,
      steps: summary.steps,
      frames: summary.frames,
      kills,
      damage,
      killsByType,
      respawn,
    },
  });
  console.log(
    `${map} ${mode} ${seconds}s: ${kills} kills, damage ${damage}, ` +
      `${summary.steps} steps, ${summary.frames} frames`
  );
}

const fixture = {
  source: 'js/stats/systems/common/combat-sim-*.js through script/combat-encounter-oracle.mjs',
  overrides: OVERRIDES,
  cases,
};
writeFileSync(
  new URL('../fixtures/combat-encounter.json', import.meta.url),
  JSON.stringify(fixture, null, 1)
);
console.log(`wrote ${maps.length} maps and ${cases.length} fixture cases`);
