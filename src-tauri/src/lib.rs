//! Heyo: press a shortcut anywhere, a tiny popup opens on a "project" profile,
//! you type, Claude answers, and the answer is written into the app you were in.

mod claude;
mod focus;
mod inject;
mod mouse;
mod settings;
mod updates;

use serde::Serialize;
use settings::{AppSettings, Profile};
use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, RunEvent, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const POPUP_WIDTH: f64 = 640.0;

pub(crate) fn log(line: &str) {
    use std::io::Write;
    let dir = settings::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("heyo.log"))
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{secs}] {line}");
    }
    eprintln!("{line}");
}

#[derive(Clone)]
pub(crate) enum Action {
    Launcher,
    Profile(String),
}

/// What the popup was opened on.
#[derive(Default)]
struct Session {
    profile_id: String,
    focus_token: Option<String>,
    selection: String,
}

struct AppState {
    settings: Mutex<AppSettings>,
    session: Mutex<Session>,
    shortcuts: Mutex<HashMap<u32, Action>>,
    cancel: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    /// Set while we hide the popup on purpose (insert), so the blur handler leaves things alone.
    inserting: AtomicBool,
    /// Set while the OS voice-typing panel is open over the popup, so its focus changes don't close it.
    dictating: AtomicBool,
    opened_at: Mutex<Instant>,
    placement: Mutex<Option<Placement>>,
    /// A newer release found on GitHub, waiting for the user to install it.
    pending_update: Mutex<Option<tauri_plugin_updater::Update>>,
}

/// Where the popup sits, in physical pixels. When it is above the caret, it grows upward
/// (its bottom edge stays put) so it never covers the line you are typing on.
#[derive(Clone, Copy)]
struct Placement {
    left: i32,
    /// Bottom edge when `above`, top edge otherwise.
    edge: i32,
    above: bool,
    /// Top and bottom of the screen's work area, to keep the window on screen.
    min_y: i32,
    max_y: i32,
    scale: f64,
    /// Caret x inside the window (logical px): the point the popup springs out of.
    origin_x: f64,
}

impl Placement {
    fn top_for(&self, height_px: i32) -> i32 {
        let y = if self.above {
            self.edge - height_px
        } else {
            self.edge
        };
        y.min(self.max_y - height_px).max(self.min_y)
    }
}

impl AppState {
    fn settings(&self) -> AppSettings {
        self.settings.lock().unwrap().clone()
    }
}

// ------------------------------------------------------------------ shortcuts

/// "Ctrl+Alt+T", "Win+Shift+K", "Cmd+Space"… → the plugin's format.
fn parse_shortcut(text: &str) -> Result<Shortcut, String> {
    let parts: Vec<String> = text
        .split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|p| match p.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "Control".to_string(),
            "win" | "cmd" | "command" | "meta" | "super" => "Super".to_string(),
            "option" | "opt" => "Alt".to_string(),
            _ => p.to_string(),
        })
        .collect();
    if parts.len() == 1 && !is_standalone_key(&parts[0]) {
        return Err(format!(
            "\"{text}\": a key on its own must be one that doesn't type anything (F1–F24, Pause, Scroll Lock, Insert…). \
             Otherwise add a modifier (Ctrl, Alt, Shift, Win/Cmd)."
        ));
    }
    parts
        .join("+")
        .parse::<Shortcut>()
        .map_err(|e| format!("\"{text}\": {e}"))
}

/// Keys that can be a shortcut on their own without getting in the way of typing.
fn is_standalone_key(key: &str) -> bool {
    let k = key.to_ascii_uppercase();
    let f_key = k
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=24).contains(&n));
    f_key
        || matches!(
            k.as_str(),
            "PAUSE" | "SCROLLLOCK" | "INSERT" | "PRINTSCREEN" | "NUMLOCK"
        )
}

/// (Re)binds every shortcut from the settings. Returns one message per shortcut that failed.
fn register_shortcuts(app: &AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    let s = state.settings();
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut map = HashMap::new();
    let mut mouse_bindings: Vec<(mouse::MouseBinding, Action)> = vec![];
    let mut errors = vec![];

    let mut wanted: Vec<(String, String, Action)> = vec![(
        "Launcher".into(),
        s.launcher_hotkey.clone(),
        Action::Launcher,
    )];
    wanted.extend(s.profiles.iter().map(|p| {
        (
            p.name.clone(),
            p.hotkey.clone(),
            Action::Profile(p.id.clone()),
        )
    }));

    for (name, text, action) in wanted {
        if text.trim().is_empty() {
            continue;
        }
        if let Some(parsed) = mouse::parse(&text) {
            match parsed {
                Ok(b) if mouse_bindings.iter().any(|(x, _)| *x == b) => {
                    errors.push(format!("{name}: {text} is already used by another profile"))
                }
                Ok(b) => mouse_bindings.push((b, action)),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
            continue;
        }
        match parse_shortcut(&text) {
            Ok(sc) => match map.entry(sc.id()) {
                std::collections::hash_map::Entry::Occupied(_) => {
                    errors.push(format!("{name}: {text} is already used by another profile"));
                }
                std::collections::hash_map::Entry::Vacant(slot) => match gs.register(sc) {
                    Ok(()) => {
                        slot.insert(action);
                    }
                    Err(e) => errors.push(format!(
                        "{name}: {text} is unavailable (taken by another app?) — {e}"
                    )),
                },
            },
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    *state.shortcuts.lock().unwrap() = map;
    mouse::set_bindings(app, mouse_bindings);
    for e in &errors {
        log(e);
    }
    errors
}

fn on_shortcut(app: &AppHandle, shortcut: &Shortcut) {
    let state = app.state::<AppState>();
    let action = state.shortcuts.lock().unwrap().get(&shortcut.id()).cloned();
    if let Some(action) = action {
        trigger(app, action);
    }
}

/// Opens the popup for a shortcut's action (keyboard or mouse).
pub(crate) fn trigger(app: &AppHandle, action: Action) {
    let state = app.state::<AppState>();
    let profile_id = match action {
        Action::Launcher => state
            .settings()
            .current_profile()
            .map(|p| p.id.clone())
            .unwrap_or_default(),
        Action::Profile(id) => id,
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move { open_popup(&app, profile_id).await });
}

// ------------------------------------------------------------------ popup

fn popup(app: &AppHandle) -> WebviewWindow {
    app.get_webview_window("popup").expect("popup window")
}

async fn open_popup(app: &AppHandle, profile_id: String) {
    let state = app.state::<AppState>();
    let win = popup(app);
    state.dictating.store(false, Ordering::SeqCst);

    // Same shortcut again while open = close.
    if win.is_visible().unwrap_or(false) {
        let same = state.session.lock().unwrap().profile_id == profile_id;
        hide_popup_inner(app);
        if same {
            return;
        }
    }

    let settings = state.settings();
    let Some(profile) = settings.profile(&profile_id).cloned() else {
        return;
    };

    let t0 = Instant::now();
    let caret = tauri::async_runtime::spawn_blocking(focus::caret_rect)
        .await
        .ok()
        .flatten();
    let focus_token = focus::capture().await;
    let selection = if profile.use_selection {
        inject::capture_selection(app).await
    } else {
        String::new()
    };
    let capture_ms = t0.elapsed().as_millis();
    {
        let mut s = state.session.lock().unwrap();
        *s = Session {
            profile_id: profile.id.clone(),
            focus_token,
            selection,
        };
    }
    remember_profile(app, &profile.id);

    position_popup(app, &win, caret);
    *state.opened_at.lock().unwrap() = Instant::now();
    let _ = win.show();
    let _ = win.set_focus();
    focus::bring_to_front(&win);
    let _ = win.emit("popup-open", popup_state_inner(app));
    let at = if caret.is_some() {
        "at text caret"
    } else {
        "at mouse"
    };
    log(&format!(
        "popup \"{}\" {at}: selection {capture_ms} ms, shown in {} ms",
        profile.name,
        t0.elapsed().as_millis()
    ));
}

/// Transparent margin around the panel (room for its shadow) + the input's left padding, in logical px.
const SHADOW_MARGIN: f64 = 14.0;
const INPUT_INSET: f64 = 14.0 + 16.0;

/// Just above the text caret of the app you were typing in, with the input lined up on the caret.
/// Below it when there's no room above; near the mouse when the caret can't be read.
fn position_popup(app: &AppHandle, win: &WebviewWindow, caret: Option<(i32, i32, i32, i32)>) {
    let (cx, cy, ch) = match caret {
        Some((x, y, _, h)) => (x, y, h),
        None => match app.cursor_position() {
            Ok(c) => (c.x as i32, c.y as i32, 0),
            Err(_) => return,
        },
    };
    let Some(m) = app
        .monitor_from_point(cx as f64, cy as f64)
        .ok()
        .flatten()
        .or_else(|| win.primary_monitor().ok().flatten())
    else {
        return;
    };
    let scale = m.scale_factor();
    let wa = m.work_area();
    let (wa_x, wa_y) = (wa.position.x, wa.position.y);
    let (wa_w, wa_h) = (wa.size.width as i32, wa.size.height as i32);
    let px = |v: f64| (v * scale).round() as i32;

    let width = px(POPUP_WIDTH);
    let left = (cx - px(INPUT_INSET)).min(wa_x + wa_w - width).max(wa_x);
    let gap = px(6.0);
    // Room for a typical popup (~180 px) above the caret?
    let above = cy - wa_y >= px(180.0);
    let edge = if above {
        cy - gap + px(SHADOW_MARGIN)
    } else {
        cy + ch + gap - px(SHADOW_MARGIN)
    };
    let origin_x =
        ((cx - left) as f64 / scale).clamp(SHADOW_MARGIN + 6.0, POPUP_WIDTH - SHADOW_MARGIN - 6.0);
    let p = Placement {
        left,
        edge,
        above,
        min_y: wa_y,
        max_y: wa_y + wa_h,
        scale,
        origin_x,
    };
    *app.state::<AppState>().placement.lock().unwrap() = Some(p);

    let height = win
        .outer_size()
        .map(|s| s.height as i32)
        .unwrap_or(px(160.0));
    let _ = win.set_position(PhysicalPosition::new(left, p.top_for(height)));
}

fn hide_popup_inner(app: &AppHandle) {
    cancel_inner(app);
    app.state::<AppState>()
        .dictating
        .store(false, Ordering::SeqCst);
    let _ = popup(app).hide();
}

fn cancel_inner(app: &AppHandle) {
    if let Some(tx) = app.state::<AppState>().cancel.lock().unwrap().take() {
        let _ = tx.send(());
    }
}

fn remember_profile(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    let mut s = state.settings.lock().unwrap();
    if s.last_profile_id != id {
        s.last_profile_id = id.to_string();
        let _ = settings::save(&s);
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProfileItem {
    id: String,
    name: String,
    hotkey: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PopupState {
    profile: Option<Profile>,
    profiles: Vec<ProfileItem>,
    selection: String,
    history: Vec<claude::Message>,
    mac: bool,
    /// The OS offers voice typing we can start from the mic button (Windows: Win+H).
    dictation: bool,
    /// Version of an update waiting to be installed (shown as a banner).
    update: Option<String>,
    /// Where the popup springs out of: caret x in the window, and whether it sits above the caret.
    origin_x: f64,
    above: bool,
}

fn popup_state_inner(app: &AppHandle) -> PopupState {
    let state = app.state::<AppState>();
    let s = state.settings();
    let placement = *state.placement.lock().unwrap();
    let session = state.session.lock().unwrap();
    let profile = s
        .profile(&session.profile_id)
        .or_else(|| s.current_profile())
        .cloned();
    let history = profile
        .as_ref()
        .filter(|p| p.keep_history)
        .map(|p| claude::load_conversation(&p.id).messages)
        .unwrap_or_default();
    PopupState {
        profile,
        profiles: s
            .profiles
            .iter()
            .map(|p| ProfileItem {
                id: p.id.clone(),
                name: p.name.clone(),
                hotkey: p.hotkey.clone(),
            })
            .collect(),
        selection: session.selection.clone(),
        history,
        mac: cfg!(target_os = "macos"),
        dictation: cfg!(target_os = "windows"),
        update: updates::pending_version(app),
        origin_x: placement.map_or(SHADOW_MARGIN + 16.0, |p| p.origin_x),
        above: placement.is_none_or(|p| p.above),
    }
}

// ------------------------------------------------------------------ commands: popup

#[tauri::command]
fn popup_state(app: AppHandle) -> PopupState {
    popup_state_inner(&app)
}

#[tauri::command]
fn switch_profile(app: AppHandle, id: String) -> PopupState {
    app.state::<AppState>().session.lock().unwrap().profile_id = id.clone();
    remember_profile(&app, &id);
    popup_state_inner(&app)
}

#[tauri::command]
async fn ask(app: AppHandle, text: String, use_selection: bool) -> Result<String, String> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let (profile, selection) = {
        let mut session = state.session.lock().unwrap();
        let p = settings
            .profile(&session.profile_id)
            .cloned()
            .ok_or("Profile not found.")?;
        // The selection goes with the first message only (and not at all if the user dismissed it).
        let sel = std::mem::take(&mut session.selection);
        (p, if use_selection { sel } else { String::new() })
    };

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    *state.cancel.lock().unwrap() = Some(tx);
    let result = tokio::select! {
        r = claude::ask(&settings, &profile, &text, &selection) => r,
        _ = rx => Err("Cancelled.".into()),
    };
    state.cancel.lock().unwrap().take();
    if result.is_err() && !selection.is_empty() {
        // Give the selection back so a retry still has it.
        state.session.lock().unwrap().selection = selection;
    }
    result
}

#[tauri::command]
fn cancel(app: AppHandle) {
    cancel_inner(&app);
}

/// Hides the popup, gives focus back to the app you were in, and writes the text there.
#[tauri::command]
async fn insert_text(app: AppHandle, text: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.inserting.store(true, Ordering::SeqCst);
    let _ = popup(&app).hide();
    let token = state.session.lock().unwrap().focus_token.clone();
    focus::restore(token.as_deref()).await;
    let method = state.settings().insert_method;
    let r = inject::insert(&app, &text, &method).await;
    state.inserting.store(false, Ordering::SeqCst);
    r
}

#[tauri::command]
fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

/// Mic button: opens the OS voice typing on the popup's text field (Windows: Win+H).
/// Pressing it again toggles it off. The transcription is done by the OS, not by Heyo.
#[tauri::command]
async fn start_dictation(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let state = app.state::<AppState>();
        state.dictating.store(true, Ordering::SeqCst);
        let win = popup(&app);
        let _ = win.set_focus();
        focus::bring_to_front(&win);
        tokio::time::sleep(Duration::from_millis(60)).await;
        inject::voice_typing_shortcut(&app).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Voice typing is only available on Windows for now.".into())
    }
}

/// "Install and restart" in the popup's update banner.
#[tauri::command]
async fn install_update(app: AppHandle) {
    updates::install_pending(&app).await;
}

#[tauri::command]
fn new_conversation(app: AppHandle) -> PopupState {
    let id = app
        .state::<AppState>()
        .session
        .lock()
        .unwrap()
        .profile_id
        .clone();
    claude::clear_conversation(&id);
    popup_state_inner(&app)
}

#[tauri::command]
fn hide_popup(app: AppHandle) {
    hide_popup_inner(&app);
}

#[tauri::command]
fn resize_popup(app: AppHandle, height: f64) {
    let win = popup(&app);
    let height = height.clamp(60.0, 720.0);
    let placement = *app.state::<AppState>().placement.lock().unwrap();
    match placement {
        Some(p) => {
            // Move and resize together so a popup above the caret grows upward.
            let h = (height * p.scale).round() as i32;
            let _ = win.set_position(PhysicalPosition::new(p.left, p.top_for(h)));
            let _ = win.set_size(tauri::PhysicalSize::new(
                (POPUP_WIDTH * p.scale).round() as u32,
                h as u32,
            ));
        }
        None => {
            let _ = win.set_size(LogicalSize::new(POPUP_WIDTH, height));
        }
    }
}

// ------------------------------------------------------------------ commands: settings

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsPayload {
    settings: AppSettings,
    api_key_source: &'static str,
    autostart: bool,
    config_path: String,
    mac: bool,
}

#[tauri::command]
fn get_settings(app: AppHandle) -> SettingsPayload {
    use tauri_plugin_autostart::ManagerExt;
    SettingsPayload {
        settings: app.state::<AppState>().settings(),
        api_key_source: settings::api_key_source(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        config_path: settings::settings_path().display().to_string(),
        mac: cfg!(target_os = "macos"),
    }
}

/// Saves and re-binds shortcuts. Returns shortcut problems (the settings are saved anyway).
#[tauri::command]
fn save_settings(app: AppHandle, new_settings: AppSettings) -> Result<Vec<String>, String> {
    settings::save(&new_settings)?;
    *app.state::<AppState>().settings.lock().unwrap() = new_settings;
    let errors = register_shortcuts(&app);
    rebuild_tray_menu(&app);
    Ok(errors)
}

/// While the settings page records a shortcut, the global ones must not fire.
#[tauri::command]
fn suspend_shortcuts(app: AppHandle, suspend: bool) {
    if suspend {
        let _ = app.global_shortcut().unregister_all();
        mouse::set_bindings(&app, vec![]); // let the recorder see mouse buttons
    } else {
        register_shortcuts(&app);
    }
}

#[tauri::command]
fn set_api_key(key: String) -> Result<&'static str, String> {
    settings::set_api_key(&key)?;
    Ok(settings::api_key_source())
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_claude(path: String) -> Result<String, String> {
    claude::claude_code_version(&path).await
}

#[tauri::command]
async fn pick_files(app: AppHandle) -> Vec<String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Knowledge files")
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .map(|f| f.to_string())
            .collect()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Code repository")
            .blocking_pick_folder()
            .map(|f| f.to_string())
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    open_settings_window(&app);
}

fn open_settings_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    match WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Heyo")
        .inner_size(900.0, 640.0)
        .min_inner_size(760.0, 520.0)
        .center()
        .build()
    {
        Ok(w) => {
            let _ = w.set_focus();
        }
        Err(e) => log(&format!("settings window: {e}")),
    }
}

// ------------------------------------------------------------------ tray

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let s = app.state::<AppState>().settings();
    let menu = Menu::new(app)?;
    for p in &s.profiles {
        let label = if p.hotkey.is_empty() {
            p.name.clone()
        } else {
            format!("{}\t{}", p.name, p.hotkey)
        };
        menu.append(&MenuItem::with_id(
            app,
            format!("profile:{}", p.id),
            label,
            true,
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "settings",
        "Settings…",
        true,
        None::<&str>,
    )?)?;
    let update_label = match updates::pending_version(app) {
        Some(v) => format!("Install update v{v}…"),
        None => format!("Check for updates (v{})", app.package_info().version),
    };
    menu.append(&MenuItem::with_id(
        app,
        "update",
        update_label,
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        "Quit Heyo",
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

fn rebuild_tray_menu(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), build_tray_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

// ------------------------------------------------------------------ app

pub fn run() {
    let first_run = !settings::settings_path().exists();
    let initial = settings::load();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            open_settings_window(app)
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        on_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .manage(AppState {
            settings: Mutex::new(initial),
            session: Mutex::new(Session::default()),
            shortcuts: Mutex::new(HashMap::new()),
            cancel: Mutex::new(None),
            inserting: AtomicBool::new(false),
            dictating: AtomicBool::new(false),
            opened_at: Mutex::new(Instant::now()),
            placement: Mutex::new(None),
            pending_update: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            popup_state,
            switch_profile,
            ask,
            cancel,
            insert_text,
            copy_text,
            start_dictation,
            install_update,
            new_conversation,
            hide_popup,
            resize_popup,
            get_settings,
            save_settings,
            suspend_shortcuts,
            set_api_key,
            set_autostart,
            check_claude,
            pick_files,
            pick_folder,
            open_settings,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory); // no Dock icon

            let handle = app.handle().clone();
            let menu = build_tray_menu(&handle)?;
            let mut tray = TrayIconBuilder::with_id("main")
                .tooltip("Heyo")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    let id = event.id().as_ref();
                    if id == "settings" {
                        open_settings_window(app);
                    } else if id == "update" {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if updates::pending_version(&app).is_some() {
                                updates::install_pending(&app).await
                            } else {
                                updates::check(&app, true).await
                            }
                        });
                    } else if id == "quit" {
                        app.exit(0);
                    } else if let Some(pid) = id.strip_prefix("profile:") {
                        let (app, pid) = (app.clone(), pid.to_string());
                        tauri::async_runtime::spawn(async move { open_popup(&app, pid).await });
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // Clicking elsewhere closes the popup (and cancels a pending request).
            let win = popup(&handle);
            let h2 = handle.clone();
            win.on_window_event(move |e| match e {
                WindowEvent::Focused(false) => {
                    let st = h2.state::<AppState>();
                    let just_opened =
                        st.opened_at.lock().unwrap().elapsed() < Duration::from_millis(400);
                    if !st.inserting.load(Ordering::SeqCst)
                        && !st.dictating.load(Ordering::SeqCst)
                        && !just_opened
                    {
                        hide_popup_inner(&h2);
                    }
                }
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    hide_popup_inner(&h2);
                }
                _ => {}
            });

            let errors = register_shortcuts(&handle);
            if first_run || !errors.is_empty() {
                open_settings_window(&handle);
            }
            updates::start_background_checks(&handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start Heyo")
        .run(|_app, event| {
            // Stay in the tray when the settings window is closed.
            if let RunEvent::ExitRequested {
                api, code: None, ..
            } = event
            {
                api.prevent_exit();
            }
        });
}
