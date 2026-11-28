//! Remembers which app you were typing in when the shortcut fired, so the answer goes back there.
//! Windows: Win32. macOS: AppleScript (System Events). Linux/X11: xdotool when installed.
//! Elsewhere (Wayland) we rely on the compositor giving focus back when the popup hides.

use std::time::Duration;

#[cfg(target_os = "windows")]
mod imp {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsWindow,
        SetForegroundWindow,
    };

    pub async fn capture() -> Option<String> {
        let h = unsafe { GetForegroundWindow() };
        (!h.is_null()).then(|| (h as isize).to_string())
    }

    pub async fn restore(token: &str) {
        if let Ok(v) = token.parse::<isize>() {
            force_foreground(v);
        }
    }

    /// Windows refuses SetForegroundWindow from a background process; attaching input queues works around it.
    pub fn force_foreground(hwnd: isize) {
        let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
        unsafe {
            if hwnd.is_null() || IsWindow(hwnd) == 0 {
                return;
            }
            let fg = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg, std::ptr::null_mut());
            let me = GetCurrentThreadId();
            let attached =
                fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, 1) != 0;
            SetForegroundWindow(hwnd);
            BringWindowToTop(hwnd);
            if attached {
                AttachThreadInput(me, fg_thread, 0);
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    pub async fn capture() -> Option<String> {
        super::run("osascript", &["-e", "tell application \"System Events\" to get unix id of first process whose frontmost is true"]).await
    }

    pub async fn restore(token: &str) {
        if token.chars().all(|c| c.is_ascii_digit()) {
            let script = format!(
                "tell application \"System Events\" to set frontmost of (first process whose unix id is {token}) to true"
            );
            super::run("osascript", &["-e", &script]).await;
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    pub async fn capture() -> Option<String> {
        std::env::var_os("DISPLAY")?; // xdotool needs X11
        super::run("xdotool", &["getactivewindow"]).await
    }

    pub async fn restore(token: &str) {
        if token.chars().all(|c| c.is_ascii_digit()) {
            super::run("xdotool", &["windowactivate", "--sync", token]).await;
        }
    }
}

pub async fn capture() -> Option<String> {
    imp::capture().await
}

/// Screen rectangle (physical pixels) of the text caret in the foreground app: x, y, width, height.
/// Windows only for now; other systems return None and the popup opens near the mouse.
#[cfg(target_os = "windows")]
pub fn caret_rect() -> Option<(i32, i32, i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GUITHREADINFO, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    };
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return None;
        }
        let tid = GetWindowThreadProcessId(fg, std::ptr::null_mut());
        let mut gi: GUITHREADINFO = std::mem::zeroed();
        gi.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        let mut target = fg;
        if GetGUIThreadInfo(tid, &mut gi) != 0 {
            // Classic Win32 caret (Notepad, Office, most native apps).
            let r = gi.rcCaret;
            if !gi.hwndCaret.is_null() && r.bottom > r.top {
                let mut p = POINT {
                    x: r.left,
                    y: r.top,
                };
                if ClientToScreen(gi.hwndCaret, &mut p) != 0 {
                    return Some((p.x, p.y, (r.right - r.left).max(1), r.bottom - r.top));
                }
            }
            if !gi.hwndFocus.is_null() {
                target = gi.hwndFocus;
            }
        }
        msaa_caret(target)
    }
}

/// Browsers, Electron apps and modern controls expose their caret through accessibility instead.
#[cfg(target_os = "windows")]
unsafe fn msaa_caret(hwnd: windows_sys::Win32::Foundation::HWND) -> Option<(i32, i32, i32, i32)> {
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Accessibility::{AccessibleObjectFromWindow, IAccessible};
    use windows::core::Interface;
    use windows_sys::Win32::UI::WindowsAndMessaging::OBJID_CARET;

    unsafe {
        let inited = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let result = (|| {
            let mut raw: *mut core::ffi::c_void = std::ptr::null_mut();
            AccessibleObjectFromWindow(
                windows::Win32::Foundation::HWND(hwnd),
                OBJID_CARET as u32,
                &IAccessible::IID,
                &mut raw,
            )
            .ok()?;
            if raw.is_null() {
                return None;
            }
            let acc = IAccessible::from_raw(raw);
            // VARIANT { vt: VT_I4, lVal: CHILDID_SELF (0) }
            let mut child: VARIANT = std::mem::zeroed();
            *(&mut child as *mut VARIANT as *mut u16) = 3;
            let (mut x, mut y, mut w, mut h) = (0, 0, 0, 0);
            acc.accLocation(&mut x, &mut y, &mut w, &mut h, &child)
                .ok()?;
            (h > 0 && (x != 0 || y != 0)).then_some((x, y, w.max(1), h))
        })();
        if inited {
            CoUninitialize();
        }
        result
    }
}

#[cfg(not(target_os = "windows"))]
pub fn caret_rect() -> Option<(i32, i32, i32, i32)> {
    None
}

pub async fn restore(token: Option<&str>) {
    if let Some(t) = token {
        imp::restore(t).await;
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
}

/// Windows 11: ask DWM to round the corners of our borderless window (used with the OS blur,
/// where the panel fills the window). No-op elsewhere and on Windows 10.
#[allow(unused_variables)]
pub fn round_corners(window: &tauri::WebviewWindow, round: bool) {
    #[cfg(target_os = "windows")]
    if let Ok(h) = window.hwnd() {
        use windows_sys::Win32::Graphics::Dwm::{
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DEFAULT, DWMWCP_ROUND, DwmSetWindowAttribute,
        };
        let pref = if round { DWMWCP_ROUND } else { DWMWCP_DEFAULT };
        unsafe {
            DwmSetWindowAttribute(
                h.0 as _,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &pref as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&pref) as u32,
            );
        }
    }
}

/// Whether the window a `capture()` token points to still exists. Windows can tell;
/// elsewhere we assume it does (the popup then waits until the user comes back to it).
pub fn is_open(token: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;
        match token.parse::<isize>() {
            Ok(h) => unsafe { IsWindow(h as windows_sys::Win32::Foundation::HWND) != 0 },
            Err(_) => false,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = token;
        true
    }
}

/// Brings one of our own windows to the front (Windows needs a nudge, other OSes are fine with set_focus).
#[allow(unused_variables)]
pub fn bring_to_front(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    if let Ok(h) = window.hwnd() {
        imp::force_foreground(h.0 as isize);
    }
}

#[allow(dead_code)]
async fn run(exe: &str, args: &[&str]) -> Option<String> {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(3), cmd.output())
        .await
        .ok()?
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}
