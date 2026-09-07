use crate::http;
use crate::manifest::{rule_allows, AssetIndex, Library, VersionJson};
use crate::paths::{assets_dir, ensure_dirs, libraries_dir, versions_dir};
use futures_util::StreamExt;
use sha1::{Digest, Sha1};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

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

    let mut last_err = String::new();
    for attempt in 1..=5u32 {
        let tmp = dest.with_extension("part");
        let _ = fs::remove_file(&tmp);

        match download_once(url, &tmp).await {
            Ok(()) => {
                if let Some(sha) = expected_sha1 {
                    if !verify_sha1(&tmp, sha)? {
                        let _ = fs::remove_file(&tmp);
                        last_err = format!("SHA1 mismatch voor {url}");
                    } else {
                        fs::rename(&tmp, dest).map_err(|e| format!("Rename mislukt: {e}"))?;
                        return Ok(());
                    }
                } else {
                    fs::rename(&tmp, dest).map_err(|e| format!("Rename mislukt: {e}"))?;
                    return Ok(());
                }
            }
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                last_err = e;
            }
        }

        let delay_ms = 300u64 * (1u64 << (attempt - 1).min(4));
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    }

    Err(format!("{last_err} (na 5 pogingen)"))
}

async fn download_once(url: &str, tmp: &Path) -> Result<(), String> {
    let response = http::get_response(url).await?;
    let mut stream = response.bytes_stream();
    let mut file = File::create(tmp).map_err(|e| format!("Kan temp-bestand niet maken: {e}"))?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download afgebroken ({url}): {e}"))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Schrijven mislukt: {e}"))?;
    }
    drop(file);
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
        &format!("Versie-metadata ophalen voor {version_id}"),
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

    if let Some(downloads) = &version.downloads {
        if let Some(client) = &downloads.client {
            emit(&app, "client", 0, 1, "Client jar downloaden");
            let jar = version_dir.join(format!("{version_id}.jar"));
            download_file(&client.url, &jar, Some(&client.sha1)).await?;
        }
    }

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

    if let Some(index_ref) = &version.asset_index {
        emit(&app, "assets", 0, 1, "Asset-index downloaden");
        let index_path = assets_dir()
            .join("indexes")
            .join(format!("{}.json", index_ref.id));
        download_file(&index_ref.url, &index_path, Some(&index_ref.sha1)).await?;

        let index: AssetIndex =
            serde_json::from_str(&fs::read_to_string(&index_path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;

        download_assets(&app, &index).await?;
    }

    let instance = crate::paths::instances_dir().join(version_id);
    fs::create_dir_all(instance.join("mods")).map_err(|e| e.to_string())?;

    emit(&app, "done", 1, 1, "Installatie klaar");
    Ok(version_id.to_string())
}

async fn download_assets(app: &AppHandle, index: &AssetIndex) -> Result<(), String> {
    let mut hashes: Vec<String> = index.objects.values().map(|o| o.hash.clone()).collect();
    hashes.sort();
    hashes.dedup();

    let total = hashes.len() as u64;
    let done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let sem = Arc::new(Semaphore::new(12));
    let mut joins = Vec::new();

    for hash in hashes {
        let permit = sem.clone().acquire_owned().await.map_err(|e| e.to_string())?;
        let app = app.clone();
        let done = done.clone();
        joins.push(tokio::spawn(async move {
            let _permit = permit;
            let prefix = &hash[..2];
            let path = assets_dir().join("objects").join(prefix).join(&hash);
            let url = format!("https://resources.download.minecraft.net/{prefix}/{hash}");
            let result = download_file(&url, &path, Some(&hash)).await;
            let current = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if current % 20 == 0 || current == total {
                emit(
                    &app,
                    "assets",
                    current,
                    total,
                    &format!("Assets {current}/{total}"),
                );
            }
            result
        }));
    }

    for join in joins {
        join.await
            .map_err(|e| format!("Asset-download taak crashed: {e}"))??;
    }
    Ok(())
}

async fn download_library(lib: &Library) -> Result<(), String> {
    if let Some(downloads) = &lib.downloads {
        if let Some(artifact) = &downloads.artifact {
            let dest = libraries_dir().join(&artifact.path);
            download_file(&artifact.url, &dest, Some(&artifact.sha1)).await?;
        }
        if let Some(classifiers) = &downloads.classifiers {
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
