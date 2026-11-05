# Spec Delta

## ADDED Requirements

### Requirement: Boxes are applied to every uploaded screenshot

Every upload SHALL send each screenshot with all of its stored boxes applied as opaque fills, regardless of which path started the upload.

#### Scenario: Generate from the review screen

- **WHEN** the user generates a recording with boxes on two steps
- **THEN** the uploaded images of those steps are opaque inside the boxes and unchanged elsewhere

#### Scenario: Unfinished recording after a restart

- **WHEN** an unfinished recording with boxes is uploaded after the app restarted
- **THEN** the uploaded images carry the boxes

#### Scenario: Retry after a failed upload

- **WHEN** an upload is retried
- **THEN** the retried images carry the boxes

### Requirement: An upload whose boxes cannot be applied does not happen

If the stored boxes for a recording cannot be read, or a box cannot be applied to its screenshot, the upload SHALL NOT start, and the user SHALL be told which step failed. Missing box storage means no boxes were drawn.

#### Scenario: Unreadable box storage

- **WHEN** the recording's box storage exists but cannot be parsed
- **THEN** nothing is uploaded and the user sees an error

#### Scenario: Boxes stored for a step without a screenshot

- **WHEN** the box storage holds boxes for a step number that has no screenshot
- **THEN** nothing is uploaded and the error names the step

#### Scenario: Box outside the image

- **WHEN** a stored box lies partly outside its screenshot
- **THEN** the part inside the image is filled and the upload proceeds

#### Scenario: Screenshot cannot be decoded

- **WHEN** a step with boxes has a screenshot that cannot be decoded
- **THEN** nothing is uploaded and the error names the step

#### Scenario: No boxes drawn

- **WHEN** no box was drawn in the recording
- **THEN** the screenshots are uploaded byte-identical to the files on disk
