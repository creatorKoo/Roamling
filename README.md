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
- a code-drawn fallback cat, so the app works without redistributing a
  third-party pet asset;
- global desktop coordinates, display topology, hot-plug handling, and
  continuous cross-display paths;
- low-speed wandering and state-dependent animation cadence;
- pointer awareness, capped evasion, fast-approach catching, click, drag, drop,
  and cross-monitor dragging;
- a sprite-sized overlay whose input region is enabled only while a catch is
  armed, leaving the underlying app alone during normal operation;
- pure-logic tests for geometry, display paths, movement, pointer interaction,
  behavior transitions, attention, reactions, and pet animation fallback.

Coding-agent integrations and Accessibility-based placement are deliberately
behind the next milestones; the core event vocabulary is not coding-specific.

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
WebP atlas. If no valid package is found, Roamling uses its built-in procedural
cat.

## Repository guide

```text
Sources/RoamlingCore/   OS-independent geometry, world, behavior, events
Sources/RoamlingPet/    Petdex/Codex manifests, atlas runtime, fallbacks
Sources/RoamlingMac/    AppKit display, pointer, overlay, and app runtime
Sources/RoamlingApp/    executable entry point
Tests/                  pure and loader tests
docs/research.md        upstream/API research with source locations
docs/architecture.md    boundaries, decisions, and milestone architecture
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
