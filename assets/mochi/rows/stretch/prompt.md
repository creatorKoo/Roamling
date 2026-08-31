# Mochi v3 — `stretch` (waking up, the stretch between sleeping and sitting)

Create one pixel-art sprite strip with **exactly four complete full-body frames**
in a single horizontal row. Each frame occupies a 192 × 208 cell of a Codex
Petdex animation atlas.

## Identity

`reference/idle-neutral.png` is the identity lock. It is Mochi: a compact calico
kitten with large brown eyes, asymmetrical orange, dark-brown and cream markings,
a dark outline, tiny paws, a short striped tail, small pink paw pads, pink inner
ears, **two** whiskers on each side of the muzzle, a small pink triangular nose,
and a thin line mouth.

Preserve exactly: the cat's markings and their colours, ear shape, tail stripes,
palette, outline weight, and face proportions. This is the same animal.

## What this is for

This is the one row that **joins two other rows**, and it only works if both ends
match them.

- `reference/sleeping-last.png` is where it starts. The cat is curled on the
  ground, asleep, 159 wide and 104 tall.
- `reference/idle-first.png` is where it ends. The cat sits upright, awake,
  123 wide and 148 tall.

The pet has been asleep and something woke it. It stretches, gets up, and sits.
Then the idle row takes over. Plays once, about 1.7 seconds, so each frame is on
screen for a long 425ms and has to read as a still.

**Frame 1 must continue from the curled sleeping pose and frame 4 must land on
the seated idle pose.** If frame 1 shows a cat already sitting up there is no
stretch, and if frame 4 does not match the seated pose the pet will snap when
the idle row starts.

## The pose sequence

1. still curled on the ground exactly as in `sleeping-last.png`, but **awake** —
   the eyes are open and the head has lifted slightly off the paws. Nothing else
   has moved yet. Roughly as wide and as low as the sleeping frame.
2. **the stretch.** Front legs pushed out forward along the ground, elbows down,
   chest low, haunches raised behind — the long low bow a cat makes on waking.
   This is the widest frame of the four and still one of the lowest. The head is
   low and forward, eyes half closed with the effort.
3. rising. The front legs have straightened and taken the weight, the chest has
   come up, the hind legs are gathering under the body. Taller than frame 2 and
   narrower. The head is up and the eyes are open.
4. sitting upright, settled, **matching `idle-first.png`**: same seated posture,
   same width and height, front paws on the ground, tail resting beside the body,
   eyes open. This is the pose the idle row will continue from.

The cat gets taller and narrower across the four frames, from about 104 tall to
about 148 tall. It stays on the same ground line the whole time — the paws and
belly never leave the floor, and the cat never hops or floats.

Do not draw a cat standing on all fours mid-walk. Every frame is either on the
ground or rising from it.

## Rules

Every frame is a **newly and completely drawn full-body Mochi**. Do not paste,
mirror, or reuse parts of another frame.

The four frames sit at the same scale and on the same ground line. The cat's
size must not change except for the pose itself — it is the same kitten from a
curl to a sit, not a kitten that grows.

Whiskers are present in all four frames, two per side, drawn as thin dark lines —
one flat colour, no outline around them. The mouth stays closed. The cat has four
legs and no more.

Background is flat solid `#00FF00` chroma key only. No shadows, floor, ground,
glow, gradients, texture, text, labels, visible grid, guide marks, hands, arms,
cursors, props, motion lines, sleep "Z" marks, sparkles, detached pixels, or
overlapping frames. Give the four sprites clear separation and generous padding;
no body part may be cropped.
