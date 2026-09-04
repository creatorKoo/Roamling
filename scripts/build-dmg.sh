#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
#
# Packages an already-built build/Roamling.app two ways, because the two
# audiences want different things:
#
#   Roamling-<version>.dmg   a person, who drags it to Applications
#   Roamling-<version>.zip   the updater, which unpacks it and swaps it in
#
# The zip is not a lesser dmg. Replacing a running app from a disk image means
# hdiutil, a mount point and an unmount that can fail with the volume busy; a
# zip is bytes in and a bundle out. Sparkle ships both for the same reason.
#
# This builds nothing and signs nothing. It reads the bundle, checks it, and
# wraps it -- so it can never be the thing that produces an ad-hoc build.
#
# Usage: scripts/build-dmg.sh [version]
set -euo pipefail

REPOSITORY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$REPOSITORY_DIR/build"
APP_DIR="$BUILD_DIR/Roamling.app"

if [[ ! -d "$APP_DIR" ]]; then
  echo "No $APP_DIR. Run scripts/build-app.sh release first." >&2
  exit 1
fi

VERSION="${1:-$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
  "$APP_DIR/Contents/Info.plist")}"

# Whatever is about to be wrapped has to be intact, and it has to be signed
# with a certificate rather than ad-hoc: a cdhash requirement would make every
# update a different app to macOS and drop the user's permissions.
codesign --verify --deep --strict "$APP_DIR"
REQUIREMENT="$(codesign -d -r- "$APP_DIR" 2>&1 |
  sed -n 's/^#* *designated => //p')"
if [[ "$REQUIREMENT" != *"certificate leaf"* ]]; then
  echo "Refusing to package a build whose requirement is:" >&2
  echo "  $REQUIREMENT" >&2
  echo "That is ad-hoc. Anyone installing it would lose Accessibility and" >&2
  echo "Screen Recording on the first update." >&2
  exit 1
fi
echo "Packaging a build whose requirement is: $REQUIREMENT"

ZIP_PATH="$BUILD_DIR/Roamling-$VERSION.zip"
rm -f "$ZIP_PATH"
# `ditto` rather than `zip`: it keeps the extended attributes and the internal
# symlinks, and `zip` has been known to break a signature by dropping them.
ditto -c -k --keepParent "$APP_DIR" "$ZIP_PATH"

# The image is built by hand rather than with `-srcfolder`, and the reason is
# worth writing down: whichever way the bundle goes into a disk image, the
# filesystem stamps `com.apple.FinderInfo` on every file, and
# `codesign --verify --strict` then refuses the app that comes out of it. The
# app still launches -- the stamp is not a broken signature, just detritus
# codesign will not tolerate under `--strict` -- but shipping something that
# fails its own verification is how you get a bug report you cannot reproduce.
#
# Clearing the attributes *inside the mounted image*, before it is compressed,
# is what fixes it. Clearing them in the staging folder does not: the stamp is
# applied on the way in.
RW_IMAGE="$BUILD_DIR/.Roamling-$VERSION-rw.dmg"
MOUNT_POINT="$(mktemp -d)"
cleanup() {
  hdiutil detach "$MOUNT_POINT" -quiet 2>/dev/null || true
  rm -rf "$MOUNT_POINT"
  rm -f "$RW_IMAGE"
}
trap cleanup EXIT

rm -f "$RW_IMAGE"
# Named without the version. The window layout comes from a committed
# .DS_Store, and the background inside it is referenced by an alias that
# records the path it was made from -- so a volume name that changed every
# release would point that alias at a volume nobody has.
hdiutil create -size 128m -fs APFS -volname "Roamling" \
  -type UDIF -ov "$RW_IMAGE" >/dev/null
hdiutil attach "$RW_IMAGE" -nobrowse -mountpoint "$MOUNT_POINT" >/dev/null

ditto "$APP_DIR" "$MOUNT_POINT/Roamling.app"
# The whole convention: the app on one side, a door to Applications on the
# other, and the user drags across.
ln -s /Applications "$MOUNT_POINT/Applications"
xattr -cr "$MOUNT_POINT/Roamling.app"

# And the picture that says which way to drag. Both files are committed --
# `scripts/build-dmg-background.sh` draws them -- because producing the
# .DS_Store means either scripting Finder, which wants a desktop session a
# release runner does not have, or two Python packages a release should not
# need to fetch.
mkdir -p "$MOUNT_POINT/.background"
cp "$REPOSITORY_DIR/assets/dmg-background.png" \
  "$MOUNT_POINT/.background/background.png"
cp "$REPOSITORY_DIR/assets/dmg/DS_Store" "$MOUNT_POINT/.DS_Store"
# Verified where it will actually be read from, which is the only place the
# stamp shows up.
codesign --verify --deep --strict "$MOUNT_POINT/Roamling.app"

hdiutil detach "$MOUNT_POINT" -quiet

DMG_PATH="$BUILD_DIR/Roamling-$VERSION.dmg"
rm -f "$DMG_PATH"
hdiutil convert "$RW_IMAGE" -format UDZO -ov -o "$DMG_PATH" >/dev/null

echo "Built $DMG_PATH"
echo "Built $ZIP_PATH"
