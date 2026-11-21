//! What is drawn onto a screenshot: the click marker and the opaque fill
//! for masks and review-screen boxes.

use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_hollow_circle_mut;
use serde::{Deserialize, Serialize};

use super::VirtualScreen;

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
    pub(super) fn scaled(self, scale: f64) -> Self {
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
    pub(super) fn clamped(self, width: u32, height: u32) -> Option<Self> {
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
/// Larger than the ring that is actually drawn: it still spans the arrow's old
/// extent, because the box is a server contract and shrinking it would change
/// what the server masks. See `ARROW_OFFSETS`.
pub(super) fn marker_box_at(cx: i32, cy: i32, scale: f64) -> MarkerBox {
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

pub(super) fn scaled_radius(scale: f64) -> i32 {
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
pub(super) const CLICK_MARKER_STROKE: i32 = 3;

/// Render a click overlay on the screenshot: a red-and-white ring around the
/// click point. No cursor arrow -- see `ARROW_OFFSETS` for why it went.
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

/// What fills the gap around a dialog, every masked password field and every
/// box the user drew on the review screen.
pub(super) const MASK: Rgba<u8> = Rgba([0, 0, 0, 255]);

/// A rectangle in an image's own pixels: `[x, y, width, height]`. Signed, so a
/// box stored partly outside the image still reads and is clipped here.
pub type PixelBox = [i64; 4];

/// Fill every box with the opaque mask colour, clipped to the image
/// (review-redaction D3). The one fill both the capture-time password masks
/// and the review screen's boxes go through, so the two cannot drift apart.
pub fn apply_boxes(img: &mut RgbaImage, boxes: &[PixelBox]) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    for &[x, y, bw, bh] in boxes {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = (x.saturating_add(bw).min(w), y.saturating_add(bh).min(h));
        for py in y0..y1 {
            for px in x0..x1 {
                img.put_pixel(px as u32, py as u32, MASK);
            }
        }
    }
}
