// Regenerates crates/jar/tests/jar-oracle.json from the upstream jar formulas,
// so the Rust port is tested against the source of truth rather than hand-picked
// numbers. Usage: node script/jar-oracle.mjs <path-to-idleon-research-optimizer>
//
// The rust test feeds the two external bonuses this app does not decode yet
// (the Cavern Resource stamp and the farming Exotic 52) from the oracle's `ext`,
// so the formulas themselves are compared end to end.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/jar-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const J = await import(join(source, 'js/stats/systems/w5/jar.js'));
const hole = await import(join(source, 'js/stats/systems/w5/hole.js'));
const fountain = await import(join(source, 'js/stats/systems/w5/fountain.js'));
const spelunking = await import(join(source, 'js/stats/systems/w7/spelunking.js'));
const farming = await import(join(source, 'js/stats/systems/w6/farming.js'));
const stamp = await import(join(source, 'js/stats/systems/w1/stamp.js'));
const helpers = await import(join(source, 'js/game-helpers.js'));
const saveData = await import(join(source, 'js/save/data.js'));

const raw = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
const decode = (value) =>
  typeof value === 'string'
    ? (() => {
        try {
          return JSON.parse(value);
        } catch {
          return value;
        }
      })()
    : value;

// The decoded save, under the names the upstream jar module reads.
const characters = Object.keys(raw)
  .filter((key) => /^Lv0_\d+$/.test(key))
  .sort((a, b) => Number(a.slice(4)) - Number(b.slice(4)))
  .map((key) => decode(raw[key]));
const S = {
  holesData: decode(raw.Holes),
  olaData: decode(raw.OptLacc),
  spelunkData: decode(raw.Spelunk),
  gamingData: decode(raw.Gaming),
  farmUpgData: decode(raw.FarmUpg),
  tasksGlobalData: decode(raw.TasksGlobalData),
  lv0AllData: characters,
};
saveData.assignSaveData({ stampLvData: decode(raw.StampLv) });

const h = (row) => S.holesData[row] || [];
const round = (value) => Math.round(Number(value) || 0);
const water = [[], [], []];
const marbles = [[], [], []];
for (let w = 0; w < 3; w++) {
  for (let u = 0; u < 20; u++) {
    water[w][u] = Number(h(31)[w]?.[u]) || 0;
    marbles[w][u] = Number(h(32)[w]?.[u]) || 0;
  }
}
const fountMonument = [0, 1, 2].map((w) => fountain.bonTOT(water, marbles, w, 13));
const safe = (fn) => {
  try {
    const value = fn();
    return typeof value === 'object' && value !== null ? Number(value.val) || 0 : Number(value) || 0;
  } catch {
    return 0;
  }
};

// `_ext()` from jar-calc.html, with the same reads.
const ext = {
  legendPts29: round(spelunking.legendPTSbonus(29, S)),
  ola355: Number(S.olaData[355]) || 0,
  monument2_1: safe(() => hole.computeMonumentROGbonus(2, 1, S, { fountMonument })),
  bell4: (Number(h(17)[4]) || 0) * 0.5,
  cosmo0_3: Math.floor(35 * (Number(h(4)[3]) || 0)),
  stampCavernRes: safe(() => stamp.computeStampBonusOfTypeX('CavernRes', S)),
  cglunko14: (Number(S.olaData[644]) || 0) * 10,
  bolaia10: (Number(h(26)[10]) || 0) * 25,
  paletteBonus2: safe(() => spelunking.computePaletteBonus(2, S)),
  exoticBonus52: safe(() => farming.computeExoticBonus(52, S)),
  spelunk6Len: (S.spelunkData[6] && S.spelunkData[6].length) || 0,
  superBit32: helpers.superBitType(32, S.gamingData?.[12]),
  fountBonTOT2_15: fountain.bonTOT(water, marbles, 2, 15),
};

const rupieValue = J.rupieValue(S, ext);
const avgRupies = J.avgRupiesPerBreak(S, ext);
const opal = J.opalChance(S, ext);
const collectible = J.collectibleChance(S, ext);
const enchant = J.enchantChance(S, ext);
const tiers = (base) => Array.from({ length: 6 }, (_, tier) => base * Math.pow(10, tier));
const tierFor100 = (base) => (base > 0 ? Math.ceil(Math.log10(1 / base)) + 1 : null);

// The rupie jars and their active colours, as `_renderRupieValue` builds them.
const rupieJars = [];
for (let jarType = 0; jarType < J.JAR_TYPE_NAMES.length; jarType++) {
  if (J.JAR_TYPE_CATS[jarType] !== 'rupie') continue;
  const [min, max] = J.rupieTypeRange(S, jarType);
  const colors = [];
  for (let color = min; color <= max; color++) colors.push(color);
  rupieJars.push({
    jarType,
    name: J.JAR_TYPE_NAMES[jarType],
    colors,
    perBreak: tiers(avgRupies * rupieValue),
    perColor: colors.map((color) => ({
      color,
      name: J.RUPIE_NAMES[color],
      perBreak: tiers((avgRupies * rupieValue) / colors.length),
    })),
  });
}

const chances = [
  { name: 'opal', jarType: 1, base: opal },
  { name: 'collectible', jarType: 2, base: collectible },
  { name: 'enchant', jarType: 4, base: enchant },
].map((chance) => ({
  ...chance,
  tiers: tiers(chance.base),
  tierFor100: tierFor100(chance.base),
  expectedBreaks: J.expectedBreaksToEvent(chance.base),
  expectedHours: J.expectedHoursToEvent(S, ext, chance.jarType, chance.base),
}));

// `_collectibleEffect` from jar-calc.html: fill the effect template with the
// bonus the collectible grants at its saved level.
const collectibleEffect = (idx, bonus) => {
  const desc = J.COLLECTIBLE_DATA[idx][3];
  if (desc.includes('Future cavern')) return desc;
  if (desc.includes('}x')) return desc.replace('}x', `${(1 + bonus / 100).toFixed(2)}x`);
  return desc.replace('{%', `${bonus.toFixed(1)}%`);
};

const collectibles = J.COLLECTIBLE_DATA.map(([name, perLevel, category, effect], idx) => ({
  idx,
  name,
  perLevel,
  category,
  categoryLabel: J.CATEGORY_LABELS[category],
  categoryColor: J.CATEGORY_COLORS[category],
  effect,
  level: J.collectibleLevel(S, idx),
  bonus: J.collectibleBonus(S, idx, ext.legendPts29),
  enchantLevels: Math.max(0, J.collectibleLevel(S, idx) - 1),
  atLevel: collectibleEffect(idx, perLevel),
  currentEffect: collectibleEffect(idx, J.collectibleBonus(S, idx, ext.legendPts29)),
}));

const enchantPriority = J.enchantPriority(S, ext).map((row) => ({
  idx: row.idx,
  name: row.name,
  category: row.category,
  categoryLabel: row.categoryLabel,
  level: row.level,
  enchantLevels: row.enchantLevels,
  totalBonus: row.totalBonus,
  marginalValue: row.marginalValue,
}));

// The schematic rows in the upstream display order.
const schematics = [];
for (const [group, indices] of Object.entries(J.B_UPG_GROUPS)) {
  for (const index of indices) {
    const row = J.B_UPG_DATA.find((entry) => entry[0] === index);
    if (!row) continue;
    const [, name, effect, description, currency, cost] = row;
    schematics.push({
      index,
      group,
      name,
      effect,
      description,
      currency,
      cost,
      owned: J.bupgOwned(S, index) === 1,
    });
  }
}

// The jar-type purchase plan: each type is paid with the rupie below it, and
// only the next type can be bought.
const ownedTypes = J.jarTypesOwned(S);
const jarTypeUpgrades = Array.from({ length: 9 }, (_, index) => {
  const cost = 50 * Math.pow(1 + index, 1.5) * Math.pow(4.8, index);
  const balance = J.rupieBalance(S, index);
  const status =
    index < ownedTypes
      ? 'owned'
      : index === ownedTypes
        ? balance >= cost
          ? 'affordable'
          : 'shortfall'
        : 'locked';
  return {
    jarType: index + 1,
    name: J.JAR_TYPE_NAMES[index + 1],
    cost,
    currency: index,
    currencyName: J.RUPIE_NAMES[index],
    balance,
    shortfall: Math.max(0, cost - balance),
    status,
  };
});

// `_renderNextTier` milestone ranking.
const productionPerHour = J.productionPerHR(S, ext);
const total = J.totalJarTypes(S);
const milestones = [];
for (let jarType = 0; jarType < total; jarType++) {
  const count = J.jarsProducedOfType(S, jarType);
  const logCount = count > 0 ? Math.floor(Math.log10(count)) : -1;
  const next = Math.pow(10, logCount + 1);
  const deficit = next - count;
  const requirement = J.productionREQ(S, jarType);
  const jarsPerHr = productionPerHour / requirement;
  milestones.push({
    jarType,
    name: J.JAR_TYPE_NAMES[jarType],
    count,
    next,
    deficit,
    hours: deficit / (jarsPerHr * 3),
    benefits: jarType + 1 < total ? J.JAR_TYPE_NAMES[jarType + 1] : null,
    lineNow: 5 * Math.log10(Math.max(1, count)),
    lineNext: 5 * (logCount + 2),
    requirement,
  });
}
milestones.sort((left, right) => left.hours - right.hours);

// `_renderNextTier`'s off-map accumulation, from `jar-calc.html`.
const perTick = productionPerHour * 5 / 3600;
const machSeven = 1 + (J.bupgOwned(S, 67) ? 30 : 0) / 100;

const simMaxTier = (tick, requirement) => {
  if (tick <= requirement) return 0;
  let progress = 0;
  let best = 0;
  for (let t = 0; t < 500; t++) {
    progress += tick;
    if (progress >= requirement) {
      const tier = Math.floor(Math.log10(progress / requirement));
      if (tier > best) best = tier;
      progress -= requirement * Math.pow(10, tier);
      if (progress < 0) progress = 0;
    }
  }
  return best;
};

const simTierStats = (tick, requirement) => {
  if (tick <= requirement) return { sustained: -1, ticksBetweenMax: Infinity };
  const sustained = Math.floor(Math.log10(tick / requirement));
  const maxTier = simMaxTier(tick, requirement);
  let progress = 0;
  let firstHit = -1;
  let hits = 0;
  for (let t = 0; t < 1000; t++) {
    progress += tick;
    if (progress >= requirement) {
      const tier = Math.floor(Math.log10(progress / requirement));
      if (tier >= maxTier && maxTier > sustained) {
        hits++;
        if (firstHit < 0) firstHit = t + 1;
      }
      progress -= requirement * Math.pow(10, tier);
      if (progress < 0) progress = 0;
    }
  }
  const avgInterval = hits > 1 ? Math.ceil(1000 / hits) : firstHit > 0 ? firstHit : Infinity;
  return { sustained, maxTier, ticksBetweenMax: avgInterval };
};

const tierProjections = [];
for (let jarType = 0; jarType < total; jarType++) {
  const requirement = J.productionREQ(S, jarType);
  const baseRequirement = 1000 + 2000 * jarType;
  const previousCount = J.jarsProducedOfType(S, Math.max(0, jarType - 1));
  const stats = simTierStats(perTick, requirement);
  const computedTier = simMaxTier(perTick, requirement);
  const sustained = stats.sustained;
  const netSustained = sustained >= 0 ? perTick - requirement * Math.pow(10, sustained) : perTick - requirement;
  const nextTier = sustained + 1;
  const netNext = perTick - requirement * Math.pow(10, nextTier);
  const secsBetween =
    perTick > requirement && Number.isFinite(stats.ticksBetweenMax)
      ? stats.ticksBetweenMax * 5
      : Math.ceil(requirement / perTick) * 5;
  const sustainNeeded = perTick / Math.pow(10, nextTier);
  const lineMultiplier = baseRequirement / (sustainNeeded * machSeven);
  const linePercent = (lineMultiplier - 1) * 100;
  tierProjections.push({
    jarType,
    name: J.JAR_TYPE_NAMES[jarType],
    requirement,
    baseRequirement,
    sustained,
    maxTier: computedTier,
    ticksBetweenMax: stats.ticksBetweenMax,
    secsBetween,
    netSustained,
    nextTier,
    netNext,
    neededLine: linePercent > 0 ? Math.pow(10, linePercent / 5) : 1,
    neededProduction: requirement * Math.pow(10, nextTier) * 3600 / 5,
    previousCount,
  });
}

const payload = {
  source: 'fixtures/data.json read through the upstream jar formulas',
  ext,
  rupieValue,
  rupieValueBreakdown: J.rupieValueBreakdown(S, ext),
  rupieValueByTier: Array.from({ length: 5 }, (_, tier) => rupieValue * Math.pow(10, tier)),
  extraRupieChance: J.extraRupieChance(S, ext),
  avgRupiesPerBreak: avgRupies,
  productionPerHour: J.productionPerHR(S, ext),
  productionBreakdown: J.productionPerHRBreakdown(S, ext),
  productionRequirement: J.JAR_TYPE_NAMES.map((_, jarType) => J.productionREQ(S, jarType)),
  timeToTier: J.JAR_TYPE_NAMES.map((_, jarType) =>
    Array.from({ length: 6 }, (_, tier) => J.timeToTier(S, jarType, tier, ext)),
  ),
  chances,
  collectibles,
  enchantPriority,
  schematics,
  jarTypeUpgrades,
  milestoneRanking: milestones,
  perTick,
  tierProjections,
  enchantMulti: J.enchantMulti(S, ext),
  totalEnchantLevels: J.totalEnchantLevels(S),
  targetedEnchantProb: J.targetedEnchantProb(S),
  rupieJars,
};

writeFileSync(
  join(root, 'crates/jar/tests/jar-oracle.json'),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(
  `wrote rupie value ${rupieValue.toFixed(4)}, ${chances.length} chances, ${rupieJars.length} rupie jars, ${collectibles.length} collectibles, ${schematics.length} schematics, ${milestones.length} milestones`,
);
