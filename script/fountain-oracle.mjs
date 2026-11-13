// Writes the upstream Fountain (W5) figures for the committed synthetic save, so
// `crates/fountain/tests/fountain.rs` compares the Rust port against the JS.
//
// Usage: node script/fountain-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/fountain-oracle.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));
const F = await import(join(source, 'js/stats/systems/w5/fountain.js'));
const { fmtVal } = await import(join(source, 'js/renderers/format.js'));

const save = JSON.parse(
  readFileSync(new URL('../fixtures/data.json', import.meta.url), 'utf8'),
);
// The committed fixture is a bare save with no `it.json` companion roster, so
// the arcade fountain-speed doubling reads as not owned on both sides.
loader.loadSaveData({ data: save });
const saveData = state.saveData;
const uLvs = F.upgLvs(saveData);
const mLvs = F.marbleLvs(saveData);
const desired = F.desiredCurrency(saveData);

const activeSpeed = F.activeSpeedMulti(uLvs, mLvs);
const fillTime = F.barFillTime(saveData, uLvs, mLvs);
const marbleFillTime = F.marbleBarFillTime();
const royalChance = F.royalChance(uLvs, mLvs);
const royalMulti = F.effectiveRoyalMulti(uLvs, mLvs);
const maxStack = F.maxStackSize(saveData, uLvs, mLvs);
const spaces = F.spacesOwned(uLvs, mLvs);
const sizeBonus = F._sizeMatterBonus(saveData);
const activeTypes = F.activeCurrencyTypes(saveData, uLvs, mLvs);
const fillsPerHrActive = 3600 / (fillTime / activeSpeed);
const fillsPerHrAfk = 3600 / fillTime;
const optimal = F.activeEarnRatesPerHr(saveData, uLvs, mLvs, desired, -1);

// The per-currency table the upstream overview computes inline.
const fullStackMulti = 1 + maxStack * sizeBonus / 100;
const singleMulti = 1 + 1 * sizeBonus / 100;
const rows = activeTypes.map((type) => {
  const value = F.currencyTotalValue(saveData, uLvs, mLvs, type, desired);
  const typeActiveFills = fillsPerHrActive / activeTypes.length;
  const typeAfkFills = fillsPerHrAfk / activeTypes.length;
  const royalWeighted = value * (1 - royalChance) + value * royalMulti * royalChance;
  const single = typeActiveFills * royalWeighted * singleMulti;
  const full = typeActiveFills * royalWeighted * fullStackMulti;
  return {
    type,
    value,
    single,
    singleAfk: typeAfkFills * royalWeighted * singleMulti,
    full,
    fullAfk: typeAfkFills * royalWeighted * fullStackMulti,
    optimal: optimal[type],
    optimalAfk: optimal[type] / activeSpeed,
  };
});
const total = (key) => rows.reduce((sum, row) => sum + row[key], 0);

const stacks = F.coinStacks(saveData).map((stack) => ({
  slot: stack.slot,
  count: stack.count,
  coins: stack.coins,
  royal: stack.royal,
  redeem: F.stackRedeemValue(saveData, uLvs, mLvs, stack),
}));

// Redeem value per currency: a mixed stack splits in proportion to the coin
// values the upstream overview tab uses.
const redeemByType = Array(9).fill(0);
for (const stack of F.coinStacks(saveData)) {
  const value = F.stackRedeemValue(saveData, uLvs, mLvs, stack);
  const counts = {};
  for (const coin of stack.coins) counts[coin] = (counts[coin] || 0) + 1;
  const keys = Object.keys(counts);
  if (keys.length === 1) {
    redeemByType[keys[0]] += value;
    continue;
  }
  const weights = keys.map(
    (key) => F.currencyTotalValue(saveData, uLvs, mLvs, Number(key), desired) * counts[key],
  );
  const weightTotal = weights.reduce((sum, weight) => sum + weight, 0);
  keys.forEach((key, index) => {
    redeemByType[key] += weightTotal > 0 ? value * weights[index] / weightTotal : 0;
  });
}

const cross = [
  ['Bravery Monument', F.bonTOT(uLvs, mLvs, 0, 13)],
  ['Justice Monument', F.bonTOT(uLvs, mLvs, 1, 13)],
  ['Wisdom Monument', F.bonTOT(uLvs, mLvs, 2, 13)],
  ['Cosmo EXP', F.bonTOT(uLvs, mLvs, 0, 14)],
  ['Golden Bucket', F.bonTOT(uLvs, mLvs, 0, 15)],
  ['Class EXP', F.bonTOT(uLvs, mLvs, 0, 16)],
  ['Sailing Treasure', F.bonTOT(uLvs, mLvs, 0, 17)],
  ['Orion Feathers', F.bonTOT(uLvs, mLvs, 0, 18)],
  ['All Damage', F.bonTOT(uLvs, mLvs, 1, 16)],
  ['Bolaia Study', F.bonTOT(uLvs, mLvs, 1, 14)],
  ['Harp EXP', F.bonTOT(uLvs, mLvs, 1, 15)],
  ['Summoning Essence', F.bonTOT(uLvs, mLvs, 1, 17)],
  ['Poppy Fish', F.bonTOT(uLvs, mLvs, 1, 18)],
  ["Minau's Costs", F.bonTOT(uLvs, mLvs, 2, 14)],
  ['Jar Enchant', F.bonTOT(uLvs, mLvs, 2, 15)],
  ['Research EXP', F.bonTOT(uLvs, mLvs, 2, 16)],
  ['Cooking Mastery', F.bonTOT(uLvs, mLvs, 2, 17)],
  ['Bubba Meat', F.bonTOT(uLvs, mLvs, 2, 18)],
];

const fLv = F.fountainLv(saveData);
const levels = [];
const cumulative = {};
for (let level = fLv; level < Math.min(fLv + 15, fLv + 50); level++) {
  const cost = F.opalCost(level);
  const type = F.fountLvCurrencyType(level);
  cumulative[type] = (cumulative[type] || 0) + cost;
  levels.push({
    level,
    cost,
    currency: type,
    cumulative: cumulative[type],
    affordable: F.fountCurrencyAvail(saveData, type) >= cumulative[type],
  });
}

// The page's own number, description and cap wording, transcribed from
// `fountain-calc.html` so the currency and upgrade tabs are compared as text.
function fmtNum(n) {
  if (n === 0) return '0';
  if (n === Math.floor(n) && Math.abs(n) < 1e3) return String(n);
  return fmtVal(n);
}

function formatUpgradeDescription(w, u) {
  const d = F.UPG_DATA[w][u];
  const bon = F.bonTOT(uLvs, mLvs, w, u);
  let desc = d[7]
    .replace(/\}x/g, fmtNum(1 + bon / 100) + 'x')
    .replace(/\+\{%/g, '+' + fmtNum(bon) + '%')
    .replace(/\+\{/g, '+' + fmtNum(bon));
  const key = w + '_' + u;
  let tokenValue = bon;
  if (key === '0_8') tokenValue = (F.currencyKeep(uLvs, mLvs) * 100).toFixed(1);
  else if (key === '0_10') tokenValue = F.spacesOwned(uLvs, mLvs);
  else if (key === '0_11') tokenValue = F.maxStackSize(saveData, uLvs, mLvs);
  else if (key === '0_12') tokenValue = F.activeSpeedMultiDisplay(uLvs, mLvs).toFixed(2);
  else if (key === '1_8') {
    let chance = F.royalChance(uLvs, mLvs);
    if (chance <= 0) {
      const preview = F.cloneUpgLvs(uLvs);
      preview[1][8] = 1;
      chance = F.royalChance(preview, mLvs);
      desc += ' Level 1 chance shown.';
    } else {
      desc += ' Current chance shown.';
    }
    tokenValue = fmtNum(1 / chance);
  } else if (key === '1_9') tokenValue = F.royalMulti(uLvs, mLvs).toFixed(2);
  else if (key === '1_10') tokenValue = fmtNum(F.marblePerFill(saveData, uLvs, mLvs));
  else if (key === '2_8') {
    let chance = F.luckyCoinBaseChance(uLvs, mLvs);
    if (chance <= 0) {
      const preview = F.cloneUpgLvs(uLvs);
      preview[2][8] = 1;
      chance = F.luckyCoinBaseChance(preview, mLvs);
      desc += ' Level 1 base chance shown; each lucky coin of that currency makes the next one 4x rarer.';
    } else {
      desc += ' Current base chance shown; each lucky coin of that currency makes the next one 4x rarer.';
    }
    tokenValue = fmtNum(1 / chance);
  } else if (key === '2_9') tokenValue = fmtNum(F.luckyCoinValuePer(uLvs, mLvs));
  else if (key === '2_12') {
    let chance = F.duckChance(saveData, uLvs, mLvs);
    if (chance <= 0) {
      const preview = F.cloneUpgLvs(uLvs);
      preview[2][12] = 1;
      chance = F.duckChance(saveData, preview, mLvs);
      desc += ' Level 1 chance shown; each owned duck makes the next one 5x rarer.';
    } else {
      desc += ' Current chance shown; each owned duck makes the next one 5x rarer.';
    }
    tokenValue = fmtNum(1 / chance);
  }
  return desc.replace(/\$/g, String(tokenValue));
}

function upgradeCap(w, u, bon) {
  const hints = [];
  if (w === 0 && u === 10) {
    const cur = F.spacesOwned(uLvs, mLvs);
    hints.push(cur >= 16
      ? { label: 'Capped', detail: 'Spaces maxed at 16' }
      : { label: 'Near cap', detail: cur + '/16 spaces' });
  }
  if (w === 0 && u === 11) {
    const cur = F.maxStackSize(saveData, uLvs, mLvs);
    hints.push(cur >= 50
      ? { label: 'Capped', detail: 'Stack size maxed at 50' }
      : { label: 'Near cap', detail: cur + '/50 stack' });
  }
  if (w === 0 && u === 12 && bon >= 1) {
    hints.push({
      label: 'Soft cap',
      detail: '4x term maxed. Extra levels give +' + F.UPG_DATA[w][u][6] / 100 + '/lv only',
    });
  }
  if (w === 0 && u === 0) {
    const owned = F.watersOwned(uLvs, mLvs);
    if (owned >= F.WATERS_IMPLEMENTED) {
      hints.push({ label: 'Max water', detail: owned + '/' + F.WATERS_IMPLEMENTED + ' waters' });
    }
  }
  return hints[0] || null;
}

// ----- Currency tab -----
const stackValueMultiplier =
  1 + F.maxStackSize(saveData, uLvs, mLvs) * F._sizeMatterBonus(saveData) / 100;
const ignoreOn = F.ignoreUnlocked(uLvs, mLvs);
const currencyRows = [];
for (let type = 0; type < 9; type++) {
  const unlocked = F.currencyUnlocked(uLvs, mLvs, type);
  const active = activeTypes.includes(type);
  const total = F.currencyTotalValue(saveData, uLvs, mLvs, type, desired);
  const fullStack = total * maxStack * stackValueMultiplier;
  currencyRows.push({
    type,
    base: F.currencyBaseValue(saveData, uLvs, mLvs, type),
    lucky: F.luckyCoinValue(saveData, uLvs, mLvs, type),
    total,
    fullStack,
    royalStack: fullStack * royalMulti,
    unlocked,
    ignored: F.currencyIgnored(saveData, type),
    active,
    create: unlocked ? (active ? 1 / activeTypes.length : 0) : 0,
  });
}

const log10 = (x) => Math.log(Math.max(x, 1)) / 2.30259;
const perWater = [3, 16, 20].map((index, water) => ({
  water,
  shared: F.currencyAllMulti(saveData, uLvs, mLvs),
  own: 1 + log10(Number((saveData.holesData[9] || [])[index]) || 0)
    * F.bonTOT(uLvs, mLvs, water, 19) / 100,
}));

const desireRows = [];
let bestDesire = -1;
let bestGround = 0;
for (let type = 0; type < 9; type++) {
  if (!F.currencyUnlocked(uLvs, mLvs, type)) continue;
  let sum = 0;
  for (const active of activeTypes) {
    sum += F.currencyTotalValue(saveData, uLvs, mLvs, active, type);
  }
  const average = activeTypes.length > 0 ? sum / activeTypes.length : 0;
  const weighted = average * (1 - royalChance) + average * royalMulti * royalChance;
  const ground = weighted * spaces * maxStack * stackValueMultiplier;
  desireRows.push({ type, average, weighted, ground, current: type === desired });
  if (ground > bestGround) {
    bestGround = ground;
    bestDesire = type;
  }
}
for (const row of desireRows) row.best = row.type === bestDesire;

const unlockRows = [
  [1, 0, 3], [2, 0, 4], [3, 1, 2], [4, 1, 3],
  [5, 1, 4], [6, 2, 2], [7, 2, 3], [8, 2, 4],
]
  .filter(([type]) => !F.currencyUnlocked(uLvs, mLvs, type))
  .map(([type, w, u]) => ({
    type,
    upgrade: F.UPG_DATA[w][u][0],
    water: w,
    level: uLvs[w][u] || 0,
    prerequisiteUnlocked: F.upgradeAccessible(uLvs, w, u),
  }));

// ----- Upgrades tab -----
const earnRates = F.earnRatesPerHr(saveData, uLvs, mLvs);
const upgradeRows = [];
for (let w = 0; w < 3; w++) {
  const water = [];
  for (let u = 0; u < 20; u++) {
    const d = F.UPG_DATA[w][u];
    const lv = uLvs[w][u] || 0;
    const ml = mLvs[w][u] || 0;
    const bon = F.bonTOT(uLvs, mLvs, w, u);
    const nextCost = F.upgCost(w, u, lv);
    const currency = d[3];
    const available = F.fountCurrencyAvail(saveData, currency);
    const remaining = Math.max(0, nextCost - available);
    const rate = earnRates[currency] || 0;
    const seconds = remaining <= 0 ? 0 : (rate > 0 ? remaining / rate * 3600 : null);
    water.push({
      level: lv,
      marble: ml,
      marbleBonus: F.marbleBon(ml),
      bonus: bon,
      unlocked: F.upgradeAccessible(uLvs, w, u),
      nextCost,
      currency,
      seconds,
      marbleCost: F.marbleCost(w, u, ml),
      marbable: F.canMarble(w, u),
      description: formatUpgradeDescription(w, u),
      cap: upgradeCap(w, u, bon),
      costs: Array.from({ length: 51 }, (_, step) => F.upgCost(w, u, lv + step)),
    });
  }
  upgradeRows.push(water);
}

// ----- Planner and optimizer -----
// `_renderPlanner` and `_renderOptimizer` compute their candidates in the page
// rather than in fountain.js, so the same loops are repeated here over the
// exported helpers, returning the rows the page would render.
function pctGainFrom(baseline, delta) {
  if (!(delta > 0)) return 0;
  return baseline > 0 ? (delta / baseline) * 100 : 100;
}

function planner(goal, options) {
  const affordableOnly = options.affordable ?? false;
  const normalize = options.normalize ?? false;
  const perHr = options.perHr ?? false;
  const desiredType = options.desired ?? desired;
  const baseline = F.measureGoal(saveData, goal, uLvs, mLvs, desiredType);
  const candidates = [];
  for (let w = 0; w < F.WATERS_IMPLEMENTED; w++) {
    for (let u = 0; u < 20; u++) {
      if (!F.upgradeAccessible(uLvs, w, u)) continue;
      const lv = uLvs[w][u] || 0;
      const cost = F.upgCost(w, u, lv);
      const type = F.UPG_DATA[w][u][3];
      const available = F.fountCurrencyAvail(saveData, type);
      const canAfford = available >= cost;
      if (affordableOnly && !canAfford) continue;
      const simLvs = F.cloneUpgLvs(uLvs);
      simLvs[w][u] = lv + 1;
      const newVal = F.measureGoal(saveData, goal, simLvs, mLvs, desiredType);
      const delta = newVal - baseline;
      if (delta <= 0) continue;
      const pctGain = pctGainFrom(baseline, delta);
      let efficiency;
      if (perHr) {
        const hrs = earnRates[type] > 0 ? cost / earnRates[type] : Infinity;
        efficiency = hrs > 0 && hrs < Infinity ? pctGain / hrs : 0;
      } else {
        let normCost = cost;
        if (normalize) {
          const cv = F.currencyTotalValue(saveData, uLvs, mLvs, type, type);
          normCost = cost / cv;
        }
        efficiency = normCost > 0 ? pctGain / normCost : 0;
      }
      const remaining = Math.max(0, cost - available);
      const tta = remaining <= 0 ? 0 : (earnRates[type] > 0 ? (remaining / earnRates[type]) * 3600 : null);
      const row = {
        water: w, upgrade: u, level: lv, cost, currency: type,
        affordable: canAfford, delta, pctGain, efficiency, seconds: tta,
      };
      if (goal === 'fill') {
        row.fillDelta = F.barFillTime(saveData, uLvs, mLvs) / F.activeSpeedMulti(uLvs, mLvs)
          - F.barFillTime(saveData, simLvs, mLvs) / F.activeSpeedMulti(simLvs, mLvs);
        row.marbleFillDelta = F.marbleBarFillTime() / F.activeSpeedMulti(uLvs, mLvs)
          - F.marbleBarFillTime() / F.activeSpeedMulti(simLvs, mLvs);
      }
      if (goal === 'royal') row.royalChance = F.royalChance(simLvs, mLvs);
      if (goal === 'duck') row.duckChance = F.duckChance(saveData, simLvs, mLvs);
      if (goal === 'optimal') {
        const startStacks = F.optimalStacksPerHr(saveData, uLvs, mLvs);
        const startValue = F.optimalStackValue(saveData, uLvs, mLvs, desiredType);
        row.speedPct = startStacks > 0 ? (F.optimalStacksPerHr(saveData, simLvs, mLvs) / startStacks - 1) * 100 : 0;
        row.valuePct = startValue > 0 ? (F.optimalStackValue(saveData, simLvs, mLvs, desiredType) / startValue - 1) * 100 : 0;
      }
      candidates.push(row);
    }
  }
  candidates.sort((a, b) => b.efficiency - a.efficiency);
  return { baseline, rows: candidates.slice(0, 8) };
}

const plannerGoals = [
  'currency', 'fill', 'bronze', 'silver', 'golden', 'dollar', 'credit',
  'treasury', 'royal', 'duck', 'lucky', 'marble-rate', 'optimal',
];
const planners = {};
for (const goal of plannerGoals) planners[goal] = planner(goal, {});

function optimizer(goal, options) {
  const affordable = options.affordable ?? true;
  const normalize = options.normalize ?? false;
  const perHr = options.perHr ?? false;
  const maxSteps = options.maxSteps ?? 50;
  const desiredType = options.desired ?? desired;
  const simLvs = F.cloneUpgLvs(uLvs);
  const budgets = {};
  for (let w = 0; w < F.WATERS_IMPLEMENTED; w++) {
    for (let u = 0; u < 20; u++) {
      const type = F.UPG_DATA[w][u][3];
      if (!(type in budgets)) budgets[type] = F.fountCurrencyAvail(saveData, type);
    }
  }
  const startValue = F.measureGoal(saveData, goal, uLvs, mLvs, desiredType);
  let baseline = F.measureGoal(saveData, goal, simLvs, mLvs, desiredType);
  const steps = [];
  let clock = 0;
  let optimizerDesired = desiredType;
  const active = F.activeCurrencyTypes(saveData, uLvs, mLvs);
  let optimizerFocus = F.ignoreUnlocked(uLvs, mLvs) && active.length === 1 ? active[0] : -1;
  for (let step = 0; step < maxSteps; step++) {
    const stepRates = F.earnRatesPerHr(saveData, simLvs, mLvs);
    let best = null;
    for (let w = 0; w < F.WATERS_IMPLEMENTED; w++) {
      for (let u = 0; u < 20; u++) {
        if (!F.upgradeAccessible(simLvs, w, u)) continue;
        const lv = simLvs[w][u] || 0;
        const cost = F.upgCost(w, u, lv);
        const type = F.UPG_DATA[w][u][3];
        if (affordable && (budgets[type] || 0) < cost) continue;
        if (!affordable && (budgets[type] || 0) < cost && !(stepRates[type] > 0)) continue;
        const tryLvs = F.cloneUpgLvs(simLvs);
        tryLvs[w][u] = lv + 1;
        const newVal = F.measureGoal(saveData, goal, tryLvs, mLvs, desiredType);
        const delta = newVal - baseline;
        if (delta <= 0) continue;
        const pctGain = pctGainFrom(baseline, delta);
        let eff;
        if (perHr) {
          const hrs = stepRates[type] > 0 ? cost / stepRates[type] : Infinity;
          eff = hrs > 0 && hrs < Infinity ? pctGain / hrs : 0;
        } else {
          let normCost = cost;
          if (normalize) {
            const cv = F.currencyTotalValue(saveData, simLvs, mLvs, type, type);
            normCost = cost / cv;
          }
          eff = normCost > 0 ? pctGain / normCost : 0;
        }
        if (best === null || eff > best.eff) best = { w, u, type, cost, delta, pctGain, eff };
      }
    }
    if (!best) break;
    if (!affordable && (budgets[best.type] || 0) < best.cost) {
      const canChangeDesired = F.bonTOT(simLvs, mLvs, 1, 11) > 0;
      const targetDesired = canChangeDesired ? best.type : optimizerDesired;
      const targetFocus = F.ignoreUnlocked(simLvs, mLvs) ? best.type : optimizerFocus;
      if (targetFocus !== optimizerFocus) {
        const flushSec = F.flushTime(saveData, simLvs, mLvs);
        const flushRates = F.activeEarnRatesPerHr(saveData, simLvs, mLvs, optimizerDesired, optimizerFocus);
        for (let t = 0; t < 9; t++) budgets[t] = (budgets[t] || 0) + flushRates[t] * flushSec / 3600;
        clock += flushSec;
      }
      optimizerDesired = targetDesired;
      optimizerFocus = targetFocus;
      const remainingCost = Math.max(0, best.cost - (budgets[best.type] || 0));
      const targetRates = F.activeEarnRatesPerHr(saveData, simLvs, mLvs, optimizerDesired, optimizerFocus);
      const waitSec = targetRates[best.type] > 0 ? remainingCost / targetRates[best.type] * 3600 : Infinity;
      if (!isFinite(waitSec)) break;
      for (let t = 0; t < 9; t++) budgets[t] = (budgets[t] || 0) + targetRates[t] * waitSec / 3600;
      clock += waitSec;
    }
    budgets[best.type] = Math.max(0, (budgets[best.type] || 0) - best.cost);
    simLvs[best.w][best.u] = (simLvs[best.w][best.u] || 0) + 1;
    baseline += best.delta;
    steps.push({
      water: best.w, upgrade: best.u, name: F.UPG_DATA[best.w][best.u][0],
      level: simLvs[best.w][best.u], cost: best.cost, currency: best.type,
      delta: best.delta, pctGain: best.pctGain, value: baseline, cumulative: clock,
    });
  }
  const groups = [];
  for (const step of steps) {
    const group = groups.find((g) => g.water === step.water && g.upgrade === step.upgrade);
    if (group) {
      group.count++;
      group.totalCost += step.cost;
      group.endLevel = step.level;
      group.cumulative = step.cumulative;
    } else {
      groups.push({
        water: step.water, upgrade: step.upgrade, name: step.name, count: 1,
        startLevel: step.level - 1, endLevel: step.level,
        totalCost: step.cost, currency: step.currency, cumulative: step.cumulative,
      });
    }
  }
  groups.sort((a, b) => a.cumulative - b.cumulative);
  return {
    goal,
    startValue,
    endValue: baseline,
    totalGainPct: pctGainFrom(startValue, baseline - startValue),
    steps,
    groups,
    remaining: budgets,
  };
}

const optimizers = {
  currency: optimizer('currency', { affordable: true, maxSteps: 50 }),
  royal: optimizer('royal', { affordable: true, maxSteps: 50 }),
  fillPerHr: optimizer('fill', { affordable: true, perHr: true, maxSteps: 25 }),
  optimalUnaffordable: optimizer('optimal', { affordable: false, maxSteps: 50 }),
};


// ----- Marble planner and optimizer -----
// `_renderMarblePlanner` and `_renderMarbleOptimizer` also compute their
// candidates in the page, so the loops are repeated here over the exported
// helpers, including the page's top-thirty cut.
function marblePlanner(goal, options) {
  const affordableOnly = options.affordable ?? false;
  const desiredType = options.desired ?? desired;
  const baseline = F.measureGoal(saveData, goal, uLvs, mLvs, desiredType);
  const marbleAvail = F.marbleCurrency(saveData);
  const baseFill = F.barFillTime(saveData, uLvs, mLvs) / F.activeSpeedMulti(uLvs, mLvs);
  const baseMarbleFill = F.marbleBarFillTime() / F.activeSpeedMulti(uLvs, mLvs);
  const baseDuckFill = F.duckBarUnlocked(uLvs, mLvs)
    ? F.duckBarFillTime() / F.activeSpeedMulti(uLvs, mLvs) : 0;
  const baseDuck = F.duckBarUnlocked(uLvs, mLvs) ? F.duckChance(saveData, uLvs, mLvs) : 0;
  const baseLucky = F.luckyCoinBaseChance(uLvs, mLvs);
  const marblePerHour = F.marblePerHr(saveData, uLvs, mLvs);
  const isOptimal = goal === 'optimal';
  const baseStacks = isOptimal ? F.optimalStacksPerHr(saveData, uLvs, mLvs) : 0;
  const baseStackVal = isOptimal ? F.optimalStackValue(saveData, uLvs, mLvs, desiredType) : 0;
  const candidates = [];
  for (let w = 0; w < F.WATERS_IMPLEMENTED; w++) {
    for (let u = 0; u < 20; u++) {
      const upgLv = uLvs[w][u] || 0;
      if (upgLv <= 0) continue;
      if (!F.canMarble(w, u)) continue;
      const mLv = mLvs[w][u] || 0;
      const cost = F.marbleCost(w, u, mLv);
      const canAfford = marbleAvail >= cost;
      if (affordableOnly && !canAfford) continue;
      const simMLvs = F.cloneMarbleLvs(mLvs);
      simMLvs[w][u] = mLv + 1;
      const newVal = F.measureGoal(saveData, goal, uLvs, simMLvs, desiredType);
      const delta = newVal - baseline;
      if (delta <= 0) continue;
      const pctGain = pctGainFrom(baseline, delta);
      const efficiency = cost > 0 ? pctGain / cost : 0;
      const row = {
        water: w, upgrade: u, name: F.UPG_DATA[w][u][0], marbleLevel: mLv,
        cost, affordable: canAfford, delta, pctGain, efficiency,
        seconds: marblePerHour > 0 && cost > marbleAvail
          ? (cost - marbleAvail) / marblePerHour * 3600 : (cost > marbleAvail ? null : 0),
        royalChance: F.royalChance(uLvs, simMLvs),
        duckChance: F.duckChance(saveData, uLvs, simMLvs),
        luckyDelta: F.luckyCoinBaseChance(uLvs, simMLvs) - baseLucky,
        speedPct: isOptimal && baseStacks > 0
          ? (F.optimalStacksPerHr(saveData, uLvs, simMLvs) / baseStacks - 1) * 100 : 0,
        valuePct: isOptimal && baseStackVal > 0
          ? (F.optimalStackValue(saveData, uLvs, simMLvs, desiredType) / baseStackVal - 1) * 100 : 0,
      };
      if (goal === 'fill') {
        row.fillDelta = baseFill
          - F.barFillTime(saveData, uLvs, simMLvs) / F.activeSpeedMulti(uLvs, simMLvs);
        row.marbleFillDelta = baseMarbleFill
          - F.marbleBarFillTime() / F.activeSpeedMulti(uLvs, simMLvs);
        if (baseDuckFill > 0) {
          row.duckFillDelta = baseDuckFill
            - F.duckBarFillTime() / F.activeSpeedMulti(uLvs, simMLvs);
        }
      }
      row.duckDelta = row.duckChance - baseDuck;
      row.royalDelta = row.royalChance - F.royalChance(uLvs, mLvs);
      candidates.push(row);
    }
  }
  candidates.sort((a, b) => b.efficiency - a.efficiency);
  return { baseline, rows: candidates.slice(0, 8) };
}

const marblePlanners = {};
for (const goal of plannerGoals) marblePlanners[goal] = marblePlanner(goal, {});

function marbleOptimizer(goal, options) {
  const affordable = options.affordable ?? true;
  const maxSteps = options.maxSteps ?? 50;
  const desiredType = options.desired ?? desired;
  const simMLvs = F.cloneMarbleLvs(mLvs);
  let budget = F.marbleCurrency(saveData);
  const startValue = F.measureGoal(saveData, goal, uLvs, mLvs, desiredType);
  let baseline = F.measureGoal(saveData, goal, uLvs, simMLvs, desiredType);
  const steps = [];
  let clock = 0;
  for (let step = 0; step < maxSteps; step++) {
    let best = null;
    for (let w = 0; w < F.WATERS_IMPLEMENTED; w++) {
      for (let u = 0; u < 20; u++) {
        if ((uLvs[w][u] || 0) <= 0) continue;
        if (!F.canMarble(w, u)) continue;
        const mLv = simMLvs[w][u] || 0;
        const cost = F.marbleCost(w, u, mLv);
        if (affordable && budget < cost) continue;
        if (!affordable && budget < cost && !(F.marblePerHr(saveData, uLvs, simMLvs) > 0)) continue;
        const tryMLvs = F.cloneMarbleLvs(simMLvs);
        tryMLvs[w][u] = mLv + 1;
        const newVal = F.measureGoal(saveData, goal, uLvs, tryMLvs, desiredType);
        const delta = newVal - baseline;
        if (delta <= 0) continue;
        const pctGain = pctGainFrom(baseline, delta);
        const efficiency = cost > 0 ? pctGain / cost : 0;
        if (best === null || efficiency > best.efficiency) {
          best = { w, u, name: F.UPG_DATA[w][u][0], cost, delta, pctGain, efficiency, newMLv: mLv + 1 };
        }
      }
    }
    if (best === null) break;
    if (!affordable && budget < best.cost) {
      const marbleRate = F.marblePerHr(saveData, uLvs, simMLvs);
      if (!(marbleRate > 0)) break;
      const wait = (best.cost - budget) / marbleRate * 3600;
      budget += marbleRate * wait / 3600;
      clock += wait;
    }
    budget = Math.max(0, budget - best.cost);
    simMLvs[best.w][best.u] = best.newMLv;
    baseline += best.delta;
    steps.push({
      water: best.w, upgrade: best.u, name: best.name, marbleLevel: best.newMLv,
      cost: best.cost, delta: best.delta, pctGain: best.pctGain,
      value: baseline, cumulative: clock,
    });
  }
  const groups = [];
  for (const step of steps) {
    const group = groups.find((g) => g.water === step.water && g.upgrade === step.upgrade);
    if (group) {
      group.count++;
      group.totalCost += step.cost;
      group.endLevel = step.marbleLevel;
      group.cumulative = step.cumulative;
    } else {
      groups.push({
        water: step.water, upgrade: step.upgrade, name: step.name, count: 1,
        startLevel: step.marbleLevel - 1, endLevel: step.marbleLevel,
        totalCost: step.cost, cumulative: step.cumulative,
      });
    }
  }
  groups.sort((a, b) => a.cumulative - b.cumulative);
  return {
    goal,
    startValue,
    endValue: baseline,
    totalGainPct: pctGainFrom(startValue, baseline - startValue),
    steps,
    groups,
    remaining: budget,
  };
}

const marbleOptimizers = {
  currency: marbleOptimizer('currency', { affordable: true, maxSteps: 50 }),
  optimal: marbleOptimizer('optimal', { affordable: true, maxSteps: 25 }),
  royalUnaffordable: marbleOptimizer('royal', { affordable: false, maxSteps: 25 }),
};

// ----- Simulation -----
// The page runs `runSim` in a module worker; the same exported function runs
// here on the fixture cavern, with the page's default 24-hour limit.
const { runSim } = await import(join(source, 'js/fountain-sim-worker.js'));
const simRun = runSim({
  saveData,
  uLvs,
  mLvs,
  desired,
  timeLimit: 24,
  marbleBarProgress: F.marbleBarProgress(saveData),
  currencies: Array.from({ length: 9 }, (_, type) => F.fountCurrencyAvail(saveData, type)),
  marbles: F.marbleCurrency(saveData),
});
const simEvents = simRun.events.map((event) => {
  const line = {
    time: event.time,
    type: event.type,
    marbleRate: event.marbleRate,
    baseCurrRate: event.baseCurrRate,
    currRates: event.currRates,
  };
  if (event.type === 'buy-upgrade') {
    Object.assign(line, {
      water: event.w, upgrade: event.u, name: event.name, cost: event.cost,
      currency: event.costType, level: event.newLv,
    });
  } else if (event.type === 'buy-marble') {
    Object.assign(line, {
      water: event.w, upgrade: event.u, name: event.name, cost: event.cost,
      marbleLevel: event.newLv,
    });
  } else if (event.type === 'switch') {
    line.desired = event.newDesired;
    line.focus = event.newFocus >= 0 ? event.newFocus : null;
  } else if (event.type === 'claim') {
    Object.assign(line, {
      currency: event.currType, amount: event.amount, seconds: event.duration,
    });
  } else if (event.type === 'marble-fill') {
    Object.assign(line, { marbles: event.marbles, total: event.totalMarbles });
  }
  return line;
});
const simulation = {
  hours: 24,
  startMarbleRate: simRun.startMarbleRate,
  endMarbleRate: simRun.endMarbleRate,
  startBaseCurr: simRun.startBaseCurr,
  endBaseCurr: simRun.endBaseCurr,
  startCurrRates: simRun.startCurrRates,
  endCurrRates: simRun.endCurrRates,
  clockSec: simRun.clockSec,
  stepCount: simRun.stepCount,
  currencies: simRun.currencies,
  marbles: simRun.marbles,
  marbleBarProgress: simRun.marbleBarProgress,
  uLvs: simRun.uLvs,
  mLvs: simRun.mLvs,
  eventCount: simEvents.length,
  // The 24-hour run buys thousands of cheap levels at the start, so the
  // fixture keeps a leading and trailing sample of the timeline instead of all
  // of it; the count and the final cavern pin the rest.
  eventSample: { leading: 40, afterStart: 40, trailing: 80 },
  events: [
    ...simEvents.slice(0, 40),
    ...simEvents.filter((event) => event.time > 0).slice(0, 40),
    ...simEvents.slice(-80),
  ],
};

const payload = {
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/stats/systems/w5/fountain.js',
      'JavaScript copied from the fountain-calc.html overview, currency and upgrade inline rows',
    ],
    save: 'fixtures/data.json wrapped as { data: save } with no companion roster',
  },
  summary: {
    fountainLevel: fLv,
    waterType: F.fountLvCurrencyType(fLv),
    nextLevelCost: F.opalCost(fLv),
    watersOwned: F.watersOwned(uLvs, mLvs),
    spaces,
    maxStack,
    maxCoins: F.maxCoins(saveData, uLvs, mLvs),
    marble: F.marbleCurrency(saveData),
    marbleBarUnlocked: F.marbleBarUnlocked(uLvs, mLvs),
    marblePerFill: F.marblePerFill(saveData, uLvs, mLvs),
    fillTime,
    marbleFillTime,
    activeSpeed,
    activeSpeedDisplay: F.activeSpeedMultiDisplay(uLvs, mLvs),
    barSpeed: F.barSpeed(saveData, uLvs, mLvs),
    totalMulti: F.currencyAllMulti(saveData, uLvs, mLvs),
    fountainMulti: F.currencyFountainMulti(uLvs, mLvs),
    externalMulti: F.currencyExternalMulti(saveData),
    royalChance,
    royalMulti,
    desireMulti: F.desireMulti(uLvs, mLvs),
    keep: F.currencyKeep(uLvs, mLvs),
    sizeBonus,
    optimalStacksPerHr: F.optimalStacksPerHr(saveData, uLvs, mLvs),
    royalStackTime: 3600 / F.optimalStacksPerHr(saveData, uLvs, mLvs),
    averageFillsPerRoyal: fillsPerHrActive / F.optimalStacksPerHr(saveData, uLvs, mLvs),
    flushTime: F.flushTime(saveData, uLvs, mLvs),
    stackFillTime: fillTime / activeSpeed * maxStack,
    allStacksFillTime: fillTime / activeSpeed * maxStack * spaces,
    afkStackFillTime: fillTime * maxStack,
    afkAllStacksFillTime: fillTime * maxStack * spaces,
    redeemTotal: F.totalRedeemValue(saveData, uLvs, mLvs),
    redeemByType,
  },
  income: {
    marblePerHr: F.marblePerHr(saveData, uLvs, mLvs),
    marblePerHrAfk: F.marblePerHr(saveData, uLvs, mLvs) / activeSpeed,
    fillsPerHrActive,
    fillsPerHrAfk,
    activeTypes,
    rows,
    totals: {
      single: total('single'),
      singleAfk: total('singleAfk'),
      full: total('full'),
      fullAfk: total('fullAfk'),
      optimal: total('optimal'),
      optimalAfk: total('optimalAfk'),
    },
  },
  lucky: {
    counts: Array.from({ length: 9 }, (_, type) => F.luckyCoinCount(saveData, type)),
    baseChance: F.luckyCoinBaseChance(uLvs, mLvs),
    valuePer: F.luckyCoinValuePer(uLvs, mLvs),
  },
  duck: {
    unlocked: F.duckBarUnlocked(uLvs, mLvs),
    count: F.duckCount(saveData),
    chance: F.duckChance(saveData, uLvs, mLvs),
    multi: F.duckMulti(saveData, uLvs, mLvs),
    progress: F.duckBarProgress(saveData),
    fillTime: F.duckBarFillTime(),
  },
  cross,
  stacks,
  levels,
  currency: {
    allMulti: F.currencyAllMulti(saveData, uLvs, mLvs),
    royalMulti,
    desireMulti: F.desireMulti(uLvs, mLvs),
    ignoreOn,
    stackSize: maxStack,
    stackValueMultiplier,
    rows: currencyRows,
    perWater,
    desired: desireRows,
    unlocks: unlockRows,
  },
  upgrades: {
    earnRates,
    waters: upgradeRows,
  },
  planners,
  optimizers,
  marblePlanners,
  marbleOptimizers,
  simulation,
};

writeFileSync(
  new URL('../fixtures/fountain.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(`wrote ${rows.length} income rows, ${stacks.length} stacks, ${cross.length} cross bonuses`);
