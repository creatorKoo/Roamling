# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Narrow one row until the animal matches another row's build.

Rows drawn in separate batches come back the same height and a different width.
Mochi's tail-flick row stands 147.5 tall against idle's 148.0 and runs 3 to 8
pixels wider at every height between the ears and the paws -- the same cat, five
percent stouter, which reads as the pet putting on weight whenever it looks at
something.

A uniform scale cannot fix that. Matching the width costs seven pixels of height,
so the pet would shrink vertically on the cut into idle instead. The rows already
agree on height; only the width is wrong, so only the width is changed.

That is an anamorphic scale, which pixel art usually will not survive. It does
here because the correction is small: at 0.955 one column in twenty-two merges,
which softens vertical strokes without breaking them, and resampling is followed
by a snap back to the sheet's own palette so no new colours survive. Check the
result rather than trusting the number -- whiskers are the first thing to go.

The frames are re-seated on the ground line and held at their original head
centre, so the body does not slide sideways. The head rather than the bounding
box, because on a row like this the tail swings and would drag the centre with
it.

    ./scripts/pyimg.sh scripts/match_row_width.py <pkg> --row 8 --reference-row 0 --report
    ./scripts/pyimg.sh scripts/match_row_width.py <pkg> --row 8 --reference-row 0
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image

HEAD_BAND = 0.35
ALPHA_FLOOR = 8


def cells(sheet: Image.Image, row: int, frame: dict) -> list[Image.Image]:
    width, height = frame["width"], frame["height"]
    found = []
    for column in range(frame["columns"]):
        box = (column * width, row * height, (column + 1) * width, (row + 1) * height)
        cell = sheet.crop(box)
        if cell.getchannel("A").point(lambda v: 255 if v > ALPHA_FLOOR else 0).getbbox():
            found.append(cell)
    return found


def width_profile(cell: Image.Image) -> dict[int, int]:
    pixels = cell.getchannel("A").load()
    profile = {}
    for y in range(cell.height):
        xs = [x for x in range(cell.width) if pixels[x, y] > ALPHA_FLOOR]
        if xs:
            profile[y] = max(xs) - min(xs) + 1
    return profile


def head_centre(cell: Image.Image) -> float:
    pixels = cell.getchannel("A").load()
    box = cell.getchannel("A").point(lambda v: 255 if v > ALPHA_FLOOR else 0).getbbox()
    cut = box[1] + round((box[3] - box[1]) * HEAD_BAND)
    xs = [
        x
        for y in range(box[1], min(cut + 1, cell.height))
        for x in range(cell.width)
        if pixels[x, y] > ALPHA_FLOOR
    ]
    return (min(xs) + max(xs)) / 2


def factor(subject: Image.Image, reference: Image.Image, skip_top: int) -> float:
    """How much narrower the subject has to be, measured height by height.

    Taken from one frame of each rather than the whole row: a row whose tail
    swings is wider in some frames than others for a reason that has nothing to
    do with the animal's build.

    The top of the silhouette is skipped because ears taper, so a row whose ears
    sit a pixel higher compares a wide slice against a narrow one and reports a
    difference of several hundred percent.
    """
    a, b = width_profile(subject), width_profile(reference)
    shared = [y for y in a if y in b][skip_top:]
    if not shared:
        raise SystemExit("the two rows share no heights to compare")
    return sum(b[y] for y in shared) / sum(a[y] for y in shared)


def palette_of(cell: Image.Image) -> list[tuple[int, int, int]]:
    """The cell's own colours, which is the whole palette the result may use.

    Not the sheet's: a lossy-WebP atlas carries 136,150 near-identical colours,
    and searching all of them per pixel does not finish. Resampling only blends
    colours that were already in the frame, so the frame's own set is both the
    correct target and a few thousand entries rather than a hundred thousand.
    """
    return list({pixel[:3] for pixel in cell.getdata() if pixel[3] > ALPHA_FLOOR})


def narrowed(
    cell: Image.Image,
    scale: float,
    palette: list[tuple[int, int, int]],
    baseline: int,
) -> Image.Image:
    mask = cell.getchannel("A").point(lambda v: 255 if v > ALPHA_FLOOR else 0)
    box = mask.getbbox()
    centre = head_centre(cell)
    art = cell.crop(box)
    art = art.resize((max(1, round(art.width * scale)), art.height), Image.LANCZOS)

    cache: dict[tuple[int, int, int], tuple[int, int, int]] = {}
    out = []
    for red, green, blue, alpha in art.getdata():
        # Resampling invents colours and soft edges. The palette snap puts every
        # pixel back on a colour the sheet already uses, and the alpha is made
        # binary again so the row keeps the hard edge the rest of the sheet has.
        if alpha < 128:
            out.append((0, 0, 0, 0))
            continue
        key = (red, green, blue)
        if key not in cache:
            cache[key] = min(
                palette,
                key=lambda c: (c[0] - red) ** 2 + (c[1] - green) ** 2 + (c[2] - blue) ** 2,
            )
        out.append((*cache[key], 255))
    art.putdata(out)
    art = art.crop(art.getchannel("A").point(lambda v: 255 if v > 0 else 0).getbbox())

    # Line the narrowed head up with where the head was, not the bounding box.
    # On a row whose tail swings the two are different places, and centring the
    # box slid the body sideways by however far the tail happened to reach --
    # the head drifted 12px across the row where it had held within 0.5px.
    seated = Image.new("RGBA", cell.size, (0, 0, 0, 0))
    offset = round(centre - head_centre(art))
    seated.alpha_composite(art, (offset, baseline + 1 - art.height))
    return seated


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("--row", type=int, required=True)
    parser.add_argument("--reference-row", type=int, required=True)
    parser.add_argument("--frame", type=int, default=0, help="frame compared in each row")
    parser.add_argument("--factor", type=float, help="override the measured factor")
    parser.add_argument("--baseline", type=int, default=175)
    parser.add_argument(
        "--skip-top",
        type=int,
        default=12,
        help="rows of tapering ear to leave out of the measurement",
    )
    parser.add_argument("--report", action="store_true")
    args = parser.parse_args()

    manifest = json.loads((args.package / "pet.json").read_text())
    path = args.package / manifest["spritesheetPath"]
    sheet = Image.open(path).convert("RGBA")
    frame = manifest["frame"]

    subject = cells(sheet, args.row, frame)
    reference = cells(sheet, args.reference_row, frame)
    scale = args.factor or factor(subject[args.frame], reference[args.frame], args.skip_top)
    print(f"row {args.row} -> row {args.reference_row}: horizontal scale {scale:.4f}")

    width, height = frame["width"], frame["height"]
    for index, cell in enumerate(subject):
        palette = palette_of(cell)
        before = cell.getchannel("A").point(lambda v: 255 if v > ALPHA_FLOOR else 0).getbbox()
        result = narrowed(cell, scale, palette, args.baseline)
        after = result.getchannel("A").point(lambda v: 255 if v > ALPHA_FLOOR else 0).getbbox()
        print(
            f"  f{index}  {before[2] - before[0]}x{before[3] - before[1]}"
            f" -> {after[2] - after[0]}x{after[3] - after[1]}"
        )
        if args.report:
            continue
        box = (index * width, args.row * height)
        sheet.paste(Image.new("RGBA", (width, height), (0, 0, 0, 0)), box)
        sheet.alpha_composite(result, box)

    if not args.report:
        sheet.save(path, lossless=True)
        print(f"-> {path}")


if __name__ == "__main__":
    main()
