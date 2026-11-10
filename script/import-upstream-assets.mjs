// Copy the upstream image tree into the committed `assets/upstream/` folder.
//
// The images are game artwork from Legends of Idleon, not original work of the
// upstream repository, so the copy is committed and shipped. The folder keeps
// the upstream layout plus a manifest and a credit file; regenerating it is a
// normal reviewed change.
//
// Usage:
//   node script/import-upstream-assets.mjs --from <checkout> --dry-run
//   node script/import-upstream-assets.mjs --from <checkout>
//   node script/import-upstream-assets.mjs --from <checkout> --force
//   node script/import-upstream-assets.mjs --verify [--from <checkout>] [--strict]
//
// `--strict` exits non-zero when the upstream checkout references an asset it
// does not ship; the default reports those gaps without failing the import.
//
// The app reads assets/upstream/manifest.json: every entry has a path relative
// to the upstream assets folder, a byte count and a sha256. A reference the
// manifest cannot resolve renders a labelled placeholder instead of a broken
// image.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';

const argv = process.argv.slice(2);
const flag = (name, fallback = null) => {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : fallback;
};
const has = (name) => argv.includes(name);
const dryRun = has('--dry-run');
const force = has('--force');
const verifyOnly = has('--verify');
const strict = has('--strict');
const from = flag('--from');
const toArgument = flag('--to');
const known = new Set(['--from', '--to', '--dry-run', '--force', '--verify', '--strict']);
for (let index = 0; index < argv.length; index += 1) {
  const argument = argv[index];
  if (!argument.startsWith('--')) continue;
  if (!known.has(argument)) {
    console.error(`unknown option ${argument}`);
    process.exit(1);
  }
  if (argument === '--from' || argument === '--to') index += 1;
}

const usage =
  'usage: node script/import-upstream-assets.mjs --from <upstream-checkout> [--dry-run] [--force]\n' +
  '       node script/import-upstream-assets.mjs --verify [--from <upstream-checkout>] [--strict]';

if (!from && !verifyOnly) {
  console.error(usage);
  process.exit(1);
}
if ((argv.includes('--from') && !from) || (argv.includes('--to') && !toArgument)) {
  console.error(usage);
  process.exit(1);
}
const to = resolve(toArgument ?? join(import.meta.dirname, '..', 'assets', 'upstream'));

/** Whether `path` is `parent` or lives under it. */
function inside(path, parent) {
  const rel = relative(parent, path);
  return rel === '' || (!rel.startsWith('..') && !isAbsolute(rel));
}

if (from && (inside(to, resolve(from)) || inside(resolve(from), to))) {
  console.error('the source checkout and the target folder must not contain each other');
  process.exit(1);
}
if (from && (!existsSync(join(from, 'index.html')) || !existsSync(join(from, 'assets')))) {
  console.error(`${from} does not look like the upstream checkout (missing index.html or assets/)`);
  process.exit(1);
}
if (!verifyOnly && existsSync(to) && !force && !dryRun) {
  console.error(`${to} already exists; pass --force to refresh it`);
  process.exit(1);
}

const bytes = (size) => `${(size / 1024 / 1024).toFixed(1)} MB`;
const sha256 = (data) => createHash('sha256').update(data).digest('hex');

/** Every file under `dir`, sorted, as paths relative to `base`. */
function files(dir, base = dir) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) =>
    a.name.localeCompare(b.name),
  )) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...files(path, base));
    else if (entry.isFile()) {
      out.push({ path: relative(base, path).split(sep).join('/'), absolute: path });
    }
  }
  return out;
}

/** The upstream sources that can reference an asset. */
function sources(root) {
  const out = [];
  const add = (path) => {
    if (existsSync(path) && statSync(path).isFile()) out.push(path);
  };
  for (const name of readdirSync(root).sort()) {
    if (name.endsWith('.html') || name === 'script.js') add(join(root, name));
  }
  for (const folder of ['js', 'css']) {
    const dir = join(root, folder);
    if (existsSync(dir)) out.push(...files(dir).map((file) => file.absolute));
  }
  return out;
}

const ASSET_REFERENCE =
  /assets\/[A-Za-z0-9_@%+=,~\-./${}'"+\s]*?\.(?:png|jpe?g|gif|webp|svg|json)/g;

/** Static asset references, dynamic templates and the ones the checkout lacks. */
function references(root) {
  const staticRefs = new Set();
  const dynamic = new Set();
  for (const source of sources(root)) {
    const text = readFileSync(source, 'utf8');
    for (const match of text.matchAll(ASSET_REFERENCE)) {
      const reference = match[0];
      if (/[$'"`+{}]/.test(reference)) dynamic.add(reference);
      else staticRefs.add(reference.replace(/[?#].*$/, ''));
    }
  }
  const missing = [...staticRefs].filter((reference) => !existsSync(join(root, reference)));
  return {
    static: staticRefs.size,
    verified: staticRefs.size - missing.length,
    missing: missing.sort(),
    dynamic: [...dynamic].sort(),
  };
}

/** `remote@commit` when the source is a git checkout, else its absolute path. */
function provenance(root) {
  try {
    const commit = execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim();
    const remote = execFileSync('git', ['-C', root, 'config', '--get', 'remote.origin.url'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    })
      .trim()
      .replace(/\.git$/, '');
    return remote ? `${remote}@${commit}` : commit;
  } catch {
    return resolve(root);
  }
}

/** Credit note committed next to the manifest. */
const ATTRIBUTION = `# Upstream artwork

Artwork from the game *Legends of Idleon* (LavaFlame2), imported with the folder
layout of the Idleon Research Optimizer by Jonathan Rock
(<https://github.com/Corgan/idleon-research-optimizer>). The images are game
assets, not original work of the upstream repository.

Generated by \`script/import-upstream-assets.mjs\`; do not edit by hand.
\`manifest.json\` lists every file with its byte count and sha256.
`;

/** Read the target's manifest and check every entry against the disk. */
function verify(target) {
  const manifestPath = join(target, 'manifest.json');
  if (!existsSync(manifestPath)) {
    return { error: `${manifestPath} does not exist; run the import first` };
  }
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  } catch (error) {
    return { error: `${manifestPath} is not valid JSON: ${error.message}` };
  }
  if (!Array.isArray(manifest.files)) {
    return { error: `${manifestPath} has no files array` };
  }
  const missing = [];
  const mismatched = [];
  let total = 0;
  for (const entry of manifest.files) {
    if (
      typeof entry?.path !== 'string' ||
      !Number.isFinite(entry.bytes) ||
      typeof entry.sha256 !== 'string'
    ) {
      return { error: `${manifestPath} has a malformed file entry` };
    }
    const path = join(target, entry.path);
    if (!existsSync(path)) {
      missing.push(entry.path);
      continue;
    }
    const data = readFileSync(path);
    total += data.length;
    if (data.length !== entry.bytes || sha256(data) !== entry.sha256) {
      mismatched.push(entry.path);
    }
  }
  return {
    manifest,
    missing,
    mismatched,
    total,
    ok: missing.length === 0 && mismatched.length === 0,
  };
}

function printReferences(report) {
  console.log(
    `references: ${report.static} static (${report.verified} present, ${report.missing.length} missing), ` +
      `${report.dynamic.length} dynamic templates`,
  );
  for (const reference of report.missing) console.log(`  missing upstream reference: ${reference}`);
  for (const reference of report.dynamic) console.log(`  dynamic upstream reference: ${reference}`);
}

if (verifyOnly) {
  const result = verify(to);
  if (result.error) {
    console.error(result.error);
    process.exit(1);
  }
  if (!result.ok) {
    for (const path of result.missing) console.error(`  missing file: ${path}`);
    for (const path of result.mismatched) console.error(`  hash or size mismatch: ${path}`);
    console.error(`${to} failed verification (${result.manifest.files.length} manifest entries)`);
    process.exit(1);
  }
  console.log(`verified ${result.manifest.files.length} files (${bytes(result.total)}) in ${to}`);
  if (from) {
    const report = references(from);
    printReferences(report);
    if (strict && report.missing.length > 0) {
      console.error(`${report.missing.length} upstream references are missing from the checkout`);
      process.exit(1);
    }
  }
  process.exit(0);
}

const assetFiles = files(join(from, 'assets'), join(from, 'assets'));
const totalBytes = assetFiles.reduce((sum, file) => sum + statSync(file.absolute).size, 0);
const source = provenance(from);
const referenceReport = references(from);

if (dryRun) {
  console.log(`source: ${source}`);
  console.log(`target: ${to}`);
  console.log(`files (${assetFiles.length}, ${bytes(totalBytes)}):`);
  for (const file of assetFiles) {
    console.log(`  ${file.path}\t${statSync(file.absolute).size}`);
  }
  console.log(`total: ${assetFiles.length} files, ${bytes(totalBytes)} (${totalBytes} bytes)`);
  printReferences(referenceReport);
  console.log(`would copy ${assetFiles.length} files (${bytes(totalBytes)}) to ${to}`);
  if (strict && referenceReport.missing.length > 0) process.exit(1);
  process.exit(0);
}

// A refresh replaces the tree so deleted upstream files do not linger. Only a
// previous import (it has a manifest) or an empty folder may be replaced.
if (existsSync(to)) {
  const previous = existsSync(join(to, 'manifest.json'));
  if (!previous && readdirSync(to).length > 0) {
    console.error(`${to} is not an asset folder; refusing to replace its contents`);
    process.exit(1);
  }
  rmSync(to, { recursive: true, force: true });
}
mkdirSync(to, { recursive: true });

const manifestFiles = [];
for (const file of assetFiles) {
  const data = readFileSync(file.absolute);
  const target = join(to, file.path);
  mkdirSync(dirname(target), { recursive: true });
  cpSync(file.absolute, target);
  manifestFiles.push({ path: file.path, bytes: data.length, sha256: sha256(data) });
}

const manifest = {
  version: 1,
  source,
  generatedAt: new Date().toISOString(),
  files: manifestFiles,
  totalFiles: manifestFiles.length,
  totalBytes,
  references: referenceReport,
};
writeFileSync(join(to, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
writeFileSync(join(to, 'README.md'), ATTRIBUTION);

const result = verify(to);
if (!result.ok) {
  for (const path of result.missing ?? []) console.error(`  missing file after copy: ${path}`);
  for (const path of result.mismatched ?? []) console.error(`  hash or size mismatch: ${path}`);
  console.error('the copy does not match the manifest it wrote');
  process.exit(1);
}

console.log(`copied ${manifest.totalFiles} files (${bytes(totalBytes)}) to ${to}`);
console.log(`source: ${source}`);
console.log(`manifest: ${join(to, 'manifest.json')} (${manifest.totalFiles} hashes)`);
console.log(`verified ${result.manifest.files.length} files against the manifest`);
printReferences(referenceReport);
console.log('reminder: keep the credit in README.md with the artwork.');
if (strict && referenceReport.missing.length > 0) process.exit(1);
