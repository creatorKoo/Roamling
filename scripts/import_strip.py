# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Turn a generated chroma-key strip into atlas-ready cells.

Generators return a large illustration on a green field, not sprite cells: the
colours drift pixel to pixel, the figure sits wherever it landed, and the pixel
grid is decorative rather than real. Everything between that and the atlas is
mechanical, so it lives here instead of being redone by hand per row.

    chroma key -> despill -> crop to the figure -> scale to the sheet's height
    -> snap every colour to the sheet's own palette -> centre and seat on the
    ground line -> emit one PNG per cell

The palette snap is what keeps a row from looking like it came from a different
pet. Rows generated separately drift apart in colour even when they look right
alone; mapping both to the approved sheet's palette removes the drift outright.

    ./scripts/pyimg.sh scripts/import_strip.py <strip.png> --palette <sheet.webp> \\
        --out output/v3/rows/waving/cells --height 148 --baseline 175
"""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image

CELL = (192, 208)


def is_key(pixel: tuple[int, int, int], slack: int) -> bool:
    red, green, blue = pixel[:3]
    return green > 110 and green - max(red, blue) > slack


def despill(pixel: tuple[int, int, int]) -> tuple[int, int, int]:
    """Pull the green fringe off an edge pixel.

    Keying leaves a rim where the subject blended into the backdrop. Left alone
    it survives the palette snap as a row of off-colour dots, which is how a
    sheet ends up with detached pixels nobody drew.
    """
    red, green, blue = pixel[:3]
    ceiling = (red + blue) // 2 + 12
    return red, min(green, ceiling), blue


def key_out(image: Image.Image, slack: int) -> Image.Image:
    out = image.convert("RGBA")
    pixels = out.load()
    for y in range(out.height):
        for x in range(out.width):
            red, green, blue, _ = pixels[x, y]
            if is_key((red, green, blue), slack):
                pixels[x, y] = (0, 0, 0, 0)
            else:
                pixels[x, y] = (*despill((red, green, blue)), 255)
    return out


def split(image: Image.Image, expected: int) -> list[Image.Image]:
    """Columns that contain anything become frames."""
    alpha = image.getchannel("A").load()
    occupied = [
        any(alpha[x, y] > 8 for y in range(0, image.height, 2))
        for x in range(image.width)
    ]
    runs, start = [], None
    for x, filled in enumerate(occupied + [False]):
        if filled and start is None:
            start = x
        elif not filled and start is not None:
            if x - start > 24:
                runs.append((start, x))
            start = None
    if len(runs) != expected:
        raise SystemExit(f"found {len(runs)} frames, expected {expected}")
    return [image.crop((a, 0, b, image.height)) for a, b in runs]


def palette_of(sheet: Image.Image, colours: int) -> list[tuple[int, int, int]]:
    """The sheet's working palette, not its literal colour count.

    The installed sheets are lossy WebP, so a row of flat cream is stored as
    thousands of near-identical creams -- 189,580 unique colours across the
    Mochi atlas. Snapping to that is both meaningless and unbearably slow, so
    the palette is the sheet quantised down to the handful of colours it was
    actually drawn with.
    """
    opaque = sheet.convert("RGBA")
    flat = Image.new("RGB", opaque.size, (255, 0, 255))
    flat.paste(opaque.convert("RGB"), mask=opaque.getchannel("A"))
    reduced = flat.quantize(colors=colours, method=Image.MEDIANCUT)
    table = reduced.getpalette()[: colours * 3]
    found = [tuple(table[i : i + 3]) for i in range(0, len(table), 3)]
    # The magenta stand-in for transparency is not part of the artwork.
    return [colour for colour in found if colour != (255, 0, 255)]


def snap(image: Image.Image, palette: list[tuple[int, int, int]]) -> Image.Image:
    cache: dict[tuple[int, int, int], tuple[int, int, int]] = {}
    out = image.copy()
    pixels = out.load()
    for y in range(out.height):
        for x in range(out.width):
            red, green, blue, alpha = pixels[x, y]
            if alpha <= 8:
                pixels[x, y] = (0, 0, 0, 0)
                continue
            key = (red, green, blue)
            if key not in cache:
                cache[key] = min(
                    palette,
                    key=lambda c: (c[0] - red) ** 2
                    + (c[1] - green) ** 2
                    + (c[2] - blue) ** 2,
                )
            pixels[x, y] = (*cache[key], 255)
    return out


def anchor_offset(figure: Image.Image, rows: int, where: str) -> float:
    """Where the anchor sits, measured from the figure's left edge.

    Centring on the bounding box looks right until a limb reaches out: a tail
    swinging 28px to one side drags the box with it, and holding the *box*
    still slides the cat the other way. A tail flick then reads as the whole
    animal skating sideways.

    Which part to hold depends on what the pose moves. The head works until the
    head itself tilts -- then its horizontal extent shifts, holding that extent
    still pushes the body the other way, and the cat slides out from under its
    own head. The paws are the safer anchor for a seated pose: they are planted
    by definition, so nothing else has to be argued about.
    """
    mask = figure.getchannel("A").point(lambda v: 255 if v > 8 else 0)
    pixels = mask.load()
    box = mask.getbbox()
    band = (
        range(box[1], min(box[1] + rows, figure.height))
        if where == "head"
        else range(max(0, box[3] - rows), box[3])
    )
    columns = [x for x in range(figure.width) for y in band if pixels[x, y]]
    return (min(columns) + max(columns)) / 2


def place(
    figure: Image.Image,
    baseline: int,
    centre: float,
    anchor: str,
    rows: int,
    margin: int,
) -> tuple[Image.Image, int]:
    """Seat one figure in a cell, and say if holding the anchor would clip it.

    The anchor wins until the figure would run off the edge -- a tail at full
    swing has no business being cut in half to keep a head at 95.5. The clamp is
    reported rather than applied quietly, because a frame that needed it is a
    frame whose pose is too wide for the cell.
    """
    cell = Image.new("RGBA", CELL, (0, 0, 0, 0))
    box = figure.getchannel("A").point(lambda v: 255 if v > 8 else 0).getbbox()
    trimmed = figure.crop(box)
    if anchor == "bbox":
        left = round(centre - trimmed.width / 2)
    else:
        left = round(centre - anchor_offset(trimmed, rows, anchor))
    clamped = min(max(left, margin), CELL[0] - margin - trimmed.width)
    top = baseline - trimmed.height + 1
    cell.alpha_composite(trimmed, (clamped, top))
    return cell, clamped - left


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("strip", type=Path)
    parser.add_argument("--palette", type=Path, required=True, help="approved sheet")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--frames", type=int, default=4)
    parser.add_argument("--height", type=int, default=148, help="target visible height")
    parser.add_argument("--baseline", type=int, default=175)
    parser.add_argument("--centre", type=float, default=95.5)
    parser.add_argument("--slack", type=int, default=40, help="chroma key margin")
    parser.add_argument(
        "--colours", type=int, default=40, help="size of the master palette"
    )
    parser.add_argument(
        "--anchor",
        choices=("feet", "head", "bbox"),
        default="feet",
        help="what to hold still horizontally",
    )
    parser.add_argument(
        "--anchor-rows",
        type=int,
        default=12,
        help="rows counted from the top (head) or bottom (feet)",
    )
    parser.add_argument(
        "--margin", type=int, default=2, help="pixels to keep clear at the cell edges"
    )
    parser.add_argument(
        "--scale-frame",
        type=int,
        default=0,
        help="frame whose height becomes --height; -1 uses the tallest",
    )
    args = parser.parse_args()

    keyed = key_out(Image.open(args.strip).convert("RGB"), args.slack)
    frames = split(keyed, args.frames)
    palette = palette_of(Image.open(args.palette), args.colours)

    # One scale for the whole strip. Scaling each frame to the same height would
    # quietly resize the cat whenever a pose is shorter than its neighbours.
    #
    # Which frame sets it matters: `--height` is the approved idle's height, and
    # the frame that has to match idle exactly is the neutral one, not whichever
    # pose happens to reach highest. Taking it from the tallest instead left the
    # neutral frames 3px short, so returning to idle popped.
    def visible_height(frame: Image.Image) -> int:
        box = frame.getchannel("A").point(lambda v: 255 if v > 8 else 0).getbbox()
        return box[3] - box[1]

    reference_height = (
        max(visible_height(frame) for frame in frames)
        if args.scale_frame < 0
        else visible_height(frames[args.scale_frame])
    )
    scale = args.height / reference_height

    args.out.mkdir(parents=True, exist_ok=True)
    for index, frame in enumerate(frames):
        box = frame.getchannel("A").point(lambda v: 255 if v > 8 else 0).getbbox()
        trimmed = frame.crop(box)
        small = trimmed.resize(
            (max(1, round(trimmed.width * scale)), max(1, round(trimmed.height * scale))),
            Image.BOX,
        )
        cell, nudge = place(
            snap(small, palette),
            args.baseline,
            args.centre,
            args.anchor,
            args.anchor_rows,
            args.margin,
        )
        cell.save(args.out / f"{index:02d}.png")
        bounds = cell.getchannel("A").point(lambda v: 255 if v > 8 else 0).getbbox()
        note = f"  nudged {nudge:+d}px to fit" if nudge else ""
        print(
            f"  f{index}: {bounds[2] - bounds[0]}x{bounds[3] - bounds[1]} "
            f"baseline {bounds[3] - 1} centre {(bounds[0] + bounds[2]) / 2:.1f}{note}"
        )
    print(f"-> {args.out}")


if __name__ == "__main__":
    main()
