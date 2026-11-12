// Regenerates crates/core/tests/cglunko-oracle.json from the upstream Cglunko
// formulas, so the Rust port is tested against the source of truth rather than
// hand-picked numbers. Usage:
//   node script/cglunko-oracle.mjs <path-to-idleon-research-optimizer>
//
// The priority rows are the upstream `rankUpgrades` table on `fixtures/data.json`
// with the page's own `upgNextCost` and `drMarginalGain`. The plan repeats this
// app's greedy rule (best affordable DR gain per cost, spending the stone the
// upgrade is bought with and refusing a purchase that lowers DR through a lost
// digit) on a hypothetical stone balance, because the fixture's own balances
// cannot afford a single level: the upstream module supplies every cost and
// drop-rate figure the rule reads.
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/cglunko-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const C = await import(join(source, 'js/stats/systems/w5/cglunko.js'));
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
const options = decode(raw.OptLacc) ?? decode(raw.OptionsListAccount);
if (!Array.isArray(options)) throw new Error('the fixture carries no OptLacc array');
saveData.assignSaveData({ optionsListData: options });
const ext = { bolaia17discount: false };
// The upstream helpers take a save object but read their rows from the module's
// `optionsListData`, so the fixture is the only input that matters here.
const S = {};

const ola = (index) => Number(options[index]) || 0;
const levels = Array.from({ length: C.UPGRADE_COUNT }, (_, index) => C.upgLevel(S, index));
const balances = Array.from({ length: 12 }, (_, index) => ola(654 + index));
const kills = ola(668);
const normalDR = ola(200);

const drAt = (overrides = {}) =>
  C.drBonusAt({ levels, balances, kills, normalDR, ...overrides });

const upgrades = Array.from({ length: C.UPGRADE_COUNT }, (_, index) => ({
  index,
  name: C.upgName(index),
  desc: C.upgDescription(index),
  perLv: C.upgPerLvVal(index),
  costBase: C.upgCostBase(index),
  category: C.UPGRADE_CATEGORIES[index],
  level: levels[index],
  bonus: C.upgBonus(S, index),
  nextCost: C.upgNextCost(S, index, ext),
  drGain: C.drMarginalGain(S, index),
  drEfficiency: (() => {
    const cost = C.upgNextCost(S, index, ext);
    return cost > 0 ? C.drMarginalGain(S, index) / cost : 0;
  })(),
  balance: balances[Math.floor(index / 2)],
}));

const ranked = C.rankUpgrades(S, ext);
// `rankUpgrades` skips the locked index 23, so the purple page ranks 11.
const expected = [C.PAGE_SIZE, C.PAGE_SIZE - 1];
const order = (page) => {
  const indices = ranked.filter((row) => row.page === page).map((row) => row.idx);
  if (indices.length !== expected[page]) throw new Error(`page ${page} ranks ${indices.length} upgrades`);
  return indices;
};

// The plan scenario: the fixture's levels, kills and normal DR, with enough
// stones on each page that the cost curve turns over several purchases.
const scenario = {
  blueBalances: [50_000, 40_000, 30_000, 20_000, 10_000, 5_000],
  purpleBalances: [50_000, 40_000, 30_000, 20_000, 10_000, 5_000],
};

function plan(page) {
  const level = levels.slice();
  const balance = balances.slice();
  const offset = page * 6;
  for (let stone = 0; stone < 6; stone++) {
    balance[offset + stone] = (page === 0 ? scenario.blueBalances : scenario.purpleBalances)[stone];
  }
  const currentDR = () => drAt({ levels: level, balances: balance });
  const start = currentDR();
  const steps = [];
  const refused = new Set();
  let spent = 0;
  for (let count = 0; count < 500; count++) {
    let best = null;
    for (let index = page * 12; index < page * 12 + 12; index++) {
      if (refused.has(index) || !(C.UPGRADE_CATEGORIES[index] || '').startsWith('dr-')) continue;
      const cost = C.withCglunkoLevels(level, () => C.upgNextCost(S, index, ext));
      const stone = Math.floor(index / 2);
      if (!Number.isFinite(cost) || cost <= 0 || cost > balance[stone]) continue;
      const nextLevels = level.slice();
      nextLevels[index]++;
      const gain = drAt({ levels: nextLevels, balances: balance }) / currentDR() - 1;
      if (!Number.isFinite(gain) || gain <= 0) continue;
      const efficiency = gain / cost;
      if (!best || efficiency > best.efficiency) {
        best = { index, cost, gain, efficiency };
      }
    }
    if (!best) break;
    const before = currentDR();
    const nextLevels = level.slice();
    nextLevels[best.index]++;
    const stone = Math.floor(best.index / 2);
    const balanceBefore = balance[stone];
    const left = balanceBefore - best.cost;
    const balanceAfter = left <= 0 ? 0.1 : left;
    const prospective = balance.slice();
    prospective[stone] = balanceAfter;
    const after = drAt({ levels: nextLevels, balances: prospective });
    if (!(after > before)) {
      refused.add(best.index);
      continue;
    }
    balance[stone] = balanceAfter;
    steps.push({
      index: best.index,
      from: level[best.index],
      cost: best.cost,
      drGain: best.gain,
      balanceBefore,
      balanceAfter,
    });
    level[best.index]++;
    spent += best.cost;
  }
  return {
    start,
    dr: currentDR(),
    spent,
    steps: steps.map(({ index, from, cost, drGain, balanceBefore, balanceAfter }) => ({
      index,
      from,
      cost,
      drGain,
      balanceBefore,
      balanceAfter,
    })),
  };
}

const payload = {
  source: 'js/stats/systems/w5/cglunko.js',
  ext,
  dr: C.drBonus(S),
  upgrades,
  priority: { blue: order(0), purple: order(1) },
  scenario,
  plans: { blue: plan(0), purple: plan(1) },
  generatedFrom: { repo: 'Corgan/idleon-research-optimizer', fixture: 'fixtures/data.json' },
};

const out = new URL('../crates/core/tests/cglunko-oracle.json', import.meta.url);
writeFileSync(out, JSON.stringify(payload, null, 1));
console.log(
  `wrote ${upgrades.length} upgrades, ${payload.plans.blue.steps.length} blue steps and ${payload.plans.purple.steps.length} purple steps`
);
