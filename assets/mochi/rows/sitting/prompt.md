# Mochi v3 — `sitting` (getting drowsy, about to go to sleep)

Create one pixel-art sprite strip with **exactly four complete full-body frames**
in a single horizontal row. Each frame occupies a 192 × 208 cell of a Codex
Petdex animation atlas.

## Identity

`reference/idle-neutral.png` is the identity lock. It is Mochi: a compact calico
kitten with large brown eyes, asymmetrical orange, dark-brown and cream markings,
a dark outline, tiny paws, a short striped tail, small pink paw pads, pink inner
ears, **two** whiskers on each side of the muzzle, a small pink triangular nose,
and a thin line mouth.

Preserve exactly: the cat's size, marking placement and colours, ear shape, tail
stripes, palette, outline weight, and face proportions. This is the same animal,
sitting in the same spot, getting sleepy.

## What this is for

The user has not touched the computer for a while. The pet sits down and starts
nodding off, and 2.4 seconds later it stands up and walks away to find a place to
sleep. This plays once, from awake to nearly asleep.

It must not read as **waiting for something**. There is a separate row where the
cat looks up expectantly because it needs the user's approval, and this is the
opposite of that — nothing is being asked for, the cat is just running out of
energy. So the head goes **down**, never up, and the eyes close rather than
widening.

It must also not read as **asleep**. There is a separate row for that, where the
cat is curled up on the ground with its eyes shut. Here the cat is still sitting
upright the whole time.

## The pose

The seated pose from the reference, unchanged: sitting squarely on its haunches,
front paws on the ground, tail resting beside it. Same size, same position, same
distance from the ground.

The body does not move at all across the four frames. Everything that changes is
the head and the eyelids.

## The animation — a nod

Four frames of a single slow nod. Each frame is held for 0.6 seconds, which is
slow, so each pose has to be readable on its own as a still.

1. sitting upright, head level, **eyes half closed** — the lids come down from
   the top, covering the upper third of each eye. Awake but heavy.
2. head tipped down a little, **eyes nearly shut** — just a dark slit left
3. **the nod** — the head drops forward and down, chin toward the chest, eyes
   fully closed as two curved lines. This is the lowest point.
4. the head comes back up part of the way, **eyes opening slightly** to a slit
   again — caught itself. Not all the way back to frame 1; it is still sleepy.

The head movement is small: the chin drops about eight pixels at the lowest
point, not more. This is a kitten dozing off, not a bow and not a headbang.

The ears follow the head — they tilt back and down slightly as the head drops in
frame 3, and lift again in frame 4. They never fold flat.

The tail stays completely still in all four frames. Nothing swings.

## Rules

Every frame is a **newly and completely drawn full-body Mochi**. Do not paste,
mirror, or reuse parts of another frame.

The four frames sit at the same scale, on the same ground line, with the body
centred the same way. The cat must not drift sideways or change size between
frames — the only thing moving is the head.

Whiskers are present in all four frames, two per side, drawn as thin dark lines —
one flat colour, no outline around them. The mouth stays closed. The cat has four
legs and no more.

Background is flat solid `#00FF00` chroma key only. No shadows, floor, ground,
glow, gradients, texture, text, labels, visible grid, guide marks, hands, arms,
cursors, props, motion lines, sleep "Z" marks, sparkles, detached pixels, or
overlapping frames. Give the four sprites clear separation and generous padding;
no body part may be cropped.
