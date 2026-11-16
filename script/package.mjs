// Package a release build of the app, with the game artwork and its credit
// note, into one archive per platform under `target/dist/`. It does not build,
// sign with an identity or upload; see docs/release.md.
//
// Usage:
//   node script/package.mjs [--target <rust-triple>] [--arch <label>]
//
// The binary is `target/<triple>/release/` with `--target`, `target/release/`
// otherwise. `--arch` names the archive's architecture instead of the target's
// (the release workflow packages a combined macOS binary as `universal`).
//
// Output: the archive, plus `<archive>.sha256` in `sha256sum` format; the
// release jobs concatenate those into one SHA256SUMS.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  appendFileSync,
  chmodSync,
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { join } from 'node:path';
import { readVersion } from './lib/cargo-version.mjs';

const root = join(import.meta.dirname, '..');
const argv = process.argv.slice(2);
const options = {};
for (let index = 0; index < argv.length; index += 2) {
  const [name, value] = [argv[index], argv[index + 1]];
  if (!['--target', '--arch'].includes(name) || !value) {
    console.error('usage: node script/package.mjs [--target <rust-triple>] [--arch <label>]');
    process.exit(1);
  }
  options[name.slice(2)] = value;
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

const target = options.target ?? null;
const platform = target
  ? target.includes('-apple-darwin')
    ? 'darwin'
    : target.includes('-windows-')
      ? 'win32'
      : 'linux'
  : process.platform;
const arch =
  options.arch ?? target?.split('-')[0] ?? { x64: 'x86_64', arm64: 'aarch64' }[process.arch] ?? process.arch;
const version = readVersion();
const executable = platform === 'win32' ? 'idleon-optimizer.exe' : 'idleon-optimizer';
const binary = join(root, 'target', ...(target ? [target] : []), 'release', executable);
if (!existsSync(binary)) {
  fail(`${binary} is missing; build it first: cargo build --release -p idleon-optimizer --no-default-features${target ? ` --target ${target}` : ''}`);
}

const artwork = join(root, 'assets/game');
const dist = join(root, 'target/dist');
mkdirSync(dist, { recursive: true });

function countFiles(dir) {
  return readdirSync(dir, { recursive: true, withFileTypes: true }).filter((entry) => entry.isFile()).length;
}

// The artwork under `beside`, where `assets.rs` looks first. The credit note
// must ship with it.
function copyArtwork(beside) {
  const copy = join(beside, 'assets/game');
  cpSync(artwork, copy, { recursive: true });
  const [source, copied] = [countFiles(artwork), countFiles(copy)];
  if (source !== copied) fail(`copied ${copied} of ${source} artwork files`);
  for (const file of ['manifest.json', 'README.md']) {
    if (!existsSync(join(copy, file))) fail(`the artwork copy has no ${file}`);
  }
}

// A clean staging folder for `name` under `target/dist/`.
function stage(name) {
  const dir = join(dist, name);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  return dir;
}

const run = (command, args) => execFileSync(command, args, { cwd: dist, stdio: 'inherit' });
const base = `idleon-optimizer-${version}`;
let archive;

if (platform === 'darwin') {
  const app = 'Idleon Optimizer.app';
  const macos = join(stage(app), 'Contents/MacOS');
  mkdirSync(macos, { recursive: true });
  copyFileSync(binary, join(macos, executable));
  chmodSync(join(macos, executable), 0o755);
  writeFileSync(join(dist, app, 'Contents/Info.plist'), infoPlist());
  // Resources, not MacOS: codesign seals data in MacOS as nested code, in
  // xattrs that tools other than ditto drop on extraction.
  copyArtwork(join(dist, app, 'Contents/Resources'));
  // Identity-less ad-hoc signature: Apple silicon refuses to launch an app
  // whose signature no longer matches its contents. Not a Developer ID.
  run('codesign', ['--force', '--deep', '--sign', '-', app]);
  run('codesign', ['--verify', '--deep', '--strict', app]);
  archive = `${base}-macos-${arch}.zip`;
  rmSync(join(dist, archive), { force: true });
  run('ditto', ['-c', '-k', '--norsrc', '--keepParent', app, archive]);
} else if (platform === 'linux') {
  const name = `${base}-linux-${arch}`;
  const dir = stage(name);
  copyFileSync(binary, join(dir, executable));
  chmodSync(join(dir, executable), 0o755);
  copyArtwork(dir);
  writeFileSync(join(dir, 'README.txt'), linuxReadme());
  archive = `${name}.tar.gz`;
  run('tar', ['-czf', archive, name]);
} else if (platform === 'win32') {
  const name = `${base}-windows-${arch}`;
  const dir = stage(name);
  copyFileSync(binary, join(dir, executable));
  copyArtwork(dir);
  archive = `${name}.zip`;
  // Windows' own bsdtar writes a zip with `-a`; Git's GNU tar, which may come
  // first on PATH, cannot.
  run(join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe'), ['-a', '-cf', archive, name]);
} else {
  fail(`unsupported platform ${platform}`);
}

const digest = createHash('sha256').update(readFileSync(join(dist, archive))).digest('hex');
writeFileSync(join(dist, `${archive}.sha256`), `${digest}  ${archive}\n`);
const megabytes = (statSync(join(dist, archive)).size / 1024 / 1024).toFixed(1);
console.log(`${join(dist, archive)} (${megabytes} MB)`);
if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, `- \`${archive}\`: ${megabytes} MB\n`);
}

function infoPlist() {
  const entries = {
    CFBundleName: 'Idleon Optimizer',
    CFBundleDisplayName: 'Idleon Optimizer',
    CFBundleIdentifier: 'io.github.mastro993.idleon-optimizer',
    CFBundleExecutable: executable,
    CFBundlePackageType: 'APPL',
    CFBundleInfoDictionaryVersion: '6.0',
    CFBundleVersion: version,
    CFBundleShortVersionString: version,
    LSMinimumSystemVersion: '12.0',
  };
  const lines = Object.entries(entries).map(
    ([key, value]) => `  <key>${key}</key>\n  <string>${value}</string>`,
  );
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
${lines.join('\n')}
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
`;
}

function linuxReadme() {
  return `Idleon Optimizer ${version} for Linux (${arch})

Run ./${executable} from this folder; keep assets/ next to it.

Built on Ubuntu 22.04 (glibc 2.35); older distributions are not supported.
It needs a Vulkan driver and a Wayland or X11 session, plus fontconfig,
freetype and xkbcommon, which desktop distributions install by default.

The artwork under assets/game/ is from Legends of Idleon by LavaFlame2;
see assets/game/README.md. First-run notes for every platform:
https://github.com/mastro993/idleon-optimizer/blob/main/docs/release.md
`;
}
