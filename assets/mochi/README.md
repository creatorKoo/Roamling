# Mochi — source material

What produced the pet, kept because it cannot be regenerated. The pet itself is
not here: the two sheets ship as
`Sources/RoamlingPet/Resources/BuiltInPets/mochi-standard-atlas.webp` (8×9) and
`mochi-extension-atlas.webp` (8×2), and are the same bytes Codex and Petdex read
from `~/.codex/pets/mochi-v3`.

```text
pet.json  roamling.json   the package contract the two sheets belong to
approvals.json            what was approved, and the reasoning for each row
rows/<row>/prompt.md      the wording that produced the approved art
rows/<row>/generated/     the strips it came back as, before any processing
bcuts/<name>/             rejected directions, with why each failed
```

`output/` is git-ignored and holds several hundred megabytes of intermediates —
extracted cells, GIFs, filmstrips, comparison pages. None of it is backed up
here because every one of those is a deterministic function of a strip in this
folder plus a script in `scripts/`. Re-derive a row with:

```sh
./scripts/pyimg.sh scripts/import_strip.py assets/mochi/rows/<row>/generated/<file>.webp \
    --palette Sources/RoamlingPet/Resources/BuiltInPets/mochi-standard-atlas.webp \
    --out output/v3/rows/<row>/cells --frames 4 --height 148 --scale-frame 0
```

The strips are lossless WebP, converted from the PNGs the generator returned and
verified pixel-identical to them. Lossless matters: these are the input to
chroma keying and palette snapping, and a lossy round trip would put thousands
of near-identical greens where the keyer expects one.

## Why the b-cuts are kept

A rejected candidate that leaves no trace gets proposed again. `bcuts/` records
what was tried and what was wrong with it — the greeting-style review row that
read as a bow rather than as reading, and four attempts at a dedicated `gaze`
row that each grew the cat instead of making it alert. Read those before
suggesting either again.

## Not finished

`stretch` is pending. `rows/stretch/` holds its prompt and the candidates, but
no row has been approved, so `approvals.json` has no entry for it and the
extension sheet's cells 84–87 are still empty. Waking still plays the idle row
for 1.7 seconds.
