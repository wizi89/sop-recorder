use image::{DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_polygon_mut};
use imageproc::point::Point;
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

/// Render a click overlay on the screenshot: red semi-transparent dot + white cursor arrow.
pub fn render_click_overlay(img: &mut RgbaImage, click_x: i32, click_y: i32) {
    let (offset_x, offset_y) = get_virtual_screen_offset();
    let cx = (click_x - offset_x) as i32;
    let cy = (click_y - offset_y) as i32;

    // Red semi-transparent dot (radius=18, alpha=0.7 -> 179)
    let red = Rgba([255, 0, 0, 179]);
    draw_filled_circle_mut(img, (cx, cy), 18, red);

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
}

/// Capture a screenshot and save it as a numbered PNG.
pub fn capture_and_save(
    output_dir: &Path,
    step_number: u32,
    click_position: Option<(i32, i32)>,
) -> Result<String, String> {
    let mut img = capture_full_screen()?;

    if let Some((x, y)) = click_position {
        render_click_overlay(&mut img, x, y);
    }

    let filename = format!("step_{:02}.png", step_number);
    let path = output_dir.join(&filename);

    // Convert RGBA to RGB before saving -- Azure OpenAI vision rejects RGBA PNGs
    let rgb_img = DynamicImage::ImageRgba8(img).to_rgb8();
    rgb_img.save(&path)
        .map_err(|e| format!("Failed to save screenshot: {}", e))?;

    log::info!("Screenshot saved: {}", path.display());
    Ok(filename)
}
