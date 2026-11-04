use std::fs;
use std::sync::Mutex;
use tauri::State;

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

// --- Chrome cookie reading (macOS) ---

#[cfg(target_os = "macos")]
fn chrome_encryption_key() -> Result<Vec<u8>, String> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-w", "-s", "Chrome Safe Storage", "-a", "Chrome"])
        .output()
        .map_err(|e| format!("Keychain access failed: {e}"))?;

    if !output.status.success() {
        return Err("Cannot read Chrome Safe Storage from Keychain — approve the prompt if it appeared".into());
    }

    let password = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password.trim().as_bytes(), b"saltysalt", 1003, &mut key);
    Ok(key.to_vec())
}

#[cfg(not(target_os = "macos"))]
fn chrome_encryption_key() -> Result<Vec<u8>, String> {
    Err("Chrome cookie reading is only supported on macOS for now".into())
}

fn decrypt_cookie_value(encrypted: &[u8], key: &[u8]) -> Result<String, String> {
    if encrypted.is_empty() {
        return Ok(String::new());
    }
    if encrypted.len() < 4 || &encrypted[..3] != b"v10" {
        return String::from_utf8(encrypted.to_vec()).map_err(|e| e.to_string());
    }

    use aes::Aes128;
    use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
    type Dec = cbc::Decryptor<Aes128>;

    let iv = [0x20u8; 16];
    let mut buf = encrypted[3..].to_vec();
    let pt = Dec::new(key.into(), &iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| format!("AES decrypt failed: {e}"))?;
    String::from_utf8(pt.to_vec()).map_err(|e| e.to_string())
}

fn read_chrome_cookies(domain: &str) -> Result<Vec<(String, String)>, String> {
    let key = chrome_encryption_key()?;
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let db_path = format!("{home}/Library/Application Support/Google/Chrome/Default/Cookies");

    if !std::path::Path::new(&db_path).exists() {
        return Err("Chrome cookies DB not found — is Chrome installed?".into());
    }

    let tmp = std::env::temp_dir().join("_ae_cookies.db");
    fs::copy(&db_path, &tmp).map_err(|e| format!("Cannot copy cookies DB (is Chrome running?): {e}"))?;

    let conn = rusqlite::Connection::open_with_flags(
        &tmp,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT name, encrypted_value FROM cookies WHERE host_key LIKE ?1")
        .map_err(|e| e.to_string())?;

    let rows: Vec<(String, Vec<u8>)> = stmt
        .query_map([format!("%{domain}%")], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let _ = fs::remove_file(&tmp);

    Ok(rows
        .into_iter()
        .filter_map(|(name, enc)| {
            decrypt_cookie_value(&enc, &key)
                .ok()
                .filter(|v| !v.is_empty())
                .map(|v| (name, v))
        })
        .collect())
}

// --- Artifact HTML extraction ---

fn extract_artifact_html(page: &str) -> String {
    if let Some(h) = extract_from_next_data(page) {
        return h;
    }
    if let Some(h) = extract_from_srcdoc(page) {
        return h;
    }
    page.to_string()
}

fn extract_from_next_data(html: &str) -> Option<String> {
    let marker = r#"<script id="__NEXT_DATA__" type="application/json">"#;
    let start = html.find(marker)? + marker.len();
    let end = html[start..].find("</script>")? + start;
    let val: serde_json::Value = serde_json::from_str(&html[start..end]).ok()?;
    find_html_string(&val)
}

fn find_html_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => {
            let t = s.trim();
            if s.len() > 200 && (t.starts_with("<!") || t.starts_with("<html") || t.starts_with("<title")) {
                Some(s.clone())
            } else {
                None
            }
        }
        serde_json::Value::Object(m) => {
            for key in ["content", "html", "body", "published_html", "source"] {
                if let Some(found) = m.get(key).and_then(find_html_string) {
                    return Some(found);
                }
            }
            m.values().find_map(find_html_string)
        }
        serde_json::Value::Array(a) => a.iter().find_map(find_html_string),
        _ => None,
    }
}

fn extract_from_srcdoc(html: &str) -> Option<String> {
    let marker = "srcdoc=\"";
    let start = html.find(marker)? + marker.len();
    let rest = html[start..].as_bytes();
    let mut end = 0;
    for (i, &b) in rest.iter().enumerate() {
        if b == b'"' && (i == 0 || rest[i - 1] != b'\\') {
            end = i;
            break;
        }
    }
    if end < 100 {
        return None;
    }
    let encoded = &html[start..start + end];
    Some(
        encoded
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&#x27;", "'")
            .replace("&#x2F;", "/"),
    )
}

#[tauri::command]
async fn fetch_artifact(url: String) -> Result<String, String> {
    if !url.contains("claude.ai") {
        return Err("URL must be from claude.ai".into());
    }

    let cookies = read_chrome_cookies("claude.ai")?;
    let cookie_hdr: String = cookies
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ");

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&url)
        .header("Cookie", &cookie_hdr)
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .header("Accept", "text/html")
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} — are you logged into claude.ai in Chrome?", resp.status()));
    }

    let body = resp.text().await.map_err(|e| e.to_string())?;

    if body.contains("sign in") && body.contains("/login") && body.len() < 50_000 {
        return Err("Session expired — log into claude.ai in Chrome and try again".into());
    }

    Ok(extract_artifact_html(&body))
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
            fetch_artifact,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
