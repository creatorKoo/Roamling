# Mochi v3 — `review` row (reading or searching)

Create one pixel-art sprite strip with **exactly six complete full-body frames**
in a single horizontal row. Each frame occupies a 192 × 208 cell of a Codex
Petdex animation atlas.

## Identity

`reference/idle-neutral.png` is the identity lock and the neutral pose. It is
Mochi: a compact calico kitten with large brown eyes, asymmetrical orange,
dark-brown and cream markings, a dark outline, tiny paws, a short striped tail,
small pink paw pads, pink inner ears, three whiskers on each side of the muzzle,
a small pink triangular nose, and a thin line mouth. `reference/idle-row.png`
shows the approved scale and proportion.

Preserve exactly: body scale, body centre, ground line, ear shape, marking
placement, palette, outline weight, and face proportions. The cat stays seated
and facing forward in every frame.

## Face details that were wrong last time

**Whiskers are one flat colour, all the way through.**

Each whisker is a thin stroke filled with a single dark near-black, edge to edge.
There is **no lighter colour inside it** and **no outline drawn around it**. The
whisker is not a small object with a fill and a border, the way the body is — it
is one solid dark mark.

Three attempts have got this wrong. The first drew them in pale cream so they
disappeared into the cheeks. The next two drew a brown fill with a darker outline
running around it, which turns each whisker into a twig glued to the face.
`reference/idle-neutral.png` has them right: flat, dark, uniform.

Three per side, in every frame, at the angle shown in the reference, as thin as
the drawing allows.

Copy the reference's face exactly: the eye shape and the position of the white
highlight inside each eye, the small pink triangular nose, and the thin closed
mouth beneath it. These do not change between frames.

Draw with clean, solid colour areas and a crisp dark outline. Do not leave loose
single pixels floating outside the outline; every part of the cat is one
connected shape.

## The animation — concentrating on something

A cat fixed on something holds its body still, flicks the end of its tail, and
tips its head a little. This plays whenever the agent reads a file, so it appears
often and must stay calm.

The tail rests on the right side of the picture and its **tip** sweeps once out
and back. Nothing else on the cat moves at all.

1. neutral — identical to `reference/idle-neutral.png`
2. tail tip starting to swing right
3. tail tip at its furthest right
4. tail tip starting back
5. tail tip nearly home
6. neutral again — identical to frame 1

**Size of the flick.** The tail tip travels about the width of the cat's own
head. It swings from around its midpoint; the base where it meets the body does
not move. A short flick, not a sweep — an earlier attempt reached so far that it
ran to the very edge of its cell with nothing to spare.

Frame 6 is held twice as long as the others, so it must read as a settled rest
pose.

## Only the tail moves

This is the whole discipline of this strip. In all six frames the head stays at
exactly the same height and angle, the ears stay upright and keep their shape,
the eyes stay open and look straight ahead at the viewer, the body does not
shrink or lean, and all four paws stay flat on the floor.

Do not lower or tilt the head, do not fold or flatten the ears, do not close the
eyes, do not compress the body, do not raise the gaze. An earlier attempt at this
row hunched the cat down and folded its ears back, which reads as a frightened
animal rather than an attentive one.

The cat's visible height must stay within two pixels of the reference across all
six frames.

## Rules

Every frame is a **newly and completely drawn full-body Mochi**. Do not paste,
mirror, or reuse parts of another frame; do not generate only a tail or a head
and composite it onto a body.

Whiskers are present in all six frames. The mouth stays closed — a thin line,
never open. Four paws on the floor in every frame; the cat has four legs.

Background is flat solid `#00FF00` chroma key only. No shadows, floor, glow,
gradients, texture, text, labels, visible grid, guide marks, props, motion lines,
sparkles, detached pixels, or overlapping frames. Give the six sprites clear
separation and generous padding; no body part may be cropped.
