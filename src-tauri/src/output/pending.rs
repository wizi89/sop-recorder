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
