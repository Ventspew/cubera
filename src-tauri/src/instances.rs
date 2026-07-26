use crate::paths::{instances_dir, versions_dir};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

static RUNNING: Lazy<Mutex<HashMap<String, u32>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceMeta {
    pub id: String,
    /// Mojang / loader profile id used to launch (may differ from folder id).
    #[serde(default)]
    pub version_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub jvm_args: Option<String>,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub last_played: Option<String>,
    #[serde(default)]
    pub play_count: u64,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceInfo {
    pub id: String,
    pub version_id: String,
    pub name: String,
    pub notes: String,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub memory_mb: Option<u32>,
    pub jvm_args: Option<String>,
    pub java_path: Option<String>,
    pub last_played: Option<String>,
    pub play_count: u64,
    pub created_at: Option<String>,
    pub mod_count: usize,
    pub running: bool,
}

fn meta_path(id: &str) -> PathBuf {
    instances_dir().join(id).join("cubera-instance.json")
}

pub fn infer_loader(id: &str) -> Option<String> {
    let lower = id.to_lowercase();
    if lower.contains("fabric") {
        Some("fabric".into())
    } else if lower.contains("quilt") {
        Some("quilt".into())
    } else if lower.contains("neoforge") {
        Some("neoforge".into())
    } else if lower.contains("forge") {
        Some("forge".into())
    } else {
        Some("vanilla".into())
    }
}

pub fn infer_game_version(id: &str) -> Option<String> {
    let re = regex::Regex::new(r"(\d+\.\d+(?:\.\d+)?)").ok()?;
    let mut matches: Vec<String> = re
        .captures_iter(id)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();
    if matches.is_empty() {
        return None;
    }
    if id.contains("fabric") || id.contains("quilt") {
        return matches.pop();
    }
    if id.contains("forge") || id.contains("neoforge") {
        return matches.first().cloned();
    }
    matches.first().cloned()
}

pub fn ensure_instance(id: &str) -> Result<InstanceMeta, String> {
    ensure_instance_with_version(id, id)
}

pub fn ensure_instance_with_version(id: &str, version_id: &str) -> Result<InstanceMeta, String> {
    let dir = instances_dir().join(id);
    fs::create_dir_all(dir.join("mods")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("resourcepacks")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("shaderpacks")).map_err(|e| e.to_string())?;

    let path = meta_path(id);
    if path.exists() {
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if let Ok(mut meta) = serde_json::from_str::<InstanceMeta>(&raw) {
            if meta.name.is_empty() {
                meta.name = id.to_string();
            }
            if meta.version_id.is_empty() {
                meta.version_id = version_id.to_string();
            }
            if meta.game_version.is_none() {
                meta.game_version = infer_game_version(&meta.version_id);
            }
            if meta.loader.is_none() {
                meta.loader = infer_loader(&meta.version_id);
            }
            let _ = save_meta(&meta);
            return Ok(meta);
        }
    }

    let now = chrono::Utc::now().to_rfc3339();
    let meta = InstanceMeta {
        id: id.to_string(),
        version_id: version_id.to_string(),
        name: id.to_string(),
        notes: String::new(),
        game_version: infer_game_version(version_id),
        loader: infer_loader(version_id),
        memory_mb: None,
        jvm_args: None,
        java_path: None,
        last_played: None,
        play_count: 0,
        created_at: Some(now),
    };
    save_meta(&meta)?;
    Ok(meta)
}

pub fn save_meta(meta: &InstanceMeta) -> Result<(), String> {
    let dir = instances_dir().join(&meta.id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    fs::write(meta_path(&meta.id), raw).map_err(|e| e.to_string())
}

pub fn load_meta(id: &str) -> Result<InstanceMeta, String> {
    ensure_instance(id)
}

pub fn update_instance(meta: InstanceMeta) -> Result<InstanceMeta, String> {
    let mut next = ensure_instance(&meta.id)?;
    next.name = if meta.name.trim().is_empty() {
        meta.id.clone()
    } else {
        meta.name.trim().to_string()
    };
    next.notes = meta.notes;
    next.memory_mb = meta.memory_mb.filter(|m| *m >= 512);
    next.jvm_args = meta.jvm_args.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    next.java_path = meta
        .java_path
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(gv) = meta.game_version {
        next.game_version = Some(gv);
    }
    if let Some(loader) = meta.loader {
        next.loader = Some(loader);
    }
    if !meta.version_id.is_empty() {
        next.version_id = meta.version_id;
    }
    save_meta(&next)?;
    Ok(next)
}

pub fn mark_played(id: &str) -> Result<(), String> {
    let mut meta = ensure_instance(id)?;
    meta.last_played = Some(chrono::Utc::now().to_rfc3339());
    meta.play_count = meta.play_count.saturating_add(1);
    save_meta(&meta)
}

pub fn list_instances() -> Result<Vec<InstanceInfo>, String> {
    crate::paths::ensure_dirs()?;
    for id in crate::download::list_installed_versions() {
        let _ = ensure_instance_with_version(&id, &id);
    }

    let Ok(entries) = fs::read_dir(instances_dir()) else {
        return Ok(vec![]);
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let meta = match ensure_instance(&id) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let version_json = versions_dir()
            .join(&meta.version_id)
            .join(format!("{}.json", meta.version_id));
        if !version_json.exists() {
            continue;
        }
        let mod_count = crate::modrinth::list_instance_mods(&id)
            .map(|m| m.len())
            .unwrap_or(0);
        out.push(InstanceInfo {
            id: meta.id.clone(),
            version_id: meta.version_id,
            name: meta.name,
            notes: meta.notes,
            game_version: meta.game_version,
            loader: meta.loader,
            memory_mb: meta.memory_mb,
            jvm_args: meta.jvm_args,
            java_path: meta.java_path,
            last_played: meta.last_played,
            play_count: meta.play_count,
            created_at: meta.created_at,
            mod_count,
            running: is_running(&id),
        });
    }

    out.sort_by(|a, b| match (&b.last_played, &a.last_played) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(out)
}

pub fn register_running(id: &str, pid: u32) {
    if let Ok(mut map) = RUNNING.lock() {
        map.insert(id.to_string(), pid);
    }
}

pub fn is_running(id: &str) -> bool {
    let pid = {
        let Ok(map) = RUNNING.lock() else {
            return false;
        };
        match map.get(id).copied() {
            Some(p) => p,
            None => return false,
        }
    };
    if process_alive(pid) {
        true
    } else {
        if let Ok(mut map) = RUNNING.lock() {
            map.remove(id);
        }
        false
    }
}

pub fn kill_instance(id: &str) -> Result<(), String> {
    let pid = {
        let Ok(map) = RUNNING.lock() else {
            return Err("Could not access process list".into());
        };
        map.get(id).copied()
    };
    let Some(pid) = pid else {
        return Err("Instance is not running".into());
    };
    #[cfg(unix)]
    {
        let status = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("Failed to stop process {pid}"));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        return Err("Process kill is only supported on Unix".into());
    }
    if let Ok(mut map) = RUNNING.lock() {
        map.remove(id);
    }
    Ok(())
}

fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

pub fn open_instance_subfolder(instance_id: &str, folder: &str) -> Result<(), String> {
    let allowed = ["mods", "resourcepacks", "shaderpacks", "screenshots", "saves", "."];
    if !allowed.contains(&folder) {
        return Err("Folder not allowed".into());
    }
    let dir = if folder == "." {
        instances_dir().join(instance_id)
    } else {
        instances_dir().join(instance_id).join(folder)
    };
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("open")
        .arg(&dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn duplicate_instance(instance_id: &str, new_name: &str) -> Result<String, String> {
    let name = new_name.trim();
    if name.is_empty() {
        return Err("Name required".into());
    }
    let src_meta = ensure_instance(instance_id)?;
    let src = instances_dir().join(instance_id);
    if !src.exists() {
        return Err("Source instance not found".into());
    }

    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    let mut dest_id = if slug.is_empty() {
        format!("{instance_id}-copy")
    } else {
        format!("{slug}")
    };
    let mut n = 2u32;
    while instances_dir().join(&dest_id).exists() {
        dest_id = format!("{slug}-copy-{n}");
        n += 1;
    }

    copy_dir(&src, &instances_dir().join(&dest_id))?;
    // Rewrite meta for the copy
    let mut meta = src_meta;
    meta.id = dest_id.clone();
    meta.name = name.to_string();
    meta.last_played = None;
    meta.play_count = 0;
    meta.created_at = Some(chrono::Utc::now().to_rfc3339());
    // Keep version_id pointing at the shared profile
    save_meta(&meta)?;
    Ok(dest_id)
}

fn copy_dir(src: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "cubera-instance.json" {
            continue;
        }
        let to = dest.join(entry.file_name());
        if ty.is_dir() {
            if matches!(
                name.as_str(),
                "saves" | "logs" | "crash-reports" | ".cache" | "natives"
            ) {
                continue;
            }
            copy_dir(&entry.path(), &to)?;
        } else if matches!(
            name.as_str(),
            "cubera-launch.log" | "cubera-launch.err.log"
        ) {
            continue;
        } else {
            fs::copy(entry.path(), to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
