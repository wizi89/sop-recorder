## Purpose

Turning a user input event during a recording into a saved screenshot that documents one step: which screen area is captured, how the click is marked on it, and what the user is told when a capture does not succeed.

## ADDED Requirements

### Requirement: Capture is scoped to the screen the action happened on

A captured step SHALL record the contents of a single display — the one the click landed on — rather than a composite of every connected display. This bounds the downscale applied before upload, so image legibility does not degrade as displays are added.

#### Scenario: Click on a secondary display

- **WHEN** two displays are connected and the user clicks on the secondary one
- **THEN** the saved screenshot contains only the secondary display's contents
- **AND** the click marker is positioned relative to that display's origin

#### Scenario: Two 4K displays do not degrade legibility

- **WHEN** two 3840×2160 displays are connected and a step is captured
- **THEN** the saved image is at least 1600 pixels wide
- **AND** it is no smaller than the image the same click would produce with only that display connected

#### Scenario: The triggering event carries no cursor position

- **WHEN** a step is triggered by a key press rather than a click, so no click position is available
- **THEN** the display under the current cursor position is captured
- **AND** if the cursor position is also unavailable, the primary display is captured

#### Scenario: The position falls outside every known display

- **WHEN** the reported position lies outside the bounds of all connected displays
- **THEN** the primary display is captured and the step is saved

### Requirement: The click marker must not obscure what was clicked

The marker drawn onto a captured step SHALL leave the pixels at the click point unmodified, so the control the user acted on remains readable in the guide and to downstream image analysis.

#### Scenario: The clicked element stays visible

- **WHEN** a step is captured with a known click position
- **THEN** the pixel at the click point is unchanged from the original capture
- **AND** the marker is still visible as a distinct shape around that point

#### Scenario: Marker geometry is unchanged for consumers

- **WHEN** a step is captured with a known click position
- **THEN** the marker bounds reported in the step's metadata cover the full drawn marker, at the same coordinates the previous filled marker reported for the same click and scale

### Requirement: Concurrent captures are bounded

The recorder SHALL limit how many screenshot captures run at the same time. A burst of rapid input SHALL queue rather than start an unbounded number of simultaneous captures, each of which allocates a full-screen image buffer.

#### Scenario: Rapid clicking

- **WHEN** the user produces more input events than the concurrency limit within a short interval
- **THEN** no more than the limit are captured simultaneously
- **AND** every event is eventually captured, in the order its step number was assigned

#### Scenario: Stopping waits for the queue

- **WHEN** the user stops the recording while captures are queued or running
- **THEN** stopping waits for them to finish before the recording is reported as stopped, within the existing timeout

### Requirement: A failed capture is reported, not swallowed

When a screenshot cannot be captured or saved, the recorder SHALL count the failure and surface it to the user before generation. A recording that lost steps SHALL NOT be presented as complete.

#### Scenario: One capture fails during a recording

- **WHEN** a capture fails and the rest succeed
- **THEN** the failure is counted for the recording
- **AND** after stopping, the user is shown how many steps could not be captured

#### Scenario: No capture fails

- **WHEN** every capture in a recording succeeds
- **THEN** no failure notice is shown
