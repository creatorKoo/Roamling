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

# The portable modules must not reach for a window system or for Apple's image
# frameworks. The compiler will not catch this on macOS -- the SDK ships them,
# so an accidental import builds fine here and only fails on the machine that
# has neither.
PORTABLE_DIRS=(
  Sources/RoamlingCore
  Sources/RoamlingPet
  Sources/RoamlingSources
  Sources/RoamlingEngine
)
if grep -rnE --include='*.swift' \
  '^[[:space:]]*(@_exported[[:space:]]+)?import[[:space:]]+(AppKit|Cocoa|SwiftUI|ScreenCaptureKit|ApplicationServices|Quartz|CoreGraphics|ImageIO|CoreImage)\b' \
  "${PORTABLE_DIRS[@]}"; then
  print -u2 "Platform image or window import found in a portable module (see docs/windows.md, W1/W2)"
  exit 1
fi

swift run "${SWIFT_ARGS[@]}" RoamlingLogicTests
