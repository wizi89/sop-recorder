## Purpose

Loading, editing and persisting the user's settings so that what the settings window shows, what the user changes, and what the settings file contains never disagree with one another or with the running app.

## ADDED Requirements

### Requirement: The settings form never discards a user edit

The settings window SHALL NOT overwrite a value the user has changed with a value arriving from an asynchronous load. Until the stored settings have loaded, the form SHALL be visibly unavailable for editing rather than showing defaults the user might act on.

#### Scenario: The user edits before the load resolves

- **WHEN** the user changes a setting and the pending load of stored settings resolves afterwards
- **THEN** the user's change is preserved
- **AND** saving writes the user's change

#### Scenario: The form before settings have loaded

- **WHEN** the settings window is open and the stored settings have not yet loaded
- **THEN** the controls and the save action are disabled
- **AND** the window indicates that settings are loading

#### Scenario: Server-supplied options arrive late

- **WHEN** the list of available generation models arrives after the settings have loaded
- **THEN** only the model selection is reconciled against that list; no other setting is replaced

### Requirement: Opening settings does not depend on the credential store

Loading settings for display SHALL NOT read the operating system credential store. Whether a key is stored SHALL be obtainable without retrieving it.

#### Scenario: The credential store is slow or prompts

- **WHEN** the operating system delays or prompts on credential access
- **THEN** the settings window still loads and becomes editable
- **AND** it still reports correctly whether an API key is stored

### Requirement: A save is durable and its outcome is honest

Saving settings SHALL write them to disk before reporting success. A failed save SHALL be reported to the user and SHALL NOT be presented as a completed save.

#### Scenario: A successful save

- **WHEN** the user saves a changed setting
- **THEN** the settings file on disk contains the new value once the save reports success
- **AND** the settings window closes

#### Scenario: The save fails

- **WHEN** writing the settings fails
- **THEN** the settings window stays open and shows what went wrong
- **AND** the window is not closed

#### Scenario: The app exits immediately after a save

- **WHEN** the app is terminated right after a save reports success
- **THEN** the saved value survives the restart

### Requirement: The reported log directory is the one used

The log directory shown in settings and stored in the settings file SHALL be the directory the application actually writes log files to, on every supported platform. Because the location is not user-selectable, it SHALL be presented as information rather than as an editable field.

#### Scenario: The path is shown

- **WHEN** the user opens settings
- **THEN** the log directory shown is the directory containing the application's log files
- **AND** the field cannot be edited

#### Scenario: Opening the log directory

- **WHEN** the user chooses to reveal the log directory
- **THEN** the operating system's file browser opens at that directory

#### Scenario: A stale path from an earlier version

- **WHEN** the settings file holds a log directory written by an earlier version that does not match the real one
- **THEN** it is corrected at startup, so the file and the application agree
