// Regenerates crates/core/tests/fishing-fixture.json from the upstream fishing
// formulas, so the Rust port is tested against the source of truth.
// Usage: node script/fishing-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
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
const motherlode = await import(join(source, 'js/stats/systems/w5/motherlode.js'));
const loader = await import(join(source, 'js/save/loader.js'));
const state = await import(join(source, 'js/state.js'));

// The Trench requirement reads the save's cavern rows, so the oracle evaluates
// the same committed fixture the Rust test imports.
const save = JSON.parse(readFileSync(join(root, 'fixtures/data.json'), 'utf8'));
loader.loadSaveData({ data: save, serverVars: {} });
const saveData = state.saveData;

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

// Lure/line rankings. The Rust port holds efficiency and the EXP multiplier
// fixed across a ranking (it has no skill-system decode yet), so the evaluator
// handed to the upstream `rankFishingToolkits` builds the per-toolkit inputs
// from the same fixed account values: depth, catch time and the outcome still
// come from the upstream formulas, and the ranking loop and sort are the
// upstream's own.
const rankCases = [
  {
    location: '54:0',
    character: { level: 100, rodSpeed: 8 },
    account: {
      efficiency: 5000,
      expMulti: 2.5,
      afkRate: 0.42,
      multiFish: 28,
      speedBonus: 12,
      attackFactor: 1.4,
      prowess: 0.12,
      equipmentDepth: [0, 0, 0],
    },
    objective: 'exp',
    hours: 1,
  },
  {
    location: '61:1',
    character: { level: 250, rodSpeed: 8 },
    account: {
      efficiency: 120_000,
      expMulti: 6.25,
      afkRate: 0.9,
      multiFish: 120,
      speedBonus: 45,
      attackFactor: 2.4,
      prowess: 0.31,
      equipmentDepth: [5, 4, 3],
    },
    objective: 'exp',
    hours: 2,
  },
  {
    location: 'x:MotherlodeFISH',
    character: { level: 500, rodSpeed: 8 },
    account: {
      efficiency: 2_000_000,
      expMulti: 10,
      afkRate: 1,
      multiFish: 350,
      speedBonus: 200,
      attackFactor: 3,
      prowess: 1,
      equipmentDepth: [40, 30, 20],
    },
    objective: 'fish',
    hours: 8,
  },
  {
    location: '72:FishBig',
    character: { level: 1, rodSpeed: 0 },
    account: {
      efficiency: 0,
      expMulti: 1,
      afkRate: 0.01,
      multiFish: 0,
      speedBonus: 0,
      attackFactor: 1,
      prowess: 0,
      equipmentDepth: [0, 0, 0],
    },
    objective: 'fish',
    hours: 1,
  },
];

const rankings = rankCases.map((rankCase) => {
  const location = data.fishingLocations().find((entry) => entry.id === rankCase.location);
  if (!location) throw new Error(`unknown fishing location ${rankCase.location}`);
  const spotBonus = data.fishingSpotDepthBonus(location.mapIdx, location.spotIdx);
  const account = rankCase.account;
  const evaluator = {
    inputsFor(selection) {
      const toolkit = fishing.toolkitStats(selection);
      const speedBonusPct = account.speedBonus + toolkit.stats.SPEED;
      const speed = fishing.fishingSpeed({
        efficiency: account.efficiency,
        level: rankCase.character.level,
        rodSpeed: rankCase.character.rodSpeed,
        speedBonusPct,
      });
      return {
        efficiency: account.efficiency,
        depths: fishing.fishingDepths(toolkit.stats, account.equipmentDepth, spotBonus),
        prowess: account.prowess,
        catchesPerHour: fishing.fishingCatchesPerHour(speed, account.attackFactor),
        afkRate: account.afkRate,
        multiFish: account.multiFish,
        expMulti: account.expMulti,
      };
    },
  };
  const rows = fishing.rankFishingToolkits(
    evaluator,
    location.poolKey,
    location,
    rankCase.objective,
    rankCase.hours
  );
  return {
    ...rankCase,
    rows: rows.map((row) => ({
      lure: row.selection.lure,
      line: row.selection.line,
      exp: round(row.outcome.exp),
      fishTotal: round(row.outcome.fishTotal),
    })),
  };
});

// The Trench depth requirement (FI-03), evaluated from the fixture's cavern
// rows: the saved depth, two projected depths and the efficiency requirements
// the upstream `fishingPoolOutcome` swaps in for the pool's own.
const trenchRequirements = [null, 9, 11, 24].map((layers) => {
  const expected =
    layers == null
      ? motherlode.cavernLayerRequirement(saveData, 3)
      : motherlode.cavernLayerRequirement(saveData, 3, layers);
  return {
    layers,
    effReqs:
      layers == null
        ? motherlode.trenchFishingEffReqs(saveData)
        : motherlode.trenchFishingEffReqs(saveData, layers),
    expected: {
      layers: expected.layers,
      base: expected.base,
      jar: expected.jarCollectible5,
      legend: expected.legend29,
      bolaia: expected.bolaia.map((term) => ({
        cavern: term.cavern,
        study: term.study,
        perLayer: term.perLayer,
        layers: term.layers,
        value: term.value,
      })),
      reductionPct: expected.reductionPct,
      value: expected.value,
      progress: expected.progress,
      remaining: expected.remaining,
    },
  };
});

// The upstream `trenchPlan` resolves efficiency and the EXP multiplier from the
// skill systems for every toolkit. This port holds both in its account card, so
// the oracle builds each selection's inputs from the same fixed values the
// ranking cases use, then runs the upstream pill's own loop: the per-depth
// requirement, the efficiency override and every outcome are upstream calls.
const trenchCases = [
  {
    character: { level: 250, rodSpeed: 8 },
    account: {
      efficiency: 5000,
      expMulti: 2.5,
      afkRate: 0.42,
      multiFish: 28,
      speedBonus: 12,
      attackFactor: 1.4,
      prowess: 0.12,
      equipmentDepth: [0, 0, 0],
    },
    depths: 3,
    selection: { lure: 0, line: 4 },
  },
  {
    character: { level: 100, rodSpeed: 3 },
    account: {
      efficiency: 2_000_000,
      expMulti: 6.25,
      afkRate: 0.9,
      multiFish: 120,
      speedBonus: 45,
      attackFactor: 2.4,
      prowess: 0.31,
      equipmentDepth: [5, 4, 3],
    },
    depths: 5,
    selection: { lure: 8, line: 4 },
  },
];

const trenchPlans = trenchCases.map((testCase) => {
  const account = testCase.account;
  const inputsFor = (selection) => {
    const toolkit = fishing.toolkitStats(selection);
    const speedBonusPct = account.speedBonus + toolkit.stats.SPEED;
    const speed = fishing.fishingSpeed({
      efficiency: account.efficiency,
      level: testCase.character.level,
      rodSpeed: testCase.character.rodSpeed,
      speedBonusPct,
    });
    return {
      efficiency: account.efficiency,
      depths: fishing.fishingDepths(toolkit.stats, account.equipmentDepth, [0, 0, 0, 0]),
      prowess: account.prowess,
      catchesPerHour: fishing.fishingCatchesPerHour(speed, account.attackFactor),
      afkRate: account.afkRate,
      multiFish: account.multiFish,
      expMulti: account.expMulti,
    };
  };
  const saved = motherlode.cavernLayerRequirement(saveData, 3);
  const count = Math.max(1, Math.min(testCase.depths, 25));
  const rows = [];
  let cumulativeSelected = 0;
  let cumulativeBest = 0;
  for (let k = 0; k < count; k++) {
    const layers = saved.layers + k;
    const requirement = k === 0 ? saved : motherlode.cavernLayerRequirement(saveData, 3, layers);
    const effReqs = motherlode.trenchFishingEffReqs(saveData, layers);
    const outcomeFor = (selection) =>
      fishing.fishingPoolOutcome(
        'MotherlodeFISH',
        Object.assign({}, inputsFor(selection), {
          effReqOverrides: { MotherlodeFISH: effReqs },
        }),
        1
      );
    const selected = outcomeFor(testCase.selection);
    let best = { selection: testCase.selection, outcome: selected };
    for (let lure = 0; lure < 15; lure++) {
      for (let line = 0; line < 15; line++) {
        const selection = { lure, line };
        const outcome = outcomeFor(selection);
        if (
          outcome.fishTotal > best.outcome.fishTotal ||
          (outcome.fishTotal === best.outcome.fishTotal &&
            outcome.exp > best.outcome.exp)
        ) {
          best = { selection, outcome };
        }
      }
    }
    const hoursSelected =
      selected.fishTotal > 0 ? requirement.remaining / selected.fishTotal : Infinity;
    const hoursBest =
      best.outcome.fishTotal > 0 ? requirement.remaining / best.outcome.fishTotal : Infinity;
    cumulativeSelected += hoursSelected;
    cumulativeBest += hoursBest;
    rows.push({
      depth: layers + 1,
      layers,
      effReqs,
      requirement: {
        base: requirement.base,
        reductionPct: requirement.reductionPct,
        value: requirement.value,
        progress: requirement.progress,
        remaining: requirement.remaining,
      },
      selected: {
        fishTotal: selected.fishTotal,
        exp: selected.exp,
        oreValue: selected.oreValue,
        hours: Number.isFinite(hoursSelected) ? hoursSelected : null,
        cumulativeHours: Number.isFinite(cumulativeSelected) ? cumulativeSelected : null,
      },
      best: {
        lure: best.selection.lure,
        line: best.selection.line,
        fishTotal: best.outcome.fishTotal,
        exp: best.outcome.exp,
        oreValue: best.outcome.oreValue,
        hours: Number.isFinite(hoursBest) ? hoursBest : null,
        cumulativeHours: Number.isFinite(cumulativeBest) ? cumulativeBest : null,
      },
    });
  }
  return { ...testCase, rows };
});

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
  rankings,
  trench: { requirements: trenchRequirements, plans: trenchPlans },
};

writeFileSync(
  join(root, 'crates/core/tests/fishing-fixture.json'),
  `${JSON.stringify(payload, null, 2)}\n`
);
console.log(
  `wrote ${depths.length} depth, ${shares.length} split, ${oreValue.length} ore, ` +
    `${multiFish.length} multi, ${speed.length} speed, ${catches.length} catch, ` +
    `${outcomeCases.length} pool outcome, ${rankings.length} ranking, ` +
    `${trenchRequirements.length} Trench requirement and ${trenchPlans.length} Trench plan cases`
);
