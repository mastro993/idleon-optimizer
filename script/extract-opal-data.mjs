// Extracts the Opal Optimizer (W5) tables the Rust port reads from upstream.
// Usage: node script/extract-opal-data.mjs <path-to-idleon-research-optimizer>
//
// `opal-calc.html` keeps its villager maths inline and pulls each account
// bonus from another system. This file holds only the per-level constants
// those systems need, so `crates/core/src/opal` can reproduce `buildExt`
// without a second copy of the whole account tree. The bell and measurement
// rows already ship in `fountain.json`, extracted from the same upstream rows.
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-opal-data.mjs <upstream-repo>');
  process.exit(1);
}

if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const lists = await import(join(source, 'js/stats/data/game/customlists.js'));
const sigils = await import(join(source, 'js/stats/data/common/sigils.js'));
const constants = await import(join(source, 'js/stats/data/game-constants.js'));

const number = (value) => Number(value) || 0;

// `TalentDescriptions[56][1]`: `[x1, x2, formula]`, the All Statues talent the
// statue chain reads at its best character level.
const talentRow = lists.TalentDescriptions[56][1];

const payload = {
  // `CosmoUpgrades[tier][i][0]`: the per-level base of each cosmo upgrade. The
  // page reads tier 1 for the villager terms and tier 0 for the monument and
  // Holeoz ones.
  cosmo: (lists.CosmoUpgrades || []).map((tier) => (tier || []).map((row) => number(row[0]))),
  // `CompassUpg[i][5]` per-level bonus and `[9]` the type flag that makes a
  // compass bonus scale with the circle-shaped upgrades.
  compassPerLevel: lists.CompassUpg.map((row) => number(row[5])),
  compassConditional: lists.CompassUpg.map((row) => number(row[9])),
  // `pristineCharmBonus(i)` from `js/stats/data/common/sigils.js`.
  pristine: Array.from({ length: 40 }, (_, index) => number(sigils.pristineCharmBonus(index))),
  // `GrimoireUpg[i][5]`, the per-level bonus, and the upgrades the upstream
  // list leaves outside the ×Grimoire-36 multiplier.
  grimoirePerLevel: lists.GrimoireUpg.map((row) => number(row[5])),
  grimoireNoMulti: Array.from(constants.GRIMOIRE_NO_MULTI),
  // `StatueInfo[i][3]`: the bonus each statue level grants.
  statueBase: lists.StatueInfo.map((row) => number(row[3])),
  // `ZenithMarket[i][4]`: the bonus each Zenith Market level grants.
  zenithPerLevel: lists.ZenithMarket.map((row) => number(row[4])),
  // `Spelunky[21]`: the `base|threshold` pair behind each Lore Episode.
  spelunkyLore: (lists.Spelunky[21] || []).map((value) => String(value)),
  // `HolesInfo[37]`: the per-slot monument bonus the statue chain reads as
  // `MonumentROGbonuses(t, i)` (`slot = 10 × t + i`).
  holesMonument: (lists.HolesInfo[37] || []).map(number),
  // `TalentDescriptions[56][1]`: the All Statues talent curve.
  statueTalent: {
    x1: number(talentRow[0]),
    x2: number(talentRow[1]),
    formula: String(talentRow[2]),
  },
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'opal-calc.html (inline module script)',
      'js/stats/data/game/customlists.js',
      'js/stats/data/common/sigils.js',
      'js/stats/data/game-constants.js',
    ],
  },
};

const out = new URL('../crates/core/data/opal.json', import.meta.url);
writeFileSync(out, `${JSON.stringify(payload, null, 1)}\n`);
console.log(
  `wrote ${payload.cosmo.flat().length} cosmo bases, ${payload.compassPerLevel.length} compass upgrades, ` +
    `${payload.pristine.length} pristine charms, ${payload.holesMonument.length} monument slots`,
);
