# Releases

## Decision

From [`plan.md`](plan.md) §2:

> **Distribution decision (2026-11-14).** The app is distributed through GitHub only: unsigned
> builds for macOS, Linux and Windows are attached to this repository's GitHub Releases, and there
> is no other channel. A release bundles the game artwork under `assets/upstream/` with its credit
> note (`assets/upstream/README.md`). No written permission is sought from the upstream
> calculator's author, who is another community tool author and not the game's owner; ports stay
> fresh rewrites with source references, as above.

No website, package manager, app store or update check: the app makes no network calls
([`plan.md`](plan.md) §5.5).

## Versions

The version is `[workspace.package] version` in the root `Cargo.toml`; every crate inherits it.
`script/version.mjs` reads it, computes the next one and writes it (to `Cargo.toml` and to the
workspace members' entries in `Cargo.lock`, exactly the lines cargo would change).

| Channel | Version | Tag | GitHub release | Commit on `main` |
|---|---|---|---|---|
| Release | `X.Y.Z`, bumped by hand | `vX.Y.Z` on the bump commit | latest release | `chore(release): vX.Y.Z` |
| Nightly | next patch + `-beta.<run>`, e.g. `0.1.1-beta.12` | `v0.1.1-beta.12` on the built commit | prerelease | none |

`main` therefore always holds the last stable version, and a beta always sorts before the patch
release that follows it.

## Cutting a release

Actions → **Release** → Run workflow, on `main`, with:

- `bump`: `patch`, `minor` or `major`; or
- `version`: an exact `X.Y.Z` newer than the current one (it wins over `bump`).

[`release.yml`](../.github/workflows/release.yml) then:

1. computes the version and fails if its tag already exists;
2. builds and packages every platform from the dispatched commit with the version stamped in
   ([`build.yml`](../.github/workflows/build.yml));
3. only if all three built: commits the bump on top of the dispatched commit, tags it, and pushes
   both to `main` atomically — if `main` moved during the build the push is rejected and nothing is
   published, so dispatch again;
4. publishes the GitHub release with the archives, `SHA256SUMS`, the first-run notes
   ([`.github/release-notes.md`](../.github/release-notes.md)) and GitHub's generated changelog.

If step 4 fails after the push, the tag exists without a release: create it by hand with
`gh release create vX.Y.Z --verify-tag …` or delete the tag and dispatch again.

### Setup: the deploy key

The `main` ruleset requires a pull request, and lets only deploy keys bypass it. The release job
pushes with the private half of a write-enabled deploy key stored as the repository secret
`RELEASE_DEPLOY_KEY`. To rotate it:

```console
ssh-keygen -t ed25519 -N '' -C 'release workflow' -f release_key
gh repo deploy-key add release_key.pub --allow-write --title 'Release workflow'
gh secret set RELEASE_DEPLOY_KEY < release_key
rm release_key release_key.pub
```

Then delete the old key under Settings → Deploy keys.

## Nightly betas

[`nightly.yml`](../.github/workflows/nightly.yml) runs every day at 03:00 UTC (and by hand from
the Actions tab). It skips the night when the head of `main` already carries a tag, so a night
with no new commits, or one right after a release, publishes nothing. Otherwise it builds
`main` as the next patch's beta, tags the commit through the release API (no commit, no deploy
key) and publishes a prerelease. Only the five newest betas are kept; older ones and their tags
are deleted. Stable releases are never touched.

## What a release contains

Every archive holds a release build without the automation bridge
(`cargo build --release --locked -p idleon-optimizer --no-default-features`), with the ported
formulas and extracted tables compiled in, and the game artwork (`assets/upstream/`, with
`manifest.json` and the credit note `README.md`) beside the binary, where the app looks first.
[`script/package.mjs`](../script/package.mjs) assembles them and fails if any artwork file or the
credit note is missing.

| Platform | Archive | Layout | Built on |
|---|---|---|---|
| macOS 12+ | `idleon-optimizer-<version>-macos-universal.zip` | `Idleon Optimizer.app`, artwork in `Contents/MacOS/assets/upstream`, ad-hoc signed | `macos-latest`, `aarch64` and `x86_64` combined with `lipo` |
| Linux | `idleon-optimizer-<version>-linux-x86_64.tar.gz` | folder with `idleon-optimizer`, `assets/`, `README.txt` | `ubuntu-22.04` (glibc 2.35) |
| Windows | `idleon-optimizer-<version>-windows-x86_64.zip` | folder with `idleon-optimizer.exe`, `assets/` | `windows-latest` (MSVC) |

`SHA256SUMS` beside them lists every archive's checksum.

- **Unsigned.** No Developer ID, notarization or Authenticode. The macOS bundle carries only the
  identity-less ad-hoc signature (`codesign --sign -`), without which Apple silicon refuses to
  launch it.
- **No console on Windows.** A Windows release build uses the GUI subsystem; warnings and panics
  go to the log file below.
- **No builder paths.** The repository-root asset lookup is debug-only, so a release binary does
  not carry the checkout path. Dependency panic locations still name the runner's cargo registry.
- **Linux packages.** The `apt-get` line in `build.yml` is what the runner needs to build. The
  Linux job summary prints the binary's `ldd` output: what a user's machine must provide.

## First run

The same notes ship in every release ([`.github/release-notes.md`](../.github/release-notes.md)).

- **macOS**: the app is unsigned, so Gatekeeper blocks the first launch. Open it once, then System
  Settings → Privacy & Security → **Open Anyway** (macOS 15+); on older macOS, Control-click →
  Open. Alternative: `xattr -dr com.apple.quarantine "/Applications/Idleon Optimizer.app"`.
- **Windows**: SmartScreen shows "Windows protected your PC" for an unsigned program; **More info**
  → **Run anyway**. Keep the `assets` folder next to the `.exe`.
- **Linux**: extract, keep `assets/` next to the binary, run `./idleon-optimizer`. Needs a Vulkan
  driver and a Wayland or X11 session. Built on Ubuntu 22.04 (glibc 2.35); older distributions are
  not supported.

The session file and the log (`idleon-optimizer.log`) live in the platform app-data folder;
deleting the app does not delete them:

| Platform | Folder |
|---|---|
| macOS | `~/Library/Application Support/idleon-optimizer` |
| Linux | `$XDG_DATA_HOME/idleon-optimizer` (default `~/.local/share/idleon-optimizer`) |
| Windows | `%LOCALAPPDATA%\idleon-optimizer\data` |

## What is verified

- macOS: packaged locally, signature verified with `codesign --verify --deep --strict`, launched
  from the extracted archive.
- Linux and Windows: built and packaged by `build.yml` on the pull request that added it; not
  launched. The test suite still runs on macOS only (`ci.yml`): the 1e-9 oracle comparisons have
  never run against Linux or Windows maths libraries.

## Follow-ups

- App icons (macOS `.icns`, Windows resource through a build script).
- Installers: `.dmg`, `.msi`, AppImage.
- ARM Linux and ARM Windows builds.
- The test suite on Linux and Windows.
- `--remap-path-prefix` for the cargo registry paths in dependency panic messages.
- Owner-only permissions (`0700`/`0600`) for the app-data folder on Linux, where another local
  user could otherwise read the saved account.
