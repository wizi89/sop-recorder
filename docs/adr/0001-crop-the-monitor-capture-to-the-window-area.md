# Crop the monitor capture to the window area

On Windows a step reads the pixels of the clicked monitor and crops them to the active window area,
rather than capturing the window itself with a per-window API (PrintWindow, Windows.Graphics.Capture).
A per-window capture renders one window alone, so a context menu or dropdown, which is a separate
popup window, would vanish from the very step that shows it being used.

## Consequences

- Anything drawn over the area by another app (a notification, an always-on-top window) is in the
  pixels, so it is blacked out where it overlaps; the content under it is lost for that step.
- The area is the foreground window plus its root owner, and a popup of the same app reaching past
  the window edge; the gap between a dialog and a distant owner is filled, not shown.
- A step only ever shows the monitor the click landed on, which bounds the downscale before upload.
