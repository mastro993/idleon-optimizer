// Extracts the bubble table the Prisma calculator reads.
// Usage: node script/extract-prisma-data.mjs <upstream-repo>
//
// Source: `js/stats/data/w2/alchemy.js` (`bubbleDefinitions`) reads
// `AlchemyDescription` from `js/stats/data/game/customlists.js`; the bubble
// ownership code letter table `N2L` comes from `js/stats/data/game/hardcoded.js`.
import { spawnSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-prisma-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { bubbleDefinitions, bubbleParams } = await import(
  join(source, 'js/stats/data/w2/alchemy.js')
);
const { N2L } = await import(join(source, 'js/stats/data/common/encoding.js'));
const { entityName } = await import(join(source, 'js/stats/entity-names.js'));

const bubbles = bubbleDefinitions().map((bubble) => {
  const params = bubbleParams(bubble.cauldron, bubble.index);
  if (!params) throw new Error(`no bubble params for ${bubble.cauldron}:${bubble.index}`);
  return {
    cauldron: bubble.cauldron,
    index: bubble.index,
    name: bubble.name,
    label: entityName('Bubble', [bubble.cauldron, bubble.index]),
    effect: bubble.effect,
    prismaEligible: bubble.prismaEligible,
    x1: params.x1,
    x2: params.x2,
    formula: params.formula,
  };
});

const payload = {
  letters: N2L,
  bubbles,
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/stats/data/w2/alchemy.js',
      'js/stats/data/common/encoding.js',
      'js/stats/entity-names.js',
    ],
  },
};
writeFileSync(
  new URL('../crates/core/data/prisma.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
const perCauldron = [0, 1, 2, 3].map(
  (cauldron) => bubbles.filter((bubble) => bubble.cauldron === cauldron).length,
);
console.log(`wrote ${bubbles.length} bubbles (${perCauldron.join(' / ')})`);
