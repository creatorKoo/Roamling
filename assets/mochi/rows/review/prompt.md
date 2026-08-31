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
shows the approved scale and proportion: the seated body is about 0.83 as wide
as it is tall.

Preserve exactly: body scale, body centre, ground line, ear shape, tail length
and stripes, marking placement on both sides, palette, outline weight, and face
proportions. The cat stays seated and facing forward in every frame.

## The animation — looking down at something, reading it

The head lowers to look at the floor in front of the cat and comes back up. This
is the "reading" pose.

1. seated neutral, head level, eyes forward — the same silhouette as
   `reference/idle-neutral.png`
2. the head begins to lower; the ears start rotating forward
3. head down, muzzle angled toward the floor, ears clearly forward, gaze down
4. still down, the eyes shifted to one side as if scanning across
5. the head coming back up, ears returning
6. seated neutral again — the same silhouette as frame 1

Depth of the bow: the chin comes close to the chest but does not touch it. The
ears do most of the work — turning them forward changes the top of the
silhouette, so the head does not have to drop far to read clearly.

Frame 6 is held twice as long as the others, so it must read as a settled rest
pose, not a mid-motion pose.

## The cat looks DOWN, never up

`reference/waiting-row-do-not-copy.png` is a different animation in which Mochi
looks **up** and tilts its head. That one means "waiting for the user". This one
must be unmistakably the opposite. Do not raise the muzzle, do not tilt the head
sideways, and do not let the gaze go above level in any frame.

`reference/review-current.png` is the row being replaced, in which the head turns
left and right. Do not do that either — the head does not rotate sideways here.

## Four legs, all four on the floor

The cat stays seated the whole time: two front paws down between the two hind
paws, **four paws on the floor in every one of the six frames**. Nothing lifts.
Count them; a previous strip for another row came back with six legs.

## What must not change between frames

Canvas, character scale, ground line, horizontal body centre, body width, the
hind legs, the haunches, the tail, and every marking. **Only the head angle, the
ears, and the eyes change.** The cat does not lean toward the viewer, does not
grow or shrink, does not stand, and does not shift left or right.

Whiskers are present in all six frames. The mouth stays closed — a thin line,
never open.

## Rules

Every frame is a **newly and completely drawn full-body Mochi**. Do not paste,
mirror, or reuse parts of another frame; do not generate only a head and
composite it onto a body.

Background is flat solid `#00FF00` chroma key only. No shadows, floor, glow,
gradients, texture, text, labels, visible grid, guide marks, props, books,
screens, motion lines, sparkles, detached pixels, or overlapping frames. Give the
six sprites clear separation and generous padding; no body part may be cropped.
