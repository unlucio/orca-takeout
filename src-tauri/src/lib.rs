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

fn try_file(dir: &Path, name: &str) -> Option<PathBuf> {
    println!("trying for file {} in path {:?}", &name, &dir);
    let fname = if name.ends_with(".json") {
        name.to_string()
    } else {
        format!("{name}.json")
    };
    let cand = dir.join(fname);
    cand.is_file().then_some(cand)
}

/// Recursively search under `dir` for `<name>.json`
fn search_recursive(dir: &Path, name: &str) -> Option<PathBuf> {
    let fname = if name.ends_with(".json") {
        name.to_string()
    } else {
        format!("{name}.json")
    };

    // Fast check in current dir
    let cand = dir.join(&fname);
    if cand.is_file() {
        return Some(cand);
    }

    // Walk subdirectories
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Some(found) = search_recursive(&p, name) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn find_profile_file(name: &str) -> Option<PathBuf> {
    // check user profiles first
    for d in user_filament_dirs() {
        if let Some(p) = try_file(&d, name) {
            return Some(p);
        }
    }

    // search whole system tree
    let sys_root = orca_root().join("system");
    search_recursive(&sys_root, name)
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
    println!("resolving chain for {}", &start_name);
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut cursor = start_name.to_string();
    // println!("starting cursor {}", &cursor);

    loop {
        // println!("looping cursor {}", &cursor);
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
            println!("found achestor {}", &cursor);
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
    println!("building profile {}", &start);
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
    println!("exporting filament profile {}", &start);
    let s = build_filament_profile(start)?;
    fs::write(&output_path, s.as_bytes())
        .map_err(|e| format!("write {}: {}", output_path, e))?;
    Ok(output_path)
}

#[tauri::command]
fn list_user_filament_profiles() -> Result<Vec<String>, String> {
    use std::collections::BTreeSet;
    let mut names: BTreeSet<String> = BTreeSet::new();

    for d in user_filament_dirs() {
        let Ok(read) = std::fs::read_dir(&d) else { continue };
        for e in read.flatten() {
            let p = e.path();
            if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json") {
                // Prefer the "name" field; fallback to filename (without .json)
                let name = load_json(&p)
                    .ok()
                    .and_then(|v| v.get("name").and_then(|s| s.as_str()).map(|s| s.to_string()))
                    .or_else(|| p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string()));

                if let Some(n) = name {
                    names.insert(n);
                }
            }
        }
    }
    Ok(names.into_iter().collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            build_filament_profile,
            export_filament_profile,
            list_user_filament_profiles
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}