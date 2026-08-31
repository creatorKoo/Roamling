# Mochi v3 — `caught` (picked up and carried)

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
held up in the air instead of sitting on the ground.

## What this is for

The user has grabbed the pet with the mouse and is carrying it. This plays for as
long as they hold it, so it loops.

Nothing is drawn holding the cat — there is no hand, no cursor, nothing but the
cat. It has to read as **lifted** on its own: what makes that clear is gravity,
so the legs and tail hang.

## The pose

A kitten held up under its front legs, dangling, seen from the same front-facing
angle as the reference.

- the body hangs **vertically**, longer than it is wide — the opposite of the
  seated pose, where it is wider than tall
- **but only a little longer.** From the ear tips to the lowest hanging thing,
  the whole cat is about one and a half times its own width, no more. A previous
  attempt stretched it to two and a half times, which does not fit the frame and
  reads as a cat pulled like taffy rather than a kitten held up.
- the front paws are up near the chest, small and tucked
- **the hind legs hang loose**, not tucked and not standing — but they stay
  **bent and short**, the way a kitten's do. They do not extend into long
  straight legs.
- the tail hangs behind in a **soft curve**, its stripes visible. It does not
  hang straight down at full length; that alone made the last attempt too tall
- the head is tipped up slightly, looking at the viewer with wide open eyes
- the ears stay upright

This must not look like a cat standing on its hind legs. Nothing supports its
weight; everything below the chest is hanging.

## The animation — a small paddle

Four frames of the loose scramble a carried cat makes, and nothing more.

1. hind legs hanging, the left a little forward
2. passing through, both near vertical
3. hind legs hanging, the right a little forward
4. passing through again

The paddle is small — each hind paw moves three or four pixels, no more. The
head, chest and front paws stay where they are, and the body does not rise, fall
or swing sideways. Frame 4 leads back into frame 1, because this loops.

The head stays the size it is in the reference. It is the one part of a cat that
does not change when the animal is picked up.

Very short legs paddling close to the body. It must never read as human arms
waving.

## Rules

Every frame is a **newly and completely drawn full-body Mochi**. Do not paste,
mirror, or reuse parts of another frame.

Whiskers are present in all four frames, two per side, drawn as thin dark lines —
one flat colour, no outline around them. The mouth stays closed. The cat has four
legs and no more.

Background is flat solid `#00FF00` chroma key only. No shadows, floor, ground,
glow, gradients, texture, text, labels, visible grid, guide marks, hands, arms,
cursors, props, motion lines, sparkles, detached pixels, or overlapping frames.
Give the four sprites clear separation and generous padding; no body part may be
cropped.
