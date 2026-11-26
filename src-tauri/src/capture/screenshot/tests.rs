use super::draw::marker_box_at;
use super::*;
use image::Rgba;

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

/// The password check reads `shown()`: a foreign window's area is cut
/// out of the kept window, since it is blacked out anyway.
#[test]
fn shown_leaves_out_the_foreign_windows() {
    let held = HeldStep {
        img: RgbaImage::new(10, 10),
        screen: VirtualScreen {
            origin: (0, 0),
            scale: 1.0,
        },
        keep: vec![Rect::new(0, 0, 100, 100)],
        foreign: vec![Rect::new(50, 50, 100, 100)],
        scope: CaptureScope::Window,
        geometry: None,
        pixels_at: std::time::Instant::now(),
    };
    let shown = held.shown();
    let area: i32 = shown
        .iter()
        .map(|r| (r.right - r.left) * (r.bottom - r.top))
        .sum();
    assert_eq!(area, 100 * 100 - 50 * 50);
    assert!(shown
        .iter()
        .all(|r| r.intersect(&Rect::new(50, 50, 100, 100)).is_none()));
}

#[test]
fn a_click_picks_the_monitor_it_landed_on() {
    assert_eq!(
        choose_monitor(Some((10, 10)), &LAPTOP_PLUS_4K, 0),
        (0, None)
    );
    assert_eq!(
        choose_monitor(Some((2000, 900)), &LAPTOP_PLUS_4K, 0),
        (1, None)
    );
}

#[test]
fn a_monitor_left_of_the_primary_is_found_at_negative_coordinates() {
    assert_eq!(
        choose_monitor(Some((-1000, 500)), &LEFT_OF_PRIMARY, 1),
        (0, None)
    );
    assert_eq!(
        choose_monitor(Some((-1, 0)), &LEFT_OF_PRIMARY, 1),
        (0, None)
    );
    assert_eq!(choose_monitor(Some((0, 0)), &LEFT_OF_PRIMARY, 1), (1, None));
}

/// Boundaries are half-open, so the pixel column where two displays meet
/// belongs to exactly one of them and no click can match both.
#[test]
fn monitor_edges_do_not_overlap() {
    assert_eq!(
        choose_monitor(Some((1511, 0)), &LAPTOP_PLUS_4K, 0),
        (0, None)
    );
    assert_eq!(
        choose_monitor(Some((1512, 0)), &LAPTOP_PLUS_4K, 0),
        (1, None)
    );
    assert_eq!(
        choose_monitor(Some((1512, 981)), &LAPTOP_PLUS_4K, 0),
        (1, None)
    );
}

/// A step on an unexpected display beats no step at all, so neither an
/// absent position nor an off-screen one is an error; the log says which
/// fallback it was.
#[test]
fn no_position_and_an_off_screen_position_both_fall_back_to_primary() {
    assert_eq!(
        choose_monitor(None, &LAPTOP_PLUS_4K, 1),
        (1, Some(Fallback::NoPosition))
    );
    assert_eq!(
        choose_monitor(Some((99_999, 99_999)), &LAPTOP_PLUS_4K, 1),
        (1, Some(Fallback::OffScreen))
    );
    assert_eq!(
        choose_monitor(Some((0, -5)), &LAPTOP_PLUS_4K, 0),
        (0, Some(Fallback::OffScreen))
    );
}

/// `follow_click` needs only the containment, without the fallback.
#[test]
fn containment_is_reported_separately_from_the_fallback() {
    assert_eq!(
        monitor_containing(Some((2000, 900)), &LAPTOP_PLUS_4K),
        Some(1)
    );
    assert_eq!(monitor_containing(Some((99_999, 0)), &LAPTOP_PLUS_4K), None);
    assert_eq!(monitor_containing(None, &LAPTOP_PLUS_4K), None);
}

#[test]
fn a_single_monitor_desktop_always_answers_zero() {
    let one = [(0, 0, 1512, 982)];
    assert_eq!(choose_monitor(Some((100, 100)), &one, 0), (0, None));
    assert_eq!(
        choose_monitor(None, &one, 0),
        (0, Some(Fallback::NoPosition))
    );
    assert_eq!(
        choose_monitor(Some((5000, 5000)), &one, 0),
        (0, Some(Fallback::OffScreen))
    );
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
    assert!(
        single >= 1600,
        "one monitor should survive the cap: {}",
        single
    );
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
/// target size equals the captured size, so `capture_one_monitor` skips the
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

// ------------------------------------------------------------- task 3.5

/// A 4K monitor whose clicked control is a solid green block, cropped to
/// a window, finished and downsized as phase B does.
fn finish_cropped(
    click: (i32, i32),
    window: Rect,
) -> (DynamicImage, Option<MarkerBox>, VirtualScreen) {
    let mut monitor = blank(3840, 2160);
    for y in click.1 - 12..click.1 + 12 {
        for x in click.0 - 12..click.0 + 12 {
            monitor.put_pixel(x as u32, y as u32, GREEN);
        }
    }
    let (img, screen) = crop_to(&monitor, screen((0, 0), 1.0), &window).unwrap();
    let (out, marker) = finish_image(img, screen, &[window], &[], Some(click));
    (out, marker, screen)
}

const GREEN: Rgba<u8> = Rgba([0, 200, 0, 255]);

#[test]
fn the_clicked_pixel_survives_the_crop_and_downsize() {
    // 2400x1600 window: downsized by 1080/1600 on the way to the PNG.
    let window = Rect::new(1000, 300, 3400, 1900);
    let click = (2000, 1000);
    let (out, _, _) = finish_cropped(click, window);

    let scale = f64::min(1920.0 / 2400.0, 1080.0 / 1600.0);
    let saved = (
        ((click.0 - window.left) as f64 * scale) as u32,
        ((click.1 - window.top) as f64 * scale) as u32,
    );
    let pixel = out.to_rgba8().get_pixel(saved.0, saved.1).0;
    assert!(
        pixel[1] > 150 && pixel[0] < 60,
        "the click lands on {pixel:?}, not the green control"
    );
}

/// `marker_box` is in saved-image pixels: translated by the crop offset,
/// then scaled by the downsize. The click itself stays in desktop units.
#[test]
fn the_marker_box_follows_the_crop_and_downsize() {
    let window = Rect::new(1000, 300, 3400, 1900);
    let click = (2000, 1000);
    let (out, marker, screen) = finish_cropped(click, window);
    assert_eq!(screen.origin, (1000, 300));

    let scale = f64::min(1920.0 / 2400.0, 1080.0 / 1600.0);
    let expected = marker_box_at(click.0 - window.left, click.1 - window.top, 1.0)
        .scaled(scale)
        .clamped(out.width(), out.height());
    assert_eq!(marker, expected);
}

#[test]
fn a_small_window_is_saved_at_its_own_size() {
    let window = Rect::new(100, 100, 900, 700);
    let (out, marker, _) = finish_cropped((300, 300), window);
    assert_eq!((out.width(), out.height()), (800, 600));
    assert_eq!(marker, marker_box_at(200, 200, 1.0).clamped(800, 600));
}

/// Pixels between a dialog and its owner are inside the crop but in
/// neither window: filled, never shown.
#[test]
fn the_gap_between_two_kept_windows_is_filled() {
    let monitor = blank(1920, 1080);
    let owner = Rect::new(100, 100, 600, 500);
    let dialog = Rect::new(1200, 600, 1600, 900);
    let crop = owner.bounding(&dialog);
    let (img, screen) = crop_to(&monitor, screen((0, 0), 1.0), &crop).unwrap();
    let (out, _) = finish_image(img, screen, &[owner, dialog], &[], None);
    let out = out.to_rgba8();

    assert_eq!(*out.get_pixel(50, 50), GROUND, "inside the owner");
    assert_eq!(*out.get_pixel(1200, 600), GROUND, "inside the dialog");
    assert_eq!(*out.get_pixel(800, 200), MASK, "the gap");
}

#[test]
fn a_mask_is_filled_in_the_cropped_image_at_its_desktop_position() {
    let monitor = blank(1920, 1080);
    let window = Rect::new(500, 200, 1300, 800);
    let field = Rect::new(700, 400, 900, 430);
    let (img, screen) = crop_to(&monitor, screen((0, 0), 1.0), &window).unwrap();
    let (out, _) = finish_image(img, screen, &[window], &[field], None);
    let out = out.to_rgba8();

    assert_eq!(
        *out.get_pixel(200, 200),
        MASK,
        "field's top-left, crop-relative"
    );
    assert_eq!(*out.get_pixel(399, 229), MASK, "field's bottom-right");
    assert_eq!(*out.get_pixel(400, 230), GROUND, "just outside the field");
}

/// A monitor left of the primary has a negative origin; the crop offset
/// has to be taken from it, not from zero.
#[test]
fn a_crop_on_a_monitor_left_of_the_primary_keeps_the_desktop_origin() {
    let monitor = blank(1920, 1080);
    let window = Rect::new(-1500, 100, -500, 900);
    let (img, cropped) = crop_to(&monitor, screen((-1920, 0), 1.0), &window).unwrap();
    assert_eq!((img.width(), img.height()), (1000, 800));
    assert_eq!(cropped.origin, (-1500, 100));
    assert_eq!(cropped.to_canvas(-1500, 100), (0, 0));
}

/// Captures the real desktop through both phases:
/// `SPIKE_OUT=<dir> cargo test --lib live_two_phase -- --ignored --nocapture`.
#[test]
#[ignore]
fn live_two_phase() {
    let dir = std::path::PathBuf::from(std::env::var("SPIKE_OUT").expect("SPIKE_OUT"));
    for (step, monitor_mode) in [(1, false), (2, true)] {
        let read = capture_step(step, None, monitor_mode).unwrap();
        let held = frame_step(read, step, None, monitor_mode);
        println!("step {step}: scope {:?}", held.scope);
        finish_and_save(held, &dir, step, None, &[]).unwrap();
    }
}

mod drawing;
mod frame;
