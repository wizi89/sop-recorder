use super::*;

// One test per rule, each fed a log line shape that exists in the codebase
// today.

fn scrubber() -> Scrubber {
    Scrubber::new(
        Some(PathBuf::from("/Users/anna")),
        Some("/Users/anna/Documents/cogniclone Workflows".to_string()),
    )
}

#[test]
fn recording_directory_loses_the_guide_title() {
    // commands/recording.rs:239
    let line = "[2026-09-03][12:00:01][INFO] Recording started: \
/Users/anna/Documents/cogniclone Workflows/Urlaubsantrag stellen 2026-09-03 12-00-00";
    let out = scrubber().scrub(line);
    assert!(!out.contains("Urlaubsantrag"));
    assert!(!out.contains("/Users/anna"));
    assert!(out.contains("<Anleitungsverzeichnis>/<Aufnahme>"));
}

#[test]
fn home_directory_outside_the_output_dir_becomes_a_tilde() {
    // capture/audio.rs:198, when the output dir has been moved elsewhere.
    let line = "Audio recording started: /Users/anna/Library/Caches/rec.wav";
    let out = scrubber().scrub(line);
    assert_eq!(out, "Audio recording started: ~/Library/Caches/rec.wav");
}

#[test]
fn an_email_address_becomes_a_placeholder() {
    let line = "Anmeldung fehlgeschlagen für anna.mueller@kunde-gmbh.de";
    let out = scrubber().scrub(line);
    assert!(!out.contains("anna.mueller"));
    assert!(out.contains("<E-Mail>"));
}

#[test]
fn a_bearer_token_is_removed() {
    let line = "GET /quota Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.abc";
    let out = scrubber().scrub(line);
    assert!(!out.contains("eyJhbGci"));
    assert!(out.contains("Bearer <entfernt>"));
}

#[test]
fn token_values_in_an_upload_error_body_are_removed() {
    // network/upload.rs:110
    let line = "Upload failed: 401 Unauthorized - \
{\"access_token\":\"eyJhbGciOi.secret\",\"refresh_token\": \"rt_9f3c\",\"error\":\"expired\"}";
    let out = scrubber().scrub(line);
    assert!(!out.contains("eyJhbGciOi.secret"));
    assert!(!out.contains("rt_9f3c"));
    assert!(out.contains("\"access_token\":\"<entfernt>\""));
    assert!(out.contains("\"refresh_token\": \"<entfernt>\""));
    // The status line and the server's error code are the diagnosis.
    assert!(out.contains("401 Unauthorized"));
    assert!(out.contains("\"error\":\"expired\""));
}

#[test]
fn an_api_key_value_is_removed() {
    let line = "settings saved {\"api_key\":\"sk-proj-abcdef123456\"}";
    let out = scrubber().scrub(line);
    assert!(!out.contains("sk-proj"));
    assert!(out.contains("\"api_key\":\"<entfernt>\""));
}

#[test]
fn cursor_coordinates_survive_scrubbing() {
    let line = "[2026-09-03][12:00:02][INFO] Click at (1284, 733) on display 1";
    assert_eq!(scrubber().scrub(line), line);
}

#[test]
fn scrubbing_without_a_configured_output_dir_still_removes_the_home() {
    let s = Scrubber::new(Some(PathBuf::from("/Users/anna")), None);
    assert_eq!(s.scrub("/Users/anna/x.log"), "~/x.log");
}
