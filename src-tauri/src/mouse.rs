//! Mouse buttons as shortcuts: side buttons ("Mouse4" = back, "Mouse5" = forward), alone or with
//! modifiers, and the middle button with at least one modifier ("Ctrl+MouseMiddle").
//! Windows: a low-level mouse hook; the bound click is swallowed so the app under the mouse
//! doesn't also go "back". Other systems: not supported yet (map the button to F13–F24 instead).

use crate::Action;
use tauri::AppHandle;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MouseBinding {
    /// 3 = middle, 4 = back (XButton1), 5 = forward (XButton2).
    pub button: u8,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

/// None if the text isn't a mouse shortcut; Some(Err) if it is one but can't be used.
pub fn parse(text: &str) -> Option<Result<MouseBinding, String>> {
    let parts: Vec<String> = text
        .split('+')
        .map(|p| p.trim().to_ascii_lowercase())
        .filter(|p| !p.is_empty())
        .collect();
    let last = parts.last()?;
    let button = match last.as_str() {
        "mouse4" | "mouseback" => 4,
        "mouse5" | "mouseforward" => 5,
        "mouse3" | "mousemiddle" => 3,
        _ => return None,
    };
    let mut b = MouseBinding {
        button,
        ctrl: false,
        alt: false,
        shift: false,
        win: false,
    };
    for m in &parts[..parts.len() - 1] {
        match m.as_str() {
            "ctrl" | "control" => b.ctrl = true,
            "alt" | "option" | "opt" => b.alt = true,
            "shift" => b.shift = true,
            "win" | "cmd" | "command" | "meta" | "super" => b.win = true,
            other => return Some(Err(format!("\"{text}\": unknown modifier \"{other}\""))),
        }
    }
    if button == 3 && !(b.ctrl || b.alt || b.shift || b.win) {
        return Some(Err(format!(
            "\"{text}\": the middle button alone is used everywhere, add a modifier (Ctrl, Alt…)"
        )));
    }
    if !cfg!(target_os = "windows") {
        return Some(Err(format!(
            "\"{text}\": mouse buttons are only supported on Windows for now. \
             Map the button to an F13–F24 key in your mouse software instead."
        )));
    }
    Some(Ok(b))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::MouseBinding;
    use crate::Action;
    use std::sync::{Mutex, Once, OnceLock};
    use tauri::AppHandle;
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, HC_ACTION, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT,
        SetWindowsHookExW, WH_MOUSE_LL, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
    };

    struct HookState {
        app: Option<AppHandle>,
        bindings: Vec<(MouseBinding, Action)>,
        /// Button-up events to swallow because we swallowed their button-down.
        swallow_up: [bool; 6],
    }

    fn state() -> &'static Mutex<HookState> {
        static S: OnceLock<Mutex<HookState>> = OnceLock::new();
        S.get_or_init(|| {
            Mutex::new(HookState {
                app: None,
                bindings: vec![],
                swallow_up: [false; 6],
            })
        })
    }

    pub fn set_bindings(app: &AppHandle, bindings: Vec<(MouseBinding, Action)>) {
        let need_hook = !bindings.is_empty();
        {
            let mut s = state().lock().unwrap();
            s.app = Some(app.clone());
            s.bindings = bindings;
        }
        if need_hook {
            install();
        }
    }

    /// The hook lives on its own thread with a message loop; installed once, on first use.
    fn install() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            std::thread::spawn(|| unsafe {
                let hook = SetWindowsHookExW(
                    WH_MOUSE_LL,
                    Some(hook_proc),
                    GetModuleHandleW(std::ptr::null()),
                    0,
                );
                if hook.is_null() {
                    crate::log("couldn't install the mouse hook");
                    return;
                }
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            });
        });
    }

    fn down(vk: u16) -> bool {
        unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
    }

    /// Returns true when the event must be swallowed. Must stay fast: Windows drops slow hooks.
    fn handle(button: u8, pressed: bool) -> bool {
        let Ok(mut s) = state().try_lock() else {
            return false;
        };
        let i = button as usize;
        if !pressed {
            return std::mem::take(&mut s.swallow_up[i]);
        }
        let (ctrl, alt, shift, win) = (
            down(VK_CONTROL),
            down(VK_MENU),
            down(VK_SHIFT),
            down(VK_LWIN) || down(VK_RWIN),
        );
        let found = s
            .bindings
            .iter()
            .find(|(b, _)| {
                b.button == button
                    && b.ctrl == ctrl
                    && b.alt == alt
                    && b.shift == shift
                    && b.win == win
            })
            .map(|(_, a)| a.clone());
        let (Some(action), Some(app)) = (found, s.app.clone()) else {
            return false;
        };
        s.swallow_up[i] = true;
        drop(s);
        crate::trigger(&app, action);
        true
    }

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            if code == HC_ACTION as i32 {
                let info = &*(lparam as *const MSLLHOOKSTRUCT);
                if info.flags & LLMHF_INJECTED == 0 {
                    let x = match (info.mouseData >> 16) & 0xffff {
                        1 => 4,
                        2 => 5,
                        _ => 0,
                    };
                    let event = match wparam as u32 {
                        WM_XBUTTONDOWN => Some((x, true)),
                        WM_XBUTTONUP => Some((x, false)),
                        WM_MBUTTONDOWN => Some((3, true)),
                        WM_MBUTTONUP => Some((3, false)),
                        _ => None,
                    };
                    if let Some((button, pressed)) = event
                        && button != 0
                        && handle(button, pressed)
                    {
                        return 1;
                    }
                }
            }
            CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
        }
    }
}

#[cfg(target_os = "windows")]
pub fn set_bindings(app: &AppHandle, bindings: Vec<(MouseBinding, Action)>) {
    imp::set_bindings(app, bindings);
}

#[cfg(not(target_os = "windows"))]
pub fn set_bindings(_app: &AppHandle, _bindings: Vec<(MouseBinding, Action)>) {}
