//! What the review screen can change before upload (review-redaction D5):
//! the boxes per step, and deleting a step.
//!
//! Every command takes the recording folder and a step number, never a file
//! path, so the frontend can only reach `step_NN.png`, `step_NN.json` and
//! `redactions.json` inside a folder that looks like a recording.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::capture::screenshot::PixelBox;
use crate::output::{redactions, step_meta};

/// The folder must hold `recording.wav` and `screenshots/`. Not restricted to
/// the configured output directory: "generate from folder" opens any folder.
fn screenshots_dir(output_dir: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(output_dir);
    let shots = root.join("screenshots");
    if !root.join("recording.wav").is_file() || !shots.is_dir() {
        return Err(format!("Kein Aufnahme-Ordner: {output_dir}"));
    }
    Ok(shots)
}

fn screenshot_steps(output_dir: &str) -> Result<BTreeSet<u32>, String> {
    Ok(super::recording::session_screenshots(output_dir)?
        .into_iter()
        .map(|(n, _)| n)
        .collect())
}

/// Steps a delete left part way: a box entry or a sidecar whose screenshot is
/// gone. The upload refuses while any exists (D2), so the review screen lists
/// them with a retry.
pub(crate) fn leftover_steps(
    shots: &Path,
    present: &BTreeSet<u32>,
    boxes: Option<&redactions::Boxes>,
) -> Vec<u32> {
    let mut left: BTreeSet<u32> = step_meta::sidecar_orders(shots)
        .into_iter()
        .filter(|n| !present.contains(n))
        .collect();
    if let Some(boxes) = boxes {
        left.extend(boxes.keys().filter(|n| !present.contains(n)));
    }
    left.into_iter().collect()
}

#[derive(serde::Serialize, Debug)]
pub struct LoadedRedactions {
    /// Boxes per step number (JSON object keys are the numbers as strings).
    pub steps: redactions::Boxes,
    pub leftover_steps: Vec<u32>,
    /// Set when `redactions.json` exists but cannot be read; `steps` is then
    /// empty and the review screen offers only the reset.
    pub unreadable: Option<String>,
}

fn load(output_dir: &str) -> Result<LoadedRedactions, String> {
    let shots = screenshots_dir(output_dir)?;
    let present = screenshot_steps(output_dir)?;
    Ok(match redactions::read(&shots) {
        Ok(steps) => LoadedRedactions {
            leftover_steps: leftover_steps(&shots, &present, Some(&steps)),
            steps,
            unreadable: None,
        },
        Err(e) => LoadedRedactions {
            leftover_steps: leftover_steps(&shots, &present, None),
            steps: redactions::Boxes::new(),
            unreadable: Some(e.to_string()),
        },
    })
}

#[tauri::command]
pub async fn load_redactions(output_dir: String) -> Result<LoadedRedactions, String> {
    load(&output_dir)
}

/// Replace one step's boxes; returns what is now stored for it.
#[tauri::command]
pub async fn save_redactions(
    output_dir: String,
    step: u32,
    boxes: Vec<PixelBox>,
) -> Result<Vec<PixelBox>, String> {
    let shots = screenshots_dir(&output_dir)?;
    redactions::save_step(&shots, step, boxes)
}

/// Delete `redactions.json`, readable or not, after the user confirmed.
#[tauri::command]
pub async fn reset_redactions(output_dir: String) -> Result<(), String> {
    let shots = screenshots_dir(&output_dir)?;
    redactions::reset(&shots)
}

fn delete(output_dir: &str, step: u32) -> Result<(), String> {
    let shots = screenshots_dir(output_dir)?;
    // PNG first, entry last: a failure part way never leaves the PNG in the
    // upload without its boxes, and the leftover entry blocks the upload until
    // the delete is retried.
    // Every PNG of that step: `step_1.png` and `step_01.png` are both step 1.
    let pngs = super::recording::session_screenshots(output_dir)?
        .into_iter()
        .filter(|(n, _)| *n == step);
    for (_, png) in pngs {
        match std::fs::remove_file(&png) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Schritt {step} konnte nicht gelöscht werden: {e}")),
        }
    }
    step_meta::delete_sidecar(&shots, step)
        .map_err(|e| format!("Schritt {step}: Metadaten konnten nicht gelöscht werden: {e}"))?;
    redactions::remove_step(&shots, step)
        .map_err(|e| format!("Schritt {step}: Schwärzungen konnten nicht entfernt werden: {e}"))?;
    log::info!("review: step {step} deleted in {}", shots.display());
    Ok(())
}

/// Remove a step's screenshot, sidecar and boxes. Idempotent: an already
/// missing part counts as removed, so a retry completes a partial delete.
#[tauri::command]
pub async fn delete_step(output_dir: String, step: u32) -> Result<(), String> {
    delete(&output_dir, step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn recording(steps: &[u32]) -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("recording.wav"), b"wav").unwrap();
        let shots = dir.path().join("screenshots");
        std::fs::create_dir(&shots).unwrap();
        for n in steps {
            std::fs::write(shots.join(format!("step_{n:02}.png")), b"png").unwrap();
            std::fs::write(shots.join(format!("step_{n:02}.json")), b"{}").unwrap();
        }
        dir
    }

    fn s(dir: &tempfile::TempDir) -> String {
        dir.path().to_string_lossy().to_string()
    }

    #[tokio::test]
    async fn a_folder_that_is_not_a_recording_is_refused() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("screenshots")).unwrap();
        assert!(load_redactions(s(&dir)).await.is_err());
        assert!(save_redactions(s(&dir), 1, vec![[0, 0, 1, 1]])
            .await
            .is_err());
        assert!(reset_redactions(s(&dir)).await.is_err());
        assert!(delete_step(s(&dir), 1).await.is_err());

        let no_shots = tempdir().unwrap();
        std::fs::write(no_shots.path().join("recording.wav"), b"wav").unwrap();
        assert!(load_redactions(s(&no_shots)).await.is_err());
    }

    /// Any folder with the recording shape is accepted, wherever it lives:
    /// "generate from folder" opens folders outside the output directory.
    #[tokio::test]
    async fn a_recording_folder_anywhere_is_accepted() {
        let dir = recording(&[1]);
        save_redactions(s(&dir), 1, vec![[0, 0, 2, 2]])
            .await
            .unwrap();
        let loaded = load_redactions(s(&dir)).await.unwrap();
        assert_eq!(loaded.steps[&1], vec![[0, 0, 2, 2]]);
        assert!(loaded.leftover_steps.is_empty());
        assert!(loaded.unreadable.is_none());
    }

    #[tokio::test]
    async fn an_unreadable_file_is_reported_not_overwritten() {
        let dir = recording(&[1]);
        let file = dir.path().join("screenshots").join(redactions::FILE_NAME);
        std::fs::write(&file, "garbage").unwrap();
        let loaded = load_redactions(s(&dir)).await.unwrap();
        assert!(loaded.unreadable.is_some());
        assert!(save_redactions(s(&dir), 1, vec![[0, 0, 1, 1]])
            .await
            .is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "garbage");
        reset_redactions(s(&dir)).await.unwrap();
        assert!(load_redactions(s(&dir)).await.unwrap().unreadable.is_none());
    }

    #[tokio::test]
    async fn delete_removes_only_that_steps_files_and_boxes() {
        let dir = recording(&[1, 2, 3]);
        save_redactions(s(&dir), 2, vec![[0, 0, 1, 1]])
            .await
            .unwrap();
        save_redactions(s(&dir), 3, vec![[0, 0, 1, 1]])
            .await
            .unwrap();
        delete_step(s(&dir), 2).await.unwrap();
        let shots = dir.path().join("screenshots");
        assert!(!shots.join("step_02.png").exists());
        assert!(!shots.join("step_02.json").exists());
        assert!(shots.join("step_01.png").exists() && shots.join("step_03.json").exists());
        let loaded = load_redactions(s(&dir)).await.unwrap();
        assert_eq!(loaded.steps.keys().copied().collect::<Vec<_>>(), vec![3]);
        assert!(loaded.leftover_steps.is_empty());
        // Idempotent.
        delete_step(s(&dir), 2).await.unwrap();
    }

    /// A delete that removed the PNG but not the entry (the file was
    /// unreadable at the time) leaves a leftover; once the file is fixed the
    /// retry completes.
    #[tokio::test]
    async fn a_partial_delete_is_reported_and_a_retry_completes() {
        let dir = recording(&[1, 2]);
        save_redactions(s(&dir), 2, vec![[0, 0, 1, 1]])
            .await
            .unwrap();
        let file = dir.path().join("screenshots").join(redactions::FILE_NAME);
        let good = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, "garbage").unwrap();
        assert!(delete_step(s(&dir), 2).await.is_err());
        assert!(!dir.path().join("screenshots/step_02.png").exists());

        std::fs::write(&file, good).unwrap();
        let loaded = load_redactions(s(&dir)).await.unwrap();
        assert_eq!(loaded.leftover_steps, vec![2]);

        delete_step(s(&dir), 2).await.unwrap();
        let loaded = load_redactions(s(&dir)).await.unwrap();
        assert!(loaded.leftover_steps.is_empty());
        assert!(loaded.steps.is_empty());
    }

    /// `step_1.png` and `step_01.png` are both step 1; deleting the step
    /// leaves neither in the upload (#59).
    #[tokio::test]
    async fn delete_removes_every_screenshot_of_that_step() {
        let dir = recording(&[1, 2]);
        let shots = dir.path().join("screenshots");
        std::fs::write(shots.join("step_1.png"), b"png").unwrap();
        delete_step(s(&dir), 1).await.unwrap();
        assert!(!shots.join("step_01.png").exists());
        assert!(!shots.join("step_1.png").exists());
        assert!(shots.join("step_02.png").exists());
    }

    /// The screenshot cannot be removed: the delete fails and the step keeps
    /// its sidecar and boxes. A non-empty directory in place of the PNG makes
    /// the removal fail on every OS, root included (#59).
    #[tokio::test]
    async fn a_screenshot_that_cannot_be_removed_keeps_the_step() {
        let dir = recording(&[1]);
        save_redactions(s(&dir), 2, vec![[0, 0, 1, 1]])
            .await
            .unwrap();
        let shots = dir.path().join("screenshots");
        std::fs::create_dir(shots.join("step_02.png")).unwrap();
        std::fs::write(shots.join("step_02.png").join("keep"), b"x").unwrap();
        std::fs::write(shots.join("step_02.json"), b"{}").unwrap();
        assert!(delete_step(s(&dir), 2).await.is_err());
        assert!(shots.join("step_02.json").exists());
        let loaded = load_redactions(s(&dir)).await.unwrap();
        assert_eq!(loaded.steps.keys().copied().collect::<Vec<_>>(), vec![2]);
    }

    #[tokio::test]
    async fn a_sidecar_without_its_screenshot_is_a_leftover() {
        let dir = recording(&[1, 2]);
        std::fs::remove_file(dir.path().join("screenshots/step_02.png")).unwrap();
        let loaded = load_redactions(s(&dir)).await.unwrap();
        assert_eq!(loaded.leftover_steps, vec![2]);
    }
}
