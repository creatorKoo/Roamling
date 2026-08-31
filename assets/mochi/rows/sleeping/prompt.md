# Mochi v3 — `sleeping` (curled up asleep)

Create one pixel-art sprite strip with **exactly four complete full-body frames**
in a single horizontal row. Each frame occupies a 192 × 208 cell of a Codex
Petdex animation atlas.

## Identity

`reference/idle-neutral.png` is the identity lock. It is Mochi: a compact calico
kitten with large brown eyes, asymmetrical orange, dark-brown and cream markings,
a dark outline, tiny paws, a short striped tail, small pink paw pads, pink inner
ears, **two** whiskers on each side of the muzzle, a small pink triangular nose,
and a thin line mouth. `reference/idle-row.png` shows the approved scale.

Preserve exactly: the cat's size, marking placement and colours, ear shape,
tail stripes, palette, outline weight, and face proportions. This is the same
animal as the reference, lying down instead of sitting.

## What this is for

The pet has been left alone for over a minute and has gone to sleep. It stays
like this until something wakes it, so this loop may run for many minutes. It
must be almost perfectly still.

It also has to be **unmistakably different from sitting**. Right now the pet
borrows its seated waiting pose for sleep, so there is no way to tell a sleeping
cat from a waiting one — that is the whole reason this row is being drawn.

## The pose

A cat curled up on the ground, seen from the same front-facing three-quarter
angle as the reference.

- the body is **curled into a rounded shape, lying on the ground** — wider than
  it is tall, the opposite of the seated pose
- the head rests down against the body, not held up
- the eyes are closed, drawn as soft downward crescents
- all four paws are tucked in under the body; no leg stands or reaches
- the tail curls around the body, its striped tip near the head
- the ears lie relaxed, neither pricked up nor folded flat in fear

## The animation — breathing

Four frames of one slow breath, and nothing else.

1. resting
2. the body rises very slightly as the cat inhales
3. the top of the inhale
4. sinking back towards resting

The rise is **two or three pixels at most**, in the back and flank only. The
head, the paws, the tail and the point where the body meets the ground do not
move at all. Frame 4 leads back into frame 1 without a jump, because this loops.

Do not let the cat shift sideways, roll, wake, open its eyes, lift its head, or
change shape between frames.

## Rules

Every frame is a **newly and completely drawn full-body Mochi**. Do not paste,
mirror, or reuse parts of another frame.

Whiskers are present in all four frames, two per side, drawn as thin dark lines —
one flat colour, no outline around them. The mouth stays closed.

Background is flat solid `#00FF00` chroma key only. No shadows, floor, glow,
gradients, texture, text, labels, visible grid, guide marks, props, pillows,
blankets, sleep symbols, "Z" letters, motion lines, sparkles, detached pixels, or
overlapping frames. Give the four sprites clear separation and generous padding;
no body part may be cropped.
