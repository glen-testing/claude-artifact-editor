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

const EXTRACTION_SCRIPT: &str = r#"
(function() {
  function isArtifactPage() {
    return /\/artifact\//.test(window.location.pathname)
        || /\/code\/artifact\//.test(window.location.pathname);
  }

  let importBar = null;
  let statusEl = null;
  let importBtn = null;

  function createImportBar() {
    if (importBar) return;
    if (!document.body) return;

    importBar = document.createElement('div');
    importBar.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:99999;background:#1e40af;color:white;display:flex;align-items:center;justify-content:space-between;padding:8px 16px;font-family:system-ui,sans-serif;font-size:14px;box-shadow:0 2px 8px rgba(0,0,0,0.3);';

    statusEl = document.createElement('span');
    importBar.appendChild(statusEl);

    importBtn = document.createElement('button');
    importBtn.textContent = 'Import to Editor';
    importBtn.style.cssText = 'padding:6px 16px;background:white;color:#1e40af;border:none;border-radius:6px;font-size:14px;font-weight:600;cursor:pointer;opacity:0.5;';
    importBtn.disabled = true;
    importBar.appendChild(importBtn);

    document.body.prepend(importBar);

    importBtn.addEventListener('click', async function() {
      var html = extractHtml();
      if (!html) return;
      statusEl.textContent = 'Importing…';
      importBtn.disabled = true;
      try {
        await window.__TAURI__.event.emit('artifact-imported', html);
        window.close();
      } catch(e) {
        statusEl.textContent = 'Import failed: ' + e;
      }
    });

    if (isArtifactPage()) {
      statusEl.textContent = 'Looking for artifact content…';
      startPolling();
    } else {
      statusEl.textContent = 'Navigate to an artifact page, then import.';
    }
  }

  function startPolling() {
    var attempts = 0;
    var check = setInterval(function() {
      attempts++;
      var html = extractHtml();
      if (html) {
        clearInterval(check);
        statusEl.textContent = 'Artifact ready (' + Math.round(html.length / 1024) + ' KB)';
        importBtn.disabled = false;
        importBtn.style.opacity = '1';
      } else if (attempts > 30) {
        clearInterval(check);
        statusEl.textContent = 'Could not find artifact content. Is it fully loaded?';
      }
    }, 1000);
  }

  function extractHtml() {
    // __NEXT_DATA__ JSON blob
    var nd = document.getElementById('__NEXT_DATA__');
    if (nd) {
      try {
        var found = findHtmlInData(JSON.parse(nd.textContent));
        if (found) return found;
      } catch(e) {}
    }

    // Any script[type="application/json"]
    var scripts = document.querySelectorAll('script[type="application/json"]');
    for (var i = 0; i < scripts.length; i++) {
      try {
        var f = findHtmlInData(JSON.parse(scripts[i].textContent));
        if (f) return f;
      } catch(e) {}
    }

    // iframe srcdoc
    var srcdocFrames = document.querySelectorAll('iframe[srcdoc]');
    for (var i = 0; i < srcdocFrames.length; i++) {
      if (srcdocFrames[i].srcdoc.length > 100) return srcdocFrames[i].srcdoc;
    }

    // iframe contentDocument (same-origin only)
    var allFrames = document.querySelectorAll('iframe');
    for (var i = 0; i < allFrames.length; i++) {
      try {
        var doc = allFrames[i].contentDocument;
        if (doc && doc.documentElement && doc.documentElement.outerHTML.length > 200) {
          return '<!DOCTYPE html>\n' + doc.documentElement.outerHTML;
        }
      } catch(e) {}
    }

    return null;
  }

  function findHtmlInData(v) {
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
        var f = findHtmlInData(v[i]);
        if (f) return f;
      }
    } else if (v && typeof v === 'object') {
      var keys = ['content', 'html', 'body', 'published_html', 'source', 'artifact_content'];
      for (var k = 0; k < keys.length; k++) {
        if (v[keys[k]]) {
          var f = findHtmlInData(v[keys[k]]);
          if (f) return f;
        }
      }
      var vals = Object.values(v);
      for (var i = 0; i < vals.length; i++) {
        var f = findHtmlInData(vals[i]);
        if (f) return f;
      }
    }
    return null;
  }

  // Re-run on every navigation within this window
  if (document.body) {
    createImportBar();
  } else {
    document.addEventListener('DOMContentLoaded', createImportBar);
  }
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
    .initialization_script(EXTRACTION_SCRIPT)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
