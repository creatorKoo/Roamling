#!/bin/zsh
# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only

set -euo pipefail

CONFIGURATION="${1:-release}"
REPOSITORY_DIR="${0:A:h:h}"

# Optional, git-ignored local settings. A signing identity name is specific to
# one machine's keychain, so it lives here instead of in the script. See
# scripts/signing.env.example.
if [[ -f "$REPOSITORY_DIR/scripts/signing.env" ]]; then
  source "$REPOSITORY_DIR/scripts/signing.env"
fi
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
# The Swift build links the Rust core statically, so it has to exist first.
"$REPOSITORY_DIR/scripts/build-rust-core.sh"

swift build -c "$CONFIGURATION" --product Roamling "${SWIFT_ARGS[@]}"
BIN_DIR="$(swift build -c "$CONFIGURATION" "${SWIFT_ARGS[@]}" --show-bin-path)"
APP_DIR="$REPOSITORY_DIR/build/Roamling.app"

mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
cp "$BIN_DIR/Roamling" "$APP_DIR/Contents/MacOS/Roamling"
cp "$REPOSITORY_DIR/Support/Info.plist" "$APP_DIR/Contents/Info.plist"
# Every target with resources produces its own bundle, and Bundle.module traps
# at runtime when one is missing. Copy them all rather than naming each.
for module_bundle in "$BIN_DIR"/*.bundle; do
  [[ -d "$module_bundle" ]] || continue
  cp -R "$module_bundle" "$APP_DIR/Contents/Resources/"
done

# A stable identity keeps the designated requirement pointed at a certificate
# instead of the binary's cdhash, so macOS keeps a granted Accessibility
# permission across rebuilds. Ad-hoc resets it on every build.
CODESIGN_IDENTITY="${ROAMLING_CODESIGN_IDENTITY:--}"
if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign "$CODESIGN_IDENTITY" "$APP_DIR"
  if [[ "$CODESIGN_IDENTITY" == "-" ]]; then
    echo "Signed ad-hoc. Set ROAMLING_CODESIGN_IDENTITY to keep TCC grants across builds."
  else
    echo "Signed with identity: $CODESIGN_IDENTITY"
  fi
  # codesign prefixes the line with '#' for ad-hoc but not for a real identity.
  codesign -d -r- "$APP_DIR" 2>&1 | sed -n 's/^#* *designated => /Designated requirement: /p' || true
fi

echo "Built $APP_DIR"
