// Writes the upstream Fountain (W5) figures for the committed synthetic save, so
// `crates/core/tests/fountain.rs` compares the Rust port against the JS.
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

const payload = {
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: ['js/stats/systems/w5/fountain.js', 'JavaScript copied from the fountain-calc.html overview inline rates'],
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
};

writeFileSync(
  new URL('../fixtures/fountain.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(`wrote ${rows.length} income rows, ${stacks.length} stacks, ${cross.length} cross bonuses`);
