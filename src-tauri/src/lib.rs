use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

fn orca_root() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join("Library/Application Support/OrcaSlicer")
}

fn user_filament_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let user_root = orca_root().join("user");
    if let Ok(entries) = fs::read_dir(user_root) {
        for e in entries.flatten() {
            let p = e.path().join("filament");
            if p.is_dir() {
                out.push(p);
            }
        }
    }
    out
}

fn system_filament_dir() -> PathBuf {
    orca_root().join("system/OrcaFilamentLibrary/filament")
}

fn system_filament_base_dir() -> PathBuf {
    system_filament_dir().join("base")
}

fn try_file(dir: &Path, name: &str) -> Option<PathBuf> {
    dbg!("trying for file {} in path {}", &name, &dir);
    let fname = if name.ends_with(".json") {
        name.to_string()
    } else {
        format!("{name}.json")
    };
    let cand = dir.join(fname);
    cand.is_file().then_some(cand)
}

/// Search order: user/*/filament → system/filament → system/filament/base
fn find_profile_file(name: &str) -> Option<PathBuf> {
    dbg!("fiding profile {}", &name);
    for d in user_filament_dirs() {
        if let Some(p) = try_file(&d, name) {
            return Some(p);
        }
    }
    if let Some(p) = try_file(&system_filament_dir(), name) {
        return Some(p);
    }
    if let Some(p) = try_file(&system_filament_base_dir(), name) {
        return Some(p);
    }
    None
}

fn load_json(path: &Path) -> Result<Value, String> {
    let mut f = fs::File::open(path).map_err(|e| format!("open {}: {}", path.display(), e))?;
    let mut s = String::new();
    f.read_to_string(&mut s)
        .map_err(|e| format!("read {}: {}", path.display(), e))?;
    serde_json::from_str::<Value>(&s).map_err(|e| format!("parse {}: {}", path.display(), e))
}

fn deep_merge(into: &mut serde_json::Value, from: &serde_json::Value) {
    if let (Some(a), Some(b)) = (into.as_object_mut(), from.as_object()) {
        for (k, v) in b {
            deep_merge(a.entry(k.clone()).or_insert(serde_json::Value::Null), v);
        }
    } else {
        *into = from.clone();
    }
}

/// Returns bottom→top chain
fn resolve_chain(start_name: &str) -> Result<Vec<(String, Value)>, String> {
    dbg!("resolving chain for {}", &start_name);
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut cursor = start_name.to_string();
    dbg!("starting cursor {}", &cursor);

    loop {
        dbg!("looping cursor {}", &cursor);
        if !seen.insert(cursor.clone()) {
            return Err(format!("cycle detected at '{}'", cursor));
        }
        let path = find_profile_file(&cursor)
            .ok_or_else(|| format!("profile not found for '{}'", cursor))?;
        let obj = load_json(&path)?;
        let chain_name = obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(&cursor)
            .to_string();
        chain.push((chain_name.clone(), obj.clone()));
        if let Some(inh) = obj.get("inherits").and_then(Value::as_str) {
            cursor = inh.to_string();
        } else {
            break;
        }
    }

    chain.reverse();
    Ok(chain)
}

fn build_final(chain: &[(String, Value)], final_name: &str) -> Value {
    let mut acc = json!({});
    for (_, obj) in chain {
        deep_merge(&mut acc, obj);
    }
    if let Value::Object(ref mut map) = acc {
        map.remove("inherits");
        map.insert("name".into(), Value::String(final_name.to_string()));
        let from = chain
            .last()
            .and_then(|(_, o)| o.get("from").and_then(Value::as_str))
            .unwrap_or("User");
        map.insert("from".into(), Value::String(from.to_string()));
        map.insert("instantiation".into(), Value::String("true".into()));
        if !map.contains_key("type") {
            map.insert("type".into(), Value::String("filament".into()));
        }
    }
    acc
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

#[tauri::command]
fn build_filament_profile(start: String) -> Result<String, String> {
    dbg!("building profile {}", &start);
    let chain = resolve_chain(&start)?;
    let final_name = chain
        .last()
        .map(|(n, o)| o.get("name").and_then(Value::as_str).unwrap_or(n))
        .unwrap_or(&start);
    let final_json = build_final(&chain, final_name);
    serde_json::to_string_pretty(&final_json).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_filament_profile(start: String, output_path: String) -> Result<String, String> {
    dbg!("exporting filament profile {}", &start);
    let s = build_filament_profile(start)?;
    fs::write(&output_path, s.as_bytes())
        .map_err(|e| format!("write {}: {}", output_path, e))?;
    Ok(output_path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            build_filament_profile,
            export_filament_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}