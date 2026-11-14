// Writes the upstream Spelunking figures for the committed synthetic save, so
// `crates/spelunking/tests/spelunking.rs` and `tests/planner.rs` compare the
// Rust port against the JS.
//
// Usage: node script/spelunking-oracle.mjs <path-to-idleon-research-optimizer>
//
// The four account multipliers upstream derives from its whole stat context
// (`Spelunking Power`, the amber multiplier, stamina regeneration and the Grand
// Discovery chance) are not ported yet; they are explicit inputs here, exactly
// as the app's account-input panel supplies them. The save-derived half — party,
// caves, tools, elixirs, shop upgrades and records — is built by the upstream
// `buildSpelunkInputs`, so the decode and the depth maths both come from the
// source of truth. The optimizer section drives `spelunk-planner.js` through its
// generator with the same simulation, so both searches are pinned to the
// upstream candidate order and fresh-seed arithmetic.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/spelunking-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const inputs = await import(
  join(source, 'js/stats/systems/w7/spelunk-inputs.js')
);
const expedition = await import(
  join(source, 'js/stats/systems/w7/spelunk-expedition.js')
);
const depth = await import(join(source, 'js/stats/systems/w7/spelunk-depth.js'));
const planner = await import(
  join(source, 'js/stats/systems/w7/spelunk-planner.js')
);

const save = JSON.parse(
  readFileSync(new URL('../fixtures/data.json', import.meta.url), 'utf8'),
);
// The committed fixture is a bare save with no `it.json` companion roster, so
// the companion terms read as unavailable on both sides.
loader.loadSaveData({ data: save });
const built = inputs.buildSpelunkInputs(state.saveData, 0);

// The account inputs the port asks the user for instead of deriving them.
const ACCOUNT = {
  power: 2500,
  holesBonus: 0.5,
  amber: built.amber,
  regen: built.regen,
  grand: built.grand,
};
const controlled = {
  ...built,
  pow: ACCOUNT.power,
  holesBonus: ACCOUNT.holesBonus,
  amber: ACCOUNT.amber,
  regen: ACCOUNT.regen,
  grand: ACCOUNT.grand,
};

/** One configuration the Rust port must reproduce, with the fields it reads. */
const CONFIGS = [
  {
    label: 'saved defaults',
    requested: {},
    range: { from: 1, to: 40, tool: 'best' },
  },
  {
    label: 'explicit party and belt',
    requested: {
      cave: 1,
      party: [0, 1],
      elixirs: [0, 2, 7],
      memorized: [0, 2],
      policy: 'dive',
      tool: 2,
      stamina: 'saved',
      fullClear: false,
    },
    range: { from: 1, to: 25, tool: 2 },
  },
  {
    label: 'full stamina, manic, refund shield',
    requested: {
      cave: 3,
      party: [2, 3, 4, 5],
      manic: true,
      elixirs: [0, 2, 2, 2, 2],
      memorized: [2, 9],
      policy: 'efficient',
      tool: 'adaptive',
      stamina: 'full',
      fullClear: true,
    },
    range: { from: 15, to: 35, tool: 'best' },
  },
  {
    label: 'boss floor',
    requested: {
      cave: 0,
      party: [0],
      elixirs: [-1, -1, -1, -1, -1],
      memorized: [],
      policy: 'clear',
      entry: 'surface',
      stamina: 'saved',
    },
    range: { from: 5, to: 7, tool: 0 },
  },
  {
    label: 'phased amber policy',
    requested: {
      cave: 1,
      party: [0, 1],
      elixirs: [3, 3, 3, 6, 6],
      memorized: [3, 6],
      policy: 'amber-farm',
      stamina: 'full',
      amber: {
        dumpDepth: 12,
        finishers: 2,
        reservePct: 80,
        harvestDepths: 4,
        ropeReservePct: 40,
      },
    },
    range: { from: 1, to: 20, tool: 'best' },
  },
];

/** The normalized config plus the whole profile, as the port serialises them. */
function profile(entry) {
  const config = expedition.normalizeSpelunkConfig(controlled, entry.requested);
  const result = depth.spelunkDepthProfile(controlled, config, entry.range);
  return {
    label: entry.label,
    requested: entry.requested,
    range: entry.range,
    config,
    cave: result.cave,
    budget: result.budget,
    power: result.power,
    refund: result.refund,
    holes: result.holes,
    rows: result.rows,
  };
}

/** The upstream rejection message for a requested party/config. */
function rejected(input, requested) {
  try {
    expedition.normalizeSpelunkConfig(input, requested);
    return null;
  } catch (error) {
    return error.message;
  }
}

// A cave whose boss is still unbeaten and whose nova, potion and charge
// upgrades are bought, so the run exercises the boss, nova, potion, Grand
// Discovery and radar branches the default save has already finished.
const UNBEATEN = {
  ...controlled,
  bosses: controlled.bosses.map(() => 0),
  grands: controlled.grands.map(() => 0),
  discoveries: [],
  shop: controlled.shop.map((value, index) => ([36, 39, 40].includes(index) ? 1 : value)),
};
const RUN_INPUTS = { controlled, unbeaten: UNBEATEN, weak: { ...controlled, pow: 25 } };

/** One whole simulated expedition, compacted for the fixture. */
function run(entry) {
  const input = RUN_INPUTS[entry.input ?? 'controlled'];
  const config = expedition.normalizeSpelunkConfig(input, entry.requested);
  const result = expedition.simulateSpelunkExpedition(input, config, {
    seed: entry.seed,
  });
  return {
    label: entry.label,
    requested: entry.requested,
    input: entry.input ?? 'controlled',
    seed: entry.seed,
    config: result.config,
    status: result.status,
    settled: result.settled,
    seconds: result.seconds,
    startDepth: result.startDepth,
    depth: result.depth,
    actions: result.actions,
    spent: result.spent,
    hits: result.hits,
    misses: result.misses,
    broken: result.broken,
    rewards: result.rewards,
    grossRewards: result.grossRewards,
    exit: result.exit,
    stamina: result.stamina,
    diagnostics: result.diagnostics,
    amber: result.amber ?? null,
    grands: result.grands,
    discoveries: result.discoveries,
    rngCalls: result.rngCalls,
    eventsDropped: result.eventsDropped,
    // The trace is compared as `[type, depth]` pairs: the times and stamina
    // reads are already pinned by the run's own figures, and the full list
    // would make the fixture megabytes wide.
    events: result.events.map((event) => [event.type, event.depth]),
    notes: result.notes,
  };
}

/** The configurations whose whole run the port must reproduce. */
const RUNS = [
  { label: 'saved defaults, seed 1', requested: {}, seed: 1 },
  {
    label: 'manual dive with a belt, seed 7',
    requested: {
      cave: 1,
      party: [0, 1],
      elixirs: [0, 2, 7],
      memorized: [0, 2],
      policy: 'dive',
      tool: 2,
      stamina: 'full',
      fullClear: false,
    },
    seed: 7,
  },
  {
    label: 'first boss with nova, radar and discovery, seed 11',
    input: 'unbeaten',
    requested: {
      cave: 0,
      party: [0, 1],
      elixirs: [1, 4, 8, 9, 10],
      memorized: [1, 9],
      policy: 'efficient',
      stamina: 'saved',
      fullClear: false,
    },
    seed: 11,
  },
  {
    label: 'surface full clear, seed 3',
    requested: {
      cave: 0,
      party: [0],
      manic: false,
      elixirs: [-1, -1, -1, -1, -1],
      memorized: [],
      policy: 'clear',
      entry: 'surface',
      stamina: 'saved',
    },
    seed: 3,
  },
  {
    label: 'phased amber farming, seed 5',
    requested: {
      cave: 1,
      party: [0, 1],
      elixirs: [3, 3, 3, 6, 6],
      memorized: [3, 6],
      policy: 'amber-farm',
      stamina: 'full',
      amber: {
        dumpDepth: 20,
        finishers: 1,
        reservePct: 100,
        harvestDepths: 3,
        ropeReservePct: 25,
      },
    },
    seed: 5,
  },
  {
    label: 'phased amber farming, early rope, seed 2',
    requested: {
      cave: 1,
      party: [0, 1],
      elixirs: [3, 3, 3, 6, 6],
      memorized: [3, 6],
      policy: 'amber-farm',
      stamina: 'full',
      amber: {
        dumpDepth: 3,
        finishers: 2,
        reservePct: 90,
        harvestDepths: 2,
        ropeReservePct: 60,
      },
    },
    seed: 2,
  },
  {
    label: 'phased amber farming, stamina burn, seed 3',
    input: 'weak',
    requested: {
      cave: 1,
      party: [0, 1],
      elixirs: [3, 3, 3, 6, 6],
      memorized: [3, 6],
      policy: 'amber-farm',
      stamina: 'full',
      amber: {
        dumpDepth: 2,
        finishers: 1,
        reservePct: 100,
        harvestDepths: 3,
        ropeReservePct: 25,
      },
    },
    seed: 3,
  },
];

// The fixture's characters are all at full stamina and Manic is unlocked, so
// the two run-state rejections need a copy of the input with that state set.
const tired = {
  ...controlled,
  characters: controlled.characters.map((character, id) =>
    id === 0 ? { ...character, stamina: Math.floor(character.max / 4) } : character,
  ),
};
const locked = { ...controlled, manic: false };
const strictDupes = { ...controlled, dupes: 1 };
const fewTools = { ...controlled, tools: controlled.tools.slice(0, 1) };
const manualOnly = { ...controlled, auto: false };

const parties = [
  { label: 'empty party', input: controlled, requested: { party: [] } },
  { label: 'duplicate member', input: controlled, requested: { party: [0, 0] } },
  {
    label: 'over capacity',
    input: controlled,
    requested: { cave: 0, party: [0, 1, 2, 3] },
  },
  { label: 'missing member', input: controlled, requested: { party: [0, 99] } },
  {
    label: 'tired member',
    input: tired,
    requested: { party: [0], stamina: 'saved' },
  },
  {
    label: 'tired member with full stamina is allowed',
    input: tired,
    requested: { party: [0], stamina: 'full' },
  },
  {
    label: 'missing member with full stamina',
    input: controlled,
    requested: { party: [0, 99], stamina: 'full' },
  },
  {
    label: 'too many belt elixirs',
    input: controlled,
    requested: { elixirs: [0, 1, 2, 3, 4, 5, 6] },
  },
  {
    label: 'too many duplicates',
    input: controlled,
    requested: { elixirs: [0, 0, 0, 0, 0, 0] },
  },
  {
    label: 'duplicated memory',
    input: controlled,
    requested: { memorized: [0, 0] },
  },
  {
    label: 'locked manic',
    input: locked,
    requested: { manic: true, cave: 0, party: [0] },
  },
  {
    label: 'duplicate limit',
    input: strictDupes,
    requested: { elixirs: [0, 0, -1, -1, -1] },
  },
  {
    label: 'too many memorized',
    input: controlled,
    requested: { memorized: [0, 1, 2] },
  },
  {
    label: 'locked policy',
    input: manualOnly,
    requested: { policy: 'auto', party: [0] },
  },
  {
    label: 'locked tool',
    input: fewTools,
    requested: { tool: 2, party: [0] },
  },
  {
    label: 'auto speed out of range',
    input: controlled,
    requested: { speed: 9, party: [0] },
  },
  // Unknown entry and stamina strings are rejected by the typed request
  // decoder before `normalize`, so they have no oracle message here.
  {
    label: 'fast reaction',
    input: controlled,
    requested: { reactionMs: 50, party: [0] },
  },
  {
    label: 'locked elixir',
    input: controlled,
    requested: { elixirs: [99, -1, -1, -1, -1], party: [0] },
  },
  {
    label: 'locked memorized elixir',
    input: controlled,
    requested: { memorized: [99], party: [0] },
  },
  {
    label: 'amber dump depth out of range',
    input: controlled,
    requested: { cave: 0, party: [0], policy: 'amber-farm', amber: { dumpDepth: 0 } },
  },
  {
    label: 'amber finishers out of range',
    input: controlled,
    requested: { cave: 0, party: [0], policy: 'amber-farm', amber: { finishers: 3 } },
  },
  {
    label: 'amber reserve out of range',
    input: controlled,
    requested: { cave: 0, party: [0], policy: 'amber-farm', amber: { reservePct: 10 } },
  },
  {
    label: 'amber harvest depths out of range',
    input: controlled,
    requested: { cave: 0, party: [0], policy: 'amber-farm', amber: { harvestDepths: 101 } },
  },
  {
    label: 'amber rope reserve out of range',
    input: controlled,
    requested: { cave: 0, party: [0], policy: 'amber-farm', amber: { ropeReservePct: 0 } },
  },
];



/** The optimizer baseline and search scope the Rust parity test replays. */
const OPTIMIZER_BASELINE = {
  cave: 0,
  party: [0, 1],
  policy: 'auto',
  speed: 4,
  elixirs: controlled.savedElixirs,
  memorized: [7],
  fullClear: false,
  entry: 'shortcut',
  tool: 'adaptive',
  reactionMs: 350,
  stamina: 'saved',
};
const OPTIMIZER_SCOPE = {
  enabled: { party: false, elixirs: false, memorized: false, entry: false },
  modes: ['auto', 'manual'],
  clears: [false, true, 'fifth'],
  speeds: [0, 1, 2, 3, 4],
};
const OPTIMIZER_OPTIONS = {
  objective: 'depth',
  screening: 1,
  trials: 2,
  scope: OPTIMIZER_SCOPE,
  seed: 1,
};
const AMBER_OPTIONS = {
  objective: 'amber',
  screening: 1,
  trials: 2,
  maxTrials: 2,
  minRopePct: 0,
  from: 6,
  to: 10,
  seed: 1,
};

/** Drive one planner generator to completion with the upstream run. */
function drive(iterator) {
  let next = iterator.next();
  while (!next.done) {
    const request = next.value;
    const measure = (entry) =>
      expedition.simulateSpelunkExpedition(controlled, entry.config, {
        seed: entry.seed,
      });
    const result = request.requests
      ? request.requests.map(measure)
      : measure(request);
    next = iterator.next(result);
  }
  return next.value;
}

/** One compact optimizer row: the means and the paired final runs. */
function optimizerRow(row) {
  return {
    label: row.label,
    referenceOnly: row.referenceOnly ?? false,
    riskEligible: row.riskEligible ?? true,
    matchingStrategies: row.matchingStrategies ?? 0,
    score: row.score,
    se: row.se,
    depth: row.depth,
    startDepth: row.startDepth,
    amber: row.amber,
    seconds: row.seconds,
    settled: row.settled,
    total: row.total,
    rope: row.rope,
    toll: row.toll,
    blocked: row.blocked,
    recoveryHours: row.recoveryHours,
    config: row.config,
    runs: row.runs.map((run) => ({
      seed: run.seed,
      status: run.status,
      settled: run.settled,
      depth: run.depth,
      startDepth: run.startDepth,
      seconds: run.seconds,
      spent: run.spent,
      rewards: run.rewards,
      grands: run.grands,
      stamina: run.stamina,
      exit: run.exit,
      amber: run.amber ?? null,
    })),
    partyAnalysis: row.partyAnalysis
      ? {
          objective: row.partyAnalysis.objective,
          retention: row.partyAnalysis.retention,
          recommendedSize: row.partyAnalysis.recommendedSize,
          trials: row.partyAnalysis.trials,
          rows: row.partyAnalysis.rows.map((party) => ({
            config: party.config,
            score: party.score,
            depth: party.depth,
            amber: party.amber,
            settled: party.settled,
            allowed: party.allowed,
            marginal: party.marginal,
          })),
        }
      : null,
  };
}

const searchSpace = planner.spelunkSearchSpace(
  controlled,
  OPTIMIZER_BASELINE,
  OPTIMIZER_SCOPE,
);
const search = drive(
  planner.createSpelunkSearch(controlled, OPTIMIZER_BASELINE, OPTIMIZER_OPTIONS),
);
const amber = drive(
  planner.createSpelunkAmberSearch(controlled, OPTIMIZER_BASELINE, AMBER_OPTIONS),
);

const payload = {
  account: ACCOUNT,
  characters: built.characters,
  shop: built.shop,
  caves: built.caves.map((cave) => ({
    id: cave.id,
    name: cave.name,
    capacity: cave.capacity,
    bossDepth: cave.bossDepth,
  })),
  tools: built.tools,
  elixirs: built.elixirs,
  elixirOwned: built.elixirOwned,
  slots: built.slots,
  dupes: built.dupes,
  memorySlots: built.memorySlots,
  memorized: built.memorized,
  savedElixirs: built.savedElixirs,
  savedManic: built.savedManic,
  fullClear: built.fullClear,
  auto: built.auto,
  speedMax: built.speedMax,
  manic: built.manic,
  partial: built.partial,
  depths: built.depths,
  manicDepths: built.manicDepths,
  bosses: built.bosses,
  grands: built.grands,
  discoveries: built.discoveries,
  nonconsume: built.nonconsume,
  overuse: built.overuse,
  switchNova: built.switchNova,
  bags: built.bags,
  prisma: built.prisma,
  exalt: built.exalt,
  shortcut: built.shortcut === 1,
  optimizerSearchSpace: {
    configurations: String(searchSpace.configurations),
    scope: searchSpace.scope,
    baseline: OPTIMIZER_BASELINE,
    options: OPTIMIZER_OPTIONS,
  },
  optimizerSearch: {
    kind: search.kind,
    objective: search.objective,
    screened: String(search.screened),
    configurations: String(search.configurations),
    exhaustive: search.exhaustive,
    delta: search.delta,
    interval: search.interval,
    winner: search.winner ? optimizerRow(search.winner) : null,
    baseline: optimizerRow(search.baseline),
    rows: search.rows.map(optimizerRow),
  },
  optimizerAmber: {
    kind: amber.kind,
    objective: amber.objective,
    screened: String(amber.screened),
    exhaustive: amber.exhaustive,
    delta: amber.delta,
    interval: amber.interval,
    winner: amber.winner ? optimizerRow(amber.winner) : null,
    baseline: optimizerRow(amber.baseline),
    rows: amber.rows.map(optimizerRow),
    info: amber.amberSearch,
  },
  configs: CONFIGS.map(profile),
  runs: RUNS.map(run),
  rejections: parties.map((entry) => ({
    label: entry.label,
    requested: entry.requested,
    input: entry.input === controlled ? 'controlled' : entry.label,
    error: rejected(entry.input, entry.requested),
  })),
};

const out = new URL('../fixtures/spelunking.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${payload.configs.length} profiles, ${payload.rejections.length} rejections, ` +
    `${payload.runs.length} runs, ${payload.characters.length} characters, ` +
    `${payload.optimizerSearch.rows.length} search rows and ${payload.optimizerAmber.rows.length} amber rows`,
);
