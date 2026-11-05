use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::capture::screenshot::MarkerBox;

/// Per-screenshot metadata persisted as a sidecar JSON next to each
/// `step_NN.png`. Each capture writes its own sidecar after the PNG is
/// successfully on disk, so an aborted recording always leaves matched
/// (png, json) pairs and never an alignment-drifted state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StepMeta {
    pub order: u32,
    pub timestamp_seconds: f64,
    pub click_x: Option<i32>,
    pub click_y: Option<i32>,
    pub trigger: String,
    /// Where the click marker was drawn, in the saved image's pixels. Absent for
    /// keypress steps, which draw none.
    ///
    /// Optional with a default on purpose: `read_all` abandons the scan on the
    /// first parse failure, so a required field here would make every sidecar
    /// written by an earlier build unreadable. A recording captured before an
    /// update and generated after it would then lose per-step alignment, and
    /// lose it silently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_box: Option<MarkerBox>,
    /// What the saved image covers (design D7). Missing in sidecars written
    /// before this field existed, which read as `unknown` and are marked on the
    /// review screen rather than assumed safe.
    #[serde(default)]
    pub capture_scope: CaptureScope,
    #[serde(default)]
    pub password_check: PasswordCheck,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureScope {
    Window,
    Monitor,
    MonitorFallback,
    /// Also what a value from a newer build reads as, so one unrecognised
    /// value does not make the whole sidecar unreadable.
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PasswordCheck {
    Ok,
    Flagged,
    Off,
    #[default]
    #[serde(other)]
    Unknown,
}

impl CaptureScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Monitor => "monitor",
            Self::MonitorFallback => "monitor_fallback",
            Self::Unknown => "unknown",
        }
    }
}

impl PasswordCheck {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Flagged => "flagged",
            Self::Off => "off",
            Self::Unknown => "unknown",
        }
    }
}

/// How each screenshot in `screenshots_dir` was captured, by step number. A
/// step whose sidecar is missing or unreadable reads as `unknown` on both
/// (design D7, D9): absence of a record is never taken as safe.
pub fn capture_records(
    screenshots_dir: &Path,
    orders: &[u32],
) -> Vec<(CaptureScope, PasswordCheck)> {
    let metas = read_all(screenshots_dir);
    orders
        .iter()
        .map(|order| {
            metas
                .iter()
                .find(|m| m.order == *order)
                .map(|m| (m.capture_scope, m.password_check))
                .unwrap_or_default()
        })
        .collect()
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

/// The step number a sidecar filename encodes, or `None` if it is not one of
/// ours. Mirrors the screenshot parser in `commands::generate`.
fn order_from_filename(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("step_")?.strip_suffix(".json")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Read every sidecar in the folder, in ascending step order. Used by the
/// upload path to derive `metadata.steps`.
///
/// Scans the directory rather than counting upward from 1. Counting stopped at
/// the first gap, which paired badly with the screenshot enumeration doing the
/// same: a capture that failed at step 2 left 20 screenshots and one readable
/// sidecar, the lengths disagreed, and the upload dropped per-step alignment
/// for the whole recording. That is the recording losing its narration, so the
/// gap has to be survivable here too.
///
/// A sidecar is only written once its PNG is on disk, so the sidecars are
/// always a subset of the screenshots. Missing one therefore shows up as a
/// length mismatch at the call site, which drops alignment -- the safe
/// direction -- rather than shifting every later step onto the wrong image.
///
/// The server pairs `metadata.steps` to the screenshots positionally, by
/// ascending key, and never reads `order` (`routes_generate.py`). Ascending
/// order here is what makes that pairing correct across a gap.
pub fn read_all(output_dir: &Path) -> Vec<StepMeta> {
    let Ok(entries) = fs::read_dir(output_dir) else {
        return Vec::new();
    };

    let mut found: Vec<(u32, StepMeta)> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let order = order_from_filename(&path.file_name()?.to_string_lossy())?;
            let content = fs::read_to_string(&path).ok()?;
            match serde_json::from_str::<StepMeta>(&content) {
                Ok(meta) => Some((order, meta)),
                Err(e) => {
                    // Skipped, not fatal: one unreadable sidecar costs this
                    // recording its alignment either way, and abandoning the
                    // scan would hide how many others were fine.
                    log::warn!(
                        "step_meta: failed to parse {} ({}); skipping it",
                        path.display(),
                        e
                    );
                    None
                }
            }
        })
        .collect();
    found.sort_by_key(|(order, _)| *order);

    found.into_iter().map(|(_, meta)| meta).collect()
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
            marker_box: None,
            ..Default::default()
        };
        write_sidecar(dir.path(), &meta).unwrap();

        let all = read_all(dir.path());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0], meta);
    }

    fn write_orders(dir: &Path, orders: &[u32]) {
        for &order in orders {
            write_sidecar(
                dir,
                &StepMeta {
                    order,
                    timestamp_seconds: order as f64,
                    click_x: None,
                    click_y: None,
                    trigger: "enter_key".into(),
                    marker_box: None,
                    ..Default::default()
                },
            )
            .unwrap();
        }
    }

    /// A capture failed at step 3. The sidecars either side of it are still the
    /// user's recording, and dropping them costs the guide its per-step
    /// narration -- the scan used to stop here and return only step 1.
    #[test]
    fn a_gap_does_not_end_the_scan() {
        let dir = tempdir().unwrap();
        write_orders(dir.path(), &[1, 2, 4]);

        let all = read_all(dir.path());
        assert_eq!(
            all.iter().map(|m| m.order).collect::<Vec<_>>(),
            vec![1, 2, 4],
        );
    }

    /// The property the server's positional pairing depends on: ascending
    /// order, matching the ascending screenshot keys it zips against.
    #[test]
    fn sidecars_are_ordered_numerically_not_lexicographically() {
        let dir = tempdir().unwrap();
        write_orders(dir.path(), &(1..=21).collect::<Vec<_>>());

        let all = read_all(dir.path());
        assert_eq!(
            all.iter().map(|m| m.order).collect::<Vec<_>>(),
            (1..=21).collect::<Vec<_>>(),
        );
    }

    /// One unreadable sidecar costs this recording its alignment either way.
    /// Skipping it rather than abandoning the scan keeps the count honest, so
    /// the log says how many were actually readable.
    #[test]
    fn an_unparseable_sidecar_is_skipped_not_fatal() {
        let dir = tempdir().unwrap();
        write_orders(dir.path(), &[1, 3]);
        fs::write(dir.path().join("step_02.json"), b"{ not json").unwrap();

        let all = read_all(dir.path());
        assert_eq!(all.iter().map(|m| m.order).collect::<Vec<_>>(), vec![1, 3]);
    }

    #[test]
    fn a_missing_folder_reads_as_no_sidecars() {
        let dir = tempdir().unwrap();
        assert!(read_all(&dir.path().join("nope")).is_empty());
    }

    #[test]
    fn sidecar_from_a_previous_build_still_parses() {
        // The shape written before marker geometry existed. If a required field
        // were added, this fails to parse, `read_all` abandons the scan here,
        // and every later step is lost with it.
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("step_01.json"),
            r#"{"order":1,"timestamp_seconds":1.5,"click_x":320,"click_y":480,"trigger":"mouse_click"}"#,
        )
        .unwrap();
        write_sidecar(
            dir.path(),
            &StepMeta {
                order: 2,
                timestamp_seconds: 2.5,
                click_x: Some(10),
                click_y: Some(20),
                trigger: "mouse_click".into(),
                marker_box: Some(MarkerBox {
                    x0: 1,
                    y0: 2,
                    x1: 3,
                    y1: 4,
                }),
                ..Default::default()
            },
        )
        .unwrap();

        let all = read_all(dir.path());
        assert_eq!(all.len(), 2, "an old sidecar must not stop the scan");
        assert_eq!(all[0].marker_box, None);
        assert_eq!(all[0].click_x, Some(320));
        assert!(all[1].marker_box.is_some());
    }

    #[test]
    fn keypress_sidecar_records_no_marker_geometry() {
        // No marker is drawn for a keypress, so the field is absent rather than
        // present-and-wrong. The server reads that absence together with
        // `trigger` to tell "no marker" from "marker, position unknown".
        let dir = tempdir().unwrap();
        let meta = StepMeta {
            order: 1,
            timestamp_seconds: 3.0,
            click_x: None,
            click_y: None,
            trigger: "enter_key".into(),
            marker_box: None,
            ..Default::default()
        };
        write_sidecar(dir.path(), &meta).unwrap();

        let raw = fs::read_to_string(dir.path().join("step_01.json")).unwrap();
        assert!(!raw.contains("marker_box"), "keypress sidecar: {}", raw);
        assert_eq!(read_all(dir.path())[0].marker_box, None);
    }

    #[test]
    fn capture_record_survives_the_round_trip() {
        let dir = tempdir().unwrap();
        for (order, (scope, check)) in [
            (CaptureScope::Window, PasswordCheck::Ok),
            (CaptureScope::Monitor, PasswordCheck::Off),
            (CaptureScope::MonitorFallback, PasswordCheck::Flagged),
        ]
        .into_iter()
        .enumerate()
        {
            write_sidecar(
                dir.path(),
                &StepMeta {
                    order: order as u32 + 1,
                    capture_scope: scope,
                    password_check: check,
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let raw = fs::read_to_string(dir.path().join("step_03.json")).unwrap();
        assert!(
            raw.contains(r#""capture_scope": "monitor_fallback""#),
            "{raw}"
        );
        assert!(raw.contains(r#""password_check": "flagged""#), "{raw}");

        let all = read_all(dir.path());
        assert_eq!(
            all.iter()
                .map(|m| (m.capture_scope, m.password_check))
                .collect::<Vec<_>>(),
            vec![
                (CaptureScope::Window, PasswordCheck::Ok),
                (CaptureScope::Monitor, PasswordCheck::Off),
                (CaptureScope::MonitorFallback, PasswordCheck::Flagged),
            ],
        );
    }

    #[test]
    fn a_sidecar_without_a_capture_record_reads_as_unknown() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("step_01.json"),
            r#"{"order":1,"timestamp_seconds":1.5,"click_x":320,"click_y":480,"trigger":"mouse_click"}"#,
        )
        .unwrap();
        let meta = &read_all(dir.path())[0];
        assert_eq!(meta.capture_scope, CaptureScope::Unknown);
        assert_eq!(meta.password_check, PasswordCheck::Unknown);
    }

    /// A value only a newer build knows must not make the sidecar unreadable,
    /// which would cost the recording its per-step alignment.
    #[test]
    fn an_unrecognised_capture_record_reads_as_unknown() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("step_01.json"),
            r#"{"order":1,"timestamp_seconds":1.5,"click_x":null,"click_y":null,"trigger":"enter_key","capture_scope":"region","password_check":"partial"}"#,
        )
        .unwrap();
        let meta = &read_all(dir.path())[0];
        assert_eq!(meta.capture_scope, CaptureScope::Unknown);
        assert_eq!(meta.password_check, PasswordCheck::Unknown);
    }

    #[test]
    fn a_screenshot_without_a_sidecar_reads_as_unknown() {
        let dir = tempdir().unwrap();
        write_sidecar(
            dir.path(),
            &StepMeta {
                order: 1,
                capture_scope: CaptureScope::Window,
                password_check: PasswordCheck::Ok,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            capture_records(dir.path(), &[1, 2]),
            vec![
                (CaptureScope::Window, PasswordCheck::Ok),
                (CaptureScope::Unknown, PasswordCheck::Unknown),
            ],
        );
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
            marker_box: None,
            ..Default::default()
        };
        write_sidecar(dir.path(), &meta).unwrap();
        delete_sidecar(dir.path(), 7).unwrap();
        delete_sidecar(dir.path(), 7).unwrap();
        assert!(read_all(dir.path()).is_empty());
    }
}
