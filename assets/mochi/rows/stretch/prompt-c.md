# Mochi v3 — `stretch`, eight frames (waking stretch)

Create one pixel-art sprite strip with **exactly eight complete full-body frames**
in a single horizontal row. Each frame occupies a 192 × 208 cell of a Codex
Petdex animation atlas.

A four-frame version of this row already works. This is the same motion drawn at
twice the resolution in time, so it plays smoothly instead of stepping. Every
frame is on screen for 212ms.

## Two things earlier attempts got wrong — keep these fixed

**1. Whiskers.** Draw **two whiskers on each side of the muzzle as thick solid
black lines**, the same near-black as the cat's outline — not cream, not beige,
not brown, not a tint of the cheek. Clearly visible against the fur at a glance.
Two per side, no more, no fewer, in **all eight frames**. An earlier attempt
returned a cat with no whiskers at all in its first frame, and another drew three
a side.

**2. The cat must not change size.** In earlier attempts the curled cat was 12
to 17% smaller than the same cat sitting at the end — it grew as it woke up. A
curled cat and a sitting cat cover about the same amount of the picture; the
curl is **wider and lower, not smaller**. Frame 1 must be as substantial as
`reference/sleeping-last.png` (159 wide, 104 tall) and frame 8 as substantial as
`reference/idle-first.png` (123 wide, 148 tall). Same animal, same bulk, eight
different shapes.

## Identity

`reference/idle-neutral.png` is the identity lock. It is Mochi: a compact calico
kitten with large brown eyes, asymmetrical orange, dark-brown and cream markings,
a dark outline, tiny paws, a short striped tail, small pink paw pads, pink inner
ears, **two thick black whiskers on each side of the muzzle**, a small pink
triangular nose, and a thin line mouth.

Preserve exactly: the cat's markings and their colours, ear shape, tail stripes,
palette, outline weight, and face proportions.

## What this is for

This row **joins two other rows** and only works if both ends match them.

- `reference/sleeping-last.png` is where it starts — curled on the ground, asleep.
- `reference/idle-first.png` is where it ends — sitting upright, awake.

The pet has been asleep and something woke it. It stretches, gets up, and sits.

## The eight frames

The motion has three beats — waking, the stretch, and getting up — and the extra
frames go **inside** them so nothing jumps.

1. curled on the ground exactly as in `sleeping-last.png`, but **awake**: eyes
   open, head just lifted off the paws. Nothing else has moved. Wide and low.
2. the head comes up and forward, front paws sliding out from under the chin.
   Still curled, still low. The body has begun to uncoil.
3. front legs pushed out along the ground, chest lowering, haunches beginning to
   rise. Halfway into the stretch. Eyes narrowing.
4. **the full stretch.** Front legs extended far forward, elbows down, chest
   pressed to the floor, back deeply curved, haunches raised high, tail up. The
   longest and lowest frame of the eight. **Eyes squeezed tightly shut**, the
   face showing the effort. Make this emphatic — longer and lower than looks
   reasonable. This is the frame people recognise as a cat stretching.
5. holding the stretch a moment longer but beginning to release: the back
   flattens slightly, the chest lifts off the floor, eyes still shut.
6. the front legs straighten and take the weight, chest well up, hind legs still
   trailing behind. Eyes opening.
7. standing squarely, hind legs gathered under the body, head up, eyes open.
   Taller and narrower than frame 6.
8. sitting upright, settled, **matching `idle-first.png`**: same seated posture,
   same width and height, front paws on the ground, tail resting beside the body,
   eyes open.

Across the eight the cat goes from wide and low to tall and narrow. It stays on
the same ground line throughout — paws and belly never leave the floor, and the
cat never hops or floats.

Consecutive frames must differ **only a little**. This is the point of drawing
eight instead of four: frame 3 is a small step from frame 2, not a new pose.

## Rules

Every frame is a **newly and completely drawn full-body Mochi**. Do not paste,
mirror, or reuse parts of another frame.

The eight frames sit at the same scale and on the same ground line.

The mouth stays closed. The cat has four legs and no more.

Background is flat solid `#00FF00` chroma key only. No shadows, floor, ground,
glow, gradients, texture, text, labels, visible grid, guide marks, hands, arms,
cursors, props, motion lines, sleep "Z" marks, sparkles, detached pixels, or
overlapping frames. Give the eight sprites clear separation and generous padding;
no body part may be cropped.
