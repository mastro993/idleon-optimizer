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
};

writeFileSync(
  new URL('../fixtures/fountain.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(`wrote ${rows.length} income rows, ${stacks.length} stacks, ${cross.length} cross bonuses`);
