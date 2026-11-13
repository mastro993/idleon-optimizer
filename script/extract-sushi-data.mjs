// Extracts the Sushi Station tables the Rust crate ships from the upstream JS
// repo. Usage: node script/extract-sushi-data.mjs <upstream-repo>
import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const source = process.argv[2];
if (!source) {
  console.error('usage: node script/extract-sushi-data.mjs <upstream-repo>');
  process.exit(1);
}

// Upstream files are ESM with .js extensions, so they need a module package.
const probe = spawnSync('node', ['-e', '1']);
if (probe.status !== 0) throw new Error('node is required');
writeFileSync(join(source, 'package.json'), JSON.stringify({ type: 'module' }));

const { SushiUPG, Research } = await import(
  join(source, 'js/stats/data/game/customlists.js')
);
const data = await import(join(source, 'js/stats/data/w7/sushi.js'));

const clean = (text) => text.replace(/_/g, ' ').replace(/\\'/g, "'");

const payload = {
  upgrades: SushiUPG.map((upg) => ({
    name: clean(upg[0]),
    maxLv: Number(upg[1]),
    costBase: Number(upg[2]),
    bonusPerLv: Number(upg[3]),
    costDN: Number(upg[4]),
    desc: clean(upg[5]),
  })),
  slotToUpg: data.SLOT_TO_UPG,
  tierToKnowledgeCat: data.TIER_TO_KNOWLEDGE_CAT,
  knowledgeCatDesc: data.KNOWLEDGE_CAT_DESC,
  knowledgeCatValue: data.KNOWLEDGE_CAT_VALUE,
  rogDesc: data.ROG_DESC,
  rogQty: data.ROG_BONUS_QTY,
  currencyPerTier: data.CURRENCY_PER_TIER,
  maxSlots: data.MAX_SLOTS,
  generatedFrom: { repo: 'Corgan/idleon-research-optimizer' },
};

if (payload.upgrades.length !== 46) throw new Error('expected 46 Sushi upgrades');
if (payload.slotToUpg.length !== payload.upgrades.length) {
  throw new Error('slot count and upgrade count disagree');
}
if (payload.rogDesc.length !== payload.rogQty.length) {
  throw new Error('RoG descriptions and quantities disagree');
}

writeFileSync(
  new URL('../crates/sushi/data/sushi.json', import.meta.url),
  `${JSON.stringify(payload, null, 1)}\n`
);
console.log(
  `wrote ${payload.upgrades.length} upgrades, ${payload.tierToKnowledgeCat.length} sushi tiers, ` +
    `${payload.knowledgeCatDesc.length} knowledge categories and ${payload.rogQty.length} RoG bonuses`
);
