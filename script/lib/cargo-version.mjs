// The app version: `[workspace.package] version` in the root Cargo.toml, which
// every crate under `crates/` inherits.
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '../..');
const manifest = join(root, 'Cargo.toml');
const lockfile = join(root, 'Cargo.lock');
// The `version` line inside the `[workspace.package]` table, before the next table.
const VERSION_LINE = /^(\[workspace\.package\][^[]*?^version = ")([^"]+)(")/m;

export function readVersion() {
  const match = readFileSync(manifest, 'utf8').match(VERSION_LINE);
  if (!match) throw new Error(`${manifest} has no [workspace.package] version`);
  return match[2];
}

// Write `version` to Cargo.toml and to the workspace members' entries in
// Cargo.lock, the only lines cargo itself would change. Editing the lock
// directly keeps cargo, and the GPUI submodule it needs, out of the commit job.
export function writeVersion(version) {
  const current = readVersion().replaceAll('.', '\\.');
  let lock = readFileSync(lockfile, 'utf8');
  for (const name of memberNames()) {
    // `\r?`: a Windows checkout may convert line endings.
    const entry = new RegExp(`(\\[\\[package\\]\\]\\r?\\nname = "${name}"\\r?\\nversion = ")${current}"`);
    if (!entry.test(lock)) throw new Error(`Cargo.lock has no ${name} ${readVersion()}`);
    lock = lock.replace(entry, `$1${version}"`);
  }
  writeFileSync(manifest, readFileSync(manifest, 'utf8').replace(VERSION_LINE, `$1${version}$3`));
  writeFileSync(lockfile, lock);
}

function memberNames() {
  return readdirSync(join(root, 'crates'), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => {
      const text = readFileSync(join(root, 'crates', entry.name, 'Cargo.toml'), 'utf8');
      const name = text.match(/^\[package\]\s*\nname = "([^"]+)"/m)?.[1];
      if (!name) throw new Error(`crates/${entry.name}/Cargo.toml has no package name`);
      return name;
    });
}
