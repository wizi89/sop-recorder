# Design

## Context

`ReviewScreen.tsx` shows thumbnails loaded via `list_session_screenshots` and `read_screenshot_bytes` and offers Cancel or Generate. It labels steps by list position (`idx + 1`), and thumbnails are 96x54 with `objectFit: "cover"`. The failed-capture notice computes its total as shown screenshots plus failed captures. The unfinished-recording offer (`useOfferUnfinished`) and "generate from folder" (`generateFromDir`, any folder the user picks, not only under the configured output directory) both open the recording on the review screen, never upload directly. Step numbers on disk can already have gaps (failed captures). The server pairs `metadata.steps` to the screenshots by position and only when the counts agree (`steps_for_upload`). Every upload goes through `run_generation_inner` (`commands/generate.rs`), which collects `step_NN.png` with `collect_step_screenshots` and calls `upload::upload_with_retry` (twice on the 401 path); `upload_multipart` reads each PNG from disk per attempt. Saved screenshots are at most 1920x1080 and already carry the click marker and any password masking. The server slices the transcript per step by the neighbouring steps' timestamps (server side). See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**
- One place where boxes are applied, which every upload from this recorder passes through.
- What the review screen shows as blacked out is exactly what is blacked out in the upload; any doubt blocks the upload.
- Nothing about a box depends on the review screen's display size.

**Non-Goals:**
- Undo history beyond removing a box; editing a box after drawing (remove and redraw).
- Deleting steps during recording (the existing `delete_last_screenshot` stays as it is).
- Redacting screenshots already uploaded by earlier generations, or other tools reading the folder (server-side tools, older recorder versions).

## Decisions

**D0. An opaque box applied to the upload, not a blur applied to the file.** The obvious design blurs an area dragged by the user and writes the blur into the file. This change instead (a) fills an opaque box rather than blurring, because blur can be partly reversed and small text stays guessable; and (b) keeps the original PNG on disk and applies the boxes to the bytes at upload, so a box stays removable until upload and the only copy that leaves the machine is redacted. The redaction takes effect on what is used, not only on screen, for the upload; it does not for the local folder, and the privacy text says so.

**D1. Boxes live in one file per recording, `screenshots/redactions.json`.** Shape: `{ "version": 1, "steps": { "3": [[x, y, w, h], ...] } }`, keys are the number in the file name `step_NN.png`, coordinates are integer saved-image pixels. Not in `step_NN.json`: a sidecar can be missing for a step (crash between PNG and sidecar), and the sidecar count drives metadata alignment (`steps_for_upload`), so creating one just to hold boxes would change upload behaviour. Missing file = no boxes. Present but unparsable, or a `version` other than 1, is "unreadable": upload refused (spec), review screen shows an error (D5). Written via a uniquely named temp file in the same folder and rename, so a crash never leaves half a file.

**D2. Burn into bytes before the upload loop, not inside it.** `run_generation_inner` reads `redactions.json` once, after the review screen has no save in flight (D5), and for each step with boxes decodes the PNG, fills the boxes (clipped to the image), and re-encodes into memory. The result, a list of `(order, file name, bytes)`, is what `upload_with_retry` sends on every attempt including the 401 retry, so retries send the same burned bytes and cannot reach the originals. Steps without boxes pass their file bytes through unchanged. `upload_multipart` changes from paths to bytes. An entry in `redactions.json` whose step has no `step_NN.png` refuses the upload with an error naming the step: it means a box was stored against the wrong step, and the step it was meant for would go out unredacted. A `step_NN.json` without its PNG (a delete that failed part way) refuses the upload the same way, so the retry offered by D5 is not skipped. This list is also the set `privacy-capture` D9 counts per uploaded screenshot, so a deleted step is neither sent nor counted. Alternative considered: a burned copy on disk before upload; rejected because it creates a second file set to keep in sync and clean up.

**D3. The fill function is pure and shared.** `fn apply_boxes(img: &mut RgbaImage, boxes: &[Rect])` (the capture path works on `RgbaImage`; a decoded PNG is converted to it) lives next to the password-mask fill from `privacy-capture` (same opaque colour); both changes call it. Whichever change lands second reuses the first's function.

**D4. Step identity and coordinates on the review screen.** The step number always comes from the file name: `list_session_screenshots` returns `{ step, path }` pairs, labels show that number (so steps keep their numbers after a delete and across gaps), and every command is called with it, never with a list position. Boxes are drawn in a click-to-enlarge view of one step that shows the image with `object-fit: contain`. A pure function maps between pointer positions and saved-image pixels using the image's rendered content rectangle (element box minus the letterbox offsets that `contain` produces, from `naturalWidth`/`naturalHeight`), so stored boxes are independent of window size. A drawn box is rounded outward before storing (floor for the left and top edge, ceil for the right and bottom edge), so the stored box covers at least what was shown. Boxes render as absolutely positioned black divs mapped back through the same function. Thumbnails switch to `contain` and render the boxes through the same function, so the strip never shows a box in the wrong place.

**D5. Commands and save consistency.**
- `load_redactions(output_dir)` returns the stored boxes and the step numbers left by a delete that failed part way (a `redactions.json` entry or a `step_NN.json` with no `step_NN.png`; a sidecar is written only after its PNG, so it is never alone otherwise), or an "unreadable" error. Such leftover steps are listed on the review screen with "Erneut löschen", which re-runs `delete_step` for that number, so the block from D2 can always be cleared in the app, also after a restart. On that error the review screen shows a message with the action "Schwärzungen zurücksetzen", which, after a confirmation, deletes the file (`reset_redactions(output_dir)`); drawing and Generate are disabled until then. `save_redactions` refuses to overwrite an unreadable file.
- `save_redactions(output_dir, step, boxes)` replaces the boxes for one step (empty list removes the entry) and returns the stored list for that step. Every write to `redactions.json` (save, the entry removal in `delete_step`, the reset) holds one global mutex in the backend, so no path spelling can give two writers different locks; the frontend sends saves, deletes and the reset through one queue, in order.
- The review screen shows a box as applied (opaque black) only after its save has returned. While a save is pending the box is shown as an outline; if a save fails the step shows an error and the box stays an outline. Generate is disabled while any queued operation (save, delete, reset) is pending, while any save has failed, and while leftover steps exist; a failed save can be retried.
- `delete_step(output_dir, step)` removes, in this order, `step_NN.png`, then `step_NN.json`, then the step's entry in `redactions.json`, and is idempotent (an already missing file counts as removed). The entry goes last, so a failure part way never leaves the PNG in the upload without its boxes. If a later part fails, the command returns an error, the screen reports it, and the leftover entry with no PNG refuses the upload (D2) until the delete is retried. If only one of PNG and sidecar is removed, the counts diverge and `steps_for_upload` drops the per-step metadata as it does today.
- An upload refused because of boxes (unreadable file, a box that cannot be applied, a leftover entry) returns the user to the review screen with the error, instead of the error screen whose Retry would repeat the same refusal.
- All four commands validate that `output_dir` is a recording folder (contains `recording.wav` and `screenshots/`; not restricted to the configured output directory, because "generate from folder" opens any folder) and take `step` as an integer from which the file names are built, so the frontend can only touch `step_NN.png`, `step_NN.json` and `redactions.json` inside a recording folder.

**D6. Gap log line and counts.** `collect_step_screenshots` already logs missing numbers as a warning. A deleted step looks the same on disk; the warning text changes to "missing (failed capture or deleted in review)" rather than tracking deletions separately. On the review screen the failed-capture notice keeps the total from when the screen opened; steps deleted in review are stated separately ("N gelöscht").

**D7. Privacy text is written last.** `docs/PRIVACY.md` (German) is written after both changes are implemented, from what the code does, and lists:
- sent: screenshots after masking and boxes, audio, per-step metadata, capture settings;
- checked: password fields on Windows when on; server PII check on transcript and OCR text, skippable;
- not checked: audio content, toasts inside the captured window, macOS full monitor without masking, anything visible that the user did not black out;
- kept locally: original screenshots and audio in the output folder, unredacted; boxes are applied by this recorder at upload, so other tools reading the folder see the originals;
- deleting a step does not cut the audio: its narration is sent and can appear in the text of a neighbouring step;
- boxes affect only uploads made after they are drawn: a guide generated earlier keeps its screenshots on the server until it is deleted there.
The app shows a two-sentence version under "Datenschutz bei der Aufnahme" (created by `privacy-capture`) with a link; README gets an English summary.

## Risks / Trade-offs

- [Re-encoding PNGs at upload costs time] → only steps with boxes are re-encoded; at most 1920x1080 each.
- [Unredacted originals remain on the user's disk] → decision D0; stated in the privacy text; the user saw them already, and the threat addressed here is the upload.
- [Narration of a deleted step still reaches the server and can surface in a neighbouring step] → stated in the privacy text and in the delete confirmation.
- [Box drawn on a thumbnail is imprecise] → drawing happens in the enlarged view (D4).
- [A save error blocks Generate] → intended; the user retries the save or removes the box.

## Migration Plan

Ships in a normal recorder release, ideally the same one as `privacy-capture`. No data migration: recordings without `redactions.json` behave as today. An older recorder opening a folder ignores `redactions.json`; the privacy text says boxes are applied by this version onward.
