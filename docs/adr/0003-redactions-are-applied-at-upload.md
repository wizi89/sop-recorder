# Redactions are opaque boxes applied at upload; originals stay on disk

A redaction is an opaque box, not a blur, because blur can be partly reversed and small text stays
guessable. The boxes are stored apart from the screenshots (`redactions.json`) and burned into the
image bytes once per upload, on every upload path; the PNG on disk stays the original. That keeps a
box removable until upload and makes the redacted copy the only one that leaves the machine. Writing
a redacted copy to disk was rejected: a second file set to keep in sync and clean up.

## Consequences

- Other tools reading the folder, and older recorder versions, see the unredacted originals; the
  privacy text says so.
- Any doubt refuses the upload: an unreadable boxes file, a box that cannot be applied, or a box
  stored for a step whose screenshot is gone.
- Boxes do not touch guides generated earlier.
