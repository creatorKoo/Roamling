#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
#
# Draws assets/Roamling.icns from the same mark the menu bar and the Windows
# tray show. Run it when the mark changes; the result is committed, because an
# app bundle needs the icon as a file and a build should not depend on which
# emoji font the machine happens to have.
#
# Every size is drawn at its own size rather than downscaled from 1024. The paw
# has small round toes, and a resampled 16px version of them is mush where a
# drawn one is still a paw -- the same reason the Windows tray draws its icon
# instead of shipping a bitmap.
set -euo pipefail

REPOSITORY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

swiftc -O -o "$WORK/render" "$REPOSITORY_DIR/assets/icon/render-icon.swift" -framework AppKit
"$WORK/render" "$WORK"

SET="$WORK/Roamling.iconset"
mkdir -p "$SET"
# `iconutil` wants Apple's names, and the @2x entries are the same pixels as the
# next size up -- a 32px image is both `32x32` and `16x16@2x`.
cp "$WORK/icon_16.png"   "$SET/icon_16x16.png"
cp "$WORK/icon_32.png"   "$SET/icon_16x16@2x.png"
cp "$WORK/icon_32.png"   "$SET/icon_32x32.png"
cp "$WORK/icon_64.png"   "$SET/icon_32x32@2x.png"
cp "$WORK/icon_128.png"  "$SET/icon_128x128.png"
cp "$WORK/icon_256.png"  "$SET/icon_128x128@2x.png"
cp "$WORK/icon_256.png"  "$SET/icon_256x256.png"
cp "$WORK/icon_512.png"  "$SET/icon_256x256@2x.png"
cp "$WORK/icon_512.png"  "$SET/icon_512x512.png"
cp "$WORK/icon_1024.png" "$SET/icon_512x512@2x.png"

iconutil -c icns "$SET" -o "$REPOSITORY_DIR/assets/Roamling.icns"
echo "Built $REPOSITORY_DIR/assets/Roamling.icns"
