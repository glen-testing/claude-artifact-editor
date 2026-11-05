const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let dirty = false;
let sourceMode = false;
let originalHtml = "";

const btnOpen = document.getElementById("btn-open");
const btnSave = document.getElementById("btn-save");
const btnSaveAs = document.getElementById("btn-save-as");
const btnSource = document.getElementById("btn-source");
const btnFetch = document.getElementById("btn-fetch");
const urlInput = document.getElementById("url-input");
const fileName = document.getElementById("file-name");
const dirtyDot = document.getElementById("dirty-indicator");
const welcome = document.getElementById("welcome");
const frame = document.getElementById("editor-frame");
const sourceView = document.getElementById("source-view");
const formatBar = document.getElementById("format-bar");

function markDirty() {
  if (!dirty) {
    dirty = true;
    dirtyDot.hidden = false;
  }
}

function markClean() {
  dirty = false;
  dirtyDot.hidden = true;
}

function enableButtons() {
  btnSave.disabled = false;
  btnSaveAs.disabled = false;
}

function getEditableHtml() {
  if (sourceMode) {
    return sourceView.value;
  }
  const doc = frame.contentDocument;
  if (!doc) return originalHtml;

  doc.designMode = "off";
  const html = "<!DOCTYPE html>\n" + doc.documentElement.outerHTML;
  doc.designMode = "on";
  return html;
}

function loadContent(html, label) {
  originalHtml = html;
  fileName.textContent = label;
  welcome.hidden = true;
  frame.hidden = false;
  formatBar.hidden = false;

  frame.srcdoc = html;
  frame.onload = () => {
    const doc = frame.contentDocument;
    doc.designMode = "on";
    doc.addEventListener("input", () => markDirty());
  };

  enableButtons();
  markClean();
  if (sourceMode) toggleSource();
}

async function loadFile(path) {
  const content = await invoke("open_file", { path });
  loadContent(content, path.split("/").pop().split("\\").pop());
}

async function openFile() {
  const { open } = window.__TAURI__.dialog;
  const selected = await open({
    filters: [{ name: "HTML", extensions: ["html", "htm"] }],
    multiple: false,
  });
  if (selected) {
    await loadFile(selected);
  }
}

async function saveFile() {
  const html = getEditableHtml();
  try {
    await invoke("save_file", { content: html });
    originalHtml = html;
    markClean();
  } catch (e) {
    await saveFileAs();
  }
}

async function saveFileAs() {
  const { save } = window.__TAURI__.dialog;
  const path = await save({
    filters: [{ name: "HTML", extensions: ["html", "htm"] }],
  });
  if (path) {
    const html = getEditableHtml();
    await invoke("save_file_as", { path, content: html });
    originalHtml = html;
    fileName.textContent = path.split("/").pop().split("\\").pop();
    markClean();
  }
}

async function fetchArtifact() {
  const url = urlInput.value.trim();
  if (!url) return;
  if (!url.includes("claude.ai")) {
    alert("URL must be from claude.ai");
    return;
  }

  btnFetch.disabled = true;
  btnFetch.textContent = "Opening…";

  try {
    await invoke("open_artifact_window", { url });
    urlInput.value = "";
  } catch (e) {
    alert("Failed to open artifact window: " + e);
  } finally {
    btnFetch.disabled = false;
    btnFetch.textContent = "Fetch";
  }
}

function toggleSource() {
  sourceMode = !sourceMode;
  btnSource.classList.toggle("active", sourceMode);

  if (sourceMode) {
    sourceView.value = getEditableHtml();
    frame.hidden = true;
    formatBar.hidden = true;
    sourceView.hidden = false;
    sourceView.focus();
  } else {
    const edited = sourceView.value;
    originalHtml = edited;
    frame.srcdoc = edited;
    frame.onload = () => {
      frame.contentDocument.designMode = "on";
      frame.contentDocument.addEventListener("input", () => markDirty());
    };
    sourceView.hidden = true;
    frame.hidden = false;
    formatBar.hidden = false;
  }
}

formatBar.addEventListener("click", (e) => {
  const btn = e.target.closest("button[data-cmd]");
  if (!btn) return;
  const cmd = btn.dataset.cmd;
  const val = btn.dataset.val || null;
  frame.contentDocument.execCommand(cmd, false, val);
  markDirty();
  frame.contentWindow.focus();
});

btnOpen.addEventListener("click", openFile);
btnSave.addEventListener("click", saveFile);
btnSaveAs.addEventListener("click", saveFileAs);
btnSource.addEventListener("click", toggleSource);
btnFetch.addEventListener("click", fetchArtifact);
urlInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") fetchArtifact();
});

sourceView.addEventListener("input", () => markDirty());

document.addEventListener("keydown", (e) => {
  const mod = e.metaKey || e.ctrlKey;
  if (mod && e.key === "o") {
    e.preventDefault();
    openFile();
  } else if (mod && e.shiftKey && e.key === "s") {
    e.preventDefault();
    saveFileAs();
  } else if (mod && e.key === "s") {
    e.preventDefault();
    saveFile();
  }
});

listen("artifact-imported", (event) => {
  loadContent(event.payload, "artifact:imported");
});
