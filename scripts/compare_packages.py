# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Play two versions of the same pet next to each other, track by track.

A change that touches every frame -- a palette pass, a baseline pass, a rescale
-- cannot be judged from one row. What matters is whether the animal still looks
like itself while it moves between rows, and that only shows when the same track
runs side by side at the same timing.

Each track is rendered from each package with the runtime's own frame order and
fps, so the pair differ by exactly the thing being reviewed. A swatch table
above them reports each row's measured anchors, because a few levels of drift
are easier to confirm as numbers than to argue about from a GIF.

    ./scripts/pyimg.sh scripts/compare_packages.py -o output/v3/compare \\
        --package "current:output/v3/mochi-v3" \\
        --package "unified:output/v3/mochi-v3-unified"
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))

from preview_pet import (  # noqa: E402
    data_uri,
    extension_sheet,
    load_manifest,
    render,
    tracks_from,
)
from unify_palette import anchors_of, bands_of, sheets_of  # noqa: E402

PAGE = """<!doctype html>
<meta charset="utf-8"><title>{title}</title>
<style>
 body {{ background:#14110f; color:#e8e2da; font:14px/1.6 ui-sans-serif,system-ui,sans-serif;
        margin:0; padding:32px 40px; }}
 h1 {{ font-size:18px; margin:0 0 4px; }}
 .lede {{ color:#8d857b; margin:0 0 28px; max-width:62em; }}
 .track {{ border-top:1px solid #2a251f; padding:18px 0; }}
 h2 {{ font-size:15px; margin:0 0 10px; }}
 h2 span {{ color:#8d857b; font-weight:400; font-size:13px; margin-left:10px; }}
 .pair {{ display:flex; gap:22px; flex-wrap:wrap; }}
 .one {{ }}
 .label {{ font-size:12px; color:#8d857b; margin-bottom:5px; }}
 .art {{ background:#faf9f6; border-radius:6px; padding:6px; display:inline-block; }}
 .art img {{ display:block; image-rendering:pixelated; }}
 table {{ border-collapse:collapse; font-size:12px; margin:0 0 28px; }}
 td, th {{ padding:2px 12px 2px 0; text-align:left; color:#c9c1b7; white-space:nowrap; }}
 th {{ color:#8d857b; font-weight:500; }}
 .sw {{ display:inline-block; width:11px; height:11px; border-radius:2px;
        vertical-align:-1px; margin-right:5px; border:1px solid #00000040; }}
</style>
<h1>{title}</h1>
<p class="lede">{lede}</p>
{chart}
{tracks}
"""


def swatch(colour: tuple[int, int, int]) -> str:
    return f'<span class="sw" style="background:rgb{colour}"></span>{colour}'


def chart(packages: list[tuple[str, Path]]) -> str:
    """Each row's measured anchors, one column per package."""
    measured: dict[str, list[tuple[str, dict]]] = {}
    for label, package in packages:
        rows = []
        for path, count in sheets_of(package):
            image = Image.open(path).convert("RGBA")
            for index, band in enumerate(bands_of(image, count)):
                rows.append((f"{path.stem}[{index}]", anchors_of(band)))
        measured[label] = rows

    families = ("cream", "dark", "orange")
    head = "".join(
        f'<th>{label} · {family}</th>' for family in families for label, _ in packages
    )
    body = []
    names = [name for name, _ in next(iter(measured.values()))]
    for position, name in enumerate(names):
        cells = "".join(
            f"<td>{swatch(measured[label][position][1][family])}</td>"
            if family in measured[label][position][1]
            else "<td>-</td>"
            for family in families
            for label, _ in packages
        )
        body.append(f"<tr><td>{name}</td>{cells}</tr>")
    return f"<table><tr><th>row</th>{head}</tr>{''.join(body)}</table>"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", action="append", required=True, help="LABEL:path")
    parser.add_argument("-o", "--out", type=Path, required=True)
    parser.add_argument("--scale", type=int, default=2)
    parser.add_argument("--ground", type=int, default=175)
    parser.add_argument("--title", default="palette comparison")
    parser.add_argument("--lede", default="")
    args = parser.parse_args()

    packages = []
    for item in args.package:
        label, path = item.split(":", 1)
        packages.append((label, Path(path)))

    args.out.mkdir(parents=True, exist_ok=True)
    rendered: dict[str, dict[str, Path]] = {}
    order: list[str] = []
    for label, package in packages:
        manifest, extension = load_manifest(package)
        sheet = Image.open(package / manifest["spritesheetPath"]).convert("RGBA")
        ext_sheet, ext_columns = extension_sheet(package, extension)
        directory = args.out / label
        directory.mkdir(parents=True, exist_ok=True)
        rendered[label] = {}
        for name, track in tracks_from(manifest, extension).items():
            if name not in order:
                order.append(name)
            render(
                name, track, sheet, manifest["frame"], directory,
                args.scale, args.ground, ext_sheet, ext_columns,
            )
            rendered[label][name] = directory / f"{name}.gif"

    manifest, extension = load_manifest(packages[0][1])
    tracks = tracks_from(manifest, extension)
    sections = []
    for name in order:
        track = tracks[name]
        seconds = len(track["frames"]) / (track.get("fps") or 12)
        pair = "".join(
            f'<div class="one"><div class="label">{label}</div>'
            f'<div class="art"><img src="{data_uri(rendered[label][name])}"></div></div>'
            for label, _ in packages
            if name in rendered[label]
        )
        sections.append(
            f'<div class="track"><h2>{name}'
            f'<span>{len(track["frames"])} frames · {seconds:.2f}s</span></h2>'
            f'<div class="pair">{pair}</div></div>'
        )

    page = args.out / "index.html"
    page.write_text(
        PAGE.format(
            title=args.title,
            lede=args.lede,
            chart=chart(packages),
            tracks="\n".join(sections),
        )
    )
    print(f"-> file://{page.resolve()}")


if __name__ == "__main__":
    main()
