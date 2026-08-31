# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Put every row of a pet on the same colours.

Rows drawn in separate batches drift apart in shade even when each looks right
alone. Measured across this pet the body cream ran from (254,233,207) on jumping
to (254,252,237) on review, and the dark brown from (95,44,21) to (118,63,35).
No single row looks wrong; watching the animal move between them, it changes
colour.

Quantising the sheets together does not fix this. Median cut allocates palette
entries by population, and each row's cream is populous enough to earn its own
entry -- asked for 24 colours it returns eight separate creams, which is the
drift preserved rather than removed.

So the correction is per family. Every colour here sits on one warm ramp, and
the ramp splits by lightness into dark brown, orange, tan and cream, with the
outline black below all of them. Each row's own anchor for a family is measured,
the shared target is the median of those anchors across rows -- the middle, so
no row is treated as the reference -- and the row is translated onto it.

Translating keeps each pixel's distance from its family anchor, so shading,
highlights and anti-aliased edges survive intact; only where the family sits
moves. Black is pinned, being already identical everywhere. Alpha is never
touched: this is a recolour, and the silhouette is not ours to change.

    ./scripts/pyimg.sh scripts/unify_palette.py <package> --report
    ./scripts/pyimg.sh scripts/unify_palette.py <package> --out <dir>
"""

from __future__ import annotations

import argparse
import colorsys
import json
from collections import Counter
from pathlib import Path

from PIL import Image

# Lightness bands, in percent. Anything below the first is outline black, which
# is pinned rather than corrected.
BLACK_CEILING = 10.0
# The lightness gaps between bands are deliberate. A band boundary drawn through
# a gradient catches a different mix of real colour and anti-aliased edge on
# every row, and a family measured that way is not the same family twice: a
# "tan" band between orange and cream anchored on light orange for one row and
# on genuine tan for another, asking for a 31-level correction between two rows
# that did not disagree. Pixels in a gap are not ignored -- they take the
# correction of whichever anchor they are nearest.
FAMILIES = (
    ("dark", 10.0, 40.0),
    ("orange", 42.0, 58.0),
    ("cream", 82.0, 100.1),
)
BUCKET = 8

Colour = tuple[int, int, int]


def lightness(colour: Colour) -> float:
    return colorsys.rgb_to_hls(*[v / 255 for v in colour])[1] * 100


def family_of(colour: Colour) -> str | None:
    light = lightness(colour)
    if light < BLACK_CEILING:
        return None
    for name, low, high in FAMILIES:
        if low <= light < high:
            return name
    return None


def anchor(census: Counter[Colour]) -> Colour:
    """The family's representative colour, robust to lossy-WebP smear.

    A sheet saved as lossy WebP stores a flat cream as thousands of creams a
    level or two apart, so the single most common colour is an arbitrary member
    of that cloud. Bucketing first finds the cloud, then averaging inside it
    finds its centre.
    """
    buckets: Counter[tuple[int, int, int]] = Counter()
    for colour, count in census.items():
        buckets[tuple(v // BUCKET for v in colour)] += count
    winner = buckets.most_common(1)[0][0]
    members = [(c, n) for c, n in census.items() if tuple(v // BUCKET for v in c) == winner]
    total = sum(n for _, n in members)
    return tuple(
        round(sum(c[channel] * n for c, n in members) / total) for channel in range(3)
    )


def median(colours: list[Colour]) -> Colour:
    """Component-wise median -- the middle row, not the average of all of them."""
    return tuple(sorted(c[channel] for c in colours)[len(colours) // 2] for channel in range(3))


def bands_of(image: Image.Image, rows: int) -> list[Image.Image]:
    height = image.height // rows
    return [image.crop((0, r * height, image.width, (r + 1) * height)) for r in range(rows)]


def sheets_of(package: Path) -> list[tuple[Path, int]]:
    """Each sheet with its row count, so bands line up with drawing batches."""
    manifest = json.loads((package / "pet.json").read_text())
    found = [(package / manifest["spritesheetPath"], manifest["frame"]["rows"])]
    extension_path = package / "roamling.json"
    if extension_path.exists():
        extension = json.loads(extension_path.read_text())
        if extension.get("schemaVersion") == 1 and (path := extension.get("spritesheetPath")):
            found.append((package / path, extension["frame"]["rows"]))
    return found


def anchors_of(band: Image.Image) -> dict[str, Colour]:
    census: dict[str, Counter[Colour]] = {name: Counter() for name, _, _ in FAMILIES}
    for pixel in band.getdata():
        if pixel[3] <= 200:
            continue
        if name := family_of(pixel[:3]):
            census[name][pixel[:3]] += 1
    return {name: anchor(counts) for name, counts in census.items() if counts}


def translate(band: Image.Image, deltas: dict[str, Colour], anchors: dict[str, Colour]) -> Image.Image:
    """Move every pixel by the correction for the family nearest to it.

    Nearest anchor rather than the lightness band it fell in: an anti-aliased
    pixel sits between two families, and picking by distance keeps it with the
    one it is actually a blend of.
    """
    targets = [((0, 0, 0), (0, 0, 0))] + [
        (anchors[name], deltas[name]) for name in anchors if name in deltas
    ]
    cache: dict[Colour, Colour] = {}
    out = []
    for pixel in band.getdata():
        if pixel[3] == 0:
            out.append(pixel)
            continue
        key = pixel[:3]
        if key not in cache:
            _, delta = min(
                targets,
                key=lambda item: sum((item[0][i] - key[i]) ** 2 for i in range(3)),
            )
            cache[key] = tuple(min(255, max(0, key[i] + delta[i])) for i in range(3))
        out.append((*cache[key], pixel[3]))
    result = Image.new("RGBA", band.size)
    result.putdata(out)
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("--out", type=Path, help="write here instead of in place")
    parser.add_argument("--report", action="store_true", help="print the table, change nothing")
    args = parser.parse_args()

    sheets = sheets_of(args.package)
    images = [(path, Image.open(path).convert("RGBA"), rows) for path, rows in sheets]

    measured: list[tuple[str, dict[str, Colour]]] = []
    for path, image, rows in images:
        for index, band in enumerate(bands_of(image, rows)):
            measured.append((f"{path.stem}[{index}]", anchors_of(band)))

    targets = {
        name: median([found[name] for _, found in measured if name in found])
        for name, _, _ in FAMILIES
        if any(name in found for _, found in measured)
    }
    print("target (median across rows):")
    for name, colour in targets.items():
        print(f"  {name:7s} {colour}")

    print("\nper row, anchor -> shift:")
    for label, found in measured:
        parts = []
        for name in targets:
            if name not in found:
                continue
            shift = tuple(targets[name][i] - found[name][i] for i in range(3))
            parts.append(f"{name} {found[name]}{'' if any(shift) else ' ='}{'' if not any(shift) else f' {shift}'}")
        print(f"  {label:18s} " + "  ".join(parts))
    if args.report:
        return

    destination = args.out or args.package
    destination.mkdir(parents=True, exist_ok=True)
    cursor = 0
    for path, image, rows in images:
        height = image.height // rows
        result = Image.new("RGBA", image.size)
        for index, band in enumerate(bands_of(image, rows)):
            found = measured[cursor + index][1]
            deltas = {
                name: tuple(targets[name][i] - found[name][i] for i in range(3))
                for name in found
                if name in targets
            }
            result.paste(translate(band, deltas, found), (0, index * height))
        cursor += rows
        result.save(destination / path.name, lossless=True)
        print(f"  {path.name} -> {destination / path.name}")


if __name__ == "__main__":
    main()
