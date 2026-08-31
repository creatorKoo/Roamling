# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Put one row's candidates on a page so they can be watched side by side.

A row is judged by how it moves, and two candidates are judged against each
other. Stills do not settle either question, so each candidate gets a GIF at the
timing the runtime will actually use, plus the frame strip and the measurements
that decide whether it can go into an atlas at all.

Everything is inlined as data URIs: a relative `src` next to the page does not
survive every viewer, and a broken candidate page wastes a review round.

    ./scripts/pyimg.sh scripts/candidate_page.py output/v3/rows/review \\
        --reference output/v3/rows/review/reference/idle-neutral.png \\
        --ms 150,150,150,150,150,280 \\
        --candidate "v1 head-down:cells" --candidate "v2 tail:cells-v2"
"""

from __future__ import annotations

import argparse
from base64 import b64encode
from collections import deque
from pathlib import Path

from PIL import Image, ImageDraw

BACKGROUND = (250, 249, 246, 255)
GROUND = (226, 122, 122, 255)


def components(cell: Image.Image) -> list[int]:
    alpha = cell.getchannel("A").load()
    width, height = cell.size
    seen = [[False] * width for _ in range(height)]
    sizes = []
    for y in range(height):
        for x in range(width):
            if alpha[x, y] <= 8 or seen[y][x]:
                continue
            queue = deque([(y, x)])
            seen[y][x] = True
            count = 0
            while queue:
                cy, cx = queue.popleft()
                count += 1
                for dy in (-1, 0, 1):
                    for dx in (-1, 0, 1):
                        ny, nx = cy + dy, cx + dx
                        if (
                            0 <= ny < height
                            and 0 <= nx < width
                            and alpha[nx, ny] > 8
                            and not seen[ny][nx]
                        ):
                            seen[ny][nx] = True
                            queue.append((ny, nx))
            sizes.append(count)
    return sorted(sizes, reverse=True)


def framed(cell: Image.Image, scale: int, ground: int) -> Image.Image:
    canvas = Image.new("RGBA", cell.size, BACKGROUND)
    canvas.alpha_composite(cell)
    ImageDraw.Draw(canvas).line(
        [(0, ground), (canvas.width, ground)], fill=GROUND, width=1
    )
    return canvas.convert("RGB").resize(
        (cell.width * scale, cell.height * scale), Image.NEAREST
    )


def build(cells: list[Image.Image], durations: list[int], scale: int, ground: int, out: Path, name: str):
    frames = [framed(cell, scale, ground) for cell in cells]
    montage = Image.new("RGB", (frames[0].width * len(frames), frames[0].height))
    for offset, frame in enumerate(frames):
        montage.paste(frame, (offset * frames[0].width, 0))
    palette = montage.quantize(colors=255)
    paletted = [frame.quantize(palette=palette, dither=Image.NONE) for frame in frames]
    gif = out / f"{name}.gif"
    paletted[0].save(
        gif,
        save_all=True,
        append_images=paletted[1:],
        duration=durations,
        loop=0,
        disposal=1,
        optimize=False,
    )
    strip = out / f"{name}-strip.png"
    montage.resize(
        (montage.width // scale, montage.height // scale), Image.NEAREST
    ).save(strip)
    return gif, strip


def uri(path: Path) -> str:
    mime = "image/gif" if path.suffix == ".gif" else "image/png"
    return f"data:{mime};base64,{b64encode(path.read_bytes()).decode('ascii')}"


PAGE = """<!doctype html>
<meta charset="utf-8"><title>{title}</title>
<style>
 body {{ background:#14110f; color:#e8e2da; font:14px/1.6 ui-sans-serif,system-ui,sans-serif;
        margin:0; padding:32px 40px; }}
 h1 {{ font-size:18px; margin:0 0 24px; }}
 .cand {{ border-top:1px solid #2a251f; padding:20px 0; }}
 .head {{ display:flex; gap:20px; align-items:center; }}
 .art {{ background:#faf9f6; border-radius:6px; padding:6px; }}
 .art img {{ display:block; image-rendering:pixelated; }}
 h2 {{ font-size:15px; margin:0 0 6px; }}
 table {{ border-collapse:collapse; font-size:12px; margin-top:4px; }}
 td, th {{ padding:1px 10px 1px 0; text-align:left; color:#c9c1b7; }}
 th {{ color:#8d857b; font-weight:500; }}
 .bad {{ color:#e08a7a; }}
 .strip {{ margin-top:12px; }}
 .strip img {{ max-width:100%; image-rendering:pixelated; border-radius:4px; }}
</style>
<h1>{title}</h1>
{rows}
"""

CARD = """<div class="cand">
  <div class="head">
    <div class="art"><img src="{gif}"></div>
    <div>
      <h2>{name}</h2>
      <table>{table}</table>
    </div>
  </div>
  <div class="strip"><img src="{strip}"></div>
</div>"""


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path, help="row directory")
    parser.add_argument(
        "--candidate",
        action="append",
        required=True,
        help="LABEL:subdir, optionally LABEL:subdir:MS,MS,... to override the timing",
    )
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--ms", required=True, help="per-frame durations, comma separated")
    parser.add_argument("--scale", type=int, default=3)
    parser.add_argument("--ground", type=int, default=175)
    parser.add_argument("--title", default=None)
    args = parser.parse_args()

    durations = [int(v) for v in args.ms.split(",")]
    entries = []

    if args.reference:
        ref = Image.open(args.reference).convert("RGBA")
        gif, strip = build([ref], [1000], args.scale, args.ground, args.root, "reference")
        entries.append(
            ("reference (approved idle)", gif, strip, "<tr><td>identity lock</td></tr>")
        )

    for item in args.candidate:
        label, subdir, *override = item.split(":")
        # A timing override lets two candidates that share frames be compared on
        # how long those frames hold, which is its own decision.
        own = [int(v) for v in override[0].split(",")] if override else durations
        cells = [
            Image.open(path).convert("RGBA")
            for path in sorted((args.root / subdir).glob("*.png"))
        ]
        if len(cells) != len(own):
            raise SystemExit(f"{label}: {len(cells)} cells but {len(own)} durations")
        gif, strip = build(cells, own, args.scale, args.ground, args.root, label.replace(" ", "-"))
        rows = ["<tr><th>frame</th><th>size</th><th>baseline</th><th>centre</th><th>pieces</th></tr>"]
        for index, cell in enumerate(cells):
            box = cell.getchannel("A").point(lambda v: 255 if v > 8 else 0).getbbox()
            sizes = components(cell)
            extra = len(sizes) - 1
            flag = ' class="bad"' if extra else ""
            rows.append(
                f"<tr><td>{index}</td>"
                f"<td>{box[2] - box[0]}x{box[3] - box[1]}</td>"
                f"<td>{box[3] - 1}</td>"
                f"<td>{(box[0] + box[2]) / 2:.1f}</td>"
                f"<td{flag}>{extra}</td></tr>"
            )
        total = sum(own) / 1000
        rows.append(f"<tr><td colspan=5>{len(cells)} frames · {total:.2f}s</td></tr>")
        entries.append((label, gif, strip, "".join(rows)))

    cards = "\n".join(
        CARD.format(name=name, gif=uri(gif), strip=uri(strip), table=table)
        for name, gif, strip, table in entries
    )
    page = args.root / "candidates.html"
    page.write_text(PAGE.format(title=args.title or args.root.name, rows=cards))
    print(f"-> file://{page.resolve()}")


if __name__ == "__main__":
    main()
