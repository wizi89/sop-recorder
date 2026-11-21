use super::*;

/// This module's executable source: no tests, no comments.
///
/// The guards below assert that certain calls are *absent*, and this file
/// is read whole by `include_str!`. Both exclusions are load-bearing. The
/// tests would otherwise match the very string they assert against, in
/// their own failure messages; and a comment naming the call that was
/// removed -- which is exactly what the code above the change carries, to
/// explain why -- would read as the call still being there.
///
/// Line endings are normalised first: git hands Windows checkouts CRLF.
fn production_source() -> String {
    let source = include_str!("../settings.rs").replace("\r\n", "\n");
    let tests_begin = source.find("\n#[cfg(test)]").expect("no test module");
    source[..tests_begin]
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Opening the settings window must not wait on the OS credential store.
///
/// A source guard rather than a behavioural test: the property is the
/// *absence* of a call, and the keyring has no seam to stub here. Reading
/// the key on this path is what made the window load slowly enough after a
/// reinstall for the pending load to overwrite an edit the user had already
/// made (2026-09-03). Nothing on this path needs the key: the only
/// consumer reads it from the keyring when it generates.
#[test]
fn the_settings_load_path_never_touches_the_credential_store() {
    let source = production_source();
    let start = source
        .find("pub async fn get_settings")
        .expect("get_settings not found");
    let end = source[start..]
        .find("\n#[tauri::command]")
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    let body = &source[start..end];

    assert!(
        !body.contains("keyring"),
        "get_settings must not read the credential store:\n{}",
        body,
    );
}

/// A save has to reach the disk before it reports success, because the
/// settings window closes on that success and the user reads the file as
/// the record of what they chose.
#[test]
fn saving_settings_flushes_the_store_to_disk() {
    let source = production_source();
    let start = source
        .find("pub async fn save_settings")
        .expect("save_settings not found");
    let end = source[start..]
        .find("\n#[tauri::command]")
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    let body = &source[start..end];

    assert!(
        body.contains("store.save()"),
        "save_settings must flush rather than rely on the autosave debounce",
    );
}

/// `logs_dir` has to name the directory the log plugin writes to, on every
/// platform. It used to be derived from `app_local_data_dir()`, which
/// coincides with the log directory on Windows and does not on macOS -- so
/// settings.json named a directory that did not exist, and only the macOS
/// test caught it.
///
/// A source guard: resolving either path for real needs a running Tauri
/// app. What is asserted is that the derivation is the log plugin's own.
#[test]
fn the_reported_log_directory_is_the_one_the_log_plugin_uses() {
    let source = production_source();

    assert!(
        source.contains("app.path().app_log_dir()"),
        "logs_dir must come from app_log_dir(), the log plugin's own target",
    );
    assert!(
        !source.contains("app_local_data_dir()"),
        "app_local_data_dir() is not the log directory on macOS",
    );
}

#[test]
fn a_stale_log_directory_is_corrected() {
    // What the 2026-09-03 test found in settings.json on macOS.
    assert_eq!(
        logs_dir_correction(
            Some("/Users/m/Library/Application Support/com.cogniclone.recorder/logs"),
            "/Users/m/Library/Logs/com.cogniclone.recorder",
        ),
        Some("/Users/m/Library/Logs/com.cogniclone.recorder".to_string()),
    );
}

#[test]
fn a_correct_log_directory_is_left_alone() {
    let actual = "/Users/m/Library/Logs/com.cogniclone.recorder";
    assert_eq!(logs_dir_correction(Some(actual), actual), None);
}

#[test]
fn a_settings_file_with_no_log_directory_gets_one() {
    let actual = "/Users/m/Library/Logs/com.cogniclone.recorder";
    assert_eq!(logs_dir_correction(None, actual), Some(actual.to_string()));
}

/// The BYOK key is no longer round-tripped through `AppSettings`. It used
/// to be loaded on `get_settings` and written back on `save_settings`,
/// which meant a save made while the load had failed -- the window's own
/// `.catch(() => {})` -- cleared the user's key. Nothing writes or clears
/// it now, so it simply survives.
#[test]
fn settings_carry_no_credential() {
    let stored = serde_json::json!({
        "output_dir": "/tmp/out",
        "logs_dir": "/tmp/logs",
        "hide_from_screenshots": true,
        "api_key": "sk-should-be-ignored"
    });
    let settings: AppSettings = serde_json::from_value(stored).unwrap();
    let round_tripped = serde_json::to_value(&settings).unwrap();

    assert!(
        round_tripped.get("api_key").is_none(),
        "a credential must not travel in the settings payload: {}",
        round_tripped,
    );
}

/// A settings file written before this change has no `error_reports` key.
/// It has to load as "ask" -- the default the user would have been given
/// -- rather than failing to parse or silently arriving at `never`.
#[test]
fn settings_from_an_older_build_default_to_ask() {
    let stored = serde_json::json!({
        "output_dir": "/Users/anna/Documents/cogniclone Workflows",
        "logs_dir": "/Users/anna/Library/Logs/com.cogniclone.recorder",
        "hide_from_screenshots": true,
        "api_key": null
    }); // an older build's file; the field is ignored now

    let settings: AppSettings = serde_json::from_value(stored).unwrap();
    assert_eq!(settings.error_reports, "ask");
    assert_eq!(
        crate::error_reports::resolve_mode(Some(&settings.error_reports), false),
        crate::error_reports::ReportMode::Ask
    );
}

#[test]
fn a_saved_mode_survives_the_round_trip() {
    for mode in ["ask", "always", "never"] {
        let stored = serde_json::json!({
            "output_dir": "",
            "logs_dir": "",
            "hide_from_screenshots": true,
            "api_key": null,
            "error_reports": mode
        });
        let settings: AppSettings = serde_json::from_value(stored).unwrap();
        assert_eq!(settings.error_reports, mode);
    }
}

/// The store of an install that predates privacy-capture: neither key.
/// `get_settings` and `start_recording` both read through this.
#[test]
fn a_store_without_the_privacy_keys_reads_as_the_privacy_defaults() {
    assert_eq!(
        read_capture_settings(|_| None),
        CaptureSettings {
            monitor_mode: false,
            mask_passwords: true,
        },
    );
}

#[test]
fn stored_privacy_choices_are_read() {
    let store = serde_json::json!({"capture_mode": "monitor", "mask_passwords": false});
    assert_eq!(
        read_capture_settings(|key| store.get(key).cloned()),
        CaptureSettings {
            monitor_mode: true,
            mask_passwords: false,
        },
    );
}

/// Anything but an explicit `monitor` is the active window: a corrupted
/// value must not widen what is captured.
#[test]
fn an_unexpected_capture_mode_reads_as_the_active_window() {
    let store = serde_json::json!({"capture_mode": "everything", "mask_passwords": "no"});
    assert_eq!(
        read_capture_settings(|key| store.get(key).cloned()),
        CaptureSettings {
            monitor_mode: false,
            mask_passwords: true,
        },
    );
}

/// `save_settings` receives what the settings window sends; a window from
/// before this change sends neither key, and saving must not write
/// anything weaker than the defaults.
#[test]
fn a_save_payload_without_the_privacy_keys_carries_the_defaults() {
    let payload = serde_json::json!({
        "output_dir": "",
        "logs_dir": "",
        "hide_from_screenshots": true
    });
    let settings: AppSettings = serde_json::from_value(payload).unwrap();
    assert_eq!(settings.capture_mode, "active_window");
    assert!(settings.mask_passwords);
}
