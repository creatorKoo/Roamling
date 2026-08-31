# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Play a walk cycle against moving ground, at the speed the pet really travels.

A gait cannot be judged from the row alone. The row shows the legs cycling; what
decides whether it reads as running or as skating is how far the pet covers
while they do, and that lives in the runtime's walking speed rather than in the
manifest. So the cell is drawn at its on-screen size, the ground scrolls beneath
it at the tuned speed, and the two are watched together.

Retiming a row to the Petdex standard changes this silently. Mochi's walk ran at
0.727s in v2 and the standard is 1.06s -- a 46% slower cycle over unchanged
ground speed, which stretches the stride from 1.89 body lengths to 2.76 without
a single frame being redrawn.

    ./scripts/pyimg.sh scripts/preview_gait.py <pkg> --track running-right \\
        --speed 160 --cadence 1.0 --cadence 1.46 -o output/gait
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))

from preview_pet import data_uri, extension_sheet, frame_image, load_manifest, tracks_from  # noqa: E402

# The overlay draws a 192x208 cell at 96x104 points, so a cell pixel is half a
# point. Everything here is in points and multiplied up only to render.
POINTS_PER_CELL_PIXEL = 0.5
SKY = (250, 249, 246)
GROUND = (226, 122, 122)
TICK = (208, 196, 184)


def ground_period(total_travel: float, per_frame: float) -> float:
    """Tick spacing that divides the loop, and is not the step size.

    The GIF wraps, so the ground has to come back to where it started or the
    scenery jumps. Spacing equal to one frame's travel divides it perfectly and
    is useless -- every tick lands where the last one was and the ground looks
    frozen while the pet's legs move.
    """
    for count in sorted(range(6, 30), key=lambda n: abs(total_travel / n - 19)):
        spacing = total_travel / count
        if abs(spacing - per_frame) > 2:
            return spacing
    return total_travel / 9


def build(
    cells: list[Image.Image],
    speed: float,
    step_ms: int,
    scale: int,
    width_pt: float,
    cycles: int,
    baseline_pt: float,
) -> tuple[list[Image.Image], list[int], float]:
    per_frame = speed * step_ms / 1000
    sequence = cells * cycles
    total = per_frame * len(sequence)
    spacing = ground_period(total, per_frame)

    cell_w = cells[0].width * POINTS_PER_CELL_PIXEL
    cell_h = cells[0].height * POINTS_PER_CELL_PIXEL
    # The ground goes under the pet's feet, not under the cell. Mochi stands on
    # row 175 of a 208-row cell, so a line at the cell's bottom edge sits 16pt
    # below the paws and the cat reads as floating.
    ground_pt = baseline_pt
    height_pt = ground_pt + 12
    size = (round(width_pt * scale), round(height_pt * scale))
    left = (width_pt - cell_w) / 2

    frames = []
    for index, cell in enumerate(sequence):
        canvas = Image.new("RGB", size, SKY)
        draw = ImageDraw.Draw(canvas)
        offset = (per_frame * index) % spacing
        # The pet holds the middle and the world slides past it, which is what a
        # loop can express; a pet crossing the frame cannot repeat seamlessly.
        x = -offset
        while x < width_pt:
            if x >= 0:
                draw.line(
                    [(x * scale, (ground_pt + 1) * scale), (x * scale, (ground_pt + 9) * scale)],
                    fill=TICK, width=max(1, scale // 2),
                )
            x += spacing
        draw.line(
            [(0, ground_pt * scale), (size[0], ground_pt * scale)],
            fill=GROUND, width=max(1, scale // 2),
        )
        art = cell.resize(
            (round(cell_w * scale), round(cell_h * scale)), Image.NEAREST
        )
        layer = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
        layer.paste(art, (round(left * scale), 0))
        canvas = Image.alpha_composite(canvas.convert("RGBA"), layer).convert("RGB")
        frames.append(canvas)
    return frames, [step_ms] * len(sequence), total


def save_gif(frames: list[Image.Image], durations: list[int], path: Path) -> None:
    montage = Image.new("RGB", (frames[0].width * len(frames), frames[0].height))
    for offset, frame in enumerate(frames):
        montage.paste(frame, (offset * frames[0].width, 0))
    palette = montage.quantize(colors=255, method=Image.MEDIANCUT)
    paletted = [frame.quantize(palette=palette, dither=Image.NONE) for frame in frames]
    paletted[0].save(
        path, save_all=True, append_images=paletted[1:], duration=durations,
        loop=0, disposal=1, optimize=False,
    )


PAGE = """<!doctype html>
<meta charset="utf-8"><title>{title}</title>
<style>
 body {{ background:#14110f; color:#e8e2da; font:14px/1.6 ui-sans-serif,system-ui,sans-serif;
        margin:0; padding:32px 40px; }}
 h1 {{ font-size:18px; margin:0 0 4px; }}
 .lede {{ color:#8d857b; margin:0 0 24px; max-width:64em; }}
 .one {{ border-top:1px solid #2a251f; padding:18px 0; }}
 h2 {{ font-size:15px; margin:0 0 8px; }}
 h2 span {{ color:#8d857b; font-weight:400; font-size:13px; margin-left:10px; }}
 img {{ display:block; image-rendering:pixelated; border-radius:6px; max-width:100%; }}
</style>
<h1>{title}</h1>
<p class="lede">{lede}</p>
{blocks}
"""


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("--track", default="running-right")
    parser.add_argument("--speed", type=float, default=160, help="points per second")
    parser.add_argument("--cadence", action="append", type=float, default=[])
    parser.add_argument("--cycles", type=int, default=2)
    parser.add_argument("--scale", type=int, default=2)
    parser.add_argument("--width", type=float, default=440, help="scene width in points")
    parser.add_argument("-o", "--out", type=Path, required=True)
    parser.add_argument("--title", default="gait against ground")
    args = parser.parse_args()

    cadences = args.cadence or [1.0]
    manifest, extension = load_manifest(args.package)
    track = tracks_from(manifest, extension)[args.track]
    sheet = Image.open(args.package / manifest["spritesheetPath"]).convert("RGBA")
    ext_sheet, ext_columns = extension_sheet(args.package, extension)
    cells = [
        frame_image(sheet, index, manifest["frame"], ext_sheet, ext_columns)
        for index in track["frames"]
    ]
    fps = track.get("fps") or 12
    # Stride is conventionally quoted against the animal's resting length, and a
    # running frame's bounding box is the stretched pose -- 89pt here against 62
    # at rest, which would flatter the stride by a third. So the reference comes
    # from the pet's own resting row.
    resting = tracks_from(manifest, extension).get("idle") or track
    rest_cell = frame_image(
        sheet, resting["frames"][0], manifest["frame"], ext_sheet, ext_columns
    )
    box = rest_cell.getchannel("A").point(lambda v: 255 if v > 8 else 0).getbbox()
    body = (box[2] - box[0]) * POINTS_PER_CELL_PIXEL
    baseline_pt = (box[3] + 1) * POINTS_PER_CELL_PIXEL

    args.out.mkdir(parents=True, exist_ok=True)
    blocks = []
    for cadence in cadences:
        step_ms = max(20, round(1000 / (fps * cadence)))
        frames, durations, total = build(
            cells, args.speed, step_ms, args.scale, args.width, args.cycles, baseline_pt
        )
        cycle_seconds = len(cells) * step_ms / 1000
        stride = args.speed * cycle_seconds
        name = f"{args.track}-x{cadence:g}"
        save_gif(frames, durations, args.out / f"{name}.gif")
        note = (
            f"{cycle_seconds:.2f}s per cycle · {stride:.0f}pt per stride · "
            f"{stride / body:.2f} body lengths · {args.speed:g}pt/s"
        )
        print(f"{name:26s} {note}")
        blocks.append(
            f'<div class="one"><h2>cadence {cadence:g}×<span>{note}</span></h2>'
            f'<img src="{data_uri(args.out / f"{name}.gif")}"></div>'
        )

    page = args.out / "index.html"
    page.write_text(
        PAGE.format(
            title=args.title,
            lede=(
                f"{args.track}, drawn at the overlay's own size with the ground "
                f"scrolling at {args.speed:g}pt/s. Body length measured at "
                f"{body:.0f}pt. Ticks are fixed to the ground, so the question is "
                "whether the legs look like they are carrying the pet past them."
            ),
            blocks="\n".join(blocks),
        )
    )
    print(f"-> file://{page.resolve()}")


if __name__ == "__main__":
    main()
