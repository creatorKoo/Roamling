# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Write approved cells into one row of a pet's spritesheet.

The last step of a row's life: cells that passed review go into the atlas and
nothing else moves. Writing the row by hand is where a sheet picks up an
off-by-one column or a silently resized cell, so it happens here instead.

Cells are placed from column 0 and the rest of the row is cleared, because a row
that used to hold six frames and now holds four must not keep the leftovers --
a consumer reading a fixed column count would play them.

    ./scripts/pyimg.sh scripts/compose_row.py output/v3/mochi-v3 \\
        --row 3 --cells output/v3/rows/waving/cells-final
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("--row", type=int, required=True)
    parser.add_argument("--cells", type=Path, required=True)
    parser.add_argument(
        "--keep-tail",
        action="store_true",
        help="leave columns after the last cell untouched (extension frames live there)",
    )
    args = parser.parse_args()

    manifest = json.loads((args.package / "pet.json").read_text())
    frame = manifest["frame"]
    width, height = frame["width"], frame["height"]
    columns = frame["columns"]

    sheet_path = args.package / manifest["spritesheetPath"]
    sheet = Image.open(sheet_path).convert("RGBA")

    cells = sorted(args.cells.glob("*.png"))
    if not cells:
        raise SystemExit(f"no cells in {args.cells}")
    if len(cells) > columns:
        raise SystemExit(f"{len(cells)} cells will not fit in {columns} columns")

    top = args.row * height
    for column in range(columns):
        if column >= len(cells) and args.keep_tail:
            break
        box = (column * width, top, (column + 1) * width, top + height)
        blank = Image.new("RGBA", (width, height), (0, 0, 0, 0))
        sheet.paste(blank, box[:2])
        if column < len(cells):
            cell = Image.open(cells[column]).convert("RGBA")
            if cell.size != (width, height):
                raise SystemExit(f"{cells[column].name} is {cell.size}, expected {width}x{height}")
            sheet.alpha_composite(cell, box[:2])

    sheet.save(sheet_path, lossless=True)
    print(f"row {args.row}: wrote {len(cells)} cells -> {sheet_path}")


if __name__ == "__main__":
    main()
