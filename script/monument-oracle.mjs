// Regenerates fixtures/monument.json from the upstream Monument calculator, so
// the Rust port is tested against the source of truth rather than hand-picked
// numbers.
// Usage: node script/monument-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/monument-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const { monumentSnapshot, monumentMilestones, braverySwordOdds, monumentRewardCharge } =
  await import(join(source, 'js/stats/systems/w5/monument.js'));
const { justiceCases } = await import(join(source, 'js/stats/data/w5/hole.js'));
const justice = await import(join(source, 'js/stats/systems/w5/monument-justice.js'));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save, serverVars: {} });
const saveData = state.saveData;

// The extracted data tables only cover the fields the section reads, so the
// snapshot note about Summoning access stays out of the oracle: the Rust port
// narrows the save the same way.
const snapshot = monumentSnapshot(saveData, 0);
if (!snapshot.available) throw new Error('the fixture has no Monument rows');
const bravery = snapshot.monuments[0];

/** The saved Bravery snapshot, in the shape the Rust `Snapshot` serialises to. */
const saved = {
  available: true,
  notes: snapshot.notes,
  training: {
    sources: snapshot.training.sources.map((source) => ({
      name: source.name,
      value: source.val,
    })),
    pct: snapshot.training.pct,
    multi: snapshot.training.multi,
    rate: snapshot.training.rate,
    perDay: snapshot.training.perDay,
  },
  stats: bravery.stats,
  milestones: monumentMilestones(saveData, 0, snapshot.training.rate),
  bonuses: bravery.bonuses,
};

/// One run state each, so boost and rethrow changes are pinned and not just the
/// saved cards.
const RUN_STATES = [
  { label: 'fresh story', wave: 0, runtime: {} },
  { label: 'boosted stacks', wave: 100, runtime: { boosts: 10 } },
  { label: 'no rethrows left', wave: 1000, runtime: { rerolls: 0 } },
  { label: 'boosted mid story', wave: 7, runtime: { boosts: 4, rerolls: 0 } },
  { label: 'late wave, many boosts', wave: 999, runtime: { boosts: 200, rerolls: 5 } },
  { label: 'last-sword HP entered', wave: 52, runtime: {}, remainingHp: 150000000 },
  {
    label: 'boosted with last-sword HP',
    wave: 50,
    runtime: { boosts: 3 },
    remainingHp: 100000,
  },
];

const odds = RUN_STATES.map(({ label, wave, runtime, remainingHp = null }) => {
  const result = braverySwordOdds(saveData, { wave, ...runtime, remainingHP: remainingHp });
  const { hours, milestones, opalChance, minDmg, maxDmg, rerolls, maxRerolls, boosts, swords,
    retellings, rightmost, autoKill } = result;
  return {
    label,
    wave,
    runtime,
    remainingHp,
    // The Rust `Odds` nests the run stats; the upstream result spreads them.
    result: {
      stats: { hours, milestones, opalChance, minDmg, maxDmg, rerolls, maxRerolls, boosts, swords,
        retellings, rightmost, autoKill },
      wave: result.wave,
      hp: result.hp,
      automatic: result.automatic,
      kill: result.kill,
      boostChance: result.boostChance,
      lastSwordChance: result.lastSwordChance,
      nextDamageFactor: result.nextDamageFactor,
    },
  };
});

/** The saved Wisdom snapshot, in the shape the Rust `WisdomSnapshot` serialises to. */
const wisdom = {
  available: true,
  notes: snapshot.notes,
  training: saved.training,
  stats: snapshot.monuments[2].stats,
  milestones: monumentMilestones(saveData, 2, snapshot.training.rate),
  bonuses: snapshot.monuments[2].bonuses,
  charge: snapshot.monuments[2].charge,
  tomorrow: snapshot.monuments[2].tomorrow,
};

/** The saved Justice snapshot, in the shape the Rust `JusticeSnapshot` serialises to. */
const justiceSnapshot = {
  available: true,
  notes: snapshot.notes,
  training: saved.training,
  stats: snapshot.monuments[1].stats,
  milestones: monumentMilestones(saveData, 1, snapshot.training.rate),
  bonuses: snapshot.monuments[1].bonuses,
};

/// The reward charge for every monument, plus a preview of a longer wait, so the
/// shared formula is pinned on all three branches.
const charges = snapshot.monuments.map((monument, index) => ({
  monument: index,
  charge: monument.charge,
  tomorrow: monument.tomorrow,
  extraWait: monumentRewardCharge(saveData, index, 0, 3600),
}));

/** Rejected reward-charge inputs and the upstream message each one prints. */
const chargeErrors = [
  { label: 'unknown monument', monument: 3, extraSeconds: 0 },
  { label: 'negative extra wait', monument: 0, extraSeconds: -1 },
].map((entry) => {
  try {
    monumentRewardCharge(saveData, entry.monument, 0, entry.extraSeconds);
  } catch (error) {
    return { ...entry, message: error.message };
  }
  throw new Error(`${entry.label}: the upstream calculator accepted the input`);
});

/** Rejected inputs and the upstream message each one prints. */
const errors = [
  { label: 'too many enemies', wave: 1001, runtime: {}, remainingHp: null },
  { label: 'rethrows over the maximum', wave: 0, runtime: { rerolls: 16 }, remainingHp: null },
  // Both invalid: the run stats are read before the HP, so the rethrow error wins.
  { label: 'too many rethrows and no HP', wave: 0, runtime: { rerolls: 16 }, remainingHp: 0 },
  { label: 'non-positive HP', wave: 0, runtime: {}, remainingHp: 0 },
].map((entry) => {
  try {
    braverySwordOdds(saveData, {
      wave: entry.wave,
      ...entry.runtime,
      remainingHP: entry.remainingHp,
    });
  } catch (error) {
    return { ...entry, message: error.message };
  }
  throw new Error(`${entry.label}: the upstream calculator accepted the input`);
});

/**
 * The Justice worksheet: the fresh-story start, every case's report from it, the
 * worked states with deals and promises active, and the effects the next case
 * settles.
 *
 * Two shapes are normalised to what the Rust types serialise: a blocked action
 * has no survival or value upstream (undefined drops out of JSON) and the port
 * reports zero, and an outcome's `caseIds` stay out of the fixture.
 */
function justiceFixture() {
  const initial = justice.justiceInitialState(saveData);
  /** A worksheet state derived from the fresh-story start. */
  const at = (patch = {}) => ({ ...structuredClone(initial), ...patch });
  /** The deals, promises and investments a long story accumulates. */
  const effects = {
    StudentLoanInvest: 120,
    ScripticusLoan: 45,
    PoppyLoan: 30,
    TimeBizInvestQ: 60,
    TimeBizInvestT: 4,
    CryptoInvest: 6,
    RetireInvest: 3,
    BeanLoan: 50,
    BeanDeal: 2,
    NoApprove: 2,
    ChestMag: 2,
    DrinksHad: 3,
    TimmyRejections: 2,
    TimmyIceCream: 1,
    RupieMagicz: 2,
    HaveMoreCoinsOrLoseHP: 500,
    popularityExample: 7,
    RejectTimmyDeal: 1,
    ScripticusApprovePromise: 1,
    ScareLegal: 1,
    HandLaw: 1,
    PermaPop: 1,
    lifeInsurance: 1,
    noDismissDeath: 1,
    BrokeDeath: 1,
  };
  const worked = at({
    round: 27,
    health: 9,
    coins: 640,
    popularity: 23,
    dismissals: 3,
    chests: 3,
    effects,
  });

  /** The report in the shape the Rust `Report` serialises to. */
  const report = (state, caseId) => {
    const result = justice.justiceResponses(state, caseId);
    return {
      row: result.row,
      ambiguous: result.ambiguous,
      responses: result.responses.map((response) => ({
        action: response.action,
        blocked: response.blocked ?? null,
        outcomes: response.outcomes.map((outcome) => ({
          label: outcome.label,
          probability: outcome.probability,
          state: outcome.state,
        })),
        survival: response.survival ?? 0,
        value: response.value ?? 0,
      })),
      recommended: result.recommended ?? null,
      heuristic: result.heuristic,
    };
  };

  // Every case from the fresh story covers each special case, gate and variant;
  // the rest pin the states the sheet reaches later in the story.
  const reports = [
    ...justiceCases().map((_, caseId) => ({
      label: `fresh story, case ${caseId}`,
      caseId,
      state: initial,
    })),
    { label: 'worked worksheet, a gift', caseId: 44, state: worked },
    { label: 'worked worksheet, a double gift', caseId: 70, state: worked },
    { label: 'worked worksheet, an ordinary case', caseId: 0, state: worked },
    { label: 'ended story', caseId: 0, state: at({ health: 0 }) },
    { label: 'no coins for the gate', caseId: 0, state: at({ coins: 0 }) },
    { label: 'no dismissals left', caseId: 0, state: at({ dismissals: 0 }) },
    {
      label: 'deltas scaled by the case number',
      caseId: 13,
      state: at({ round: 12, coins: 900 }),
    },
    {
      label: 'Harbinger coin growth',
      caseId: 14,
      state: at({ round: 13, coins: 900 }),
    },
    {
      label: 'last case, five dismissals',
      caseId: 87,
      state: at({ round: 49, dismissals: 5, popularity: 40 }),
    },
  ].map(({ label, caseId, state }) => ({
    label,
    caseId,
    state,
    report: report(state, caseId),
  }));

  const opened = [
    { label: 'fresh story opens the first case', caseId: 0, state: initial },
    { label: 'worked worksheet opens a case', caseId: 0, state: worked },
    {
      label: 'time business pays out',
      caseId: 0,
      state: at({ round: 1, effects: { TimeBizInvestQ: 60, TimeBizInvestT: 2 } }),
    },
    {
      label: 'time business one case left',
      caseId: 0,
      state: at({ round: 1, effects: { TimeBizInvestQ: 60, TimeBizInvestT: 1 } }),
    },
    {
      label: 'investments pay out',
      caseId: 0,
      state: at({ effects: { CryptoInvest: 1, RetireInvest: 1 } }),
    },
    {
      label: 'investment timers tick',
      caseId: 0,
      state: at({ effects: { CryptoInvest: 4, RetireInvest: 2 } }),
    },
    { label: 'approval ban ticks', caseId: 0, state: at({ effects: { NoApprove: 1 } }) },
    {
      label: 'scaring costs popularity',
      caseId: 0,
      state: at({ popularity: 3, effects: { ScareLegal: 1 } }),
    },
    { label: 'chest magnet ticks', caseId: 0, state: at({ effects: { ChestMag: 1 } }) },
    { label: 'drinks are forgotten', caseId: 0, state: at({ effects: { DrinksHad: 3 } }) },
    { label: 'Harbinger turn sets the debt', caseId: 14, state: at({ round: 4 }) },
    {
      label: 'Harbinger turn clears the debt',
      caseId: 15,
      state: at({ round: 4, effects: { BrokeDeath: 1 } }),
    },
    {
      label: 'Harbinger turn clears Timmy',
      caseId: 13,
      state: at({ round: 4, effects: { TimmyRejections: 2 } }),
    },
    {
      label: 'student loan and life insurance',
      caseId: 2,
      state: at({ effects: { StudentLoanInvest: 120, lifeInsurance: 1 } }),
    },
    { label: 'bean loan', caseId: 26, state: at({ effects: { BeanLoan: 50 } }) },
    {
      label: 'Scripticus loans',
      caseId: 24,
      state: at({ effects: { ScripticusLoan: 45, PoppyLoan: 30 } }),
    },
    {
      label: 'Hand law',
      caseId: 62,
      state: at({ popularity: 10, effects: { HandLaw: 1 } }),
    },
    {
      label: 'Chester promise kept',
      caseId: 33,
      state: at({ coins: 900, effects: { HaveMoreCoinsOrLoseHP: 500 } }),
    },
    {
      label: 'Chester promise broken',
      caseId: 33,
      state: at({ coins: 100, effects: { HaveMoreCoinsOrLoseHP: 500 } }),
    },
    {
      label: 'Poppy example returns',
      caseId: 74,
      state: at({ effects: { popularityExample: 7 } }),
    },
  ].map(({ label, caseId, state }) => ({
    label,
    caseId,
    state,
    opened: justice.justiceOpenCase(state, caseId),
  }));

  const errors = [
    { label: 'unknown case', kind: 'responses', state: initial, caseId: 999 },
    { label: 'negative health', kind: 'responses', state: at({ health: -1 }), caseId: 0 },
    { label: 'negative coins', kind: 'responses', state: at({ coins: -1 }), caseId: 0 },
    { label: 'negative dismissals', kind: 'responses', state: at({ dismissals: -2 }), caseId: 0 },
    { label: 'ended story cannot open', kind: 'open', state: at({ health: 0 }), caseId: 0 },
    { label: 'unknown case cannot open', kind: 'open', state: initial, caseId: 500 },
  ].map((entry) => {
    try {
      if (entry.kind === 'open') justice.justiceOpenCase(entry.state, entry.caseId);
      else justice.justiceResponses(entry.state, entry.caseId);
    } catch (error) {
      return { ...entry, message: error.message };
    }
    throw new Error(`${entry.label}: the upstream advisor accepted the input`);
  });

  return {
    snapshot: justiceSnapshot,
    initial,
    reports,
    opened,
    delayed: [initial, worked].map((state) =>
      justice.justiceDelayedEffects(state).map(({ key, name, help, value }) => ({
        key,
        name,
        help,
        value,
      }))
    ),
    errors,
  };
}

writeFileSync(
  join(root, 'fixtures/monument.json'),
  `${JSON.stringify(
    {
      source: 'fixtures/data.json',
      calculator: 'js/stats/systems/w5/monument.js',
      character: 0,
      snapshot: saved,
      wisdom,
      charges,
      chargeErrors,
      justice: justiceFixture(),
      odds,
      errors,
    },
    null,
    2
  )}\n`
);
console.log(
  `wrote the Bravery, Justice and Wisdom snapshots, ${charges.length} reward charges, ${odds.length} run states and ${errors.length} rejected inputs to fixtures/monument.json`
);
