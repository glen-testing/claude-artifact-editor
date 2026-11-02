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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
