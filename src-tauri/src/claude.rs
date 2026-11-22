//! Talking to Claude, two ways:
//! - Claude Code CLI (`claude -p`): uses your Claude subscription, no API key. Can read a repo.
//! - Anthropic API (Messages API over HTTPS): needs an API key, lower latency.

use crate::settings::{self, AppSettings, BACKEND_API, OUTPUT_INSERT, Profile};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, time::Duration};
use tokio::io::AsyncWriteExt;

const MAX_KNOWLEDGE_BYTES: u64 = 400_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "user" | "assistant"
    pub content: String,
}

/// A profile's running conversation, saved between uses when "keep history" is on.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Conversation {
    /// Claude Code session to resume.
    pub session_id: Option<String>,
    /// Full transcript (sent back to the API; shown in the popup for both backends).
    pub messages: Vec<Message>,
}

fn conversation_path(profile_id: &str) -> PathBuf {
    settings::config_dir()
        .join("conversations")
        .join(format!("{profile_id}.json"))
}

pub fn load_conversation(profile_id: &str) -> Conversation {
    fs::read_to_string(conversation_path(profile_id))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_conversation(profile_id: &str, c: &Conversation) {
    let path = conversation_path(profile_id);
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(c) {
        let _ = fs::write(path, json);
    }
}

pub fn clear_conversation(profile_id: &str) {
    let _ = fs::remove_file(conversation_path(profile_id));
}

/// Instructions + knowledge files (+ formatting rule when the answer is typed into a field).
fn system_prompt(p: &Profile) -> String {
    let mut s = p.instructions.trim().to_string();
    for path in &p.knowledge_files {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        match fs::metadata(path) {
            Ok(m) if m.len() > MAX_KNOWLEDGE_BYTES => {
                crate::log(&format!("knowledge file too large, skipped: {path}"));
            }
            Ok(_) => match fs::read_to_string(path) {
                Ok(text) => s.push_str(&format!(
                    "\n\n<document path=\"{path}\">\n{text}\n</document>"
                )),
                Err(e) => crate::log(&format!("cannot read {path}: {e}")),
            },
            Err(e) => crate::log(&format!("file not found {path}: {e}")),
        }
    }
    if p.output == OUTPUT_INSERT {
        s.push_str(
            "\n\nYour reply is inserted verbatim into the text field the user is typing in. \
             Output only the final text: no preamble, no explanation, no surrounding quotes, no code fences, no markdown unless the input uses it.",
        );
    }
    s
}

fn user_message(text: &str, selection: &str) -> String {
    let text = text.trim();
    if selection.trim().is_empty() {
        return text.to_string();
    }
    let ask = if text.is_empty() {
        "Apply your instructions to the selected text."
    } else {
        text
    };
    format!("<selected_text>\n{selection}\n</selected_text>\n\n{ask}")
}

/// Sends one turn and returns Claude's reply. Updates the saved conversation when the profile keeps history.
pub async fn ask(
    settings: &AppSettings,
    p: &Profile,
    text: &str,
    selection: &str,
) -> Result<String, String> {
    let mut conv = if p.keep_history {
        load_conversation(&p.id)
    } else {
        Conversation::default()
    };
    let user = user_message(text, selection);
    if user.is_empty() {
        return Err("Empty message.".into());
    }
    let system = system_prompt(p);

    let reply = if p.backend == BACKEND_API {
        ask_api(p, &system, &conv.messages, &user).await?
    } else {
        let (reply, session) =
            ask_claude_code(settings, p, &system, conv.session_id.as_deref(), &user).await?;
        conv.session_id = session;
        reply
    };

    if p.keep_history {
        conv.messages.push(Message {
            role: "user".into(),
            content: user,
        });
        conv.messages.push(Message {
            role: "assistant".into(),
            content: reply.clone(),
        });
        save_conversation(&p.id, &conv);
    }
    Ok(reply)
}

// ---------------------------------------------------------------- Claude Code CLI

async fn ask_claude_code(
    settings: &AppSettings,
    p: &Profile,
    system: &str,
    session: Option<&str>,
    user: &str,
) -> Result<(String, Option<String>), String> {
    let exe = if settings.claude_path.trim().is_empty() {
        "claude"
    } else {
        settings.claude_path.trim()
    };
    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(["-p", "--output-format", "json", "--model", &p.model]);
    if !p.effort.is_empty() {
        cmd.args(["--effort", &p.effort]);
    }

    let repo = p.repo_path.trim();
    if !repo.is_empty() {
        // Repo mode: keep Claude Code's own prompt (and the repo's CLAUDE.md), read-only tools.
        cmd.current_dir(repo).args([
            "--append-system-prompt",
            system,
            "--tools",
            "Read,Glob,Grep",
        ]);
    } else {
        // Plain mode: our prompt only, no tools, no user plugins/hooks → faster and predictable.
        let dir = settings::config_dir().join("workdir");
        let _ = fs::create_dir_all(&dir);
        cmd.current_dir(dir)
            .args(["--system-prompt", system, "--tools", "", "--safe-mode"]);
    }

    match (p.keep_history, session) {
        (true, Some(id)) => {
            cmd.args(["--resume", id]);
        }
        (true, None) => {}
        (false, _) => {
            cmd.arg("--no-session-persistence");
        }
    }

    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashing

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "Couldn't start Claude Code (\"{exe}\"): {e}. Install it, or set its path in Settings."
        )
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(user.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        drop(stdin);
    }
    let out = tokio::time::timeout(Duration::from_secs(300), child.wait_with_output())
        .await
        .map_err(|_| "Claude Code didn't answer within 5 minutes.".to_string())?
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let Ok(v) = serde_json::from_str::<Value>(stdout.trim()) else {
        let err = String::from_utf8_lossy(&out.stderr);
        let detail = if err.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            err.trim().to_string()
        };
        return Err(format!(
            "Unexpected reply from Claude Code: {}",
            truncate(&detail, 400)
        ));
    };
    let result = v["result"].as_str().unwrap_or_default().trim().to_string();
    if v["is_error"].as_bool().unwrap_or(false) {
        let hint = if result.contains("authenticate")
            || result.contains("OAuth")
            || result.contains("login")
        {
            "\nRun `claude` once in a terminal and use /login to sign in again."
        } else {
            ""
        };
        return Err(format!("Claude Code: {result}{hint}"));
    }
    let session = v["session_id"].as_str().map(str::to_string);
    Ok((result, session))
}

// ---------------------------------------------------------------- Anthropic API

/// Models that support server-side refusal fallbacks ("default" routes by refusal category).
fn supports_fallbacks(model: &str) -> bool {
    matches!(
        model,
        "claude-opus-5-5"
            | "claude-opus-5"
            | "claude-sonnet-5-5"
            | "claude-fable-5-2"
            | "claude-fable-5-1"
    )
}

async fn ask_api(
    p: &Profile,
    system: &str,
    history: &[Message],
    user: &str,
) -> Result<String, String> {
    let key =
        settings::api_key().ok_or("No API key: add one in Settings (or set ANTHROPIC_API_KEY).")?;

    let mut messages: Vec<Value> = history
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    messages.push(json!({ "role": "user", "content": user }));

    let mut body = json!({
        "model": p.model,
        "max_tokens": 16000,
        "messages": messages,
    });
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    if !p.effort.is_empty() {
        body["output_config"] = json!({ "effort": p.effort });
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json");
    if supports_fallbacks(&p.model) {
        body["fallbacks"] = json!("default");
        req = req.header("anthropic-beta", "server-side-fallback-2026-07-01");
    }

    let resp = req
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Couldn't reach the API: {e}"))?;
    let status = resp.status();
    let v: Value = resp
        .json()
        .await
        .map_err(|e| format!("Unreadable API response: {e}"))?;
    if !status.is_success() {
        let msg = v["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(match status.as_u16() {
            401 => "API key rejected (401). Check it in Settings.".into(),
            429 => format!("Rate limit reached (429): {msg}"),
            500..=599 => {
                format!("The API is overloaded or unavailable ({status}). Try again in a moment.")
            }
            _ => format!("API error {status}: {msg}"),
        });
    }
    if v["stop_reason"] == "refusal" {
        let why = v["stop_details"]["explanation"]
            .as_str()
            .unwrap_or("request declined");
        return Err(format!("Claude declined: {why}"));
    }
    let text: String = v["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    if v["stop_reason"] == "max_tokens" {
        crate::log("answer truncated (max_tokens)");
    }
    Ok(text.trim().to_string())
}

/// For the settings page: is Claude Code reachable?
pub async fn claude_code_version(path: &str) -> Result<String, String> {
    let exe = if path.trim().is_empty() {
        "claude"
    } else {
        path.trim()
    };
    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg("--version")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x0800_0000);
    let out = tokio::time::timeout(Duration::from_secs(15), cmd.output())
        .await
        .map_err(|_| "timed out".to_string())?
        .map_err(|e| format!("not found ({e})"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}
