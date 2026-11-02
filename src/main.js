const { invoke } = window.__TAURI__.core;

let dirty = false;
let sourceMode = false;
let originalHtml = "";

const btnOpen = document.getElementById("btn-open");
const btnSave = document.getElementById("btn-save");
const btnSaveAs = document.getElementById("btn-save-as");
const btnSource = document.getElementById("btn-source");
const fileName = document.getElementById("file-name");
const dirtyDot = document.getElementById("dirty-indicator");
const welcome = document.getElementById("welcome");
const frame = document.getElementById("editor-frame");
const sourceView = document.getElementById("source-view");

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

  const clone = doc.documentElement.cloneNode(true);
  clone.querySelectorAll("[data-artifact-editable]").forEach((el) => {
    el.removeAttribute("contenteditable");
    el.removeAttribute("data-artifact-editable");
  });

  const injectedStyle = clone.querySelector("#artifact-editor-injected-style");
  if (injectedStyle) injectedStyle.remove();

  return "<!DOCTYPE html>\n" + clone.outerHTML;
}

function injectEditable(doc) {
  const blockTags = new Set([
    "P", "H1", "H2", "H3", "H4", "H5", "H6",
    "LI", "TD", "TH", "BLOCKQUOTE", "FIGCAPTION",
    "CAPTION", "DT", "DD", "SUMMARY", "PRE",
  ]);

  const textTags = new Set([
    "P", "H1", "H2", "H3", "H4", "H5", "H6",
    "LI", "TD", "TH", "BLOCKQUOTE", "FIGCAPTION",
    "SPAN", "A", "STRONG", "EM", "B", "I", "U", "S",
    "LABEL", "CAPTION", "DT", "DD", "SUMMARY",
    "CODE", "PRE", "CITE", "Q", "ABBR", "TIME",
    "MARK", "SMALL", "SUB", "SUP", "DEL", "INS",
  ]);

  const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_ELEMENT, {
    acceptNode(node) {
      if (node.tagName === "SCRIPT" || node.tagName === "STYLE") {
        return NodeFilter.FILTER_REJECT;
      }
      return NodeFilter.FILTER_ACCEPT;
    },
  });

  const targets = new Set();
  let node;
  while ((node = walker.nextNode())) {
    if (!node.textContent.trim()) continue;

    if (blockTags.has(node.tagName)) {
      targets.add(node);
    } else if (textTags.has(node.tagName)) {
      let covered = false;
      let p = node.parentElement;
      while (p && p !== doc.body) {
        if (targets.has(p)) { covered = true; break; }
        p = p.parentElement;
      }
      if (!covered) targets.add(node);
    }
  }

  targets.forEach((el) => {
    el.setAttribute("contenteditable", "true");
    el.setAttribute("data-artifact-editable", "");
  });

  const style = doc.createElement("style");
  style.id = "artifact-editor-injected-style";
  style.textContent = `
    [data-artifact-editable] {
      outline: 1px dashed rgba(59,130,246,0.25);
      outline-offset: 2px;
      border-radius: 2px;
      min-height: 1em;
    }
    [data-artifact-editable]:focus {
      outline: 2px solid rgba(59,130,246,0.5);
    }
  `;
  doc.head.appendChild(style);

  doc.body.addEventListener("input", () => markDirty());
}

async function loadFile(path) {
  const content = await invoke("open_file", { path });
  originalHtml = content;
  fileName.textContent = path.split("/").pop().split("\\").pop();
  welcome.hidden = true;
  frame.hidden = false;

  frame.srcdoc = content;
  frame.onload = () => {
    injectEditable(frame.contentDocument);
  };

  enableButtons();
  markClean();

  if (sourceMode) {
    toggleSource();
  }
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

function toggleSource() {
  sourceMode = !sourceMode;
  btnSource.classList.toggle("active", sourceMode);

  if (sourceMode) {
    sourceView.value = getEditableHtml();
    frame.hidden = true;
    sourceView.hidden = false;
    sourceView.focus();
  } else {
    const edited = sourceView.value;
    originalHtml = edited;
    frame.srcdoc = edited;
    frame.onload = () => {
      injectEditable(frame.contentDocument);
    };
    sourceView.hidden = true;
    frame.hidden = false;
  }
}

btnOpen.addEventListener("click", openFile);
btnSave.addEventListener("click", saveFile);
btnSaveAs.addEventListener("click", saveFileAs);
btnSource.addEventListener("click", toggleSource);

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
