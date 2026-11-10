// Copy the upstream image tree into a local, gitignored folder.
//
// The upstream repository is all-rights-reserved, so these files are never
// committed or shipped. This script exists so a local build can render the same
// images from a checkout the operator is allowed to use.
//
// Usage:
//   node script/import-upstream-assets.mjs --from /path/to/idleon-research-optimizer --dry-run
//   node script/import-upstream-assets.mjs --from /path/to/idleon-research-optimizer
//   node script/import-upstream-assets.mjs --from ... --force
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';

const argv = process.argv.slice(2);
const flag = (name, fallback = null) => {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : fallback;
};
const dryRun = argv.includes('--dry-run');
const force = argv.includes('--force');
const from = flag('--from');
const to = flag('--to', join(import.meta.dirname, '..', 'assets', 'upstream'));
const folders = [
  'assets/combat-sim',
  'assets/jelly-operator',
  'assets/royal-guardian',
  'assets/sneaking',
  'assets/spelunking',
  'assets/worship-td',
];

if (!from) {
  console.error('usage: node script/import-upstream-assets.mjs --from <upstream-checkout> [--dry-run] [--force]');
  process.exit(1);
}
if (!existsSync(join(from, 'index.html')) || !existsSync(join(from, 'assets'))) {
  console.error(`${from} does not look like the upstream checkout (missing index.html or assets/)`);
  process.exit(1);
}
if (existsSync(to) && !force && !dryRun) {
  console.error(`${to} already exists; pass --force to refresh it`);
  process.exit(1);
}

function files(dir) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...files(path));
    else if (entry.isFile()) out.push(path);
  }
  return out;
}

const manifest = { source: from, generatedAt: new Date().toISOString(), files: [] };
let bytes = 0;
for (const folder of folders) {
  const source = join(from, folder);
  if (!existsSync(source)) {
    console.warn(`missing ${folder}; skipped`);
    continue;
  }
  for (const path of files(source)) {
    const size = statSync(path).size;
    const relativePath = relative(from, path);
    manifest.files.push({
      path: relativePath,
      bytes: size,
      sha256: createHash('sha256').update(readFileSync(path)).digest('hex'),
    });
    bytes += size;
    if (!dryRun) {
      const target = join(to, relativePath);
      mkdirSync(join(target, '..'), { recursive: true });
      cpSync(path, target);
    }
  }
}
manifest.totalBytes = bytes;
manifest.totalFiles = manifest.files.length;

if (dryRun) {
  console.log(`would copy ${manifest.totalFiles} files (${(bytes / 1024 / 1024).toFixed(1)} MB) to ${to}`);
  console.log(JSON.stringify(manifest.files.slice(0, 3), null, 2));
} else {
  mkdirSync(to, { recursive: true });
  writeFileSync(join(to, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
  console.log(`copied ${manifest.totalFiles} files (${(bytes / 1024 / 1024).toFixed(1)} MB) to ${to}`);
  console.log('reminder: these files are not licensed for redistribution; keep the folder gitignored.');
}
