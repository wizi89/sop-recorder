//! The box fill and the click marker as drawn.

use super::*;
use crate::capture::screenshot::draw::{scaled_radius, CLICK_MARKER_STROKE};

fn masked(img: &RgbaImage) -> Vec<(u32, u32)> {
    img.enumerate_pixels()
        .filter(|(_, _, p)| **p == MASK)
        .map(|(x, y, _)| (x, y))
        .collect()
}

#[test]
fn a_box_inside_the_image_fills_exactly_its_pixels() {
    let mut img = blank(10, 10);
    apply_boxes(&mut img, &[[2, 3, 4, 2]]);
    let filled = masked(&img);
    assert_eq!(filled.len(), 8);
    assert!(filled
        .iter()
        .all(|&(x, y)| (2..6).contains(&x) && (3..5).contains(&y)));
}

#[test]
fn a_box_partly_outside_is_clipped_to_the_image() {
    let mut img = blank(10, 10);
    apply_boxes(&mut img, &[[-3, 8, 5, 10]]);
    let filled = masked(&img);
    // x 0..2, y 8..10
    assert_eq!(filled.len(), 4);
    assert!(filled.iter().all(|&(x, y)| x < 2 && y >= 8));
}

#[test]
fn a_box_fully_outside_changes_nothing() {
    let mut img = blank(10, 10);
    apply_boxes(&mut img, &[[20, 20, 5, 5], [-10, 0, 5, 5], [0, 0, 0, 5]]);
    assert!(masked(&img).is_empty());
}

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
    assert_eq!(
        *img.get_pixel(200 + radius - 1, 200),
        MARKER_RED,
        "red stroke"
    );
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
        MarkerBox {
            x0: 82,
            y0: 82,
            x1: 118,
            y1: 124
        },
    );
    assert_eq!(
        marker_box_at(100, 100, 2.0),
        MarkerBox {
            x0: 64,
            y0: 64,
            x1: 136,
            y1: 148
        },
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
    assert_eq!(
        *img.get_pixel(400 + radius, 400),
        MARKER_WHITE,
        "outer halo"
    );
    assert_eq!(
        *img.get_pixel(400 + radius - 5, 400),
        MARKER_RED,
        "a 6 px stroke at scale 2 should still be red 5 px in",
    );
    assert_eq!(*img.get_pixel(400, 400), GROUND, "centre still clear");
}
