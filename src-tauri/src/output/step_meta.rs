use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Per-screenshot metadata persisted as a sidecar JSON next to each
/// `step_NN.png`. Each capture writes its own sidecar after the PNG is
/// successfully on disk, so an aborted recording always leaves matched
/// (png, json) pairs and never an alignment-drifted state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepMeta {
    pub order: u32,
    pub timestamp_seconds: f64,
    pub click_x: Option<i32>,
    pub click_y: Option<i32>,
    pub trigger: String,
}

fn sidecar_path(output_dir: &Path, order: u32) -> std::path::PathBuf {
    output_dir.join(format!("step_{:02}.json", order))
}

pub fn write_sidecar(output_dir: &Path, meta: &StepMeta) -> Result<(), std::io::Error> {
    let path = sidecar_path(output_dir, meta.order);
    let content = serde_json::to_string_pretty(meta)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, content)
}

pub fn delete_sidecar(output_dir: &Path, order: u32) -> Result<(), std::io::Error> {
    let path = sidecar_path(output_dir, order);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Read sidecars `step_01.json`, `step_02.json`, ... in order, stopping at
/// the first gap. Used by the upload path to derive `metadata.steps`.
pub fn read_all(output_dir: &Path) -> Vec<StepMeta> {
    let mut metas = Vec::new();
    let mut order = 1u32;
    loop {
        let path = sidecar_path(output_dir, order);
        let Ok(content) = fs::read_to_string(&path) else {
            break;
        };
        match serde_json::from_str::<StepMeta>(&content) {
            Ok(m) => {
                metas.push(m);
                order += 1;
            }
            Err(e) => {
                log::warn!(
                    "step_meta: failed to parse {} ({}); stopping scan",
                    path.display(),
                    e
                );
                break;
            }
        }
    }
    metas
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_and_read_roundtrip() {
        let dir = tempdir().unwrap();
        let meta = StepMeta {
            order: 1,
            timestamp_seconds: 1.523,
            click_x: Some(320),
            click_y: Some(480),
            trigger: "mouse_click".into(),
        };
        write_sidecar(dir.path(), &meta).unwrap();

        let all = read_all(dir.path());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0], meta);
    }

    #[test]
    fn read_all_stops_at_first_gap() {
        let dir = tempdir().unwrap();
        for order in [1u32, 2, 4] {
            write_sidecar(
                dir.path(),
                &StepMeta {
                    order,
                    timestamp_seconds: order as f64,
                    click_x: None,
                    click_y: None,
                    trigger: "enter_key".into(),
                },
            )
            .unwrap();
        }

        let all = read_all(dir.path());
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].order, 1);
        assert_eq!(all[1].order, 2);
    }

    #[test]
    fn delete_sidecar_is_idempotent() {
        let dir = tempdir().unwrap();
        let meta = StepMeta {
            order: 7,
            timestamp_seconds: 12.0,
            click_x: None,
            click_y: None,
            trigger: "mouse_click".into(),
        };
        write_sidecar(dir.path(), &meta).unwrap();
        delete_sidecar(dir.path(), 7).unwrap();
        delete_sidecar(dir.path(), 7).unwrap();
        assert!(read_all(dir.path()).is_empty());
    }
}
