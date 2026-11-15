// Which upstream commit a generated file came from.
//
// Every oracle and extract script calls `recordProvenance` after it writes an
// output. The entry lands in fixtures/PROVENANCE.json, keyed by the output's
// repository-relative path, so a fixture's shape never changes and a reviewer
// can see at a glance which files were generated from which upstream commit.
// Usage: node script/lib/provenance.mjs --self-test
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { basename, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(import.meta.dirname, '..', '..');
const ledger = join(root, 'fixtures/PROVENANCE.json');

function git(source, ...args) {
  return execFileSync('git', ['-C', source, ...args], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
  });
}

/** `remote@commit` when `source` is a git checkout, else its absolute path. */
export function provenance(source) {
  try {
    const commit = git(source, 'rev-parse', 'HEAD').trim();
    const remote = git(source, 'config', '--get', 'remote.origin.url')
      .trim()
      .replace(/\.git$/, '');
    return remote ? `${remote}@${commit}` : commit;
  } catch {
    return resolve(source);
  }
}

/** Whether the checkout has uncommitted changes; `null` when it is not one. */
function dirty(source) {
  try {
    const status = git(source, 'status', '--porcelain', '--untracked-files=no');
    // The generators drop a `package.json` module marker into the checkout.
    return status.split('\n').some((line) => line.trim() && !line.endsWith('package.json'));
  } catch {
    return null;
  }
}

/**
 * Record that `output` (a path or file URL inside this repository) was just
 * generated from the upstream checkout at `source` by the calling script.
 */
export function recordProvenance(output, source, script = basename(process.argv[1])) {
  const path = output instanceof URL ? fileURLToPath(output) : output;
  const entries = JSON.parse(readFileSync(ledger, 'utf8'));
  entries[relative(root, path)] = {
    script: `script/${script}`,
    upstream: provenance(source),
    dirty: dirty(source),
  };
  const sorted = Object.fromEntries(Object.entries(entries).sort(([a], [b]) => a.localeCompare(b)));
  writeFileSync(ledger, `${JSON.stringify(sorted, null, 2)}\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url) && process.argv.includes('--self-test')) {
  const here = provenance(root);
  if (!/@[0-9a-f]{40}$|^[0-9a-f]{40}$/.test(here)) throw new Error(`unexpected provenance: ${here}`);
  console.log('ok');
}
