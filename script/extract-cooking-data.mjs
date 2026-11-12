// Extracts the Cooking tables the ladle kitchen plan reads.
// Usage: node script/extract-cooking-data.mjs <upstream-repo>
//
// Sources, all in `Corgan/idleon-research-optimizer`:
// - `js/stats/data/game/customlists.js`: `MealINFO`, whose rows are
//   `[name, requirement, bonusPerLevel, effect, flavor, effectKey]`.
// - `js/stats/data/game/hardcoded.js`: `N2L`, the `Number2Letter` alphabet the
//   emporium and event-shop ownership strings encode their flags with.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-cooking-data.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { MealINFO } = await import(join(source, 'js/stats/data/game/customlists.js'));
const { N2L } = await import(join(source, 'js/stats/data/game/hardcoded.js'));

const meals = MealINFO.map((row, index) => {
  const requirement = Number(row[1]);
  if (!Number.isFinite(requirement) || requirement <= 0) {
    throw new Error(`meal ${index} has no cooking requirement`);
  }
  return {
    name: String(row[0]).replace(/_/g, ' '),
    requirement,
    bonus: Number(row[2]),
  };
});

const payload = {
  meals,
  // `emporiumBonus` and `eventShopOwned` match `N2L[index]` inside a save's
  // ownership string; the kitchen plan reads emporium 16 for No Meal Left
  // Behind.
  numberToLetter: String(N2L),
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: ['js/stats/data/game/customlists.js', 'js/stats/data/game/hardcoded.js'],
  },
};
writeFileSync(
  new URL('../crates/core/data/cooking.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(`cooking: ${meals.length} meals, ${payload.numberToLetter.length} encoding letters`);
