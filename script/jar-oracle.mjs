// Regenerates crates/core/tests/jar-oracle.json from the upstream jar formulas,
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
  enchantMulti: J.enchantMulti(S, ext),
  totalEnchantLevels: J.totalEnchantLevels(S),
  targetedEnchantProb: J.targetedEnchantProb(S),
  rupieJars,
};

writeFileSync(
  join(root, 'crates/core/tests/jar-oracle.json'),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(
  `wrote rupie value ${rupieValue.toFixed(4)}, ${chances.length} chances, ${rupieJars.length} rupie jars`,
);
