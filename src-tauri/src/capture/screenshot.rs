use image::{DynamicImage, RgbaImage};
use std::path::Path;
use xcap::Monitor;

use super::window_area::{self, Geometry, Rect, Snapshot};
use crate::output::step_meta::CaptureScope;

mod draw;
mod frame;

use draw::MASK;
pub use draw::{apply_boxes, render_click_overlay, MarkerBox, PixelBox};
pub use frame::frame_step;

/// The desktop a capture was composited against.
///
/// Two coordinate spaces meet in this module. Monitor geometry and the OS's
/// cursor position share one of them; `Monitor::capture_image` hands back
/// physical pixels, which on a Retina Mac are not the same thing. What that
/// first space *is* differs by platform -- points on macOS, physical pixels on
/// Windows -- so nothing here names it, and the ratio between the two is
/// measured rather than assumed. See `canvas_scale`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualScreen {
    /// Top-left of the virtual desktop, in the units monitor geometry and
    /// cursor positions arrive in, so it is what gets subtracted from a click.
    origin: (i32, i32),
    /// Canvas pixels per unit of that space.
    scale: f64,
}

impl VirtualScreen {
    /// Where a click lands on the composited canvas.
    fn to_canvas(self, click_x: i32, click_y: i32) -> (i32, i32) {
        (
            ((click_x - self.origin.0) as f64 * self.scale).round() as i32,
            ((click_y - self.origin.1) as f64 * self.scale).round() as i32,
        )
    }
}

/// Canvas pixels per unit of the space monitor geometry is reported in, given
/// `(geometry_width, captured_width)` for each monitor.
///
/// Measured from the captures rather than read from `Monitor::scale_factor`,
/// because a DPI scale only means something next to geometry whose units you
/// know, and xcap's units differ by platform: `CGDisplayBounds` on macOS, which
/// is points, and `dmPelsWidth` on Windows, which is already physical pixels.
/// Multiplying Windows' physical geometry by its DPI scale upscaled every
/// capture by that factor, only for the final resize to throw the added pixels
/// away again: softer screenshots, several times the memory, and about a second
/// added to every step.
///
/// The ratio answers the question directly and needs no per-platform branch. A
/// 2560px Windows monitor reports 2560 and captures 2560, giving 1.0 whatever
/// its DPI scale is; a Retina Mac reports 1512 points and captures 3024 pixels,
/// giving 2.0. The densest monitor wins, so no display is composited below its
/// native resolution, and a desktop whose displays agree needs no resample at
/// all.
fn canvas_scale(monitors: &[(u32, u32)]) -> f64 {
    monitors
        .iter()
        .filter(|(geometry_w, _)| *geometry_w > 0)
        .map(|(geometry_w, captured_w)| *captured_w as f64 / *geometry_w as f64)
        .fold(1.0f64, f64::max)
}

/// One monitor's placement, in the units cursor positions arrive in.
/// `(x, y, width, height)`.
pub type MonitorBounds = (i32, i32, u32, u32);

/// Which monitor an action happened on.
///
/// A step documents the screen the user was working on, so the capture is
/// scoped to that screen rather than to the whole desktop. Compositing every
/// display into one canvas and then fitting it under the upload's 1920x1080 cap
/// cost real legibility: two 4K displays side by side arrive at 960x540 each,
/// against 1663x1080 for the same click with one display connected. That is the
/// difference that made the model misread an application name in the
/// 2026-09-03 test.
///
/// `point` is the click position. `None` means the event carried none -- a key
/// press -- and the caller passes the cursor position in its place; when even
/// that is unavailable the primary monitor is the answer. A point outside every
/// monitor resolves to the primary too rather than failing: a step on an
/// unexpected screen is worth more than no step.
///
/// Pure, and separate from `Monitor::all()`, so the choice can be tested
/// against real display layouts without a display. The `Fallback` says why
/// the primary was chosen, which is worth saying in the log, because a step
/// captured on the wrong display looks like nothing else.
pub fn choose_monitor(
    point: Option<(i32, i32)>,
    bounds: &[MonitorBounds],
    primary: usize,
) -> (usize, Option<Fallback>) {
    match (monitor_containing(point, bounds), point) {
        (Some(i), _) => (i, None),
        (None, None) => (primary, Some(Fallback::NoPosition)),
        (None, Some(_)) => (primary, Some(Fallback::OffScreen)),
    }
}

/// Why `choose_monitor` fell back to the primary display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    /// Neither a click position nor the cursor position was available.
    NoPosition,
    /// The point falls on none of the monitors.
    OffScreen,
}

/// The monitor a point falls on, or `None` when there is no point or it falls
/// on none of them.
pub(crate) fn monitor_containing(
    point: Option<(i32, i32)>,
    bounds: &[MonitorBounds],
) -> Option<usize> {
    let (px, py) = point?;
    bounds
        .iter()
        .position(|&(x, y, w, h)| px >= x && px < x + w as i32 && py >= y && py < y + h as i32)
}

/// Capture one monitor, and the geometry to place a click on it.
///
/// `index` addresses `Monitor::all()`'s ordering, which is also what
/// `choose_monitor` indexes into; the two read the list once, together, in
/// `capture_and_save`.
///
/// The returned `VirtualScreen` has this monitor's own origin, so a click in
/// desktop coordinates lands correctly on an image that starts at that origin,
/// and its own geometry-to-capture ratio, so a Retina display still draws a
/// marker sized for its pixels.
fn capture_one_monitor(monitor: &Monitor) -> Result<(RgbaImage, VirtualScreen), String> {
    let x = monitor
        .x()
        .map_err(|e| format!("Failed to read monitor x position: {}", e))?;
    let y = monitor
        .y()
        .map_err(|e| format!("Failed to read monitor y position: {}", e))?;
    let geometry_w = monitor
        .width()
        .map_err(|e| format!("Failed to read monitor width: {}", e))?;
    let img = monitor
        .capture_image()
        .map_err(|e| format!("Capture failed for monitor: {}", e))?;

    // Measured, not read from `scale_factor()`: xcap reports geometry in points
    // on macOS and physical pixels on Windows, and the ratio answers the
    // question without a per-platform branch. See `canvas_scale`.
    let scale = canvas_scale(&[(geometry_w, img.width())]);

    Ok((
        img,
        VirtualScreen {
            origin: (x, y),
            scale,
        },
    ))
}

/// Capture the monitor the action happened on.
///
/// Reads `Monitor::all()` once, so the index `choose_monitor` returns and
/// the monitor that gets captured come from the same list -- asking twice
/// invites a different answer if a display is unplugged between the calls.
///
/// A key press carries no click position, so the cursor stands in for it. The
/// cursor is where the user is working even when the keyboard is what they
/// used, which is a better answer than the primary display and a much better
/// one than every display at once.
fn capture_active_monitor(
    click_position: Option<(i32, i32)>,
) -> Result<(RgbaImage, VirtualScreen), String> {
    let monitors = Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {}", e))?;
    if monitors.is_empty() {
        return Err("No monitors found".into());
    }

    let mut bounds: Vec<MonitorBounds> = Vec::with_capacity(monitors.len());
    let mut primary = 0usize;
    for (i, m) in monitors.iter().enumerate() {
        bounds.push((
            m.x()
                .map_err(|e| format!("Failed to read monitor x position: {}", e))?,
            m.y()
                .map_err(|e| format!("Failed to read monitor y position: {}", e))?,
            m.width()
                .map_err(|e| format!("Failed to read monitor width: {}", e))?,
            m.height()
                .map_err(|e| format!("Failed to read monitor height: {}", e))?,
        ));
        if m.is_primary().unwrap_or(false) {
            primary = i;
        }
    }

    let point = click_position.or_else(super::input_hooks::get_cursor_position);
    let (chosen, fallback) = choose_monitor(point, &bounds, primary);

    // Logged for a single display too, not only for several. "Which screen did
    // this step come from" is the first question when a guide shows the wrong
    // one, and a line that appears only on multi-monitor machines is missing
    // from exactly the recordings that need it. One line per step, beside the
    // one the save already writes.
    let why = match fallback {
        None => "",
        Some(Fallback::NoPosition) => " (no position available; fell back to the primary display)",
        Some(Fallback::OffScreen) => " (point is on no display; fell back to the primary display)",
    };
    log::info!(
        "Capturing monitor {} of {} at {:?} for point {:?}{}",
        chosen + 1,
        monitors.len(),
        bounds[chosen],
        point,
        why,
    );

    capture_one_monitor(&monitors[chosen])
}

/// One step after phase A (privacy-capture D3): its pixels, already cropped to
/// the step's area in the active window mode, and the geometry phase B needs.
pub struct HeldStep {
    img: RgbaImage,
    screen: VirtualScreen,
    /// Desktop rectangles the step shows; everything else in `img` is filled
    /// in phase B. Empty means the whole image.
    keep: Vec<Rect>,
    /// Where other apps' windows cover the step's windows: blacked out
    /// in phase B, with the password masks.
    foreign: Vec<Rect>,
    pub scope: CaptureScope,
    /// Both snapshots merged, for the password check. `None` off Windows.
    pub geometry: Option<Geometry>,
    /// Just before the pixel read: a window counts as checked only by queries
    /// that completed before it (amendment A2).
    pub pixels_at: std::time::Instant,
}

impl HeldStep {
    /// The desktop rectangles the saved image shows: the kept window
    /// rectangles without the foreign windows over them, or the whole image.
    pub fn shown(&self) -> Vec<Rect> {
        if self.keep.is_empty() {
            return vec![monitor_rect(&self.screen, &self.img)];
        }
        let mut shown = self.keep.clone();
        for cut in &self.foreign {
            shown = shown.iter().flat_map(|r| r.subtract(cut)).collect();
        }
        shown
    }
}

/// The monitor a `VirtualScreen` was captured from, in desktop units.
fn monitor_rect(screen: &VirtualScreen, img: &RgbaImage) -> Rect {
    Rect::new(
        screen.origin.0,
        screen.origin.1,
        screen.origin.0 + (img.width() as f64 / screen.scale).round() as i32,
        screen.origin.1 + (img.height() as f64 / screen.scale).round() as i32,
    )
}

/// A desktop rectangle in `img`'s pixels, clipped to it.
fn canvas_rect(screen: &VirtualScreen, img: &RgbaImage, r: &Rect) -> Option<(u32, u32, u32, u32)> {
    let (x0, y0) = screen.to_canvas(r.left, r.top);
    let (x1, y1) = screen.to_canvas(r.right, r.bottom);
    let (x0, y0) = (x0.max(0) as u32, y0.max(0) as u32);
    let (x1, y1) = (
        (x1.max(0) as u32).min(img.width()),
        (y1.max(0) as u32).min(img.height()),
    );
    (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
}

/// Cut `img` down to `area`, keeping `screen` pointed at the new origin so a
/// click in desktop coordinates still lands on the right pixel.
fn crop_to(
    img: &RgbaImage,
    screen: VirtualScreen,
    area: &Rect,
) -> Option<(RgbaImage, VirtualScreen)> {
    let (x0, y0, x1, y1) = canvas_rect(&screen, img, area)?;
    let cropped = image::imageops::crop_imm(img, x0, y0, x1 - x0, y1 - y0).to_image();
    let (left, top) = (
        screen.origin.0 + (x0 as f64 / screen.scale).round() as i32,
        screen.origin.1 + (y0 as f64 / screen.scale).round() as i32,
    );
    Some((
        cropped,
        VirtualScreen {
            origin: (left, top),
            scale: screen.scale,
        },
    ))
}

/// Pixels of the largest monitor, for sizing the buffer slots. In xcap's
/// units, which are points on macOS: a Retina display reads as a quarter of
/// its pixels (1470x956 for a 2940x1912 canvas), so a Mac gets six slots
/// where its true size would give four.
pub fn largest_monitor_pixels() -> Option<u64> {
    Monitor::all()
        .ok()?
        .iter()
        .filter_map(|m| Some(u64::from(m.width().ok()?) * u64::from(m.height().ok()?)))
        .max()
}

/// Phase A's OS reads: the window geometry before and after the pixels, and
/// the pixels of the monitor the action happened on. `frame_step` decides
/// what of it the step keeps.
pub struct PixelRead {
    img: RgbaImage,
    screen: VirtualScreen,
    /// `None` off Windows.
    before: Option<Snapshot>,
    after: Option<Snapshot>,
    /// Just before the pixel read (amendment A2).
    pixels_at: std::time::Instant,
}

/// Phase A's reads: geometry snapshot, pixel read, second snapshot (D2, D3).
/// No UIA work, so it is as quick as the pixel read.
pub fn capture_step(
    step_number: u32,
    click_position: Option<(i32, i32)>,
    monitor_mode: bool,
) -> Result<PixelRead, String> {
    let started = std::time::Instant::now();
    let before = super::geometry::snapshot(click_position);
    // In the active window mode a key press reads the foreground window's
    // monitor, not the cursor's: the area is that window, and on another
    // monitor it would fall back to capturing the cursor's monitor whole. In
    // monitor mode the spec keeps the cursor's monitor.
    let point = click_position.or_else(|| {
        if monitor_mode {
            None
        } else {
            before.as_ref().and_then(window_area::key_press_point)
        }
    });
    let pixels_at = std::time::Instant::now();
    let (img, screen) = capture_active_monitor(point)?;
    let after = super::geometry::snapshot(click_position);
    log::info!(
        "Step {step_number}: pixels read in {} ms ({}x{})",
        started.elapsed().as_millis(),
        img.width(),
        img.height()
    );
    Ok(PixelRead {
        img,
        screen,
        before,
        after,
        pixels_at,
    })
}

/// Phase B's image work, without the disk: gap fill, masks, marker, downsize.
///
/// Masks go on before the marker, so the marker stays visible over a masked
/// field, and everything happens before the encode, so no unmasked image is
/// ever written. Returns the image to save and the marker box in its pixels.
fn finish_image(
    mut img: RgbaImage,
    screen: VirtualScreen,
    keep: &[Rect],
    masks: &[Rect],
    click_position: Option<(i32, i32)>,
) -> (DynamicImage, Option<MarkerBox>) {
    // One kept rectangle is the crop itself; only a dialog with a distant
    // owner, or a menu past the window edge, leaves a gap.
    if keep.len() > 1 {
        let kept: Vec<_> = keep
            .iter()
            .filter_map(|r| canvas_rect(&screen, &img, r))
            .collect();
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            if !kept
                .iter()
                .any(|&(x0, y0, x1, y1)| x >= x0 && x < x1 && y >= y0 && y < y1)
            {
                *pixel = MASK;
            }
        }
    }
    let mask_boxes: Vec<PixelBox> = masks
        .iter()
        .filter_map(|mask| canvas_rect(&screen, &img, mask))
        .map(|(x0, y0, x1, y1)| [x0 as i64, y0 as i64, (x1 - x0) as i64, (y1 - y0) as i64])
        .collect();
    apply_boxes(&mut img, &mask_boxes);

    let marker = click_position.map(|(x, y)| render_click_overlay(&mut img, x, y, &screen));

    // Resize if needed to stay under Azure OpenAI's 4MB limit
    let dynamic = DynamicImage::ImageRgba8(img);
    let (w, h) = (dynamic.width(), dynamic.height());
    let max_w = 1920u32;
    let max_h = 1080u32;
    let (resized, scale) = if w > max_w || h > max_h {
        let scale = f64::min(max_w as f64 / w as f64, max_h as f64 / h as f64);
        let new_w = (w as f64 * scale) as u32;
        let new_h = (h as f64 * scale) as u32;
        log::info!(
            "Resizing screenshot from {}x{} to {}x{}",
            w,
            h,
            new_w,
            new_h
        );
        (
            dynamic.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3),
            scale,
        )
    } else {
        (dynamic, 1.0)
    };
    let saved_marker =
        marker.and_then(|m| m.scaled(scale).clamped(resized.width(), resized.height()));
    (resized, saved_marker)
}

/// Phase B after the password check: finish the image and write the PNG.
///
/// Returns the marker box in the coordinate space of the **saved** image, or
/// `None` when no marker was drawn. The crop offset, scale factor and
/// virtual-screen offset are known only here, so this is the only place the
/// box can be expressed in the pixels the server will actually receive.
pub fn finish_and_save(
    held: HeldStep,
    output_dir: &Path,
    step_number: u32,
    click_position: Option<(i32, i32)>,
    masks: &[Rect],
) -> Result<Option<MarkerBox>, String> {
    let started = std::time::Instant::now();
    let (w, h) = (held.img.width(), held.img.height());
    let all_masks: Vec<Rect> = held.foreign.iter().chain(masks).copied().collect();
    let (image, marker) = finish_image(
        held.img,
        held.screen,
        &held.keep,
        &all_masks,
        click_position,
    );

    let path = output_dir.join(format!("step_{:02}.png", step_number));
    let rgb_img = image.to_rgb8();
    rgb_img
        .save(&path)
        .map_err(|e| format!("Failed to save screenshot: {}", e))?;

    log::info!(
        "Screenshot saved: {} (canvas {}x{}, saved {}x{}, {} mask(s), finish {} ms)",
        path.display(),
        w,
        h,
        rgb_img.width(),
        rgb_img.height(),
        masks.len(),
        started.elapsed().as_millis(),
    );
    Ok(marker)
}

#[cfg(test)]
mod tests;
