# Proposal

## Why

Automatic masking (change `privacy-capture`) only finds password fields. Everything else a step shows, such as a customer name, an email or a balance, reaches the server unless the user can remove it first. The review screen already shows every screenshot but can only cancel or generate. The obvious design is to let the user blur an area by dragging, or delete a step, with the blur applied to the file, plus an honest description of what the recorder sends and checks. This change departs from that in two ways (design D0): an opaque box instead of blur, and the box applied to the uploaded bytes while the file on disk stays the original.

## What Changes

- The review screen lets the user draw opaque boxes ("Schwärzen") on any screenshot and remove them again. Boxes are saved the moment they are drawn, so they survive a restart, and stay editable until upload.
- A box shows as applied only once it is saved; while a save is pending or has failed, Generate is not possible.
- Boxes are burned into the image bytes at upload, on every upload path of this recorder (Generate, the unfinished-recording offer, generate from folder, retries). If a box cannot be applied, the upload is aborted. The PNG on disk stays the original.
- The review screen lets the user delete a step. Its screenshot and sidecar are removed; numbering keeps the gap; the audio is not cut.
- A German privacy text (`docs/PRIVACY.md`) states what is sent, what is checked and what is not, including that local originals stay unredacted, deleted steps keep their narration (which can appear in a neighbouring step), and boxes do not change guides generated earlier. A short version appears in the app and an English summary in the README.

Out of scope: blur effects, automatic detection beyond password fields, cutting audio, renumbering steps.

## Capabilities

### New Capabilities

- `step-review`: what the user can do with the captured steps between stopping a recording and uploading it: black out areas and delete steps.

### Modified Capabilities

- `guide-generation`: the uploaded screenshots carry the user's boxes, and an upload whose boxes cannot be applied does not happen.

## Impact

- `ReviewScreen.tsx`: drawing, removing boxes, delete per step; new i18n strings.
- New Tauri commands: load, save and reset boxes, delete a step; `list_session_screenshots` returns step numbers.
- New per-recording file `redactions.json` next to the screenshots.
- `src-tauri/src/commands/generate.rs` / `network/upload.rs`: burn boxes into the bytes before the multipart part is built.
- `docs/PRIVACY.md`, `README.md`, settings and review screen text.
- Depends on `privacy-capture` for the flagged-step behaviour on the review screen and for the privacy text (section 4: the settings section it creates, and the behaviour the text describes). Sections 1 to 3 apply independently.
