use std::fs;
use std::sync::Mutex;
use tauri::{Manager, State};

struct AppState {
    current_file: Mutex<Option<String>>,
}

#[tauri::command]
fn open_file(path: String, state: State<AppState>) -> Result<String, String> {
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    *state.current_file.lock().unwrap() = Some(path);
    Ok(content)
}

#[tauri::command]
fn save_file(content: String, state: State<AppState>) -> Result<(), String> {
    let guard = state.current_file.lock().unwrap();
    let path = guard.as_ref().ok_or("No file open")?;
    fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_file_as(path: String, content: String, state: State<AppState>) -> Result<(), String> {
    fs::write(&path, content).map_err(|e| e.to_string())?;
    *state.current_file.lock().unwrap() = Some(path);
    Ok(())
}

#[tauri::command]
fn get_current_path(state: State<AppState>) -> Option<String> {
    state.current_file.lock().unwrap().clone()
}

// Runs in the top-level frame only: UI bar + fetch interceptor + postMessage listener
const TOP_SCRIPT: &str = r#"
(function() {
  var capturedHtml = null;

  // --- Intercept fetch() to capture artifact HTML from API responses ---
  var origFetch = window.fetch;
  window.fetch = function() {
    var p = origFetch.apply(this, arguments);
    p.then(function(resp) {
      var clone = resp.clone();
      clone.text().then(function(text) {
        if (text.length > 500) {
          try {
            var json = JSON.parse(text);
            var h = findHtml(json);
            if (h && best.depth < 0) onFound(h);
          } catch(e) {
            var t = text.trim();
            if (best.depth < 0 && t.length > 500 && (t.indexOf('<!') === 0 || t.indexOf('<html') === 0)) {
              onFound(text);
            }
          }
        }
      }).catch(function(){});
    }).catch(function(){});
    return p;
  };

  // --- Listen for postMessage from iframes (cross-origin extraction) ---
  // Prefer the deepest frame (the artifact sits inside claude.ai's wrapper frame), then the largest.
  var best = { depth: -1, len: 0 };
  window.addEventListener('message', function(e) {
    var d = e.data;
    if (!d || d.type !== '__artifact_extract__' || !d.html || d.html.length < 200) return;
    var depth = d.depth || 1;
    if (depth > best.depth || (depth === best.depth && d.html.length >= best.len)) {
      best = { depth: depth, len: d.html.length };
      onFound(d.html);
    }
  });

  function onFound(html) {
    capturedHtml = html;
    if (statusEl) {
      statusEl.textContent = 'Artifact ready (' + Math.round(html.length / 1024) + ' KB)';
    }
    if (importBtn) {
      importBtn.disabled = false;
      importBtn.style.opacity = '1';
    }
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  // --- DOM-based extraction (fallback for same-origin iframes / inline data) ---
  function extractFromDom() {
    var nd = document.getElementById('__NEXT_DATA__');
    if (nd) {
      try {
        var f = findHtml(JSON.parse(nd.textContent));
        if (f) return f;
      } catch(e) {}
    }

    var scripts = document.querySelectorAll('script[type="application/json"]');
    for (var i = 0; i < scripts.length; i++) {
      try {
        var f = findHtml(JSON.parse(scripts[i].textContent));
        if (f) return f;
      } catch(e) {}
    }

    var allFrames = document.querySelectorAll('iframe');
    for (var i = 0; i < allFrames.length; i++) {
      var fr = allFrames[i];
      if (fr.srcdoc && fr.srcdoc.length > 200) return fr.srcdoc;
      try {
        var doc = fr.contentDocument;
        if (doc && doc.documentElement && doc.documentElement.outerHTML.length > 200) {
          return '<!DOCTYPE html>\n' + doc.documentElement.outerHTML;
        }
      } catch(e) {}
    }

    return null;
  }

  function findHtml(v) {
    if (typeof v === 'string' && v.length > 200) {
      var t = v.trim();
      if (t.indexOf('<!') === 0 || t.indexOf('<html') === 0
          || t.indexOf('<head') === 0 || t.indexOf('<title') === 0
          || t.indexOf('<style') === 0) {
        return v;
      }
    }
    if (Array.isArray(v)) {
      for (var i = 0; i < v.length; i++) {
        var f = findHtml(v[i]);
        if (f) return f;
      }
    } else if (v && typeof v === 'object') {
      var keys = ['content','html','body','published_html','source','artifact_content'];
      for (var k = 0; k < keys.length; k++) {
        if (v[keys[k]]) { var f = findHtml(v[keys[k]]); if (f) return f; }
      }
      var vals = Object.values(v);
      for (var i = 0; i < vals.length; i++) {
        var f = findHtml(vals[i]);
        if (f) return f;
      }
    }
    return null;
  }

  // --- UI ---
  var statusEl = null;
  var importBtn = null;
  var pollTimer = null;

  function createBar() {
    if (!document.body) return;

    var bar = document.createElement('div');
    bar.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:99999;background:#1e40af;color:white;display:flex;align-items:center;justify-content:space-between;padding:8px 16px;font-family:system-ui,sans-serif;font-size:14px;box-shadow:0 2px 8px rgba(0,0,0,0.3);';

    statusEl = document.createElement('span');
    statusEl.textContent = 'Looking for artifact content…';
    bar.appendChild(statusEl);

    importBtn = document.createElement('button');
    importBtn.textContent = 'Import to Editor';
    importBtn.style.cssText = 'padding:6px 16px;background:white;color:#1e40af;border:none;border-radius:6px;font-size:14px;font-weight:600;cursor:pointer;opacity:0.5;';
    importBtn.disabled = true;
    bar.appendChild(importBtn);

    document.body.prepend(bar);

    importBtn.addEventListener('click', async function() {
      var html = capturedHtml || extractFromDom();
      if (!html) {
        statusEl.textContent = 'No content found yet.';
        return;
      }
      statusEl.textContent = 'Importing…';
      importBtn.disabled = true;
      try {
        await window.__TAURI__.event.emit('artifact-imported', html);
        statusEl.textContent = 'Imported.';
      } catch(e) {
        statusEl.textContent = 'Import failed: ' + e;
      }
    });

    if (capturedHtml) {
      onFound(capturedHtml);
    } else {
      var attempts = 0;
      pollTimer = setInterval(function() {
        attempts++;
        if (capturedHtml) { onFound(capturedHtml); return; }
        var dom = extractFromDom();
        if (dom) { onFound(dom); return; }
        if (attempts > 30) {
          clearInterval(pollTimer);
          pollTimer = null;
          statusEl.textContent = 'Could not find artifact content. Is it fully loaded?';
        }
      }, 1000);
    }
  }

  if (document.body) createBar();
  else document.addEventListener('DOMContentLoaded', createBar);
})();
"#;

// Runs in ALL frames (including cross-origin artifact iframes).
// If we're inside an iframe, extract our own HTML and post it to the parent.
const FRAME_SCRIPT: &str = r#"
(function() {
  var isTop;
  try { isTop = (window === window.top); } catch(e) { isTop = false; }
  if (isTop) return;

  var depth = 0;
  try { for (var w = window; w !== window.top && depth < 20; w = w.parent) depth++; } catch(e) {}

  function trySend() {
    try {
      var el = document.documentElement;
      if (!el) return;
      var html = el.outerHTML;
      if (html && html.length > 200 && document.body && document.body.innerText.trim().length > 0) {
        window.top.postMessage({
          type: '__artifact_extract__',
          depth: depth,
          html: '<!DOCTYPE html>\n' + html
        }, '*');
      }
    } catch(e) {}
  }

  if (document.readyState === 'complete' || document.readyState === 'interactive') {
    setTimeout(trySend, 500);
  }
  window.addEventListener('load', function() { setTimeout(trySend, 500); });
  document.addEventListener('DOMContentLoaded', function() { setTimeout(trySend, 500); });

  var n = 0;
  var poll = setInterval(function() {
    n++;
    trySend();
    if (n >= 15) clearInterval(poll);
  }, 2000);
})();
"#;

#[tauri::command]
async fn open_artifact_window(app: tauri::AppHandle, url: String) -> Result<(), String> {
    if !url.contains("claude.ai") {
        return Err("URL must be from claude.ai".into());
    }

    let parsed = url::Url::parse(&url).map_err(|e| format!("Invalid URL: {e}"))?;

    if let Some(existing) = app.get_webview_window("artifact-fetch") {
        let _ = existing.close();
    }

    tauri::WebviewWindowBuilder::new(
        &app,
        "artifact-fetch",
        tauri::WebviewUrl::External(parsed),
    )
    .title("Import Artifact")
    .inner_size(900.0, 700.0)
    .initialization_script(TOP_SCRIPT)
    .initialization_script_for_all_frames(FRAME_SCRIPT)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn close_artifact_window(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("artifact-fetch") {
        let _ = w.close();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            current_file: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            open_file,
            save_file,
            save_file_as,
            get_current_path,
            open_artifact_window,
            close_artifact_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
