// Regenerates the Kitchens & Jellyfish fixture from the upstream calculator.
// Usage: node script/ladle-kitchen-oracle.mjs <upstream-repo>
//
// The upstream `js/stats/systems/w4/cooking-ladle.js` builds kitchen speeds from
// the account stats engine. This app enters that speed instead, so the oracle
// feeds `optimizeKitchenReset` and `planMealLeveling` a hand-built model whose
// meal coefficients are empty and whose account multipliers are zero: every
// kitchen's speed is then the port's `base × (1 + Speed Lv / 10) × (1 + dn1)`,
// and the batch and daily plan run exactly as the port does. `baseSpeed` is read
// back out of the upstream speed model so the Rust side is compared against the
// calculator's own number.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/ladle-kitchen-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const L = await import(join(source, 'js/stats/systems/w4/cooking-ladle.js'));
const { MealINFO } = await import(join(source, 'js/stats/data/game/customlists.js'));

const mealCostInputs = (overrides = {}) => ({
  comp162: 0,
  ach233: 0,
  cloud33: 0,
  ola193: 0,
  dream11: 0,
  ...overrides,
});

/// The upstream speed model with every account term neutral: only the base speed,
/// the kitchen's speed level and the gem doubler move a rate.
const speedModel = (kitchens, gems120 = 0) => ({
  charIdx: 0,
  t59: 0,
  t59Source: 'account best',
  cropSC3: 0,
  enh146: { val: 0, gate: 0, maps: 0 },
  eventShop53: 0,
  clam10: 0,
  gems120,
  vote13: 0,
  vault54: 0,
  farmLv: 0,
  mealSpdzBase: 0,
  atom8PerLevel: 0,
  msa1: 0,
  art13: 0,
  art13PerLog: 0,
  turkeyStock: 0,
  button7: 0,
  arcade28: 0,
  vialTurtle: 0,
  vialMealCook: 0,
  stampMealCook: 0,
  mf114: 0,
  star58: 0,
  win15: 0,
  menagerie4: 0,
  monument: 0,
  bupg56: 0,
  cardW6c1: 0,
  lamp: 0,
  vialCookSpd: 0,
  mf100: 0,
  achTerm: 0,
  mealCoef: { Mcook: [], zMealFarm: [], KitchenEff: [] },
  kitchens: kitchens.map((kitchen, index) => ({
    idx: index,
    // 0 locked, 1 idle, 2 cooking the saved meal.
    state: !kitchen.unlocked ? 0 : kitchen.savedMeal >= 0 ? 2 : 1,
    meal: kitchen.savedMeal ?? -1,
    unlocked: kitchen.unlocked,
    dn1: gems120 > index ? 2 : 0,
    speedLv: kitchen.speedLv,
    fireLv: 0,
    luckLv: 0,
  })),
});

/// The port's base speed: one kitchen's rate at Speed Lv 0 with no doubler.
const baseSpeed = (model, levels) => {
  const zeros = levels.map(() => 0);
  const kitchen = model.kitchens.find((row) => row.unlocked);
  return (
    L.cookingSpeed(model, kitchen.idx, zeros) /
    (1 + kitchen.speedLv / 10) /
    (1 + kitchen.dn1)
  );
};

/// The speed model's own total at `levels`, which the port's kitchens must match.
const rates = (model, base) =>
  model.kitchens
    .filter((kitchen) => kitchen.unlocked)
    .map((kitchen) => base * (1 + kitchen.speedLv / 10) * (1 + kitchen.dn1));

const speedTotal = (model, levels) => L.totalKitchenSpeed(model, levels, 0);

// The meal cost grid: every input the formula reads, over the tiers it steps at.
const costGrid = [];
for (const level of [0, 1, 5, 11, 30, 60, 111, 130, 149, 150, 260]) {
  for (const inputs of [
    mealCostInputs(),
    mealCostInputs({ comp162: 25, ach233: 1, cloud33: 0.5, dream11: 6 }),
    mealCostInputs({ comp162: 5, cloud33: 1, dream11: 10 }),
  ]) {
    for (const discountDays of [0, 3, 10]) {
      const cost = L.mealLevelCost(level, inputs, discountDays);
      // Levels past the formula's range overflow a double; JSON writes them as null.
      if (Number.isFinite(cost)) costGrid.push({ level, inputs, discountDays, cost });
    }
  }
}

// Meal levels and stocks a mid-game save carries, short enough to read.
const levels = [14, 12, 11, 9, 7, 6, 4, 3, 2, 1];
const stocks = levels.map((_, meal) => meal * 250);

const kitchenSets = [
  [
    { unlocked: true, savedMeal: 1, speedLv: 20 },
    { unlocked: true, savedMeal: 1, speedLv: 0 },
    { unlocked: true, savedMeal: -1, speedLv: 40 },
    { unlocked: false, savedMeal: -1, speedLv: 0 },
  ],
  [
    { unlocked: true, savedMeal: 0, speedLv: 100 },
    { unlocked: true, savedMeal: 2, speedLv: 50 },
    { unlocked: true, savedMeal: 4, speedLv: 10 },
  ],
];

// The standalone batch: one reset's Blue Jellyfish grant in every kitchen.
const batchScenarios = [];
for (const [index, kitchens] of kitchenSets.entries()) {
  const model = speedModel(kitchens, index === 1 ? 2 : 0);
  const base = baseSpeed(model, levels);
  const rateTable = rates(model, base);
  for (const kitchenLayout of [
    { mode: 'optimized' },
    { mode: 'saved' },
    { mode: 'optimized', excluded: [2, 6], pins: { 0: 3 } },
  ]) {
    for (const options of [
      { hours: 12, maxLevel: 30, ola193: 0, inputs: mealCostInputs() },
      {
        hours: 40,
        maxLevel: 30,
        ola193: 4,
        inputs: mealCostInputs({ dream11: 6, comp162: 10 }),
      },
    ]) {
      const batch = L.optimizeKitchenReset(
        {},
        {
          model,
          levels,
          stocks,
          inputs: options.inputs,
          maxLevel: options.maxLevel,
          hours: options.hours,
          ola193: options.ola193,
          cachedTurkeyStock: 0,
          objective: 'ranks',
          kitchenLayout,
        },
      );
      batchScenarios.push({
        baseSpeed: base,
        rates: rateTable,
        speedTotal: speedTotal(model, levels),
        levels,
        stocks,
        hours: options.hours,
        maxLevel: options.maxLevel,
        discountDays: options.ola193,
        inputs: options.inputs,
        kitchenLayout,
        kitchens: kitchens.map((kitchen, position) => ({
          index: position,
          ...kitchen,
          doubler: index === 1 && position < 2,
        })),
        expected: {
          ranks: batch.ranks,
          purchaseOrder: batch.purchaseOrder,
          levels: batch.levels,
          stocks: batch.stocks,
          meals: batch.meals,
          kitchens: batch.kitchens,
        },
      });
    }
  }
}

// The daily plan: claims, donations, the direct spend order and the reset batch.
const planScenarios = [];
for (const scenario of [
  {
    name: 'share the income, three resets',
    kitchens: kitchenSets[0],
    inputs: mealCostInputs({ comp162: 25, dream11: 4 }),
    settings: {
      days: 3,
      hoursToReset: 7.5,
      ladlesPerDay: 12,
      startLadles: 40,
      ladleMulti: 1.5,
      passive: true,
      ladleableOnly: true,
      maxLevel: 30,
      policy: { kind: 'share', incomeFraction: 0.5, ownedFraction: 0.5 },
      kitchenLayout: { mode: 'optimized' },
    },
  },
  {
    name: 'keep every ladle, two resets',
    kitchens: kitchenSets[1],
    inputs: mealCostInputs({ ach233: 1, cloud33: 0.5, dream11: 3 }),
    settings: {
      days: 2,
      hoursToReset: 24,
      ladlesPerDay: 30,
      startLadles: 120,
      ladleMulti: 1,
      passive: true,
      ladleableOnly: true,
      maxLevel: 40,
      policy: { kind: 'none', incomeFraction: 1, ownedFraction: 0 },
      kitchenLayout: { mode: 'optimized', excluded: [5] },
    },
  },
  {
    name: 'donate a fixed share, one reset',
    kitchens: kitchenSets[0],
    inputs: mealCostInputs(),
    settings: {
      days: 1,
      hoursToReset: 12,
      ladlesPerDay: 20,
      startLadles: 0,
      ladleMulti: 1,
      passive: false,
      ladleableOnly: false,
      maxLevel: 30,
      policy: { kind: 'fixed', perDay: 7, ownedFraction: 0 },
      kitchenLayout: { mode: 'saved' },
    },
  },
]) {
  const { settings } = scenario;
  const model = speedModel(scenario.kitchens, 0);
  const base = baseSpeed(model, levels);
  // The port builds the donation schedule from the guarantee, so the oracle does too.
  const income = (hours) => settings.ladlesPerDay * hours / 24;
  const target = (amount) => {
    switch (settings.policy.kind) {
      case 'none':
        return 0;
      case 'all':
        return amount;
      case 'share':
        return amount * settings.policy.incomeFraction;
      default:
        return Math.min(amount, settings.policy.perDay);
    }
  };
  const daily = [];
  for (let day = 1; day <= settings.days; day += 1) {
    const hours = day === 1 ? settings.hoursToReset : 24;
    daily.push(settings.policy.kind === 'none' ? 0 : Math.floor(target(income(hours))));
  }
  const owned =
    settings.policy.kind === 'none'
      ? 0
      : Math.floor(settings.startLadles * settings.policy.ownedFraction);
  const saved = {
    menagerieData: [[0, 0, 0, 0, 0, 9], [], [0, 0, 0, 0, 0, 1], []],
    menagerieDataAvailable: true,
    ninjaData: { 102: { 9: 'p' } },
    bundlesData: { bun_s: 1 },
    mealsData: [levels, [], stocks],
  };
  const plan = L.planMealLeveling(saved, {
    model,
    levels,
    stocks,
    inputs: scenario.inputs,
    maxLevel: settings.maxLevel,
    ladleMulti: settings.ladleMulti,
    days: settings.days,
    ladlesPerDay: settings.ladlesPerDay,
    startLadles: settings.startLadles,
    passive: settings.passive,
    ladleableOnly: settings.ladleableOnly,
    hoursToReset: settings.hoursToReset,
    strategy: 'cheapest',
    floorGuard: false,
    kitchenLayout: settings.kitchenLayout,
    jellyDonations: { owned, daily },
  });
  planScenarios.push({
    name: scenario.name,
    baseSpeed: base,
    rates: rates(model, base),
    speedTotal: speedTotal(model, levels),
    levels,
    stocks,
    kitchens: scenario.kitchens.map((kitchen, position) => ({
      index: position,
      ...kitchen,
      doubler: false,
    })),
    jelly: { available: true, unlocked: true, dailyHours: 9 },
    nmlb: 3,
    inputs: scenario.inputs,
    settings,
    donationSchedule: { owned, daily },
    expected: {
      days: plan.days,
      speed: speedTotal(model, levels),
      dailyIncome: plan.dailyIncome,
      ladleMultiplier: plan.ladleMultiplier,
      maxLevel: plan.maxLevel,
      jelly: {
        donated: plan.jelly.donated,
        dailyHoursEnd: plan.jelly.dailyHoursEnd,
        totalHours: plan.jelly.totalHours,
        ownedDonation: plan.jelly.ownedDonation,
      },
      startLevels: plan.startLevels,
      levels: plan.levels,
      stocks: plan.stocks,
      gained: plan.gained,
      bySource: plan.bySource,
      ladlesUsed: plan.ladlesUsed,
      nextNmlb: plan.nextNmlb,
      timeline: plan.timeline.map((day) => ({
        day: day.day,
        windowHours: day.windowHours,
        ladleIncome: day.ladleIncome,
        expectedIncome: day.expectedIncome,
        jellyDonation: day.jellyDonation,
        jellyHours: day.jellyHours,
        levelsBought: day.levelsBought,
        ladlesUsed: day.ladlesUsed,
        passiveHoursUsed: day.passiveHoursUsed,
        nmlbMeal: day.nmlbMeal,
        nmlbFrom: day.nmlbFrom,
        nmlbTo: day.nmlbTo,
        nmlbSaved: day.nmlbSaved,
        totalLevels: day.totalLevels,
        meals: day.meals,
        steps: day.steps.map((step) => ({
          meal: step.meal,
          from: step.fromLevel,
          to: step.toLevel,
          ladles: step.ladles,
          hours: step.hours,
          cumulativeLadles: step.cumulativeLadles,
          mapRefresh: step.mapRefresh,
        })),
        savings: day.savings.map((saving) => ({
          meal: saving.meal,
          toLevel: saving.toLevel,
          hours: saving.hours,
          ladles: saving.ladles,
          progress: saving.progress,
          target: saving.target,
          mapRefresh: saving.mapRefresh,
        })),
        phases: day.phases.map((phase) => ({
          kind: phase.kind,
          ladles: phase.ladles,
          expectedLadles: phase.expectedLadles,
          passiveHours: phase.passiveHours,
          jellyHours: phase.jellyHours,
          ladlesUsed: phase.ladlesUsed,
          passiveHoursUsed: phase.passiveHoursUsed,
          levelsBought: phase.levelsBought,
          saving: phase.saving
            ? {
                meal: phase.saving.meal,
                toLevel: phase.saving.toLevel,
                hours: phase.saving.hours,
                ladles: phase.saving.ladles,
                progress: phase.saving.progress,
                target: phase.saving.target,
                mapRefresh: phase.saving.mapRefresh,
              }
            : null,
          meals: phase.meals,
          layout: phase.layout
            ? {
                ranks: phase.layout.ranks,
                purchaseOrder: phase.layout.purchaseOrder,
                meals: phase.layout.meals,
                kitchens: phase.layout.kitchens,
              }
            : null,
        })),
      })),
    },
  });
}

// The rules the plan shares with the save: the lowest eligible NMLB meal.
const nmlbCases = [];
for (const candidate of [
  [0, 0, 2, 2, 3, 9],
  [0, 0, 0, 0, 2, 2],
  [4, 4, 4, 4, 4, 4],
  [1, 2, 3, 4, 5, 150],
  [2, 2, 2, 2, 2, 2],
]) {
  for (const [maxLevel, gain] of [
    [30, 1],
    [30, 3],
    [2, 3],
    [150, 3],
  ]) {
    nmlbCases.push({
      levels: candidate,
      maxLevel,
      gain,
      target: L.nmlbTarget(candidate, maxLevel),
      result: L.nmlbResultLevel(2, gain, maxLevel),
    });
  }
}

const payload = {
  mealNames: MealINFO.map((row) => String(row[0]).replace(/_/g, ' ')),
  costGrid,
  batchScenarios,
  planScenarios,
  nmlbCases,
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/stats/systems/w4/cooking-ladle.js',
      'js/stats/data/game/customlists.js',
    ],
  },
};
writeFileSync(
  new URL('../crates/ladle/tests/ladle-kitchen-oracle.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(
  `ladle kitchen: ${costGrid.length} costs, ${batchScenarios.length} batches, ${planScenarios.length} plans, ${nmlbCases.length} NMLB cases`,
);
