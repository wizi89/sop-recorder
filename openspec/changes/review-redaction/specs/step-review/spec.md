# Spec Delta

## Purpose

What the user can do with a recording's captured steps between stopping and uploading: black out areas of a screenshot and delete steps, so nothing they did not mean to share is sent.

## ADDED Requirements

### Requirement: The user can black out an area of a screenshot

The review screen SHALL let the user draw a rectangle on any screenshot that is shown as an opaque box. The box SHALL cover the area in the uploaded image exactly as shown on the review screen, at any display scale.

#### Scenario: Drawing a box

- **WHEN** the user drags across part of a screenshot on the review screen
- **THEN** that area is shown fully opaque once the box is stored
- **AND** the step keeps its place and number

#### Scenario: Several boxes on one step

- **WHEN** the user draws three boxes on one screenshot
- **THEN** all three are shown and all three are applied at upload

#### Scenario: Box at a fractional display scale

- **WHEN** the screenshot is shown at a scale where the box edges fall between image pixels
- **THEN** the uploaded box covers at least every image pixel the shown box covers

#### Scenario: Recording with a gap in its step numbers

- **WHEN** a recording has screenshots for steps 1, 3 and 4 and the user draws a box on the step labelled 3
- **THEN** the box is applied to step 3's screenshot at upload

### Requirement: A box is shown as applied only when it is stored

The review screen SHALL show a box as applied only after it has been stored, and SHALL NOT allow generation while a box change is still being stored or failed to be stored.

#### Scenario: Saving fails

- **WHEN** storing a box fails
- **THEN** the box is not shown as applied, the user sees an error for that step
- **AND** generation is not possible until the save succeeds or the box is removed

#### Scenario: Generate right after drawing

- **WHEN** the user draws a box and its save has not finished
- **THEN** generation is not possible until it has

### Requirement: Unreadable box storage is reported, never overwritten

If a recording's stored boxes cannot be read, the review screen SHALL say so and SHALL NOT allow drawing or generation until the user resets the boxes; storing a new box SHALL NOT overwrite the unreadable storage.

#### Scenario: Opening a recording with corrupt box storage

- **WHEN** the review screen opens a recording whose box storage cannot be parsed
- **THEN** the user sees an error with the option to reset the boxes
- **AND** generation is not possible until the boxes are reset

### Requirement: A box can be removed until upload

The user SHALL be able to remove any box they drew, at any time before the upload starts.

#### Scenario: Removing a box

- **WHEN** the user removes a box they drew
- **THEN** the area is shown unmasked again and is not masked in the upload

### Requirement: Boxes survive a restart

A box SHALL be stored as soon as it is drawn or removed, so closing or crashing the app does not lose it.

#### Scenario: Crash after drawing

- **WHEN** the user draws a box and the app then closes unexpectedly
- **THEN** after restart, the recording offered as unfinished shows the box
- **AND** uploading it applies the box

### Requirement: The original screenshot on disk is kept

Drawing a box SHALL NOT modify the screenshot file on disk; the box is applied to the uploaded bytes only.

#### Scenario: File after drawing

- **WHEN** the user has drawn boxes on a step
- **THEN** the step's screenshot file on disk is byte-identical to the one captured

### Requirement: The user can delete a step

The review screen SHALL let the user delete a step. Its screenshot and per-step metadata SHALL be removed from disk, the remaining steps SHALL keep their numbers, and the recording's audio SHALL remain unchanged.

#### Scenario: Deleting a middle step

- **WHEN** a recording has steps 1 to 5 and the user deletes step 3
- **THEN** steps 1, 2, 4 and 5 remain with those numbers on the review screen and are uploaded in that order
- **AND** step 3's screenshot, per-step metadata and boxes no longer exist

#### Scenario: Deleting fails part way

- **WHEN** a step's screenshot is removed but removing the rest of its data fails
- **THEN** the user sees an error and generation is refused until the delete is retried successfully
- **AND** the review screen offers that retry, also after a restart

#### Scenario: Deleting the screenshot fails

- **WHEN** the step's screenshot cannot be removed
- **THEN** the step stays on the review screen with its boxes and the user sees an error

#### Scenario: Deleting the last remaining step

- **WHEN** the user deletes the only remaining step
- **THEN** generation is not offered, as for a recording without screenshots

#### Scenario: Deleting a flagged step

- **WHEN** the only flagged step is deleted
- **THEN** the confirmation for flagged steps is no longer required
