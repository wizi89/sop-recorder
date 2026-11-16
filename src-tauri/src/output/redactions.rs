//! The boxes the user drew on the review screen (review-redaction D1).
//!
//! One file per recording, `screenshots/redactions.json`:
//! `{ "version": 1, "steps": { "3": [[x, y, w, h], ...] } }`. Keys are the
//! number in the screenshot's file name, coordinates are saved-image pixels.
//! The PNGs on disk are never touched; the boxes are burned into the bytes at
//! upload (`commands::generate`).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::capture::screenshot::PixelBox;

pub const FILE_NAME: &str = "redactions.json";
const VERSION: u32 = 1;

/// Boxes per step number. Missing file = empty map.
pub type Boxes = BTreeMap<u32, Vec<PixelBox>>;

/// Every write to any `redactions.json` holds this (design D5). One global
/// lock rather than one per path, so two spellings of the same folder cannot
/// get two different locks.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    steps: BTreeMap<String, Vec<PixelBox>>,
}

/// Why the stored boxes could not be read. Distinct from an I/O error on a
/// write, because the review screen offers a reset only for this one.
#[derive(Debug, PartialEq)]
pub struct Unreadable(pub String);

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ist nicht lesbar: {}", FILE_NAME, self.0)
    }
}

/// The stored boxes. A missing file means none were drawn; a file that is
/// present but unparsable, has another `version`, a non-numeric step key or a
/// box with no area is unreadable, and nothing may be uploaded on it.
pub fn read(screenshots_dir: &Path) -> Result<Boxes, Unreadable> {
    let path = screenshots_dir.join(FILE_NAME);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Boxes::new()),
        Err(e) => return Err(Unreadable(e.to_string())),
    };
    let stored: Stored = serde_json::from_str(&content).map_err(|e| Unreadable(e.to_string()))?;
    if stored.version != VERSION {
        return Err(Unreadable(format!("unbekannte Version {}", stored.version)));
    }
    let mut boxes = Boxes::new();
    for (key, list) in stored.steps {
        let step: u32 = key
            .parse()
            .map_err(|_| Unreadable(format!("ungültiger Schritt {key:?}")))?;
        if let Some(b) = list.iter().find(|b| b[2] <= 0 || b[3] <= 0) {
            return Err(Unreadable(format!("leere Box {b:?} in Schritt {step}")));
        }
        if !list.is_empty() {
            boxes.insert(step, list);
        }
    }
    Ok(boxes)
}

/// Write the whole map through a uniquely named temp file in the same folder
/// and a rename, so a crash never leaves half a file.
fn write(screenshots_dir: &Path, boxes: &Boxes) -> Result<(), String> {
    let stored = Stored {
        version: VERSION,
        steps: boxes
            .iter()
            .filter(|(_, list)| !list.is_empty())
            .map(|(step, list)| (step.to_string(), list.clone()))
            .collect(),
    };
    let json = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".redactions-")
        .suffix(".tmp")
        .tempfile_in(screenshots_dir)
        .map_err(|e| format!("{FILE_NAME} konnte nicht geschrieben werden: {e}"))?;
    tmp.write_all(json.as_bytes())
        .and_then(|_| tmp.as_file().sync_all())
        .map_err(|e| format!("{FILE_NAME} konnte nicht geschrieben werden: {e}"))?;
    tmp.persist(screenshots_dir.join(FILE_NAME))
        .map_err(|e| format!("{FILE_NAME} konnte nicht geschrieben werden: {}", e.error))?;
    Ok(())
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Replace one step's boxes (an empty list removes the entry) and return what
/// is now stored for it. Refuses to overwrite an unreadable file.
pub fn save_step(
    screenshots_dir: &Path,
    step: u32,
    list: Vec<PixelBox>,
) -> Result<Vec<PixelBox>, String> {
    if let Some(b) = list.iter().find(|b| b[2] <= 0 || b[3] <= 0) {
        return Err(format!("Leere Box {b:?} für Schritt {step}"));
    }
    let _guard = lock();
    let mut boxes = read(screenshots_dir).map_err(|e| e.to_string())?;
    if list.is_empty() {
        boxes.remove(&step);
    } else {
        boxes.insert(step, list);
    }
    write(screenshots_dir, &boxes)?;
    Ok(boxes.get(&step).cloned().unwrap_or_default())
}

/// Drop one step's entry, if any. A missing file or entry counts as done.
pub fn remove_step(screenshots_dir: &Path, step: u32) -> Result<(), String> {
    let _guard = lock();
    if !screenshots_dir.join(FILE_NAME).exists() {
        return Ok(());
    }
    let mut boxes = read(screenshots_dir).map_err(|e| e.to_string())?;
    if boxes.remove(&step).is_some() {
        write(screenshots_dir, &boxes)?;
    }
    Ok(())
}

/// Delete the file, readable or not. Missing counts as done.
pub fn reset(screenshots_dir: &Path) -> Result<(), String> {
    let _guard = lock();
    match std::fs::remove_file(screenshots_dir.join(FILE_NAME)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{FILE_NAME} konnte nicht gelöscht werden: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn a_missing_file_means_no_boxes() {
        let dir = tempdir().unwrap();
        assert_eq!(read(dir.path()).unwrap(), Boxes::new());
    }

    #[test]
    fn boxes_round_trip() {
        let dir = tempdir().unwrap();
        save_step(dir.path(), 3, vec![[1, 2, 3, 4], [5, 6, 7, 8]]).unwrap();
        save_step(dir.path(), 10, vec![[0, 0, 1, 1]]).unwrap();
        let boxes = read(dir.path()).unwrap();
        assert_eq!(boxes[&3], vec![[1, 2, 3, 4], [5, 6, 7, 8]]);
        assert_eq!(boxes[&10], vec![[0, 0, 1, 1]]);
        let raw = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(raw.contains("\"version\": 1"), "{raw}");
        assert!(raw.contains("\"3\""), "{raw}");
    }

    #[test]
    fn an_empty_list_removes_the_entry() {
        let dir = tempdir().unwrap();
        save_step(dir.path(), 3, vec![[1, 2, 3, 4]]).unwrap();
        assert!(save_step(dir.path(), 3, vec![]).unwrap().is_empty());
        assert!(read(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn a_corrupt_file_is_unreadable() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), "{ not json").unwrap();
        assert!(read(dir.path()).is_err());
    }

    #[test]
    fn an_unknown_version_is_unreadable() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE_NAME),
            r#"{"version":2,"steps":{"1":[[0,0,1,1]]}}"#,
        )
        .unwrap();
        let err = read(dir.path()).unwrap_err();
        assert!(err.0.contains('2'), "{err}");
    }

    #[test]
    fn a_non_numeric_key_or_empty_box_is_unreadable() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE_NAME),
            r#"{"version":1,"steps":{"x":[[0,0,1,1]]}}"#,
        )
        .unwrap();
        assert!(read(dir.path()).is_err());
        std::fs::write(
            dir.path().join(FILE_NAME),
            r#"{"version":1,"steps":{"1":[[0,0,0,1]]}}"#,
        )
        .unwrap();
        assert!(read(dir.path()).is_err());
    }

    #[test]
    fn a_save_never_overwrites_an_unreadable_file() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), "garbage").unwrap();
        assert!(save_step(dir.path(), 1, vec![[0, 0, 1, 1]]).is_err());
        assert!(remove_step(dir.path(), 1).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap(),
            "garbage"
        );
    }

    #[test]
    fn reset_deletes_an_unreadable_file_and_tolerates_none() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), "garbage").unwrap();
        reset(dir.path()).unwrap();
        assert!(!dir.path().join(FILE_NAME).exists());
        reset(dir.path()).unwrap();
    }

    #[test]
    fn concurrent_saves_for_different_steps_both_persist() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let handles: Vec<_> = (1..=8u32)
            .map(|step| {
                let p = path.clone();
                std::thread::spawn(move || save_step(&p, step, vec![[0, 0, 1, 1]]).unwrap())
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(read(&path).unwrap().len(), 8);
        // Only the final file remains; no temp file is left behind.
        let names: Vec<_> = std::fs::read_dir(&path)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from(FILE_NAME)]);
    }

    #[test]
    fn a_save_concurrent_with_a_removal_keeps_the_saved_box() {
        let dir = tempdir().unwrap();
        save_step(dir.path(), 2, vec![[0, 0, 1, 1]]).unwrap();
        let p1 = dir.path().to_path_buf();
        let p2 = dir.path().to_path_buf();
        let a = std::thread::spawn(move || save_step(&p1, 5, vec![[1, 1, 2, 2]]).unwrap());
        let b = std::thread::spawn(move || remove_step(&p2, 2).unwrap());
        a.join().unwrap();
        b.join().unwrap();
        let boxes = read(dir.path()).unwrap();
        assert_eq!(boxes.get(&5), Some(&vec![[1, 1, 2, 2]]));
        assert!(!boxes.contains_key(&2));
    }
}
