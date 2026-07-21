use crate::manifest::{rule_allows, AssetIndex, Library, VersionJson};
use crate::paths::{assets_dir, ensure_dirs, libraries_dir, versions_dir};
use futures_util::StreamExt;
use sha1::{Digest, Sha1};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

#[derive(Clone, serde::Serialize)]
pub struct ProgressEvent {
    pub stage: String,
    pub current: u64,
    pub total: u64,
    pub message: String,
}

pub async fn download_file(url: &str, dest: &Path, expected_sha1: Option<&str>) -> Result<(), String> {
    if dest.exists() {
        if let Some(sha) = expected_sha1 {
            if verify_sha1(dest, sha)? {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;

    let mut stream = response.bytes_stream();
    let tmp = dest.with_extension("tmp");
    let mut file = File::create(&tmp).map_err(|e| e.to_string())?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
    }
    drop(file);

    if let Some(sha) = expected_sha1 {
        if !verify_sha1(&tmp, sha)? {
            let _ = fs::remove_file(&tmp);
            return Err(format!("SHA1 mismatch for {}", dest.display()));
        }
    }

    fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    Ok(())
}

fn verify_sha1(path: &Path, expected: &str) -> Result<bool, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha1::new();
    hasher.update(&data);
    let hash = hex::encode(hasher.finalize());
    Ok(hash.eq_ignore_ascii_case(expected))
}

pub async fn install_vanilla(
    app: AppHandle,
    version_id: &str,
    version_url: &str,
) -> Result<String, String> {
    ensure_dirs()?;
    emit(
        &app,
        "fetch",
        0,
        1,
        &format!("Fetching version metadata for {version_id}"),
    );

    let raw = crate::manifest::fetch_version_json(version_url).await?;
    let version: VersionJson = serde_json::from_value(raw.clone()).map_err(|e| e.to_string())?;

    let version_dir = versions_dir().join(version_id);
    fs::create_dir_all(&version_dir).map_err(|e| e.to_string())?;
    fs::write(
        version_dir.join(format!("{version_id}.json")),
        serde_json::to_string_pretty(&raw).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    // Client jar
    if let Some(downloads) = &version.downloads {
        if let Some(client) = &downloads.client {
            emit(&app, "client", 0, 1, "Downloading client jar");
            let jar = version_dir.join(format!("{version_id}.jar"));
            download_file(&client.url, &jar, Some(&client.sha1)).await?;
        }
    }

    // Libraries
    let libs: Vec<&Library> = version
        .libraries
        .iter()
        .filter(|l| rule_allows(&l.rules))
        .collect();
    let total = libs.len() as u64;
    for (i, lib) in libs.iter().enumerate() {
        emit(
            &app,
            "libraries",
            i as u64,
            total,
            &format!("Library {}", lib.name),
        );
        download_library(lib).await?;
    }

    // Assets
    if let Some(index_ref) = &version.asset_index {
        emit(&app, "assets", 0, 1, "Downloading asset index");
        let index_path = assets_dir().join("indexes").join(format!("{}.json", index_ref.id));
        download_file(&index_ref.url, &index_path, Some(&index_ref.sha1)).await?;

        let index: AssetIndex =
            serde_json::from_str(&fs::read_to_string(&index_path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;

        let objects: Vec<_> = index.objects.values().collect();
        let total = objects.len() as u64;
        for (i, obj) in objects.iter().enumerate() {
            if i % 25 == 0 {
                emit(
                    &app,
                    "assets",
                    i as u64,
                    total,
                    &format!("Assets {i}/{total}"),
                );
            }
            let prefix = &obj.hash[..2];
            let path = assets_dir().join("objects").join(prefix).join(&obj.hash);
            let url = format!(
                "https://resources.download.minecraft.net/{prefix}/{}",
                obj.hash
            );
            download_file(&url, &path, Some(&obj.hash)).await?;
        }
    }

    // Default instance
    let instance = crate::paths::instances_dir().join(version_id);
    fs::create_dir_all(instance.join("mods")).map_err(|e| e.to_string())?;

    emit(&app, "done", 1, 1, "Install complete");
    Ok(version_id.to_string())
}

async fn download_library(lib: &Library) -> Result<(), String> {
    if let Some(downloads) = &lib.downloads {
        if let Some(artifact) = &downloads.artifact {
            let dest = libraries_dir().join(&artifact.path);
            download_file(&artifact.url, &dest, Some(&artifact.sha1)).await?;
        }
        if let Some(classifiers) = &downloads.classifiers {
            // Prefer macOS natives classifiers
            for (key, artifact) in classifiers {
                if key.contains("natives-osx")
                    || key.contains("natives-macos")
                    || (cfg!(target_arch = "aarch64") && key.contains("arm64"))
                {
                    let dest = libraries_dir().join(&artifact.path);
                    download_file(&artifact.url, &dest, Some(&artifact.sha1)).await?;
                }
            }
        }
    } else if let Some(base_url) = &lib.url {
        let rel = crate::manifest::maven_path(&lib.name);
        let dest = libraries_dir().join(&rel);
        let url = format!("{base_url}{rel}");
        download_file(&url, &dest, None).await?;
    } else {
        let rel = crate::manifest::maven_path(&lib.name);
        let dest = libraries_dir().join(&rel);
        let url = format!("https://libraries.minecraft.net/{rel}");
        let _ = download_file(&url, &dest, None).await;
    }
    Ok(())
}

fn emit(app: &AppHandle, stage: &str, current: u64, total: u64, message: &str) {
    let _ = app.emit(
        "install-progress",
        ProgressEvent {
            stage: stage.to_string(),
            current,
            total,
            message: message.to_string(),
        },
    );
}

pub fn version_installed(version_id: &str) -> bool {
    versions_dir()
        .join(version_id)
        .join(format!("{version_id}.json"))
        .exists()
}

pub fn list_installed_versions() -> Vec<String> {
    let Ok(entries) = fs::read_dir(versions_dir()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if version_installed(&name) {
                out.push(name);
            }
        }
    }
    out.sort();
    out.reverse();
    out
}

pub fn load_version_json(version_id: &str) -> Result<serde_json::Value, String> {
    let path = versions_dir()
        .join(version_id)
        .join(format!("{version_id}.json"));
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn resolve_version_chain(version_id: &str) -> Result<Vec<serde_json::Value>, String> {
    let mut chain = Vec::new();
    let mut current = Some(version_id.to_string());
    while let Some(id) = current {
        let json = load_version_json(&id)?;
        let inherits = json
            .get("inheritsFrom")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        chain.push(json);
        current = inherits;
    }
    Ok(chain)
}

pub fn natives_dir_for(version_id: &str) -> PathBuf {
    versions_dir().join(version_id).join("natives")
}
