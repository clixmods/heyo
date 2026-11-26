//! Profiles and app settings, stored as JSON in the OS config folder.
//! The API key is kept in the OS keychain (Windows Credential Manager, macOS Keychain, Secret Service on Linux).

use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

pub const BACKEND_CLAUDE_CODE: &str = "claude-code"; // Claude Code CLI: uses your Claude subscription
pub const BACKEND_API: &str = "api"; // Anthropic API: needs an API key, faster

pub const OUTPUT_INSERT: &str = "insert"; // the answer is written into the field you were typing in
pub const OUTPUT_CHAT: &str = "chat"; // the answer is shown in the popup

/// One shortcut = one profile: a "project" (instructions + knowledge files), a model and a behaviour.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub hotkey: String,
    pub backend: String,
    pub model: String,
    pub effort: String,
    /// System prompt: the project's instructions.
    pub instructions: String,
    /// Text files added to the context, like a claude.ai project's knowledge.
    pub knowledge_files: Vec<String>,
    /// Claude Code only: a repository Claude may read (Read/Glob/Grep) to answer.
    pub repo_path: String,
    pub output: String,
    /// Copy the text selected in the target app and send it along (replaced in insert mode).
    pub use_selection: bool,
    /// Keep a running conversation between uses instead of starting fresh every time.
    pub keep_history: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string()[..8].to_string(),
            name: "New profile".into(),
            hotkey: String::new(),
            backend: BACKEND_CLAUDE_CODE.into(),
            model: "claude-opus-5-5".into(),
            effort: "low".into(),
            instructions: String::new(),
            knowledge_files: vec![],
            repo_path: String::new(),
            output: OUTPUT_INSERT.into(),
            use_selection: true,
            keep_history: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub version: u32,
    /// Opens the popup on the last used profile.
    pub launcher_hotkey: String,
    pub claude_path: String,
    /// "paste" = through the clipboard (fast, restored afterwards); "type" = simulated keystrokes.
    pub insert_method: String,
    /// Check GitHub releases in the background and tell the user when a new version is out.
    pub check_updates: bool,
    pub last_profile_id: String,
    pub profiles: Vec<Profile>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: 1,
            launcher_hotkey: "Ctrl+Alt+K".into(),
            claude_path: "claude".into(),
            insert_method: "paste".into(),
            check_updates: true,
            last_profile_id: "translate".into(),
            profiles: vec![
                Profile {
                    id: "translate".into(),
                    name: "Translate → EN".into(),
                    hotkey: "Ctrl+Alt+T".into(),
                    instructions: "You are a translator. Translate the user's message (or the selected text) into natural, idiomatic English.\nKeep the tone, register, punctuation, emojis and formatting. Don't answer the message: translate it.".into(),
                    ..Default::default()
                },
                Profile {
                    id: "fix".into(),
                    name: "Fix writing".into(),
                    hotkey: "Ctrl+Alt+R".into(),
                    instructions: "Fix the spelling, grammar and punctuation of the text, in its original language.\nDon't change its meaning, tone or style. Don't answer the text: return it corrected.".into(),
                    ..Default::default()
                },
                Profile {
                    id: "chat".into(),
                    name: "Chat".into(),
                    hotkey: "Ctrl+Alt+C".into(),
                    effort: "medium".into(),
                    output: OUTPUT_CHAT.into(),
                    use_selection: false,
                    keep_history: true,
                    instructions: "You are a concise assistant. Answer briefly, in the language the user writes in, unless asked otherwise.".into(),
                    ..Default::default()
                },
            ],
        }
    }
}

impl AppSettings {
    pub fn profile(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// The last used profile, or the first one.
    pub fn current_profile(&self) -> Option<&Profile> {
        self.profile(&self.last_profile_id)
            .or_else(|| self.profiles.first())
    }
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|h| {
                        let h = PathBuf::from(h);
                        if cfg!(target_os = "macos") {
                            h.join("Library/Application Support")
                        } else {
                            h.join(".config")
                        }
                    })
                })
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_DIR)
}

const APP_DIR: &str = "Hoppy";
/// Names this app had before, newest first: their settings and API key are carried over once.
const OLD_NAMES: &[&str] = &["Heyo", "TinyClaude"];

/// First launch under the new name: copy settings, conversations and the API key from an older name.
fn migrate_from_old_names() {
    let dir = config_dir();
    if !dir.join("settings.json").exists() {
        let parent = dir.parent().map(PathBuf::from).unwrap_or_default();
        if let Some(old) = OLD_NAMES
            .iter()
            .map(|n| parent.join(n))
            .find(|d| d.join("settings.json").exists())
        {
            let _ = fs::create_dir_all(&dir);
            let _ = fs::copy(old.join("settings.json"), dir.join("settings.json"));
            if let Ok(entries) = fs::read_dir(old.join("conversations")) {
                let _ = fs::create_dir_all(dir.join("conversations"));
                for e in entries.flatten() {
                    let _ = fs::copy(e.path(), dir.join("conversations").join(e.file_name()));
                }
            }
            crate::log(&format!("settings carried over from {}", old.display()));
        }
    }

    let has_key = key_entry()
        .and_then(|e| e.get_password().ok())
        .is_some_and(|k| !k.is_empty());
    if !has_key {
        let old_key = OLD_NAMES.iter().find_map(|n| {
            keyring::Entry::new(n, "anthropic-api-key")
                .ok()?
                .get_password()
                .ok()
                .filter(|k| !k.is_empty())
        });
        if let (Some(k), Some(entry)) = (old_key, key_entry()) {
            let _ = entry.set_password(&k);
            crate::log("API key carried over from the previous app name");
        }
    }
}

pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn load() -> AppSettings {
    migrate_from_old_names();
    let path = settings_path();
    if let Ok(text) = fs::read_to_string(&path) {
        match serde_json::from_str::<AppSettings>(&text) {
            Ok(s) => return s,
            Err(e) => {
                // Keep the broken file aside rather than silently overwriting the user's work.
                let _ = fs::copy(&path, path.with_extension("json.broken"));
                crate::log(&format!("invalid settings.json ({e}), loaded defaults"));
            }
        }
    }
    let s = AppSettings::default();
    let _ = save(&s);
    s
}

pub fn save(s: &AppSettings) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join("settings.json.tmp");
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, settings_path()).map_err(|e| e.to_string())
}

// ---------- API key (OS keychain, falls back to ANTHROPIC_API_KEY) ----------

fn key_entry() -> Option<keyring::Entry> {
    keyring::Entry::new(APP_DIR, "anthropic-api-key").ok()
}

pub fn api_key() -> Option<String> {
    if let Some(k) = key_entry()
        .and_then(|e| e.get_password().ok())
        .filter(|k| !k.is_empty())
    {
        return Some(k);
    }
    std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|k| !k.is_empty())
}

/// "keychain", "env" or "none".
pub fn api_key_source() -> &'static str {
    if key_entry()
        .and_then(|e| e.get_password().ok())
        .is_some_and(|k| !k.is_empty())
    {
        "keychain"
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok_and(|k| !k.is_empty()) {
        "env"
    } else {
        "none"
    }
}

pub fn set_api_key(key: &str) -> Result<(), String> {
    let entry = key_entry().ok_or("system keychain unavailable")?;
    let key = key.trim();
    if key.is_empty() {
        let _ = entry.delete_credential();
        Ok(())
    } else {
        entry.set_password(key).map_err(|e| e.to_string())
    }
}
