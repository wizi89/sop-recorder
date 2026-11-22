# CogniClone Recorder

The desktop app that records a workflow on the user's machine, with narration and one screenshot per
action, and sends it to the server, which turns it into a guide. German UI words are given in
quotes where the UI uses them.

## Language

### Recording

**Recorder**:
The desktop app this repo builds. It records and sends; it does not write guides itself.
_Avoid_: client app, capture tool

**Server**:
The service that turns a recording into a guide. The recorder knows it only through the server
contract.

**Recording**:
One run from Start to Stop: the narration and the steps captured in it, kept together as one folder
on the user's machine.
_Avoid_: recording session, capture (for the whole run)

**Narration**:
What the user says while recording, kept as one audio track for the whole recording. It is never cut
per step.
_Avoid_: voice-over

**Step**:
One user action during a recording, a left click or the Return key, documented by one screenshot.
_Avoid_: event, click (for the step itself)

**Step number**:
The label a step gets when it is captured. It never changes: numbers can have gaps, and their order
is the order of the steps.
_Avoid_: index, position

**Trigger**:
What started a step: a click or the Return key.

**Failed capture**:
A step whose screenshot could not be taken or saved. It leaves a gap in the step numbers and is
reported before Generate.

**Marker**:
The ring drawn around the click point on a step's screenshot. The clicked spot itself stays
unchanged.

**Recording bar**:
The small always-on-top window shown while recording. It is not part of any screenshot.

**Unfinished recording**:
A stopped recording that has not produced a guide yet. The recorder offers to pick it up again.

### Capture

**Monitor**:
One physical display. A step only ever shows the monitor its click landed on.

**Capture mode**:
The user's choice of what screenshots show: the active window ("Aktives Fenster") or the whole
monitor ("Ganzer Bildschirm"). It is fixed for a recording when it starts.

**Active window**:
The window the user is working in at a step, together with the window that owns it and the menus or
dropdowns it has open. The default capture mode shows only this.
_Avoid_: current window, focused window

**Capture area**:
The part of the monitor a step's screenshot keeps.

**Capture scope**:
What a step's screenshot actually shows: the active window, the monitor by choice, the monitor
because no usable window was found (monitor fallback), or unknown.

### Privacy

**Masking**:
The recorder blacking out, automatically and before anything is saved, what must not appear in a
screenshot: password fields, and other apps' windows lying over the capture area.
_Avoid_: redaction (for the automatic part)

**Password check**:
The search for password fields in the windows a step shows. Its outcome per step is ok, flagged, off
or unknown.

**Flagged**:
A password check outcome: the check could not confirm it found every password field.
_Avoid_: failed (for this outcome)

**Partial window list**:
A step for which the recorder could not see every window over the capture area, so another app's
window may not have been masked.

**Review mark**:
The "Prüfen" mark on a step the user has to look at before Generate: its password check is not ok,
its capture scope is a fallback or unknown, or its window list was partial. One tick confirms all
marked steps of a recording.
_Avoid_: flag (for the mark), warning

**Redaction**:
A box the user draws on a step on the review screen. It is blacked out in what is sent; the
screenshot on the user's disk stays the original.
_Avoid_: blur, mask (for a user's box)

**Step deletion**:
Removing a step on the review screen. Its number stays a gap, and its narration is still sent.

**PII check**:
The server's check of a recording for personal data before it generates. It can refuse a recording,
and the user can switch it off.
_Avoid_: privacy check, PII filter

**Error report**:
A description of a failure, with a cleaned-up log excerpt, sent only with the user's consent.
_Avoid_: crash report

### Generation

**Review screen**:
The screen between Stop and Generate where the user looks over, redacts and deletes steps.

**Generate**:
Sending a recording to the server and receiving its guide back. It can be repeated for any earlier
recording ("Aus Ordner generieren").

**Guide**:
The step-by-step instructions the server makes from one recording, saved next to it.
_Avoid_: SOP (for the file), document

**Generation job**:
The server's run that turns one recording into a guide. The recorder can follow it again after its
connection drops.

**Pipeline**:
A named way of generating a guide that the server offers and the user can pick.

**Generation model**:
The AI model the server uses to write the guide, chosen from the ones the server offers.

**Quota**:
How many guides the user may generate, and how many are left.

**Upload target**:
Which server a recording goes to: production, staging or local.

### Account

**Desktop sign-in**:
Signing in through the user's browser, which gives the recorder a session of its own, separate from
the browser's.
_Avoid_: desktop login, device login

**Password sign-in**:
Signing in with email and password in the app itself; the second way in, kept for machines where the
browser handoff fails.
_Avoid_: legacy login

**Session**:
The recorder's signed-in state, which survives a restart until the user signs out or the server
rejects it. Never a recording.

**Server contract**:
The agreed wire format between recorder and server, written in `docs/server-contract.md`. Both
sides follow it.
_Avoid_: API spec
