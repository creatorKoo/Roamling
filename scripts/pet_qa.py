# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Measure a pet's frames against the invariants in docs/art.

Run through the Pillow wrapper, since neither system nor Homebrew python has it:

    ./scripts/pyimg.sh scripts/pet_qa.py <sheet.png|frames-dir> [options]

The invariants are the ones frames actually break. A frame whose baseline sits a
few pixels off makes the pet bob between states; one with a second alpha
component has a whisker or a paw floating beside it. Both are invisible in a
contact sheet at 100% and obvious in a table of numbers, which is why this
exists rather than a reviewer squinting at fifty-seven frames.

The centre is checked within a row rather than against the cell's middle. The
defect this catches is the animal sliding while it animates, which is a
statement about a frame and its neighbours; a row drawn a pixel off-centre as a
whole is a different and much smaller problem, so it is checked separately and
more loosely.

Which measurement means "the body did not move" depends on what the row does,
and no single one works everywhere. Measured across this pet, the silhouette's
bounding centre holds within 0.5px on eight of nine rows -- and drifts 12.5px on
the tail flick, where the cat is provably still and only its tail swings. The
head band is exact there and useless on a run, where the head lunges 39.5px
ahead of the body. So the measure is declared per row and defaults to the
bounding centre.

Exits non-zero when any frame fails, so it can gate a generation loop.
"""

from __future__ import annotations

import argparse
import sys
from collections import defaultdict, deque
from pathlib import Path

from PIL import Image

ALPHA_FLOOR = 8
HEAD_BAND = 0.35


class FrameStats:
    """Where a frame's cat actually sits inside its cell."""

    def __init__(self, name: str, image: Image.Image) -> None:
        self.name = name
        alpha = image.convert("RGBA").getchannel("A")
        width, height = alpha.size
        pixels = alpha.load()
        self.mask = [[pixels[x, y] > ALPHA_FLOOR for x in range(width)] for y in range(height)]
        self.width = width
        self.height = height
        xs = [x for y in range(height) for x in range(width) if self.mask[y][x]]
        ys = [y for y in range(height) for x in range(width) if self.mask[y][x]]
        self.empty = not xs
        if self.empty:
            return
        self.baseline = max(ys)
        self.top = min(ys)
        self.visible_height = self.baseline - self.top + 1
        self.center_x = (min(xs) + max(xs)) / 2
        self.head_x = self._head_center()
        self.components = self._components()

    def _head_center(self) -> float:
        """Bounding centre of the top band only.

        A tail, a stretched hind leg or an outflung paw moves the silhouette's
        bounding centre without moving the animal. The head does not swing, so
        on rows where something else does, this is what "the body stayed put"
        actually means.
        """
        cut = self.top + round(self.visible_height * HEAD_BAND)
        xs = [
            x
            for y in range(self.top, min(cut + 1, self.height))
            for x in range(self.width)
            if self.mask[y][x]
        ]
        return (min(xs) + max(xs)) / 2

    def measured(self, measure: str) -> float:
        return self.head_x if measure == "head" else self.center_x

    def _components(self) -> list[int]:
        """Sizes of the 8-connected opaque regions, largest first.

        Anything past the first is a detached piece. The art rules say to redraw
        the whole frame rather than patch one, so the count is what matters.
        """
        seen = [[False] * self.width for _ in range(self.height)]
        sizes: list[int] = []
        for y in range(self.height):
            for x in range(self.width):
                if not self.mask[y][x] or seen[y][x]:
                    continue
                queue = deque([(x, y)])
                seen[y][x] = True
                count = 0
                while queue:
                    cx, cy = queue.popleft()
                    count += 1
                    for dy in (-1, 0, 1):
                        for dx in (-1, 0, 1):
                            nx, ny = cx + dx, cy + dy
                            if (
                                0 <= nx < self.width
                                and 0 <= ny < self.height
                                and self.mask[ny][nx]
                                and not seen[ny][nx]
                            ):
                                seen[ny][nx] = True
                                queue.append((nx, ny))
                sizes.append(count)
        sizes.sort(reverse=True)
        return sizes


def load_frames(source: Path, columns: int, rows: int) -> list[tuple[str, Image.Image]]:
    if source.is_dir():
        return [(p.name, Image.open(p)) for p in sorted(source.glob("*.png"))]
    sheet = Image.open(source).convert("RGBA")
    cell_w = sheet.width // columns
    cell_h = sheet.height // rows
    if cell_w * columns != sheet.width or cell_h * rows != sheet.height:
        raise SystemExit(
            f"{source.name} is {sheet.width}x{sheet.height}, "
            f"which is not {columns}x{rows} whole cells"
        )
    frames = []
    for row in range(rows):
        for column in range(columns):
            box = (column * cell_w, row * cell_h, (column + 1) * cell_w, (row + 1) * cell_h)
            frames.append((f"r{row}c{column}", sheet.crop(box)))
    return frames


def row_of(name: str) -> int:
    """The row a frame name belongs to; a directory of frames is one row."""
    if not name.startswith("r") or "c" not in name:
        return -1
    head = name[1:].split("c")[0]
    return int(head) if head.isdigit() else -1


def median(values: list[float]) -> float:
    ordered = sorted(values)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[middle]
    return (ordered[middle - 1] + ordered[middle]) / 2


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="spritesheet, or a directory of frame PNGs")
    parser.add_argument("--columns", type=int, default=8)
    parser.add_argument("--rows", type=int, default=9)
    parser.add_argument(
        "--baseline",
        type=int,
        help="required bottom-most opaque row. Omit to take the median and report drift.",
    )
    parser.add_argument("--baseline-tolerance", type=int, default=0)
    parser.add_argument(
        "--center",
        type=float,
        help="where a row should sit in the cell. Defaults to the cell's own middle.",
    )
    parser.add_argument(
        "--center-tolerance",
        type=float,
        default=1.0,
        help="how far a frame may sit from its own row's centre",
    )
    parser.add_argument(
        "--row-center-tolerance",
        type=float,
        default=3.0,
        help="how far a whole row may sit from the cell's centre",
    )
    parser.add_argument("--max-components", type=int, default=1)
    parser.add_argument(
        "--allow-airborne",
        action="append",
        default=[],
        help="a row index, or one frame as rRcC, whose baseline may move. Repeatable.",
    )
    parser.add_argument(
        "--center-measure",
        action="append",
        default=[],
        help="ROW=head to judge that row by its head band instead of its "
        "silhouette, for a row where a tail or a limb swings. Repeatable.",
    )
    args = parser.parse_args()

    frames = [
        (name, FrameStats(name, image))
        for name, image in load_frames(args.source, args.columns, args.rows)
    ]
    frames = [(name, stats) for name, stats in frames if not stats.empty]
    if not frames:
        raise SystemExit("no frames with any opaque pixels")

    baselines = sorted(stats.baseline for _, stats in frames)
    baseline = args.baseline if args.baseline is not None else baselines[len(baselines) // 2]
    center = args.center if args.center is not None else (frames[0][1].width - 1) / 2

    # A bare row index exempts the row, as it did before; rRcC exempts one frame,
    # so a jump's grounded frames stay pinned instead of the whole row going
    # unchecked because two of its five cells leave the floor.
    airborne_rows = {int(v) for v in args.allow_airborne if "c" not in v}
    airborne_frames = {v for v in args.allow_airborne if "c" in v}

    measures: dict[int, str] = {}
    for item in args.center_measure:
        key, _, value = item.partition("=")
        if value not in ("bbox", "head"):
            raise SystemExit(f"--center-measure {item}: want bbox or head")
        measures[int(key)] = value

    grouped: dict[int, list[tuple[str, FrameStats]]] = defaultdict(list)
    for name, stats in frames:
        grouped[row_of(name)].append((name, stats))

    print(f"target  baseline {baseline}  centre {center:.1f}  components <= {args.max_components}")
    if args.baseline is None:
        print("        (baseline taken from the median frame; pass --baseline to fix it)")
    print()

    failures: list[tuple[str, list[str]]] = []
    for row in sorted(grouped):
        entries = grouped[row]
        measure = measures.get(row, "bbox")
        row_center = median([stats.measured(measure) for _, stats in entries])

        # A row uniformly off-centre is a placement mistake, not the animal
        # sliding, so it is one failure against the row rather than one per
        # frame. Judged on the silhouette either way: a head band is measured
        # against its own row, not against where other rows put their heads.
        row_bbox = median([stats.center_x for _, stats in entries])
        if abs(row_bbox - center) > args.row_center_tolerance:
            failures.append(
                (f"r{row}" if row >= 0 else "row", [f"row centre {row_bbox:.1f} (off by {row_bbox - center:+.1f})"])
            )

        for name, stats in entries:
            problems = []
            exempt = row in airborne_rows or name in airborne_frames
            if not exempt and abs(stats.baseline - baseline) > args.baseline_tolerance:
                problems.append(
                    f"baseline {stats.baseline} (off by {stats.baseline - baseline:+d})"
                )
            drift = stats.measured(measure) - row_center
            if abs(drift) > args.center_tolerance:
                label = "head" if measure == "head" else "centre"
                problems.append(
                    f"{label} {stats.measured(measure):.1f} (off row by {drift:+.1f})"
                )
            if len(stats.components) > args.max_components:
                detached = stats.components[args.max_components :]
                problems.append(f"{len(detached)} detached piece(s), {detached}px")
            if problems:
                failures.append((name, problems))

    for name, problems in failures:
        print(f"  FAIL {name:10} {'; '.join(problems)}")

    print()
    for row in sorted(grouped):
        entries = grouped[row]
        measure = measures.get(row, "bbox")
        values = [stats.measured(measure) for _, stats in entries]
        label = f"r{row}" if row >= 0 else "frames"
        print(
            f"  {label:6} {len(entries):2d} frames  {measure:4}"
            f"  spread {max(values) - min(values):4.1f}px"
            f"  baseline {min(s.baseline for _, s in entries)}~{max(s.baseline for _, s in entries)}"
        )

    heights = [stats.visible_height for _, stats in frames]
    print()
    print(f"{len(frames)} frames, {len(failures)} failing")
    print(f"visible height {min(heights)}~{max(heights)}  baseline {baselines[0]}~{baselines[-1]}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
