// Writes the upstream Royal Guardian figures for the committed synthetic save,
// so `crates/royal-guardian/tests/royal_guardian.rs` compares the Rust port against the JS.
//
// Usage: node script/royal-oracle.mjs <path-to-idleon-research-optimizer>
//
// The clearing rate multiplies talent 231's *purchased* level by the Orblet,
// Spelunk and Jelly factors. Upstream raises that level with the all-talent
// bonus levels, which the Rust port does not decode yet, so the oracle reports
// the rate both ways: `xtraClearKillz` as upstream computes it, and
// `xtraClearKillzPurchasedOnly` with the all-talent levels left out. The test
// asserts the two agree on this save and names the gap when they do not.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/royal-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const R = await import(join(source, 'js/stats/systems/w7/royal-guardian.js'));
const O = await import(join(source, 'js/stats/systems/w7/royal-guardian-optimizer.js'));
const royalData = await import(join(source, 'js/stats/data/w7/royal-guardian.js'));
const C = await import(join(source, 'js/stats/systems/w7/royal-collect.js'));
const sushi = await import(join(source, 'js/stats/systems/w7/sushi.js'));
const arcade = await import(join(source, 'js/stats/systems/w2/arcade.js'));
const data = await import(join(source, 'js/stats/data/w7/royal-guardian.js'));
const talents = await import(join(source, 'js/stats/data/common/talent.js'));
const talentSystem = await import(join(source, 'js/stats/systems/common/talent.js'));
const maps = await import(join(source, 'js/stats/data/common/maps.js'));
const spelunk = await import(join(source, 'js/stats/systems/w7/spelunking.js'));
const jelly = await import(join(source, 'js/stats/systems/w7/jelly-operator.js'));

const save = JSON.parse(
  readFileSync(new URL('../fixtures/data.json', import.meta.url), 'utf8'),
);
loader.loadSaveData({ data: save });
const S = state.saveData;

const WORLDS = 7;
const number = (value) => Number(value) || 0;

// The purchased-only talent, as the port reads it: the highest saved level
// across the characters through the talent's own curve. The loader decodes
// `SL_<n>` into `skillLvData`, so the raw save is the place to read the row.
const curve = talents.talentParams(231);
let purchasedLevel = 0;
for (const key of Object.keys(save).filter((name) => /^SL_\d+$/.test(name))) {
  const levels = JSON.parse(save[key]);
  purchasedLevel = Math.max(purchasedLevel, number(levels?.['231']));
}
const purchasedMulti = purchasedLevel > 0
  ? Math.max(1, 1 + curve.x1 * purchasedLevel / (purchasedLevel + curve.x2))
  : 0;
const upstreamDetail = talentSystem.maxTalentBonusDetail(
  231,
  R.royalGuardianCharacter(S).index,
  S,
);
const ext = { talent231Multi: purchasedMulti };

const counters = R.kingdomTotals(S);
const rows = R.outpostRows(S, ext);
const eligible = [0, 1, 2, 3, 4, 5, 6].flatMap((world) => R.worldOutpostRows(S, world));
const clearingTargets = eligible.filter(
  (row) => !row.built && R.outpostClearMetrics(S, row.mapIdx, ext).state === 'clearing',
);
const built = rows.map((row) => ({
  map: row.mapIdx,
  type: row.type,
  range: row.range,
  units: R.outpostTotalUnits(S, row.mapIdx),
  guards: R.totalUnitsByType(S, row.mapIdx)[2],
  purified: R.outpostIsPurified(S, row.mapIdx),
  glorified: R.outpostIsGlorified(S, row.mapIdx),
}));

const worlds = [];
for (let world = 0; world < WORLDS; world += 1) {
  const list = R.worldOutpostRows(S, world);
  const clearing = list.filter((row) => !row.built && R.outpostClearMetrics(S, row.mapIdx, ext).state === 'clearing');
  worlds.push({
    world,
    unlocked: R.royalWorldUnlocked(S, world),
    outposts: list.length,
    built: list.filter((row) => row.built).length,
    clearing: clearing.length,
  });
}

// The page lists the maps the world pages carry: a save row past the map
// catalog has no anchor, so the sections never show it.
// RG-02: the collection rate, the outpost map, the resource nodes, the
// currencies, the drain staffing and the banked hours. Every rate is computed
// with the upstream's own derived inputs, so the Rust port can be compared
// factor by factor and, once those inputs are handed in, number by number.
const derived = R.royalGuardianDerivedInputs(S);
const rateExt = {
  derivedInputs: derived,
  talent231Multi: purchasedMulti,
  spelunkBigFish6: derived.spelunkBigFish6,
};
const globalRate = R.globalResourceRateBreakdown(S, rateExt);

// The four talents as the port reads them: the highest purchased level through
// the talent's own curve. Upstream adds the all-talent bonus levels per talent,
// which the port does not decode, so the oracle prints both values.
const purchasedTalent = (id) => {
  const curve = talents.talentParams(id);
  let level = 0;
  for (const key of Object.keys(save).filter((name) => /^SL_\d+$/.test(name))) {
    const rows = JSON.parse(save[key]);
    level = Math.max(level, number(rows?.[String(id)]));
  }
  if (level <= 0) return 0;
  const value = curve.formula === 'decayMulti'
    ? 1 + curve.x1 * level / (level + curve.x2)
    : curve.x1 * level / (level + curve.x2);
  return value;
};
const purchasedTalents = {
  225: purchasedTalent(225),
  226: purchasedTalent(226),
  230: Math.max(1, purchasedTalent(230)),
  // The clearing rate's own figure above, at the multiplier's floor.
  231: Math.max(1, purchasedMulti),
};
const portRate = R.globalResourceRateBreakdown(S, {
  ...rateExt,
  talent225: purchasedTalents[225],
  talent226: purchasedTalents[226],
  talent230Multi: purchasedTalents[230],
  alchemyW14: 0,
  companion141: 0,
});
const allocation = O.resourceAllocationToReset(S, { ext: rateExt });
const bankedProjection = O.bankedResourceProjection(S, { mode: 'current', ext: rateExt });

const connectionRows = (mapIdx) =>
  R.outpostConnections(S, mapIdx).map((connection, slot) => ({
    slot,
    kind: connection.kind,
    endpoint: connection.kind === 'empty' ? null : connection.id,
    raw: Number.isFinite(Number(S.royalMapsData?.[mapIdx]?.[8 + slot]))
      ? Number(S.royalMapsData[mapIdx][8 + slot])
      : null,
  }));

const outpostRows = [];
for (let mapIdx = 0; mapIdx < (S.royalMapsData || []).length; mapIdx += 1) {
  const row = R.outpostEligibility(S, mapIdx);
  if (!row.staticEligible) continue;
  const position = R.outpostMapPosition(mapIdx);
  const resources = R.reachableResourcesForOutpost(S, mapIdx, rateExt);
  outpostRows.push({
    map: mapIdx,
    world: row.worldIdx,
    built: row.built,
    state: row.state,
    type: R.outpostType(S, mapIdx),
    x: position?.x ?? null,
    y: position?.y ?? null,
    range: row.built ? R.outpostRange(S, mapIdx, rateExt) : null,
    purified: row.built ? R.outpostIsPurified(S, mapIdx) : false,
    glorified: row.built ? R.outpostIsGlorified(S, mapIdx) : false,
    supports: R.outpostSupports(S, mapIdx),
    supportCount: row.built ? R.supportCount(S, mapIdx) : 0,
    supportLinks: row.built
      ? R.outpostSupportLinks(S, mapIdx).map((link) => ({ map: link.mapIdx, slot: link.slot }))
      : [],
    connections: connectionRows(mapIdx),
    units: row.built ? R.totalUnitsByType(S, mapIdx) : [0, 0, 0, 0],
    collectionRate: row.built ? R.outpostResourceRateBreakdown(S, mapIdx, rateExt).value : 0,
    resourceRate: row.built
      ? resources.map((value) => ({
          resource: value.resourceIdx,
          distance: value.distance,
          reachable: value.reachable,
        }))
      : [],
    supportTargets: row.built && R.outpostType(S, mapIdx) === 1
      ? R.supportTargetsForOutpost(S, mapIdx, rateExt)
          .filter((target) => target.reachable)
          .map((target) => target.mapIdx)
      : [],
  });
}

const nodeRows = [];
for (let resourceIdx = 0; resourceIdx < 80; resourceIdx += 1) {
  const detail = allocation.details[resourceIdx];
  nodeRows.push({
    resource: resourceIdx,
    world: R.resourceWorld(resourceIdx),
    grade: R.resourceGrade(S, resourceIdx),
    baseCapacity: Number(royalData.ROYAL_RESOURCES[resourceIdx]?.baseCapacity) || 0,
    capacity: R.resourceCapacity(S, resourceIdx),
    rawProgress: R.resourceRawProgress(S, resourceIdx),
    collected: R.resourceCollected(S, resourceIdx),
    remaining: R.resourceRemaining(S, resourceIdx),
    drained: R.resourceDrained(S, resourceIdx),
    currencySlot: R.resourceCurrency(resourceIdx),
    drainRate: detail.rate,
    currencyRate: detail.currencyRate,
    drainHours: Number.isFinite(detail.drainHours) ? detail.drainHours : null,
    collectableBeforeReset: detail.collectableBeforeReset,
    initiallyDrained: detail.initiallyDrained,
    initiallyEmpty: detail.initiallyEmpty,
    emptiesByReset: detail.emptiesByReset,
    refillsAtReset: detail.refillsAtReset,
    refillRequiresCollection: detail.refillRequiresCollection,
    refillBoundary: detail.refillBoundary,
    gradeGainAtReset: detail.gradeGainAtReset,
    projectedGradeAtReset: detail.projectedGradeAtReset,
    projectedRemainingAtReset: detail.projectedRemainingAtReset,
    streams: detail.streams.map((stream) => ({
      map: stream.mapIdx,
      slot: stream.slot,
      rate: stream.rate,
      drainRate: stream.drainRate,
      currencyRate: stream.currencyRate,
      savage: stream.savage,
    })),
  });
}

const currencyRows = O.validCurrencySlots().map((slot) => {
  const income = O.currencyIncomeToReset(S, slot, { ext: rateExt });
  return {
    slot,
    balance: income.balance,
    nominalPerHour: income.nominalCurrencyPerHour,
    collectableWithinWindow: income.collectableWithinWindow,
    activeStreams: income.activeStreams,
    activeNodes: income.activeNodes,
    remainingNow: income.remainingNow,
    projectedAvailableAtReset: income.projectedAvailableAtReset,
    collectableBeforeReset: income.collectableBeforeReset,
    nodes: income.nodes.map((node) => ({
      resource: node.resourceIdx,
      currencyRate: node.currencyRate,
      collectableWithinWindow: node.collectableWithinWindow,
    })),
  };
});

const clearingRows = [];
for (const mapIdx of eligible.map((row) => row.mapIdx)) {
  const detail = R.outpostClearMetrics(S, mapIdx, ext);
  clearingRows.push({
    map: mapIdx,
    state: detail.state,
    // The page derives `active` from the state; `outpostClearMetrics` reports
    // `accruing` instead.
    active: detail.state === 'clearing',
    accruing: detail.accruing === true,
    progress: detail.progress,
    requirement: detail.requirement,
    remaining: detail.remaining,
    ratePerHour: detail.ratePerHour,
    assignments: detail.assignments,
    etaHours: Number.isFinite(detail.etaHours) ? detail.etaHours : null,
  });
}

const rangeTargets = [];
for (const row of rows) {
  const target = R.nextResourceRangeTarget(S, row.mapIdx, ext);
  rangeTargets.push({
    map: row.mapIdx,
    range: row.range,
    target: target
      ? {
          resource: target.resourceIdx,
          distance: target.distance,
          reach: row.range + 15,
          possible: target.plan.possible,
          currentLevel: target.plan.currentLevel,
          requiredRange: target.plan.requiredRange,
          targetLevel: target.plan.targetLevel,
          levelsNeeded: target.plan.levelsNeeded,
          reason: target.plan.reason ?? null,
        }
      : null,
  });
}


// RG-03: the upgrade list, the global breakdowns and the forecast.
//
// Three sources the port does not decode yet - the all-talent bonus levels, the
// W14 alchemy bubble and the companion roster - are handed to the upstream as
// explicit overrides, so the comparison is against the same inputs the port
// reads. Without the override the upstream would price and project with bonuses
// the port leaves out, and every figure would differ for the wrong reason.
const purchased = {
  talent225: purchasedTalents[225],
  talent226: purchasedTalents[226],
  talent230Multi: purchasedTalents[230],
};
const portExt = {
  derivedInputs: derived,
  talent231Multi: purchasedMulti,
  spelunkBigFish6: derived.spelunkBigFish6,
  talent225: purchased.talent225,
  talent226: purchased.talent226,
  talent230Multi: purchased.talent230Multi,
  alchemyW14: 0,
  companion141: 0,
};

const redux = R.allMasterclassCostReduxDetail(S);
const unlockedCount = R.armoryUnlockedCount(S);
const armoryList = R.armoryRows(S).map((row) => {
  const upgrade = data.ARMORY_UPGRADES[row.index];
  const balance = number(S.royalGData?.[1]?.[row.cost.currencySlot]);
  return {
    index: row.index,
    order: row.orderIndex,
    name: upgrade?.name ?? '',
    level: row.currentLevel,
    maxLevel: number(upgrade?.maxLevel),
    bonus: row.currentBonus,
    nextBonus: row.nextBonus,
    bonusPerLevel: number(upgrade?.bonusPerLevel),
    cost: row.cost.value,
    currencySlot: row.cost.currencySlot,
    balance,
    maxed: row.currentLevel >= number(upgrade?.maxLevel),
    unlocked: row.orderIndex < unlockedCount,
    affordable: row.currentLevel < number(upgrade?.maxLevel) && row.cost.value <= balance,
  };
});
const planFor = (order) => {
  const plan = R.armoryCostProjection(S, order, 50);
  if (!plan) return null;
  return {
    index: plan.index,
    order: plan.orderIndex,
    currencySlot: plan.currencySlot,
    balance: plan.balance,
    dailyPurchaseLimit: plan.dailyPurchaseLimit,
    purchaseCounter: plan.purchaseCounter,
    discountsLeftToday: plan.discountsLeftToday,
    discountAvailable: plan.discountAvailable,
    rows: plan.rows.map((row) => ({
      level: row.level,
      nextLevel: row.nextLevel,
      discounted: row.discounted,
      full: row.full,
      cumulativeDiscounted: row.cumulativeDiscounted,
      cumulativeFull: row.cumulativeFull,
    })),
  };
};
const orbletList = R.orbletRows(S).map((row) => ({
  index: row.index,
  name: row.name,
  level: row.level,
  maxLevel: row.maxLevel,
  maxed: row.maxed,
  unlocked: row.unlocked,
  kind: row.kind,
  cost: Number.isFinite(row.cost) ? row.cost : null,
  bonus: row.bonus,
  nextBonus: row.nextBonus,
  owned: row.owned,
  affordable: row.affordable,
  partial: row.partial,
  missing: row.missing,
}));
const ownedOrblets = R.orbletOwnedDetail(S);

const collectionRate = R.globalResourceRateBreakdown(S, portExt);
const rangeRules = R.globalRangeBreakdown(S);
const rankExp = R.globalRankExpBreakdown(S, portExt);
const pointRules = R.globalPointBreakdown(S);
const unitRules = R.globalUnitBreakdowns(S, portExt);
const supportRules = R.globalSupportBreakdown(S);
const savageRules = R.globalSavageBreakdown(S);
const breakdowns = {
  collection: {
    value: collectionRate.value,
    factors: collectionRate.factors.map((factor) => ({ name: factor.name, value: factor.value })),
  },
  range: {
    base: rangeRules.base,
    orblet: rangeRules.orblet.bonus,
    cap: rangeRules.cap,
    resourceAllowance: rangeRules.allowances.resource,
    outpostAllowance: rangeRules.allowances.outpost,
    value: rangeRules.base + rangeRules.orblet.bonus,
  },
  rankExp: {
    value: rankExp.bars[rankExp.selectedBar].baseRate,
    selectedBar: rankExp.selectedBar,
    orblet: rankExp.orblet.bonus,
    shop65: rankExp.shop65,
    shop77: rankExp.shop77,
    bars: rankExp.bars.map((bar) => ({ bar: bar.bar, name: bar.name, baseRate: bar.baseRate })),
  },
  points: {
    value: pointRules.base + pointRules.worlds.reduce((sum, world) => sum + world.armory.bonus, 0),
    base: pointRules.base,
    worlds: pointRules.worlds.map((world) => world.armory.bonus),
  },
  units: {
    value: unitRules.rules.reduce((sum, rule) => sum + rule.stats[0].value, 0),
    rules: unitRules.rules.map((rule) => ({
      id: rule.id,
      name: rule.name,
      label: rule.stats[0].label,
      base: rule.stats[0].base,
      armory: rule.stats[0].armory.bonus,
      value: rule.stats[0].value,
      unit: rule.stats[0].unit,
    })),
  },
  support: {
    value: supportRules.value,
    base: supportRules.basePercent,
    armory: supportRules.armory.bonus,
    jelly: supportRules.jellyBonus,
  },
  savage: {
    value: savageRules.value,
    base: savageRules.baseMultiplier,
    armory: savageRules.armory.bonus,
    currency: savageRules.currencyMultiplier,
  },
};

// The forecast engine, ported by royal-forecast.js. Two scenarios: the default
// horizon with no collects, and a three-collect run with a menu-open window.
const F = await import(join(source, 'js/stats/systems/w7/royal-forecast.js'));
const flattenForecast = (result) => ({
  horizonHours: result.horizonHours,
  partial: result.partial,
  missing: result.missing,
  banks: result.banks,
  bankedStartHours: result.bankedStartHours,
  endBankedHours: result.endBankedHours,
  resets: result.resets,
  ignoredCollects: result.ignoredCollects,
  truncated: result.truncated,
  collects: result.collects.map((commit) => ({
    at: commit.at,
    kind: commit.kind,
    banked: commit.banked,
    currency: commit.currency,
    drained: commit.drained,
    rankUps: commit.rankUps.map((up) => ({ map: up.mapIdx, bar: up.bar, from: up.from, to: up.to, pointsBefore: up.pointsBefore, pointsAfter: up.pointsAfter })),
    points: commit.points,
    secured: commit.secured,
    // A commit's affordable entries carry no hour upstream; the ghost versions do.
    affordable: commit.affordable.map((item) => ({ index: item.index, order: item.orderIndex, cost: item.cost, currencySlot: item.currencySlot, at: item.at ?? null })),
  })),
  windows: result.windows.map((window) => ({
    start: window.start,
    end: window.end,
    entry: window.entry ? { at: window.entry.at, kind: window.entry.kind, banked: window.entry.banked, currency: window.entry.currency } : null,
    summary: {
      currency: window.summary.currency,
      drained: window.summary.drained,
      rankUps: window.summary.rankUps.length,
      points: window.summary.points,
      secured: window.summary.secured,
      affordable: window.summary.affordable.map((item) => ({ index: item.index, order: item.orderIndex, cost: item.cost, at: item.at })),
    },
    steps: window.steps,
    truncated: window.truncated,
  })),
  resetEvents: result.resetEvents.map((event) => ({
    at: event.at,
    refills: event.refills,
    refilled: event.refilled,
    gradeGains: event.gradeGains,
    missed: event.missed.map((item) => ({ resource: item.resourceIdx, readyAt: item.readyAt })),
  })),
  segments: result.segments.map((segment) => ({
    start: segment.start,
    end: segment.end,
    bankedAtStart: segment.bankedAtStart,
    bankedAtEnd: segment.bankedAtEnd,
    ghosts: {
      fills: segment.ghosts.fills.map((fill) => ({ resource: fill.resourceIdx, at: fill.at })),
      ranks: segment.ghosts.ranks.map((ghost) => ({ map: ghost.mapIdx, bar: ghost.bar, rank: ghost.rank, from: ghost.from, at: ghost.at, ready: ghost.ready === true })),
      affordable: segment.ghosts.affordable.map((item) => ({ index: item.index, cost: item.cost, currencySlot: item.currencySlot, at: item.at })),
      clears: segment.ghosts.clears.map((ghost) => ({ map: ghost.mapIdx, at: ghost.at })),
    },
    nodes: segment.nodes.map((node) => ({
      resource: node.resourceIdx,
      capacity: node.capacity,
      grade: node.grade,
      quantity: node.quantity,
      drained: node.drained,
      committed: node.committed,
      fillAt: node.fillAt,
      projected: node.projected.map((point) => ({ t: point.t, f: point.f })),
    })),
    currency: segment.currency,
  })),
  summary: {
    collects: result.summary.collects,
    windows: result.summary.windows,
    currency: result.summary.currency,
    rankUps: result.summary.rankUps,
    points: result.summary.points,
    secured: result.summary.secured,
    refilled: result.summary.refilled,
    missed: result.summary.missed,
    gradeGains: result.summary.gradeGains,
    bankedAtHorizon: result.summary.bankedAtHorizon,
  },
});
const forecastDefault = flattenForecast(F.forecastRoyal(S, { horizonHours: 72, ext: portExt }));
const forecastPlanned = flattenForecast(F.forecastRoyal(S, {
  horizonHours: 48,
  collects: [6, 12.5, 30],
  windows: [{ start: 20, end: 22 }],
  ext: portExt,
}));

const planOrders = armoryList
  .filter((row) => !row.maxed)
  .slice(0, 2)
  .map((row) => row.order);
const upgrades = {
  masterclassRedux: {
    value: redux.value,
    prefix: redux.prefix,
    bargain: redux.bargain,
    reductionPercent: redux.reductionPercent,
    purchaseCounter: redux.purchaseCounter,
    dailyLimit: redux.dailyPurchaseLimit,
  },
  unlockedCount,
  armory: armoryList,
  planOrders,
  plans: planOrders.map((order) => planFor(order)),
  orblets: orbletList,
  owned: ownedOrblets,
};

const payload = {
  counters: {
    builtOutposts: counters.outposts,
    outposts: worlds.reduce((sum, world) => sum + world.outposts, 0),
    worldsUnlocked: worlds.filter((world) => world.unlocked).length,
    armoryLevels: R.armoryTotalLevels(S),
    units: built.reduce((sum, row) => sum + row.units, 0),
    purified: built.filter((row) => row.purified).length,
    support: built.filter((row) => row.type === 1).length,
    savage: built.filter((row) => row.type === 2).length,
    militiaClearing: clearingTargets.length,
    militiaClearsPerHour: clearingTargets.reduce(
      (sum, row) => sum + R.militiaClearRate(S, row.mapIdx, ext),
      0,
    ),
  },
  talent: {
    purchasedLevel,
    purchasedMulti,
    // What upstream adds on top: the all-talent levels the port does not read.
    allTalentLevels: upstreamDetail.detail?.bonus ?? 0,
    xtraClearKillz: R.XtraClearKillz(S),
    xtraClearKillzPurchasedOnly: R.XtraClearKillz(S, ext),
  },
  // The two cross-system factors the clearing rate reads, beside the Orblet one
  // the save carries.
  crossSystem: {
    bigFish6: spelunk.computeBigFishBonus(6, S),
    jelly2: jelly.jellyCompletionBonus(2, S),
  },
  collection: {
    // The derived inputs the rate multiplies in, as upstream reads them. The
    // port reads all of them but the alchemy bubble and the companion roster.
    inputs: {
      charLevel: derived.charLevel,
      talent225: derived.talent225,
      talent226: derived.talent226,
      talent230Multi: derived.talent230Multi,
      sushi60: derived.sushi60,
      arcade70: derived.arcade70,
      zenith10: derived.zenith10,
      alchemyW14: derived.alchemyW14,
      companion141: derived.companion141,
      uniqueSushi: sushi.computeUniqueSushi(S.sushiData),
      arcadeLevel70: Number(S.arcadeUpgData?.[70]) || 0,
      spelunkZenithLevel: Number(S.spelunkData?.[45]?.[10]) || 0,
    },
    global: globalRate.value,
    factors: Object.fromEntries(globalRate.factors.map((factor) => [factor.name, factor.value])),
    // What the section shows today: the port's purchased-only talents, with the
    // alchemy bubble and the companion roster it does not read left at one.
    talents: purchasedTalents,
    globalReadOnly: portRate.value,
    supportCollection: R.supportCollection(S),
    savageCollection: R.savageCollection(S),
    totalStats: R.TotalStatz(S),
  },
  banked: {
    banksTime: C.royalBanksTime(S),
    hours: C.royalBankedHours(S),
    available: bankedProjection.available,
    bankedGain: bankedProjection.bankedGain,
    currencyGain: bankedProjection.currencyGain,
  },
  reset: {
    available: allocation.reset.available,
    hoursRemaining: allocation.hoursToReset,
    projectionAvailable: allocation.resetProjectionAvailable,
    collectableBeforeReset: allocation.collectableBeforeReset,
    projectedRemainingAtReset: allocation.projectedRemainingAtReset,
    nodesEmptyingByReset: allocation.nodesEmptyingByReset,
    projectedRefillsAtReset: allocation.projectedRefillsAtReset,
    projectedGradeGainsAtReset: allocation.projectedGradeGainsAtReset,
  },
  outposts: outpostRows,
  resources: nodeRows,
  currencies: currencyRows,
  worlds,
  built,
  rangeTargets,
  clearing: clearingRows,
  upgrades,
  breakdowns,
  forecast: { default: forecastDefault, planned: forecastPlanned },
  // The map names the tables print, kept beside the numbers so a rename shows up.
  names: Object.fromEntries(
    [...new Set(clearingRows.map((row) => row.map))].map((map) => [map, maps.mapDisplayName(map)]),
  ),
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/stats/systems/w7/royal-guardian.js',
      'js/stats/data/w7/royal-guardian.js',
      'js/stats/data/common/talent.js',
    ],
    save: 'fixtures/data.json',
  },
};

writeFileSync(
  new URL('../fixtures/royal-guardian.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(
  `wrote ${clearingRows.length} clearing rows, ${built.length} built outposts, ${rangeTargets.length} range rows, ` +
    `${armoryList.length} shelves, ${forecastPlanned.summary.collects} planned collects`,
);
