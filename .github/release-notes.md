Download the archive for your platform below; `SHA256SUMS` lists their checksums. The builds are
unsigned, so the first launch needs one extra step:

- **macOS** (`macos-universal.zip`, Apple silicon and Intel): open the app once, then System
  Settings → Privacy & Security → **Open Anyway**. On macOS 14 or older, Control-click the app →
  Open.
- **Windows** (`windows-x86_64.zip`): extract the whole folder, run `idleon-optimizer.exe`, and on
  "Windows protected your PC" choose **More info** → **Run anyway**. Keep `assets` next to the
  `.exe`.
- **Linux** (`linux-x86_64.tar.gz`): extract, keep `assets` next to the binary, run
  `./idleon-optimizer`. Needs glibc 2.35+ (Ubuntu 22.04 or newer), a Vulkan driver and Wayland
  or X11.

Details: [docs/release.md](https://github.com/mastro993/idleon-optimizer/blob/main/docs/release.md).
The game artwork in `assets/upstream` belongs to Legends of Idleon and is credited in its
`README.md`.
