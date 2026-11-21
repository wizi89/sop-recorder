use super::*;
use tempfile::tempdir;

fn touch(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), b"not really a png").unwrap();
}

fn steps(dir: &Path) -> Vec<u32> {
    collect_step_screenshots(dir)
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect()
}

/// The 2026-09-03 regression. A capture failed at step 2; the counting loop
/// this replaces stopped there and shipped one screenshot out of three.
#[test]
fn a_gap_does_not_end_the_sequence() {
    let dir = tempdir().unwrap();
    for name in ["step_01.png", "step_03.png", "step_04.png"] {
        touch(dir.path(), name);
    }

    assert_eq!(steps(dir.path()), vec![1, 3, 4]);
}

/// Lexicographic order puts step 10 before step 9. Sorting on the parsed
/// number is what keeps a recording longer than nine steps in order.
#[test]
fn steps_are_ordered_numerically_not_lexicographically() {
    let dir = tempdir().unwrap();
    for step in 1..=21u32 {
        touch(dir.path(), &format!("step_{:02}.png", step));
    }

    assert_eq!(steps(dir.path()), (1..=21).collect::<Vec<_>>());
}

#[test]
fn unrelated_files_are_ignored() {
    let dir = tempdir().unwrap();
    touch(dir.path(), "step_01.png");
    for name in [
        "step_01.json",
        "step_.png",
        "step_2a.png",
        "notes.txt",
        "step_01.png.bak",
    ] {
        touch(dir.path(), name);
    }

    assert_eq!(steps(dir.path()), vec![1]);
}

#[test]
fn an_empty_folder_yields_no_steps() {
    let dir = tempdir().unwrap();
    assert!(collect_step_screenshots(dir.path()).unwrap().is_empty());
}

/// A folder that is not there at all is an error rather than an empty
/// result, so the caller can name it instead of reporting "no screenshots"
/// for a recording that was never written.
#[test]
fn a_missing_folder_is_an_error_naming_it() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("screenshots");

    let err = collect_step_screenshots(&missing).unwrap_err();
    assert!(err.contains("screenshots"), "{}", err);
}

#[test]
fn step_numbers_past_ninety_nine_still_parse() {
    assert_eq!(step_number_from_filename("step_100.png"), Some(100));
    assert_eq!(step_number_from_filename("step_01.png"), Some(1));
    assert_eq!(step_number_from_filename("step_1.png"), Some(1));
    assert_eq!(step_number_from_filename("step_01.json"), None);
    assert_eq!(step_number_from_filename("step_.png"), None);
    assert_eq!(step_number_from_filename("recording.wav"), None);
}

fn png(dir: &Path, step: u32, w: u32, h: u32) -> PathBuf {
    let path = dir.join(format!("step_{:02}.png", step));
    image::RgbaImage::from_pixel(w, h, image::Rgba([200, 210, 220, 255]))
        .save(&path)
        .unwrap();
    path
}

fn upload_list(dir: &Path) -> Result<Vec<upload::UploadScreenshot>, String> {
    build_upload_screenshots(dir, &collect_step_screenshots(dir).unwrap())
}

fn decoded(bytes: &[u8]) -> image::RgbaImage {
    image::load_from_memory(bytes).unwrap().to_rgba8()
}

const BLACK: image::Rgba<u8> = image::Rgba([0, 0, 0, 255]);

#[test]
fn no_boxes_uploads_the_files_byte_identical() {
    let dir = tempdir().unwrap();
    let paths = [png(dir.path(), 1, 8, 8), png(dir.path(), 2, 8, 8)];
    let list = upload_list(dir.path()).unwrap();
    for (shot, path) in list.iter().zip(&paths) {
        assert_eq!(shot.bytes, std::fs::read(path).unwrap());
    }
}

#[test]
fn boxes_are_opaque_inside_and_the_rest_is_unchanged() {
    let dir = tempdir().unwrap();
    let path = png(dir.path(), 1, 10, 10);
    let before = std::fs::read(&path).unwrap();
    crate::output::redactions::save_step(dir.path(), 1, vec![[2, 2, 3, 3], [8, 8, 5, 5]]).unwrap();
    let list = upload_list(dir.path()).unwrap();
    let img = decoded(&list[0].bytes);
    for (x, y, p) in img.enumerate_pixels() {
        let inside = ((2..5).contains(&x) && (2..5).contains(&y)) || (x >= 8 && y >= 8);
        assert_eq!(*p == BLACK, inside, "pixel {x},{y}");
    }
    // The original on disk is untouched.
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

/// The list is built once; every attempt sends it. Even with the files
/// gone from disk afterwards, it still holds the burned bytes.
#[test]
fn the_upload_list_holds_the_burned_bytes_independent_of_the_file() {
    let dir = tempdir().unwrap();
    let path = png(dir.path(), 1, 4, 4);
    crate::output::redactions::save_step(dir.path(), 1, vec![[0, 0, 4, 4]]).unwrap();
    let list = upload_list(dir.path()).unwrap();
    let first = list[0].bytes.clone();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(list[0].bytes, first);
    assert!(decoded(&first).pixels().all(|p| *p == BLACK));
}

#[test]
fn corrupt_boxes_upload_nothing() {
    let dir = tempdir().unwrap();
    png(dir.path(), 1, 4, 4);
    std::fs::write(dir.path().join("redactions.json"), "{").unwrap();
    let err = upload_list(dir.path()).unwrap_err();
    assert!(err.starts_with(REDACTION_REFUSAL), "{err}");
}

#[test]
fn a_box_entry_without_a_screenshot_uploads_nothing_and_names_the_step() {
    let dir = tempdir().unwrap();
    png(dir.path(), 1, 4, 4);
    crate::output::redactions::save_step(dir.path(), 7, vec![[0, 0, 1, 1]]).unwrap();
    let err = upload_list(dir.path()).unwrap_err();
    assert!(
        err.starts_with(REDACTION_REFUSAL) && err.contains("Schritt 7"),
        "{err}"
    );
}

#[test]
fn a_sidecar_without_a_screenshot_uploads_nothing() {
    let dir = tempdir().unwrap();
    png(dir.path(), 1, 4, 4);
    std::fs::write(dir.path().join("step_02.json"), "{}").unwrap();
    let err = upload_list(dir.path()).unwrap_err();
    assert!(err.contains("Schritt 2"), "{err}");
}

#[test]
fn an_undecodable_screenshot_with_boxes_uploads_nothing_and_names_the_step() {
    let dir = tempdir().unwrap();
    png(dir.path(), 1, 4, 4);
    touch(dir.path(), "step_02.png");
    crate::output::redactions::save_step(dir.path(), 2, vec![[0, 0, 1, 1]]).unwrap();
    let err = upload_list(dir.path()).unwrap_err();
    assert!(
        err.starts_with(REDACTION_REFUSAL) && err.contains("Schritt 2"),
        "{err}"
    );
}

#[test]
fn a_box_partly_outside_is_clipped_and_the_upload_proceeds() {
    let dir = tempdir().unwrap();
    png(dir.path(), 1, 4, 4);
    crate::output::redactions::save_step(dir.path(), 1, vec![[2, 2, 50, 50]]).unwrap();
    let img = decoded(&upload_list(dir.path()).unwrap()[0].bytes);
    assert_eq!(img.pixels().filter(|p| **p == BLACK).count(), 4);
}

#[test]
fn boxes_on_a_recording_with_a_gap_land_on_the_right_step() {
    let dir = tempdir().unwrap();
    for n in [1, 3, 4] {
        png(dir.path(), n, 4, 4);
    }
    crate::output::redactions::save_step(dir.path(), 3, vec![[0, 0, 4, 4]]).unwrap();
    let list = upload_list(dir.path()).unwrap();
    assert_eq!(
        list.iter().map(|s| s.order).collect::<Vec<_>>(),
        vec![1, 3, 4]
    );
    let black = |i: usize| decoded(&list[i].bytes).pixels().all(|p| *p == BLACK);
    assert!(!black(0) && black(1) && !black(2));
}

fn meta(order: u32) -> step_meta::StepMeta {
    step_meta::StepMeta {
        order,
        timestamp_seconds: order as f64,
        click_x: None,
        click_y: None,
        trigger: "enter_key".into(),
        marker_box: None,
        ..Default::default()
    }
}

#[test]
fn metadata_travels_when_the_counts_agree() {
    let metas = vec![meta(1), meta(2)];
    assert!(steps_for_upload(&metas, 2).is_some());
}

#[test]
fn metadata_is_dropped_when_the_counts_disagree() {
    let metas = vec![meta(1)];
    assert!(steps_for_upload(&metas, 20).is_none());
    assert!(steps_for_upload(&metas, 0).is_none());
}

#[test]
fn no_sidecars_sends_no_metadata() {
    assert!(steps_for_upload(&[], 3).is_none());
    assert!(steps_for_upload(&[], 0).is_none());
}

/// The end-to-end property the server's positional pairing rests on: after
/// a failed capture, the screenshots and the sidecars still describe the
/// same steps, in the same order, at the same length -- so alignment
/// survives rather than being dropped for the whole recording.
#[test]
fn a_failed_capture_keeps_its_alignment() {
    let dir = tempdir().unwrap();
    // Step 2's capture failed: no PNG, and so no sidecar either.
    for step in [1u32, 3, 4] {
        touch(dir.path(), &format!("step_{:02}.png", step));
        crate::output::step_meta::write_sidecar(dir.path(), &meta(step)).unwrap();
    }

    let screenshots = collect_step_screenshots(dir.path()).unwrap();
    let metas = crate::output::step_meta::read_all(dir.path());

    assert_eq!(screenshots.len(), metas.len());
    assert_eq!(
        screenshots.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        metas.iter().map(|m| m.order).collect::<Vec<_>>(),
        "screenshot and sidecar order must match position for position",
    );
    assert!(steps_for_upload(&metas, screenshots.len()).is_some());
}
