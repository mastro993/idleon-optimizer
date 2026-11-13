// Regenerates crates/sushi/tests/fixtures/upstream.json from the upstream
// Sushi Station calculator, so the Rust port is tested against the source of
// truth rather than hand-picked numbers.
// Usage: node script/sushi-oracle.mjs <path-to-idleon-research-optimizer>
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/sushi-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
const loader = await import(join(source, 'js/save/loader.js'));
loader.loadSaveData(save);
const { saveData } = await import(join(source, 'js/state.js'));
const gbWith = (await import(join(source, 'js/sim-math.js'))).gbWith;
const { MINEHEAD_BONUS_QTY } = await import(join(source, 'js/stats/data/w7/minehead.js'));
const { arcadeBonus } = await import(join(source, 'js/stats/systems/w2/arcade.js'));
const { superBitType, eventShopOwned } = await import(join(source, 'js/game-helpers.js'));
const { jellyRewardBonus } = await import(
  join(source, 'js/stats/data/w7/jelly-operator.js')
);
const { computeButtonBonus } = await import(join(source, 'js/stats/systems/w7/minehead.js'));
const systems = await import(join(source, 'js/stats/systems/w7/sushi.js'));
const data = await import(join(source, 'js/stats/data/w7/sushi.js'));

// `_getExternalSources` from `js/ui/sushi-ui.js`, verbatim: the tab's own
// account reads, so the fixture is the calculator's output rather than a
// second decoder written for this script.
const _gbCtx = { abm: saveData.allBonusMulti || 1 };
const _gb = (idx) => gbWith(saveData.gridLevels, saveData.shapeOverlay, idx, _gbCtx);
const mineFloor = saveData.stateR7?.[4] || 0;
const ext = {
  gridBonus189: _gb(189),
  gridBonus188: _gb(188),
  // `arcadeBonus` returns a tree node whose `valueOf` is the number upstream
  // divides; keep the number so the fixture is JSON-native.
  arcade67: Number(arcadeBonus(67, saveData)),
  mineheadBonus11: mineFloor > 11 ? MINEHEAD_BONUS_QTY[11] || 0 : 0,
  atom14: Number(saveData.atomsData?.[14]) || 0,
  sailing39: Number(saveData.sailingData?.[3]?.[39]) || 0,
  hasBundleV: saveData.bundlesData?.bon_v ? true : false,
  gamingSuperBit67: superBitType(67, saveData.gamingData?.[12]),
  buttonBonus2: computeButtonBonus(2, saveData),
  doublers: systems.sushiDoublers(saveData),
  eventShopStr: saveData.cachedEventShopStr,
  eventShop66: eventShopOwned(66, saveData.cachedEventShopStr),
  jelly18: jellyRewardBonus(saveData, 18),
};

const sd = saveData.sushiData || [];
const ul = sd?.[2] || [];
const us = saveData.cachedUniqueSushi || 0;
const kt = systems.knowledgeBonusTotals(sd);
const fpBase = systems.fireplaceEffectBase(kt, Number(sd?.[4]?.[2]) || 0);
const orangeFire = systems.computeOrangeFireSum(sd, fpBase);
const slots = data.MAX_SLOTS;
const tiers = data.MAX_TIER;
const upgrades = data.SUSHI_UPG.length;
const slotsToUpg = data.SLOT_TO_UPG.length;

const formulas = {
  source: 'fixtures/data.json Sushi rows and the account terms the tab reads',
  researchLevel: saveData.researchLevel,
  uniqueSushi: us,
  slots,
  tiers,
  ext,
  doublers: ext.doublers,
  upgLvReq: Array.from({ length: slotsToUpg }, (_, t) => systems.upgLvReq(t)),
  upgradeQTY: Array.from({ length: upgrades }, (_, i) => systems.upgradeQTY(i, ul, ext.doublers)),
  upgradeQTYNoDoublers: Array.from({ length: upgrades }, (_, i) =>
    systems.upgradeQTY(i, ul)
  ),
  upgCost: Array.from({ length: slotsToUpg }, (_, slot) =>
    systems.upgCost(slot, ul, kt, saveData)
  ),
  currencyPerTier: Array.from({ length: 70 }, (_, tier) => systems.currencyPerTier(tier)),
  fuelCostPerTier: Array.from({ length: 70 }, (_, tier) => systems.fuelCostPerTier(tier)),
  knowledgeBonusSpecific: Array.from({ length: tiers + 1 }, (_, tier) =>
    systems.knowledgeBonusSpecific(tier, sd)
  ),
  knowledgeTotals: kt,
  // `knNext` from `_renderSushi`: the bonus one more level of the tier pays.
  knowledgeBonusNext: Array.from({ length: tiers + 1 }, (_, tier) => {
    const cat = data.TIER_TO_KNOWLEDGE_CAT[tier];
    const baseVal = data.KNOWLEDGE_CAT_VALUE[cat] || 0;
    const discoveryMult = Math.min(2, 1 + (Number(sd?.[5]?.[tier]) || 0));
    return baseVal * discoveryMult * (1 + tier / 30);
  }),
  fireplaceEffectBase: fpBase,
  slotEffectBase: systems.slotEffectBase(kt),
  orangeFireSum: orangeFire,
  fuelGen: systems.fuelGenPerHr(ul, sd, kt, orangeFire, ext.hasBundleV, ext.doublers),
  fuelCap: systems.fuelCapacity(ul, kt, ext.hasBundleV, ext.doublers, ext.eventShopStr),
  currencyMulti: systems.computeCurrencyMulti(ul, sd, us, kt, ext),
  bucksPerHr: systems.totalBucksPerHr(sd, ul, us, kt, ext),
  bucksPerSlot: Array.from({ length: slots }, (_, slot) =>
    systems.currencyPerSlot(
      slot,
      sd,
      systems.computeCurrencyMulti(ul, sd, us, kt, ext),
      kt
    )
  ),
  overtunedMulti: systems.computeOvertunedMulti(sd),
  combo: systems.comboMeterDetails(sd, ul),
  slotsOwned: systems.slotsOwned(ul),
  activeSlots: systems.countActiveSlots(sd),
  maxCookTier: systems.maxCookTier(ul),
  bonusCookTierPct: systems.bonusCookTierPct(ul, kt),
  freeShakerChance: systems.freeShakerChance(ul, kt, ext.gridBonus188, ext.doublers),
  saffronHrs: systems.saffronHrs(ul, ext.doublers),
  perfectoOdds: Array.from({ length: tiers + 1 }, (_, tier) => systems.perfectoOdds(tier, kt)),
  knowledgeXPReq: Array.from({ length: 31 }, (_, level) => systems.knowledgeXPReq(level)),
  knowledgeXPBase: systems.knowledgeXPBase(ul, ext.doublers),
  knowledgeXPMulti: Array.from({ length: slots }, (_, slot) =>
    systems.knowledgeXPMulti(slot, sd, ul, kt, ext.doublers)
  ),
  fireplaceEffects: Array.from({ length: 5 }, (_, type) =>
    systems.fireplaceEffectByType(type, kt, Number(sd?.[4]?.[2]) || 0)
  ),
  summary: systems.buildSushiSummary(sd, ul, us, kt, ext),
};

// The fixture save doubles no upgrades, so a second pass pins the doubled
// paths with the Menagerie rows set the way a real save carries them.
const doubledRows = [0, 2, 8, 22, 37, 39];
saveData.spelunkData[52] = doubledRows;
const doubledDoublers = systems.sushiDoublerMulti
  ? doubledRows.map((index) => systems.sushiDoublerMulti(index, doubledRows))
  : [];
formulas.doubled = {
  rows: doubledRows,
  doublers: systems.sushiDoublers(saveData),
  doublerMulti: doubledDoublers,
  upgradeQTY: Array.from({ length: upgrades }, (_, i) =>
    systems.upgradeQTY(i, ul, doubledRows)
  ),
  upgCost: Array.from({ length: slotsToUpg }, (_, slot) =>
    systems.upgCost(slot, ul, kt, saveData)
  ),
  fuelGen: systems.fuelGenPerHr(ul, sd, kt, orangeFire, ext.hasBundleV, doubledRows),
  fuelCap: systems.fuelCapacity(ul, kt, ext.hasBundleV, doubledRows, ext.eventShopStr),
  freeShakerChance: systems.freeShakerChance(ul, kt, ext.gridBonus188, doubledRows),
  saffronHrs: systems.saffronHrs(ul, doubledRows),
  knowledgeXPBase: systems.knowledgeXPBase(ul, doubledRows),
  slotsOwned: systems.slotsOwned(ul),
  maxCookTier: systems.maxCookTier(ul),
};
saveData.spelunkData[52] = [];

const out = join(root, 'crates/sushi/tests/fixtures');
mkdirSync(out, { recursive: true });
writeFileSync(join(out, 'upstream.json'), `${JSON.stringify(formulas, null, 1)}\n`);
console.log(
  `wrote ${formulas.upgCost.length} upgrade costs, ${formulas.bucksPerSlot.length} slot rates ` +
    `and ${formulas.perfectoOdds.length} tier odds`
);
