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

## Branches (git flow)

| Branch | Purpose |
|---|---|
| `main` | Released code. **Every push builds the installers and publishes a GitHub release** for the version in `src-tauri/tauri.conf.json`. |
| `develop` | Integration branch, the default. **Open your pull requests against `develop`.** |
| `feature/<name>`, `fix/<name>` | Your work, branched from `develop`. |
| `release/<x.y.z>` | Branched from `develop` to prepare a release: bump the version, then PR into `main`. |
| `hotfix/<name>` | Urgent fix branched from `main`, PR into `main`, then merged back into `develop`. |

**Cutting a release** (maintainers):

1. `git checkout -b release/0.2.0 develop`
2. Bump the version in `src-tauri/tauri.conf.json` (this names the release), `src-tauri/Cargo.toml` and `package.json`.
3. PR `release/0.2.0` → `main`. Merging it runs the Release workflow, which creates `v0.2.0` with the installers.
4. Merge `main` back into `develop`.

Pushing to `main` without bumping the version rebuilds the installers of the current release in place.

**Update signing.** Installed copies of Heyo check `latest.json` on the latest release and offer to update. Update bundles are signed: the public key is in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`), and the release workflow needs two repository secrets, `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Only the maintainer holds the private key; never commit it. If it's ever lost, generate a new pair with `npx tauri signer generate`, update the public key, and users will have to reinstall once manually.

## CI

Every push to `develop`/`main` and every PR runs `cargo fmt --check`, `cargo clippy -- -D warnings` and a build on Windows, macOS and Linux (`.github/workflows/ci.yml`). Keep it green.

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
