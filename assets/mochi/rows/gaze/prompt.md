# Mochi v3 — `gaze` (watching the cursor)

Create one pixel-art sprite strip with **exactly four complete full-body frames**
in a single horizontal row. Each frame occupies a 192 × 208 cell of a Codex
Petdex animation atlas.

## Identity

`reference/idle-neutral.png` is the identity lock and the base pose. It is Mochi:
a compact calico kitten with large brown eyes, asymmetrical orange, dark-brown
and cream markings, a dark outline, tiny paws, a short striped tail, small pink
paw pads, pink inner ears, **two** whiskers on each side of the
muzzle, a small pink triangular nose, and a thin line mouth. `reference/idle-row.png` shows the
approved scale and proportion.

Preserve exactly: body scale, body centre, ground line, tail position and
stripes, marking placement, palette, outline weight, and face proportions. The
cat is seated and facing forward, with all four paws on the floor.

**Match the reference's proportions, head first.** Mochi's head is nearly as wide
as its seated body -- the cheeks reach almost as far out as the haunches do, and
the head takes up roughly the top half of the whole animal. Two attempts at this
row came back with a head a good deal narrower than that, which beside the
approved artwork reads as a different, thinner cat. Draw the head large and round
and set the ears on top of it.

## What this is for

This plays while the mouse pointer is near the pet, for as long as it stays
near. It is the quietest thing the cat does — it has noticed you and is
watching, nothing more. It must be unmistakably *attentive* and completely
*still*.

## The four frames — one slow blink

The only difference from `idle-neutral.png` is in the ears and the eyes.

1. the same seated pose, but **the ears have rotated to stand straight up** and
   the eyes are a little wider, looking directly at the viewer
2. the eyelids halfway down
3. the eyes closed
4. the eyelids halfway up again

**The cat is exactly the same size as the reference.** It has not grown, and the
top of its head is at the same height. In the reference the ears lean outward;
here they swivel upright, so the silhouette changes at the ears while the
animal's overall height does not. A previous attempt made the whole cat sixteen
pixels taller, which reads as a different, bigger cat rather than an alert one.

The ears stand *up*. They do not rotate forward, fold, flatten, or tilt sideways.
A strip for another row folded them flat and the cat read as frightened.

Frames 2, 3 and 4 must be identical to frame 1 in every respect except how far
the eyelids have come down: same ear angle, same head position, same body, same
tail, same paws, same markings. Frames 2 and 4 are the same amount of eyelid,
caught on the way down and on the way up.

Frame 1 is held for most of the loop and the other three pass quickly, so frame 1
is the one that must read as a settled, attentive rest pose.

## Rules

Every frame is a **newly and completely drawn full-body Mochi**. Do not paste,
mirror, or reuse parts of another frame; do not generate only a head or a pair of
eyes and composite them onto a body.

**Whiskers, in every frame — two per side, no more.** Copy them from the
reference and count them there: two short straight dark strokes on each side of
the muzzle, not three and not four. They begin at the edge of the cheek at about
the height of the nose and reach outward past the outline of the head, so they
show against the background.

Two attempts at this row came back with bare cheeks, one came back with four per
side, and another row came back with them drawn as thick brown sticks with their
own outline. A whisker is a thin dark line, one flat colour, no border.

The mouth stays closed — a thin line, never open. Four paws on the floor.

Background is flat solid `#00FF00` chroma key only. No shadows, floor, glow,
gradients, texture, text, labels, visible grid, guide marks, props, motion lines,
sparkles, detached pixels, or overlapping frames. Give the four sprites clear
separation and generous padding; no body part may be cropped.
