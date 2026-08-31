# Mochi v3 — `waving` row (turn complete)

Create one pixel-art sprite strip with **exactly four complete full-body frames**
in a single horizontal row. Each frame occupies a 192 × 208 cell of a Codex
Petdex animation atlas.

## Identity

`reference/idle-neutral.png` is the identity lock and the neutral pose. It is
Mochi: a compact calico kitten with large brown eyes, asymmetrical orange,
dark-brown and cream markings, a dark outline, tiny paws, a short striped tail,
and small pink paw pads. `reference/idle-row.png` shows the approved idle row for
scale and proportion.

Preserve exactly: body scale, body centre, ground line, ear shape, tail length
and stripes, marking placement on both sides, palette, outline weight, face
proportions, and pixel-art style. The cat stays seated and facing forward in
every frame.

## The animation — a two-paw cheer, "done!"

Both front paws lift together and come back down. Symmetry matters: the previous
version raised one paw and the body drifted sideways with it.

1. seated neutral, both front paws on the ground, eyes open — identical
   silhouette to `reference/idle-neutral.png`
2. both front paws lifting together, elbows still low, eyes widening
3. **peak** — both front paws raised high and level with each other, eyes closed
   in happy crescents, ears up
4. back to seated neutral, paws down, eyes open — the same silhouette as frame 1

Frame 4 is held twice as long as the others, so it must read as a settled rest
pose, not a mid-motion pose.

## The cat has four legs

This is the failure to avoid. In frames 2 and 3 the front paws leave the ground,
so **only the two hind paws touch the floor**. The front paws must not appear
both raised and planted in the same frame. Count them: four legs, never more.

Frames 1 and 4 are the seated neutral -- two front paws down between the hind
paws, four paws on the floor, exactly as in `reference/idle-neutral.png`.

## Keep these, they were lost last time

- **Whiskers.** Three on each side of the muzzle, at the angle shown in the
  reference. Present in every frame.
- **The face.** A small pink triangular nose and a thin line mouth. **The mouth
  never opens**, not even at the peak; the happy expression comes from the eyes
  closing into crescents.
- **Pink inner ears.**
- **Body width.** The seated body is about 0.83 as wide as it is tall. Last time
  it came back too slim at 0.79.
- **Body markings at the peak.** The orange and dark-brown patches on the chest,
  haunches and tail stay exactly where they are in the neutral frames. Do not
  flatten the body to plain cream when the paws go up, and do not add a band
  across the chest that the other frames do not have.

## Rules

Every frame is a **newly and completely drawn full-body Mochi**. Do not paste,
mirror, or reuse parts of another frame; do not generate only the paws and
composite them onto a body.

Across the four frames these stay fixed: canvas, character scale, ground line,
and horizontal body centre. Only the front paws, eyes, and ears change. The hind
legs, tail, haunches and head position do not move. The cat does not stand up,
lean, hop, or shift left or right.

Background is flat solid `#00FF00` chroma key only. No shadows, floor, glow,
gradients, texture, text, labels, visible grid, guide marks, props, motion lines,
sparkles, detached pixels, or overlapping frames. Give the four sprites clear
separation and generous padding; no body part may be cropped.
