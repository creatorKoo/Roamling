# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Move whole frames vertically so every row shares one ground line.

Sprites drawn in separate batches land on slightly different floors, and a pet
that sinks two pixels when it stops walking reads as broken long before anyone
can say why. This shifts complete frames inside their cells -- the operation the
art invariants allow -- and never edits pixels within a frame.

Shifts are given explicitly rather than inferred. A row's spread is not always
error: the walk's airborne frames really do leave the ground, and flattening them
would make the pet skate. Measure first (`--report`), decide which part is a
placement mistake, then say so.

    ./scripts/pyimg.sh scripts/align_baseline.py <package> --report
    ./scripts/pyimg.sh scripts/align_baseline.py <package> -o <out> \\
        --row 0=-2 --row 1=+2 --row 2=+2 --row 5=+1 --cell 32=-26 --cell 36=-26
"""

from __future__ import annotations

import argparse
import json
import shutil
from collections import deque
from pathlib import Path

from PIL import Image

ALPHA_FLOOR = 8


def main_component_bounds(cell: Image.Image) -> tuple[int, int] | None:
    """Top and bottom of the largest connected component.

    The largest one is the body. Keying residue -- stray single pixels the chroma
    removal left behind -- would otherwise decide where the floor is.
    """
    alpha = cell.getchannel("A").load()
    width, height = cell.size
    seen = [[False] * width for _ in range(height)]
    best: list[tuple[int, int]] = []
    for y in range(height):
        for x in range(width):
            if alpha[x, y] <= ALPHA_FLOOR or seen[y][x]:
                continue
            queue = deque([(y, x)])
            seen[y][x] = True
            pixels = []
            while queue:
                cy, cx = queue.popleft()
                pixels.append((cy, cx))
                for dy in (-1, 0, 1):
                    for dx in (-1, 0, 1):
                        ny, nx = cy + dy, cx + dx
                        if (
                            0 <= ny < height
                            and 0 <= nx < width
                            and alpha[nx, ny] > ALPHA_FLOOR
                            and not seen[ny][nx]
                        ):
                            seen[ny][nx] = True
                            queue.append((ny, nx))
            if len(pixels) > len(best):
                best = pixels
    if not best:
        return None
    return min(y for y, _ in best), max(y for y, _ in best)


def cells(sheet: Image.Image, frame: dict) -> list[tuple[int, Image.Image]]:
    columns, rows = frame["columns"], frame["rows"]
    width, height = frame["width"], frame["height"]
    out = []
    for row in range(rows):
        for column in range(columns):
            box = (
                column * width,
                row * height,
                (column + 1) * width,
                (row + 1) * height,
            )
            out.append((row * columns + column, sheet.crop(box)))
    return out


def parse_pairs(values: list[str] | None) -> dict[int, int]:
    result: dict[int, int] = {}
    for item in values or []:
        key, _, delta = item.partition("=")
        result[int(key)] = int(delta)
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("-o", "--out", type=Path, help="destination package directory")
    parser.add_argument("--row", action="append", help="ROW=DY (negative is up)")
    parser.add_argument("--cell", action="append", help="INDEX=DY, overrides its row")
    parser.add_argument("--report", action="store_true", help="measure and stop")
    args = parser.parse_args()

    manifest = json.loads((args.package / "pet.json").read_text())
    sheet_path = args.package / manifest["spritesheetPath"]
    sheet = Image.open(sheet_path).convert("RGBA")
    frame = manifest["frame"]
    columns = frame["columns"]

    row_shift = parse_pairs(args.row)
    cell_shift = parse_pairs(args.cell)

    out_sheet = Image.new("RGBA", sheet.size, (0, 0, 0, 0))
    moved = 0
    for index, cell in cells(sheet, frame):
        bounds = main_component_bounds(cell)
        row = index // columns
        delta = cell_shift.get(index, row_shift.get(row, 0))
        if args.report:
            if bounds:
                print(
                    f"  r{row}c{index % columns} baseline {bounds[1]:3d} "
                    f"top {bounds[0]:3d} -> {bounds[1] + delta:3d}"
                )
            continue
        if delta and bounds:
            shifted = Image.new("RGBA", cell.size, (0, 0, 0, 0))
            shifted.paste(cell, (0, delta))
            cell = shifted
            moved += 1
        column = index % columns
        out_sheet.paste(
            cell, (column * frame["width"], row * frame["height"])
        )

    if args.report:
        return
    if not args.out:
        parser.error("--out is required unless --report")

    args.out.mkdir(parents=True, exist_ok=True)
    shutil.copy2(args.package / "pet.json", args.out / "pet.json")
    extension = args.package / "roamling.json"
    if extension.exists():
        shutil.copy2(extension, args.out / "roamling.json")
    destination = args.out / manifest["spritesheetPath"]
    out_sheet.save(destination, lossless=True)
    print(f"{moved} frames moved -> {destination}")


if __name__ == "__main__":
    main()
