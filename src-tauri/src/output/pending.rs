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

/// Scan a workflows directory for the most recent folder containing pending.json.
#[allow(dead_code)]
pub fn find_pending(workflows_dir: &Path) -> Option<std::path::PathBuf> {
    if !workflows_dir.is_dir() {
        return None;
    }

    let mut entries: Vec<_> = fs::read_dir(workflows_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| e.path().join("pending.json").exists())
        .collect();

    // Sort by name descending (timestamps sort lexicographically)
    entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    entries.first().map(|e| e.path())
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

    #[test]
    fn find_pending_returns_most_recent() {
        let base = tempdir().unwrap();

        let dir_a = base.path().join("SOP 2026-01-01 10-00-00");
        let dir_b = base.path().join("SOP 2026-01-02 10-00-00");
        let dir_c = base.path().join("SOP 2026-01-03 10-00-00");

        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::create_dir_all(&dir_c).unwrap();

        // Only dir_a and dir_c have pending.json
        write_pending(&dir_a, "Old").unwrap();
        write_pending(&dir_c, "New").unwrap();

        let found = find_pending(base.path()).unwrap();
        assert_eq!(found, dir_c);
    }

    #[test]
    fn find_pending_returns_none_when_no_pending() {
        let base = tempdir().unwrap();
        let dir = base.path().join("SOP 2026-01-01");
        fs::create_dir_all(&dir).unwrap();

        assert!(find_pending(base.path()).is_none());
    }
}
