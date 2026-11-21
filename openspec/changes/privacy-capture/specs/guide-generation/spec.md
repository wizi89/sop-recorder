# Spec Delta

## ADDED Requirements

### Requirement: The upload states how the recording was protected

Every upload SHALL carry, counted over its steps, how each step was captured (window, display, display as fallback, or unknown) and the outcome of each step's password check (checked, flagged, off, or unknown), and how many steps were captured from a partial window list, counted apart from the password check and with password masking on or off. The counts SHALL come from what was recorded with each step, never from current settings or defaults. The server SHALL NOT change generation because of them.

#### Scenario: Default settings

- **WHEN** a recording of five steps made with default settings on Windows is uploaded, one step flagged
- **THEN** the metadata reports five steps captured as window, four checked and one flagged

#### Scenario: Full display, masking off

- **WHEN** a recording made in monitor mode with password masking off is uploaded
- **THEN** every step is reported as captured as display with the password check off

#### Scenario: A partial window list

- **WHEN** a recording of three steps made with password masking off is uploaded, two of them captured from a partial window list
- **THEN** the metadata reports two steps from a partial window list and three with the password check off

#### Scenario: Regenerating a folder

- **WHEN** a recording is generated a second time from its folder
- **THEN** the metadata reports the same counts as the first time

#### Scenario: A recording from an earlier version

- **WHEN** a folder recorded before this version is uploaded
- **THEN** its steps are reported as unknown for both values
