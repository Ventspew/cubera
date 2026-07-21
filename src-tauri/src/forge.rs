use crate::download::download_file;
use crate::launch::find_java;
use crate::paths::{assets_dir, libraries_dir, load_settings, versions_dir};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Emitter};

/// Extract embedded maven artifacts from a Forge installer jar into libraries/.
pub fn extract_installer_maven(installer_jar: &Path) -> Result<usize, String> {
    let file = fs::File::open(installer_jar).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut count = 0usize;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if !name.starts_with("maven/") || name.ends_with('/') {
            continue;
        }
        let rel = name.trim_start_matches("maven/");
        let dest = libraries_dir().join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
}

/// Run Forge install_profile processors for the CLIENT side.
pub async fn run_forge_processors(
    app: &AppHandle,
    install_profile: &Value,
    installer_jar: &Path,
    mc_version: &str,
) -> Result<(), String> {
    let Some(processors) = install_profile.get("processors").and_then(|v| v.as_array()) else {
        return Ok(());
    };
    if processors.is_empty() {
        return Ok(());
    }

    let java = find_java(load_settings().java_path.as_deref())?;
    let data_map = build_data_map(install_profile, installer_jar, mc_version)?;

    let total = processors.len() as u64;
    for (idx, proc) in processors.iter().enumerate() {
        // Skip server-only processors
        if let Some(sides) = proc.get("sides").and_then(|v| v.as_array()) {
            let client = sides.iter().any(|s| s.as_str() == Some("client"));
            if !client {
                continue;
            }
        }

        let jar_coord = proc
            .get("jar")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "processor missing jar".to_string())?;
        let main_class = find_main_class(jar_coord)?;
        let mut classpath = vec![library_path_for(jar_coord)?];
        if let Some(cps) = proc.get("classpath").and_then(|v| v.as_array()) {
            for c in cps {
                if let Some(coord) = c.as_str() {
                    classpath.push(library_path_for(coord)?);
                }
            }
        }
        let cp = classpath
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(":");

        let mut args = Vec::new();
        if let Some(raw_args) = proc.get("args").and_then(|v| v.as_array()) {
            for a in raw_args {
                let s = a.as_str().unwrap_or("");
                args.push(resolve_token(s, &data_map)?);
            }
        }

        let _ = app.emit(
            "install-progress",
            crate::download::ProgressEvent {
                stage: "forge".into(),
                current: idx as u64,
                total,
                message: format!("Forge processor {}/{}", idx + 1, total),
            },
        );

        let output = Command::new(&java)
            .arg("-cp")
            .arg(&cp)
            .arg(&main_class)
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to run Forge processor: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!(
                "Forge processor failed ({main_class}):\n{stderr}\n{stdout}"
            ));
        }
    }
    Ok(())
}

fn build_data_map(
    profile: &Value,
    installer_jar: &Path,
    mc_version: &str,
) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    let root = crate::paths::data_dir();
    let minecraft_jar = versions_dir()
        .join(mc_version)
        .join(format!("{mc_version}.jar"));

    map.insert("SIDE".into(), "client".into());
    map.insert("ROOT".into(), path_literal(&root));
    map.insert("INSTALLER".into(), path_literal(installer_jar));
    map.insert("MINECRAFT_JAR".into(), path_literal(&minecraft_jar));
    map.insert("LIBRARY_DIR".into(), path_literal(&libraries_dir()));
    map.insert("GAME_DIR".into(), path_literal(&crate::paths::instances_dir().join(mc_version)));

    if let Some(data) = profile.get("data").and_then(|v| v.as_object()) {
        for (key, val) in data {
            let raw = val
                .get("client")
                .and_then(|v| v.as_str())
                .or_else(|| val.as_str())
                .unwrap_or("");
            map.insert(key.clone(), resolve_data_value(raw)?);
        }
    }

    // Second pass: expand {ROOT}-style refs inside data values
    let keys: Vec<String> = map.keys().cloned().collect();
    for _ in 0..3 {
        for key in &keys {
            if let Some(val) = map.get(key).cloned() {
                let resolved = resolve_token(&val, &map)?;
                map.insert(key.clone(), resolved);
            }
        }
    }
    Ok(map)
}

fn resolve_data_value(raw: &str) -> Result<String, String> {
    if raw.starts_with('[') && raw.ends_with(']') {
        let coord = &raw[1..raw.len() - 1];
        return Ok(path_literal(&library_path_for(coord)?));
    }
    if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
        return Ok(raw[1..raw.len() - 1].to_string());
    }
    Ok(raw.to_string())
}

fn resolve_token(token: &str, data: &HashMap<String, String>) -> Result<String, String> {
    let mut out = token.to_string();
    // Replace {KEY} placeholders
    for (key, val) in data {
        let needle = format!("{{{key}}}");
        if out.contains(&needle) {
            out = out.replace(&needle, val);
        }
    }
    if out.starts_with('[') && out.ends_with(']') {
        let coord = &out[1..out.len() - 1];
        return Ok(path_literal(&library_path_for(coord)?));
    }
    // Still unresolved single {KEY}
    if out.starts_with('{') && out.ends_with('}') && !out[1..out.len() - 1].contains('{') {
        let key = &out[1..out.len() - 1];
        if let Some(v) = data.get(key) {
            return Ok(v.clone());
        }
        return Err(format!("Unknown Forge data key: {key}"));
    }
    Ok(out)
}

fn library_path_for(coord: &str) -> Result<PathBuf, String> {
    let path = libraries_dir().join(crate::manifest::maven_path(coord));
    if !path.exists() {
        return Err(format!("Missing library for processor: {coord} ({})", path.display()));
    }
    Ok(path)
}

fn find_main_class(jar_coord: &str) -> Result<String, String> {
    let jar = library_path_for(jar_coord)?;
    let file = fs::File::open(&jar).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut manifest = archive
        .by_name("META-INF/MANIFEST.MF")
        .map_err(|_| "Processor jar missing MANIFEST.MF".to_string())?;
    let mut text = String::new();
    manifest.read_to_string(&mut text).map_err(|e| e.to_string())?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Main-Class:") {
            return Ok(rest.trim().to_string());
        }
    }
    Err(format!("No Main-Class in {jar_coord}"))
}

fn path_literal(path: &Path) -> String {
    path.display().to_string()
}

pub async fn ensure_client_lzma(installer_jar: &Path) -> Result<(), String> {
    // Some processors need data client.lzma extracted from installer
    let file = fs::File::open(installer_jar).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    if let Ok(mut entry) = archive.by_name("data/client.lzma") {
        let dest = versions_dir().join("client.lzma");
        let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        let _ = out.flush();
    }
    Ok(())
}

pub async fn download_profile_libraries(profile: &Value) -> Result<(), String> {
    if let Some(libs) = profile.get("libraries").and_then(|v| v.as_array()) {
        for lib in libs {
            download_one_lib(lib).await?;
        }
    }
    Ok(())
}

async fn download_one_lib(lib: &Value) -> Result<(), String> {
    let name = match lib.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return Ok(()),
    };

    if let Some(artifact) = lib.pointer("/downloads/artifact") {
        let path = artifact.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let url = artifact.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let sha1 = artifact.get("sha1").and_then(|v| v.as_str());
        if !url.is_empty() && !path.is_empty() {
            return download_file(url, &libraries_dir().join(path), sha1).await;
        }
    }

    let base = lib
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://maven.minecraftforge.net/");
    let rel = crate::manifest::maven_path(name);
    let dest = libraries_dir().join(&rel);
    let url = format!("{base}{rel}");
    let _ = download_file(&url, &dest, None).await;
    Ok(())
}

#[allow(dead_code)]
pub fn assets_index_hint() -> PathBuf {
    assets_dir()
}
