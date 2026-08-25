<!-- SPDX-FileCopyrightText: 2026 GooBeom Jeoung -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Built-in mascot pose auditions

`mochi-poses.png` and `fat-mochi-poses.png` are the first visual audition
sheets for Roamling's built-in mascots. They were generated with OpenAI's
built-in image-generation tool on 2026-08-25, selected by the project owner,
and converted locally from a flat chroma-key background to PNG alpha. No
third-party pet artwork is included in these files.

The sheets intentionally contain only four key poses: idle, walking right,
sleeping, and caught. `MascotPetFactory` derives a temporary multi-step runtime
atlas from these poses, using reversible transform sequences instead of
inventing new artwork. This lets the mascots be evaluated at actual desktop
size before a hand-cleaned animation set is commissioned or authored.

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

The idle transform briefly settles the whole pose by less than one rendered
point. Its pixel resampling reads like a tiny blink at desktop size; the
runtime deliberately returns through intermediate frames so that effect and
the walking bob do not snap at their loop boundaries.

The working chroma-key prompts changed only the background to a flat solid
`#00FF00`; transparency was produced locally without network upload.

These two PNG files are distributed under GPL-3.0-only with this repository.
Roamling names and branding are additionally covered by `TRADEMARKS.md`.
