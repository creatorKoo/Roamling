# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Writes the .DS_Store that arranges the disk image window.

Finder keeps a folder's window size, icon positions and background picture in
.DS_Store, and the usual way to produce one is to script Finder into opening
the volume and moving things by hand. That needs a desktop session and
Automation permission, which a release runner has neither of -- measured: it
answers -1712, an AppleEvent timeout, and leaves a stub behind.

So it is written directly, the way dmgbuild writes it. The result is committed
next to the background picture and copied into the image at build time, so a
release needs neither Finder nor this script.

The background is referenced by an alias, and an alias records the path it was
made from. That is why the volume is named `Roamling` with no version in it:
the name is part of the path, and a name that changed every release would point
the alias at a volume that does not exist.
"""

import sys
from ds_store import DSStore
from mac_alias import Alias

VOLUME = "/Volumes/Roamling"
# Kept in step with assets/dmg/render-dmg-background.swift, which draws the
# arrow between these two points. The lists have to agree or it points at
# nothing.
# left, top, right, bottom of the *frame*, not the content. Finder takes the
# title bar out of this -- 27 points -- and then the path bar too if the reader
# has it on, which is a setting a disk image cannot reach. Asking for 400 left
# 341 to draw in and cut the captions off the bottom, so this asks for the
# content it wants plus the title bar, and the picture keeps its last 60 points
# empty for the path bar to eat.
WINDOW = (200, 120, 840, 548)   # 640 x 428 of frame -> 400 of content, bare
ICON_SIZE = 112
POSITIONS = {"Roamling.app": (160, 165), "Applications": (480, 165)}
BACKGROUND = ".background/background.png"


def main(output):
    alias = Alias.for_file(f"{VOLUME}/{BACKGROUND}")
    left, top, right, bottom = WINDOW
    with DSStore.open(output, "w+") as store:
        store["."]["bwsp"] = {
            "WindowBounds": f"{{{{{left}, {top}}}, {{{right - left}, {bottom - top}}}}}",
            "ShowSidebar": False,
            "ShowToolbar": False,
            "ShowStatusBar": False,
            "ShowPathbar": False,
            "SidebarWidth": 0,
        }
        store["."]["icvp"] = {
            "viewOptionsVersion": 1,
            "arrangeBy": "none",
            "iconSize": float(ICON_SIZE),
            "gridSpacing": 100.0,
            "textSize": 12.0,
            "labelOnBottom": True,
            "showItemInfo": False,
            "showIconPreview": False,
            # 2 means "a picture", and the picture is named by the alias.
            "backgroundType": 2,
            "backgroundColorRed": 1.0,
            "backgroundColorGreen": 1.0,
            "backgroundColorBlue": 1.0,
            "backgroundImageAlias": alias.to_bytes(),
        }
        # Icon view, not whatever the reader's Finder last used elsewhere.
        store["."]["vSrn"] = ("long", 1)
        store["."]["ICVO"] = ("bool", True)
        for name, (x, y) in POSITIONS.items():
            store[name]["Iloc"] = (x, y)
    print(f"wrote {output}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".DS_Store")
