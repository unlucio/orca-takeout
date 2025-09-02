// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn save_text(path: String, contents: String) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![save_text])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
