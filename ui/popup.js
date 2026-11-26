// Heyo popup: type, Enter, done.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const els = {
  panel: $("panel"), profileBtn: $("profileBtn"), profileName: $("profileName"), profileHotkey: $("profileHotkey"),
  modelTag: $("modelTag"), newConvBtn: $("newConvBtn"), settingsBtn: $("settingsBtn"), menu: $("menu"),
  history: $("history"), selection: $("selection"), selText: $("selText"), selClear: $("selClear"),
  input: $("input"), status: $("status"), statusText: $("statusText"), error: $("error"),
  answer: $("answer"), answerText: $("answerText"), insertBtn: $("insertBtn"), copyBtn: $("copyBtn"),
  insertKbd: $("insertKbd"), hints: $("hints"), micBtn: $("micBtn"),
  insertKbd: $("insertKbd"), hints: $("hints"),
  update: $("update"), updateText: $("updateText"), updateBtn: $("updateBtn"),
};

let state = null; // { profile, profiles, selection, history, mac }
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
  const p = state?.profile;
  if (!p) {
    els.profileName.textContent = "No profile";
    els.input.placeholder = "Create a profile in Settings.";
    fit();
    return;
  }
  els.profileName.textContent = p.name;
  els.profileHotkey.textContent = prettyHotkey(p.hotkey);
  const engine = p.backend === "api" ? "API" : "Claude Code";
  els.modelTag.textContent = [MODEL_LABELS[p.model] ?? p.model, p.effort, engine].filter(Boolean).join(" · ");
  els.newConvBtn.classList.toggle("hidden", !p.keepHistory);
  els.micBtn.classList.toggle("hidden", !state.dictation);
  els.update.classList.toggle("hidden", !state.update);
  els.updateText.textContent = state.update ? `Heyo v${state.update} is available` : "";

  // Selection chip
  const sel = (state.selection || "").trim();
  els.selection.classList.toggle("hidden", !sel);
  els.selText.textContent = sel.replace(/\s+/g, " ").slice(0, 140);
  els.selText.title = sel.slice(0, 1000);

  els.input.placeholder = sel
    ? (isChat() ? "Ask about this text… (or just press ↵)" : "↵ to apply, or add instructions…")
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
  const k = (t) => `<kbd>${t}</kbd>`;
  const items = [
    `${k("↵")} send`,
    `${k("⇧↵")} new line`,
    `${k("Tab")} profile`,
  ];
  if (isChat()) {
    items.push(`${k(mod() + " ↵")} insert`);
    if (state?.profile?.keepHistory) items.push(`${k(mod() + " N")} new chat`);
  }
  if (state?.dictation) items.push(`${k(mod() + " M")} dictate`);
  items.push(`${k("Esc")} close`);
  els.hints.innerHTML = items.map((h) => `<span>${h}</span>`).join("");
  els.insertKbd.textContent = `${mod()} ↵`;
}

function renderMenu() {
  const items = state?.profiles || [];
  els.menu.replaceChildren(
    ...items.map((p, i) => {
      const li = document.createElement("li");
      li.className = (p.id === state.profile?.id ? "current " : "") + (i === menuIndex ? "active" : "");
      li.innerHTML = `<span class="n">${i < 9 ? i + 1 : ""}</span><span class="name"></span><span class="hk"></span>`;
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

// Resize the native window to the panel (+ the 14px shadow margin on each side).
let fitQueued = false;
function fit() {
  if (fitQueued) return;
  fitQueued = true;
  requestAnimationFrame(() => {
    fitQueued = false;
    autosize();
    const h = Math.ceil(els.panel.getBoundingClientRect().height) + 28;
    invoke("resize_popup", { height: h });
  });
}

// ---------------------------------------------------------------- actions

function reset() {
  els.input.value = "";
  showError("");
  showAnswer("");
  toggleMenu(false);
  setBusy(false);
  setDictating(false);
}

// ---------------------------------------------------------------- voice typing

// The OS does the transcription (Windows voice typing, Win+H) straight into the text field.
// We can't see when it stops, so the button state is our best guess: it flips on each press.
let dictating = false;

function setDictating(on) {
  dictating = on;
  els.micBtn.setAttribute("aria-pressed", String(on));
}

async function toggleDictation() {
  if (!state?.dictation || busy) return;
  els.input.focus();
  try {
    await invoke("start_dictation");
    setDictating(!dictating);
  } catch (e) {
    showError(String(e));
  }
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
  const text = els.input.value;
  const hasSel = !!(state.selection || "").trim();
  if (!text.trim() && !hasSel) return;

  showError("");
  showAnswer("");
  setBusy(true, isChat() ? "Thinking…" : "Writing…");
  try {
    const reply = await invoke("ask", { text, useSelection: hasSel });
    setBusy(false);
    if (!isChat()) {
      els.input.value = "";
      state.selection = "";
      await invoke("insert_text", { text: reply });
      reset();
      return;
    }
    // Chat: keep the exchange on screen.
    els.input.value = "";
    state.selection = "";
    if (state.profile.keepHistory) {
      state = await invoke("popup_state");
      render();
    } else {
      render();
    }
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

els.input.addEventListener("input", fit);

document.addEventListener("keydown", (e) => {
  const cmd = state?.mac ? e.metaKey : e.ctrlKey;
  const menuOpen = !els.menu.classList.contains("hidden");

  if (e.key === "Escape") {
    e.preventDefault();
    if (busy) { invoke("cancel"); return; }
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
    toggleDictation();
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
els.micBtn.addEventListener("click", toggleDictation);
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

listen("popup-open", (event) => {
  state = event.payload;
  reset();
  render();
  setTimeout(() => els.input.focus(), 30);
});


window.addEventListener("focus", () => { if (!busy) els.input.focus(); });

invoke("popup_state").then((s) => { state = s; render(); });
