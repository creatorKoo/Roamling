#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
set -euo pipefail
REPOSITORY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"$REPOSITORY_DIR/scripts/pyimg.sh" "$REPOSITORY_DIR/scripts/build-icon.py"
