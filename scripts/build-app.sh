#!/bin/zsh
# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only

set -euo pipefail

CONFIGURATION="${1:-release}"
REPOSITORY_DIR="${0:A:h:h}"
mkdir -p "$REPOSITORY_DIR/.build/cache" "$REPOSITORY_DIR/.build/config" \
  "$REPOSITORY_DIR/.build/security" "$REPOSITORY_DIR/.build/module-cache"
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

cd "$REPOSITORY_DIR"
swift build -c "$CONFIGURATION" --product Roamling "${SWIFT_ARGS[@]}"
BIN_DIR="$(swift build -c "$CONFIGURATION" "${SWIFT_ARGS[@]}" --show-bin-path)"
APP_DIR="$REPOSITORY_DIR/build/Roamling.app"

mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
cp "$BIN_DIR/Roamling" "$APP_DIR/Contents/MacOS/Roamling"
cp "$REPOSITORY_DIR/Support/Info.plist" "$APP_DIR/Contents/Info.plist"

# Bind Info.plist and resources into a valid local bundle signature. Release
# distribution will replace this ad-hoc identity with Developer ID signing.
if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$APP_DIR"
fi

echo "Built $APP_DIR"
