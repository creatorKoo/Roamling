# B-cut — gaze with perked ears (set aside 2026-08-31)

Four attempts at a dedicated `gaze` row: the cat sits as in idle, ears swivelled
up, and blinks slowly while the pointer is near. Set aside in favour of driving
the existing tail flick faster as the pointer closes, which needs no drawing at
all and spends a channel -- speed -- that nothing else was using.

`cells/` is the best take (v4, whiskers trimmed to two a side). It is finished
enough to use if a still, ears-up pose is ever wanted.

## What each attempt got wrong

```
v1  body 0.72 wide against the reference's 0.83, no whiskers
v2  same narrow head (cheeks 100 against 113), no whiskers
v3  cheeks 111, whiskers present -- but 16px taller and four whiskers a side
v4  cheeks 113, still 12px taller, three to four whiskers a side
```

Two things never converged in four tries.

**Height.** Every attempt drew the ears *longer* as well as more upright, so the
whole cat grew 12-16px and read as a bigger cat rather than an alert one. The
last prompt said in as many words that this is a change of angle and not of
length; it came back 12px taller anyway.

It cannot be fixed on import either: scaling to the reference's height shrinks
the cheeks from 113 to about 105 and undoes the width match. Once the ears are
drawn long, no single scale satisfies both.

**Whisker count.** The reference has two a side. The first prompt asked for three
-- that was our error, not the generator's -- and later attempts came back with
three or four even after the count was corrected and the generator was told to
count them in the reference. The cells here have the extras deleted rather than
redrawn.

Neither failure says the pose is wrong. Both say this row costs more attempts
than the signal is worth when a tail is already available.
