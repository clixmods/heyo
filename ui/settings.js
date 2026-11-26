// Heyo settings: profiles on the left, one form on the right, one Save button.
const { invoke } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);

let data = null;      // payload from get_settings
let draft = null;     // AppSettings being edited
let savedJson = "";   // to detect unsaved changes
let selected = null;  // profile id, or "general"
let mac = false;

// ---------------------------------------------------------------- helpers

const MOUSE_LABELS = { mouse4: "Mouse 4 (back)", mouse5: "Mouse 5 (forward)", mousemiddle: "Middle click" };

function prettyHotkey(hk) {
  if (!hk) return "";
  const parts = hk.split("+").map((p) => MOUSE_LABELS[p.trim().toLowerCase()] ?? p.trim());
  if (!mac) return parts.join("+");
  const map = { ctrl: "⌃", control: "⌃", alt: "⌥", option: "⌥", shift: "⇧", cmd: "⌘", win: "⌘", meta: "⌘", super: "⌘" };
  return parts.map((p) => map[p.toLowerCase()] ?? p).join("");
}

function currentProfile() {
  return draft.profiles.find((p) => p.id === selected);
}

function newId() {
  return Math.random().toString(36).slice(2, 10);
}

function dirty() {
  return JSON.stringify(draft) !== savedJson;
}

function updateSaveBar(msg = "", cls = "") {
  const m = $("saveMsg");
  m.className = cls;
  m.textContent = msg || (dirty() ? "Unsaved changes" : "");
  $("saveBtn").disabled = !dirty();
}

function setSeg(el, value) {
  el.querySelectorAll("button").forEach((b) => b.classList.toggle("on", b.dataset.v === value));
}

function onSeg(el, cb) {
  el.addEventListener("click", (e) => {
    const b = e.target.closest("button");
    if (!b) return;
    setSeg(el, b.dataset.v);
    cb(b.dataset.v);
  });
}

function setHotkeyBtn(el, value) {
  el.textContent = value ? prettyHotkey(value) : "Click to set";
  el.classList.toggle("empty", !value);
}

// ---------------------------------------------------------------- render

function renderSidebar() {
  const nav = $("profileList");
  nav.replaceChildren(
    ...draft.profiles.map((p) => {
      const b = document.createElement("button");
      b.className = "side-item" + (p.id === selected ? " active" : "");
      b.innerHTML = `<span class="name"></span><span class="hk"></span>`;
      b.querySelector(".name").textContent = p.name || "Untitled";
      b.querySelector(".hk").textContent = prettyHotkey(p.hotkey);
      b.addEventListener("click", () => select(p.id));
      return b;
    })
  );
  $("generalBtn").classList.toggle("active", selected === "general");
}

function renderProfile() {
  const p = currentProfile();
  $("profilePane").classList.toggle("hidden", !p);
  $("generalPane").classList.toggle("hidden", selected !== "general");
  if (!p) return;

  $("pName").value = p.name;
  setHotkeyBtn($("pHotkey"), p.hotkey);
  $("pInstructions").value = p.instructions;
  setSeg($("pOutput"), p.output);
  setSeg($("pBackend"), p.backend);
  $("pModel").value = p.model;
  if ($("pModel").value !== p.model) {
    // Unknown model id typed in settings.json: keep it selectable.
    const o = new Option(p.model, p.model);
    $("pModel").add(o);
    $("pModel").value = p.model;
  }
  $("pEffort").value = p.effort || "";
  $("pUseSelection").checked = p.useSelection;
  $("pKeepHistory").checked = p.keepHistory;
  $("pRepo").value = p.repoPath || "";
  $("pRepoField").classList.toggle("hidden", p.backend !== "claude-code");
  $("pBackendHelp").textContent =
    p.backend === "api"
      ? "Faster. Uses your API key (billed per use)."
      : "Uses your Claude subscription through Claude Code. Takes a few seconds to start.";
  renderFiles(p);
}

function renderFiles(p) {
  $("pFiles").replaceChildren(
    ...p.knowledgeFiles.map((f, i) => {
      const li = document.createElement("li");
      const s = document.createElement("span");
      s.textContent = f;
      s.title = f;
      const x = document.createElement("button");
      x.textContent = "Remove";
      x.addEventListener("click", () => {
        p.knowledgeFiles.splice(i, 1);
        renderFiles(p);
        updateSaveBar();
      });
      li.append(s, x);
      return li;
    })
  );
}

function renderGeneral() {
  setHotkeyBtn($("gLauncher"), draft.launcherHotkey);
  $("gClaudePath").value = draft.claudePath;
  setSeg($("gInsert"), draft.insertMethod);
  $("gAutostart").checked = data.autostart;
  $("gCheckUpdates").checked = draft.checkUpdates;
  $("gConfigPath").textContent = "Settings file: " + data.configPath;
  renderKeyStatus(data.apiKeySource);
}

function renderKeyStatus(source) {
  const el = $("gKeyStatus");
  const text = { keychain: "Key saved in the system keychain.", env: "Key read from the ANTHROPIC_API_KEY variable.", none: "No key: profiles using the API won't work." }[source];
  el.textContent = text;
  el.className = "help " + (source === "none" ? "" : "ok");
}

function select(id) {
  selected = id;
  renderSidebar();
  renderProfile();
  if (id === "general") renderGeneral();
}

// ---------------------------------------------------------------- bindings: profile

function bindProfileField(id, key, transform = (v) => v) {
  $(id).addEventListener("input", (e) => {
    const p = currentProfile();
    if (!p) return;
    p[key] = transform(e.target.type === "checkbox" ? e.target.checked : e.target.value);
    if (key === "name") renderSidebar();
    updateSaveBar();
  });
}

bindProfileField("pName", "name");
bindProfileField("pInstructions", "instructions");
bindProfileField("pModel", "model");
bindProfileField("pEffort", "effort");
bindProfileField("pRepo", "repoPath");
$("pModel").addEventListener("change", (e) => { currentProfile().model = e.target.value; updateSaveBar(); });
$("pEffort").addEventListener("change", (e) => { currentProfile().effort = e.target.value; updateSaveBar(); });
$("pUseSelection").addEventListener("change", (e) => { currentProfile().useSelection = e.target.checked; updateSaveBar(); });
$("pKeepHistory").addEventListener("change", (e) => { currentProfile().keepHistory = e.target.checked; updateSaveBar(); });

onSeg($("pOutput"), (v) => { currentProfile().output = v; updateSaveBar(); });
onSeg($("pBackend"), (v) => { currentProfile().backend = v; renderProfile(); updateSaveBar(); });

$("pAddFiles").addEventListener("click", async () => {
  const files = await invoke("pick_files");
  const p = currentProfile();
  for (const f of files) if (!p.knowledgeFiles.includes(f)) p.knowledgeFiles.push(f);
  renderFiles(p);
  updateSaveBar();
});

$("pPickRepo").addEventListener("click", async () => {
  const dir = await invoke("pick_folder");
  if (dir) {
    currentProfile().repoPath = dir;
    $("pRepo").value = dir;
    updateSaveBar();
  }
});

$("pHotkeyClear").addEventListener("click", () => {
  currentProfile().hotkey = "";
  setHotkeyBtn($("pHotkey"), "");
  renderSidebar();
  updateSaveBar();
});

$("addProfile").addEventListener("click", () => {
  const p = {
    id: newId(), name: "New profile", hotkey: "", backend: "claude-code", model: "claude-opus-5-5",
    effort: "low", instructions: "", knowledgeFiles: [], repoPath: "", output: "insert",
    useSelection: true, keepHistory: false,
  };
  draft.profiles.push(p);
  select(p.id);
  $("pName").select();
  updateSaveBar();
});

$("deleteProfile").addEventListener("click", () => {
  const p = currentProfile();
  if (!p || !confirm(`Delete the profile “${p.name}”?`)) return;
  draft.profiles = draft.profiles.filter((x) => x.id !== p.id);
  select(draft.profiles[0]?.id ?? "general");
  updateSaveBar();
});

// ---------------------------------------------------------------- bindings: general

$("generalBtn").addEventListener("click", () => select("general"));
$("gClaudePath").addEventListener("input", (e) => { draft.claudePath = e.target.value; updateSaveBar(); });
onSeg($("gInsert"), (v) => { draft.insertMethod = v; updateSaveBar(); });
$("gCheckUpdates").addEventListener("change", (e) => { draft.checkUpdates = e.target.checked; updateSaveBar(); });
$("gLauncherClear").addEventListener("click", () => {
  draft.launcherHotkey = "";
  setHotkeyBtn($("gLauncher"), "");
  updateSaveBar();
});

$("gCheckClaude").addEventListener("click", async () => {
  const el = $("gClaudeStatus");
  el.className = "help";
  el.textContent = "Checking…";
  try {
    const v = await invoke("check_claude", { path: $("gClaudePath").value });
    el.textContent = "Found: " + v;
    el.className = "help ok";
  } catch (e) {
    el.textContent = "Claude Code not found: " + e + ". Install it (claude.com/claude-code) or enter its full path.";
    el.className = "help err";
  }
});

$("gSaveKey").addEventListener("click", async () => {
  try {
    const source = await invoke("set_api_key", { key: $("gApiKey").value });
    $("gApiKey").value = "";
    data.apiKeySource = source;
    renderKeyStatus(source);
  } catch (e) {
    $("gKeyStatus").textContent = "Error: " + e;
    $("gKeyStatus").className = "help err";
  }
});

$("gAutostart").addEventListener("change", async (e) => {
  try {
    await invoke("set_autostart", { enabled: e.target.checked });
    data.autostart = e.target.checked;
  } catch (err) {
    e.target.checked = !e.target.checked;
    updateSaveBar("Launch at startup: " + err, "err");
  }
});

// ---------------------------------------------------------------- shortcut recorder

let recording = null; // "profile" | "launcher"

// Physical key codes understood by the shortcut parser (layout-independent, so AZERTY works).
const OTHER_CODES = ["Space", "Enter", "Backquote", "Minus", "Equal", "Comma", "Period", "Slash", "Semicolon", "Quote", "BracketLeft", "BracketRight", "Backslash"];

function keyFromCode(code) {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  if (/^Arrow(Up|Down|Left|Right)$/.test(code)) return code;
  return OTHER_CODES.includes(code) ? code : null;
}

function startRecording(target) {
  recording = target;
  $("recKeys").textContent = "…";
  $("recMeta").textContent = mac ? "Cmd" : "Win";
  $("recorder").classList.remove("hidden");
  invoke("suspend_shortcuts", { suspend: true });
}

function stopRecording(value) {
  const target = recording;
  recording = null;
  $("recorder").classList.add("hidden");
  invoke("suspend_shortcuts", { suspend: false });
  if (value === undefined) return;
  if (target === "profile") {
    currentProfile().hotkey = value;
    setHotkeyBtn($("pHotkey"), value);
    renderSidebar();
  } else {
    draft.launcherHotkey = value;
    setHotkeyBtn($("gLauncher"), value);
  }
  updateSaveBar();
}

document.addEventListener("keydown", (e) => {
  if (!recording) return;
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") return stopRecording();
  const mods = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push(mac ? "Cmd" : "Win");
  const key = keyFromCode(e.code);
  $("recKeys").textContent = prettyHotkey([...mods, key ?? ""].filter(Boolean).join("+")) || "…";
  if (key && (mods.length || STANDALONE.test(key))) stopRecording([...mods, key].join("+"));
}, true);

// Keys that work alone because they don't type anything.
const STANDALONE = /^(F([1-9]|1[0-9]|2[0-4])|Pause|ScrollLock|Insert|PrintScreen|NumLock)$/;

// Mouse buttons: side buttons (back/forward) alone or with modifiers, middle button with a modifier.
const MOUSE_BUTTONS = { 1: "MouseMiddle", 3: "Mouse4", 4: "Mouse5" };

function recordMouse(e) {
  if (!recording) return;
  const name = MOUSE_BUTTONS[e.button];
  if (!name) return;
  e.preventDefault();
  e.stopPropagation();
  const mods = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push(mac ? "Cmd" : "Win");
  if (name === "MouseMiddle" && !mods.length) {
    $("recKeys").textContent = "Middle click needs Ctrl, Alt or Shift";
    return;
  }
  stopRecording([...mods, name].join("+"));
}

document.addEventListener("mousedown", recordMouse, true);
// Stop the side buttons from acting as back/forward in this window.
for (const ev of ["mouseup", "auxclick"]) {
  document.addEventListener(ev, (e) => { if (e.button === 3 || e.button === 4) e.preventDefault(); }, true);
}

$("pHotkey").addEventListener("click", () => startRecording("profile"));
$("gLauncher").addEventListener("click", () => startRecording("launcher"));

// ---------------------------------------------------------------- save

$("saveBtn").addEventListener("click", save);
document.addEventListener("keydown", (e) => {
  if ((mac ? e.metaKey : e.ctrlKey) && e.key.toLowerCase() === "s") {
    e.preventDefault();
    if (dirty()) save();
  }
});

async function save() {
  for (const p of draft.profiles) p.name = p.name.trim() || "Untitled";
  try {
    const problems = await invoke("save_settings", { newSettings: draft });
    savedJson = JSON.stringify(draft);
    renderSidebar();
    if (problems.length) updateSaveBar("Saved, but:\n" + problems.join("\n"), "err");
    else updateSaveBar("Saved ✓", "ok");
    $("saveBtn").disabled = true;
  } catch (e) {
    updateSaveBar("Error: " + e, "err");
  }
}

window.addEventListener("beforeunload", (e) => {
  if (dirty()) { e.preventDefault(); e.returnValue = ""; }
});

// ---------------------------------------------------------------- init

(async () => {
  data = await invoke("get_settings");
  mac = data.mac;
  draft = structuredClone(data.settings);
  savedJson = JSON.stringify(draft);
  select(draft.profiles[0]?.id ?? "general");
  updateSaveBar();
})();
