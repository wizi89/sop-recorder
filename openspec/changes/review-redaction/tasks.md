## 1. Storage and commands

- [ ] 1.1 `redactions.json` read/write (design D1): atomic write via a uniquely named temp file, missing = empty, unparsable or unknown `version` = unreadable error; tests for round trip, missing, corrupt, unknown version
- [ ] 1.2 `load_redactions`, `save_redactions`, `reset_redactions` and `delete_step` commands with folder and step validation (design D5); tests that a folder without `recording.wav` and `screenshots/` is refused, that a folder picked from outside the configured output directory is accepted, that `save_redactions` refuses to overwrite an unreadable file, that concurrent saves for different steps of one recording both persist, that a save concurrent with a delete keeps the saved box, and that `load_redactions` reports leftover entries and sidecars without a screenshot
- [ ] 1.3 `delete_step` order PNG, sidecar, entry, idempotent (design D5); tests that it removes the three only for that step, that a failure after the PNG leaves an entry the upload refuses, and that a retry completes
- [ ] 1.4 `list_session_screenshots` returns `{ step, path }` with the number parsed from the file name (design D4); `lib/tauri.ts` updated; test with a gap (files 1, 3, 4)

## 2. Burn at upload

- [ ] 2.1 Pure `apply_boxes` fill, clipped to the image (shared with `privacy-capture` masking if it landed first); tests for inside, partly outside, fully outside
- [ ] 2.2 `run_generation_inner`: build the in-memory upload list once (design D2); `upload_multipart` / `upload_with_retry` take bytes on both call sites; refuse to upload with a step-naming error when boxes cannot be read or applied or an entry has no screenshot
- [ ] 2.3 Tests: no boxes means bytes identical to disk; boxes produce opaque pixels only inside; a retry sends the same bytes; corrupt `redactions.json` uploads nothing; an entry without a screenshot uploads nothing; boxes on a recording with a gap land on the right step
- [ ] 2.4 Gap warning text per design D6

## 3. Review screen

- [ ] 3.1 Labels and every command use the step number from 1.4, never the list position (design D4)
- [ ] 3.2 Click-to-enlarge a step with `object-fit: contain`; draw boxes by dragging; remove a box; pure coordinate mapping against the rendered content rectangle with outward rounding, unit-tested without layout (letterboxed horizontally and vertically, fractional scale); thumbnails switch to `contain` and show boxes through the same mapping
- [ ] 3.3 Load boxes on mount; saves through one queue; a box shows opaque only after its save returned, as an outline while pending or failed; failed save shows an error with retry; Generate disabled while any save, delete or reset is pending, any save failed, or leftover steps exist; unreadable storage shows an error with "Schwärzungen zurücksetzen" (confirmation, then the file is deleted), drawing and Generate disabled until then (design D5)
- [ ] 3.4 Delete a step with a short confirmation that says the narration stays and can appear in a neighbouring step; thumbnails update; a failed delete shows an error; leftover steps from `load_redactions` listed with "Erneut löschen"; an upload refused because of boxes returns to the review screen with the error (design D5); deleting the last step disables Generate; the failed-capture notice keeps its total and states deleted steps separately (design D6)
- [ ] 3.5 Flagged-step confirmation (from `privacy-capture`) recomputes after a delete
- [ ] 3.6 German strings in `i18n/de.ts` with real umlauts ("Schwärzen", "Schritt löschen", "Schwärzungen zurücksetzen"); component tests for draw, remove, delete, a failed save blocking Generate, corrupt storage, and a recording with a gap

## 4. Privacy text (after `privacy-capture` is fully implemented)

- [ ] 4.1 `docs/PRIVACY.md` in German per design D7, checked against the implemented behaviour of both changes
- [ ] 4.2 Short in-app version and link in the settings section "Datenschutz bei der Aufnahme"
- [ ] 4.3 English summary in `README.md`
- [ ] 4.4 In `sop-sorcery` `docs/ROADMAP.md`: reword the M2 review row to the decided behaviour (opaque box, applied to the upload, original kept locally; design D0) and point M4's pilot agreement item at `docs/PRIVACY.md`

## 5. Verification

- [ ] 5.1 Manual run: draw, remove, restart mid-review, upload; download the stored screenshots from the generated guide and confirm the boxes; delete a middle step and check the guide's step order; a recording with a failed capture gap; on Windows, make `redactions.json` read-only and confirm the save fails and Generate is blocked (on macOS a rename over a read-only file succeeds, so this check is Windows-only)
- [ ] 5.2 CHANGELOG entry
