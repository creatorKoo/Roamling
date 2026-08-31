<!-- SPDX-FileCopyrightText: 2026 GooBeom Jeoung -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Built-in mascot assets

`mochi-poses.png` and `fat-mochi-poses.png` are the first visual audition
sheets for Roamling's built-in mascots. They were generated with OpenAI's
built-in image-generation tool on 2026-08-25, selected by the project owner,
and converted locally from a flat chroma-key background to PNG alpha. No
third-party pet artwork is included in these files.

The pose sheets contain four key poses: idle, walking, sleeping, and caught.
Both mascots now have authored runtime atlases; the pose sheets remain the
identity references and the fallback source if a bundled atlas cannot load.

`fat-mochi-runtime-atlas.png` is FatMochi's authored runtime asset. It has
8 columns of 192×208 cells and seven internal rows: idle,
running right, running left, sleeping, caught, stretching, and landing. Walk
frames articulate diagonal front/hind pairs and add restrained body/tail
follow-through; the other current-MVP states use blink, breathing, a compact
four-paw scramble, forward-stretch, and landing sequences. The stretch is the
feline play-bow shape—forepaws forward, chest low, hindquarters raised. An
upright, human-like arms-up draft was explicitly rejected and is not included.

This seven-row layout is an internal built-in asset, not a new Petdex package
version. Imported Petdex pets still use their v1/v2 manifest and atlas layouts
without modification.

`mochi-standard-atlas.webp` is Mochi's runtime asset and uses the standard
8×9 Codex/Petdex row set instead: idle, running right, running left, waving,
jumping, failed, waiting, running, review. It replaces an earlier seven-row Mochi
atlas that was rejected on 2026-08-27 for detached alpha fragments left by
slicing a generated strip that was not on an exact equal-cell grid.

Both webp files here are the shipped `mochi-v3` package, copied in on
2026-08-31 — the same bytes Codex and Petdex read from `~/.codex/pets/mochi-v3`.
Each row was reviewed and approved on its own before the next was drawn, and the
approval record with the reasoning for each is `output/v3/approvals.json`.

`mochi-extension-atlas.webp` is that package's `roamling.json` sheet: 8×3, whose
cells continue the frame index past the package grid, so cell 72 is index 72.
It draws what the nine-row contract has no word for — sleeping curled on the
ground (72–74), being carried (75–78), nodding off (80–83), and the waking
stretch (88–95). `gaze` is in the same manifest but draws nothing: it plays the
package's own review row faster the closer the pointer gets.

So Mochi borrows only two things now, both on purpose: `dragged` takes the caught
pose, and `landing` takes the jumping row. `sit` and `sleep` used to fall back to
`waiting`, which means "blocked on the user", and a drowsy pet looked like it was
asking for approval; they now have their own rows here and fall back to idle
elsewhere.

`stretching` is the row that joins two others — it leaves the curl `sleeping`
ends on and arrives at the seat `idle` begins from, so both ends had to match
their neighbours. `wake` and `stretch` are one capability and the player only
rewinds on a capability change, so its eight frames run straight through both
states rather than replaying.

The factory writes out two tracks the standard set gets wrong for this art. Idle
overrides the standard 1.10s: its six frames are one long hold and a blink, so
the standard timing blinks the cat continuously, and the package holds frame
zero for 1.2s instead. Landing is built from the jumping row so a drop does not
replay the celebration.

Core prompt, Mochi:

> Preserve candidate 3's compact calico identity, large bright eyes, alert
> ears, short legs, and striped tail. Show exactly four consistent pixel-art
> poses in one row: idle, walking right, curled sleeping, and caught/surprised.
> Use a limited palette, crisp silhouette, no props, labels, or extra animals.

Core prompt, FatMochi:

> Preserve candidate 1's very round mochi-shaped calico identity, sleepy eyes,
> tiny deadpan mouth, cheek blush, and short legs. Show exactly four consistent
> pixel-art poses in one row: idle, waddling right, bun-like sleeping, and
> caught/squished. Use a limited palette, crisp silhouette, no props, labels,
> or extra animals.

The final FatMochi idle revision keeps the solid-black pixel shape, uses a
shorter and slightly taller profile for readability at 96×104pt, and flattens
the inward angry slant so the expression reads as blankly sleepy and a little
dazed.

The authored idle preserves that exact approved open-eye frame and adds local
half-closed/closed eye pixels for the blink. The generated open-eye variants
were not used because they drifted from the approved eye shape. The first and
last forward-stretch frames likewise use the approved idle artwork so the
transition returns to the same character cleanly.

Additional image-generation prompts preserved FatMochi's approved identity and
requested: an eight-frame four-paw alternating walk with a fixed ground line;
a four-frame sleeping breath; a four-frame restrained caught wiggle; a
six-frame feline forward stretch; and a five-frame drop/squash/recovery. Frame
normalization and chroma removal were performed locally with nearest-neighbor
sampling. The latest identity pass used local idle, walk, and caught atlas
extracts as explicit image-to-image references. Final chroma removal, exact
face restoration, sizing, centering, and green-spill cleanup were local steps.

The latest identity pass treats the approved idle pose as the source of truth.
Every walk frame uses idle's 174×170 alpha silhouette and retains the exact
approved face pixels. Frame 16 is the exact idle artwork; the selected frames
then alternate short planted and lifted paws before returning to a stable
contact pose. The cheeks narrow into a readable neck/shoulder transition in
every frame instead of merging into the former pear-shaped torso. Alpha-bounds
center varies by less than two source pixels, preventing the old leftward drift
and snap-back that read as a moonwalk. The walk prompt explicitly specifies the
neck invariant, planted-paw travel, lifted-paw recovery, chronological opposing
contacts, a fixed torso, and a seamless transition back to idle.

Frames 32–35 keep the prior 174×170 one-shot caught transition beginning with
the exact approved idle frame. Frames 36–39 restore the earlier belly-facing
caught silhouette while replacing only its long limbs with compact paws. The
four frames stay within 180×183 alpha bounds and alternate front/hind diagonal
pairs close to the belly. A short click plays the same caught-to-scramble
response once while remaining click-through; a held drag repeats the loop. The
approved idle/blink row and caught intro were preserved pixel-for-pixel during
this pass.

The 2026-08-26 gait pass used the approved local idle/walk references with
OpenAI's built-in image-generation mode. The prompt requested an eight-frame
four-beat walk in which every paw changes position and a compact four-frame
scramble constrained below the belly. Generated faces and torsos were rejected;
only selected lower-body motion was chroma-keyed and combined locally with the
approved FatMochi body, using nearest-neighbor sampling. A subsequent repair
pass replaced the cut lower-body splice with a connected lower-belly drawing,
restored the belly-facing drag pose, and removed three detached debris components
from existing stretch/landing frames without changing those poses. Tests now
require one connected visible-alpha component in every built-in FatMochi frame.

The working chroma-key prompts changed only the background to a flat solid
`#00FF00`; transparency was produced locally without network upload.

These built-in PNG files are distributed under GPL-3.0-only with this repository.
Roamling names and branding are additionally covered by `TRADEMARKS.md`.
