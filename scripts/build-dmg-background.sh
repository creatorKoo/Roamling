#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
#
# Draws the picture behind the disk image window, and writes the .DS_Store that
# puts it there. Run it when the drawing or the icon positions change; both
# results are committed, because a release build should have to draw nothing and
# script nothing.
#
# The .DS_Store cannot be produced the usual way. Finder writes one when it is
# told to open the volume and arrange it, and that needs a desktop session with
# Automation permission -- measured here, and on a release runner, as -1712, an
# AppleEvent timeout. So it is written directly, which needs uv for two pure
# Python packages, and only ever on a machine like this one.
set -euo pipefail

REPOSITORY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

swiftc -O -o "$WORK/render" \
  "$REPOSITORY_DIR/assets/dmg/render-dmg-background.swift" -framework AppKit
"$WORK/render" "$WORK"

# One file with both resolutions in it. Finder picks the right one per display,
# and a folder holding two loose PNGs would need the .DS_Store to name both.
tiffutil -cathidpicheck "$WORK/dmg-background.png" "$WORK/dmg-background@2x.png" \
  -out "$REPOSITORY_DIR/assets/dmg-background.tiff" >/dev/null

echo "Built $REPOSITORY_DIR/assets/dmg-background.tiff"

if ! command -v uv >/dev/null 2>&1; then
  echo "uv is required to write the .DS_Store (https://docs.astral.sh/uv/)." >&2
  exit 127
fi

# The alias inside records the path it was made from, so the background has to
# be sitting at that path when it is made. A scratch volume with the name a
# release uses is the cheapest way to arrange that.
SCRATCH="$WORK/scratch.dmg"
hdiutil create -size 8m -fs APFS -volname "Roamling" -type UDIF -ov "$SCRATCH" >/dev/null
hdiutil attach "$SCRATCH" -noverify -noautoopen -nobrowse >/dev/null
trap 'hdiutil detach /Volumes/Roamling -quiet 2>/dev/null || true; rm -rf "$WORK"' EXIT
mkdir -p /Volumes/Roamling/.background
cp "$REPOSITORY_DIR/assets/dmg-background.tiff" /Volumes/Roamling/.background/background.tiff

uv run --quiet --with ds-store --with mac-alias \
  python "$REPOSITORY_DIR/assets/dmg/write-ds-store.py" \
  "$REPOSITORY_DIR/assets/dmg/DS_Store"

hdiutil detach /Volumes/Roamling -quiet
echo "Built $REPOSITORY_DIR/assets/dmg/DS_Store"
