//! UI Automation behind the password check on Windows (privacy-capture D4,
//! D5, amendment A1): the query, and the warm-up triggers of a recording.

use std::sync::{mpsc, Arc, Mutex, OnceLock};

use windows::core::{implement, Interface, Result, VARIANT};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThreadId, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::password_check::{Checker, FieldQuery, QueryResult};
use super::window_area::Rect;

struct Uia {
    uia: IUIAutomation,
    is_password: IUIAutomationCondition,
}

// SAFETY: created on an MTA thread and only ever used from MTA threads
// (`client` joins the caller to the MTA first). UIA client objects in the MTA
// are free-threaded.
unsafe impl Send for Uia {}
unsafe impl Sync for Uia {}

/// The process's one UIA client and the timeout it is set to. Shared rather
/// than one per thread: creating clients on many threads at once fails with
/// E_FAIL, which is what warming up every window at recording start did on a
/// live desktop.
static CLIENT: Mutex<Option<(Arc<Uia>, u32)>> = Mutex::new(None);

/// Join the calling thread to the MTA and hand out the shared client, created
/// on first use, with UIA's own timeouts set to the step budget so a hung
/// provider gives up on its side too.
fn client(timeout_ms: u32) -> Result<Arc<Uia>> {
    unsafe {
        // UIA reports field rectangles in the caller's DPI space and DWM
        // reports windows in physical pixels. They only agree in a per-monitor
        // aware process (main.rs sets it); anywhere else every mask would land
        // beside its field, silently. Refusing makes every step flagged instead.
        use windows::Win32::UI::HiDpi::{
            GetAwarenessFromDpiAwarenessContext, GetThreadDpiAwarenessContext,
            DPI_AWARENESS_PER_MONITOR_AWARE,
        };
        if GetAwarenessFromDpiAwarenessContext(GetThreadDpiAwarenessContext())
            != DPI_AWARENESS_PER_MONITOR_AWARE
        {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "the process is not per-monitor DPI aware, so field rectangles cannot be trusted",
            ));
        }
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let mut slot = CLIENT.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)?;
            let is_password =
                uia.CreatePropertyCondition(UIA_IsPasswordPropertyId, &VARIANT::from(true))?;
            *slot = Some((Arc::new(Uia { uia, is_password }), 0));
        }
        let Some((uia, timeout)) = slot.as_mut() else {
            unreachable!("created above");
        };
        if *timeout != timeout_ms {
            if let Ok(uia2) = uia.uia.cast::<IUIAutomation2>() {
                uia2.SetConnectionTimeout(timeout_ms)?;
                uia2.SetTransactionTimeout(timeout_ms)?;
            }
            *timeout = timeout_ms;
        }
        Ok(uia.clone())
    }
}

impl Uia {
    fn fields(&self, hwnd: isize) -> Result<Vec<Rect>> {
        unsafe {
            let root = self.uia.ElementFromHandle(HWND(hwnd as *mut _))?;
            let found = root.FindAll(TreeScope_Descendants, &self.is_password)?;
            let mut fields = Vec::new();
            for i in 0..found.Length()? {
                fields.push(rect(found.GetElement(i)?.CurrentBoundingRectangle()?));
            }
            Ok(fields)
        }
    }
}

fn rect(r: RECT) -> Rect {
    Rect::new(r.left, r.top, r.right, r.bottom)
}

/// `None` when the token cannot be read.
fn token_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0;
        let read = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        let _ = CloseHandle(token);
        read.ok()?;
        Some(elevation.TokenIsElevated != 0)
    }
}

fn recorder_elevated() -> bool {
    static ELEVATED: OnceLock<bool> = OnceLock::new();
    *ELEVATED.get_or_init(|| token_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false))
}

/// Why a window is not queried at all (A1), or `None` to query it. An
/// elevated window returns no fields and no error to a normal process, so
/// the query cannot be what reveals it.
fn not_checkable(hwnd: isize) -> Option<&'static str> {
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(HWND(hwnd as *mut _), Some(&mut pid));
        if pid == 0 {
            return Some("the window has no process");
        }
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return Some("its process cannot be opened");
        };
        let elevated = token_elevated(process);
        let _ = CloseHandle(process);
        match elevated {
            None => Some("its process token cannot be read"),
            Some(true) if !recorder_elevated() => Some("its process runs as administrator"),
            Some(_) => None,
        }
    }
}

/// The real query: A1 first, then every `IsPassword` element of the window.
pub struct UiaQuery {
    pub timeout_ms: u32,
}

impl FieldQuery for UiaQuery {
    fn query(&self, hwnd: isize) -> QueryResult {
        // ponytail: one line per query, warm-ups included, to set the step
        // budget from real desktops (8.1). Down to debug once it is set.
        let started = std::time::Instant::now();
        let result = self.query_uncounted(hwnd);
        let mut class = [0u16; 64];
        let len = unsafe { GetClassNameW(HWND(hwnd as *mut _), &mut class) } as usize;
        log::info!(
            "Password check: window {hwnd:#x} ({}) answered in {} ms: {}",
            String::from_utf16_lossy(&class[..len]),
            started.elapsed().as_millis(),
            match &result {
                QueryResult::Fields(f) => format!("{} field(s)", f.len()),
                QueryResult::Failed(e) => format!("failed, {e}"),
                QueryResult::NotCheckable(why) => format!("not checkable, {why}"),
            }
        );
        result
    }
}

impl UiaQuery {
    fn query_uncounted(&self, hwnd: isize) -> QueryResult {
        if let Some(why) = not_checkable(hwnd) {
            return QueryResult::NotCheckable(why);
        }
        match client(self.timeout_ms) {
            Err(e) => QueryResult::Failed(format!("UIA unavailable: {e}")),
            Ok(uia) => match uia.fields(hwnd) {
                Ok(fields) => QueryResult::Fields(fields),
                Err(e) => QueryResult::Failed(format!("0x{:08X} {}", e.code().0, e.message())),
            },
        }
    }
}

/// The checker the window-event callback warms up. A callback is a plain
/// function, so it reaches the recording's checker through this.
static TARGET: Mutex<Option<Arc<Checker>>> = Mutex::new(None);

unsafe extern "system" fn on_window_event(
    _: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _: u32,
    _: u32,
) {
    if hwnd.0.is_null()
        || id_object != OBJID_WINDOW.0
        || id_child != CHILDID_SELF as i32
        || GetAncestor(hwnd, GA_ROOT) != hwnd
    {
        return;
    }
    // Tooltips and other small popups fire show events constantly and hold
    // no password field.
    let mut r = RECT::default();
    if GetWindowRect(hwnd, &mut r).is_err() || r.right - r.left < 100 || r.bottom - r.top < 100 {
        return;
    }
    let target = TARGET.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(checker) = target {
        checker.warm_up(hwnd.0 as isize);
    }
}

#[implement(IUIAutomationFocusChangedEventHandler)]
struct FocusWatch {
    checker: Arc<Checker>,
}

impl IUIAutomationFocusChangedEventHandler_Impl for FocusWatch_Impl {
    fn HandleFocusChangedEvent(&self, sender: Option<&IUIAutomationElement>) -> Result<()> {
        let Some(element) = sender else {
            return Ok(());
        };
        unsafe {
            if element
                .CurrentIsPassword()
                .map(|b| b.as_bool())
                .unwrap_or(false)
            {
                if let Ok(r) = element.CurrentBoundingRectangle() {
                    // ponytail: the focused field belongs to the foreground
                    // window; a Chromium page element has no window handle of
                    // its own to ask.
                    let window = GetAncestor(GetForegroundWindow(), GA_ROOT);
                    self.checker.note_focused_field(window.0 as isize, rect(r));
                }
            }
        }
        Ok(())
    }
}

/// The warm-up triggers of one recording (D5): every visible window at the
/// start, foreground and show events, and focus on a password field. Dropping
/// it stops them.
pub struct Watch {
    hook_thread: u32,
    focus_stop: Option<mpsc::Sender<()>>,
}

impl Watch {
    pub fn start(checker: Arc<Checker>, timeout_ms: u32) -> Watch {
        *TARGET.lock().unwrap_or_else(|e| e.into_inner()) = Some(checker.clone());
        if let Some(snapshot) = super::geometry::snapshot(None) {
            for w in snapshot
                .windows
                .iter()
                .filter(|w| !w.cloaked && !w.excluded)
            {
                checker.warm_up(w.hwnd);
            }
        }

        // WINEVENT_OUTOFCONTEXT delivers on the installing thread's message
        // loop, so the hooks get a thread of their own.
        let (ready_tx, ready_rx) = mpsc::channel();
        std::thread::spawn(move || unsafe {
            let foreground = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(on_window_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            let show = SetWinEventHook(
                EVENT_OBJECT_SHOW,
                EVENT_OBJECT_SHOW,
                None,
                Some(on_window_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            // A window restored from the taskbar comes to the front while it
            // still has its minimised size, which the size filter drops; its
            // warm-up runs when the restore ends (found in the 8.1 run).
            let restored = SetWinEventHook(
                EVENT_SYSTEM_MINIMIZEEND,
                EVENT_SYSTEM_MINIMIZEEND,
                None,
                Some(on_window_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            let mut msg = MSG::default();
            // Creates the queue before the thread id is handed out, so the
            // WM_QUIT `drop` posts cannot arrive before there is a queue.
            let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
            let _ = ready_tx.send(GetCurrentThreadId());
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                DispatchMessageW(&msg);
            }
            let _ = UnhookWinEvent(foreground);
            let _ = UnhookWinEvent(show);
            let _ = UnhookWinEvent(restored);
        });
        let hook_thread = ready_rx.recv().unwrap_or(0);

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            let uia = match client(timeout_ms) {
                Ok(uia) => uia,
                Err(e) => {
                    log::warn!("Password check: no focus events, UIA unavailable: {e}");
                    return;
                }
            };
            let handler: IUIAutomationFocusChangedEventHandler = FocusWatch { checker }.into();
            if let Err(e) = unsafe { uia.uia.AddFocusChangedEventHandler(None, &handler) } {
                log::warn!("Password check: no focus events: {e}");
                return;
            }
            let _ = stop_rx.recv();
            unsafe {
                let _ = uia.uia.RemoveAllEventHandlers();
            }
        });

        log::info!("Password check: warm-up and focus watch started");
        Watch {
            hook_thread,
            focus_stop: Some(stop_tx),
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.hook_thread, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        self.focus_stop.take();
        *TARGET.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}
