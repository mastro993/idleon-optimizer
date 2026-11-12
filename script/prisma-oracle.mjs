// Regenerates `crates/prisma/tests/prisma-oracle.json` from the upstream Prisma
// plan and comparison, so the Rust port is tested against the source of truth
// rather than hand-picked numbers.
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
const { prismaPlan, prismaVariant, prismaChanges } = await import(
  join(source, 'js/stats/prisma.js')
);
const { evaluatePrismaOutcomes } = await import(join(source, 'js/stats/prisma-evaluator.js'));
const { getPrismaBonusMult } = await import(join(source, 'js/stats/systems/w2/alchemy.js'));
const { bubbleParams } = await import(join(source, 'js/stats/data/w2/alchemy.js'));
const { formulaEval } = await import(join(source, 'js/formulas.js'));

/// One save's plan in `cauldrons`, plus its Prisma multiplier.
function read(file, cauldrons = [0, 1, 2, 3]) {
  const save = JSON.parse(readFileSync(join(root, file), 'utf8'));
  // A committed save may already be an `it.json` wrapper with `charNames` beside it.
  const raw = save.data ? save : { data: save };
  loadSaveData(raw);
  const multiplier = getPrismaBonusMult(saveData);
  const blocks = {};
  for (const cauldron of cauldrons) {
    const plan = prismaPlan(raw, { charIdx: 0, cauldron });
    blocks[cauldron] = {
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
    raw,
    plan: {
      characters: prismaPlan(raw, { charIdx: 0, cauldron: 0 }).characters.map((character) => ({
        ci: character.ci,
        name: character.name,
        className: character.className,
      })),
      multiplier,
      cauldrons: blocks,
    },
  };
}

// The downstream metrics the Rust port models. A row comparison keeps only
// these, so the oracle stays small even though upstream evaluates thousands.
const MODELLED = new Set(['statue-retention']);

/// One cauldron's per-candidate comparison, as `evaluatePrismaCauldron` makes it.
///
/// `bubble-cache` entries are intermediates upstream's TalentCalc cache rewrites
/// (class, slab, tome and big-bubble passes); the Rust port measures the plain
/// bubble bonus (base × Prisma) instead, so only the downstream metrics are
/// compared here.
async function comparisons(raw, cauldrons) {
  loadSaveData(raw);
  const block = {};
  for (const cauldron of cauldrons) {
    const plan = prismaPlan(raw, { charIdx: 0, cauldron });
    const baseline = await evaluatePrismaOutcomes(raw, 0, { afkHours: 1 });
    const rows = [];
    for (const row of plan.rows.filter((candidate) => candidate.comparable)) {
      const retained = row.status === 'already';
      let changed;
      try {
        changed = await evaluatePrismaOutcomes(
          prismaVariant(raw, cauldron, row.index, { prismad: !retained }),
          0,
          { afkHours: 1 },
        );
      } catch (error) {
        // `prismaVariant` re-checks Prisma ownership through the formula
        // sandbox, which rejects the fixture's `0c9,` entry the plan's substring
        // test counts as Prisma-ed. Upstream's own run stores this error row.
        rows.push({ id: row.id, comparison: retained ? 'retained' : 'added', error: String(error.message || error) });
        continue;
      }
      const diff = prismaChanges(retained ? changed : baseline, retained ? baseline : changed);
      const params = bubbleParams(row.cauldron, row.index);
      const base = row.level > 0 ? formulaEval(params.formula, params.x1, params.x2, row.level) : 0;
      rows.push({
        id: row.id,
        comparison: retained ? 'retained' : 'added',
        base,
        withPrisma: base * Math.max(1, getPrismaBonusMult(saveData)),
        changes: [...diff.changes, ...diff.intermediates]
          .filter((change) => MODELLED.has(change.id))
          .map((change) => ({
            id: change.id,
            name: change.name,
            before: change.before,
            after: change.after,
            delta: change.delta,
            percent: change.percent,
          })),
      });
    }
    block[cauldron] = rows;
  }
  return block;
}

const fixture = read('fixtures/data.json');
const tiny = read('crates/prisma/tests/prisma-save.json');
// The purpose-built save only needs its STARTUE EXP cauldron in the oracle.
const outcomes = read('crates/prisma/tests/prisma-outcomes-save.json', [3]);

const payload = {
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/stats/prisma.js',
      'js/stats/systems/w2/alchemy.js',
      'js/stats/systems/common/statue-leveling.js',
    ],
    saves: [
      'fixtures/data.json',
      'crates/prisma/tests/prisma-save.json',
      'crates/prisma/tests/prisma-outcomes-save.json',
    ],
  },
  saves: {
    fixture: { ...fixture.plan, comparisons: await comparisons(fixture.raw, [3]) },
    tiny: { ...tiny.plan, comparisons: await comparisons(tiny.raw, [0]) },
    outcomes: { ...outcomes.plan, comparisons: await comparisons(outcomes.raw, [3]) },
  },
};
writeFileSync(
  join(root, 'crates/prisma/tests/prisma-oracle.json'),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(
  `wrote prisma-oracle.json: ${payload.saves.fixture.characters.length} characters, ` +
    `multiplier ${payload.saves.fixture.multiplier}`,
);
