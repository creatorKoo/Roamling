# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
"""Package the BoriSsal artwork for Windows and macOS from one source."""
from pathlib import Path
from PIL import Image

root = Path(__file__).resolve().parents[1]
source = Image.open(root / 'assets/icon/borissal.png').convert('RGBA')
assert source.getchannel('A').getextrema()[0] == 0, 'Icon must have real transparency'
ink = source.crop(source.getchannel('A').getbbox())
side = max(ink.size)
margin = round(side * .055)
square = Image.new('RGBA', (side + margin * 2, side + margin * 2))
square.paste(ink, ((square.width - ink.width)//2, (square.height - ink.height)//2))
master = square.resize((1024, 1024), Image.Resampling.LANCZOS)
master.save(root/'assets/Roamling.icns', format='ICNS')
master.save(root/'assets/Roamling.ico', format='ICO', sizes=[(s,s) for s in (16,20,24,32,40,48,64,128,256)])
master.resize((64,64), Image.Resampling.LANCZOS).save(root/'assets/icon/borissal-menu.png')
ico = Image.open(root/'assets/Roamling.ico')
assert {(16,16),(24,24),(32,32),(48,48),(256,256)} <= ico.ico.sizes()
assert Image.open(root/'assets/Roamling.icns').size == (1024,1024)
print('BoriSsal ICO/ICNS ready; shared artwork, transparent edges, nine Windows sizes')
