use super::corner;

#[test]
fn at_100_percent_the_bar_sits_12_pixels_in_from_the_bottom_right() {
    assert_eq!(corner((0, 0, 1920, 1040), (240, 34), 1.0), (1668, 994));
}

#[test]
fn at_150_percent_the_margin_scales_with_the_display() {
    assert_eq!(corner((0, 0, 2880, 1560), (360, 51), 1.5), (2502, 1491));
}

#[test]
fn a_taskbar_on_the_top_edge_offsets_the_work_area_down() {
    assert_eq!(corner((0, 48, 1920, 1032), (240, 34), 1.0), (1668, 1034));
}

#[test]
fn a_taskbar_on_the_left_edge_offsets_the_work_area_right() {
    assert_eq!(corner((60, 0, 1860, 1080), (240, 34), 1.0), (1668, 1034));
}

#[test]
fn a_monitor_left_of_the_primary_has_a_negative_origin() {
    assert_eq!(corner((-1920, 0, 1920, 1040), (240, 34), 1.0), (-252, 994));
}

#[cfg(target_os = "macos")]
mod overlay {
    use super::super::overlay_behavior;
    use objc2_app_kit::NSWindowCollectionBehavior as B;

    #[test]
    fn the_three_overlay_flags_are_added() {
        let got = overlay_behavior(B::empty());
        assert!(got.contains(B::CanJoinAllSpaces));
        assert!(got.contains(B::FullScreenAuxiliary));
        assert!(got.contains(B::Stationary));
    }

    #[test]
    fn flags_the_window_already_had_survive() {
        let got = overlay_behavior(B::FullScreenPrimary);
        assert!(got.contains(B::FullScreenPrimary));
        assert!(got.contains(B::CanJoinAllSpaces));
    }
}
