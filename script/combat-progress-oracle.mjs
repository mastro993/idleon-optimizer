// Extracts the forge rows and Scurge table the combat progress panel reads, and
// writes the cases the Rust port compares against upstream's own module.
// Usage: node script/combat-progress-oracle.mjs <path-to-idleon-research-optimizer>
//
// Sources: `js/stats/data/game/items.js` for the `Cooldown`/`Amount`/`Effect`
// rows the forge reads, `js/stats/data/game/customlists.js` for the Spelunky
// Scurge upgrade column, and `js/stats/systems/common/combat-sim-progress.js`
// for the maths: the fixture drives `initCombatProgress` and the module's own
// grant, tick, Scurge, Coral and Omega methods on a seeded stream and records
// what `combatProgressSummary` reports.
//
// Two files come out of one checkout:
//   - `crates/combat/data/progress.json`: the forge item rows, the Spelunky
//     Scurge column and the pet names the shiny table shows;
//   - `fixtures/combat-progress.json`: the upstream module's own summaries,
//     which `crates/combat/tests/progress.rs` compares the port against, plus
//     the `combatProgressInputs` config the save reader in
//     `crates/import/src/combat.rs` is compared with.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/combat-progress-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { ITEMS } = await import(join(source, 'js/stats/data/game/items.js'));
const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const breeding = await import(join(source, 'js/stats/data/w4/breeding.js'));
const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const core = await import(join(source, 'js/stats/systems/common/combat-sim-core.js'));
const world = await import(join(source, 'js/stats/systems/common/combat-sim-world.js'));
const progress = await import(join(source, 'js/stats/systems/common/combat-sim-progress.js'));

const root = new URL('..', import.meta.url);
const save = JSON.parse(readFileSync(new URL('fixtures/data.json', root), 'utf8'));
loader.loadSaveData({ data: save });

// `cleanName` in combat-sim-calc.html: underscores and pipes become spaces.
const cleanName = (text) =>
  String(text || '')
    .replace(/[_|]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();

// ---------------------------------------------------------------------------
// Data: the forge item rows, the Spelunky Scurge column and the pet names.
// ---------------------------------------------------------------------------
const items = {};
for (const [key, def] of Object.entries(ITEMS)) {
  if (!def) continue;
  // The forge reads the ore and oil columns; the bars only need their names.
  const forge = def.Cooldown != null || def.Amount != null || def.Effect != null;
  if (!forge && def.typeGen !== 'bBar') continue;
  items[key] = {
    name: cleanName(def.displayName) || key,
    cooldown: Number(def.Cooldown) || 0,
    amount: Number(def.Amount) || 0,
    effect: String(def.Effect ?? ''),
  };
}
// `Blank` carries no forge fields; `_forgeStats` still reads its row.
items.Blank = { name: 'Blank', cooldown: 0, amount: 0, effect: '' };

const petNames = breeding.PET_NAMES.map((world) => world.map((name) => cleanName(name)));
const petKeys = breeding.PET_STATS.map((world) => world.map((pet) => String(pet[0])));

writeFileSync(
  new URL('crates/combat/data/progress.json', root),
  JSON.stringify({
    source: 'js/stats/data/game/{items,customlists}.js + js/stats/data/w4/breeding.js',
    items,
    // `Spelunky[28]`: the Scurge upgrade column `scurgeKill` multiplies.
    scurgeColumn: (lists.Spelunky[28] || []).map((value) => Number(value) || 0),
    petNames,
    petKeys,
  })
);

// ---------------------------------------------------------------------------
// Fixture: the upstream module's own summaries on a seeded stream.
// ---------------------------------------------------------------------------
const idle = () => core.idlePlayer(world.defaultPlayerNode(1));
const base = () => {
  const sim = core.createCombatSim({
    saveData: state.saveData,
    charIdx: 0,
    mapIdx: 1,
    seed: 1,
    options: { rawSave: { data: save }, player: idle() },
  });
  return sim;
};
const baseInputs = base().inputs;
const defaultConfig = () => JSON.parse(JSON.stringify(baseInputs.values.combatProgress));
// A case only carries the section it exercises; the rest stay at the neutral
// values `Config::default` holds, so both sides start from the same state.
const NEUTRAL = {
  forge: {
    available: false, order: [], qty: [], progress: [], storage: {},
    storageAvailable: false, slots: 0, cap: 0, refill: false, gemSlots: 0,
    speed: 1, bars: 1, oilUse: 1,
  },
  pets: { available: false, deployed: [], breeding: [], shinySpeed: 1 },
  scurge: { unlocked: false, menagerie: [] },
  coral: { unlocked: false, kills: 0 },
  omega: { unlocked: false, available: false, balls: 0, drops: 0 },
};
const only = (key, value) => ({ ...NEUTRAL, [key]: value });
// The save stores some Breeding cells as strings; `N` coerces them upstream.
const numericRows = (rows) => rows.map((row) => row.map((value) => Number(value) || 0));
const stringRows = (rows) => rows.map((row) => row.map((value) => String(value)));
const makeSim = (config, seed) => {
  const inputs = {
    ...baseInputs,
    values: { ...baseInputs.values, combatProgress: config },
  };
  const sim = core.createCombatSim({
    saveData: state.saveData,
    charIdx: 0,
    mapIdx: 1,
    seed,
    options: { rawSave: { data: save }, inputs, player: idle() },
  });
  sim.rng = core.createRng(seed);
  return sim;
};

const blank = (count, value = 'Blank') => Array(count).fill(value);

// The furnace the cases share: one ore, one oil and one bar, with the oil's
// Effect and the gem slots driving the produced quantity.
const furnace = (over = {}) => {
  const forge = defaultConfig().forge;
  forge.order = blank(48);
  forge.qty = blank(48, 0);
  forge.progress = Array(16).fill(0);
  forge.storage = {};
  forge.slots = 16;
  forge.cap = 1e6;
  forge.refill = false;
  forge.gem_slots = 0;
  forge.speed = 5;
  forge.bars = 1.5;
  forge.oil_use = 1;
  forge.order[0] = 'Copper';
  forge.order[1] = 'OilBarrel1';
  forge.order[2] = 'CopperBar';
  forge.qty[0] = 100000;
  forge.qty[1] = 1000;
  forge.qty[2] = 0;
  return { ...forge, ...over };
};

// One summary, without the cloned furnace state's `storage` object identity.
const summary = (sim) => {
  const s = progress.combatProgressSummary(sim);
  return { ...s, calls: sim.rng.calls };
};

// A script step runs one upstream method on the sim.
const runScript = (sim, steps) => {
  for (const step of steps) {
    if (step.grant) sim.progress.grant(step.grant, step.seconds);
    else if (step.scurgeKill) sim.progress.scurgeKill();
    else if (step.coralKill != null) sim.progress.coralKill(step.coralKill);
    else if (step.omegaKill) sim.progress.omegaKill(step.omegaKill);
    else if (step.ticks) {
      for (let i = 0; i < step.ticks; i++) sim.sched.update(10);
    } else if (step.petBatch) sim.sceneUpdaters[0](sim);
  }
  return summary(sim);
};

const simple = (sim) => {
  const s = progress.combatProgressSummary(sim);
  return {
    gains: {
      forgeSeconds: s.forgeSeconds,
      petSeconds: s.petSeconds,
      alchemySeconds: s.alchemySeconds,
      scurgeExp: s.scurgeExp,
      coralKills: s.coralKills,
      omegaBalls: s.omegaBalls,
    },
    bars: s.bars,
    oreUsed: s.oreUsed,
    oilUsed: s.oilUsed,
    storageUsed: s.storageUsed,
    notices: s.notices.map((n) => n.text),
    state: {
      petPendingSeconds: s.state.petPendingSeconds,
      forgeAvailable: s.state.forgeAvailable,
      forge: s.state.forge && {
        order: s.state.forge.order,
        qty: s.state.forge.qty,
        progress: s.state.forge.progress,
        storage: s.state.forge.storage,
      },
      scurge: s.state.scurge,
      coral: s.state.coral,
      omega: s.state.omega,
    },
    pets: s.pets,
    calls: sim.rng.calls,
  };
};

const forgeCases = [];
const forgeCase = (note, config, seed, steps) => {
  const sim = makeSim(config, seed);
  runScript(sim, steps);
  forgeCases.push({ note, config, seed, steps, expected: simple(sim) });
};
forgeCase('grant: one hour with a speed oil and no gem slots', only('forge', furnace()), 7, [
  { grant: 'forge', seconds: 3600 },
]);
forgeCase('grant: gem slots pay another half bar', only('forge', furnace({
  gem_slots: 2,
  order: ['Copper', 'OilBarrel2', 'CopperBar', ...blank(45)],
  qty: [4000, 400, 0, ...blank(45, 0)],
})), 11, [{ grant: 'forge', seconds: 600 }, { grant: 'forge', seconds: 30 }]);
forgeCase('grant: storage refills the ore and the oil', only('forge', furnace({
    refill: true,
    storage_available: true,
    storage: { Copper: 300, OilBarrel1: 500 },
    cap: 250,
  qty: [4, 150, 0, ...blank(45, 0)],
})), 5, [{ grant: 'forge', seconds: 120 }]);
forgeCase('grant: no oil keeps the ore furnace at its base speed', only('forge', furnace({
  order: ['Iron', 'Blank', 'IronBar', ...blank(45)],
  qty: [800, 0, 0, ...blank(45, 0)],
})), 5, [{ grant: 'forge', seconds: 90 }]);
forgeCase('grant: an unavailable furnace credits the time and warns once', only('forge', {
  ...NEUTRAL.forge,
}), 9, [{ grant: 'forge', seconds: 90 }, { grant: 'alchemy', seconds: 12.5 }]);
forgeCase('tick: the furnace produces on its own clock', only('forge', furnace({
  qty: [500, 80, 3, ...blank(45, 0)],
  progress: [14.5, ...Array(15).fill(0)],
})), 3, [{ ticks: 400 }]);
forgeCase('tick: a blank slot resets its progress', only('forge', furnace({
  order: blank(48),
  qty: blank(48, 0),
  progress: [12, ...Array(15).fill(0)],
})), 3, [{ ticks: 100 }]);


const petCases = [];
{
  const pets = JSON.parse(JSON.stringify(defaultConfig().pets));
  pets.breeding = numericRows(pets.breeding);
  pets.deployed = stringRows(pets.deployed);
  // A deployed shiny pet in world 0: `[key, shinyLevel]` with level 5.
  pets.deployed[0] = ['mushG', '5'];
  pets.breeding[22] = pets.breeding[22].map((value, index) => (index === 0 ? 1000 : value));
  const config = only('pets', { ...pets, shinySpeed: 86400 });
  const sim = makeSim(config, 13);
  const steps = [{ grant: 'pets', seconds: 30 }, { petBatch: true }, { grant: 'pets', seconds: 5 }, { petBatch: true }];
  runScript(sim, steps);
  petCases.push({ note: 'a deployed shiny pet banks the grant in one batch', config, seed: 13, steps, expected: simple(sim) });
}
{
  const config = only('pets', { available: false, deployed: [], breeding: [], shinySpeed: 1 });
  const sim = makeSim(config, 13);
  const steps = [{ grant: 'pets', seconds: 30 }, { petBatch: true }];
  runScript(sim, steps);
  petCases.push({ note: 'pet time is credited but an unavailable roster warns', config, seed: 13, steps, expected: simple(sim) });
}

const scurgeCases = [];
{
  const config = only('scurge', {
    unlocked: true,
    menagerie: JSON.parse(JSON.stringify(defaultConfig().scurge.menagerie)),
  });
  const sim = makeSim(config, 17);
  const steps = [{ scurgeKill: true }, { scurgeKill: true }, { scurgeKill: true }];
  runScript(sim, steps);
  scurgeCases.push({ note: 'three Scurge kills bank their EXP', config, seed: 17, steps, expected: simple(sim) });
}
{
  const config = only('scurge', { unlocked: false, menagerie: defaultConfig().scurge.menagerie });
  const sim = makeSim(config, 17);
  const steps = [{ scurgeKill: true }];
  runScript(sim, steps);
  scurgeCases.push({ note: 'a locked Scurge banks nothing', config, seed: 17, steps, expected: simple(sim) });
}

const coralCases = [];
{
  const config = only('coral', { unlocked: true, kills: 100 });
  const sim = makeSim(config, 19);
  const steps = [{ coralKill: 1 }, { coralKill: 2.5 }];
  runScript(sim, steps);
  coralCases.push({ note: 'coral kills bank the credit and update the option row', config, seed: 19, steps, expected: simple(sim), ola623: sim.state.ola[623] });
}
{
  const config = only('coral', { unlocked: false, kills: 100 });
  const sim = makeSim(config, 19);
  const steps = [{ coralKill: 1 }];
  runScript(sim, steps);
  coralCases.push({ note: 'a locked Coral banks nothing', config, seed: 19, steps, expected: simple(sim) });
}

const omegaCases = [];
{
  const config = only('omega', { unlocked: true, available: false, balls: 3, drops: 7 });
  const sim = makeSim(config, 23);
  const steps = [{ omegaKill: 'mushG' }];
  runScript(sim, steps);
  omegaCases.push({ note: 'an unavailable ball balance warns instead of assuming a fresh account', config, seed: 23, steps, expected: simple(sim) });
}
{
  const config = only('omega', { unlocked: true, available: true, balls: 3, drops: 7 });
  const sim = makeSim(config, 23);
  const steps = Array.from({ length: 40 }, () => ({ omegaKill: 'mushG' }));
  runScript(sim, steps);
  omegaCases.push({ note: 'Omega balls follow the drop chance', config, seed: 23, steps, expected: simple(sim), ola625: sim.state.ola[625], ola626: sim.state.ola[626] });
}

// The save reader's expected config: `combatProgressInputs` on the committed
// save, with the three raw reads the loader stashes but does not expose.
const raw = (key) => state.saveData?.[key] ?? save[key];
const account = progress.combatProgressInputs(state.saveData, 0, raw);

const fixture = {
  source:
    'js/stats/systems/common/combat-sim-progress.js through script/combat-progress-oracle.mjs',
  account: {
    forge: {
      available: account.forge.available,
      slots: account.forge.slots,
      cap: account.forge.cap,
      refill: account.forge.refill,
      gemSlots: account.forge.gemSlots,
      speed: account.forge.speed,
      bars: account.forge.bars,
      oilUse: account.forge.oilUse,
      storageAvailable: account.forge.storageAvailable,
      storage: account.forge.storage,
      order: account.forge.order,
      qty: account.forge.qty,
      progress: account.forge.progress,
    },
    pets: { available: account.pets.available, shinySpeed: account.pets.shinySpeed },
    scurge: { unlocked: account.scurge.unlocked, menagerie: account.scurge.menagerie },
    coral: account.coral,
    omega: account.omega,
    expected: {
      capTerms: {
        cosmo: 'cosmoBonus(S, 2, 1)',
        arcade: 'arcadeBonus(26, S)',
        stamp: "computeStampBonusOfTypeX('ForgeCap', S)",
        bribe: 'getBribeBonus(35, S)',
        vault: 'vaultUpgBonus(26, S)',
        achievement: 'achieveStatus(281, S)',
        rift: 'computeRiftSkillBonus(1, 1, S)',
        godshard: "computeCardLv('Godshard', S)",
      },
      speedTerms: {
        menagerie: 'menagerieBoostQTY(S, 11)',
      },
    },
  },
  forgeCases,
  petCases,
  scurgeCases,
  coralCases,
  omegaCases,
};
writeFileSync(new URL('fixtures/combat-progress.json', root), JSON.stringify(fixture, null, 1));
console.log(
  `wrote ${Object.keys(items).length} forge items, ${forgeCases.length} forge, ` +
    `${petCases.length} pet, ${scurgeCases.length} Scurge, ${coralCases.length} coral and ` +
    `${omegaCases.length} Omega cases`
);
