use genpdf::elements::{Break, Image, PageBreak, Paragraph};
use genpdf::fonts;
use genpdf::style::Style;
use genpdf::{Alignment, Document, Element, SimplePageDecorator};
use std::path::Path;

/// Try to load a font family from individual files.
fn load_font_family(regular: &str, bold: &str, italic: &str, bold_italic: &str) -> Option<fonts::FontFamily<fonts::FontData>> {
    let regular = fonts::FontData::new(std::fs::read(regular).ok()?, None).ok()?;
    let bold = fonts::FontData::new(std::fs::read(bold).ok()?, None).ok()?;
    let italic = fonts::FontData::new(std::fs::read(italic).ok()?, None).ok()?;
    let bold_italic = fonts::FontData::new(std::fs::read(bold_italic).ok()?, None).ok()?;

    Some(fonts::FontFamily {
        regular,
        bold,
        italic,
        bold_italic,
    })
}

/// Strip markdown formatting from text for plain-text PDF rendering.
fn strip_markdown(text: &str) -> String {
    let mut result = text.to_string();
    // Bold: **text** or __text__
    result = result.replace("**", "");
    result = result.replace("__", "");
    // Italic: *text* or _text_ (only single markers left after bold removal)
    // Skip single * and _ -- too aggressive, may hit legitimate uses
    // Headers: # at start of line
    result = result
        .lines()
        .map(|line| line.trim_start_matches('#').trim_start())
        .collect::<Vec<_>>()
        .join("\n");
    result
}

/// Generate a PDF from enriched step data.
/// The image belonging to the guide's `order`-th step (1-based).
///
/// A lookup by position into the uploaded set, never a filename rebuilt from
/// the number. `step_NN.png` is the number of the *capture*, and a capture that
/// failed leaves a gap -- after one, the guide's third step is `step_04.png`.
/// Rebuilding the name put the previous step's picture under the text and left
/// the step at the gap with none, which is worse than either alone: the guide
/// looked complete and was wrong.
fn screenshot_for_step(
    screenshots: &[(u32, std::path::PathBuf)],
    order: usize,
) -> Option<&(u32, std::path::PathBuf)> {
    screenshots.get(order.checked_sub(1)?)
}

/// Build the local PDF fallback.
///
/// `screenshots` is the uploaded set, in the order it was sent, as
/// `(step number, path)`. Passed in rather than reconstructed from the step's
/// position: a capture that failed leaves a gap, so the third step of a guide
/// is not necessarily `step_03.png`. Rebuilding the name from the position put
/// the wrong picture under the text -- and, for the step at the gap, no picture
/// at all.
pub fn generate_pdf(
    output_dir: &Path,
    guide_title: &str,
    enriched: &[serde_json::Value],
    screenshots: &[(u32, std::path::PathBuf)],
) -> Result<(), String> {
    let font_dir = "C:/Windows/Fonts";

    let font_family = load_font_family(
        &format!("{}/segoeui.ttf", font_dir),
        &format!("{}/segoeuib.ttf", font_dir),
        &format!("{}/segoeuii.ttf", font_dir),
        &format!("{}/segoeuiz.ttf", font_dir),
    )
    .or_else(|| {
        load_font_family(
            &format!("{}/calibri.ttf", font_dir),
            &format!("{}/calibrib.ttf", font_dir),
            &format!("{}/calibrii.ttf", font_dir),
            &format!("{}/calibriz.ttf", font_dir),
        )
    })
    .or_else(|| {
        load_font_family(
            &format!("{}/arial.ttf", font_dir),
            &format!("{}/arialbd.ttf", font_dir),
            &format!("{}/ariali.ttf", font_dir),
            &format!("{}/arialbi.ttf", font_dir),
        )
    })
    .ok_or("No suitable font found on this system")?;

    let mut doc = Document::new(font_family);
    doc.set_title(guide_title);

    let mut decorator = SimplePageDecorator::new();
    decorator.set_margins(20);
    doc.set_page_decorator(decorator);

    // Title page
    doc.push(
        Paragraph::new(guide_title)
            .aligned(Alignment::Center)
            .styled(Style::new().bold().with_font_size(24)),
    );
    doc.push(Break::new(1.0));
    doc.push(
        Paragraph::new(format!("{} Schritte", enriched.len()))
            .aligned(Alignment::Center)
            .styled(Style::new().with_font_size(14)),
    );

    // Step pages (one step per page)
    for (i, step) in enriched.iter().enumerate() {
        doc.push(PageBreak::new());

        let order = i + 1;
        let title_raw = step
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("(ohne Titel)");
        let title = strip_markdown(title_raw);
        let body_raw = step
            .get("body_markdown")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let body = strip_markdown(body_raw);

        doc.push(
            Paragraph::new(format!("Schritt {} - {}", order, title))
                .styled(Style::new().bold().with_font_size(16)),
        );
        doc.push(Break::new(0.5));

        if !body.is_empty() {
            // Split body into paragraphs on double-newlines or single newlines
            // so the 3-part structure (Bildschirmzustand / Handlung / Ergebnis)
            // renders as separate blocks instead of one squished blob.
            for paragraph in body.split('\n') {
                let trimmed = paragraph.trim();
                if trimmed.is_empty() {
                    doc.push(Break::new(0.2));
                } else {
                    doc.push(
                        Paragraph::new(trimmed).styled(Style::new().with_font_size(11)),
                    );
                }
            }
            doc.push(Break::new(0.5));
        }

        // By position in the uploaded set, which is what `order` counts, not by
        // a filename rebuilt from it.
        if let Some((step_number, screenshot_path)) = screenshot_for_step(screenshots, order) {
            match Image::from_path(screenshot_path) {
                Ok(img) => {
                    doc.push(img);
                }
                Err(e) => {
                    log::warn!(
                        "Failed to embed screenshot {} ({}): {}",
                        step_number,
                        screenshot_path.display(),
                        e,
                    );
                }
            }
        } else {
            log::warn!(
                "No screenshot for step {} of {}; the guide has more steps than \
                 the recording has images",
                order,
                screenshots.len(),
            );
        }

        // Warnings (skip empty strings)
        if let Some(warnings) = step.get("warnings").and_then(|v| v.as_array()) {
            for w in warnings.iter().filter_map(|v| v.as_str()).filter(|w| !w.trim().is_empty()) {
                doc.push(Break::new(0.3));
                doc.push(
                    Paragraph::new(format!("Warnung: {}", strip_markdown(w)))
                        .styled(Style::new().bold().with_font_size(10)),
                );
            }
        }

        // Notes (skip empty strings)
        if let Some(notes) = step.get("notes").and_then(|v| v.as_array()) {
            for n in notes.iter().filter_map(|v| v.as_str()).filter(|n| !n.trim().is_empty()) {
                doc.push(Break::new(0.3));
                doc.push(
                    Paragraph::new(format!("Hinweis: {}", strip_markdown(n)))
                        .styled(Style::new().italic().with_font_size(10)),
                );
            }
        }
    }

    let pdf_path = output_dir.join("guide.pdf");
    doc.render_to_file(&pdf_path)
        .map_err(|e| format!("PDF render failed: {}", e))?;

    log::info!("PDF saved: {}", pdf_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A recording whose second capture failed: the files on disk are 1, 3, 4.
    fn gapped() -> Vec<(u32, PathBuf)> {
        vec![
            (1, PathBuf::from("screenshots/step_01.png")),
            (3, PathBuf::from("screenshots/step_03.png")),
            (4, PathBuf::from("screenshots/step_04.png")),
        ]
    }

    /// The regression. Guide step 2 is the *second uploaded* image, which after
    /// a gap is `step_03.png`. Rebuilding the name from the position asked for
    /// `step_02.png`, which does not exist, and gave step 3 the picture from
    /// step 2.
    #[test]
    fn a_step_after_a_gap_gets_the_image_that_was_uploaded_for_it() {
        let shots = gapped();

        assert_eq!(screenshot_for_step(&shots, 1).unwrap().0, 1);
        assert_eq!(screenshot_for_step(&shots, 2).unwrap().0, 3);
        assert_eq!(screenshot_for_step(&shots, 3).unwrap().0, 4);
    }

    #[test]
    fn a_gapless_recording_is_unchanged() {
        let shots: Vec<(u32, PathBuf)> = (1..=3)
            .map(|n| (n, PathBuf::from(format!("screenshots/step_{:02}.png", n))))
            .collect();

        for order in 1..=3usize {
            assert_eq!(screenshot_for_step(&shots, order).unwrap().0, order as u32);
        }
    }

    /// The server can return fewer steps than there were screenshots. Asking
    /// past the end is a missing image, not a panic.
    #[test]
    fn asking_beyond_the_uploaded_set_yields_nothing() {
        assert!(screenshot_for_step(&gapped(), 4).is_none());
        assert!(screenshot_for_step(&[], 1).is_none());
    }

    /// `order` is 1-based; zero would silently wrap to the last element.
    #[test]
    fn a_zero_order_is_refused_rather_than_wrapping() {
        assert!(screenshot_for_step(&gapped(), 0).is_none());
    }
}
