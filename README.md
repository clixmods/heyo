<p align="center">
  <img src="docs/logo.svg" width="96" height="96" alt="Hoppy logo" />
</p>

<h1 align="center">Hoppy</h1>

<p align="center">
  <b>Summon AI from anywhere.</b><br />
  Press a shortcut or a mouse button, a tiny window pops up right where you're typing,<br />
  and the answer is written straight into your app.
</p>

<p align="center">
  <a href="https://github.com/clixmods/hoppy/releases/latest"><b>Download</b></a> ·
  <a href="#build-from-source">Build from source</a> ·
  <a href="CONTRIBUTING.md">Contribute</a>
</p>

<p align="center">
  <a href="https://github.com/clixmods/hoppy/actions/workflows/ci.yml"><img src="https://github.com/clixmods/hoppy/actions/workflows/ci.yml/badge.svg?branch=develop" alt="CI" /></a>
  <a href="https://github.com/clixmods/hoppy/releases/latest"><img src="https://img.shields.io/github/v/release/clixmods/hoppy?label=release" alt="Latest release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/clixmods/hoppy" alt="MIT license" /></a>
</p>

---

Hoppy works in any app: your chat, your email, your IDE, a browser form. You pick a **profile** (instructions + knowledge files + model), type, hit Enter, and the answer is typed into the field you were in. Or shown in the window, chat-style.

Example: a "Translate → EN" profile on your side mouse button. Write your message in any language, press the button, Enter, and it lands in English in your text field. If text is selected, it's sent along and replaced by the answer.

> Status: early, Windows-first. macOS and Linux build and run, with a few gaps listed below. Contributions welcome!

## Features

- **Global shortcuts per profile**: key combos (`Ctrl+Alt+T`), a single non-typing key (F1–F24, Pause, Insert…), or a **mouse side button** (Windows).
- **Opens at your text caret**, above the line you're typing on.
- **Types the answer for you** (clipboard paste that restores your clipboard, or simulated keystrokes).
- **Uses your selection**: highlighted text is sent and replaced.
- **Profiles = projects**: a system prompt, knowledge files, model and thinking effort.
- **Two engines**: [Claude Code](https://claude.com/claude-code) (your Claude subscription, no API key) or the Anthropic API (faster, needs a key, stored in the OS keychain).
- **Chat mode** with per-profile history, and read-only access to a code repository (Claude Code engine).
- **Updates from GitHub**: Hoppy tells you when a new version is out (banner in the popup, tray menu) and installs it when you click. Signed updates, nothing installed behind your back.
- Lives in the system tray, tiny footprint (Rust + Tauri, no bundled browser).

## Profiles

| Setting | What it does |
|---|---|
| Shortcut | opens the window on this profile: a combo, a single non-typing key, or a mouse side button (Windows; that click is then reserved for Hoppy) |
| Instructions | the profile's system prompt |
| Knowledge files | text files added to the context, like files in a claude.ai project |
| Answer | **Type into the field** or **Show in the window** (chat) |
| Engine | **Claude Code** (your Claude subscription) or **API** (Anthropic key) |
| Model, effort | Opus 5.5 by default, low effort for speed |
| Use the selected text | sends the highlighted text and replaces it with the answer |
| Keep the conversation | the profile remembers previous exchanges |
| Code repository | (Claude Code) a folder Claude can read, read-only, to answer |

The **launcher** (`Ctrl+Alt+K` by default) reopens the window on the last used profile.

## In the window

| Key | Action |
|---|---|
| `↵` | send |
| `⇧↵` | new line |
| `Tab` / `⇧Tab` | next / previous profile |
| `Ctrl+1…9` (`⌘` on macOS) | jump to profile n |
| `Ctrl+↵` | (chat) insert the answer into the field |
| `Ctrl+N` | (chat) new conversation |
| `Esc` | cancel the request, otherwise close |

## Install

Grab the installer for your OS from the [latest release](https://github.com/clixmods/hoppy/releases/latest): `…_x64-setup.exe` for Windows, `.dmg` for macOS, `.AppImage` / `.deb` / `.rpm` for Linux. Builds aren't code-signed yet, so Windows SmartScreen and macOS Gatekeeper will ask you to confirm the first time.

## Build from source

Requirements: [Rust](https://rustup.rs), Node.js, and per OS:

- **Windows**: Visual Studio C++ Build Tools, WebView2 (preinstalled on Windows 10/11).
- **macOS**: Xcode Command Line Tools (`xcode-select --install`).
- **Linux**: `webkit2gtk-4.1`, `libayatana-appindicator3`, `libxdo` (see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)), plus `xdotool` to give focus back to the right window on X11.

```sh
npm install
npm run dev      # run the app in development mode
npm run build    # build installers (.msi/.exe, .dmg, .deb/.AppImage) into src-tauri/target/release/bundle
```

For the **Claude Code** engine, install [Claude Code](https://claude.com/claude-code) and run `claude` once in a terminal to sign in (`/login`).
For the **API** engine, paste your key in Settings → General (stored in the OS keychain), or set `ANTHROPIC_API_KEY`.

## Platform notes

- **Windows**: everything works, including mouse buttons and opening at the text caret. Some combos may already be taken by another app; Hoppy reports it in Settings.
- **macOS**: on first use, allow Hoppy in System Settings → Privacy & Security → **Accessibility** (needed to read the selection and type text). Hoppy lives in the menu bar, with no Dock icon. The window opens near the mouse, and mouse-button shortcuts aren't supported yet (map the button to F13–F24 in your mouse software).
- **Linux**: global shortcuts work on X11. On **Wayland**, most compositors block global shortcuts from apps: use an X11 session or bind a desktop shortcut. Same gaps as macOS for the caret and mouse buttons.

## Your data

- Settings: `settings.json` in the config folder (`%APPDATA%\Hoppy`, `~/Library/Application Support/Hoppy`, `~/.config/Hoppy`).
- Conversations of profiles that keep history: `conversations/<profile>.json` in the same folder.
- Log: `hoppy.log` in the same folder (timings and errors only, never your text).
- The API key is stored in the OS keychain, never in a file.
- Your text goes only to the engine you chose (Claude Code or the Anthropic API). No telemetry.

## Known limitations

- After inserting, the clipboard is restored **only if it held text** (a copied image would be lost).
- The Claude Code engine takes a few seconds to start on each request; the API engine is faster.
- Only Anthropic models for now. The engine layer (`src-tauri/src/claude.rs`) is the place to add others (OpenAI, Gemini, Ollama…): see [CONTRIBUTING.md](CONTRIBUTING.md).

## Project layout

```
ui/                    front end (plain HTML/CSS/JS, no framework)
  index.html, popup.*  the popup window
  settings.*           the settings window
  base.css             shared design tokens (light and dark)
src-tauri/src/
  lib.rs               windows, shortcuts, popup placement, tray, Tauri commands
  settings.rs          profiles, settings file, API key (keychain)
  claude.rs            engines: Claude Code CLI and Anthropic API
  inject.rs            reading the selection, inserting text
  focus.rs             remembering/restoring the app you were in, reading the text caret
  mouse.rs             mouse-button shortcuts (Windows low-level hook)
```

## Contributing

Issues and pull requests are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) first. Be kind, and credit others' work.

## License

[MIT](LICENSE) © clixmods and contributors.
