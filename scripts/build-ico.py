# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
#
# Draws assets/Roamling.ico out of assets/Roamling.icns, so the Windows app
# icon is the same mark as the macOS one -- not a lookalike, the same pixels.
#
#   uv run --with pillow python scripts/build-ico.py      (Windows)
#   ./scripts/pyimg.sh scripts/build-ico.py               (macOS)
#
# Run it after scripts/build-icon.sh, which is what produces the .icns. Both
# results are committed, for the reason build-icon.sh gives: an app icon has to
# be a file, and a build should not depend on which emoji font the machine
# happens to have.
#
# **Every size stored here was drawn at that size.** The .icns holds one image
# per size for the reason CLAUDE.md gives -- a 1024 paw resampled to 16 is mush
# where a drawn one is still a paw -- and throwing that away by resampling from
# the largest would undo it. So this only ever copies; it never scales. Windows
# asks for 48 and there is no authored 48, so none is stored and the shell
# scales 64 itself, which is the same arithmetic done one layer down.

import io
import os
import struct
import sys

from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SOURCE = os.path.join(ROOT, "assets", "Roamling.icns")
TARGET = os.path.join(ROOT, "assets", "Roamling.ico")

# Every authored size the .icns carries below 512. Above that is more than any
# Windows surface asks for and would only make the executable bigger.
WANTED = (16, 32, 64, 128, 256)


def unpack_bits(packed, expected):
    """The run-length coding icns uses for its pre-PNG entries.

    A lead byte under 0x80 means "the next n + 1 bytes are literal"; at or above
    it means "the next byte, repeated n - 125 times". Not the same arithmetic as
    PackBits proper, which is why this is written out rather than borrowed.
    """
    out = bytearray()
    at = 0
    while at < len(packed) and len(out) < expected:
        lead = packed[at]
        at += 1
        if lead < 0x80:
            count = lead + 1
            out += packed[at : at + count]
            at += count
        else:
            count = lead - 125
            out += bytes([packed[at]]) * count
            at += 1
    return bytes(out[:expected])


def read_icns(path):
    """Every image in the file, by pixel size."""
    data = open(path, "rb").read()
    if data[:4] != b"icns":
        raise SystemExit(f"{path} is not an icns file")

    # The 16 and 32 pixel entries predate PNG in this format and are stored as
    # four run-length coded channels instead.
    argb_sizes = {b"ic04": 16, b"ic05": 32}
    images = {}
    at = 8
    while at < len(data) - 8:
        kind = data[at : at + 4]
        length = struct.unpack(">I", data[at + 4 : at + 8])[0]
        if length < 8:
            break
        payload = data[at + 8 : at + length]
        at += length

        if payload[:8] == b"\x89PNG\r\n\x1a\n":
            image = Image.open(io.BytesIO(payload)).convert("RGBA")
            images.setdefault(image.size[0], image)
        elif payload[:4] == b"ARGB" and kind in argb_sizes:
            side = argb_sizes[kind]
            pixels = side * side
            planes = unpack_bits(payload[4:], pixels * 4)
            if len(planes) < pixels * 4:
                continue
            a, r, g, b = (planes[i * pixels : (i + 1) * pixels] for i in range(4))
            image = Image.merge(
                "RGBA",
                [Image.frombytes("L", (side, side), channel) for channel in (r, g, b, a)],
            )
            images.setdefault(side, image)
    return images


def bmp_entry(image):
    """One icon image in the BMP form, which every Windows surface can read.

    PNG inside an .ico is understood from Vista onwards, but not by every place
    an icon is asked for -- an installer's own window among them. The 256 is
    left as PNG because that entry is a late addition anyway and the size is
    worth saving; the rest are written out.
    """
    width, height = image.size
    pixels = image.load()

    header = struct.pack(
        "<IiiHHIIiiII",
        40,           # header size
        width,
        height * 2,   # colour rows plus the mask rows, as the format wants
        1,            # planes
        32,           # bits per pixel
        0,            # BI_RGB
        0,            # image size, may be zero when uncompressed
        0, 0, 0, 0,   # resolution and palette counts
    )

    colour = bytearray()
    for y in range(height - 1, -1, -1):  # bottom-up
        for x in range(width):
            r, g, b, a = pixels[x, y]
            colour += bytes((b, g, r, a))

    # A 1-bit mask is still required even though the alpha above carries the
    # shape. All zero means "every pixel is the image", which is what alpha
    # already said; rows pad to four bytes.
    stride = ((width + 31) // 32) * 4
    mask = bytes(stride * height)
    return header + bytes(colour) + mask


def main():
    images = read_icns(SOURCE)
    chosen = [(size, images[size]) for size in WANTED if size in images]
    missing = [size for size in WANTED if size not in images]
    if missing:
        print(f"note: {SOURCE} has no authored {missing}; skipping those sizes")
    if not chosen:
        raise SystemExit("no usable images in the icns")

    payloads = []
    for size, image in chosen:
        if size >= 256:
            buffer = io.BytesIO()
            image.save(buffer, format="PNG", optimize=True)
            payloads.append(buffer.getvalue())
        else:
            payloads.append(bmp_entry(image))

    offset = 6 + 16 * len(chosen)
    directory = bytearray(struct.pack("<HHH", 0, 1, len(chosen)))
    for (size, _), payload in zip(chosen, payloads):
        directory += struct.pack(
            "<BBBBHHII",
            0 if size >= 256 else size,   # 256 is stored as zero
            0 if size >= 256 else size,
            0,                            # not a palette
            0,                            # reserved
            1,                            # planes
            32,                           # bits per pixel
            len(payload),
            offset,
        )
        offset += len(payload)

    with open(TARGET, "wb") as out:
        out.write(directory)
        for payload in payloads:
            out.write(payload)

    sizes = ", ".join(str(size) for size, _ in chosen)
    print(f"wrote {TARGET} ({os.path.getsize(TARGET)} bytes) with {sizes}")


if __name__ == "__main__":
    sys.exit(main())
