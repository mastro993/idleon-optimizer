// Extracts the Worship Tower Defense tables the Rust app ships from the upstream JS repo.
// Usage: node script/extract-worship-td-data.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-worship-td-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const td = await import(join(source, 'js/stats/data/w3/worship-td.js'));
const customlists = await import(join(source, 'js/stats/data/game/customlists.js'));
const hardcoded = await import(join(source, 'js/stats/data/game/hardcoded.js'));
const scenesData = await import(join(source, 'js/stats/data/game/scenes.js'));

const scenes = new Set();
const trials = [];
for (let trial = 0; trial < td.TD_TRIAL_COUNT; trial += 1) {
  const base = td.tdTrialBase(trial);
  const path = td.tdTrialPath(trial);
  const scene = td.tdTrialScene(trial);
  const mob = td.tdMob(trial);
  const sprite = td.tdMobSprite(trial);
  scenes.add(base.map);
  trials.push({
    trial,
    map: base.map,
    hpBase: base.hpBase,
    mobKey: base.mob,
    totemX: base.totemX,
    totemY: base.totemY,
    charge: base.charge,
    effReq: base.effReq,
    path: path.path,
    flags: path.flags,
    scene: { plat: scene.plat, w: scene.w, h: scene.h },
    background: td.TD_TRIAL_BG[trial],
    waves: customlists.WorshipInfo[trial].map((row) => row.slice(0, 4).map(Number)),
    mobDef: {
      key: mob.key,
      name: mob.name,
      moveSpeed: mob.moveSpeed,
      height: mob.height,
      frameW: mob.frameW,
      frameH: mob.frameH,
    },
    sprite: {
      across: sprite.across,
      down: sprite.down,
      idle: sprite.idle,
      walk: sprite.walk,
    },
    npcs: td.tdTrialNpcZones(trial),
  });
}

// Scene node tables for the maps those trials use.
const geometry = {};
for (const map of scenes) {
  const geom = scenesData.SCENE_GEOMETRY[map];
  geometry[map] = {
    nodes: geom.nodes.map((node) => ({
      x: node.x,
      length: node.length,
      y: node.y,
      neighbors: (node.neighbors || []).map(Number),
    })),
    portals: (geom.portals || []).map((portal) => portal.map(Number)),
  };
}

const towers = [];
for (let type = 0; type < td.TD_TOWER_COUNT; type += 1) {
  const def = td.tdTowerDef(type);
  towers.push({
    name: def.name,
    perLvA: def.perLvA,
    perLvB: def.perLvB,
    baseCost: def.baseCost,
  });
}

const traits = [];
for (let type = 0; type < td.TD_TOWER_COUNT; type += 1) {
  const row = td.tdTrait(type);
  traits.push({
    a: { value: row.A.value, key: row.A.key },
    b: { value: row.B.value, key: row.B.key },
  });
}

// Menagerie unlock 18 doubles the first trait; boost 15 sets the pre-cook seconds.
const menagerieResearch = await import(join(source, 'js/stats/data/w7/menagerie.js'));
const payload = {
  trials,
  geometry,
  towers,
  traits,
  // `superBitType(t, Gaming[12])` matches the token at N2L[t].
  superBits: { 6: hardcoded.N2L[6], 15: hardcoded.N2L[15] },
  menagerieBoost15PerLevel: menagerieResearch.menagerieBoostPerLevel(15),
};

writeFileSync(
  new URL('../crates/core/data/worship-td.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(
  `wrote ${trials.length} trials, ${Object.keys(geometry).length} maps, ${towers.length} towers`,
);
