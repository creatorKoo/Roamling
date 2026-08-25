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

swift run "${SWIFT_ARGS[@]}" RoamlingLogicTests
