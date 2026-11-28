# Doubt in masking becomes a review mark, never "ok"

Masking happens in memory before the first file is written, so no unmasked screenshot ever exists on
disk. The password check runs after the pixels are read and may be slow, time out, meet an elevated
window it cannot query, or see a field appear or vanish; every such case ends as `flagged` or
`unknown` and puts a review mark on the step, and Generate needs one confirmation tick. We chose
"masked as far as known, then shown to the user" over blocking the step or retrying until sure,
because a check that waits would delay the screenshot past the moment the user clicked.

## Consequences

- A missed field can still reach the server after the user ticks the confirmation; the review
  screen's redactions are the remedy.
- The check never holds up the pixels: it runs in a second phase with its own time budget, and
  abandoned queries are capped.
- Masking uses the union of what earlier queries found and what the step's own query finds, so a
  submitted login whose field is already gone stays masked; over-masking a stale rectangle is the
  accepted cost.
- What earlier queries found is taken as of the step's trigger (the click or the Return key), not
  as of the pixel read: a query that completes between the trigger and the read never drops a
  cached field from that step's screenshot. A page can remove the field from its accessibility tree
  the moment the Return key lands while the old pixels are still on screen (owner decision
  2026-11-28, #66).
- If the flag rate on ordinary work gets high, the tick becomes a reflex; that rate is watched.
