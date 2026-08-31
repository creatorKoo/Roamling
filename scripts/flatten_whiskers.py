# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Repaint whiskers a single flat colour.

Image generators draw this pet's whiskers the way they draw its body: a coloured
fill with a darker border around it. At sprite scale that turns each whisker into
a twig stuck to the cheek. The approved artwork draws them as one flat black
mark, and no amount of prompting has produced that -- the generator's working
grid is far coarser than the final cell, so its thinnest possible stroke is still
wide enough that it wants an outline.

So the whiskers are found and repainted here. This is a recolour, not a redraw:
no pixel moves, nothing is pasted in from another image, and the silhouette is
untouched. Only the colour of pixels already identified as whisker changes.

Whiskers are found by shape rather than by colour. A morphological opening
removes any structure thinner than the brush from the silhouette; whatever the
opening removed is a thin protrusion, which on a seated cat means whiskers. The
stroke is then followed inward through non-cream pixels so the part crossing the
cheek is caught too.

    ./scripts/pyimg.sh scripts/flatten_whiskers.py <cells-dir> --out <dir>
"""

from __future__ import annotations

import argparse
from collections import deque
from pathlib import Path

from PIL import Image

INK = (0, 0, 0)


def alpha_mask(image: Image.Image) -> list[list[bool]]:
    alpha = image.getchannel("A").load()
    return [
        [alpha[x, y] > 8 for x in range(image.width)] for y in range(image.height)
    ]


def erode(mask: list[list[bool]], radius: int) -> list[list[bool]]:
    height, width = len(mask), len(mask[0])
    out = [[False] * width for _ in range(height)]
    for y in range(height):
        for x in range(width):
            if not mask[y][x]:
                continue
            if all(
                0 <= y + dy < height and 0 <= x + dx < width and mask[y + dy][x + dx]
                for dy in range(-radius, radius + 1)
                for dx in range(-radius, radius + 1)
            ):
                out[y][x] = True
    return out


def dilate(mask: list[list[bool]], radius: int) -> list[list[bool]]:
    height, width = len(mask), len(mask[0])
    out = [[False] * width for _ in range(height)]
    for y in range(height):
        for x in range(width):
            if not mask[y][x]:
                continue
            for dy in range(-radius, radius + 1):
                for dx in range(-radius, radius + 1):
                    if 0 <= y + dy < height and 0 <= x + dx < width:
                        out[y + dy][x + dx] = True
    return out


def is_pale(pixel: tuple[int, int, int]) -> bool:
    """Cheek cream and the white of the muzzle, which a whisker is drawn over."""
    red, green, blue = pixel
    return red > 200 and green > 190 and blue > 165


def whisker_pixels(
    image: Image.Image, radius: int, reach: int, band: tuple[int, int]
) -> set[tuple[int, int]]:
    """Whiskers are the thin protrusions inside the muzzle band.

    The band matters. An ear tip and the end of a tail are also thin enough for
    the opening to remove, and blackening those turns a cat into a cat with a
    charred ear -- which is exactly what happened on one frame before this was
    bounded.
    """
    mask = alpha_mask(image)
    core = dilate(erode(mask, radius), radius)
    top, bottom = band
    found = {
        (y, x)
        for y in range(max(0, top), min(image.height, bottom + 1))
        for x in range(image.width)
        if mask[y][x] and not core[y][x]
    }
    if not found:
        return found

    # Follow each stroke inward. Outside the silhouette the whisker is obvious;
    # where it crosses the cheek it is merely the only thing there that is not
    # cream, so that is the test. `reach` bounds it so a stroke cannot wander
    # into an eye.
    rgb = image.convert("RGB").load()
    queue = deque((y, x, 0) for y, x in found)
    while queue:
        y, x, step = queue.popleft()
        if step >= reach:
            continue
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                ny, nx = y + dy, x + dx
                if not (0 <= ny < image.height and 0 <= nx < image.width):
                    continue
                if (ny, nx) in found or not mask[ny][nx]:
                    continue
                if is_pale(rgb[nx, ny]):
                    continue
                found.add((ny, nx))
                queue.append((ny, nx, step + 1))
    return found


def thin(
    found: set[tuple[int, int]],
    thickness: int,
    core: list[list[bool]],
) -> tuple[set[tuple[int, int]], set[tuple[int, int]]]:
    """Narrow each stroke to `thickness` pixels, column by column.

    The whiskers run close to horizontal, so a vertical run within one column is
    one stroke crossed once. Keeping the middle of each run narrows the stroke
    without breaking it, which a naive erosion would.

    Only the part sticking out past the body is narrowed. Where a whisker meets
    the face it merges with the outline, and thinning there punches cream holes
    in that outline -- the root ends up looking chewed. Inside the body the
    whisker simply stays dark, which is what the approved artwork does too.

    Returns the pixels to keep and the pixels to clear.
    """
    columns: dict[int, list[int]] = {}
    keep_always = set()
    for y, x in found:
        if core[y][x]:
            keep_always.add((y, x))
        else:
            columns.setdefault(x, []).append(y)

    keep: set[tuple[int, int]] = set(keep_always)
    for x, rows in columns.items():
        rows.sort()
        run = [rows[0]]
        for y in rows[1:]:
            if y == run[-1] + 1:
                run.append(y)
            else:
                keep.update((yy, x) for yy in middle(run, thickness))
                run = [y]
        keep.update((yy, x) for yy in middle(run, thickness))
    return keep, found - keep


def middle(run: list[int], thickness: int) -> list[int]:
    if len(run) <= thickness:
        return run
    start = (len(run) - thickness) // 2
    return run[start : start + thickness]


def cheek_near(image: Image.Image, x: int, y: int) -> tuple[int, int, int]:
    """The pale colour surrounding a pixel, for filling a removed whisker."""
    rgb = image.convert("RGB").load()
    alpha = image.getchannel("A").load()
    for radius in range(1, 5):
        found = [
            rgb[nx, ny]
            for ny in range(y - radius, y + radius + 1)
            for nx in range(x - radius, x + radius + 1)
            if 0 <= ny < image.height
            and 0 <= nx < image.width
            and alpha[nx, ny] > 8
            and is_pale(rgb[nx, ny])
        ]
        if found:
            return max(set(found), key=found.count)
    return (254, 240, 216)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cells", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--radius", type=int, default=2, help="opening brush radius")
    parser.add_argument(
        "--reach", type=int, default=4, help="how far a stroke is followed inward"
    )
    parser.add_argument(
        "--band",
        default="88,112",
        help="Y range the whiskers live in, as TOP,BOTTOM",
    )
    parser.add_argument(
        "--thickness",
        type=int,
        default=0,
        help="narrow each stroke to this many pixels; 0 leaves the width alone",
    )
    parser.add_argument(
        "--dry-run", action="store_true", help="report what was found, change nothing"
    )
    args = parser.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)
    for path in sorted(args.cells.glob("*.png")):
        image = Image.open(path).convert("RGBA")
        top, bottom = (int(v) for v in args.band.split(","))
        found = whisker_pixels(image, args.radius, args.reach, (top, bottom))
        rows = sorted({y for y, _ in found})
        span = f"y {rows[0]}-{rows[-1]}" if rows else "none"
        print(f"  {path.name}: {len(found)} px  {span}")
        if args.dry_run:
            continue

        pixels = image.load()
        if args.thickness:
            mask = alpha_mask(image)
            core = dilate(erode(mask, args.radius), args.radius)
            keep, drop = thin(found, args.thickness, core)
            # Everything narrowed away was outside the body, so it goes back to
            # nothing rather than needing a colour guessed for it.
            for y, x in drop:
                pixels[x, y] = (0, 0, 0, 0)
            found = keep
        for y, x in found:
            pixels[x, y] = (*INK, 255)
        image.save(args.out / path.name)
    if not args.dry_run:
        print(f"-> {args.out}")


if __name__ == "__main__":
    main()
