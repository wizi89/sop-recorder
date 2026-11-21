//! The screenshots as they will be uploaded, with the review screen's boxes
//! burned in (review-redaction D2).

use std::path::{Path, PathBuf};

use crate::network::upload;

/// Marks an upload refused because of the review screen's boxes, so the
/// frontend returns to the review screen instead of the error screen, whose
/// Retry would repeat the same refusal (review-redaction D5).
pub const REDACTION_REFUSAL: &str = "[redaction] ";

fn refuse(msg: String) -> String {
    format!("{REDACTION_REFUSAL}{msg}")
}

/// The screenshots exactly as they will be uploaded, built once before the
/// upload loop (review-redaction D2): steps with boxes are decoded, filled and
/// re-encoded in memory; every other step is its file's bytes unchanged. Every
/// attempt, including the 401 retry, sends this list, so no retry can reach
/// the originals on disk.
///
/// Refuses, naming the step, when the boxes cannot be read, a screenshot with
/// boxes cannot be decoded, or a box entry or sidecar has no screenshot (a
/// delete that failed part way: the box may belong to a step that would
/// otherwise go out unredacted).
pub(super) fn build_upload_screenshots(
    screenshots_dir: &Path,
    screenshots: &[(u32, PathBuf)],
) -> Result<Vec<upload::UploadScreenshot>, String> {
    let boxes = crate::output::redactions::read(screenshots_dir).map_err(|e| {
        refuse(format!(
            "Die Schwärzungen dieser Aufnahme sind nicht lesbar, es wurde nichts hochgeladen. ({e})"
        ))
    })?;
    let present: std::collections::BTreeSet<u32> = screenshots.iter().map(|(n, _)| *n).collect();
    if let Some(step) =
        crate::commands::review::leftover_steps(screenshots_dir, &present, Some(&boxes)).first()
    {
        return Err(refuse(format!(
            "Schritt {step} wurde nicht vollständig gelöscht, es wurde nichts hochgeladen. \
             Bitte den Schritt erneut löschen."
        )));
    }

    screenshots
        .iter()
        .map(|(order, path)| {
            let bytes = std::fs::read(path)
                .map_err(|e| format!("Failed to read screenshot {}: {}", order, e))?;
            let bytes = match boxes.get(order) {
                Some(list) if !list.is_empty() => burn(&bytes, list).map_err(|e| {
                    refuse(format!(
                        "Schritt {order} konnte nicht geschwärzt werden, es wurde nichts \
                         hochgeladen. ({e})"
                    ))
                })?,
                _ => bytes,
            };
            Ok(upload::UploadScreenshot {
                order: *order,
                path: path.clone(),
                bytes,
            })
        })
        .collect()
}

/// Decode a PNG, fill the boxes, encode it again.
fn burn(png: &[u8], boxes: &[crate::capture::screenshot::PixelBox]) -> Result<Vec<u8>, String> {
    let mut img = image::load_from_memory(png)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    crate::capture::screenshot::apply_boxes(&mut img, boxes);
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}
