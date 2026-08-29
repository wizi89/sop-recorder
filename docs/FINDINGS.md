# Findings

Hard-won, non-obvious facts about the recorder. Each entry cost real debugging time.
Read this before diagnosing anything in capture, window management, or permissions.

**Adding entries:** newest at the top. One entry per finding. Keep the shape
`Symptom / Cause / Rule`, because the symptom is what a future reader will search for.
Only add things that were genuinely surprising: if it is obvious from the code, skip it.

---

## 2026-08-29 xcap reports monitor geometry in different units per platform, and nothing says so

**Symptom:** On a single-monitor Windows laptop at 150% display scaling, every step logged

```
Rescaling monitor capture from 2560x1440 to 3840x2160 for the shared canvas
Resizing screenshot from 3840x2160 to 1920x1080
```

The first line should never appear on a single-monitor desktop, and the code comment directly above
it says as much: the resample "is skipped on the overwhelmingly common single-monitor desktop rather
than paying for a no-op resample". Screenshots still came out at the right size with the click
marker in the right place, so nothing looked broken. What it cost was sharpness (an upscale followed
by a downscale is softer than the single downscale it replaced), three times the canvas memory, and
about a second of wall clock on every captured step. At 100% scaling the scale factor is 1.0 and the
whole thing disappears, so a developer machine at 100% sees nothing at all.

**Cause:** `capture_full_screen` sized each monitor's slot on the shared canvas as
`Monitor::width() * max_scale_factor`. `xcap` does not report `width()` in the same units on every
platform, and its API gives no hint of this:

| Platform | `Monitor::width()` | Source |
|---|---|---|
| macOS | logical points | `CGDisplayBounds` |
| Windows | physical pixels | `dmPelsWidth` (`EnumDisplaySettings`) |

On macOS the multiplication is correct and necessary: 1512 points x 2.0 = 3024 pixels, which is
exactly what `capture_image` returns, so the resize is skipped. On Windows the geometry is *already*
physical, so multiplying by 1.5 asked for 3839x2160 from a capture that was already 2560x1440. The
mixed-DPI canvas logic was written and tested on a Mac, where it is right, and the same expression
is wrong on Windows for a reason no type or name exposes. Note the target was 3839, not 3840: the
reported scale factor is 1.4997071, so the old path also forced a non-integer resample.

The misleading part is that the result stays *geometrically* correct. Monitor origins, the cursor
position, and the click marker are all scaled by the same factor and the final resize to 1920x1080
normalises everything, so the marker lands within a pixel of where it should. Only the log line and
the step latency give it away.

**Rule:** never combine `Monitor::scale_factor()` with `Monitor::width()/height()/x()/y()` and
assume you know what space the result is in. Derive the density from the captures instead:
`capture_image().width() / Monitor::width()` is the pixels-per-geometry-unit ratio, it needs no
per-platform branch, and it collapses to 1.0 on Windows and 2.0 on a Retina Mac. That is what
`canvas_scale` in `capture/screenshot.rs` now does, guarded by tests that assert a single-monitor
desktop needs no resample on either platform. More generally: this module has two coordinate spaces
that coincide on Windows and do not on macOS, so a change that is verified on one platform is not
verified. The log line is the cheap check. If "Rescaling monitor capture" appears on a
single-monitor machine, the units are wrong again.
