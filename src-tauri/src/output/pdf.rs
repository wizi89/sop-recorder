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
pub fn generate_pdf(
    output_dir: &Path,
    guide_title: &str,
    enriched: &[serde_json::Value],
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

        let screenshot_path = output_dir.join("screenshots").join(format!("step_{:02}.png", order));
        if screenshot_path.exists() {
            match Image::from_path(&screenshot_path) {
                Ok(img) => {
                    doc.push(img);
                }
                Err(e) => {
                    log::warn!("Failed to embed screenshot {}: {}", order, e);
                }
            }
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
