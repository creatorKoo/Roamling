# 🐾 Roamling

**A tiny companion that actually lives on your desktop.**

Roamling is a native macOS companion runtime. Petdex-compatible creatures roam
across your monitors, avoid your pointer, let you catch and drag them, and will
eventually react to coding, games, media, and the rest of your desktop life.

> Cute first. Useful second. Never annoying.

## Current status

This repository contains the first working vertical slice:

- a native AppKit menu-bar app and transparent, non-activating overlay;
- Codex/Petdex v1 (8×9) and v2 (8×11) pet loading, plus custom animation
  definitions and graceful capability fallback;
- two original built-in mascots, **Mochi** and **FatMochi**, with authored
  idle/walk/sleep/caught/stretch/landing animation sets, selectable from the
  menu, plus a code-drawn emergency fallback;
- global desktop coordinates, display topology, hot-plug handling, and
  continuous cross-display paths;
- calmer wandering with visible idle pauses, shorter local trips, and more
  noticeable multi-display exploration;
- pointer awareness, capped evasion, fast-approach catching, click, drag, drop,
  cross-monitor dragging, and connected-edge escape when gently cornered;
- a live **Behavior Tuning…** panel for MVP 0/0.5 movement, pointer, catch,
  and hit-region values, with persistent settings and one-click reset;
- permission-free idle detection plus sit, safe sleep-spot travel, sleep,
  wake, and stretch behavior at a reduced sleeping cadence;
- basic corner/Dock-adjacent placement that stays inside each display's
  visible frame and avoids the pointer;
- an opt-in Claude Code hook integration with a token-authenticated local
  receiver, coarse permission-free work-window placement, and start,
  attention, completion, and failure reactions;
- an opt-in Codex 0.147+ hook integration that preserves existing hooks and
  `notify`, plus shared multi-source attention, hysteresis, and reaction policy;
- a sprite-sized overlay whose input region is enabled only while a catch is
  armed, leaving the underlying app alone during normal operation;
- pure-logic tests for geometry, display paths, movement, pointer interaction,
  behavior transitions, attention, reactions, and pet animation fallback.

Accessibility/caret tracking and visual placement remain behind the next
milestones; the core event vocabulary is not coding-specific.

## Build and run

Requirements: macOS 13 or newer and Swift 6. Building the command-line target
works with Apple Command Line Tools; creating a signed distributable app will
eventually require full Xcode.

```sh
swift build
./scripts/test.sh
swift run Roamling
```

Create a local `.app` bundle:

```sh
./scripts/build-app.sh release
open build/Roamling.app
```

The bundle is signed ad-hoc by default, which makes macOS treat every rebuild as
a different app and forget granted permissions. To keep those grants across
builds, copy `scripts/signing.env.example` to `scripts/signing.env` (git-ignored)
and set `ROAMLING_CODESIGN_IDENTITY` to a code signing identity. A free
self-signed certificate works; the example file has the steps. The same variable
can be exported in the environment instead.

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
WebP atlas. FatMochi is the default. Mochi and discovered Petdex-compatible
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

## Repository guide

```text
Sources/RoamlingCore/   OS-independent geometry, world, behavior, events
Sources/RoamlingPet/    Petdex/Codex manifests, atlas runtime, fallbacks
Sources/RoamlingSources/ activity adapters and local hook transport
Sources/RoamlingMac/    AppKit display, pointer, overlay, and app runtime
Sources/RoamlingApp/    executable entry point
Tests/                  pure and loader tests
docs/research.md        upstream/API research with source locations
docs/architecture.md    boundaries, decisions, and milestone architecture
docs/mvp.md             current MVP gate, scope, and acceptance criteria
docs/windows.md         Windows port measurement, decisions, and gates
```

Roamling is an independent project and is not affiliated with OpenAI,
Anthropic, Petdex, or the comparison projects named in the research notes.

## License and contributions

Copyright (C) 2026 GooBeom Jeoung.

Roamling source code is licensed under the
[GNU General Public License v3.0 only](LICENSE). You may use, modify, distribute,
and sell the software under that license; covered derivative works must preserve
the same freedoms and provide their corresponding source.

Pet packages remain subject to their own authors' licenses. Installing or
loading a pet does not change its license. The Roamling name and branding are
handled separately from the source license; see [TRADEMARKS.md](TRADEMARKS.md).

Contributors keep ownership of their work but must accept the
[Contributor License Agreement](CLA.md), which lets the project keep every
accepted contribution available under the GPL while also permitting official
commercial distribution. See [CONTRIBUTING.md](CONTRIBUTING.md).
