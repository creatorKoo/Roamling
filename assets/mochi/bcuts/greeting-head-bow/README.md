# B-cut — head bow (set aside 2026-08-31)

Made as candidate A for the `review` row. Rejected there because a repeated bow
is what `review` would look like: that row fires every time the agent reads a
file, at most once per 1.5s, so the pet would be nodding almost continuously and
it reads as bowing rather than reading.

Kept because the pose itself is good and this repo has no greeting animation yet.
If one is ever wanted -- a session opening, a first-run hello -- this is a
finished six-frame bow with the right identity.

## What it is

```
1  neutral            2  head lowering, ears rotating
3  head down          4  down, eyes shifted
5  head coming up     6  neutral
```

## Known defects, if it is ever used

- The ears **fold flat** in frames 2-4 rather than staying upright. On a cat that
  reads as fear, not deference. Would need redrawing for a friendly greeting.
- Visible height drops 148 -> 114, a 34px collapse. That is deeper than the
  `failed` slump (30px), so it currently looks more dejected than a failure.
- Detached 1-3px pieces in frames 1, 3, 4 and 6.

Both problems trace to the same prompt mistake: it asked for ears "rotating
forward", and a front-facing sprite cannot show depth-direction rotation. The
generator drew the only thing it could -- folded ears. Any reuse should describe
the ears as staying upright and put the signal in the head angle alone.
