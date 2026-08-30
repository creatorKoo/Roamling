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

Exits non-zero when any frame fails, so it can gate a generation loop.
"""

from __future__ import annotations

import argparse
import sys
from collections import deque
from pathlib import Path

from PIL import Image

ALPHA_FLOOR = 8


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
        self.components = self._components()

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
        help="required horizontal centre. Defaults to the cell's own middle.",
    )
    parser.add_argument("--center-tolerance", type=float, default=1.0)
    parser.add_argument("--max-components", type=int, default=1)
    parser.add_argument(
        "--allow-airborne",
        action="append",
        default=[],
        help="row index whose baseline may move, for a jump or a landing. Repeatable.",
    )
    args = parser.parse_args()

    frames = [
        (name, FrameStats(name, image)) for name, image in load_frames(args.source, args.columns, args.rows)
    ]
    frames = [(name, stats) for name, stats in frames if not stats.empty]
    if not frames:
        raise SystemExit("no frames with any opaque pixels")

    baselines = sorted(stats.baseline for _, stats in frames)
    baseline = args.baseline if args.baseline is not None else baselines[len(baselines) // 2]
    center = args.center if args.center is not None else (frames[0][1].width - 1) / 2
    airborne = {int(value) for value in args.allow_airborne}

    print(f"target  baseline {baseline}  centre {center:.1f}  components <= {args.max_components}")
    if args.baseline is None:
        print("        (baseline taken from the median frame; pass --baseline to fix it)")
    print()

    failures = []
    for name, stats in frames:
        row = int(name[1:].split("c")[0]) if name.startswith("r") else -1
        problems = []
        if row not in airborne and abs(stats.baseline - baseline) > args.baseline_tolerance:
            problems.append(f"baseline {stats.baseline:+d}".replace("+", "") + f" (off by {stats.baseline - baseline:+d})")
        if abs(stats.center_x - center) > args.center_tolerance:
            problems.append(f"centre {stats.center_x:.1f} (off by {stats.center_x - center:+.1f})")
        if len(stats.components) > args.max_components:
            detached = stats.components[args.max_components :]
            problems.append(f"{len(detached)} detached piece(s), {detached}px")
        if problems:
            failures.append((name, problems))

    for name, problems in failures:
        print(f"  FAIL {name:10} {'; '.join(problems)}")

    heights = [stats.visible_height for _, stats in frames]
    print()
    print(f"{len(frames)} frames, {len(failures)} failing")
    print(f"visible height {min(heights)}~{max(heights)}  baseline {baselines[0]}~{baselines[-1]}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
