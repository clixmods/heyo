# CLAUDE.md

Guidance for AI coding agents (Claude Code and others) working on this repository. Humans: see [README.md](README.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

## What Heyo is

A tray app: a global shortcut (keys or a mouse button) opens a small popup at the user's text caret, sends the typed text (plus the selection) to an AI engine with the profile's instructions, then types the answer back into the app the user was in, or shows it (chat mode).

Stack: **Rust + Tauri 2** back end (`src-tauri/`), **plain HTML/CSS/JS** front end (`ui/`, no framework, no bundler, no npm runtime deps). `withGlobalTauri` is on: the UI calls `window.__TAURI__.core.invoke(...)`.

## Commands

```sh
npm install                   # Tauri CLI only
npm run dev                   # run the app (tauri dev)
npm run build                 # release build + installers
cd src-tauri && cargo build   # fast compile check; must have zero warnings
cd src-tauri && cargo clippy && cargo fmt --check
```

The UI is embedded at compile time: after editing `ui/`, rebuild (or use `npm run dev`).
There are no automated tests yet; verify by running the app.

## Map

| File | Responsibility |
|---|---|
| `src-tauri/src/lib.rs` | app setup, shortcut registration (`register_shortcuts`, `parse_shortcut`), `trigger` → `open_popup`, popup placement (`Placement`, `position_popup`, `resize_popup`), tray menu, all `#[tauri::command]`s |
| `src-tauri/src/settings.rs` | `AppSettings` / `Profile` (serde camelCase, JSON in the OS config dir `Heyo/`), defaults, API key in the OS keychain (`keyring`) |
| `src-tauri/src/claude.rs` | engines: Claude Code CLI (`claude -p --output-format json`) and the Anthropic Messages API; system prompt assembly; per-profile conversation files |
| `src-tauri/src/inject.rs` | capture selection (Ctrl/Cmd+C, Windows clipboard sequence number), insert text (paste with clipboard restore, or typed), modifier release |
| `src-tauri/src/focus.rs` | remember/restore the foreground app per OS; `caret_rect` (Win32 GUI thread info, then MSAA `OBJID_CARET`) |
| `src-tauri/src/updates.rs` | update checks against the GitHub release's `latest.json` (tauri-plugin-updater); never installs without the user's click |
| `src-tauri/src/mouse.rs` | mouse-button shortcuts via a Windows `WH_MOUSE_LL` hook (swallows the bound click) |
| `ui/index.html`, `popup.js`, `popup.css` | popup window |
| `ui/settings.html`, `settings.js`, `settings.css` | settings window (profiles editor, shortcut recorder) |
| `ui/base.css` | shared design tokens, light and dark via `prefers-color-scheme` |

## Conventions

- **Cross-platform**: OS-specific code goes behind `#[cfg(target_os = ...)]` with a working fallback for the other platforms (see `focus.rs`, `mouse.rs`). Windows is the primary target; never break macOS/Linux compilation.
- **Speed matters**: the popup must appear in well under 200 ms after the shortcut. Don't add blocking work to `open_popup`; measure with the timing line it writes to `heyo.log`.
- **Privacy**: never log user text, selections or answers; never write the API key to disk or logs. The log is for timings and errors.
- **No telemetry, no network calls** other than the chosen engine and the update check (GitHub releases, can be turned off in Settings).
- User-facing strings: English, sentence case, short. Errors say what happened and what to do.
- Settings JSON is user-editable: keep fields backward compatible (`#[serde(default)]`).
- Keep dependencies minimal; prefer the Tauri plugins already in `Cargo.toml`.
- The popup window is transparent with a 14 px margin for the shadow; `resize_popup` resizes the native window to the panel height and keeps it anchored above the caret. Avoid CSS transforms/animations on `.panel`: combined with live window resizing they caused rendering glitches in WebView2.

## Branches, CI and releases

- Git flow: work on `feature/*` or `fix/*` from `develop`, PR into `develop` (the default branch). `main` = released code.
- `.github/workflows/ci.yml`: fmt (Windows), `clippy -D warnings` and `cargo build` on Windows, macOS and Linux for every push/PR. Code behind `#[cfg(target_os = ...)]` must compile warning-free on all three.
- `.github/workflows/release.yml`: every push to `main` builds installers with `tauri-action` and attaches them to the release `v<version>` (version from `src-tauri/tauri.conf.json`; bump it with `Cargo.toml` and `package.json` in a `release/x.y.z` branch).
- Never commit secrets, `.env` files, logs, or machine-specific paths. The only token CI uses is the built-in `GITHUB_TOKEN`.

## Adding an AI provider

1. Add a backend constant in `settings.rs` (next to `BACKEND_API`, `BACKEND_CLAUDE_CODE`).
2. Add the request function in `claude.rs` and dispatch on `p.backend` in `ask`.
3. Store its key in the keychain like `api_key` / `set_api_key` (one entry per provider).
4. Add it to the Engine selector and model list in `ui/settings.html` / `settings.js`.
