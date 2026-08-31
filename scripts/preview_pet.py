# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Render a pet package's animations as GIFs and filmstrips.

Reviewing a row means watching it, not reading its frame list. This renders what
the runtime actually plays: the manifest's frame order and fps, with
`roamling.json` overrides applied on top the same way the loader applies them, so
a track that Roamling retimes is previewed at Roamling's length rather than the
Petdex one.

The ground line is drawn by default. Most defects this repo has chased were
baseline drift, and they are invisible until something straight sits next to the
sprite.

    ./scripts/pyimg.sh scripts/preview_pet.py ~/.codex/pets/mochi-v2 -o output/preview
    ./scripts/pyimg.sh scripts/preview_pet.py <pkg> --only waving --scale 3
"""

from __future__ import annotations

import argparse
import json
from base64 import b64encode
from pathlib import Path

from PIL import Image, ImageDraw

BACKGROUND = (250, 249, 246, 255)
GROUND = (226, 122, 122, 255)


def load_manifest(package: Path) -> tuple[dict, dict]:
    manifest = json.loads((package / "pet.json").read_text())
    extension = {}
    extension_path = package / "roamling.json"
    if extension_path.exists():
        extension = json.loads(extension_path.read_text())
    return manifest, extension


def tracks_from(manifest: dict, extension: dict) -> dict[str, dict]:
    """Manifest tracks, then extension tracks on top -- the loader's order."""
    tracks = dict(manifest.get("animations") or {})
    if extension.get("schemaVersion") == 1:
        tracks.update(extension.get("animations") or {})
    return tracks


def extension_sheet(package: Path, extension: dict) -> tuple[Image.Image | None, int]:
    """The extension sheet and its column count, or nothing if there is none."""
    if extension.get("schemaVersion") != 1:
        return None, 0
    path = extension.get("spritesheetPath")
    grid = extension.get("frame") or {}
    if not path or not grid.get("columns"):
        return None, 0
    return Image.open(package / path).convert("RGBA"), grid["columns"]


def frame_image(
    sheet: Image.Image,
    index: int,
    frame: dict,
    extension: Image.Image | None = None,
    extension_columns: int = 0,
) -> Image.Image:
    """Frames continue past the package grid onto the extension sheet."""
    width, height = frame["width"], frame["height"]
    base_cells = frame["columns"] * frame["rows"]
    if index < base_cells or extension is None:
        source, offset, stride = sheet, index, frame["columns"]
    else:
        source, offset, stride = extension, index - base_cells, extension_columns
    column, row = offset % stride, offset // stride
    return source.crop(
        (column * width, row * height, (column + 1) * width, (row + 1) * height)
    )


def compose(cell: Image.Image, scale: int, ground: int | None) -> Image.Image:
    canvas = Image.new("RGBA", cell.size, BACKGROUND)
    canvas.alpha_composite(cell)
    if ground is not None and 0 <= ground < cell.height:
        ImageDraw.Draw(canvas).line(
            [(0, ground), (cell.width, ground)], fill=GROUND, width=1
        )
    if scale != 1:
        canvas = canvas.resize(
            (canvas.width * scale, canvas.height * scale), Image.NEAREST
        )
    return canvas.convert("RGB")


def collapse(indices: list[int], step_ms: int) -> list[tuple[int, int]]:
    """Runs of the same cell become one frame that holds longer.

    Manifests build a pause by repeating an index -- `idle` holds its first cell
    twelve steps. Writing those as twelve identical GIF frames makes the encoder
    drop them as duplicates and the pause disappears, so the hold is expressed as
    a duration instead, which is what a GIF wants anyway.
    """
    runs: list[tuple[int, int]] = []
    for index in indices:
        if runs and runs[-1][0] == index:
            runs[-1] = (index, runs[-1][1] + step_ms)
        else:
            runs.append((index, step_ms))
    return runs


def render(
    name: str,
    track: dict,
    sheet: Image.Image,
    frame: dict,
    out: Path,
    scale: int,
    ground: int | None,
    extension: Image.Image | None = None,
    extension_columns: int = 0,
) -> str:
    indices = track["frames"]
    fps = track.get("fps") or 12
    runs = collapse(indices, max(20, round(1000 / fps)))
    frames = [
        compose(
            frame_image(sheet, index, frame, extension, extension_columns), scale, ground
        )
        for index, _ in runs
    ]

    # One palette for the whole track. Quantising each frame on its own lets the
    # palette drift between frames, which reads as the colours crawling.
    montage = Image.new("RGB", (frames[0].width * len(frames), frames[0].height))
    for offset, image in enumerate(frames):
        montage.paste(image, (offset * frames[0].width, 0))
    palette = montage.quantize(colors=255, method=Image.MEDIANCUT)
    paletted = [image.quantize(palette=palette, dither=Image.NONE) for image in frames]

    gif = out / f"{name}.gif"
    paletted[0].save(
        gif,
        save_all=True,
        append_images=paletted[1:],
        duration=[duration for _, duration in runs],
        loop=0 if track.get("loop", True) else 1,
        # Every frame is a full opaque repaint, so the previous one is simply
        # left in place. `disposal=2` clears to background between frames and
        # shows through as a flash on each step.
        disposal=1,
        optimize=False,
    )
    # The page inlines this, so keep it at sheet scale rather than preview scale.
    strip = montage.resize(
        (montage.width // scale, montage.height // scale), Image.NEAREST
    ) if scale > 1 else montage
    strip.save(out / f"{name}-strip.png")

    total = len(indices) / fps
    loop = "loop" if track.get("loop", True) else "once"
    return (
        f"{name:14s} {len(indices):3d} steps -> {len(runs):2d} frames  "
        f"{total:5.2f}s  {loop}"
    )


PAGE = """<!doctype html>
<meta charset="utf-8"><title>{title}</title>
<style>
 body {{ background:#14110f; color:#e8e2da; font:14px/1.6 ui-sans-serif,system-ui,sans-serif;
        margin:0; padding:32px 40px; }}
 h1 {{ font-size:18px; font-weight:600; margin:0 0 4px; }}
 p.sub {{ color:#8d857b; margin:0 0 28px; }}
 .row {{ display:flex; gap:24px; align-items:center; padding:18px 0;
         border-top:1px solid #2a251f; }}
 .art {{ background:#faf9f6; border-radius:6px; padding:6px; flex:0 0 auto; }}
 .art img {{ display:block; image-rendering:pixelated; }}
 .meta h2 {{ font-size:15px; margin:0 0 6px; font-weight:600; }}
 .meta dl {{ display:grid; grid-template-columns:auto 1fr; gap:2px 14px; margin:0; }}
 dt {{ color:#8d857b; }} dd {{ margin:0; }}
 .strip {{ margin-top:10px; opacity:.8; }}
 .strip img {{ max-width:100%; image-rendering:pixelated; border-radius:4px; }}
 .badge {{ font-size:11px; padding:2px 8px; border-radius:99px; margin-left:8px;
           vertical-align:2px; font-weight:600; }}
 .approved {{ background:#1f3a24; color:#7fd18f; }}
 .pending {{ background:#3a3320; color:#d9b45f; }}
 .note {{ color:#8d857b; margin-top:6px; max-width:60ch; }}
</style>
<h1>{title}</h1>
<p class="sub">{subtitle}</p>
{rows}
"""

ROW = """<div class="row">
  <div class="art"><img src="{gif}" alt="{name}"></div>
  <div class="meta">
    <h2>{name}<span class="badge {status}">{status}</span></h2>
    <dl>
      <dt>프레임</dt><dd>{steps} 스텝 · 셀 {cells}</dd>
      <dt>길이</dt><dd>{duration:.2f}s @ {fps:.2f} fps · {loop}</dd>
    </dl>
    <div class="note">{note}</div>
    <div class="strip"><img src="{strip}" alt="{name} strip"></div>
  </div>
</div>"""


def data_uri(path: Path) -> str:
    """Inline the asset rather than linking it.

    A relative `src` next to the page is the obvious thing and it does not
    survive contact with every viewer -- some refuse `file://` subresources
    outright, and the page then shows nothing but broken icons. One
    self-contained file always opens.
    """
    mime = "image/gif" if path.suffix == ".gif" else "image/png"
    return f"data:{mime};base64,{b64encode(path.read_bytes()).decode('ascii')}"


def write_page(
    out: Path, title: str, subtitle: str, entries: list[dict]
) -> Path:
    rows = "\n".join(
        ROW.format(
            **entry,
            gif=data_uri(out / f"{entry['name']}.gif"),
            strip=data_uri(out / f"{entry['name']}-strip.png"),
        )
        for entry in entries
    )
    page = out / "index.html"
    page.write_text(PAGE.format(title=title, subtitle=subtitle, rows=rows))
    return page


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path, help="pet package directory")
    parser.add_argument("-o", "--out", type=Path, default=Path("output/preview"))
    parser.add_argument("--only", action="append", help="track name (repeatable)")
    parser.add_argument("--scale", type=int, default=2)
    parser.add_argument(
        "--ground",
        type=int,
        default=175,
        help="ground line row inside the cell; -1 to omit",
    )
    parser.add_argument(
        "--html",
        action="store_true",
        help="also write index.html so every track can be watched on one page",
    )
    parser.add_argument("--title", default=None)
    parser.add_argument(
        "--approvals",
        type=Path,
        help="JSON of {track: {status, change|notes}} shown as a badge per row",
    )
    args = parser.parse_args()

    approvals = {}
    if args.approvals and args.approvals.exists():
        approvals = {
            key: value
            for key, value in json.loads(args.approvals.read_text()).items()
            if not key.startswith("_")
        }

    manifest, extension_manifest = load_manifest(args.package)
    sheet = Image.open(args.package / manifest["spritesheetPath"]).convert("RGBA")
    frame = manifest["frame"]
    extension, extension_columns = extension_sheet(args.package, extension_manifest)
    tracks = tracks_from(manifest, extension_manifest)
    wanted = args.only or sorted(tracks)

    args.out.mkdir(parents=True, exist_ok=True)
    ground = None if args.ground < 0 else args.ground
    entries = []
    for name in wanted:
        if name not in tracks:
            print(f"{name}: no such track")
            continue
        track = tracks[name]
        print(
            render(
                name, track, sheet, frame, args.out, args.scale, ground,
                extension, extension_columns,
            )
        )
        fps = track.get("fps") or 12
        record = approvals.get(name, {})
        entries.append(
            {
                "name": name,
                "steps": len(track["frames"]),
                "cells": len(set(track["frames"])),
                "fps": fps,
                "duration": len(track["frames"]) / fps,
                "loop": "loop" if track.get("loop", True) else "once",
                "status": record.get("status", "pending"),
                "note": record.get("change") or record.get("notes", ""),
            }
        )

    # Approved first is the wrong order for a review: what still needs a decision
    # should be at the top, where it is seen without scrolling.
    entries.sort(key=lambda entry: (entry["status"] == "approved", entry["name"]))

    if args.html and entries:
        page = write_page(
            args.out,
            args.title or manifest.get("displayName", "pet"),
            f"{len(entries)} tracks · "
            f"{sum(e['status'] == 'approved' for e in entries)} approved · "
            f"지면선 y={ground} · {args.scale}x",
            entries,
        )
        print(f"\n-> file://{page.resolve()}")
    else:
        print(f"\n-> {args.out}")


if __name__ == "__main__":
    main()
