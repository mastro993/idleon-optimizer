// Extracts the statue table and fixture-level EXP requirements the Rust app ships.
// Usage: node script/extract-statue-data.mjs <upstream-repo> [fixtures/data.json]
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-statue-data.mjs <upstream-repo> [save.json]');
  process.exit(1);
}
const fixture = process.argv[3];

// Upstream files are ESM with .js extensions, so they need a module package.
const marker = join(source, 'package.json');
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(marker, JSON.stringify({ type: 'module' }));

const { STATUE_COUNT, statueInfo } = await import(
  join(source, 'js/stats/data/common/statues.js')
);
const { statueExpRequirement } = await import(
  join(source, 'js/stats/systems/common/statue-leveling.js')
);

const statues = Array.from({ length: STATUE_COUNT }, (_, index) => {
  const info = statueInfo(index);
  if (!info) throw new Error(`the upstream statue table has no row ${index}`);
  return info;
});

const payload = {
  count: STATUE_COUNT,
  statues,
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    file: 'js/stats/data/common/statues.js',
  },
};
writeFileSync(
  new URL('../crates/core/data/statues.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(`wrote ${STATUE_COUNT} statues`);

if (!fixture) process.exit(0);

const save = JSON.parse(readFileSync(fixture, 'utf8'));
const data = save.data ?? save;
const characters = [];
for (let charIdx = 0; ; charIdx += 1) {
  const raw = data[`StatueLevels_${charIdx}`];
  if (raw == null) break;
  const rows = typeof raw === 'string' ? JSON.parse(raw) : raw;
  if (!Array.isArray(rows) || rows.length === 0) {
    characters.push([]);
    continue;
  }
  const S = { statueLvAllData: [rows] };
  characters.push(
    rows.map((row, index) => {
      const level = Number(row[0]);
      return {
        index,
        level,
        exp: Number(row[1]),
        required: statueExpRequirement(S, index, level),
        nextRequired: statueExpRequirement(S, index, level + 1),
      };
    }),
  );
}

const expected = {
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    file: 'js/stats/systems/common/statue-leveling.js',
    fixture: 'fixtures/data.json',
  },
  characters: characters.length,
  firstCharacter: characters.find((rows) => rows.length) ?? [],
};
writeFileSync(
  new URL('../crates/core/tests/statue-requirements.json', import.meta.url),
  `${JSON.stringify(expected, null, 1)}\n`,
);
console.log(`wrote the requirement table for character ${characters.findIndex((rows) => rows.length)}`);

// The target-cost oracle: the upstream loader's view of the fixture, its
// statue leveling inputs and `statueTargetCost` for three targets per statue.
const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const { saveData } = await import(join(source, 'js/state.js'));
const { statueLevelingInputs, statueTargetCost } = await import(
  join(source, 'js/stats/systems/common/statue-leveling.js')
);
loadSaveData(save);
const costs = [];
for (let charIdx = 0; charIdx < (saveData.statueLvAllData || []).length; charIdx += 1) {
  const savedRows = saveData.statueLvAllData[charIdx] || [];
  if (!savedRows.length) continue;
  const inputs = statueLevelingInputs(saveData, charIdx);
  const rows = [];
  for (let index = 0; index < STATUE_COUNT; index += 1) {
    const saved = savedRows[index];
    const level = Number(saved?.[0]);
    if (!Number.isSafeInteger(level)) continue;
    const targets = [level + 1, level + 10, Math.max(level - 1, 0)];
    const entries = targets.map((target) => {
      try {
        const cost = statueTargetCost(saveData, index, target, { charIdx, inputs });
        return cost.available
          ? {
              target,
              expNeeded: cost.expNeeded,
              statuesGuaranteed: cost.statuesGuaranteed,
              statuesAverage: cost.statuesAverage,
            }
          : { target, unavailable: cost.missing };
      } catch (error) {
        return { target, unavailable: error.message };
      }
    });
    const sample = statueTargetCost(saveData, index, level + 1, { charIdx, inputs });
    rows.push({
      index,
      level,
      exp: Number(saved?.[1]),
      flairLevel: sample.flairLevel,
      flairMultiplier: sample.flairMultiplier,
      guaranteedExp: sample.guaranteedExp,
      averageExp: sample.averageExp,
      costs: entries,
    });
  }
  costs.push({
    character: charIdx,
    name: saveData.charNames?.[charIdx] || '',
    inputs: {
      bubblePct: inputs.bubblePct,
      costFactor: inputs.costFactor,
      retentionPct: inputs.retentionPct,
      doubleChance: inputs.doubleChance,
      onyxCount: inputs.onyxCount,
      zenithCount: inputs.zenithCount,
      tierMultiplier: inputs.tierMultiplier,
      flairAvailable: inputs.flairAvailable,
    },
    rows,
  });
}
writeFileSync(
  new URL('../crates/core/tests/statue-costs.json', import.meta.url),
  JSON.stringify({
    generatedFrom: {
      repo: 'Corgan/idleon-research-optimizer',
      file: 'js/stats/systems/common/statue-leveling.js',
      fixture: 'fixtures/data.json',
    },
    characters: costs,
  }),
);
console.log(`wrote the target-cost table for ${costs.length} characters`);
