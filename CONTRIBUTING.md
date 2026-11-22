# Contributing to Heyo

Thanks for helping! Heyo is open source under the [MIT license](LICENSE). Anyone can contribute.

## Ground rules

- **Be kind.** Assume good intent, review the code, not the person.
- **Credit others.** If you build on someone's work, say so in your PR. Keep copyright notices.
- **Keep it small and fast.** Heyo should open instantly and stay light. No heavy frameworks in the UI, no new background services.
- **Privacy first.** Never log or send the user's text anywhere other than the engine they chose. No telemetry. Secrets go in the OS keychain, never in files or logs.

## Getting started

1. Install the prerequisites listed in the [README](README.md#build-from-source).
2. `npm install`, then `npm run dev`.
3. Before opening a PR, make sure `cargo build` (in `src-tauri/`) passes with **no warnings**, and that `cargo fmt` and `cargo clippy` are clean.
4. Test the flow you changed by hand: shortcut → popup → answer → inserted text. Say in the PR which OS you tested on.

## Pull requests

- One topic per PR, with a clear description of what changes for the user.
- UI text is in English, in sentence case.
- Update the README if you change behavior, settings or shortcuts.
- Screenshots or a short GIF help a lot for UI changes.

## Good first contributions

- **Other AI providers** (OpenAI, Gemini, Mistral, Ollama…): engines live in `src-tauri/src/claude.rs`. Each profile has a `backend` field; a new provider is a new backend value plus its request code, model list and key storage.
- **macOS / Linux parity**: reading the text caret position (`focus.rs`, `caret_rect`) and mouse-button shortcuts (`mouse.rs`).
- **Clipboard**: restore non-text clipboard content (images, files) after inserting.
- **Translations** of the UI.

## Reporting bugs

Open an issue with your OS, what you did, what you expected, and the relevant lines from `heyo.log` (check it doesn't contain anything private first).

## Security

If you find a security issue (for example something that could leak a user's text or API key), please don't open a public issue: contact the maintainer through GitHub first.
