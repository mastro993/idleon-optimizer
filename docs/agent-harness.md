# Agent harness notes

Notes for running this repo with Pi and background subagents. Nothing here is required to build or
run the app.

## Background children need `@earendil-works/chord`

`pi-subagents` launches each background child from a separate Node process and hands it an alias map
for the host packages. From Pi 0.85 onward that map includes `@earendil-works/chord` and
`@earendil-works/chord/context`, and the preflight fails closed when the host package cannot provide
them:

```
Background children require the host npm package (@earendil-works/pi-coding-agent)
with its dependencies; /opt/homebrew/Cellar/pi-coding-agent/<version>/libexec/lib/node_modules/
@earendil-works/pi-coding-agent does not provide @earendil-works/chord,
@earendil-works/chord/context.
```

The Homebrew/npm build of Pi does not vendor chord, so every child launch fails until it is present
next to the host. `script/install-pi-chord.mjs` fixes that:

```console
node script/install-pi-chord.mjs --dry-run   # show what it would do
node script/install-pi-chord.mjs             # install it
```

It finds the newest installed `@earendil-works/pi-coding-agent` (env `PI_CODING_AGENT_ROOT` first,
then `npm root -g`, then the Homebrew Cellar), reads the host version, and installs the matching
`@earendil-works/chord` into that package's `node_modules/@earendil-works/chord` by unpacking the
npm tarball. Matching the host version is deliberate: chord pins `@earendil-works/pi-schema` and
`@earendil-works/pi-telemetry`, and those already sit next to the host at the same version.

It is idempotent and needs no restart: the next child launch resolves the alias.

**After `brew upgrade pi-coding-agent` the Cellar directory is replaced**, so re-run the script. If
`--dry-run` reports the package already installed, nothing is pending.

To undo it, delete the installed copy:

```console
rm -rf "$(npm root -g)/@earendil-works/pi-coding-agent/node_modules/@earendil-works/chord"
```

## One window per lane

A single shared bridge endpoint means every agent drives the same window: one
lane's clicks land in another lane's state, focus moves under someone else's
hands, and screenshots race. Both sides of gpui-mcp can scope discovery to a
private directory, so each lane owns an instance:

- the app takes `--endpoint-dir <absolute dir>` and publishes only there;
- the server takes `--endpoint-dir <same dir>` and lists only that app.

`script/mcp-drive.mjs` and `script/mcp-smoke.mjs --spawn` do this for you: they
create a temp directory, launch `target/debug/idleon-optimizer --endpoint-dir …`,
wait for the endpoint file, start `gpui-mcp` against it, run, then stop the app.
Three lanes can run concurrently with no interference:

```console
node script/mcp-drive.mjs --shot /tmp/lane-a.png &
node script/mcp-drive.mjs --shot /tmp/lane-b.png &
node script/mcp-smoke.mjs --spawn --shot /tmp/lane-c.png &
wait
```

Use `--attach` (drive) or drop `--spawn` (smoke) to drive a window you started
yourself through the default discovery directory. `--keep` leaves a spawned
window running so you can look at it.

Two consequences worth knowing:

- Every instance activates itself on open, so the last one launched takes focus.
  The harnesses re-activate their own pid before frame-dependent calls and retry
  a `frame wait timed out` once.
- A lane must not `pkill -f target/debug/idleon-optimizer`: that kills other
  lanes' windows. Let the harness stop the instance it started.

## Verifying a lane without the MCP client

The app exposes itself through [gpui-mcp](https://github.com/mastro993/gpui-mcp); both harnesses in
`script/` are plain Node clients, so they work from a terminal, a subagent or CI:

```console
cargo build -p idleon-optimizer
./target/debug/idleon-optimizer &          # leave it running
node script/mcp-smoke.mjs --shot /tmp/ui.png
node script/mcp-drive.mjs
pkill -f target/debug/idleon-optimizer
```

`mcp-drive.mjs` exits non-zero when a check fails, which makes it usable as a gate for a lane that
touches the UI.
