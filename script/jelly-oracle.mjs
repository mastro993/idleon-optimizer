// Writes the upstream Jelly Operator figures for the committed synthetic save,
// so `crates/jelly/tests/jelly.rs` compares the Rust port against the JS.
//
// Usage: node script/jelly-oracle.mjs <path-to-idleon-research-optimizer>
//
// Every case runs `js/stats/systems/w7/jelly-operator.js` itself: the layout,
// its metrics, the seeded Monte Carlo estimate, the simulator's single seeded
// operation, the progression insights and the board section plan. A case may
// override the save's Fever, upgrade levels, cell levels and layout; the
// overrides are written to the fixture and replayed by the Rust test.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/jelly-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const J = await import(join(source, 'js/stats/systems/w7/jelly-operator.js'));

const save = JSON.parse(
  readFileSync(new URL('../fixtures/data.json', import.meta.url), 'utf8'),
);
loader.loadSaveData({ data: save });
const base = state.saveData;

/// Apply one case's overrides to a copy of the save.
function patched(overrides) {
  const S = structuredClone(base);
  if (overrides.fever != null) S.research[7][13] = overrides.fever;
  if (overrides.obstruction != null) S.research[7][9] = overrides.obstruction;
  if (overrides.upgrades) {
    const upgrades = Array.isArray(S.research[17]) ? S.research[17].slice() : [];
    for (const [id, level] of Object.entries(overrides.upgrades)) upgrades[Number(id)] = level;
    S.research[17] = upgrades;
  }
  if (overrides.plots) S.research[18] = overrides.plots.slice();
  if (overrides.cellLevels) S.research[16] = overrides.cellLevels;
  if (overrides.cellExp) S.research[15] = overrides.cellExp;
  if (overrides.layout) {
    S.research[14] = Array.from({ length: J.JELLY_SIZE }, () => -1);
    for (const [anchor, type] of Object.entries(overrides.layout)) {
      S.research[14][Number(anchor)] = type;
    }
  }
  return S;
}

/// The state the Rust decoder has to reproduce before any maths runs.
function stateOf(S) {
  const progress = J.jellyProgress(S);
  return {
    progress,
    unitsOwned: J.jellyUnitsOwned(S),
    unlocked: Array.from(J.jellyUnlockedSlots(S)).sort((a, b) => a - b),
    slotPurchasesLeft: J.jellySlotPurchasesLeft(S),
    cellLevels: Array.from({ length: 9 }, (_, type) => J.jellyCellLevel(S, type)),
    cellExp: Array.from({ length: 9 }, (_, type) => J.jellyCellExp(S, type)),
    dailyTries: J.jellyDailyTries(S),
    currencyMultiplier: J.jellyCurrencyMultiplier(S, progress.fever),
    cellDamageMultiplier: J.jellyCellDamageMultiplier(S, {
      fever: progress.fever,
      totalCellLevel: J.jellyTotalCellLevel(S),
    }),
    cellSpeedMultiplier: J.jellyCellSpeedMultiplier(S, progress.fever),
    cellExpMultiplier: J.jellyCellExpMultiplier(S, progress.fever),
  };
}

function metricsOf(S, layout) {
  const metrics = J.jellyLayoutMetrics(layout, S, { fever: J.jellyProgress(S).fever });
  if (!metrics.valid) return { valid: false, reason: metrics.reason };
  return {
    valid: true,
    counts: Array.from(metrics.counts),
    rawCounts: Array.from(metrics.rawCounts),
    infectedSlots: metrics.infectedSlots,
    damagePassive: metrics.damagePassive,
    speedPassive: metrics.speedPassive,
    virusMultiplier: metrics.virusMultiplier,
    dps: metrics.dps,
    attacksPerSecond: metrics.attacksPerSecond,
    amoebaAttacksPerSecond: metrics.amoebaAttacksPerSecond,
    cells: metrics.cells.map((cell) => ({
      anchor: cell.anchor,
      type: cell.type,
      slots: cell.slots,
      organelle: cell.organelle,
      proximity: cell.proximity,
      damage: cell.damage,
      cooldownProgress: cell.cooldownProgress,
      progressPerFrame: cell.progressPerFrame,
      framesPerAttack: cell.framesPerAttack,
      attacksPerSecond: cell.attacksPerSecond,
      dps: cell.dps,
    })),
  };
}

/// The trial fields the Rust port compares, shared by the estimate's
/// representative and the simulator's single seeded operation.
function trialOf(trial) {
  return {
    success: trial.success,
    endedBy: trial.endedBy,
    time: trial.time,
    normalTime: trial.normalTime,
    criticalTime: trial.criticalTime,
    damage: trial.damage,
    shots: trial.shots,
    hits: trial.hits,
    bloodcells: trial.bloodcells,
    cellExp: trial.cellExp,
    expByType: trial.expByType,
    feverSeconds: trial.feverSeconds,
    bestDpsBefore: trial.bestDpsBefore,
    bestDpsAfter: trial.bestDpsAfter,
    peakDps: trial.peakDps,
    deaths: trial.deaths,
    bossAttacks: trial.bossAttacks,
    immunoidHits: trial.immunoidHits,
    immunoidFocusTime: trial.immunoidFocusTime,
    liveSlots: trial.liveSlots,
    revivesUsed: trial.revivesUsed,
    revivesRemaining: trial.revivesRemaining,
    roidUsed: trial.roidUsed,
    roidStart: trial.roidStart,
    amoebaStacks: trial.amoebaStacks,
    amoebaStacksAtCriticalStart: trial.amoebaStacksAtCriticalStart,
    damageAtCriticalStart: trial.damageAtCriticalStart,
    attemptDelta: trial.attemptDelta,
    obstructionAfter: trial.obstructionAfter,
    gameplayRandomCalls: trial.gameplayRandomCalls,
    deathLog: trial.deathLog.slice(0, 8).map((row) => ({
      time: row.time,
      slot: row.slot,
      focused: row.focused,
      disabledAnchor: row.disabledAnchor,
      revived: row.revived,
    })),
    contribution: trial.contribution.map((row) => ({
      type: row.type,
      shots: row.shots,
      hits: row.hits,
      damage: row.damage,
      bloodcells: row.bloodcells,
      exp: row.exp,
    })),
  };
}

function estimateOf(S, layout, options) {
  const estimate = J.simulateJellyTrials(layout, S, {
    trials: options.trials,
    seed: options.seed,
    firingCalendar: false,
  });
  if (!estimate.valid) return { valid: false, reason: estimate.reason };
  const representative = estimate.representative;
  return {
    valid: true,
    trials: estimate.trials,
    seed: estimate.seed,
    successCount: estimate.successCount,
    clearProbability: estimate.clearProbability,
    clearProbabilityLow: estimate.clearProbabilityLow,
    clearProbabilityHigh: estimate.clearProbabilityHigh,
    meanTime: estimate.meanTime,
    medianTime: estimate.medianTime,
    p90Time: estimate.p90Time,
    meanDamage: estimate.meanDamage,
    meanDamageStandardError: estimate.meanDamageStandardError,
    medianDamage: estimate.medianDamage,
    meanBloodcells: estimate.meanBloodcells,
    medianBloodcells: estimate.medianBloodcells,
    p90Bloodcells: estimate.p90Bloodcells,
    maxBloodcells: estimate.maxBloodcells,
    meanCellExp: estimate.meanCellExp,
    meanCellExpByType: estimate.meanCellExpByType,
    meanCriticalTime: estimate.meanCriticalTime,
    meanDeaths: estimate.meanDeaths,
    meanRevives: estimate.meanRevives,
    meanBestDps: estimate.meanBestDps,
    meanBestDpsMultiplier: estimate.meanBestDpsMultiplier,
    meanPeakDps: estimate.meanPeakDps,
    meanNormalDamage: estimate.meanNormalDamage,
    meanCriticalDamage: estimate.meanCriticalDamage,
    meanShots: estimate.meanShots,
    meanHits: estimate.meanHits,
    meanBossAttacks: estimate.meanBossAttacks,
    meanImmunoidFocusTime: estimate.meanImmunoidFocusTime,
    meanLiveSlots: estimate.meanLiveSlots,
    meanAttemptDelta: estimate.meanAttemptDelta,
    meanAmoebaStacksAtCriticalStart: estimate.meanAmoebaStacksAtCriticalStart,
    meanAmoebaStacksAtCriticalEnd: estimate.meanAmoebaStacksAtCriticalEnd,
    minDamage: estimate.minDamage,
    maxDamage: estimate.maxDamage,
    contribution: estimate.meanContributionByType.map((row) => ({
      type: row.type,
      shots: row.shots,
      hits: row.hits,
      damage: row.damage,
      bloodcells: row.bloodcells,
      exp: row.exp,
    })),
    representative: trialOf(representative),
  };
}

/// One operation exactly as the simulator runs it: the sandbox's stage and
/// Fever come from the save, the knobs from the case.
function simulationOf(S, layout, options) {
  const trial = J.simulateJellyOperation(layout, S, {
    ...options,
    obstruction: J.jellyProgress(S).obstruction,
    fever: J.jellyProgress(S).fever,
  });
  return { valid: trial.valid, reason: trial.reason || '', trial: trialOf(trial) };
}

/// The operation summary a section plan row keeps: `stripTrialResults` without
/// the per-trial records the planner only uses while ranking.
function operationSummaryOf(operation) {
  if (!operation) return null;
  return {
    valid: operation.valid,
    reason: operation.reason || '',
    trials: operation.trials,
    seed: operation.seed,
    successCount: operation.successCount,
    clearProbability: operation.clearProbability,
    clearProbabilityLow: operation.clearProbabilityLow,
    clearProbabilityHigh: operation.clearProbabilityHigh,
    meanTime: operation.meanTime,
    meanDamage: operation.meanDamage,
    meanDamageStandardError: operation.meanDamageStandardError,
    meanBloodcells: operation.meanBloodcells,
    meanCellExp: operation.meanCellExp,
    meanCellExpByType: operation.meanCellExpByType,
    meanCriticalTime: operation.meanCriticalTime,
    meanBossAttacks: operation.meanBossAttacks,
  };
}

function sectionPlanOf(S, options) {
  const plan = J.planJellySections(S, {
    ...options,
    layout: J.jellyLayoutFromSave(S),
  });
  const rowOf = (row) => row == null ? null : {
    plotId: row.plotId ?? null,
    start: row.start ?? null,
    width: row.width ?? null,
    height: row.height ?? null,
    plotIds: row.plotIds,
    newUsableSlots: row.newUsableSlots,
    newSlots: row.newSlots,
    proximitySlots: row.proximitySlots,
    newlyLegalFootprintPlacements: row.newlyLegalFootprintPlacements,
    // A single section's completed board is compared by key; the simulated
    // sequences carry the whole map, so the test can also validate it.
    layout: row.plotId == null ? row.layout : undefined,
    layoutKey: row.layout ? J.jellyLayoutKey(row.layout) : '',
    proxyScore: row.proxyScore,
    proxyGain: row.proxyGain,
    operation: operationSummaryOf(row.operation),
    objectiveValue: row.objectiveValue ?? null,
    objectiveGain: row.objectiveGain ?? null,
    gainPerSlot: row.gainPerSlot ?? null,
    confidence: row.confidence,
    chanceBeatsBaseline: row.chanceBeatsBaseline,
    simulationTrials: row.simulationTrials ?? null,
    finalist: row.finalist ?? null,
  };
  return {
    objective: plan.objective,
    expCellType: plan.expCellType,
    quality: plan.quality,
    qualityProfile: plan.qualityProfile,
    strategy: plan.strategy,
    tokens: plan.tokens,
    requestedTokens: plan.requestedTokens,
    availableTokens: plan.availableTokens,
    baselineLayout: plan.baselineLayout,
    baselineUnlockedSlots: plan.baselineUnlockedSlots,
    baselineObstruction: plan.baselineObstruction,
    baselineOperation: operationSummaryOf(plan.baselineOperation),
    plots: plan.plots.map(rowOf),
    sequences: plan.sequences.map(rowOf),
    recommendedSequence: rowOf(plan.recommendedSequence),
    recommendedPlotIds: plan.recommendedPlotIds,
    finalLayout: plan.finalLayout,
    finalOperation: operationSummaryOf(plan.finalOperation),
    note: plan.note,
  };
}

function insightsOf(S, layout, operation) {
  const insights = J.jellyOperationInsights(operation, S, { layout });
  return {
    valid: insights.valid,
    dailyAttempts: insights.dailyAttempts,
    expectedAttemptsToClear: insights.expectedAttemptsToClear,
    probabilityWithinDailyAttempts: insights.probabilityWithinDailyAttempts,
    expectedDaysToClear: insights.expectedDaysToClear,
    expectedBloodcellsPerAttempt: insights.expectedBloodcellsPerAttempt,
    savedDailyPassiveBloodcells: insights.savedDailyPassiveBloodcells,
    expectedBloodcellsPerDailyCycle: insights.expectedBloodcellsPerDailyCycle,
    expectedNetAttemptDelta: insights.expectedNetAttemptDelta,
    normalDamage: insights.normalDamage,
    criticalDamage: insights.criticalDamage,
    normalDamageShare: insights.normalDamageShare,
    criticalDamageShare: insights.criticalDamageShare,
    expRows: insights.expRows.map((row) => ({
      type: row.type,
      currentLevel: row.currentLevel,
      currentExp: row.currentExp,
      requirement: row.requirement,
      remainingExp: row.remainingExp,
      meanExpPerAttempt: row.meanExpPerAttempt,
      attemptsToNextLevel: row.attemptsToNextLevel,
      daysToNextLevel: row.daysToNextLevel,
    })),
    obstructionOutlook: insights.obstructionOutlook.map((row) => ({
      obstruction: row.obstruction,
      hp: row.hp,
      time: row.time,
      estimatedClearProbability: row.estimatedClearProbability,
      damageRatio: row.damageRatio,
      expectedAttempts: row.expectedAttempts,
      clearWithinDailyAttempts: row.clearWithinDailyAttempts,
    })),
  };
}

/// Eight unpurchased plots, so a multi-token plan screens a real beam without
/// the committed save's twenty-five sections making the case slow.
const SECTION_PLOTS = {
  plots: Array.from({ length: 40 }, (_, plot) => plot).filter(
    (plot) => ![11, 39, 1, 29, 36, 6, 21, 12].includes(plot),
  ),
};

const cases = [
  {
    name: 'save',
    overrides: {},
    estimate: { trials: 8, seed: 7 },
  },
  {
    name: 'small-board',
    overrides: {
      fever: 1,
      obstruction: 32,
      layout: { 0: 6, 4: 2, 59: 1, 65: 0, 67: 5, 72: 4, 84: 3, 94: 7 },
      upgrades: {
        0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 1, 7: 1,
        10: 1, 12: 1, 13: 2, 14: 1, 15: 1, 16: 2, 17: 3, 18: 5,
        23: 2, 28: 1, 29: 10, 35: 2, 36: 1, 38: 2,
      },
      cellLevels: [3, 2, 1, 0, 0, 0, 0, 0, 0],
      cellExp: [10, 5, 0, 0, 0, 0, 0, 0, 0],
    },
    estimate: { trials: 6, seed: 11 },
  },
  {
    name: 'clear',
    overrides: {
      fever: 2,
      obstruction: 0,
      layout: { 77: 1, 95: 1 },
      upgrades: { 0: 1, 1: 1, 12: 1, 16: 3, 28: 1, 36: 1 },
      cellLevels: [0, 0, 0, 0, 0, 0, 0, 0, 0],
      cellExp: [0, 0, 0, 0, 0, 0, 0, 0, 0],
    },
    estimate: { trials: 5, seed: 3 },
  },
  {
    name: 'simulator',
    overrides: {
      fever: 2,
      obstruction: 32,
      layout: { 0: 6, 4: 2, 59: 1, 65: 0, 67: 5, 72: 4, 84: 3, 94: 7 },
      upgrades: {
        0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 1, 7: 1,
        10: 1, 12: 1, 13: 2, 14: 1, 15: 1, 16: 3, 17: 3, 18: 5,
        23: 2, 28: 1, 29: 10, 35: 3, 36: 1, 38: 2,
      },
      cellLevels: [3, 2, 1, 0, 0, 0, 0, 0, 0],
      cellExp: [10, 5, 0, 0, 0, 0, 0, 0, 0],
    },
    // The simulator's own knobs: Stronkroid at 5s and the highest-DPS revive.
    simulation: { seed: 13, useRoid: true, roidTiming: 5, revivePolicy: 'dps-first' },
  },
  {
    name: 'simulator-empty',
    overrides: {
      fever: 0,
      obstruction: 0,
      layout: {},
      upgrades: { 0: 1, 1: 1 },
      cellLevels: [0, 0, 0, 0, 0, 0, 0, 0, 0],
      cellExp: [0, 0, 0, 0, 0, 0, 0, 0, 0],
    },
    simulation: { seed: 4, useRoid: false, roidTiming: 'none', revivePolicy: 'none' },
  },
  {
    name: 'sections-single',
    overrides: {},
    sections: { tokens: 1, quality: 'fast', seed: 5 },
  },
  {
    name: 'sections-beam-narrow',
    overrides: SECTION_PLOTS,
    sections: {
      tokens: 3,
      allowHypotheticalTokens: true,
      quality: 'fast',
      seed: 5,
      beamWidth: 2,
    },
  },
  {
    name: 'sections-beam-wide',
    overrides: SECTION_PLOTS,
    sections: {
      tokens: 3,
      allowHypotheticalTokens: true,
      quality: 'fast',
      seed: 5,
      beamWidth: 8,
    },
  },
  {
    name: 'sections-low-risk',
    overrides: SECTION_PLOTS,
    sections: {
      tokens: 1,
      allowHypotheticalTokens: true,
      quality: 'fast',
      seed: 9,
      strategy: 'low-risk',
      objective: 'clear',
    },
  },
  {
    name: 'sections-zero',
    overrides: SECTION_PLOTS,
    sections: { tokens: 0, quality: 'fast', seed: 5 },
  },
  {
    name: 'sections-exp',
    overrides: SECTION_PLOTS,
    sections: {
      tokens: 1,
      allowHypotheticalTokens: true,
      quality: 'fast',
      seed: 7,
      strategy: 'exp',
      objective: 'exp',
      expCellType: 1,
    },
  },
  {
    name: 'sections-dps',
    overrides: SECTION_PLOTS,
    sections: { tokens: 0, quality: 'fast', seed: 5, objective: 'dps' },
  },
  {
    name: 'locked-cell',
    overrides: {
      obstruction: 0,
      layout: { 77: 7 },
      upgrades: { 0: 1, 1: 1 },
    },
  },
  {
    name: 'overlap',
    overrides: {
      obstruction: 0,
      layout: { 77: 1, 78: 0 },
      upgrades: { 0: 1, 1: 1 },
    },
  },
];

const payload = {
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: ['js/stats/systems/w7/jelly-operator.js'],
    save: 'fixtures/data.json',
  },
  cases: cases.map((entry) => {
    const S = patched(entry.overrides);
    const layout = J.jellyLayoutFromSave(S);
    const output = {
      name: entry.name,
      overrides: entry.overrides,
      state: stateOf(S),
    };
    if (entry.estimate) {
      const operation = J.simulateJellyTrials(layout, S, {
        trials: entry.estimate.trials,
        seed: entry.estimate.seed,
        firingCalendar: false,
      });
      const estimate = estimateOf(S, layout, entry.estimate);
      output.estimate = estimate;
      output.estimateOptions = entry.estimate;
      output.metrics = metricsOf(S, layout);
      output.insights = estimate.valid ? insightsOf(S, layout, operation) : null;
    }
    if (entry.simulation) {
      output.simulationOptions = entry.simulation;
      output.simulation = simulationOf(S, layout, entry.simulation);
    }
    if (entry.sections) {
      output.sectionsOptions = entry.sections;
      output.sections = sectionPlanOf(S, entry.sections);
    }
    if (!entry.estimate && !entry.simulation && !entry.sections) {
      const validation = J.jellyValidateLayout(layout, S);
      output.validation = { valid: validation.valid, reason: validation.reason };
    }
    return output;
  }),
};

writeFileSync(
  new URL('../fixtures/jelly.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(`jelly: ${payload.cases.length} cases`);
for (const entry of payload.cases) {
  if (entry.estimate) {
    console.log(
      `${entry.name}: ${entry.estimate.successCount}/${entry.estimate.trials} clears, ` +
        `${entry.estimate.meanDamage.toFixed(0)} mean damage, ${entry.estimate.representative.endedBy}`,
    );
  } else if (entry.simulation) {
    console.log(
      `${entry.name}: ${entry.simulation.valid ? 'valid' : entry.simulation.reason}, ` +
        `${entry.simulation.trial.endedBy}, ${entry.simulation.trial.damage.toFixed(0)} damage`,
    );
  } else if (entry.sections) {
    console.log(
      `${entry.name}: tokens ${entry.sections.tokens}, ` +
        `${entry.sections.sequences.length} sequence(s), ` +
        `recommended ${entry.sections.recommendedPlotIds.join(',') || 'none'}`,
    );
  } else {
    console.log(`${entry.name}: ${entry.validation.valid ? 'valid' : entry.validation.reason}`);
  }
}
