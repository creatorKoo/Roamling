#!/bin/zsh
# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only

# Runs a Python image-processing script with Pillow available.
#
# Neither the system nor the Homebrew python3 on this machine has Pillow, so
# atlas inspection, frame extraction, and sprite packing scripts fail with a
# bare `python3 script.py`. This wrapper resolves the dependency through uv's
# ephemeral environment instead of mutating any interpreter.
#
#   ./scripts/pyimg.sh path/to/script.py --arg value
#   ./scripts/pyimg.sh -c 'from PIL import Image; print(Image.open("a.png").size)'

set -euo pipefail

if ! command -v uv >/dev/null 2>&1; then
  print -u2 "pyimg: uv is required (https://docs.astral.sh/uv/). Install it or run the script with an interpreter that already has Pillow."
  exit 127
fi

if [[ $# -eq 0 ]]; then
  print -u2 "usage: ${0:t} <script.py|-c 'code'> [args...]"
  exit 64
fi

exec uv run --quiet --with pillow python "$@"
