// Regenerates crates/core/tests/fishing-fixture.json from the upstream fishing
// formulas, so the Rust port is tested against the source of truth.
// Usage: node script/fishing-oracle.mjs <path-to-idleon-research-optimizer>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/fishing-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const fishing = await import(join(source, 'js/stats/systems/w2/fishing.js'));
const data = await import(join(source, 'js/stats/data/w2/fishing.js'));

const round = (value) => Number(Number(value).toFixed(10));

const depths = [
  [{ D0: 0, D1: 0, D2: 0, D3: 0 }, [0, 0, 0], [0, 0, 0, 0]],
  [{ D0: 2, D1: 1, D2: 0, D3: 0 }, [0, 0, 0], [0, 25, 5, 0]],
  [{ D0: 10, D1: 20, D2: 30, D3: 40 }, [1, 2, 3], [0, 0, 0, 0]],
  [{ D0: 12, D1: 12, D2: 0, D3: 0 }, [40, 30, 20], [0, 0, 35, 0]],
  [{ D0: 0, D1: 0, D2: 35, D3: 10 }, [0, 0, 0], [0, 0, 45, 70]],
].map(([toolkit, equipment, spot]) => ({
  toolkit,
  equipment,
  spot,
  expected: fishing.fishingDepths(toolkit, equipment, spot),
}));

const shareCases = [
  [0, { D1: 0, D2: 0, D3: 0 }, [50, 200, 400]],
  [49.99, { D1: 0, D2: 0, D3: 0 }, [50, 200, 400]],
  [50, { D1: 0, D2: 0, D3: 0 }, [50, 200, 400]],
  [5000, { D1: 20, D2: 30, D3: 40 }, [50, 200, 400]],
  [10000, { D1: 5, D2: 15, D3: 54 }, [10000, 50000, 200000]],
  [2_000_000, { D1: 1, D2: 33.666666666666664, D3: 39.666666666666664 }, [10000, 11000, 12000]],
  [1e9, { D1: 1e6, D2: 1e6, D3: 1e6 }, [1000000, 5000000, 30000000]],
];
const shares = shareCases.map(([efficiency, d, effReqs]) => ({
  efficiency,
  depths: d,
  effReqs,
  expected: fishing.fishingCatchShares(efficiency, d, effReqs),
}));

const oreValue = [
  [0, 400, 0],
  [50, 400, 0],
  [10000, 400, 0.25],
  [2_000_000, 12000, 0.1],
  [1e9, 30000000, 1],
].map(([efficiency, req, prowess]) => ({
  efficiency,
  effReq: req,
  prowess,
  expected: fishing.fishingOreValue(efficiency, req, prowess),
}));

const multiFish = [
  { power: 0, level: 0, stamp: 0, bubble: 0, starSign: 0, cosmo: 0 },
  { power: 1200, level: 100, stamp: 25, bubble: 19, starSign: 5, cosmo: 0 },
  { power: 1200, level: 100, stamp: 25, bubble: 20, starSign: 5, cosmo: 10 },
  { power: 999999, level: 500, stamp: 1000, bubble: 200, starSign: 100, cosmo: 100 },
].map((inputs) => ({ inputs, expected: fishing.fishingMultiFish(inputs) }));

const speed = [
  { efficiency: 0, level: 0, rodSpeed: 0, speedBonusPct: 0 },
  { efficiency: 5000, level: 100, rodSpeed: 3, speedBonusPct: 20 },
  { efficiency: 12000, level: 250, rodSpeed: 8, speedBonusPct: 120 },
  { efficiency: 1e6, level: 1000, rodSpeed: 10, speedBonusPct: 500 },
].map((inputs) => ({ inputs, expected: fishing.fishingSpeed(inputs) }));

const catches = [
  [0, 0],
  [2.5, 1],
  [1.2, 2.4],
  [0.57, 5],
].map(([catchesPerHour, attackFactor]) => ({
  speed: catchesPerHour,
  attackFactor,
  expected: fishing.fishingCatchesPerHour(catchesPerHour, attackFactor),
}));

const inputSets = [
  {
    efficiency: 5000,
    depths: { D0: 2, D1: 22, D2: 5, D3: 1 },
    prowess: 0.12,
    catchesPerHour: 220,
    afkRate: 0.42,
    multiFish: 28,
    expMulti: 2.5,
  },
  {
    efficiency: 2_000_000,
    depths: { D0: 12, D1: 0, D2: 33.666666666666664, D3: 39.666666666666664 },
    prowess: 0.31,
    catchesPerHour: 310,
    afkRate: 0.9,
    multiFish: 120,
    expMulti: 6.25,
  },
  {
    efficiency: 1e9,
    depths: { D0: 0, D1: 1000, D2: 2000, D3: 3000 },
    prowess: 1,
    catchesPerHour: 400,
    afkRate: 1,
    multiFish: 350,
    expMulti: 10,
  },
];

const outcomeCases = [];
for (const poolKey of data.FISHING_POOL_KEYS) {
  inputSets.forEach((inputs, index) => {
    const hours = index === 0 ? 1 : index === 1 ? 2 : 8;
    const outcome = fishing.fishingPoolOutcome(poolKey, inputs, hours);
    outcomeCases.push({
      pool: poolKey,
      inputs,
      hours,
      expected: {
        hours: outcome.hours,
        shares: outcome.shares,
        shareTotal: outcome.shareTotal,
        oreValue: outcome.oreValue,
        catches: outcome.catches,
        fishPerCatch: outcome.fishPerCatch,
        fishTotal: outcome.fishTotal,
        rolledFishTotal: outcome.rolledFishTotal,
        overwrittenFish: outcome.overwrittenFish,
        exp: outcome.exp,
        expPerCatch: outcome.expPerCatch,
        panelCatchesPerHour: outcome.panelCatchesPerHour,
        panelExpPerHour: outcome.panelExpPerHour,
        panelOreValue: outcome.panelOreValue,
        fish: outcome.fish.map((fish) => ({
          slot: fish.slot,
          itemKey: fish.itemKey,
          share: fish.share,
          active: fish.active,
          unlocked: fish.unlocked,
          effReq: fish.effReq,
          count: fish.count,
          exp: fish.exp,
          credited: fish.credited,
          overwritten: fish.overwritten,
          creditedCount: fish.creditedCount,
        })),
      },
    });
  });
}

const payload = {
  source: 'Corgan/idleon-research-optimizer js/stats/systems/w2/fishing.js',
  depths: depths.map((entry) => ({
    ...entry,
    expected: {
      D0: round(entry.expected.D0),
      D1: round(entry.expected.D1),
      D2: round(entry.expected.D2),
      D3: round(entry.expected.D3),
    },
  })),
  shares,
  oreValue,
  multiFish,
  speed,
  catches,
  outcomes: outcomeCases,
};

writeFileSync(
  join(root, 'crates/core/tests/fishing-fixture.json'),
  `${JSON.stringify(payload, null, 2)}\n`
);
console.log(
  `wrote ${depths.length} depth, ${shares.length} split, ${oreValue.length} ore, ` +
    `${multiFish.length} multi, ${speed.length} speed, ${catches.length} catch and ` +
    `${outcomeCases.length} pool outcome cases`
);
