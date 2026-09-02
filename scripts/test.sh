#!/bin/zsh
# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only

set -euo pipefail

REPOSITORY_DIR="${0:A:h:h}"
cd "$REPOSITORY_DIR"

mkdir -p .build/cache .build/config .build/security .build/module-cache
export CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$REPOSITORY_DIR/.build/module-cache}"
export SWIFTPM_MODULECACHE_OVERRIDE="${SWIFTPM_MODULECACHE_OVERRIDE:-$REPOSITORY_DIR/.build/module-cache}"

SWIFT_ARGS=(
  --disable-sandbox
  --cache-path "$REPOSITORY_DIR/.build/cache"
  --config-path "$REPOSITORY_DIR/.build/config"
  --security-path "$REPOSITORY_DIR/.build/security"
  --disable-dependency-cache
  --manifest-cache local
)
if [[ -n "${ROAMLING_SWIFT_SDK:-}" ]]; then
  export SDKROOT="$ROAMLING_SWIFT_SDK"
  SWIFT_ARGS+=(--sdk "$ROAMLING_SWIFT_SDK")
fi

# The portable modules must not reach for a window system, for Apple's image
# frameworks, or for Network. The compiler will not catch this on macOS -- the
# SDK ships them all, so an accidental import builds fine here and only fails on
# the machine that has none of them. W0 found exactly two such lines.
PORTABLE_DIRS=(
  Sources/RoamlingCore
  Sources/RoamlingPet
  Sources/RoamlingSources
  Sources/RoamlingEngine
  Sources/RoamlingShell
)
if grep -rnE --include='*.swift' \
  '^[[:space:]]*(@_exported[[:space:]]+)?import[[:space:]]+(AppKit|Cocoa|SwiftUI|ScreenCaptureKit|ApplicationServices|Quartz|CoreGraphics|ImageIO|CoreImage|Network)\b' \
  "${PORTABLE_DIRS[@]}"; then
  print -u2 "Platform image or window import found in a portable module (see docs/windows.md, W1/W2)"
  exit 1
fi

swift run "${SWIFT_ARGS[@]}" RoamlingLogicTests

# The Rust core is being ported one unit at a time, and each unit is gated by a
# fixture the Swift original generated. Running both here keeps that one command.
if command -v cargo >/dev/null 2>&1; then
  cargo test --quiet --manifest-path "$REPOSITORY_DIR/rust/Cargo.toml"
elif [[ -n "${CI:-}" ]]; then
  print -u2 "cargo is required: the Rust core's differential tests cannot run"
  exit 1
else
  print -u2 "warning: cargo not found, skipping the Rust core's differential tests"
fi
