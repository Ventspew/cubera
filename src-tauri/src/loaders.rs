use crate::download::{download_file, install_vanilla};
use crate::manifest::maven_path;
use crate::paths::{ensure_dirs, libraries_dir, versions_dir};
use serde::Deserialize;
use std::fs;
use std::io::Read;
use tauri::{AppHandle, Emitter};

const FABRIC_META: &str = "https://meta.fabricmc.net/v2";

#[derive(Debug, Deserialize, serde::Serialize, Clone)]
pub struct FabricLoaderVersion {
    pub separator: String,
    pub build: u32,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

pub async fn list_fabric_loaders(game_version: &str) -> Result<Vec<FabricLoaderVersion>, String> {
    let url = format!("{FABRIC_META}/versions/loader/{game_version}");
    let client = reqwest::Client::new();
    let loaders: Vec<serde_json::Value> = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for item in loaders {
        if let Some(loader) = item.get("loader") {
            if let Ok(v) = serde_json::from_value::<FabricLoaderVersion>(loader.clone()) {
                out.push(v);
            }
        }
    }
    Ok(out)
}

pub async fn install_fabric(
    app: AppHandle,
    game_version: &str,
    game_version_url: &str,
    loader_version: &str,
) -> Result<String, String> {
    ensure_dirs()?;
    // Ensure vanilla base exists
    if !versions_dir()
        .join(game_version)
        .join(format!("{game_version}.json"))
        .exists()
    {
        install_vanilla(app.clone(), game_version, game_version_url).await?;
    }

    let profile_id = format!("fabric-loader-{loader_version}-{game_version}");
    let url = format!("{FABRIC_META}/versions/loader/{game_version}/{loader_version}/profile/json");
    let client = reqwest::Client::new();
    let profile: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let dir = versions_dir().join(&profile_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(
        dir.join(format!("{profile_id}.json")),
        serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    if let Some(libs) = profile.get("libraries").and_then(|v| v.as_array()) {
        for lib in libs {
            let name = lib.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let base = lib
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://maven.fabricmc.net/");
            let rel = maven_path(name);
            let dest = libraries_dir().join(&rel);
            let url = format!("{base}{rel}");
            download_file(&url, &dest, None).await?;
        }
    }

    let _ = crate::instances::ensure_instance_with_version(&profile_id, &profile_id);

    Ok(profile_id)
}

// --- Quilt (Fabric-compatible meta API) ---

const QUILT_META: &str = "https://meta.quiltmc.org/v3";

pub async fn list_quilt_loaders(game_version: &str) -> Result<Vec<FabricLoaderVersion>, String> {
    let url = format!("{QUILT_META}/versions/loader/{game_version}");
    let client = reqwest::Client::new();
    let loaders: Vec<serde_json::Value> = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for item in loaders {
        if let Some(loader) = item.get("loader") {
            let version = loader
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if version.is_empty() {
                continue;
            }
            out.push(FabricLoaderVersion {
                separator: ".".into(),
                build: 0,
                maven: loader
                    .get("maven")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                version,
                stable: loader
                    .get("stable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            });
        }
    }
    Ok(out)
}

pub async fn install_quilt(
    app: AppHandle,
    game_version: &str,
    game_version_url: &str,
    loader_version: &str,
) -> Result<String, String> {
    ensure_dirs()?;
    if !versions_dir()
        .join(game_version)
        .join(format!("{game_version}.json"))
        .exists()
    {
        install_vanilla(app.clone(), game_version, game_version_url).await?;
    }

    let profile_id = format!("quilt-loader-{loader_version}-{game_version}");
    let url = format!("{QUILT_META}/versions/loader/{game_version}/{loader_version}/profile/json");
    let client = reqwest::Client::new();
    let profile: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let dir = versions_dir().join(&profile_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(
        dir.join(format!("{profile_id}.json")),
        serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    if let Some(libs) = profile.get("libraries").and_then(|v| v.as_array()) {
        for lib in libs {
            let name = lib.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let base = lib
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://maven.quiltmc.org/repository/release/");
            let rel = maven_path(name);
            let dest = libraries_dir().join(&rel);
            let url = format!("{base}{rel}");
            download_file(&url, &dest, None).await?;
        }
    }

    let _ = crate::instances::ensure_instance_with_version(&profile_id, &profile_id);
    Ok(profile_id)
}

// --- Forge ---

#[derive(Debug, Deserialize, serde::Serialize, Clone)]
pub struct ForgeVersionEntry {
    pub raw: String,
    pub mc: String,
    pub forge: String,
}

pub async fn list_forge_versions(mc_filter: Option<String>) -> Result<Vec<ForgeVersionEntry>, String> {
    let url = "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
    let client = reqwest::Client::new();
    let map: serde_json::Map<String, serde_json::Value> = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for (mc, versions) in map {
        if let Some(ref filter) = mc_filter {
            if &mc != filter {
                continue;
            }
        }
        if let Some(arr) = versions.as_array() {
            for v in arr {
                if let Some(raw) = v.as_str() {
                    // raw like "1.20.1-47.2.0"
                    let forge = raw
                        .strip_prefix(&format!("{mc}-"))
                        .unwrap_or(raw)
                        .to_string();
                    out.push(ForgeVersionEntry {
                        raw: raw.to_string(),
                        mc: mc.clone(),
                        forge,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| b.raw.cmp(&a.raw));
    Ok(out)
}

pub async fn install_forge(
    app: AppHandle,
    mc_version: &str,
    mc_version_url: &str,
    forge_full: &str,
) -> Result<String, String> {
    ensure_dirs()?;
    if !versions_dir()
        .join(mc_version)
        .join(format!("{mc_version}.json"))
        .exists()
    {
        install_vanilla(app.clone(), mc_version, mc_version_url).await?;
    }

    let installer_url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{forge_full}/forge-{forge_full}-installer.jar"
    );
    let installer_path = versions_dir().join(format!("forge-{forge_full}-installer.jar"));
    download_file(&installer_url, &installer_path, None).await?;

    // Extract version.json / install_profile.json from installer
    let file = fs::File::open(&installer_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    let mut version_json: Option<serde_json::Value> = None;
    let mut install_profile: Option<serde_json::Value> = None;

    for name in ["version.json", "install_profile.json"] {
        if let Ok(mut entry) = archive.by_name(name) {
            let mut buf = String::new();
            entry.read_to_string(&mut buf).map_err(|e| e.to_string())?;
            let json: serde_json::Value =
                serde_json::from_str(&buf).map_err(|e| e.to_string())?;
            if name == "version.json" {
                version_json = Some(json);
            } else {
                install_profile = Some(json);
            }
        }
    }

    // Newer installers nest version under install_profile.json "versionInfo" or separate
    if version_json.is_none() {
        if let Some(profile) = &install_profile {
            if let Some(v) = profile.get("versionInfo") {
                version_json = Some(v.clone());
            }
        }
    }

    let mut version_json = version_json.ok_or_else(|| {
        "Could not find version.json in Forge installer (unsupported installer format)".to_string()
    })?;

    let profile_id = version_json
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(&format!("forge-{forge_full}"))
        .to_string();

    // Ensure inheritsFrom
    if version_json.get("inheritsFrom").is_none() {
        if let Some(obj) = version_json.as_object_mut() {
            obj.insert("inheritsFrom".into(), serde_json::json!(mc_version));
        }
    }

    let dir = versions_dir().join(&profile_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(
        dir.join(format!("{profile_id}.json")),
        serde_json::to_string_pretty(&version_json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    // Download forge libraries from version json
    if let Some(libs) = version_json.get("libraries").and_then(|v| v.as_array()) {
        for lib in libs {
            download_forge_library(lib).await?;
        }
    }

    // Also download libraries listed in install_profile + extract embedded maven + run processors
    if let Some(profile) = &install_profile {
        crate::forge::download_profile_libraries(profile).await?;
        let extracted = crate::forge::extract_installer_maven(&installer_path)?;
        let _ = app.emit(
            "install-progress",
            crate::download::ProgressEvent {
                stage: "forge".into(),
                current: 0,
                total: 1,
                message: format!("Extracted {extracted} Forge maven artifacts"),
            },
        );
        crate::forge::ensure_client_lzma(&installer_path).await?;
        crate::forge::run_forge_processors(&app, profile, &installer_path, mc_version).await?;
    } else {
        // Fallback: try client/universal jars from maven
        let forge_jar_rel = format!(
            "net/minecraftforge/forge/{forge_full}/forge-{forge_full}-client.jar"
        );
        let client_url = format!("https://maven.minecraftforge.net/{forge_jar_rel}");
        let client_dest = libraries_dir().join(&forge_jar_rel);
        if download_file(&client_url, &client_dest, None).await.is_err() {
            let uni_rel =
                format!("net/minecraftforge/forge/{forge_full}/forge-{forge_full}-universal.jar");
            let uni_url = format!("https://maven.minecraftforge.net/{uni_rel}");
            let uni_dest = libraries_dir().join(&uni_rel);
            let _ = download_file(&uni_url, &uni_dest, None).await;
        }
    }

    let _ = crate::instances::ensure_instance_with_version(&profile_id, &profile_id);

    Ok(profile_id)
}

async fn download_forge_library(lib: &serde_json::Value) -> Result<(), String> {
    download_forge_library_with_base(lib, "https://maven.minecraftforge.net/").await
}

async fn download_forge_library_with_base(
    lib: &serde_json::Value,
    default_base: &str,
) -> Result<(), String> {
    let name = lib
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "library missing name".to_string())?;

    if let Some(artifact) = lib.pointer("/downloads/artifact") {
        let path = artifact.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let url = artifact.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let sha1 = artifact.get("sha1").and_then(|v| v.as_str());
        if !url.is_empty() && !path.is_empty() {
            let dest = libraries_dir().join(path);
            return download_file(url, &dest, sha1).await;
        }
    }

    let base = lib.get("url").and_then(|v| v.as_str()).unwrap_or(default_base);
    let rel = maven_path(name);
    let dest = libraries_dir().join(&rel);
    let url = format!("{base}{rel}");
    download_file(&url, &dest, None).await
}

// --- NeoForge ---

const NEOFORGE_META: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/";

/// Infer Minecraft version from a NeoForge version string (e.g. 21.1.77 → 1.21.1).
pub fn neoforge_mc_version(neo: &str) -> Option<String> {
    let cleaned = neo.split('-').next().unwrap_or(neo);
    let parts: Vec<&str> = cleaned.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let major = parts[0];
    let minor = parts[1];
    if minor == "0" {
        Some(format!("1.{major}"))
    } else {
        Some(format!("1.{major}.{minor}"))
    }
}

pub async fn list_neoforge_versions(
    mc_filter: Option<String>,
) -> Result<Vec<ForgeVersionEntry>, String> {
    let client = reqwest::Client::new();
    let xml = client
        .get(NEOFORGE_META)
        .header("User-Agent", "Cubera/0.1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for cap in regex::Regex::new(r"<version>([^<]+)</version>")
        .map_err(|e| e.to_string())?
        .captures_iter(&xml)
    {
        let raw = cap[1].to_string();
        let Some(mc) = neoforge_mc_version(&raw) else {
            continue;
        };
        if let Some(ref filter) = mc_filter {
            if &mc != filter {
                continue;
            }
        }
        out.push(ForgeVersionEntry {
            forge: raw.clone(),
            raw,
            mc,
        });
    }
    out.sort_by(|a, b| b.raw.cmp(&a.raw));
    Ok(out)
}

pub async fn install_neoforge(
    app: AppHandle,
    mc_version: &str,
    mc_version_url: &str,
    neo_version: &str,
) -> Result<String, String> {
    ensure_dirs()?;
    if !versions_dir()
        .join(mc_version)
        .join(format!("{mc_version}.json"))
        .exists()
    {
        install_vanilla(app.clone(), mc_version, mc_version_url).await?;
    }

    let installer_url = format!(
        "{NEOFORGE_MAVEN}net/neoforged/neoforge/{neo_version}/neoforge-{neo_version}-installer.jar"
    );
    let installer_path = versions_dir().join(format!("neoforge-{neo_version}-installer.jar"));
    download_file(&installer_url, &installer_path, None).await?;

    let file = fs::File::open(&installer_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    let mut version_json: Option<serde_json::Value> = None;
    let mut install_profile: Option<serde_json::Value> = None;

    for name in ["version.json", "install_profile.json"] {
        if let Ok(mut entry) = archive.by_name(name) {
            let mut buf = String::new();
            entry.read_to_string(&mut buf).map_err(|e| e.to_string())?;
            let json: serde_json::Value =
                serde_json::from_str(&buf).map_err(|e| e.to_string())?;
            if name == "version.json" {
                version_json = Some(json);
            } else {
                install_profile = Some(json);
            }
        }
    }

    if version_json.is_none() {
        if let Some(profile) = &install_profile {
            if let Some(v) = profile.get("versionInfo") {
                version_json = Some(v.clone());
            }
        }
    }

    // Prefer minecraft version from install profile when present
    let profile_mc = install_profile
        .as_ref()
        .and_then(|p| p.get("minecraft"))
        .and_then(|v| v.as_str())
        .unwrap_or(mc_version)
        .to_string();

    let mut version_json = version_json.ok_or_else(|| {
        "Could not find version.json in NeoForge installer".to_string()
    })?;

    let profile_id = version_json
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(&format!("neoforge-{neo_version}"))
        .to_string();

    if version_json.get("inheritsFrom").is_none() {
        if let Some(obj) = version_json.as_object_mut() {
            obj.insert("inheritsFrom".into(), serde_json::json!(profile_mc));
        }
    }

    let dir = versions_dir().join(&profile_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(
        dir.join(format!("{profile_id}.json")),
        serde_json::to_string_pretty(&version_json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    if let Some(libs) = version_json.get("libraries").and_then(|v| v.as_array()) {
        for lib in libs {
            download_forge_library_with_base(lib, NEOFORGE_MAVEN).await?;
        }
    }

    if let Some(profile) = &install_profile {
        // Prefer NeoForge maven for profile libs without explicit URLs
        if let Some(libs) = profile.get("libraries").and_then(|v| v.as_array()) {
            for lib in libs {
                download_forge_library_with_base(lib, NEOFORGE_MAVEN).await?;
            }
        }
        let extracted = crate::forge::extract_installer_maven(&installer_path)?;
        let _ = app.emit(
            "install-progress",
            crate::download::ProgressEvent {
                stage: "neoforge".into(),
                current: 0,
                total: 1,
                message: format!("Extracted {extracted} NeoForge maven artifacts"),
            },
        );
        crate::forge::ensure_client_lzma(&installer_path).await?;
        crate::forge::run_forge_processors(&app, profile, &installer_path, &profile_mc).await?;
    }

    let _ = crate::instances::ensure_instance_with_version(&profile_id, &profile_id);
    Ok(profile_id)
}
