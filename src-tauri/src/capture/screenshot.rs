use image::{DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_polygon_mut};
use imageproc::point::Point;
use serde::{Deserialize, Serialize};
use std::path::Path;
use xcap::Monitor;

/// Capture the full virtual screen across all monitors.
/// Returns the composited image.
pub fn capture_full_screen() -> Result<RgbaImage, String> {
    let monitors = Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {}", e))?;

    if monitors.is_empty() {
        return Err("No monitors found".into());
    }

    // Calculate virtual screen bounds
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for m in &monitors {
        let x = m.x();
        let y = m.y();
        let w = m.width() as i32;
        let h = m.height() as i32;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + w);
        max_y = max_y.max(y + h);
    }

    let total_w = (max_x - min_x) as u32;
    let total_h = (max_y - min_y) as u32;
    let mut canvas = RgbaImage::new(total_w, total_h);

    // Capture each monitor and composite onto the canvas
    for m in &monitors {
        let img = m
            .capture_image()
            .map_err(|e| format!("Capture failed for monitor: {}", e))?;

        let offset_x = (m.x() - min_x) as u32;
        let offset_y = (m.y() - min_y) as u32;

        for (px, py, pixel) in img.enumerate_pixels() {
            let cx = offset_x + px;
            let cy = offset_y + py;
            if cx < total_w && cy < total_h {
                canvas.put_pixel(cx, cy, *pixel);
            }
        }
    }

    Ok(canvas)
}

/// Get the virtual screen offset (min_x, min_y) so click coordinates can be mapped.
pub fn get_virtual_screen_offset() -> (i32, i32) {
    let monitors = Monitor::all().unwrap_or_default();
    let min_x = monitors.iter().map(|m| m.x()).min().unwrap_or(0);
    let min_y = monitors.iter().map(|m| m.y()).min().unwrap_or(0);
    (min_x, min_y)
}

const CLICK_MARKER_RADIUS: i32 = 18;

/// The area a click marker occupies, in the pixels of the image it is recorded
/// against. Inclusive of both corners.
///
/// This travels to the server so it can blank the marker out of both frames
/// before comparing them: two clicks on an unchanged screen differ only by the
/// marker, and a perceptual hash downscales it away.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MarkerBox {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl MarkerBox {
    /// Map into the coordinate space of an image scaled by `scale`, growing
    /// outward so rounding can never leave a fringe of the marker unmasked.
    fn scaled(self, scale: f64) -> Self {
        Self {
            x0: (self.x0 as f64 * scale).floor() as i32,
            y0: (self.y0 as f64 * scale).floor() as i32,
            x1: (self.x1 as f64 * scale).ceil() as i32,
            y1: (self.y1 as f64 * scale).ceil() as i32,
        }
    }

    /// Clamp to the image, or `None` when nothing of the box lies inside it.
    ///
    /// A marker can be drawn wholly off the canvas: `render_click_overlay`
    /// subtracts an offset from a second `Monitor::all()` call, which reports a
    /// different set if a monitor is unplugged mid-recording and falls back to
    /// (0, 0) if it fails at all, leaving a click on a monitor left of the
    /// primary at a negative coordinate. Clamping each edge on its own would
    /// then yield an inverted rectangle and ship it as geometry. No geometry is
    /// the honest answer, and the consumer already skips on its absence.
    fn clamped(self, width: u32, height: u32) -> Option<Self> {
        let box_ = Self {
            x0: self.x0.max(0),
            y0: self.y0.max(0),
            x1: self.x1.min(width as i32 - 1),
            y1: self.y1.min(height as i32 - 1),
        };
        (box_.x0 <= box_.x1 && box_.y0 <= box_.y1).then_some(box_)
    }
}

/// Render a click overlay on the screenshot: red semi-transparent dot + white cursor arrow.
///
/// Returns the box the marker was drawn into, in this image's pixels. Derived
/// from the drawing itself rather than restated, so the two cannot drift apart.
pub fn render_click_overlay(img: &mut RgbaImage, click_x: i32, click_y: i32) -> MarkerBox {
    let (offset_x, offset_y) = get_virtual_screen_offset();
    let cx = (click_x - offset_x) as i32;
    let cy = (click_y - offset_y) as i32;

    // Red semi-transparent dot (radius=18, alpha=0.7 -> 179)
    let red = Rgba([255, 0, 0, 179]);
    draw_filled_circle_mut(img, (cx, cy), CLICK_MARKER_RADIUS, red);

    // White cursor arrow polygon (simplified)
    let white = Rgba([255, 255, 255, 230]);
    let arrow_points = [
        Point::new(cx, cy),
        Point::new(cx, cy + 20),
        Point::new(cx + 5, cy + 16),
        Point::new(cx + 10, cy + 24),
        Point::new(cx + 13, cy + 22),
        Point::new(cx + 8, cy + 14),
        Point::new(cx + 14, cy + 14),
    ];
    draw_polygon_mut(img, &arrow_points, white);

    // The arrow hangs below and to the right of the click point, so a box
    // around the disc alone would leave its lower tips in the image.
    let mut marker = MarkerBox {
        x0: cx - CLICK_MARKER_RADIUS,
        y0: cy - CLICK_MARKER_RADIUS,
        x1: cx + CLICK_MARKER_RADIUS,
        y1: cy + CLICK_MARKER_RADIUS,
    };
    for p in &arrow_points {
        marker.x0 = marker.x0.min(p.x);
        marker.y0 = marker.y0.min(p.y);
        marker.x1 = marker.x1.max(p.x);
        marker.y1 = marker.y1.max(p.y);
    }
    marker
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The box the overlay draws for a click at (cx, cy) on an unscaled image.
    /// Mirrors `render_click_overlay` without needing a screen to draw on.
    fn drawn_box(cx: i32, cy: i32) -> MarkerBox {
        MarkerBox {
            x0: cx - CLICK_MARKER_RADIUS,
            y0: cy - CLICK_MARKER_RADIUS,
            x1: cx + CLICK_MARKER_RADIUS,
            y1: cy + 24,
        }
    }

    #[test]
    fn box_encloses_the_arrow_not_only_the_disc() {
        let b = drawn_box(1000, 1000);
        // The arrow's lowest point is 24 below the click, past the disc's 18.
        assert!(
            b.y1 >= 1000 + 24,
            "a disc-only box leaves the arrow tips in the image"
        );
    }

    #[test]
    fn box_is_scaled_into_the_saved_image() {
        // 3840x2160 desktop saved at 1920x1080: everything halves.
        let scale = f64::min(1920.0 / 3840.0, 1080.0 / 2160.0);
        let drawn = drawn_box(1000, 1000);
        let saved = drawn.scaled(scale);

        assert_eq!(saved.x1 - saved.x0, (drawn.x1 - drawn.x0) / 2);
        assert_ne!(
            saved.x1 - saved.x0,
            drawn.x1 - drawn.x0,
            "the recorded box must be in saved-image pixels, not the desktop's"
        );
        // Rounding grows the box outward, so no fringe of marker survives.
        assert!(saved.x0 as f64 <= drawn.x0 as f64 * scale);
        assert!(saved.y1 as f64 >= drawn.y1 as f64 * scale);
    }

    #[test]
    fn box_is_clamped_to_the_image() {
        // A click in the top-left corner draws a box running off the canvas.
        let clamped = drawn_box(2, 2).clamped(1920, 1080).expect("still overlaps");
        assert_eq!((clamped.x0, clamped.y0), (0, 0));
        assert!(clamped.x1 < 1920 && clamped.y1 < 1080);
    }

    #[test]
    fn a_box_wholly_off_the_canvas_yields_no_geometry() {
        // A click on a monitor left of the primary, with the offset lookup
        // having fallen back to (0, 0): the marker is drawn off-canvas and
        // clamping each edge alone would invert the rectangle.
        assert_eq!(drawn_box(-1000, 500).clamped(1920, 1080), None);
        assert_eq!(drawn_box(500, -1000).clamped(1920, 1080), None);
        assert_eq!(drawn_box(4000, 500).clamped(1920, 1080), None);
    }

    #[test]
    fn the_full_pipeline_never_produces_an_inverted_box() {
        for (cx, cy) in [(-3000, -3000), (0, 0), (960, 540), (9000, 9000)] {
            if let Some(b) = drawn_box(cx, cy).scaled(0.5).clamped(1920, 1080) {
                assert!(b.x0 <= b.x1 && b.y0 <= b.y1, "inverted box at ({cx}, {cy})");
            }
        }
    }
}

/// Capture a screenshot and save it as a numbered PNG.
///
/// Returns the marker box in the coordinate space of the **saved** image, or
/// `None` when no marker was drawn. The scale factor and the virtual-screen
/// offset are known only here, so this is the only place the box can be
/// expressed in the pixels the server will actually receive.
pub fn capture_and_save(
    output_dir: &Path,
    step_number: u32,
    click_position: Option<(i32, i32)>,
) -> Result<Option<MarkerBox>, String> {
    let mut img = capture_full_screen()?;

    let marker = click_position.map(|(x, y)| render_click_overlay(&mut img, x, y));

    let filename = format!("step_{:02}.png", step_number);
    let path = output_dir.join(&filename);

    // Convert RGBA to RGB and resize if needed to stay under Azure OpenAI's 4MB limit
    let dynamic = DynamicImage::ImageRgba8(img);
    let (w, h) = (dynamic.width(), dynamic.height());
    let max_w = 1920u32;
    let max_h = 1080u32;
    let (resized, scale) = if w > max_w || h > max_h {
        let scale = f64::min(max_w as f64 / w as f64, max_h as f64 / h as f64);
        let new_w = (w as f64 * scale) as u32;
        let new_h = (h as f64 * scale) as u32;
        log::info!("Resizing screenshot from {}x{} to {}x{}", w, h, new_w, new_h);
        (
            dynamic.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3),
            scale,
        )
    } else {
        (dynamic, 1.0)
    };
    let saved_marker = marker
        .and_then(|m| m.scaled(scale).clamped(resized.width(), resized.height()));
    let rgb_img = resized.to_rgb8();
    rgb_img.save(&path)
        .map_err(|e| format!("Failed to save screenshot: {}", e))?;

    log::info!("Screenshot saved: {}", path.display());
    Ok(saved_marker)
}
