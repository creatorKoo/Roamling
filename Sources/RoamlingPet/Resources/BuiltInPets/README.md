<!-- SPDX-FileCopyrightText: 2026 GooBeom Jeoung -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Built-in mascot assets

`mochi-poses.png` and `fat-mochi-poses.png` are the first visual audition
sheets for Roamling's built-in mascots. They were generated with OpenAI's
built-in image-generation tool on 2026-08-25, selected by the project owner,
and converted locally from a flat chroma-key background to PNG alpha. No
third-party pet artwork is included in these files.

The pose sheets contain four key poses: idle, walking, sleeping, and caught.
Mochi still uses a reversible transform sequence derived from those poses for
evaluation at actual desktop size.

`fat-mochi-runtime-atlas.png` is the authored runtime asset for the default
FatMochi. It has 8 columns of 192×208 cells and seven internal rows: idle,
running right, running left, sleeping, caught, stretching, and landing. Walk
frames articulate all four paws and add restrained body/tail follow-through;
the other current-MVP states use blink, breathing, paw-wiggle, forward-stretch,
and landing sequences. The stretch is the feline play-bow shape—forepaws
forward, chest low, hindquarters raised. An upright, human-like arms-up draft
was explicitly rejected and is not included.

This seven-row layout is an internal built-in asset, not a new Petdex package
version. Imported Petdex pets still use their v1/v2 manifest and atlas layouts
without modification.

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
Walk frames use a 176×168 alpha silhouette beside idle's 174×170 silhouette,
retain the exact approved face pixels, and move the short paws beneath a stable
round torso. Their alpha centroid varies by less than two source pixels across
the cycle, preventing the old leftward drift and snap-back that read as a
moonwalk. The walk prompt explicitly specifies planted-paw travel, lifted-paw
recovery, chronological opposing contacts, a fixed torso, and a seamless
frame-eight-to-frame-one transition.

The caught/drag pass follows the same size rule at 174×170. Frames 32–35 form a
one-shot transition beginning with the exact approved idle frame; frames 36–39
hold that compact pose as a seamless paw/ear/tail wiggle while dragging. This
keeps click and drag responsive without making FatMochi change into a larger,
more upright cat. The approved idle/blink row was preserved pixel-for-pixel
during this pass.

The working chroma-key prompts changed only the background to a flat solid
`#00FF00`; transparency was produced locally without network upload.

These built-in PNG files are distributed under GPL-3.0-only with this repository.
Roamling names and branding are additionally covered by `TRADEMARKS.md`.
