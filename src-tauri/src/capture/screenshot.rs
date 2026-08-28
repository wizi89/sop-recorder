use image::{DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_polygon_mut};
use imageproc::point::Point;
use serde::{Deserialize, Serialize};
use std::path::Path;
use xcap::Monitor;

/// The desktop a capture was composited against.
///
/// Two coordinate spaces meet in this module and they are not the same one on
/// every platform. `Monitor`'s geometry and the OS's cursor position are in
/// *logical* units; `Monitor::capture_image` hands back *physical* pixels. On
/// Windows the two coincide, so the difference stayed invisible; on a Retina
/// Mac there are two pixels per logical unit and conflating them cost the
/// screenshot three quarters of the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualScreen {
    /// Top-left of the virtual desktop, in logical units -- the space a click
    /// position arrives in, so it is what gets subtracted from one.
    origin: (i32, i32),
    /// Canvas pixels per logical unit.
    scale: f64,
}

impl VirtualScreen {
    /// Where a click lands on the composited canvas.
    fn to_canvas(&self, click_x: i32, click_y: i32) -> (i32, i32) {
        (
            ((click_x - self.origin.0) as f64 * self.scale).round() as i32,
            ((click_y - self.origin.1) as f64 * self.scale).round() as i32,
        )
    }
}

/// Capture the full virtual screen across all monitors.
/// Returns the composited image and the geometry it was composited against.
pub fn capture_full_screen() -> Result<(RgbaImage, VirtualScreen), String> {
    let monitors = Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {}", e))?;

    if monitors.is_empty() {
        return Err("No monitors found".into());
    }

    // Virtual screen bounds, in logical units.
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for m in &monitors {
        let x = m
            .x()
            .map_err(|e| format!("Failed to read monitor x position: {}", e))?;
        let y = m
            .y()
            .map_err(|e| format!("Failed to read monitor y position: {}", e))?;
        let w = m
            .width()
            .map_err(|e| format!("Failed to read monitor width: {}", e))? as i32;
        let h = m
            .height()
            .map_err(|e| format!("Failed to read monitor height: {}", e))? as i32;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + w);
        max_y = max_y.max(y + h);
    }

    // One canvas scale for the whole desktop, taken from the densest monitor so
    // no display is composited below its native resolution. A less dense one is
    // scaled up to match, which is the only way a mixed-DPI desktop can share a
    // single canvas at all.
    let scale = monitors
        .iter()
        .filter_map(|m| m.scale_factor().ok())
        .filter(|s| *s > 0.0)
        .fold(1.0f64, |acc, s| acc.max(s as f64));

    let total_w = (((max_x - min_x) as f64) * scale).round() as u32;
    let total_h = (((max_y - min_y) as f64) * scale).round() as u32;
    let screen = VirtualScreen {
        origin: (min_x, min_y),
        scale,
    };
    let mut canvas = RgbaImage::new(total_w, total_h);

    // Capture each monitor and composite onto the canvas
    for m in &monitors {
        let img = m
            .capture_image()
            .map_err(|e| format!("Capture failed for monitor: {}", e))?;

        let x = m
            .x()
            .map_err(|e| format!("Failed to read monitor x position: {}", e))?;
        let y = m
            .y()
            .map_err(|e| format!("Failed to read monitor y position: {}", e))?;
        let logical_w = m
            .width()
            .map_err(|e| format!("Failed to read monitor width: {}", e))?;
        let logical_h = m
            .height()
            .map_err(|e| format!("Failed to read monitor height: {}", e))?;

        // What this monitor must occupy on the canvas. The captured image is
        // already this size whenever the monitor is the one that set the scale,
        // so the resize is skipped on the overwhelmingly common single-monitor
        // desktop rather than paying for a no-op resample.
        let target_w = ((logical_w as f64) * scale).round() as u32;
        let target_h = ((logical_h as f64) * scale).round() as u32;
        let img = if img.width() == target_w && img.height() == target_h {
            img
        } else {
            log::info!(
                "Rescaling monitor capture from {}x{} to {}x{} for the shared canvas",
                img.width(),
                img.height(),
                target_w,
                target_h,
            );
            DynamicImage::ImageRgba8(img)
                .resize_exact(target_w, target_h, image::imageops::FilterType::Lanczos3)
                .to_rgba8()
        };

        let (offset_x, offset_y) = screen.to_canvas(x, y);
        let (offset_x, offset_y) = (offset_x.max(0) as u32, offset_y.max(0) as u32);

        for (px, py, pixel) in img.enumerate_pixels() {
            let cx = offset_x + px;
            let cy = offset_y + py;
            if cx < total_w && cy < total_h {
                canvas.put_pixel(cx, cy, *pixel);
            }
        }
    }

    Ok((canvas, screen))
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

/// The cursor arrow, as offsets from the click point at scale 1. Kept apart
/// from the drawing so the marker box and the tests measure the same shape the
/// renderer draws rather than a restatement of it.
const ARROW_OFFSETS: [(i32, i32); 7] = [
    (0, 0),
    (0, 20),
    (5, 16),
    (10, 24),
    (13, 22),
    (8, 14),
    (14, 14),
];

/// The box the marker occupies for a click at a canvas point, at `scale`.
///
/// The arrow hangs below and to the right of the click point, so a box around
/// the disc alone would leave its lower tips in the image.
fn marker_box_at(cx: i32, cy: i32, scale: f64) -> MarkerBox {
    let radius = scaled_radius(scale);
    let mut marker = MarkerBox {
        x0: cx - radius,
        y0: cy - radius,
        x1: cx + radius,
        y1: cy + radius,
    };
    for (dx, dy) in arrow_points(cx, cy, scale) {
        marker.x0 = marker.x0.min(dx);
        marker.y0 = marker.y0.min(dy);
        marker.x1 = marker.x1.max(dx);
        marker.y1 = marker.y1.max(dy);
    }
    marker
}

fn scaled_radius(scale: f64) -> i32 {
    (CLICK_MARKER_RADIUS as f64 * scale).round() as i32
}

fn arrow_points(cx: i32, cy: i32, scale: f64) -> [(i32, i32); 7] {
    ARROW_OFFSETS.map(|(dx, dy)| {
        (
            cx + (dx as f64 * scale).round() as i32,
            cy + (dy as f64 * scale).round() as i32,
        )
    })
}

/// Render a click overlay on the screenshot: red semi-transparent dot + white
/// cursor arrow.
///
/// `click` is in the OS's logical coordinate space, as the input hook reports
/// it; `screen` is the geometry the image was composited against and is the
/// only thing that knows where that lands in these pixels. The marker is drawn
/// at the canvas's scale so it stays the same apparent size on a Retina display
/// as on a 1x one, rather than shrinking to a speck the model cannot see.
///
/// Returns the box the marker was drawn into, in this image's pixels. Derived
/// from the drawing itself rather than restated, so the two cannot drift apart.
pub fn render_click_overlay(
    img: &mut RgbaImage,
    click_x: i32,
    click_y: i32,
    screen: &VirtualScreen,
) -> MarkerBox {
    let (cx, cy) = screen.to_canvas(click_x, click_y);

    // Red semi-transparent dot (alpha=0.7 -> 179)
    let red = Rgba([255, 0, 0, 179]);
    draw_filled_circle_mut(img, (cx, cy), scaled_radius(screen.scale), red);

    // White cursor arrow polygon (simplified)
    let white = Rgba([255, 255, 255, 230]);
    let points: Vec<Point<i32>> = arrow_points(cx, cy, screen.scale)
        .iter()
        .map(|(x, y)| Point::new(*x, *y))
        .collect();
    draw_polygon_mut(img, &points, white);

    marker_box_at(cx, cy, screen.scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The box the overlay draws for a click at canvas point (cx, cy) on a 1x
    /// desktop. The renderer's own geometry, so a test cannot pass against a
    /// shape the renderer does not draw.
    fn drawn_box(cx: i32, cy: i32) -> MarkerBox {
        marker_box_at(cx, cy, 1.0)
    }

    fn screen(origin: (i32, i32), scale: f64) -> VirtualScreen {
        VirtualScreen { origin, scale }
    }

    /// The Retina defect this module was rebuilt around. `Monitor` reports
    /// geometry and the OS reports the cursor in logical units, while
    /// `capture_image` returns physical pixels: on a 2x display a click at
    /// logical (500, 400) is at pixel (1000, 800) of the capture. Marking the
    /// logical point put every marker at a quarter of the desktop's area, up
    /// and to the left of the thing the user actually clicked.
    #[test]
    fn a_click_is_mapped_into_the_canvas_at_its_scale() {
        assert_eq!(screen((0, 0), 2.0).to_canvas(500, 400), (1000, 800));
        assert_eq!(screen((0, 0), 1.0).to_canvas(500, 400), (500, 400));
    }

    /// The origin is subtracted in logical units, before the scale is applied:
    /// a monitor left of the primary is placed by the OS in the same space the
    /// cursor is reported in, not in canvas pixels.
    #[test]
    fn the_origin_is_subtracted_before_the_scale_is_applied() {
        // Primary at (0, 0), a second display 1470 logical units to its left.
        assert_eq!(screen((-1470, 0), 2.0).to_canvas(-1470, 0), (0, 0));
        assert_eq!(screen((-1470, 0), 2.0).to_canvas(0, 0), (2940, 0));
    }

    /// A marker drawn at a fixed pixel radius onto a 2x canvas is half the
    /// apparent size, and after the downscale to 1920 wide it is a speck.
    #[test]
    fn the_marker_keeps_its_apparent_size_on_a_dense_display() {
        let at_1x = drawn_box(1000, 1000);
        let at_2x = marker_box_at(2000, 2000, 2.0);
        assert_eq!(
            at_2x.x1 - at_2x.x0,
            (at_1x.x1 - at_1x.x0) * 2,
            "the marker must scale with the canvas, not stay 18 pixels on every display"
        );
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
    let (mut img, screen) = capture_full_screen()?;

    let marker = click_position.map(|(x, y)| render_click_overlay(&mut img, x, y, &screen));

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
    rgb_img
        .save(&path)
        .map_err(|e| format!("Failed to save screenshot: {}", e))?;

    log::info!("Screenshot saved: {}", path.display());
    Ok(saved_marker)
}
