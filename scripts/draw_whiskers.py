# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Put the approved whiskers onto cells that came back without any.

Generated strips lose whiskers or ruin them. Asked for plainly they arrive pale
enough to vanish into the cheek; asked for dark they arrive as outlined brown
twigs. Both failures trace to the same thing -- the generator's working grid is
far coarser than a 192x208 cell, so its thinnest stroke is already wide enough to
want a border, and a hairline is not a shape it can draw.

So the strokes are measured off a cell that has them right and drawn onto the
ones that do not. Each stroke is recorded as offsets from two features the face
always has: how far it reaches past the edge of the cheek, and where it sits
relative to the nose. Placing by feature rather than by absolute position means a
head drawn slightly smaller or higher still gets its whiskers on its cheeks.

This adds pixels, which the art invariants otherwise forbid. It is here because
four generation attempts could not, and because a whisker is a fixed piece of
this character's face rather than part of any pose.

    ./scripts/pyimg.sh scripts/draw_whiskers.py <cells-dir> \\
        --reference <cell-with-whiskers.png> --out <dir>
"""

from __future__ import annotations

import argparse
from collections import deque
from pathlib import Path

from PIL import Image

INK = (0, 0, 0)
HEAD_ROWS = 45


def alpha_mask(image: Image.Image) -> list[list[bool]]:
    alpha = image.getchannel("A").load()
    return [[alpha[x, y] > 8 for x in range(image.width)] for y in range(image.height)]


def erode(mask: list[list[bool]], radius: int) -> list[list[bool]]:
    height, width = len(mask), len(mask[0])
    out = [[False] * width for _ in range(height)]
    for y in range(height):
        for x in range(width):
            if mask[y][x] and all(
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


def face(image: Image.Image) -> tuple[int, int, int, int]:
    """Head left edge, head right edge, and the nose's centre."""
    alpha = image.getchannel("A").load()
    rgb = image.convert("RGB").load()
    box = image.getchannel("A").point(lambda v: 255 if v > 8 else 0).getbbox()
    columns = [
        x
        for y in range(box[1], min(box[1] + HEAD_ROWS, image.height))
        for x in range(image.width)
        if alpha[x, y] > 8
    ]
    # The inner ears are pink too, and they sit higher and further out. Only
    # pixels near the head's centre line can be the nose; anchoring on an ear
    # put a set of whiskers up by the eyes.
    left, right = min(columns), max(columns)
    centre = (left + right) / 2
    reach = (right - left) * 0.16
    nose = [
        (x, y)
        for y in range(box[1] + 40, min(box[1] + HEAD_ROWS + 40, image.height))
        for x in range(round(centre - reach), round(centre + reach) + 1)
        if 0 <= x < image.width
        and alpha[x, y] > 8
        and rgb[x, y][0] > 200
        and 120 < rgb[x, y][1] < 195
        and 90 < rgb[x, y][2] < 165
    ]
    if not nose:
        raise SystemExit("no nose found; the pink triangle is the anchor")
    return (
        left,
        right,
        round(sum(x for x, _ in nose) / len(nose)),
        round(sum(y for _, y in nose) / len(nose)),
    )


def strokes(image: Image.Image, radius: int) -> list[list[tuple[int, int]]]:
    """Thin protrusions, grouped into one list per whisker."""
    mask = alpha_mask(image)
    core = dilate(erode(mask, radius), radius)
    loose = {
        (y, x)
        for y in range(image.height)
        for x in range(image.width)
        if mask[y][x] and not core[y][x]
    }
    seen: set[tuple[int, int]] = set()
    out = []
    for start in sorted(loose):
        if start in seen:
            continue
        queue = deque([start])
        seen.add(start)
        group = []
        while queue:
            y, x = queue.popleft()
            group.append((y, x))
            for dy in (-1, 0, 1):
                for dx in (-1, 0, 1):
                    step = (y + dy, x + dx)
                    if step in loose and step not in seen:
                        seen.add(step)
                        queue.append(step)
        out.append(group)
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cells", type=Path)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--radius", type=int, default=2)
    args = parser.parse_args()

    source = Image.open(args.reference).convert("RGBA")
    left, right, _, nose_y = face(source)
    recorded = []
    for stroke in strokes(source, args.radius):
        side = "left" if min(x for _, x in stroke) < left + (right - left) / 2 else "right"
        edge = left if side == "left" else right
        recorded.append([(side, x - edge, y - nose_y) for y, x in stroke])
    print(f"{args.reference.name}: {len(recorded)} strokes")

    args.out.mkdir(parents=True, exist_ok=True)
    for path in sorted(args.cells.glob("*.png")):
        image = Image.open(path).convert("RGBA")
        target_left, target_right, _, target_nose = face(image)
        pixels = image.load()
        alpha = image.getchannel("A").load()
        drawn = 0
        bridged = 0
        for stroke in recorded:
            placed = []
            for side, dx, dy in stroke:
                edge = target_left if side == "left" else target_right
                x, y = edge + dx, target_nose + dy
                if 0 <= x < image.width and 0 <= y < image.height:
                    pixels[x, y] = (*INK, 255)
                    placed.append((x, y))
                    drawn += 1
            if not placed:
                continue
            # A cheek drawn a pixel or two narrower than the reference leaves the
            # stroke hanging in the air. Walk its inner end towards the head until
            # it meets the face, so a whisker is never a loose fragment.
            step = 1 if stroke[0][0] == "left" else -1
            x, y = max(placed) if step == 1 else min(placed)
            for _ in range(8):
                x += step
                if not (0 <= x < image.width) or alpha[x, y] > 8:
                    break
                pixels[x, y] = (*INK, 255)
                bridged += 1
        image.save(args.out / path.name)
        note = f" (+{bridged} bridging)" if bridged else ""
        print(f"  {path.name}: {drawn} px{note}")
    print(f"-> {args.out}")


if __name__ == "__main__":
    main()
