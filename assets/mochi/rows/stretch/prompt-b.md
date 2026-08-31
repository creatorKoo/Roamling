# Mochi v3 — `stretch`, second attempt (waking stretch)

Create one pixel-art sprite strip with **exactly four complete full-body frames**
in a single horizontal row. Each frame occupies a 192 × 208 cell of a Codex
Petdex animation atlas.

## Two things the last attempt got wrong — read these first

**1. The whiskers went missing.** Three strips came back with the whiskers drawn
as pale cream marks the same colour as the cheek, or not drawn at all. One strip
had a cat with no whiskers whatsoever in its first frame.

Every frame must show **two whiskers on each side of the muzzle, drawn as thick
solid black lines** — the same near-black as the cat's outline, not a tint of the
cheek, not cream, not beige, not brown. They must be clearly visible against the
cream fur at a glance. Draw them boldly: a whisker that is subtle at this size
disappears. Two per side, no more, no fewer, on all four frames.

**2. The cat changed size.** In the last attempt the curled cat in frame 1 was
17% smaller than the same cat sitting in frame 4 — it grew as it woke up.

The cat is one animal at one size throughout. A curled cat and a sitting cat
cover about the same amount of the picture; the curl is wider and lower, not
smaller. Frame 1 must be as substantial as `reference/sleeping-last.png`, which
is 159 wide and 104 tall, and frame 4 as substantial as
`reference/idle-first.png`, which is 123 wide and 148 tall. Same cat, same bulk,
different shape.

## Identity

`reference/idle-neutral.png` is the identity lock. It is Mochi: a compact calico
kitten with large brown eyes, asymmetrical orange, dark-brown and cream markings,
a dark outline, tiny paws, a short striped tail, small pink paw pads, pink inner
ears, **two thick black whiskers on each side of the muzzle**, a small pink
triangular nose, and a thin line mouth.

Preserve exactly: the cat's markings and their colours, ear shape, tail stripes,
palette, outline weight, and face proportions.

## What this is for

This is the one row that **joins two other rows**, and it only works if both ends
match them.

- `reference/sleeping-last.png` is where it starts — curled on the ground, asleep.
- `reference/idle-first.png` is where it ends — sitting upright, awake.

The pet has been asleep and something woke it. It stretches, gets up, and sits.
Plays once, about 1.7 seconds, so each frame is on screen for a long 425ms and
has to read as a still.

## The pose sequence

1. still curled on the ground as in `sleeping-last.png`, but **awake** — eyes
   open, head just lifted off the paws. Nothing else has moved. Wide and low.
2. **the stretch, and this is the frame that matters most.** A long luxurious
   full-body stretch: front legs pushed far out forward along the ground, elbows
   down, chest low and pressed to the floor, back curved, haunches raised high
   behind. The body is drawn out long — this is the widest frame of the four.
   **The eyes are squeezed tightly shut** and the face shows the effort of it.
   This is the pose people recognise as a cat stretching, so make it emphatic:
   longer, lower, and more extended than looks reasonable.
3. rising. The front legs have straightened and taken the weight, the chest has
   come up, the hind legs are gathering under the body. Taller than frame 2 and
   narrower. Head up, eyes open.
4. sitting upright, settled, **matching `idle-first.png`**: same seated posture,
   same width and height, front paws on the ground, tail resting beside the body,
   eyes open.

The cat gets taller and narrower across the four frames. It stays on the same
ground line the whole time — the paws and belly never leave the floor, and the
cat never hops or floats.

## Rules

Every frame is a **newly and completely drawn full-body Mochi**. Do not paste,
mirror, or reuse parts of another frame.

The four frames sit at the same scale and on the same ground line.

The mouth stays closed. The cat has four legs and no more.

Background is flat solid `#00FF00` chroma key only. No shadows, floor, ground,
glow, gradients, texture, text, labels, visible grid, guide marks, hands, arms,
cursors, props, motion lines, sleep "Z" marks, sparkles, detached pixels, or
overlapping frames. Give the four sprites clear separation and generous padding;
no body part may be cropped.
