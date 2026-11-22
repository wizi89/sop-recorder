# Step numbers are labels, taken from the file name

A step's number is the `NN` in `step_NN.png`, given at capture and never changed. A failed capture or
a deleted step leaves a gap; nothing is renumbered, and the UI and every command address a step by
this number, never by its position in a list. Renumbering on delete was rejected: boxes, sidecars and
the server's pairing of screenshots with step metadata all key on the number, and renaming files
mid-review would open a window where they disagree.

## Consequences

- The server receives `step_NN` parts that may skip numbers and pairs them with the per-step metadata
  in ascending order (see `docs/server-contract.md`).
- Deleting a step does not cut the narration, so what was said during it can appear in a neighbouring
  step of the guide.
