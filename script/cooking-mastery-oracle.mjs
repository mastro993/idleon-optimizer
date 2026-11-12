// Regenerates fixtures/cooking-mastery.json from the upstream Cooking Mastery
// page, so the Rust port is tested against the source of truth rather than
// hand-picked numbers.
//
// Usage: node script/cooking-mastery-oracle.mjs <path-to-idleon-research-optimizer>
//
// The page keeps the Mastery EXP maths inline, so this script rebuilds it from
// the page's own expressions while importing every external system from the
// upstream modules (grid bonus, arcade, salt lick, fountain, lab mainframe,
// vault, meritocracy, companions, menagerie). Change the page and this fixture
// has to move with it.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/cooking-mastery-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const load = (path) => import(join(source, path));
const { loadSaveData } = await load('js/save/loader.js');
const { saveData } = await load('js/state.js');
// A live binding, not a destructured copy: the loader fills these in, and the
// page reads whatever they hold by the time it renders.
const saveFields = await load('js/save/data.js');
const cauldronInfoData = saveFields.cauldronInfoData;
const { getLOG, formulaEval } = await load('js/formulas.js');
const { RandoListo2, AlchemyDescription } = await load('js/stats/data/game/customlists.js');
const { companions } = await load('js/stats/systems/common/companions.js');
const { arcadeBonus } = await load('js/stats/systems/w2/arcade.js');
const { computeSaltLick } = await load('js/stats/systems/w3/construction.js');
const { bonTOT, upgLvs, marbleLvs } = await load('js/stats/systems/w5/fountain.js');
const { superBitType } = await load('js/game-helpers.js');
const { gbWith } = await load('js/stats/systems/w7/research-math.js');
const { mainframeBonus } = await load('js/stats/systems/w4/lab.js');
const { vaultUpgBonus } = await load('js/stats/systems/common/vault.js');
const { computeMeritocBonusz } = await load('js/stats/systems/w7/meritoc.js');
const { menagerieBoostQTY } = await load('js/stats/systems/w7/menagerie.js');

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loadSaveData({ data: save, serverVars: {} });

// ===== cooking-mastery-calc.html, copied expression for expression =====
const PERLV = [];
for (let index = 0; index < 6; index++) PERLV.push(Number(RandoListo2[8][index]) || 0);
const UPG_NAMES = [
  'SALTY — Ladles Used',
  'SPICY — Account Cook LV',
  'SWEET — Divorce Cake',
  'SMOKY — Ribbon Rank+',
  'SOUR — Total Ribbons',
  'SAVORY — Mastery Rank',
];
const RANK_REQ = [0, 1, 5, 10, 25, 100];
const EXP_UPGRADES = [0, 1, 2, 4, 5];

const getCM = (row, index) =>
  (saveData.cookMasterData && Number(saveData.cookMasterData[row]?.[index])) || 0;
const getMasteryLevel = () => getCM(1, 0);
const getLadlesUsed = () => getCM(1, 3);
const getUpgLevel = (t) => getCM(2, t);
const getTotalCookingLv = () => {
  let total = 0;
  const lv0All = saveData.lv0AllData || [];
  for (let ci = 0; ci < saveFields.numCharacters; ci++) total += Number(lv0All[ci]?.[10]) || 0;
  return total;
};
const getMeal73Level = () => Number(saveData.mealsData?.[0]?.[73]) || 0;
const getTotalRibbonLv = () => {
  let sum = 0;
  for (let index = 28; index < (saveData.ribbonData || []).length; index++) {
    sum += Number(saveData.ribbonData[index]) || 0;
  }
  return sum;
};

function scalingFactor(t) {
  switch (t) {
    case 0: return getLOG(Math.max(1, getLadlesUsed()));
    case 1: return Math.max(0, getTotalCookingLv() - 1000);
    case 2: return Math.max(0, getMeal73Level() - 75);
    case 4: return getTotalRibbonLv();
    case 5: return getMasteryLevel() + 1;
    default: return 0;
  }
}
const bonusBase = (t, level) => PERLV[t] * (level ?? getUpgLevel(t));
const bonusTotal = (t, level) => bonusBase(t, level) * scalingFactor(t);
const expMultiplier = (t, level) => 1 + bonusTotal(t, level) / 100;

const safe = (fn) => {
  try {
    return fn() || 0;
  } catch {
    return 0;
  }
};

function computeExternalMulti() {
  const grid190 = safe(() =>
    gbWith(saveData.gridLevels, saveData.shapeOverlay || [], 190, {
      abm: saveData.allBonusMulti || 1,
    })
  );
  const superBit68 = safe(() => superBitType(68, saveData.gamingData?.[12]));
  const comp87 = safe(() => companions(87, saveData));
  const menagerie21 = safe(() => menagerieBoostQTY(saveData, 21));
  const fountainBonus = safe(() => bonTOT(upgLvs(saveData), marbleLvs(saveData), 2, 17));
  // `arcadeBonus` returns a breakdown node; the page's arithmetic coerces it.
  const arcade69 = safe(() => Number(arcadeBonus(69, saveData)) || 0);
  const saltLick10 = safe(() => Number(computeSaltLick(10, saveData)) || 0);

  let vialBonus = 0;
  let vialLevel = 0;
  for (const [index, row] of (AlchemyDescription[4] || []).entries()) {
    if (row[11] !== '7cookmastery') continue;
    vialLevel = Number((cauldronInfoData[4] || [])[index]) || 0;
    if (vialLevel <= 0) break;
    const base = formulaEval(row[3], Number(row[1]) || 0, Number(row[2]) || 0, vialLevel);
    const riftActive = Number(saveData.riftData?.[0]) > 34;
    let maxedVials = 0;
    if (riftActive) {
      for (const level of cauldronInfoData[4] || []) if ((Number(level) || 0) >= 13) maxedVials++;
    }
    const dNzz = (riftActive ? 2 * maxedVials : 0) + safe(() => vaultUpgBonus(42, saveData));
    const mf10 = safe(() => mainframeBonus(10, saveData));
    const meritoc20 = safe(() => computeMeritocBonusz(20, saveData));
    vialBonus = (mf10 === 2 ? 2 : 1) * (1 + dNzz / 100) * (1 + meritoc20 / 100) * base;
    break;
  }

  const additivePct = vialBonus + arcade69 + saltLick10;
  return {
    menagerie: menagerie21,
    grid190,
    superBit68,
    companion87: comp87,
    companion87Owned: saveData.companionIds ? saveData.companionIds.has(87) : false,
    fountain: fountainBonus,
    vial: vialBonus,
    vialLevel,
    arcade69,
    saltLick10,
    additivePct,
    total: (1 + menagerie21 / 100) * (1 + grid190 / 100) * (1 + 40 * superBit68 / 100)
      * (1 + 2 * comp87) * (1 + fountainBonus / 100) * (1 + additivePct / 100),
  };
}

const external = computeExternalMulti();
const computeExpRate = (overrides) => {
  let rate = 2;
  for (const t of EXP_UPGRADES) {
    rate *= expMultiplier(t, overrides?.[t] ?? getUpgLevel(t));
  }
  return rate * external.total;
};
const expReq = (level) => 100 * Math.pow(2.5, level) * Math.pow(5, Math.max(0, level - 40));

const currentRate = computeExpRate();
const masteryLv = getMasteryLevel();
const spending = (saveData.cookMasterData?.[2] || []).reduce((sum, level) => sum + (Number(level) || 0), 0);
const companionPoints = external.companion87Owned ? 5 : 0;

const upgrades = EXP_UPGRADES.map((t) => {
  const curLv = getUpgLevel(t);
  const locked = masteryLv < (RANK_REQ[t] || 0);
  const newRate = locked ? currentRate : computeExpRate({ [t]: curLv + 1 });
  const gain = newRate - currentRate;
  return {
    t,
    name: UPG_NAMES[t],
    curLv,
    locked,
    reqLv: RANK_REQ[t],
    perLv: PERLV[t],
    scalingFactor: scalingFactor(t),
    curBonus: bonusTotal(t),
    newBonus: bonusTotal(t, curLv + 1),
    gain,
    pctGain: currentRate > 0 ? (gain / currentRate) * 100 : 0,
    newRate,
  };
});
upgrades.sort((a, b) => b.pctGain - a.pctGain || a.t - b.t);

// ===== cooking-mastery-calc.html, optimizeDistribution, copied expression
// for expression: the page's own greedy, so the Rust plan is compared against
// the algorithm and not a re-derivation of it.
function optimizeDistribution(totalPoints) {
  const levels = {};
  for (const t of EXP_UPGRADES) levels[t] = 0;

  const mLv = getMasteryLevel();
  for (let p = 0; p < totalPoints; p++) {
    let bestT = -1;
    let bestRatio = 0;
    for (const t of EXP_UPGRADES) {
      if (mLv < (RANK_REQ[t] || 0)) continue;
      const curMulti = expMultiplier(t, levels[t]);
      const newMulti = expMultiplier(t, levels[t] + 1);
      const ratio = newMulti / curMulti;
      if (ratio > bestRatio) { bestRatio = ratio; bestT = t; }
    }
    if (bestT >= 0) levels[bestT]++;
  }

  let optRate = 2 * external.total;
  for (const t of EXP_UPGRADES) optRate *= expMultiplier(t, levels[t]);
  return { levels, rate: optRate };
}

const totalPts = masteryLv + 1 + companionPoints;const budgets = [totalPts, Math.floor(totalPts / 2), 0, totalPts + 5, -3];
const distribution = {
  totalPts,
  variants: budgets.map((request) => {
    const spendPts = Math.min(totalPts, Math.max(0, request));
    const optimal = optimizeDistribution(spendPts);
    return {
      request,
      spendPts,
      levels: EXP_UPGRADES.map((t) => optimal.levels[t]),
      rate: optimal.rate,
      hoursToNextLevel: optimal.rate > 0 ? expReq(masteryLv) / optimal.rate : null,
    };
  }),
  rows: EXP_UPGRADES.map((t) => {
    const curLv = getUpgLevel(t);
    const optLv = optimizeDistribution(totalPts).levels[t];
    return { t, curLv, optLv, diff: optLv - curLv, optBonus: bonusTotal(t, optLv) };
  }),
};

// ===== the yellow point allocator, from meal-impact.js itself =====
// The page's second tab calls these functions directly, so the fixture is the
// upstream optimizer's own output rather than a re-derivation of it.
const {
  GOALS,
  bonusMultiCook,
  farmingMealCookingScale,
  kitchenCookingAggregate,
  optimizeForGoal,
  summoningCropEvolutionScale,
  yellowPointBudget,
} = await load('js/stats/systems/common/meal-impact.js');
const { cookingMealMulti } = await load('js/stats/systems/common/cooking.js');
const { ribbonBonusAt } = await load('js/game-helpers.js');
const { jellyRewardBonus } = await load('js/stats/data/w7/jelly-operator.js');

function yellowFixture() {
  const budget = yellowPointBudget(saveData);
  // The page only offers a goal with two leveled meals to compare; the two
  // characters in the fixture give the stat goals a different scaling step.
  const offered = GOALS.filter((goal) => {
    const preview = optimizeForGoal(goal.id, 0, saveData, { activeCharIdx: 0 });
    return preview && preview.meals.length > 1;
  });
  const plan = (goal, request, charIdx) => {
    // `renderMealAlloc` clamps the input before it plans: `Math.min(budget,
    // Math.max(0, override))`. A non-finite entry never reaches upstream, so the
    // fixture keeps the three requests a save or a document can hold.
    const spend = Math.min(budget, Math.max(0, request));
    const result = optimizeForGoal(goal.id, spend, saveData, { activeCharIdx: charIdx });
    return {
      goal: goal.id,
      charIdx,
      request,
      spend,
      goalValue: result.goalValue,
      curGoalValue: result.curGoalValue,
      meals: result.meals.map((meal) => ({
        idx: meal.idx,
        name: meal.name,
        cat: meal.cat,
        mealLv: meal.mealLv,
        base: meal.base,
        baseWeight: meal.baseWeight,
        curLv: meal.curLv,
        optLv: meal.optLv,
      })),
    };
  };
  const charCount = Math.max(
    (saveData.charNames && saveData.charNames.length) || 0,
    (saveData.lv0AllData && saveData.lv0AllData.length) || 0,
  );
  const characters = Array.from({ length: charCount }, (_, index) => ({
    name: (saveData.charNames && saveData.charNames[index]) || `Character ${index + 1}`,
    farmSteps: farmingMealCookingScale(saveData, index).steps,
    summonSteps: summoningCropEvolutionScale(saveData, index).steps,
    farmingLv: farmingMealCookingScale(saveData, index).farmingLv,
    summoningLv: summoningCropEvolutionScale(saveData, index).summoningLv,
  }));
  const mealKitchens = kitchenCookingAggregate(saveData, 'meal');
  const recipeKitchens = kitchenCookingAggregate(saveData, 'recipe');
  return {
    budget,
    cookMulti: cookingMealMulti(saveData).val,
    mealMulti: bonusMultiCook(20),
    // Each meal's own ribbon factor, the other half of its base weight.
    ribbons: Array.from({ length: 74 }, (_, meal) =>
      ribbonBonusAt(28 + meal, saveData.ribbonData || [],
        String((saveData.olaData && saveData.olaData[379]) || ''), saveData.weeklyBossData,
        undefined, jellyRewardBonus(saveData, 60))),
    characters,
    kitchens: {
      count: mealKitchens.kitchenCount,
      averageLevels: mealKitchens.averageUpgradeLevels,
      minSteps: mealKitchens.minSteps,
      maxSteps: mealKitchens.maxSteps,
      mealSteps: mealKitchens.weightedSteps,
      recipeSteps: recipeKitchens.weightedSteps,
    },
    goals: offered.map((goal) => ({
      id: goal.id,
      name: goal.name,
      categories: goal.categories,
      unit: goal.unit,
      note: goal.note,
      // The full budget, then a smaller one so the greedy's split shows, then
      // the requests the page clamps: a negative one and one above the budget.
      plans: [budget, 7, -3, budget + 6].map((spend) => plan(goal, spend, 0)),
      characterPlans: ['mcook', 'cropevo'].includes(goal.id)
        ? characters.map((_, index) => plan(goal, budget, index))
        : [],
    })),
  };
}

const payload = {
  source: 'fixtures/data.json through the upstream js/save/loader.js',
  page: 'cooking-mastery-calc.html',
  perLv: PERLV,
  rankReq: RANK_REQ,
  masteryLevel: masteryLv,
  ladlesUsed: getLadlesUsed(),
  totalCookingLevel: getTotalCookingLv(),
  meal73Level: getMeal73Level(),
  totalRibbonLevel: getTotalRibbonLv(),
  pointsSpent: spending,
  pointsFromCompanion: companionPoints,
  pointsAvailable: Math.max(0, masteryLv + 1 + companionPoints - spending),
  expRequired: expReq(masteryLv),
  expRate: currentRate,
  hoursToNextLevel: currentRate > 0 ? expReq(masteryLv) / currentRate : null,
  external,
  upgrades,
  distribution,
  yellow: yellowFixture(),
};

writeFileSync(
  join(root, 'fixtures/cooking-mastery.json'),
  `${JSON.stringify(payload, null, 2)}\n`
);
console.log(
  `wrote fixtures/cooking-mastery.json: ${payload.expRate.toFixed(2)} EXP/hr, ` +
    `${upgrades.length} upgrades ranked`
);
