//! Windows: which unrecorded window is in front, from the foreground and
//! desktop-switch events of the whole session.

use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Instant;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThreadId, OpenProcess, OpenProcessToken,
    QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::{is_unrecorded, Log, UnrecordedWindow, MEDIUM_LEVEL, SYSTEM_LEVEL};

/// What the event callback, a plain function, works on.
struct Target {
    log: Log,
    started: Instant,
    recorder: u32,
    on_secure_desktop: bool,
    in_front: bool,
    notify: Box<dyn Fn(bool) + Send>,
}

static TARGET: Mutex<Option<Target>> = Mutex::new(None);

/// The integrity level of a process token, `None` when it cannot be read.
fn token_level(process: HANDLE) -> Option<u32> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        // u64 keeps the label's pointer aligned.
        let mut buf = [0u64; 16];
        let mut len = 0;
        let read = GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buf.as_mut_ptr() as *mut _),
            std::mem::size_of_val(&buf) as u32,
            &mut len,
        );
        let _ = CloseHandle(token);
        read.ok()?;
        let sid = (*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL)).Label.Sid;
        let count = *GetSidSubAuthorityCount(sid);
        if count == 0 {
            return None;
        }
        Some(*GetSidSubAuthority(sid, u32::from(count) - 1))
    }
}

fn recorder_level() -> u32 {
    static LEVEL: OnceLock<u32> = OnceLock::new();
    *LEVEL.get_or_init(|| {
        token_level(unsafe { GetCurrentProcess() }).unwrap_or_else(|| {
            log::warn!("Unrecorded windows: own integrity level unreadable, taken as medium");
            MEDIUM_LEVEL
        })
    })
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// The file description of an executable ("Registry Editor"), if it has one.
fn file_description(path: &[u16]) -> Option<String> {
    unsafe {
        let file = windows::core::PCWSTR(path.as_ptr());
        let size = GetFileVersionInfoSizeW(file, None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(file, 0, size, data.as_mut_ptr() as *mut _).ok()?;
        let mut ptr = std::ptr::null_mut();
        let mut len = 0;
        let key = wide("\\VarFileInfo\\Translation");
        if !VerQueryValueW(
            data.as_ptr() as *const _,
            windows::core::PCWSTR(key.as_ptr()),
            &mut ptr,
            &mut len,
        )
        .as_bool()
            || ptr.is_null()
            || len < 4
        {
            return None;
        }
        // Language and code page, two u16s; read unaligned, as the block is
        // a byte buffer.
        let [lang, codepage] = std::ptr::read_unaligned(ptr as *const [u16; 2]);
        let key = wide(&format!(
            "\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription"
        ));
        if !VerQueryValueW(
            data.as_ptr() as *const _,
            windows::core::PCWSTR(key.as_ptr()),
            &mut ptr,
            &mut len,
        )
        .as_bool()
            || ptr.is_null()
            || len == 0
        {
            return None;
        }
        let text = std::slice::from_raw_parts(ptr as *const u16, len as usize);
        let text = String::from_utf16_lossy(text);
        let text = text.trim_end_matches('\0').trim();
        (!text.is_empty()).then(|| text.to_string())
    }
}

/// The app's name: its file description, else its file name without `.exe`.
fn app_name(process: HANDLE) -> Option<String> {
    unsafe {
        let mut path = [0u16; 1024];
        let mut len = path.len() as u32;
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(path.as_mut_ptr()),
            &mut len,
        )
        .ok()?;
        let path = &path[..len as usize];
        let mut terminated = path.to_vec();
        terminated.push(0);
        file_description(&terminated).or_else(|| {
            let path = String::from_utf16_lossy(path);
            std::path::Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
    }
}

/// `None` for a recorded window, else what to call the unrecorded one.
fn classify(hwnd: HWND, recorder: u32) -> Option<UnrecordedWindow> {
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        // A window closed while its event was on the way.
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok();
        let level = process.and_then(token_level);
        if !is_unrecorded(level, recorder) {
            if let Some(p) = process {
                let _ = CloseHandle(p);
            }
            return None;
        }
        let name = process.and_then(app_name);
        if let Some(p) = process {
            let _ = CloseHandle(p);
        }
        if level.is_none() {
            log::info!(
                "Unrecorded windows: integrity level of process {pid} ({}) unreadable, \
                 counted as unrecorded",
                name.as_deref().unwrap_or("name unreadable")
            );
        }
        Some(name.map_or(UnrecordedWindow::UnknownApp, UnrecordedWindow::App))
    }
}

/// The secure desktop (the security prompt, also the lock screen) refuses a
/// process that is not SYSTEM, so an input desktop that cannot be opened is it.
fn input_desktop_is_secure() -> bool {
    match unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) } {
        Ok(desktop) => {
            let _ = unsafe { CloseDesktop(desktop) };
            false
        }
        Err(_) => true,
    }
}

fn update(target: &mut Target, window: Option<UnrecordedWindow>) {
    let now = target.started.elapsed().as_secs_f64();
    if let Some(w) = &window {
        if !target.in_front {
            log::info!("Unrecorded windows: {w:?} in front at {now:.1}s");
        }
    }
    let in_front = target.log.foreground(now, window);
    if in_front != target.in_front {
        target.in_front = in_front;
        (target.notify)(in_front);
    }
}

unsafe extern "system" fn on_event(
    _: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    let mut guard = TARGET.lock().unwrap_or_else(|e| e.into_inner());
    let Some(target) = guard.as_mut() else {
        return;
    };
    if event == EVENT_SYSTEM_DESKTOPSWITCH {
        target.on_secure_desktop = input_desktop_is_secure();
        if target.on_secure_desktop {
            let window = is_unrecorded(Some(SYSTEM_LEVEL), target.recorder)
                .then_some(UnrecordedWindow::SecurityPrompt);
            update(target, window);
            return;
        }
        // Back on the user's desktop: what is in front now decides.
        let front = GetForegroundWindow();
        if !front.0.is_null() {
            let window = classify(front, target.recorder);
            update(target, window);
        } else {
            update(target, None);
        }
        return;
    }
    if target.on_secure_desktop || hwnd.0.is_null() {
        return;
    }
    let window = classify(hwnd, target.recorder);
    update(target, window);
}

/// Watches the foreground for one recording. Dropping it stops the events and
/// ends a visit still open at that moment.
pub struct Watch {
    hook_thread: u32,
}

impl Watch {
    /// `notify` hears whether an unrecorded window is in front, on each change.
    pub fn start(
        output_dir: &std::path::Path,
        started: Instant,
        notify: impl Fn(bool) + Send + 'static,
    ) -> Watch {
        let recorder = recorder_level();
        let mut target = Target {
            log: Log::new(output_dir),
            started,
            recorder,
            on_secure_desktop: false,
            in_front: false,
            notify: Box::new(notify),
        };
        // The window in front when the recording starts counts too.
        let front = unsafe { GetForegroundWindow() };
        if !front.0.is_null() {
            let window = classify(front, recorder);
            update(&mut target, window);
        }
        *TARGET.lock().unwrap_or_else(|e| e.into_inner()) = Some(target);

        // WINEVENT_OUTOFCONTEXT delivers on the installing thread's message
        // loop, so the hooks get a thread of their own.
        let (ready_tx, ready_rx) = mpsc::channel();
        std::thread::spawn(move || unsafe {
            let foreground = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(on_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            let desktop = SetWinEventHook(
                EVENT_SYSTEM_DESKTOPSWITCH,
                EVENT_SYSTEM_DESKTOPSWITCH,
                None,
                Some(on_event),
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
            let _ = UnhookWinEvent(desktop);
        });
        let hook_thread = ready_rx.recv().unwrap_or(0);
        log::info!("Unrecorded windows: watch started (recorder level {recorder:#x})");
        Watch { hook_thread }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.hook_thread, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(mut target) = TARGET.lock().unwrap_or_else(|e| e.into_inner()).take() {
            update(&mut target, None);
        }
    }
}
