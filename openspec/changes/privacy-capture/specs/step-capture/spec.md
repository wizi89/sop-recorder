# Spec Delta

## MODIFIED Requirements

### Requirement: Capture is scoped to the screen the action happened on

In the monitor capture mode, a captured step SHALL record the contents of a single display — the one the click landed on — rather than a composite of every connected display. This bounds the downscale applied before upload, so image legibility does not degrade as displays are added.

#### Scenario: Click on a secondary display

- **WHEN** the capture mode is monitor, two displays are connected and the user clicks on the secondary one
- **THEN** the saved screenshot contains only the secondary display's contents
- **AND** the click marker is positioned relative to that display's origin

#### Scenario: Two 4K displays do not degrade legibility

- **WHEN** the capture mode is monitor, two 3840×2160 displays are connected and a step is captured
- **THEN** the saved image is at least 1600 pixels wide
- **AND** it is no smaller than the image the same click would produce with only that display connected

#### Scenario: The triggering event carries no cursor position

- **WHEN** the capture mode is monitor and a step is triggered by a key press rather than a click, so no click position is available
- **THEN** the display under the current cursor position is captured
- **AND** if the cursor position is also unavailable, the primary display is captured

#### Scenario: The position falls outside every known display

- **WHEN** the capture mode is monitor and the reported position lies outside the bounds of all connected displays
- **THEN** the primary display is captured and the step is saved

## ADDED Requirements

### Requirement: The capture mode is a user setting that defaults to the active window

The recorder SHALL offer two capture modes, active window and monitor, as a user setting. Active window SHALL be the default on Windows for new installations and for existing ones that never chose a mode. A change of the setting SHALL apply from the next recording. On macOS only the monitor mode SHALL be offered.

#### Scenario: Fresh installation on Windows

- **WHEN** a user records on Windows without having changed the capture mode
- **THEN** steps are captured in the active window mode

#### Scenario: Existing installation after the update

- **WHEN** a Windows user who recorded with an earlier version updates and records without opening the settings
- **THEN** steps are captured in the active window mode

#### Scenario: Changed during a recording

- **WHEN** the user changes the capture mode while a recording is running
- **THEN** that recording keeps the mode it started with and the next recording uses the new one

#### Scenario: macOS

- **WHEN** a user opens the settings on macOS
- **THEN** only the monitor mode is selectable, no password masking option is shown, and a note says window capture and password masking are not available yet

#### Scenario: Monitor mode is explained

- **WHEN** the user looks at the monitor option in the settings
- **THEN** it states that the taskbar, notifications and all visible windows are captured

### Requirement: Active window capture shows the window and its owner

In the active window mode, a step SHALL show only the area of the foreground window and its root owner window; everything else on the display, including the taskbar, the tray and other windows outside that area, SHALL NOT appear, unless the step's click lands on it. Windows drawn above that area, such as notifications or always-on-top windows, remain visible where they overlap it.

#### Scenario: Click inside a normal application window

- **WHEN** the user clicks inside a non-maximised application window while other windows and the taskbar are visible
- **THEN** the saved screenshot shows that window only
- **AND** the click marker is drawn at the clicked point relative to the saved image

#### Scenario: A dialog in front of its owner

- **WHEN** the foreground window is a dialog owned by an application window
- **THEN** the saved screenshot shows the dialog and its owner window

#### Scenario: A dialog away from its owner

- **WHEN** the dialog is placed so that other windows or the desktop are visible between it and its owner
- **THEN** the area between them is opaque in the saved screenshot

#### Scenario: A menu or popup drawn over the window

- **WHEN** an open dropdown or context menu lies within the window's area
- **THEN** it is visible in the saved screenshot

#### Scenario: A window drawn above the captured window

- **WHEN** an always-on-top window or a notification overlaps the captured window
- **THEN** the overlapping part is visible in the saved screenshot and is included in the password check

#### Scenario: Click on a window that is not yet in front

- **WHEN** the user clicks on a window other than the foreground window
- **THEN** the clicked window and its owner are captured

#### Scenario: Click on the taskbar

- **WHEN** the user clicks a button on the taskbar
- **THEN** the saved screenshot shows the taskbar and nothing else

#### Scenario: Activation completes while the screenshot is taken

- **WHEN** the clicked window comes to the front while the step's pixels are being read
- **THEN** both windows are included in the password check and the step is flagged

#### Scenario: No window can be determined

- **WHEN** no application window can be determined for the step, for example a click on the desktop
- **THEN** the display containing the click is captured, the step is saved and marked for review, and the fallback is logged

#### Scenario: Window partly off screen

- **WHEN** the captured window extends beyond the edge of its display
- **THEN** the saved screenshot contains the visible part only, and the step is saved

### Requirement: Password fields are masked before the screenshot is written

When password masking is on, every password field visible in the saved area SHALL be filled with an opaque colour before the screenshot is written to disk, whether or not the field holds text or was clicked. No unmasked version of the image SHALL be written. This applies on Windows; macOS does not mask in this version.

#### Scenario: Login form in the captured window

- **WHEN** a step is captured while a browser login page with a password field is in the captured area
- **THEN** the password field's area is opaque in the saved screenshot
- **AND** the rest of the window is unchanged

#### Scenario: Password field not clicked

- **WHEN** a password field is visible in the captured area but the user clicked elsewhere
- **THEN** the field is still masked

#### Scenario: Password field in another window inside the area

- **WHEN** a window other than the captured one shows a password field inside the captured area
- **THEN** that field is masked

#### Scenario: Monitor mode

- **WHEN** the capture mode is monitor and a password field is visible in any window on the captured display
- **THEN** that field is masked

#### Scenario: Password field hidden behind another window

- **WHEN** a password field belongs to a window that is fully covered by an opaque window in the captured area
- **THEN** the covering window is not blacked out because of it

#### Scenario: A login page loaded inside the same window and filled by keyboard

- **WHEN** a login page loads in a browser tab after the last step, the user types the password and presses Enter, and the field has disappeared by the time it is checked
- **THEN** the field's area at the time it was focused is masked in the saved screenshot
- **AND** the step is flagged

#### Scenario: The login form is submitted by the captured input

- **WHEN** the step is triggered by Enter in a password field or a click on the form's submit button, and the field has disappeared by the time it is checked
- **THEN** the field's last known area is masked in the saved screenshot
- **AND** the step is flagged

### Requirement: The password check never delays a screenshot

The screenshot of every step SHALL be taken at the time of its input event, regardless of how long the password check of this or any earlier step takes, as long as fewer than six earlier steps are still being processed. Beyond that the step SHALL wait for one to finish, as when captures queue today, and the wait SHALL be logged; the number of screenshots held in memory SHALL stay bounded.

#### Scenario: A burst of clicks against a slow application

- **WHEN** the user clicks five times within one second in an application whose password check takes the full time budget
- **THEN** each screenshot shows the screen at the moment of its own click

#### Scenario: A long burst against a hanging application

- **WHEN** the user clicks twenty times quickly while every password check runs out of time
- **THEN** no more than six screenshots are held in memory at any moment
- **AND** every click still produces a saved step

#### Scenario: Stopping while checks are running

- **WHEN** the user stops the recording while steps are still being checked
- **THEN** stopping waits until those steps' screenshots and records are written, within the existing timeout

### Requirement: Password masking is on by default and can be switched off

Password masking SHALL be a user setting that is on by default. Switching it off SHALL need no confirmation and SHALL apply from the next recording. While it is off, the setting SHALL state plainly that password fields are captured.

#### Scenario: Default

- **WHEN** a user records without having changed the setting
- **THEN** password fields are masked

#### Scenario: Switched off

- **WHEN** the user turns password masking off and then starts a recording
- **THEN** the setting reads that password fields are captured
- **AND** that recording's steps are saved without masking and without a password check

### Requirement: A step that may show an unchecked password field is flagged

When password masking is on, a step SHALL be saved and flagged if any window visible in its saved area could not be checked completely: the check failed, ran out of time, was refused, was skipped because earlier checks are still hanging, or the window had never been checked before the step. The flag SHALL be stored with the step.

#### Scenario: The check times out

- **WHEN** the password check for a step exceeds its time budget
- **THEN** the step's screenshot is saved with whatever fields were found masked
- **AND** the step is flagged

#### Scenario: A window cannot be inspected

- **WHEN** the captured area contains a window the recorder may not inspect, such as one running with administrator rights
- **THEN** the step is flagged

#### Scenario: A window seen for the first time

- **WHEN** a window appears and is captured before the recorder has checked it once
- **THEN** the step is flagged

#### Scenario: A window checked only once

- **WHEN** a window has been checked once before the step, and that check found no password field
- **THEN** the step is flagged, because a first check may run before the window's contents are ready

#### Scenario: An application keeps hanging

- **WHEN** an application's checks keep running past their budget
- **THEN** after a fixed number of hanging checks, further steps skip the check and are flagged
- **AND** the recording continues

#### Scenario: The check completes

- **WHEN** every window visible in the captured area was checked before and is checked completely in time
- **THEN** the step is not flagged

### Requirement: Uncertain steps must be acknowledged before generation

The review screen SHALL mark every step that is flagged, was captured as a full display in the active window mode, or carries no record of how it was captured. When at least one step is marked, generation SHALL stay disabled until the user ticks a single confirmation that they checked the marked steps.

#### Scenario: A recording with flagged steps

- **WHEN** the review screen shows a recording with two flagged steps
- **THEN** both steps carry a visible mark
- **AND** generate is disabled until the user ticks "Ich habe die markierten Schritte geprüft"

#### Scenario: A recording made before this version

- **WHEN** the review screen shows a recording whose steps carry no record of how they were captured
- **THEN** those steps are marked and the confirmation is required

#### Scenario: Nothing marked

- **WHEN** no step in the recording is marked
- **THEN** no confirmation is shown and generate is enabled as before
