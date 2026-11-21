//! The part of the monitor a step keeps in the active window mode
//! (privacy-capture D2), and the window geometry it is computed from (D3).
//!
//! Pure on purpose: the Windows reads that fill a `Snapshot` live in
//! `capture::geometry`, so every rule here is tested on any platform against
//! desktops written out by hand.

/// A rectangle in desktop coordinates (physical pixels on Windows), right and
/// bottom exclusive, as `RECT` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    pub fn contains(&self, (x, y): (i32, i32)) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let r = Rect::new(
            self.left.max(other.left),
            self.top.max(other.top),
            self.right.min(other.right),
            self.bottom.min(other.bottom),
        );
        (!r.is_empty()).then_some(r)
    }

    /// What is left of `self` once `cut` is taken out: up to four pieces.
    pub fn subtract(&self, cut: &Rect) -> Vec<Rect> {
        let Some(hole) = self.intersect(cut) else {
            return vec![*self];
        };
        [
            Rect::new(self.left, self.top, self.right, hole.top),
            Rect::new(self.left, hole.bottom, self.right, self.bottom),
            Rect::new(self.left, hole.top, hole.left, hole.bottom),
            Rect::new(hole.right, hole.top, self.right, hole.bottom),
        ]
        .into_iter()
        .filter(|r| !r.is_empty())
        .collect()
    }

    pub fn bounding(&self, other: &Rect) -> Rect {
        Rect::new(
            self.left.min(other.left),
            self.top.min(other.top),
            self.right.max(other.right),
            self.bottom.max(other.bottom),
        )
    }
}

/// One visible top-level window, as the geometry snapshot saw it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WindowInfo {
    pub hwnd: isize,
    /// DWM extended frame bounds: the window as drawn, without the invisible
    /// resize border.
    pub rect: Rect,
    pub class: String,
    pub pid: u32,
    /// `WS_POPUP`: menus, dropdowns and other windows drawn over their owner.
    pub popup: bool,
    /// `GetAncestor(GA_ROOTOWNER)`; the window itself when it has no owner.
    pub root_owner: isize,
    /// Not drawn at all (another virtual desktop, a suspended app).
    pub cloaked: bool,
    /// `WS_EX_LAYERED` or `WS_EX_TRANSPARENT`: may let what is below show
    /// through, so it never counts as hiding anything (D4).
    pub layered: bool,
    pub transparent: bool,
    /// `WS_EX_TRANSPARENT` alone: a click-through overlay (a dictation bar),
    /// never masked as a foreign window. Unlike `transparent`, not set
    /// when a window's state cannot be read.
    pub click_through: bool,
    /// Display affinity other than `WDA_NONE`: absent from the pixels.
    pub excluded: bool,
}

impl Default for Rect {
    fn default() -> Self {
        Rect::new(0, 0, 0, 0)
    }
}

/// Foreground window, window under the click and every visible top-level
/// window, top of the z-order first (D3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    pub foreground: Option<isize>,
    /// The root window under the click; `None` for a key press.
    pub under_click: Option<isize>,
    pub windows: Vec<WindowInfo>,
    /// The list came from `EnumWindows`, which leaves out shell windows such
    /// as toasts: they are neither masked nor checked, so the step is flagged.
    pub partial: bool,
}

impl Snapshot {
    pub fn get(&self, hwnd: isize) -> Option<&WindowInfo> {
        self.windows.iter().find(|w| w.hwnd == hwnd)
    }
}

/// The two snapshots taken either side of the pixel read, as the password
/// check needs them (D3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Geometry {
    /// Before and after. Visibility is worked out in each and combined: a
    /// window counts where it is visible in either, because the pixels may
    /// show either moment.
    pub snapshots: [Snapshot; 2],
    /// Activation completed during the pixel read, so the pixels may show
    /// either foreground window. Flags the step when masking is on.
    pub foreground_changed: bool,
}

pub fn merge(before: &Snapshot, after: &Snapshot) -> Geometry {
    Geometry {
        foreground_changed: before.foreground != after.foreground,
        snapshots: [before.clone(), after.clone()],
    }
}

/// Whether a window certainly hides what is below it (D4). Anything that may
/// let the lower window show through counts as not hiding it, so an error
/// here over-masks rather than leaks.
fn hides(w: &WindowInfo) -> bool {
    !w.cloaked && !w.layered && !w.transparent && !w.excluded
}

/// Whether a window's pixels can be in the screenshot at all: a cloaked window
/// is not drawn and a window excluded from capture (the recording bar) is not
/// in the pixels, so neither is checked.
fn drawn(w: &WindowInfo) -> bool {
    !w.cloaked && !w.excluded
}

/// Each drawn window with its visible part inside `shown`: its rectangle minus
/// every window above it that certainly hides it (D4). Windows with no visible
/// part are left out.
fn visible_in(snapshot: &Snapshot, shown: &[Rect]) -> Vec<(isize, Vec<Rect>)> {
    let mut out = Vec::new();
    for (i, w) in snapshot.windows.iter().enumerate() {
        if !drawn(w) {
            continue;
        }
        let mut parts: Vec<Rect> = shown.iter().filter_map(|s| w.rect.intersect(s)).collect();
        for above in snapshot.windows[..i].iter().filter(|a| hides(a)) {
            parts = parts.iter().flat_map(|p| p.subtract(&above.rect)).collect();
        }
        // A `#32768` menu still hides what is below it, but holds no input
        // field, and is usually closed by the click before the check runs.
        if !parts.is_empty() && w.class != "#32768" {
            out.push((w.hwnd, parts));
        }
    }
    out
}

/// The windows the password check covers and where each is visible in the
/// saved area, top of the z-order first (D4). Visible in either snapshot
/// counts as visible (D3).
pub fn visible_windows(geometry: &Geometry, shown: &[Rect]) -> Vec<(isize, Vec<Rect>)> {
    let mut out = visible_in(&geometry.snapshots[0], shown);
    for (hwnd, parts) in visible_in(&geometry.snapshots[1], shown) {
        match out.iter_mut().find(|(h, _)| *h == hwnd) {
            Some((_, seen)) => {
                for part in parts {
                    if !seen.contains(&part) {
                        seen.push(part);
                    }
                }
            }
            None => out.push((hwnd, parts)),
        }
    }
    out
}

/// The parts of `fields` that are visible: masked only there, so a password
/// field of a covered window does not black out the window in front (D4).
pub fn visible_fields(fields: &[Rect], visible: &[Rect]) -> Vec<Rect> {
    fields
        .iter()
        .flat_map(|f| visible.iter().filter_map(|v| f.intersect(v)))
        .collect()
}

/// What a step keeps of its monitor.
#[derive(Debug, Clone, PartialEq)]
pub enum Area {
    /// Crop to `crop`; pixels inside it but outside every `keep` rectangle are
    /// filled, so the gap between a dialog and a distant owner shows nothing.
    /// `windows` are the step's own windows, for `foreign_rects`.
    Window {
        crop: Rect,
        keep: Vec<Rect>,
        windows: Vec<isize>,
    },
    /// No usable window: the whole monitor is kept and the step is marked.
    Fallback(&'static str),
}

#[derive(Debug, PartialEq)]
enum Kind {
    /// `Progman` / `WorkerW`: counts as no window at all.
    Desktop,
    /// Taskbar, Start, tray flyouts: captured alone when clicked.
    Shell,
    App,
}

const DESKTOP_CLASSES: [&str; 2] = ["Progman", "WorkerW"];
const SHELL_CLASSES: [&str; 7] = [
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
    "NotifyIconOverflowWindow",
    "TopLevelWindowForOverflowXamlIsland",
    "Shell_InputSwitchTopLevelWindow",
];

fn kind(w: &WindowInfo) -> Kind {
    if DESKTOP_CLASSES.contains(&w.class.as_str()) {
        Kind::Desktop
    } else if SHELL_CLASSES.contains(&w.class.as_str()) {
        Kind::Shell
    } else {
        Kind::App
    }
}

/// A context menu, or a popup of the foreground window's process reaching past
/// its edge (a dropdown): shown together with the window it belongs to.
fn belongs_to(popup: &WindowInfo, fg: &WindowInfo) -> bool {
    popup.class == "#32768" || (popup.popup && popup.pid == fg.pid && popup.hwnd != fg.hwnd)
}

/// The area a step keeps in the active window mode (D2).
///
/// The foreground window and its root owner, unless the click lands outside
/// both: then the root window under the click decides. A click fires before
/// its own activation, so clicking a background window has to capture that
/// window, not the one about to lose the foreground.
/// Whether every kept rectangle lies inside a shell window (taskbar, Start,
/// flyouts). Such an area holds no app window, so a foreground change during
/// the read cannot put one in it; whatever is drawn over it is in the
/// snapshots and checked anyway (8.1 run: every app-switching taskbar click
/// was flagged).
pub fn shell_only(snapshot: &Snapshot, keep: &[Rect]) -> bool {
    !keep.is_empty()
        && keep.iter().all(|k| {
            snapshot
                .windows
                .iter()
                .any(|w| !w.cloaked && kind(w) == Kind::Shell && w.rect.intersect(k) == Some(*k))
        })
}

/// Where a key press happened, for choosing the monitor to read: the middle of
/// the foreground window. A key press has no position of its own, and the
/// cursor can rest on another monitor than the window being typed in; the area
/// is the foreground window, so its monitor is the one to read. `None` (use
/// the cursor) when there is no drawn foreground window or it is the desktop.
pub fn key_press_point(snapshot: &Snapshot) -> Option<(i32, i32)> {
    let w = snapshot
        .foreground
        .and_then(|h| snapshot.get(h))
        .filter(|w| !w.cloaked && !w.rect.is_empty() && kind(w) != Kind::Desktop)?;
    Some((
        w.rect.left + (w.rect.right - w.rect.left) / 2,
        w.rect.top + (w.rect.bottom - w.rect.top) / 2,
    ))
}

pub fn capture_area<'a>(snapshot: &'a Snapshot, click: Option<(i32, i32)>, monitor: Rect) -> Area {
    // A cloaked window is in the list but never drawn (a closed Start menu,
    // another virtual desktop), so it can be no step's window.
    let drawn = |hwnd: isize| snapshot.get(hwnd).filter(|w| !w.cloaked);
    let with_owner = |w: &'a WindowInfo| -> Vec<&'a WindowInfo> {
        let mut rects = vec![w];
        if let Some(owner) = drawn(w.root_owner).filter(|o| o.hwnd != w.hwnd) {
            rects.push(owner);
        }
        rects
    };
    let window_rects = |w: &'a WindowInfo| -> Option<Vec<&'a WindowInfo>> {
        match kind(w) {
            Kind::Desktop => None,
            Kind::Shell => Some(vec![w]),
            Kind::App => Some(with_owner(w)),
        }
    };

    let fg = snapshot.foreground.and_then(drawn);
    let fg_rects = fg.and_then(window_rects);

    let rects = match click {
        None => match fg_rects {
            Some(rects) => rects,
            None => return Area::Fallback("no foreground window"),
        },
        Some(point) if fg_rects.iter().flatten().any(|w| w.rect.contains(point)) => {
            fg_rects.unwrap()
        }
        Some(_) => {
            let Some(under) = snapshot.under_click.and_then(drawn) else {
                return Area::Fallback("no window under the click");
            };
            match (fg, &fg_rects) {
                (Some(fg), Some(fg_rects)) if kind(under) == Kind::App && belongs_to(under, fg) => {
                    let mut rects = fg_rects.clone();
                    rects.push(under);
                    rects
                }
                _ => match window_rects(under) {
                    Some(rects) => rects,
                    None => return Area::Fallback("click on the desktop"),
                },
            }
        }
    };

    let keep: Vec<Rect> = rects
        .iter()
        .filter_map(|w| w.rect.intersect(&monitor))
        .collect();
    let Some(crop) = keep.iter().copied().reduce(|a, b| a.bounding(&b)) else {
        return Area::Fallback("window is not on the captured monitor");
    };
    Area::Window {
        crop,
        keep,
        windows: rects.iter().map(|w| w.hwnd).collect(),
    }
}

/// Where other apps' windows cover the step's windows: toasts, the
/// notification center, always-on-top windows, an app overlapping the owner.
/// They are in the pixels, so these rectangles are blacked out.
///
/// A window counts when it is drawn, above a step window in the z-order and
/// overlaps it, in either snapshot (it may appear or grow during the read).
/// Not counted: the step's own windows, windows they own, their menus and
/// dropdowns (`belongs_to`), the window under the click (the user is operating
/// it), the desktop, and click-through overlays (`WS_EX_TRANSPARENT`, such as
/// dictation bars), which owners chose to keep visible.
pub fn foreign_rects(snapshots: &[&Snapshot], step: &[isize]) -> Vec<Rect> {
    let mut out: Vec<Rect> = Vec::new();
    for snapshot in snapshots {
        let own: Vec<(usize, &WindowInfo)> = snapshot
            .windows
            .iter()
            .enumerate()
            .filter(|(_, w)| step.contains(&w.hwnd))
            .collect();
        for (i, w) in snapshot.windows.iter().enumerate() {
            let foreign = drawn(w)
                && !w.click_through
                && kind(w) != Kind::Desktop
                && snapshot.under_click != Some(w.hwnd)
                && !own.iter().any(|(_, s)| {
                    s.hwnd == w.hwnd
                        || w.root_owner == s.hwnd
                        || (s.root_owner != 0 && w.root_owner == s.root_owner)
                        || belongs_to(w, s)
                });
            if !foreign {
                continue;
            }
            // Step windows above `w` are drawn over it, so they stay visible.
            let above: Vec<Rect> = own
                .iter()
                .filter(|(j, _)| *j < i)
                .map(|(_, s)| s.rect)
                .collect();
            for (_, s) in own.iter().filter(|(j, _)| *j > i) {
                let Some(r) = w.rect.intersect(&s.rect) else {
                    continue;
                };
                let mut pieces = vec![r];
                for cut in &above {
                    pieces = pieces.iter().flat_map(|p| p.subtract(cut)).collect();
                }
                for p in pieces {
                    if !out.contains(&p) {
                        out.push(p);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
