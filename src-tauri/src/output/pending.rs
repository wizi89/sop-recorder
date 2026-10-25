use std::fs;
use std::path::Path;

/// Write a pending.json marker to the output directory.
pub fn write_pending(output_dir: &Path, guide_title: &str) -> Result<(), std::io::Error> {
    let path = output_dir.join("pending.json");
    let data = serde_json::json!({
        "guide_title": guide_title,
    });
    fs::write(&path, serde_json::to_string_pretty(&data).unwrap())?;
    log::info!("Pending marker written: {}", path.display());
    Ok(())
}

/// Read pending.json metadata from an output directory.
pub fn read_pending(output_dir: &Path) -> Option<serde_json::Value> {
    let path = output_dir.join("pending.json");
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Clear (delete) the pending.json marker after successful generation.
pub fn clear_pending(output_dir: &Path) {
    let path = output_dir.join("pending.json");
    if path.exists() {
        let _ = fs::remove_file(&path);
        log::info!("Pending marker cleared: {}", path.display());
    }
}

/// The newest recording that was stopped but never generated: a folder under
/// `workflows_dir` with a pending.json written within `max_age` of `now`, and
/// at least one captured step. Older markers are left alone -- an offer about
/// a recording from months ago is noise, and the folder is still there to
/// generate from by hand.
pub fn find_pending(
    workflows_dir: &Path,
    now: std::time::SystemTime,
    max_age: std::time::Duration,
) -> Option<std::path::PathBuf> {
    let recent = |dir: &Path| {
        fs::metadata(dir.join("pending.json"))
            .and_then(|m| m.modified())
            .is_ok_and(|t| now.duration_since(t).map_or(true, |age| age <= max_age))
    };
    let has_steps = |dir: &Path| {
        fs::read_dir(dir.join("screenshots")).is_ok_and(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name.starts_with("step_") && name.ends_with(".png")
            })
        })
    };

    let mut entries: Vec<_> = fs::read_dir(workflows_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && recent(p) && has_steps(p))
        .collect();

    // Folder names start with the recording's timestamp, which sorts.
    entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    entries.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_and_read_pending() {
        let dir = tempdir().unwrap();
        write_pending(dir.path(), "Test Guide").unwrap();

        let meta = read_pending(dir.path()).unwrap();
        assert_eq!(meta["guide_title"], "Test Guide");
    }

    #[test]
    fn read_pending_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        assert!(read_pending(dir.path()).is_none());
    }

    #[test]
    fn clear_pending_removes_file() {
        let dir = tempdir().unwrap();
        write_pending(dir.path(), "Test").unwrap();
        assert!(dir.path().join("pending.json").exists());

        clear_pending(dir.path());
        assert!(!dir.path().join("pending.json").exists());
    }

    const WEEK: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 3600);

    /// A stopped recording with one captured step.
    fn recording(base: &Path, name: &str) -> std::path::PathBuf {
        let dir = base.join(name);
        fs::create_dir_all(dir.join("screenshots")).unwrap();
        fs::write(dir.join("screenshots").join("step_01.png"), b"png").unwrap();
        write_pending(&dir, name).unwrap();
        dir
    }

    #[test]
    fn find_pending_returns_most_recent() {
        let base = tempdir().unwrap();
        recording(base.path(), "SOP 2026-01-01 10-00-00");
        fs::create_dir_all(base.path().join("SOP 2026-01-02 10-00-00")).unwrap();
        let newest = recording(base.path(), "SOP 2026-01-03 10-00-00");

        let found = find_pending(base.path(), std::time::SystemTime::now(), WEEK);
        assert_eq!(found, Some(newest));
    }

    #[test]
    fn find_pending_returns_none_when_no_pending() {
        let base = tempdir().unwrap();
        let dir = recording(base.path(), "SOP 2026-01-01");
        clear_pending(&dir);

        assert!(find_pending(base.path(), std::time::SystemTime::now(), WEEK).is_none());
    }

    #[test]
    fn a_recording_without_steps_is_not_offered() {
        let base = tempdir().unwrap();
        let dir = recording(base.path(), "SOP 2026-01-01");
        fs::remove_file(dir.join("screenshots").join("step_01.png")).unwrap();

        assert!(find_pending(base.path(), std::time::SystemTime::now(), WEEK).is_none());
    }

    #[test]
    fn a_marker_older_than_max_age_is_left_alone() {
        let base = tempdir().unwrap();
        recording(base.path(), "SOP 2026-01-01");
        let in_eight_days =
            std::time::SystemTime::now() + 8 * 24 * std::time::Duration::from_secs(3600);

        assert!(find_pending(base.path(), in_eight_days, WEEK).is_none());
    }

    #[test]
    fn a_missing_output_folder_finds_nothing() {
        let base = tempdir().unwrap();
        let missing = base.path().join("never-created");
        assert!(find_pending(&missing, std::time::SystemTime::now(), WEEK).is_none());
    }
}
