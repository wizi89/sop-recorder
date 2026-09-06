## Purpose

Assembling a finished recording folder into an upload and reporting progress while the server generates the guide: which screenshots are sent, whether per-step timing accompanies them, and what the user sees during the wait.

## ADDED Requirements

### Requirement: Every saved screenshot is uploaded

Generation SHALL upload every screenshot present in the recording folder. A missing step number in the sequence SHALL NOT stop enumeration or exclude the screenshots that follow it.

#### Scenario: A step number is missing

- **WHEN** the recording folder contains `step_01.png`, `step_03.png` and `step_04.png`
- **THEN** all three are uploaded, in ascending step order
- **AND** the gap is recorded in the log with the missing step number

#### Scenario: Step numbers past nine

- **WHEN** the recording folder contains steps 1 through 21
- **THEN** all 21 are uploaded in ascending numeric order, not lexicographic order

#### Scenario: No screenshots at all

- **WHEN** the recording folder contains no screenshots
- **THEN** generation fails with an error naming the empty folder, and nothing is uploaded

### Requirement: Losing per-step alignment is visible

Per-step metadata (timing and click position) SHALL accompany the upload whenever it is complete. When it cannot be sent because it does not match the screenshots, that SHALL be recorded with both counts, because the resulting guide loses its link to what the user said at each step.

#### Scenario: Metadata matches the screenshots

- **WHEN** the number of per-step metadata records equals the number of screenshots
- **THEN** the metadata is included in the upload

#### Scenario: Metadata does not match

- **WHEN** the counts differ
- **THEN** no metadata is sent, and the log records both counts and that alignment was dropped

### Requirement: Processing shows that it is still working

While the server generates a guide, the app SHALL continuously indicate that work is in progress, so a long wait is distinguishable from a stalled app.

#### Scenario: Time passes between status messages

- **WHEN** generation is running
- **THEN** the elapsed processing time is displayed and updates while the user waits

#### Scenario: A long silence between status messages

- **WHEN** no new status message arrives for 20 seconds
- **THEN** the user is told that the app is still waiting on the server
- **AND** the notice clears when the next status message arrives

### Requirement: A recovered connection is not reported as a failure

A dropped server connection that reconnects and delivers the result SHALL be presented as a recovery, not as an error. Only an exhausted retry budget SHALL be presented as a failure.

#### Scenario: The connection drops and recovers

- **WHEN** the status connection drops and reconnects, and the result arrives
- **THEN** the user sees a reconnecting notice while it is down and the normal result afterwards
- **AND** no error state is shown
- **AND** each reconnect attempt is logged with its cause

#### Scenario: Reconnection never succeeds

- **WHEN** the reconnect budget is exhausted
- **THEN** an error is shown, and it is reportable through the existing error-report flow
