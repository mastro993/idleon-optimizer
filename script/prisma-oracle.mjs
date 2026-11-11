// Regenerates `crates/core/tests/prisma-oracle.json` from the upstream Prisma
// plan, so the Rust port is tested against the source of truth rather than
// hand-picked numbers.
// Usage: node script/prisma-oracle.mjs <path-to-idleon-research-optimizer>
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/prisma-oracle.mjs <upstream-repo>');
  process.exit(1);
}
const root = join(import.meta.dirname, '..');

// Upstream files are ESM with .js extensions, so they need a module package.
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { loadSaveData } = await import(join(source, 'js/save/loader.js'));
const { saveData } = await import(join(source, 'js/state.js'));
const { prismaPlan } = await import(join(source, 'js/stats/prisma.js'));
const { getPrismaBonusMult } = await import(join(source, 'js/stats/systems/w2/alchemy.js'));
const { bubbleParams } = await import(join(source, 'js/stats/data/w2/alchemy.js'));
const { formulaEval } = await import(join(source, 'js/formulas.js'));

/// One save's plan in every cauldron, plus its Prisma multiplier.
function read(file) {
  const save = JSON.parse(readFileSync(join(root, file), 'utf8'));
  // A committed save may already be an `it.json` wrapper with `charNames` beside it.
  const raw = save.data ? save : { data: save };
  loadSaveData(raw);
  const multiplier = getPrismaBonusMult(saveData);
  const cauldrons = {};
  for (let cauldron = 0; cauldron < 4; cauldron += 1) {
    const plan = prismaPlan(raw, { charIdx: 0, cauldron });
    cauldrons[cauldron] = {
      fragments: plan.fragments,
      map: plan.mapIdx,
      rows: plan.rows.map((row) => {
        const params = bubbleParams(row.cauldron, row.index);
        const base = row.level > 0 ? formulaEval(params.formula, params.x1, params.x2, row.level) : 0;
        return {
          id: row.id,
          name: row.name,
          effect: row.effect,
          level: row.level,
          status: row.status,
          comparable: row.comparable,
          reason: row.reason,
          base,
          withPrisma: base * Math.max(1, multiplier),
        };
      }),
    };
  }
  return {
    characters: prismaPlan(raw, { charIdx: 0, cauldron: 0 }).characters.map((character) => ({
      ci: character.ci,
      name: character.name,
      className: character.className,
    })),
    multiplier,
    cauldrons,
  };
}

const payload = {
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: ['js/stats/prisma.js', 'js/stats/systems/w2/alchemy.js'],
    saves: ['fixtures/data.json', 'crates/core/tests/prisma-save.json'],
  },
  saves: {
    fixture: read('fixtures/data.json'),
    tiny: read('crates/core/tests/prisma-save.json'),
  },
};
writeFileSync(
  join(root, 'crates/core/tests/prisma-oracle.json'),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(
  `wrote prisma-oracle.json: ${payload.saves.fixture.characters.length} characters, ` +
    `multiplier ${payload.saves.fixture.multiplier}`,
);
