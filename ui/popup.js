// Hoppy popup: type (or speak), Enter, done.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const els = {
  panel: $("panel"), profileBtn: $("profileBtn"), profileName: $("profileName"), profileHotkey: $("profileHotkey"),
  modelTag: $("modelTag"), newConvBtn: $("newConvBtn"), settingsBtn: $("settingsBtn"), menu: $("menu"),
  history: $("history"), selection: $("selection"), selText: $("selText"), selClear: $("selClear"),
  shot: $("shot"), shotClear: $("shotClear"),
  input: $("input"), status: $("status"), statusText: $("statusText"), error: $("error"),
  answer: $("answer"), answerText: $("answerText"), insertBtn: $("insertBtn"), copyBtn: $("copyBtn"),
  insertKbd: $("insertKbd"), hints: $("hints"), micBtn: $("micBtn"),
  listening: $("listening"), listenText: $("listenText"), listenKbd: $("listenKbd"),
  update: $("update"), updateText: $("updateText"), updateBtn: $("updateBtn"),
};

let state = null; // PopupState from Rust: { profile, profiles, selection, screenshot, history, look, … }
let busy = false;
let lastAnswer = "";
let menuIndex = -1;

const MODEL_LABELS = {
  "claude-opus-5-5": "Opus 5.5",
  "claude-sonnet-5-5": "Sonnet 5.5",
  "claude-haiku-5-5": "Haiku 5.5",
  "claude-fable-5-2": "Fable 5.2",
};

const mod = () => (state?.mac ? "⌘" : "Ctrl");
const look = () => state?.look ?? { showHints: true, showModel: true, translucent: false };

const MOUSE_LABELS = { mouse4: "Mouse 4", mouse5: "Mouse 5", mousemiddle: "Middle click" };

function prettyHotkey(hk) {
  if (!hk) return "";
  const parts = hk.split("+").map((p) => MOUSE_LABELS[p.trim().toLowerCase()] ?? p.trim());
  if (!state?.mac) return parts.join("+");
  const map = { ctrl: "⌃", control: "⌃", alt: "⌥", option: "⌥", shift: "⇧", cmd: "⌘", win: "⌘", meta: "⌘", super: "⌘" };
  return parts.map((p) => map[p.toLowerCase()] ?? p).join("");
}

function isChat() { return state?.profile?.output === "chat"; }

// ---------------------------------------------------------------- render

function render() {
  document.body.classList.toggle("glass", !!look().translucent);
  const p = state?.profile;
  if (!p) {
    els.profileName.textContent = "No profile";
    els.input.placeholder = "Create a profile in Settings.";
    fit();
    return;
  }
  els.profileName.textContent = p.name;
  $("profileIcon").replaceChildren(profileMark(p.icon));
  els.profileHotkey.textContent = prettyHotkey(p.hotkey);
  const engine = p.backend === "api" ? "API" : "Claude Code";
  els.modelTag.textContent = [MODEL_LABELS[p.model] ?? p.model, p.effort, engine].filter(Boolean).join(" · ");
  els.modelTag.classList.toggle("hidden", !look().showModel);
  els.newConvBtn.classList.toggle("hidden", !p.keepHistory);
  els.micBtn.classList.toggle("hidden", !state.dictation);
  els.update.classList.toggle("hidden", !state.update);
  els.updateText.textContent = state.update ? `Hoppy v${state.update} is available` : "";

  // Selection and screenshot chips
  const sel = (state.selection || "").trim();
  els.selection.classList.toggle("hidden", !sel);
  els.selText.textContent = sel.replace(/\s+/g, " ").slice(0, 140);
  els.selText.title = sel.slice(0, 1000);
  els.shot.classList.toggle("hidden", !state.screenshot);

  const context = sel || state.screenshot;
  els.input.placeholder = context
    ? (isChat() ? "Ask about this… (or just press ↵)" : "↵ to apply, or add instructions…")
    : (isChat() ? "Ask anything…" : "Type your text, it gets written for you…");

  renderHistory();
  renderHints();
  fit();
}

function renderHistory() {
  const msgs = state?.profile?.keepHistory ? state.history || [] : [];
  els.history.replaceChildren(
    ...msgs.map((m) => {
      const d = document.createElement("div");
      d.className = `msg ${m.role}`;
      d.textContent = m.role === "user" ? stripSelection(m.content) : m.content;
      return d;
    })
  );
  els.history.classList.toggle("hidden", msgs.length === 0);
  els.history.scrollTop = els.history.scrollHeight;
}

// Shows "“excerpt”  question" instead of the raw <selected_text> wrapper.
function stripSelection(text) {
  const m = text.match(/^<selected_text>\n([\s\S]*?)\n<\/selected_text>\n\n([\s\S]*)$/);
  if (!m) return text;
  const excerpt = m[1].replace(/\s+/g, " ").slice(0, 60);
  return `“${excerpt}${m[1].length > 60 ? "…" : ""}”  ${m[2]}`;
}

function renderHints() {
  els.insertKbd.textContent = `${mod()} ↵`;
  els.listenKbd.textContent = `${mod()} M`;
  els.hints.classList.toggle("hidden", !look().showHints);
  if (!look().showHints) return;
  const k = (t) => `<kbd>${t}</kbd>`;
  const items = [
    `${k("↵")} send`,
    `${k("⇧↵")} new line`,
    `${k("↑↓")} previous`,
    `${k("Tab")} profile`,
  ];
  if (isChat()) {
    items.push(`${k(mod() + " ↵")} insert`);
    if (state?.profile?.keepHistory) items.push(`${k(mod() + " N")} new chat`);
  }
  if (state?.dictation) items.push(`${k(mod() + " M")} dictate`);
  items.push(`${k("Esc")} close`);
  els.hints.innerHTML = items.map((h) => `<span>${h}</span>`).join("");
}

function renderMenu() {
  const items = state?.profiles || [];
  els.menu.replaceChildren(
    ...items.map((p, i) => {
      const li = document.createElement("li");
      li.className = (p.id === state.profile?.id ? "current " : "") + (i === menuIndex ? "active" : "");
      li.innerHTML = `<span class="n">${i < 9 ? i + 1 : ""}</span><span class="picon"></span><span class="name"></span><span class="hk"></span>`;
      li.querySelector(".picon").replaceChildren(profileMark(p.icon));
      li.querySelector(".name").textContent = p.name;
      li.querySelector(".hk").textContent = prettyHotkey(p.hotkey);
      li.addEventListener("click", () => switchTo(p.id));
      return li;
    })
  );
}

function toggleMenu(show = els.menu.classList.contains("hidden")) {
  els.menu.classList.toggle("hidden", !show);
  if (show) {
    menuIndex = Math.max(0, (state?.profiles || []).findIndex((p) => p.id === state.profile?.id));
    renderMenu();
  }
  fit();
}

function setBusy(on, text = "Thinking…") {
  busy = on;
  els.status.classList.toggle("hidden", !on);
  els.statusText.textContent = text;
  els.input.disabled = on;
  if (!on) els.input.focus();
  fit();
}

function showError(msg) {
  els.error.textContent = msg || "";
  els.error.classList.toggle("hidden", !msg);
  fit();
}

function showAnswer(text) {
  lastAnswer = text;
  els.answerText.textContent = text;
  els.answer.classList.toggle("hidden", !text);
  fit();
}

function autosize() {
  els.input.style.height = "auto";
  els.input.style.height = Math.min(els.input.scrollHeight, 220) + "px";
}

// Resize the native window to the panel. The solid style keeps a 14 px transparent margin
// around the panel for its shadow; the translucent style fills the window (the OS draws the shadow).
let fitQueued = false;
function fit() {
  if (fitQueued) return;
  fitQueued = true;
  requestAnimationFrame(() => {
    fitQueued = false;
    autosize();
    const margin = look().translucent ? 0 : 28;
    const h = Math.ceil(els.panel.getBoundingClientRect().height) + margin;
    invoke("resize_popup", { height: h });
  });
}

// ---------------------------------------------------------------- input history (↑ / ↓)

// What you sent before, newest first, kept on this computer only.
const PAST_KEY = "hoppy.inputHistory";
const PAST_MAX = 100;
let past = loadPast();
let pastIndex = -1; // -1 = the text being typed, not a past one
let draftText = "";

function loadPast() {
  try { return JSON.parse(localStorage.getItem(PAST_KEY)) || []; } catch { return []; }
}

function rememberInput(text) {
  text = text.trim();
  if (!text) return;
  past = [text, ...past.filter((t) => t !== text)].slice(0, PAST_MAX);
  try { localStorage.setItem(PAST_KEY, JSON.stringify(past)); } catch { /* storage unavailable */ }
  pastIndex = -1;
}

function recall(step) {
  const next = pastIndex + step;
  if (next < -1 || next >= past.length) return false;
  if (pastIndex === -1) draftText = els.input.value;
  pastIndex = next;
  els.input.value = pastIndex === -1 ? draftText : past[pastIndex];
  const end = els.input.value.length;
  els.input.setSelectionRange(end, end);
  fit();
  return true;
}

// Only when the caret is on the first line (↑) or the last line (↓), so multi-line editing still works.
function caretOnFirstLine() {
  return !els.input.value.slice(0, els.input.selectionStart).includes("\n");
}
function caretOnLastLine() {
  return !els.input.value.slice(els.input.selectionEnd).includes("\n");
}

// ---------------------------------------------------------------- dictation

// Hoppy listens itself (Windows speech recognition, no OS panel): partial words show in the
// "Listening…" line, finished phrases are typed into the field.
let listening = false;

function setListening(on) {
  listening = on;
  els.micBtn.setAttribute("aria-pressed", String(on));
  els.listening.classList.toggle("hidden", !on);
  els.listenText.textContent = "Listening…";
  fit();
}

async function toggleListening() {
  if (!state?.dictation || busy) return;
  if (listening) {
    invoke("stop_listening");
    setListening(false);
    return;
  }
  showError("");
  setListening(true);
  els.listenText.textContent = "Starting the microphone…";
  try {
    await invoke("start_listening");
    if (listening) els.listenText.textContent = "Listening…";
  } catch (e) {
    setListening(false);
    showError(String(e));
  }
  els.input.focus();
}

function insertAtCaret(text) {
  const el = els.input;
  const before = el.value.slice(0, el.selectionStart);
  const after = el.value.slice(el.selectionEnd);
  const sep = before && !/\s$/.test(before) ? " " : "";
  el.value = before + sep + text + after;
  const pos = (before + sep + text).length;
  el.setSelectionRange(pos, pos);
  fit();
}

listen("dictation-partial", (e) => {
  if (listening) els.listenText.textContent = `Listening… ${e.payload}`;
});
listen("dictation-text", (e) => {
  if (!listening) return;
  insertAtCaret(e.payload);
  els.listenText.textContent = "Listening…";
});
listen("dictation-stopped", () => { if (listening) setListening(false); });

// ---------------------------------------------------------------- actions

function reset() {
  els.input.value = "";
  pastIndex = -1;
  showError("");
  showAnswer("");
  toggleMenu(false);
  setBusy(false);
  if (listening) setListening(false);
}

async function switchTo(id) {
  state = await invoke("switch_profile", { id });
  toggleMenu(false);
  showAnswer("");
  showError("");
  render();
  els.input.focus();
}

function cycleProfile(step) {
  const list = state?.profiles || [];
  if (list.length < 2) return;
  const i = list.findIndex((p) => p.id === state.profile?.id);
  switchTo(list[(i + step + list.length) % list.length].id);
}

async function send() {
  if (busy || !state?.profile) return;
  if (listening) { invoke("stop_listening"); setListening(false); }
  const text = els.input.value;
  const hasSel = !!(state.selection || "").trim();
  if (!text.trim() && !hasSel && !state.screenshot) return;

  rememberInput(text);
  showError("");
  showAnswer("");
  setBusy(true, isChat() ? "Thinking…" : "Writing…");
  try {
    const reply = await invoke("ask", { text, useSelection: hasSel });
    setBusy(false);
    state.selection = "";
    state.screenshot = false;
    if (!isChat()) {
      els.input.value = "";
      await invoke("insert_text", { text: reply });
      reset();
      return;
    }
    // Chat: keep the exchange on screen.
    els.input.value = "";
    if (state.profile.keepHistory) state = await invoke("popup_state");
    render();
    showAnswer(reply);
  } catch (e) {
    setBusy(false);
    showError(String(e));
  }
}

async function insertAnswer() {
  const text = lastAnswer || lastAssistantMessage();
  if (!text) return;
  await invoke("insert_text", { text });
  reset();
}

function lastAssistantMessage() {
  const h = state?.history || [];
  for (let i = h.length - 1; i >= 0; i--) if (h[i].role === "assistant") return h[i].content;
  return "";
}

async function copyAnswer() {
  const text = lastAnswer || lastAssistantMessage();
  if (!text) return;
  await invoke("copy_text", { text });
  els.copyBtn.textContent = "Copied";
  setTimeout(() => (els.copyBtn.textContent = "Copy"), 1200);
}

async function newConversation() {
  state = await invoke("new_conversation");
  showAnswer("");
  render();
  els.input.focus();
}

function close() {
  invoke("hide_popup");
  reset();
}

// ---------------------------------------------------------------- events

els.input.addEventListener("input", () => { pastIndex = -1; fit(); });

document.addEventListener("keydown", (e) => {
  const cmd = state?.mac ? e.metaKey : e.ctrlKey;
  const menuOpen = !els.menu.classList.contains("hidden");

  if (e.key === "Escape") {
    e.preventDefault();
    if (busy) { invoke("cancel"); return; }
    if (listening) { toggleListening(); return; }
    if (menuOpen) { toggleMenu(false); return; }
    close();
    return;
  }
  if (menuOpen && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
    e.preventDefault();
    const n = state.profiles.length;
    menuIndex = (menuIndex + (e.key === "ArrowDown" ? 1 : -1) + n) % n;
    renderMenu();
    return;
  }
  if (menuOpen && e.key === "Enter") {
    e.preventDefault();
    switchTo(state.profiles[menuIndex].id);
    return;
  }
  if (document.activeElement === els.input && !e.shiftKey && !e.altKey && !cmd) {
    if (e.key === "ArrowUp" && caretOnFirstLine() && recall(1)) { e.preventDefault(); return; }
    if (e.key === "ArrowDown" && caretOnLastLine() && recall(-1)) { e.preventDefault(); return; }
  }
  if (e.key === "Tab") {
    e.preventDefault();
    cycleProfile(e.shiftKey ? -1 : 1);
    return;
  }
  if (cmd && /^[1-9]$/.test(e.key)) {
    const p = state?.profiles?.[Number(e.key) - 1];
    if (p) { e.preventDefault(); switchTo(p.id); }
    return;
  }
  if (cmd && e.key === "Enter") {
    e.preventDefault();
    if (isChat()) insertAnswer();
    return;
  }
  if (cmd && e.key.toLowerCase() === "n" && state?.profile?.keepHistory) {
    e.preventDefault();
    newConversation();
    return;
  }
  if (cmd && e.key === ",") {
    e.preventDefault();
    invoke("open_settings");
    return;
  }
  if (cmd && e.key.toLowerCase() === "m") {
    e.preventDefault();
    toggleListening();
    return;
  }
  if (e.key === "Enter" && !e.shiftKey && document.activeElement === els.input) {
    e.preventDefault();
    send();
  }
});

els.profileBtn.addEventListener("click", () => toggleMenu());
els.settingsBtn.addEventListener("click", () => invoke("open_settings"));
els.newConvBtn.addEventListener("click", newConversation);
els.insertBtn.addEventListener("click", insertAnswer);
els.copyBtn.addEventListener("click", copyAnswer);
els.micBtn.addEventListener("click", toggleListening);
els.updateBtn.addEventListener("click", () => {
  els.updateBtn.disabled = true;
  els.updateBtn.textContent = "Installing…";
  invoke("install_update");
});
els.selClear.addEventListener("click", () => {
  state.selection = "";
  render();
  els.input.focus();
});
els.shotClear.addEventListener("click", () => {
  invoke("drop_screenshot");
  state.screenshot = false;
  render();
  els.input.focus();
});

listen("popup-open", (event) => {
  state = event.payload;
  reset();
  render();
  setTimeout(() => els.input.focus(), 30);
});

window.addEventListener("focus", () => { if (!busy) els.input.focus(); });

invoke("popup_state").then((s) => { state = s; render(); });
