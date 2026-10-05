<!-- SPDX-FileCopyrightText: 2026 GooBeom Jeoung -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Built-in pets: Bori and Ssal

The product ships Bori (`mochi-*`) and Ssal (`ssal-*`). FatMochi's pose sheet,
runtime atlas, menu and factory were removed on 2026-10-05. macOS migrates an old
`fat-mochi` built-in selection to `ssal`; external pet selections stay intact.
The earlier artwork provenance is preserved in `docs/history/built-in-assets-before-ssal.md`.

Ssal uses the approved neutral standard/extension WebP sheets and the accompanying
`ssal-pet.json` / `ssal-roamling.json`. Windows embeds the four resources; macOS
reads them through `petResourceBundle` and the same manifest validation rules as
external pets. Its nine coat/iris variants are generated from these two sheets
by the shared Rust `ssal::Source`, with no per-colour images. See `docs/ssal-palette.md`.

Bori's approved sheets and animation timing remain unchanged. `mochi-poses.png`
is the original approved identity/fallback sheet; current animation uses
`mochi-standard-atlas.webp` (8x9) plus `mochi-extension-atlas.webp` (8x3), with
192x208 cells. The palette engine always works from the neutral sheets before
premultiplication. No character image was regenerated for the macOS connection.

Artwork is distributed under GPL-3.0-only with this repository. Names and branding
are additionally covered by `TRADEMARKS.md`.
