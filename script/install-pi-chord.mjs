// Installs @earendil-works/chord next to the pi host, which is what
// pi-subagents' background runner needs for its alias map.
//
// pi >= 0.85 ships as a peer of chord but the Homebrew/npm package does not
// vendor it; without it every child launch fails with:
//   "Background children require the host npm package ... does not provide
//    @earendil-works/chord, @earendil-works/chord/context."
//
// Homebrew upgrades replace the Cellar directory, so re-run this after one.
// Idempotent: it exits when the package is already there at the host's version.
//
// Usage: node script/install-pi-chord.mjs [--dry-run]
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const dryRun = process.argv.includes('--dry-run');
const HOST = '@earendil-works/pi-coding-agent';
const CHORD = '@earendil-works/chord';

const readJson = (path) => JSON.parse(readFileSync(path, 'utf8'));

const candidates = () => {
  const found = [];
  if (process.env.PI_CODING_AGENT_ROOT) found.push(process.env.PI_CODING_AGENT_ROOT);
  try {
    const global = execFileSync('npm', ['root', '-g'], { encoding: 'utf8' }).trim();
    found.push(join(global, HOST));
  } catch {
    // npm may be absent; the Homebrew paths below still apply.
  }
  const globs = [
    '/opt/homebrew/Cellar/pi-coding-agent/*/libexec/lib/node_modules',
    '/usr/local/Cellar/pi-coding-agent/*/libexec/lib/node_modules',
    '/opt/homebrew/lib/node_modules',
    '/usr/local/lib/node_modules',
  ];
  for (const pattern of globs) {
    let entries = [];
    try {
      entries = execFileSync('sh', ['-c', `ls -d ${pattern}/${HOST} 2>/dev/null`], { encoding: 'utf8' })
        .split('\n')
        .filter(Boolean);
    } catch {
      entries = [];
    }
    found.push(...entries);
  }
  return found;
};

const hostRoot = candidates()
  .filter((path) => existsSync(join(path, 'package.json')))
  .map((path) => ({ path, manifest: readJson(join(path, 'package.json')) }))
  .filter(({ manifest }) => manifest.name === HOST)
  .sort((a, b) => b.manifest.version.localeCompare(a.manifest.version, undefined, { numeric: true }))[0];

if (!hostRoot) {
  console.error(`no installed ${HOST} found; set PI_CODING_AGENT_ROOT to its package directory`);
  process.exit(1);
}

const needsChord = () => {
  // pi-subagents drops the requirement only for 0.x hosts below 0.85.
  const stable = /^0\.(\d+)\.\d+$/.exec(hostRoot.manifest.version);
  return !(stable && Number(stable[1]) < 85);
};

if (!needsChord()) {
  console.log(`${HOST} ${hostRoot.manifest.version} predates chord; nothing to do`);
  process.exit(0);
}

const target = join(hostRoot.path, 'node_modules', CHORD);
const installed = existsSync(join(target, 'package.json')) ? readJson(join(target, 'package.json')) : null;
if (installed && installed.name === CHORD) {
  console.log(`${CHORD} ${installed.version} already installed for ${HOST} ${hostRoot.manifest.version}`);
  process.exit(0);
}

// Match the host's own version so chord's pinned pi-schema/pi-telemetry resolve
// to the copies the host already carries.
const version = hostRoot.manifest.version;
console.log(`installing ${CHORD}@${version} into ${target}`);
if (dryRun) {
  process.exit(0);
}

const work = mkdtempSync(join(tmpdir(), 'pi-chord-'));
try {
  const tarball = execFileSync('npm', ['pack', `${CHORD}@${version}`, '--silent', '--pack-destination', work], {
    encoding: 'utf8',
  })
    .trim()
    .split('\n')
    .pop();
  execFileSync('mkdir', ['-p', target]);
  execFileSync('tar', ['xzf', join(work, tarball), '-C', target, '--strip-components=1']);
  const result = readJson(join(target, 'package.json'));
  if (result.name !== CHORD) throw new Error(`unexpected package ${result.name}`);
  for (const subpath of ['dist/index.js', 'dist/node/context.js', 'dist/context.js']) {
    const probe = join(target, subpath);
    if (existsSync(probe)) console.log(`  provides ${subpath}`);
  }
  console.log(`done: ${CHORD} ${result.version}`);
} finally {
  rmSync(work, { recursive: true, force: true });
}
