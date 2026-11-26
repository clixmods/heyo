//! Reads the selection of the app you were typing in, and writes the answer back into it.
//! Keystrokes are simulated with enigo, the clipboard goes through Tauri's clipboard plugin.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Cmd on macOS, Ctrl everywhere else.
fn shortcut_modifier() -> Key {
    if cfg!(target_os = "macos") {
        Key::Meta
    } else {
        Key::Control
    }
}

/// Copies the current selection (Ctrl/Cmd+C) without losing the text that was on the clipboard.
pub async fn capture_selection(app: &AppHandle) -> String {
    let t = std::time::Instant::now();
    release_modifiers(app).await;
    let t_release = t.elapsed().as_millis();
    let cb = app.clipboard();
    let saved = cb.read_text().ok();
    let t_saved = t.elapsed().as_millis();

    // Windows: the clipboard sequence number tells us at once whether Ctrl+C copied something,
    // and if nothing was selected the clipboard is never touched.
    if let Some(before) = clipboard_seq() {
        chord(app, 'c').await;
        let t_chord = t.elapsed().as_millis();
        let start = std::time::Instant::now();
        // Apps answer Ctrl+C within a few ms; past ~120 ms we assume nothing was selected.
        while start.elapsed() < Duration::from_millis(120) {
            sleep(4).await;
            if clipboard_seq() != Some(before) {
                sleep(8).await; // let the source app finish writing
                let text = cb.read_text().unwrap_or_default();
                restore_clipboard(app, saved);
                crate::log(&format!(
                    "  release {t_release} ms, save {t_saved} ms, ctrl+c {t_chord} ms, copy received at {} ms",
                    t.elapsed().as_millis()
                ));
                return text;
            }
        }
        crate::log(&format!(
            "  release {t_release} ms, save {t_saved} ms, ctrl+c {t_chord} ms, nothing selected"
        ));
        return String::new();
    }

    let sentinel = format!("hoppy-{}", uuid::Uuid::new_v4().simple());
    let _ = cb.write_text(sentinel.clone());
    sleep(20).await;
    chord(app, 'c').await;

    let mut text = String::new();
    for _ in 0..15 {
        // up to ~300 ms
        sleep(20).await;
        if let Ok(now) = cb.read_text()
            && now != sentinel
        {
            text = now;
            break;
        }
    }
    restore_clipboard(app, saved);
    text
}

/// Writes the text into the focused field: paste (fast, clipboard restored) or simulated typing.
pub async fn insert(app: &AppHandle, text: &str, method: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }
    release_modifiers(app).await;

    if method == "type" {
        let t = text.to_string();
        return on_main(app, move || -> Result<(), String> {
            let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
            e.text(&t).map_err(|e| e.to_string())
        })
        .await;
    }

    let cb = app.clipboard();
    let saved = cb.read_text().ok();
    cb.write_text(text.to_string()).map_err(|e| e.to_string())?;
    sleep(40).await;
    chord(app, 'v').await;
    sleep(450).await; // let the target app read the clipboard before restoring it
    restore_clipboard(app, saved);
    Ok(())
}

async fn chord(app: &AppHandle, c: char) {
    let res = on_main(app, move || -> Result<(), String> {
        let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
        let m = shortcut_modifier();
        e.key(m, Direction::Press).map_err(|e| e.to_string())?;
        let r = e.key(Key::Unicode(c), Direction::Click);
        e.key(m, Direction::Release).map_err(|e| e.to_string())?;
        r.map_err(|e| e.to_string())
    })
    .await;
    if let Err(e) = res {
        crate::log(&format!("keyboard simulation: {e}"));
    }
}

/// Only text is restored: images or files that were on the clipboard are not kept.
fn restore_clipboard(app: &AppHandle, saved: Option<String>) {
    let cb = app.clipboard();
    match saved {
        Some(s) => {
            let _ = cb.write_text(s);
        }
        None => {
            let _ = cb.clear();
        }
    }
}

/// Presses Win+H, which opens (or closes) Windows voice typing on the focused text field.
/// Uses the virtual key code for H so it works whatever the keyboard layout.
#[cfg(target_os = "windows")]
pub async fn voice_typing_shortcut(app: &AppHandle) -> Result<(), String> {
    const VK_H: u32 = 0x48;
    on_main(app, || -> Result<(), String> {
        let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
        e.key(Key::Meta, Direction::Press)
            .map_err(|e| e.to_string())?;
        let r = e.key(Key::Other(VK_H), Direction::Click);
        e.key(Key::Meta, Direction::Release)
            .map_err(|e| e.to_string())?;
        r.map_err(|e| e.to_string())
    })
    .await
}

#[cfg(target_os = "windows")]
fn clipboard_seq() -> Option<u32> {
    Some(unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() })
}

#[cfg(not(target_os = "windows"))]
fn clipboard_seq() -> Option<u32> {
    None
}

#[derive(Default, Clone, Copy)]
struct HeldModifiers {
    meta: bool,
    alt: bool,
    shift: bool,
    ctrl: bool,
}

/// Snapshot of the physical modifier keys. Synchronous on purpose: the device handle
/// isn't `Send` on Linux (X11), so it must never live across an `.await`.
fn held_modifiers() -> Option<HeldModifiers> {
    use device_query::{DeviceQuery, DeviceState, Keycode};
    let keys = std::panic::catch_unwind(|| DeviceState::new().get_keys()).ok()?;
    let any = |ks: &[Keycode]| keys.iter().any(|k| ks.contains(k));
    Some(HeldModifiers {
        meta: any(&[
            Keycode::LMeta,
            Keycode::RMeta,
            Keycode::Command,
            Keycode::RCommand,
        ]),
        alt: any(&[
            Keycode::LAlt,
            Keycode::RAlt,
            Keycode::LOption,
            Keycode::ROption,
        ]),
        shift: any(&[Keycode::LShift, Keycode::RShift]),
        ctrl: any(&[Keycode::LControl, Keycode::RControl]),
    })
}

/// The shortcut's modifiers are usually still held, which would turn Ctrl+C into Ctrl+Alt+C.
/// Instead of waiting for the user to let go, release Alt/Shift/Ctrl ourselves (Alt and Shift first,
/// while Ctrl is still down, so a lone Alt release can't open the app's menu bar).
/// Win/Cmd is the exception: releasing it by hand could open the Start menu, so we wait for it.
async fn release_modifiers(app: &AppHandle) {
    // DeviceState can fail where there is no X11 (pure Wayland): then just wait a moment.
    let Some(mut mods) = held_modifiers() else {
        sleep(150).await;
        return;
    };
    for _ in 0..100 {
        // up to 2 s, only while Win/Cmd is held
        if !mods.meta {
            break;
        }
        sleep(20).await;
        mods = held_modifiers().unwrap_or_default();
    }

    let HeldModifiers {
        alt, shift, ctrl, ..
    } = mods;
    if !(alt || shift || ctrl) {
        return;
    }
    let res = on_main(app, move || -> Result<(), String> {
        let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
        let up = |e: &mut Enigo, k| e.key(k, Direction::Release).map_err(|e| e.to_string());
        if alt {
            up(&mut e, Key::Alt)?;
        }
        if shift {
            up(&mut e, Key::Shift)?;
        }
        if ctrl {
            up(&mut e, Key::Control)?;
        }
        Ok(())
    })
    .await;
    if let Err(e) = res {
        crate::log(&format!("releasing modifier keys: {e}"));
    }
    sleep(10).await;
}

/// macOS wants input simulation on the main thread; doing it everywhere keeps behaviour identical.
async fn on_main<T: Send + 'static>(app: &AppHandle, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = app.app_handle().clone();
    if handle
        .run_on_main_thread(move || {
            let _ = tx.send(f());
        })
        .is_err()
    {
        panic!("main thread unavailable");
    }
    rx.await.expect("main thread task dropped")
}

async fn sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}
