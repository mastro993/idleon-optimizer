// Extracts the Jelly Operator tables the W7 section reads.
// Usage: node script/extract-jelly-data.mjs <upstream-repo>
//
// Sources, all in `Corgan/idleon-research-optimizer`:
// - `js/stats/data/w7/jelly-operator.js`: `CELL_NAMES`, `CELL_BASE_COOLDOWNS`,
//   `CELL_BASE_DAMAGE`, `jellyUpgradeData` (`JellyUPG`), `jellyUpgradeOrder`
//   (`Research[44]`), `jellyRewardData` (`Research[45..47]`), `cellFootprint`
//   (`Research[49]`), `jellySlotPlots` (`Research[50]`) and
//   `jellyProjectileData` (`Research[51]`).
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-jelly-data.mjs <upstream-repo>');
  process.exit(1);
}
if (spawnSync('node', ['-e', '1']).status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const data = await import(join(source, 'js/stats/data/w7/jelly-operator.js'));
const { AtomInfo } = await import(join(source, 'js/stats/data/game/customlists.js'));
const cells = data.CELL_NAMES.length;
if (data.CELL_BASE_COOLDOWNS.length !== cells || data.CELL_BASE_DAMAGE.length !== cells) {
  throw new Error('the cell tables disagree on their length');
}

const payload = {
  cols: data.JELLY_COLS,
  rows: data.JELLY_ROWS,
  obstructionSlots: data.JELLY_OBSTRUCTION_SLOTS,
  cells: {
    names: data.CELL_NAMES.map((name) => name.replace(/_/g, ' ')),
    baseCooldowns: data.CELL_BASE_COOLDOWNS,
    baseDamage: data.CELL_BASE_DAMAGE,
    footprints: data.CELL_NAMES.map((_, type) => data.cellFootprint(type)),
    projectiles: data.CELL_NAMES.map((_, type) => data.jellyProjectileData(type)),
  },
  plots: data.jellySlotPlots(),
  // The purchase order lists future upgrades the game's `JellyUPG` table does not
  // define yet; only the rows with data are extracted.
  upgrades: data
    .jellyUpgradeOrder()
    .map((id) => data.jellyUpgradeData(Number(id)))
    .filter(Boolean),
  upgradeOrder: data.jellyUpgradeOrder().map(Number),
  rewards: data.jellyRewards().map((reward) => ({
    name: reward.name,
    description: reward.description,
    value: reward.value,
  })),
  // `AtomInfo[15]` is Sulfur, which boosts Jelly Operator Bloodcell gain by
  // column 4 percent a level.
  atomJellyPerLevel: Number(AtomInfo[15][4]),
  generatedFrom: {
    repo: 'Corgan/idleon-research-optimizer',
    files: [
      'js/stats/data/w7/jelly-operator.js',
      'js/stats/data/game/customlists.js',
      'js/stats/data/game/hardcoded.js',
    ],
  },
};
writeFileSync(
  new URL('../crates/core/data/jelly.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`,
);
console.log(
  `jelly: ${cells} cells, ${payload.plots.length} plots, ${payload.upgrades.length} upgrades, ${payload.rewards.length} rewards`,
);
