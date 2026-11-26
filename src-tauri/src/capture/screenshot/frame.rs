//! What a step keeps of its pixels: the capture scope, the crop and the other
//! apps' windows to black out. Pure, so it is tested without a display.

use super::window_area::{self, Area};
use super::{crop_to, monitor_rect, HeldStep, PixelRead};
use crate::output::step_meta::CaptureScope;

/// Phase A's decisions, after `capture_step`'s reads (D2, D3): the area the
/// step keeps in the active window mode, cropped at once, and the foreign
/// windows from both snapshots.
pub fn frame_step(
    read: PixelRead,
    step_number: u32,
    click_position: Option<(i32, i32)>,
    monitor_mode: bool,
) -> HeldStep {
    let PixelRead {
        img,
        screen,
        before,
        after,
        pixels_at,
    } = read;
    let geometry = before
        .as_ref()
        .zip(after.as_ref())
        .map(|(b, a)| window_area::merge(b, a));

    let monitor = monitor_rect(&screen, &img);
    let area = (!monitor_mode).then(|| match &before {
        Some(snapshot) => window_area::capture_area(snapshot, click_position, monitor),
        None => Area::Fallback("no window geometry on this platform"),
    });

    // One line per step saying what it kept (task 3.6): the first question
    // when a guide shows too little or too much.
    let (img, screen, keep, foreign, scope) = match area {
        None => {
            log::info!("Step {step_number}: capture monitor {monitor:?}");
            (img, screen, Vec::new(), Vec::new(), CaptureScope::Monitor)
        }
        Some(Area::Fallback(reason)) => {
            log::warn!(
                "Step {step_number}: capture active_window fell back to monitor {monitor:?}: {reason}"
            );
            (
                img,
                screen,
                Vec::new(),
                Vec::new(),
                CaptureScope::MonitorFallback,
            )
        }
        Some(Area::Window {
            crop,
            keep,
            windows,
        }) => match crop_to(&img, screen, &crop) {
            // Cropped at once, so what phase B holds is the area, not the monitor.
            Some((cropped, cropped_screen)) => {
                // Both snapshots: a toast may appear or grow during the read.
                let snapshots: Vec<_> = before.iter().chain(after.iter()).collect();
                let foreign = window_area::foreign_rects(&snapshots, &windows);
                log::info!(
                    "Step {step_number}: capture active_window, area {crop:?} from {} window rectangle(s), \
                     {} foreign window area(s) masked",
                    keep.len(),
                    foreign.len()
                );
                (cropped, cropped_screen, keep, foreign, CaptureScope::Window)
            }
            None => {
                log::warn!(
                    "Step {step_number}: capture active_window fell back to monitor {monitor:?}: \
                     area {crop:?} is not in the captured pixels"
                );
                (
                    img,
                    screen,
                    Vec::new(),
                    Vec::new(),
                    CaptureScope::MonitorFallback,
                )
            }
        },
    };
    HeldStep {
        img,
        screen,
        keep,
        foreign,
        scope,
        geometry,
        pixels_at,
    }
}
