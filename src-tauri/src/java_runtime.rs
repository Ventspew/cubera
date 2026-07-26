use crate::download::download_file;
use crate::paths::{java_dir, load_settings, save_settings};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

const JAVA_ALL: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[derive(Debug, Clone, Serialize)]
pub struct ManagedJavaInfo {
    pub component: String,
    pub version: String,
    pub path: String,
    pub installed: bool,
}

#[derive(Deserialize)]
struct RuntimeEntry {
    manifest: RuntimeManifestRef,
    version: RuntimeVersion,
}

#[derive(Deserialize)]
struct RuntimeManifestRef {
    url: String,
}

#[derive(Deserialize)]
struct RuntimeVersion {
    name: String,
}

#[derive(Deserialize)]
struct FileManifest {
    files: std::collections::HashMap<String, FileEntry>,
}

#[derive(Deserialize)]
struct FileEntry {
    #[serde(rename = "type")]
    kind: String,
    downloads: Option<FileDownloads>,
    executable: Option<bool>,
}

#[derive(Deserialize)]
struct FileDownloads {
    raw: Option<FileDl>,
    lzma: Option<FileDl>,
}

#[derive(Deserialize)]
struct FileDl {
    url: String,
    sha1: Option<String>,
}

fn platform_key() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "mac-os-arm64"
        } else {
            "mac-os"
        }
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "x86") {
            "linux-i386"
        } else {
            "linux"
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            "windows-arm64"
        } else if cfg!(target_arch = "x86") {
            "windows-x86"
        } else {
            "windows-x64"
        }
    } else {
        "linux"
    }
}

fn component_dir(component: &str) -> PathBuf {
    java_dir().join(platform_key()).join(component)
}

pub fn managed_java_bin(component: &str) -> Option<PathBuf> {
    let base = component_dir(component);
    let candidates = [
        base.join("jre.bundle/Contents/Home/bin/java"),
        base.join("bin/java"),
        base.join("java"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

pub fn list_managed_java() -> Vec<ManagedJavaInfo> {
    let components = [
        "java-runtime-epsilon",
        "java-runtime-delta",
        "java-runtime-gamma",
        "java-runtime-beta",
        "java-runtime-alpha",
        "jre-legacy",
    ];
    components
        .iter()
        .map(|c| {
            let path = managed_java_bin(c);
            ManagedJavaInfo {
                component: (*c).into(),
                version: String::new(),
                path: path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                installed: path.is_some(),
            }
        })
        .collect()
}

pub async fn ensure_java_component(
    app: AppHandle,
    component: &str,
) -> Result<ManagedJavaInfo, String> {
    if let Some(bin) = managed_java_bin(component) {
        return Ok(ManagedJavaInfo {
            component: component.into(),
            version: "installed".into(),
            path: bin.display().to_string(),
            installed: true,
        });
    }
    install_java_component(app, component).await
}

pub async fn install_java_component(
    app: AppHandle,
    component: &str,
) -> Result<ManagedJavaInfo, String> {
    crate::paths::ensure_dirs()?;
    let client = reqwest::Client::new();
    let all: serde_json::Value = client
        .get(JAVA_ALL)
        .header("User-Agent", "Cubera/0.1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let platform = platform_key();
    let mut entry: Option<RuntimeEntry> = None;
    let mut tried = vec![platform.to_string()];
    if platform == "mac-os-arm64" {
        tried.push("mac-os".into());
    }
    for key in &tried {
        if let Some(arr) = all.get(key).and_then(|v| v.get(component)).and_then(|v| v.as_array()) {
            if let Some(first) = arr.first() {
                entry = serde_json::from_value(first.clone()).ok();
                if entry.is_some() {
                    break;
                }
            }
        }
    }
    let entry = entry.ok_or_else(|| {
        format!("Java component `{component}` not available for {platform}")
    })?;

    let _ = app.emit(
        "install-progress",
        crate::download::ProgressEvent {
            stage: "java".into(),
            current: 0,
            total: 1,
            message: format!("Fetching Java {} manifest…", entry.version.name),
        },
    );

    let files: FileManifest = client
        .get(&entry.manifest.url)
        .header("User-Agent", "Cubera/0.1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let dest_root = component_dir(component);
    if dest_root.exists() {
        fs::remove_dir_all(&dest_root).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&dest_root).map_err(|e| e.to_string())?;

    let file_entries: Vec<_> = files
        .files
        .iter()
        .filter(|(_, e)| e.kind == "file")
        .collect();
    let total = file_entries.len() as u64;

    for (i, (rel, meta)) in file_entries.iter().enumerate() {
        if i % 20 == 0 || i + 1 == file_entries.len() {
            let _ = app.emit(
                "install-progress",
                crate::download::ProgressEvent {
                    stage: "java".into(),
                    current: i as u64,
                    total,
                    message: format!("Downloading Java files {i}/{total}"),
                },
            );
        }
        let Some(dls) = &meta.downloads else {
            continue;
        };
        let Some(raw) = &dls.raw else {
            // Skip lzma-only for simplicity — Mojang always provides raw for runtimes
            if dls.lzma.is_some() {
                return Err(format!("Java file `{rel}` only has lzma download"));
            }
            continue;
        };
        let dest = dest_root.join(rel);
        download_file(&raw.url, &dest, raw.sha1.as_deref()).await?;
        if meta.executable.unwrap_or(false) {
            set_executable(&dest)?;
        }
    }

    // Ensure java binary is executable
    if let Some(bin) = managed_java_bin(component) {
        set_executable(&bin)?;
        // Prefer managed Java in settings if none set
        let mut settings = load_settings();
        if settings.java_path.is_none() {
            settings.java_path = Some(bin.display().to_string());
            let _ = save_settings(&settings);
        }
        let _ = app.emit(
            "install-progress",
            crate::download::ProgressEvent {
                stage: "java".into(),
                current: total,
                total,
                message: format!("Java {} ready", entry.version.name),
            },
        );
        Ok(ManagedJavaInfo {
            component: component.into(),
            version: entry.version.name,
            path: bin.display().to_string(),
            installed: true,
        })
    } else {
        Err("Java installed but binary not found".into())
    }
}

fn set_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(perms.mode() | 0o755);
        fs::set_permissions(path, perms).map_err(|e| e.to_string())?;
    }
    let _ = path;
    Ok(())
}

/// Pick best Mojang component for a major version hint.
pub fn component_for_major(major: u32) -> &'static str {
    match major {
        0..=8 => "jre-legacy",
        9..=16 => "java-runtime-alpha",
        17 => "java-runtime-gamma",
        18..=21 => "java-runtime-delta",
        _ => "java-runtime-epsilon",
    }
}
