// Read, bump or set the app version (`[workspace.package] version` in the root
// Cargo.toml). The release workflows call it; see docs/release.md.
//
// Usage:
//   node script/version.mjs                          # print the current version
//   node script/version.mjs next <major|minor|patch> # print the bumped version
//   node script/version.mjs next <X.Y.Z>             # check X.Y.Z is newer, print it
//   node script/version.mjs set <version>            # write it to Cargo.toml and Cargo.lock
import { readVersion, writeVersion } from './lib/cargo-version.mjs';

const SEMVER = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/;
const STABLE = /^\d+\.\d+\.\d+$/;

function fail(message) {
  console.error(message);
  process.exit(1);
}

function usage() {
  fail('usage: node script/version.mjs [next <major|minor|patch|X.Y.Z> | set <version>]');
}

function parse(version) {
  const match = version.match(SEMVER);
  if (!match) fail(`not a version: ${version}`);
  return { core: match.slice(1, 4).map(Number), pre: match[4] ?? null };
}

// The release after `current`. `main` only ever holds a stable version: a
// nightly stamps its prerelease into the build, never into a commit.
function next(current, part) {
  const { core, pre } = parse(current);
  if (pre) fail(`the current version ${current} is a prerelease`);
  const [major, minor, patch] = core;
  if (part === 'major') return `${major + 1}.0.0`;
  if (part === 'minor') return `${major}.${minor + 1}.0`;
  if (part === 'patch') return `${major}.${minor}.${patch + 1}`;
  if (!STABLE.test(part)) fail(`expected major, minor, patch or X.Y.Z, got ${part}`);
  const order = parse(part)
    .core.map((value, index) => value - core[index])
    .find((delta) => delta !== 0);
  if (!(order > 0)) fail(`${part} is not newer than ${current}`);
  return part;
}

const [command, value, ...rest] = process.argv.slice(2);
if (rest.length > 0) usage();
if (command === undefined) {
  console.log(readVersion());
} else if (command === 'next' && value) {
  console.log(next(readVersion(), value));
} else if (command === 'set' && value) {
  parse(value);
  writeVersion(value);
  console.log(value);
} else {
  usage();
}
