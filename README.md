**English** | [한국어](README.ko.md)

# 🐾 Roamling

**A tiny companion that actually lives on your desktop.**

Roamling is a native companion runtime for macOS and Windows. Petdex-compatible
creatures roam across your monitors, avoid your pointer, let you catch and drag
them, and react to coding agents and configured work apps. Game and media
reactions remain future work.

The decisions a pet makes live in one Rust core that both platforms share; each
platform brings its own window, tray, and input. See `docs/windows.md`.

> Cute first. Useful second. Never annoying.

## Current status

The current desktop implementation includes:

- a native AppKit menu-bar app and transparent, non-activating overlay;
- Codex/Petdex v1 (8×9) and v2 (8×11) pet loading, plus custom animation
  definitions and graceful capability fallback;
- two original built-in mascots, **Bori** and **FatBori**, with authored
  idle/walk/sleep/caught/stretch/landing animation sets, selectable from the
  menu, plus a code-drawn emergency fallback;
- global desktop coordinates, display topology, hot-plug handling, and
  continuous cross-display paths;
- calmer wandering with visible idle pauses, shorter local trips, and more
  noticeable multi-display exploration;
- pointer awareness, capped evasion, direct body-click catching, drag, drop,
  cross-monitor dragging, and connected-edge escape when gently cornered;
- a live **Behavior Tuning…** panel for MVP 0/0.5 movement, pointer approach,
  and hit-region values, with persistent settings and one-click reset;
- permission-free idle detection plus sit, safe sleep-spot travel, sleep,
  wake, and stretch behavior at a reduced sleeping cadence;
- shared text-clearance ranking for roaming, work seats, and sleep spots when
  visual awareness is enabled; permission-free placement remains available;
- shared-edge margins and continuous monitor crossings; ordinary pointer tail
  wags only while seated, with working-agent reactions preserved;
- an opt-in Claude Code hook integration with a token-authenticated local
  receiver, coarse permission-free work-window placement, and start,
  attention, completion, and failure reactions;
- an opt-in Codex 0.147+ hook integration that preserves existing hooks and
  `notify`, plus shared multi-source attention, hysteresis, and reaction policy;
- a sprite-sized overlay that accepts a body click without a fast approach,
  including while moving or working; clicks outside the pet pass through;
- a native first-run guide, with one-time notices for meaningful usage changes
  and **Bori User Guide…** in the menu to reopen the basics;
- pure-logic tests for geometry, display paths, movement, pointer interaction,
  behavior transitions, attention, reactions, and pet animation fallback.

Accessibility/caret tracking and visual placement are implemented behind their
platform permissions/settings. Coarse visual sampling cannot identify every
glyph. See [behavior](docs/behavior-flow.md) and [capture](docs/capture.md).
Android service/overlay development and remaining device checks are tracked
separately in [the Android guide](docs/android.md).

## Build and run

Requirements: macOS 13 or newer, Swift 6, and the Rust toolchain. Build the shared
Rust library and generated bindings before building Swift.

```sh
./scripts/build-rust-core.sh
swift build
./scripts/test.sh
swift run Roamling
```

Create a local `.app` bundle:

```sh
./scripts/build-app.sh release
open build/Roamling.app
```

The bundle build requires a signing identity; it refuses ad-hoc signing by
default because that resets permissions across rebuilds. Copy
`scripts/signing.env.example` to `scripts/signing.env` (git-ignored)
and set `ROAMLING_CODESIGN_IDENTITY` to a code signing identity. A free
self-signed certificate works; the example file has the steps. The same variable
can be exported in the environment instead. See [release instructions](docs/release.md).

The tests use a dependency-free executable harness so they also run on minimal
Command Line Tools installations that do not ship a compatible XCTest runner.
They exit non-zero on any failed unit case. If a local CLT installation has a
compiler/SDK mismatch, point both scripts at a compatible installed SDK with
`ROAMLING_SWIFT_SDK=/path/to/MacOSX.sdk`.

Roamling checks these pet locations, in order:

```text
$ROAMLING_PET_PATH
~/Library/Application Support/Roamling/Pets
~/.codex/pets
~/.petdex/pets
```

`ROAMLING_PET_PATH` may point either to one package directory or to a directory
containing packages. A package contains `pet.json` and the referenced PNG or
WebP atlas. Bori is the default. Other built-ins and discovered Petdex-compatible
packages can be selected from the menu, and the choice persists across launches.

Claude Code integration is disabled until explicitly installed from
**Roamling → Claude Code → Install Integration…**. Installation preserves
existing `~/.claude/settings.json` values and hooks, creates a one-time backup,
and can be removed from the same menu. The receiver listens only on
`127.0.0.1` and does not store prompt text, tool input/output, transcripts, or
source code.

Codex integration is also opt-in at
**Roamling → Codex → Install Integration…**. It merges only Roamling handlers
into `~/.codex/hooks.json`, creates a one-time backup, and leaves
`~/.codex/config.toml`, existing `notify`, and sibling hooks unchanged. Restart
Codex after installation and approve its hook trust prompt. The Codex receiver
uses a separate authenticated `127.0.0.1` port and applies the same no-content-
storage rule.

## Windows

Download `Roamling-Setup.exe` from the
[latest release](https://github.com/creatorKoo/Roamling/releases/latest) and run
it. It installs per user, into `%LOCALAPPDATA%\Programs\Roamling`, and never
asks for administrator approval. `roamling.exe` from the same release is the
portable form: one file, no runtime to install, nothing outside Windows' own
DLLs.

**The installer is not yet code-signed**, so Windows will show
*"Windows protected your PC"* the first time. Choose **More info → Run anyway**.
Signing is on the list; until then this is the honest state of it. Updates are
not affected — Roamling replaces its own executable rather than re-running an
installer, so the warning appears once, on first install.

### Updates

Roamling checks for a new version at startup and once a day, downloads it in the
background, and puts it in place. **Downloading needs no dialog or restart prompt**:
the new version is simply the one that runs the next time Roamling starts. The
tray menu says so quietly when one is waiting.

A short guide appears on first launch and after meaningful usage changes, once per
guide revision. Bug-fix releases do not repeat it. See [guide maintenance](docs/usage-guide.md).

Every release is signed with an Ed25519 key whose public half is compiled into
the app, and both the version feed and the executable are checked against it
before anything is written. A build that cannot verify a signature refuses to
update rather than updating anyway. Turn the whole thing off with
**Automatic Updates** in the tray menu.

### Start at login

On by default: macOS registers the login item the first time the app runs,
and the Windows installer's "Start Roamling when I sign in" box is checked
unless you untick it. **Start at Login** in the tray menu turns it off or back
on. The OS keeps that setting, not Roamling: on macOS it also appears under
System Settings → General → Login Items, and on Windows it is the same registry
value the installer writes. Change it in either place and the menu shows the
new answer the next time you open it, and Roamling never turns it back on by
itself.

### Building on Windows

There is no Swift here; the Windows build is Rust all the way down. Requires the
[Rust toolchain](https://rustup.rs) and, for the installer,
[Inno Setup 6](https://jrsoftware.org/isinfo.php).

```powershell
.\scripts\test.ps1        # core, pet, agent, update, and shell tests
.\scripts\run.ps1         # stop, build, start -- a running copy locks its own exe
.\scripts\run.ps1 -Debug  # the same, with a console for the state log
```

## Repository guide

```text
Sources/RoamlingCore/   Swift reference implementation for differential tests
Sources/RoamlingPet/    Petdex/Codex manifests, atlas runtime, fallbacks
Sources/RoamlingSources/ activity adapters and local hook transport
Sources/RoamlingEngine/ macOS orchestration and Rust binding adapters
Sources/RoamlingMac/    AppKit display, pointer, overlay, and platform providers
Sources/RoamlingApp/    executable entry point
rust/roamling-core/     shared decisions, image decoding and palette logic
rust/roamling-agent/    Claude Code and Codex hooks, normalization, receiver
rust/roamling-pet/      built-in mascots and pet packages
rust/roamling-update/   version feed parsing and release signature checking
rust/roamling-win/      the Windows shell: window, tray, input, tick loop
rust/roamling-android/  Android bindings for the shared core and player
android/               Android app, overlay, service, and device tests
installer/roamling.iss  the Windows installer
Tests/                  pure and loader tests
docs/README.md          documentation map and current maintenance review
docs/history/research.md upstream/API research history
docs/architecture.md    boundaries, decisions, and milestone architecture
docs/history/mvp.md     completed MVP gates and acceptance records
docs/windows.md         current Windows operation and remaining checks
```

Roamling is an independent project and is not affiliated with OpenAI,
Anthropic, Petdex, or the comparison projects named in the research notes.

## License and contributions

Copyright (C) 2026 GooBeom Jeoung.

Roamling source code is licensed under the
[GNU General Public License v3.0 only](LICENSE). You may use, modify, distribute,
and sell the software under that license; covered derivative works must preserve
the same freedoms and provide their corresponding source.

**The built-in mascot Bori is not covered by the GPL.** The character and its
artwork are copyrighted with all rights reserved; [ARTWORK.md](ARTWORK.md) says
what you may do with them and what needs permission.

Pet packages remain subject to their own authors' licenses. Installing or
loading a pet does not change its license. The Roamling name and branding are
handled separately from the source license; see [TRADEMARKS.md](TRADEMARKS.md).

Contributors keep ownership of their work but must accept the
[Contributor License Agreement](CLA.md), which lets the project keep every
accepted contribution available under the GPL while also permitting official
commercial distribution. See [CONTRIBUTING.md](CONTRIBUTING.md).
