use image::{DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::draw_hollow_circle_mut;
use serde::{Deserialize, Serialize};
use std::path::Path;
use xcap::Monitor;

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
    fn to_canvas(&self, click_x: i32, click_y: i32) -> (i32, i32) {
        (
            ((click_x - self.origin.0) as f64 * self.scale).round() as i32,
            ((click_y - self.origin.1) as f64 * self.scale).round() as i32,
        )
    }
}

/// One monitor's capture, paired with the geometry it was reported at.
struct Shot {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    img: RgbaImage,
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
/// against real display layouts without a display.
pub fn monitor_for_click(
    point: Option<(i32, i32)>,
    bounds: &[MonitorBounds],
    primary: usize,
) -> usize {
    monitor_containing(point, bounds).unwrap_or(primary)
}

/// The monitor a point falls on, or `None` when there is no point or it falls
/// on none of them. Split out from `monitor_for_click` so a caller can tell a
/// deliberate choice from a fallback -- which is worth saying in the log,
/// because a step captured on the wrong display looks like nothing else.
pub fn monitor_containing(
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
/// `monitor_for_click` indexes into; the two read the list once, together, in
/// `capture_and_save`.
///
/// The returned `VirtualScreen` has this monitor's own origin, so a click in
/// desktop coordinates lands correctly on an image that starts at that origin,
/// and its own geometry-to-capture ratio, so a Retina display still draws a
/// marker sized for its pixels.
fn capture_one_monitor(
    monitor: &Monitor,
) -> Result<(RgbaImage, VirtualScreen), String> {
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

    Ok((img, VirtualScreen { origin: (x, y), scale }))
}

/// Capture the full virtual screen across all monitors.
/// Returns the composited image and the geometry it was composited against.
///
/// Retained for the case where no monitor can be singled out at all. Ordinary
/// steps go through `capture_one_monitor`.
#[allow(dead_code)]
pub fn capture_full_screen() -> Result<(RgbaImage, VirtualScreen), String> {
    let monitors = Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {}", e))?;

    if monitors.is_empty() {
        return Err("No monitors found".into());
    }

    // Every monitor is captured before any of it is composited: the canvas
    // scale is measured from the captures, so they all have to be in hand
    // before the canvas can be sized.
    let mut shots = Vec::with_capacity(monitors.len());
    for m in &monitors {
        shots.push(Shot {
            x: m.x()
                .map_err(|e| format!("Failed to read monitor x position: {}", e))?,
            y: m.y()
                .map_err(|e| format!("Failed to read monitor y position: {}", e))?,
            w: m.width()
                .map_err(|e| format!("Failed to read monitor width: {}", e))?,
            h: m.height()
                .map_err(|e| format!("Failed to read monitor height: {}", e))?,
            img: m
                .capture_image()
                .map_err(|e| format!("Capture failed for monitor: {}", e))?,
        });
    }

    // Virtual screen bounds, in the units monitor geometry is reported in.
    let min_x = shots.iter().map(|s| s.x).min().unwrap_or(0);
    let min_y = shots.iter().map(|s| s.y).min().unwrap_or(0);
    let max_x = shots.iter().map(|s| s.x + s.w as i32).max().unwrap_or(0);
    let max_y = shots.iter().map(|s| s.y + s.h as i32).max().unwrap_or(0);

    let scale = canvas_scale(
        &shots
            .iter()
            .map(|s| (s.w, s.img.width()))
            .collect::<Vec<_>>(),
    );

    let total_w = (((max_x - min_x) as f64) * scale).round() as u32;
    let total_h = (((max_y - min_y) as f64) * scale).round() as u32;
    let screen = VirtualScreen {
        origin: (min_x, min_y),
        scale,
    };
    let mut canvas = RgbaImage::new(total_w, total_h);

    log::info!(
        "Compositing {} monitor(s) onto a {}x{} canvas at scale {}",
        shots.len(),
        total_w,
        total_h,
        scale,
    );

    for shot in shots {
        // What this monitor must occupy on the canvas. Equal to what was
        // captured unless a denser monitor set the scale, so the resample is
        // skipped outright on any desktop whose displays agree.
        let target_w = ((shot.w as f64) * scale).round() as u32;
        let target_h = ((shot.h as f64) * scale).round() as u32;
        let img = if shot.img.width() == target_w && shot.img.height() == target_h {
            shot.img
        } else {
            log::info!(
                "Rescaling monitor capture from {}x{} to {}x{} for the shared canvas",
                shot.img.width(),
                shot.img.height(),
                target_w,
                target_h,
            );
            DynamicImage::ImageRgba8(shot.img)
                .resize_exact(target_w, target_h, image::imageops::FilterType::Lanczos3)
                .to_rgba8()
        };

        let (offset_x, offset_y) = screen.to_canvas(shot.x, shot.y);
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

/// The cursor arrow that used to be drawn at the click point, as offsets from
/// it at scale 1.
///
/// No longer drawn. It was a filled 15x25 px glyph anchored on the click point,
/// so it sat on top of the control the step exists to identify -- the other
/// half of the 2026-09-03 "the marker covers the button name" finding, and the
/// half a ring alone does not fix. It also drew the same arrow whatever the
/// real cursor was, contradicting the screenshot on a text field or a link.
///
/// Retained because `marker_box_at` is a server contract: the box travels with
/// the step and the server blanks that rectangle out of both frames before
/// comparing them (design D9). Shrinking it to the ring would change what the
/// server masks, so the reported box stays exactly what it was. It is now
/// larger than the drawing, which costs only a slightly blinder near-duplicate
/// comparison -- the same region was masked before, when the arrow was there.
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

/// The ring's stroke, in canvas pixels at scale 1. Drawn as concentric hollow
/// circles inward from the marker radius, so the outer edge -- and therefore
/// `marker_box_at` -- is exactly where the filled disc's edge used to be.
const CLICK_MARKER_STROKE: i32 = 3;

/// Render a click overlay on the screenshot: a red ring around the click point
/// and a white cursor arrow.
///
/// A ring, not a disc. `imageproc`'s primitives write pixels rather than
/// blending them, so the `alpha` in a colour passed to them does nothing: the
/// "semi-transparent" dot this replaces was an opaque 36 px circle centred on
/// exactly the thing the user had just clicked, and it took the button label
/// with it in the 2026-09-03 test. The ring marks the same point and leaves it
/// readable, to a person and to the model.
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

    // Opaque by construction: these primitives do not blend, so an alpha here
    // would be a comment that looks like code.
    let red = Rgba([255, 0, 0, 255]);
    let white = Rgba([255, 255, 255, 255]);
    let radius = scaled_radius(screen.scale);

    // Concentric circles rather than one thick stroke, because imageproc draws
    // a hollow circle one pixel wide. Scaled with the canvas so the ring stays
    // visible on a Retina display instead of thinning to a hairline.
    let stroke = ((CLICK_MARKER_STROKE as f64 * screen.scale).round() as i32).max(1);

    // A white hairline on each edge of the red. Red alone vanishes against red
    // or dark application chrome, and a marker that disappears on some screens
    // is a marker you cannot rely on. Both hairlines sit *inside* `radius`, so
    // the outer edge stays exactly where the filled disc's was and
    // `marker_box_at` keeps reporting the same rectangle.
    let mut draw_ring = |r: i32, colour| {
        if r > 0 {
            draw_hollow_circle_mut(img, (cx, cy), r, colour);
        }
    };
    draw_ring(radius, white);
    for inset in 1..=stroke {
        draw_ring(radius - inset, red);
    }
    draw_ring(radius - stroke - 1, white);

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

    /// A laptop left of an external 4K display, the arrangement in the
    /// 2026-09-03 test. Retina geometry is in points, so 1512x982.
    const LAPTOP_PLUS_4K: [MonitorBounds; 2] = [(0, 0, 1512, 982), (1512, 0, 3840, 2160)];

    /// An external display placed to the *left* of the primary, which is where
    /// negative coordinates come from and where an unsigned-arithmetic mistake
    /// would show up.
    const LEFT_OF_PRIMARY: [MonitorBounds; 2] = [(-1920, 0, 1920, 1080), (0, 0, 1512, 982)];

    #[test]
    fn a_click_picks_the_monitor_it_landed_on() {
        assert_eq!(monitor_for_click(Some((10, 10)), &LAPTOP_PLUS_4K, 0), 0);
        assert_eq!(monitor_for_click(Some((2000, 900)), &LAPTOP_PLUS_4K, 0), 1);
    }

    #[test]
    fn a_monitor_left_of_the_primary_is_found_at_negative_coordinates() {
        assert_eq!(monitor_for_click(Some((-1000, 500)), &LEFT_OF_PRIMARY, 1), 0);
        assert_eq!(monitor_for_click(Some((-1, 0)), &LEFT_OF_PRIMARY, 1), 0);
        assert_eq!(monitor_for_click(Some((0, 0)), &LEFT_OF_PRIMARY, 1), 1);
    }

    /// Boundaries are half-open, so the pixel column where two displays meet
    /// belongs to exactly one of them and no click can match both.
    #[test]
    fn monitor_edges_do_not_overlap() {
        assert_eq!(monitor_for_click(Some((1511, 0)), &LAPTOP_PLUS_4K, 0), 0);
        assert_eq!(monitor_for_click(Some((1512, 0)), &LAPTOP_PLUS_4K, 0), 1);
        assert_eq!(monitor_for_click(Some((1512, 981)), &LAPTOP_PLUS_4K, 0), 1);
    }

    /// A step on an unexpected display beats no step at all, so neither an
    /// absent position nor an off-screen one is an error.
    #[test]
    fn no_position_and_an_off_screen_position_both_fall_back_to_primary() {
        assert_eq!(monitor_for_click(None, &LAPTOP_PLUS_4K, 1), 1);
        assert_eq!(monitor_for_click(Some((99_999, 99_999)), &LAPTOP_PLUS_4K, 1), 1);
        assert_eq!(monitor_for_click(Some((0, -5)), &LAPTOP_PLUS_4K, 0), 0);
    }

    /// The caller distinguishes a real containment from a fallback so the log
    /// can say which happened; `monitor_for_click` collapses the two.
    #[test]
    fn containment_is_reported_separately_from_the_fallback() {
        assert_eq!(monitor_containing(Some((2000, 900)), &LAPTOP_PLUS_4K), Some(1));
        assert_eq!(monitor_containing(Some((99_999, 0)), &LAPTOP_PLUS_4K), None);
        assert_eq!(monitor_containing(None, &LAPTOP_PLUS_4K), None);
    }

    #[test]
    fn a_single_monitor_desktop_always_answers_zero() {
        let one = [(0, 0, 1512, 982)];
        assert_eq!(monitor_for_click(Some((100, 100)), &one, 0), 0);
        assert_eq!(monitor_for_click(None, &one, 0), 0);
        assert_eq!(monitor_for_click(Some((5000, 5000)), &one, 0), 0);
    }

    /// The measurement from the finding, as a regression.
    ///
    /// The upload caps the saved image at 1920x1080. Composited, two 4K
    /// displays are 7680x2160 and each one lands at 960x540 -- the pixelation
    /// that made the model misread an application name. Captured singly, the
    /// same display is 3840x2160 and lands at 1920x1080.
    #[test]
    fn one_4k_monitor_survives_the_upload_cap_that_two_composited_did_not() {
        fn saved_width(canvas_w: u32, canvas_h: u32) -> u32 {
            let (max_w, max_h) = (1920f64, 1080f64);
            if canvas_w as f64 <= max_w && canvas_h as f64 <= max_h {
                return canvas_w;
            }
            let scale = f64::min(max_w / canvas_w as f64, max_h / canvas_h as f64);
            (canvas_w as f64 * scale) as u32
        }

        let composited = saved_width(7680, 2160) / 2;
        let single = saved_width(3840, 2160);

        assert_eq!(composited, 960, "the behaviour being replaced");
        assert!(single >= 1600, "one monitor should survive the cap: {}", single);
        // Exactly double the linear resolution, so four times the pixels on the
        // display the user was actually looking at.
        assert_eq!(single, composited * 2);
    }

    /// A plain canvas whose pixels are all distinguishable from the marker, so
    /// "unchanged" and "drawn on" can be told apart.
    fn blank(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([10, 20, 30, 255]))
    }

    const GROUND: Rgba<u8> = Rgba([10, 20, 30, 255]);
    const MARKER_RED: Rgba<u8> = Rgba([255, 0, 0, 255]);
    const MARKER_WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);

    /// The finding: the marker used to be an opaque disc over exactly the thing
    /// that was clicked, and the button label went with it.
    #[test]
    fn the_clicked_pixel_survives_the_marker() {
        let mut img = blank(400, 400);
        render_click_overlay(&mut img, 200, 200, &screen((0, 0), 1.0));

        assert_eq!(
            *img.get_pixel(200, 200),
            GROUND,
            "the click point must still show what was clicked",
        );
    }

    /// ...but the marker still has to be there. Without this, "leave the centre
    /// alone" is satisfiable by drawing nothing at all.
    #[test]
    fn the_ring_is_drawn_at_the_marker_radius() {
        let mut img = blank(400, 400);
        render_click_overlay(&mut img, 200, 200, &screen((0, 0), 1.0));

        let radius = scaled_radius(1.0) as u32;
        // Outermost pixel is the white hairline; the red stroke is just inside
        // it. Both are part of the ring, and neither is the ground colour.
        assert_eq!(*img.get_pixel(200 + radius, 200), MARKER_WHITE, "halo");
        assert_eq!(*img.get_pixel(200 + radius - 1, 200), MARKER_RED, "red stroke");
    }

    /// The interior between the click point and the ring stays readable too --
    /// a ring that is nearly filled is the old defect with a smaller hole.
    #[test]
    fn the_interior_of_the_ring_is_left_alone() {
        let mut img = blank(400, 400);
        render_click_overlay(&mut img, 200, 200, &screen((0, 0), 1.0));

        let radius = scaled_radius(1.0);
        // Inside the inner white hairline, which sits at radius - stroke - 1.
        for offset in 0..(radius - CLICK_MARKER_STROKE - 2) {
            assert_eq!(
                *img.get_pixel((200 + offset) as u32, 200),
                GROUND,
                "the ring interior was painted over at offset {}",
                offset,
            );
        }
    }

    /// The marker box travels to the server, which blanks that rectangle out of
    /// both frames before comparing them. Changing the drawing must not move
    /// it, so these are the values the filled disc reported.
    #[test]
    fn marker_geometry_is_unchanged_by_the_ring() {
        assert_eq!(
            marker_box_at(100, 100, 1.0),
            MarkerBox { x0: 82, y0: 82, x1: 118, y1: 124 },
        );
        assert_eq!(
            marker_box_at(100, 100, 2.0),
            MarkerBox { x0: 64, y0: 64, x1: 136, y1: 148 },
        );
    }

    /// At scale 2 a one-pixel ring would read as a hairline. The stroke scales
    /// with the canvas, and the outer edge stays on the radius the box is
    /// computed from.
    #[test]
    fn the_ring_thickens_on_a_dense_display_without_moving_its_outer_edge() {
        let mut img = blank(800, 800);
        render_click_overlay(&mut img, 200, 200, &screen((0, 0), 2.0));

        let radius = scaled_radius(2.0) as u32;
        assert_eq!(*img.get_pixel(400 + radius, 400), MARKER_WHITE, "outer halo");
        assert_eq!(
            *img.get_pixel(400 + radius - 5, 400),
            MARKER_RED,
            "a 6 px stroke at scale 2 should still be red 5 px in",
        );
        assert_eq!(*img.get_pixel(400, 400), GROUND, "centre still clear");
    }

    fn screen(origin: (i32, i32), scale: f64) -> VirtualScreen {
        VirtualScreen { origin, scale }
    }

    /// The regression this function exists for. A Windows monitor reports
    /// `dmPelsWidth`, which is already the number of pixels the capture
    /// contains, so its DPI scale must not enter the canvas maths at all. The
    /// previous version multiplied by it and upscaled every capture by 1.5 on a
    /// 150% display, only to resize it back down again on the way to the PNG.
    #[test]
    fn a_windows_monitor_never_rescales_whatever_its_dpi_scale() {
        assert_eq!(canvas_scale(&[(2560, 2560)]), 1.0);
        assert_eq!(canvas_scale(&[(1920, 1920)]), 1.0);
    }

    /// A Retina Mac reports `CGDisplayBounds` in points, half its pixels, and
    /// the canvas must be built at the pixels or three quarters of the desktop
    /// is composited below native resolution.
    #[test]
    fn a_retina_monitor_composites_at_its_own_density() {
        assert_eq!(canvas_scale(&[(1512, 3024)]), 2.0);
    }

    /// A shared canvas can only hold one density, so it takes the highest and
    /// scales the sparser displays up to meet it.
    #[test]
    fn the_densest_monitor_sets_the_scale() {
        assert_eq!(canvas_scale(&[(1512, 3024), (1920, 1920)]), 2.0);
        assert_eq!(canvas_scale(&[(1920, 1920), (1512, 3024)]), 2.0);
    }

    /// Windows reports every monitor in physical pixels, so even a mixed-DPI
    /// desktop is already one uniform pixel space and needs no resample.
    #[test]
    fn a_mixed_dpi_windows_desktop_still_needs_no_resample() {
        assert_eq!(canvas_scale(&[(2560, 2560), (1920, 1920)]), 1.0);
    }

    /// Never below native: a monitor that somehow reports more geometry than it
    /// captured must not drag the whole canvas down with it.
    #[test]
    fn the_canvas_is_never_scaled_below_native() {
        assert_eq!(canvas_scale(&[(3840, 1920)]), 1.0);
        assert_eq!(canvas_scale(&[(0, 2560)]), 1.0);
    }

    /// What the caller actually depends on: on a single-monitor desktop the
    /// target size equals the captured size, so `capture_full_screen` skips the
    /// Lanczos pass entirely. This is the assertion the old code failed on
    /// Windows while its comment claimed otherwise.
    #[test]
    fn a_single_monitor_desktop_is_composited_without_a_resample() {
        for (geometry_w, captured_w) in [(2560u32, 2560u32), (1512, 3024), (1920, 1920)] {
            let scale = canvas_scale(&[(geometry_w, captured_w)]);
            let target = ((geometry_w as f64) * scale).round() as u32;
            assert_eq!(
                target, captured_w,
                "geometry {} capturing {} must need no resize",
                geometry_w, captured_w
            );
        }
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

/// Capture the monitor the action happened on.
///
/// Reads `Monitor::all()` once, so the index `monitor_for_click` returns and
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
            m.x().map_err(|e| format!("Failed to read monitor x position: {}", e))?,
            m.y().map_err(|e| format!("Failed to read monitor y position: {}", e))?,
            m.width().map_err(|e| format!("Failed to read monitor width: {}", e))?,
            m.height().map_err(|e| format!("Failed to read monitor height: {}", e))?,
        ));
        if m.is_primary().unwrap_or(false) {
            primary = i;
        }
    }

    let point = click_position.or_else(super::input_hooks::get_cursor_position);
    let containing = monitor_containing(point, &bounds);
    let chosen = containing.unwrap_or(primary);

    // Logged for a single display too, not only for several. "Which screen did
    // this step come from" is the first question when a guide shows the wrong
    // one, and a line that appears only on multi-monitor machines is missing
    // from exactly the recordings that need it. One line per step, beside the
    // one the save already writes.
    let why = match (containing, point) {
        (Some(_), _) => "",
        (None, None) => " (no position available; fell back to the primary display)",
        (None, Some(_)) => " (point is on no display; fell back to the primary display)",
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
    // Timed and dimensioned on every step, because the one question the
    // 2026-09-03 test could not answer from the logs was which capture failed
    // and how big the canvas was when it did.
    let started = std::time::Instant::now();
    let (mut img, screen) = capture_active_monitor(click_position)?;
    let captured_in = started.elapsed();

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

    log::info!(
        "Screenshot saved: {} (canvas {}x{}, saved {}x{}, capture {} ms, total {} ms)",
        path.display(),
        w,
        h,
        rgb_img.width(),
        rgb_img.height(),
        captured_in.as_millis(),
        started.elapsed().as_millis(),
    );
    Ok(saved_marker)
}
