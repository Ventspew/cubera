use crate::download::{download_file, install_vanilla};
use crate::paths::{ensure_dirs, instances_dir, versions_dir};
use serde::Deserialize;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use tauri::{AppHandle, Emitter};
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrPackIndex {
    format_version: u32,
    game: String,
    version_id: Option<String>,
    name: String,
    summary: Option<String>,
    files: Vec<MrPackFile>,
    dependencies: MrPackDeps,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrPackFile {
    path: String,
    downloads: Vec<String>,
    env: Option<MrPackEnv>,
    file_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct MrPackEnv {
    client: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct MrPackDeps {
    minecraft: Option<String>,
    #[serde(rename = "fabric-loader")]
    fabric_loader: Option<String>,
    #[serde(rename = "quilt-loader")]
    quilt_loader: Option<String>,
    forge: Option<String>,
    neoforge: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MrPackInstallResult {
    pub instance_id: String,
    pub name: String,
    pub game_version: String,
    pub loader: String,
}

/// Install a Modrinth `.mrpack` from a direct URL (version file download).
pub async fn install_mrpack_url(
    app: AppHandle,
    file_url: &str,
    suggested_name: Option<String>,
) -> Result<MrPackInstallResult, String> {
    ensure_dirs()?;
    let tmp = instances_dir().join(".tmp-mrpack.zip");
    if let Some(parent) = tmp.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let _ = app.emit(
        "install-progress",
        crate::download::ProgressEvent {
            stage: "mrpack".into(),
            current: 0,
            total: 1,
            message: "Downloading modpack…".into(),
        },
    );
    download_file(file_url, &tmp, None).await?;
    let result = install_mrpack_file(app, &tmp, suggested_name).await;
    let _ = fs::remove_file(&tmp);
    result
}

pub async fn install_mrpack_file(
    app: AppHandle,
    pack_path: &Path,
    suggested_name: Option<String>,
) -> Result<MrPackInstallResult, String> {
    let file = fs::File::open(pack_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

    let index: MrPackIndex = {
        let mut entry = archive
            .by_name("modrinth.index.json")
            .map_err(|_| "Not a valid .mrpack (missing modrinth.index.json)".to_string())?;
        let mut buf = String::new();
        entry.read_to_string(&mut buf).map_err(|e| e.to_string())?;
        serde_json::from_str(&buf).map_err(|e| format!("Invalid modrinth.index.json: {e}"))?
    };

    if index.game != "minecraft" {
        return Err(format!("Unsupported mrpack game: {}", index.game));
    }
    if index.format_version > 1 {
        // Still try — format is largely compatible
    }

    let mc = index
        .dependencies
        .minecraft
        .clone()
        .ok_or_else(|| "Modpack missing minecraft dependency".to_string())?;

    let name = suggested_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| index.name.clone());

    // Resolve vanilla version URL from manifest
    let manifest = crate::manifest::fetch_version_manifest().await?;
    let mc_info = manifest
        .versions
        .iter()
        .find(|v| v.id == mc)
        .ok_or_else(|| format!("Minecraft {mc} not found in Mojang manifest"))?;

    let _ = app.emit(
        "install-progress",
        crate::download::ProgressEvent {
            stage: "mrpack".into(),
            current: 0,
            total: 1,
            message: format!("Installing loader for {mc}…"),
        },
    );

    let (version_id, loader_name) = install_loader_for_pack(
        app.clone(),
        &mc,
        &mc_info.url,
        &index.dependencies,
    )
    .await?;

    // Reuse the loader's default instance folder when present
    let instance_id = if instances_dir().join(&version_id).exists() {
        version_id.clone()
    } else {
        unique_instance_id(&slugify(&name))
    };
    let instance_dir = instances_dir().join(&instance_id);
    fs::create_dir_all(instance_dir.join("mods")).map_err(|e| e.to_string())?;

    // Download pack files
    let files: Vec<&MrPackFile> = index
        .files
        .iter()
        .filter(|f| {
            match f.env.as_ref().and_then(|e| e.client.as_deref()) {
                Some("unsupported") => false,
                _ => true,
            }
        })
        .collect();
    let total = files.len() as u64;
    for (i, f) in files.iter().enumerate() {
        let _ = app.emit(
            "install-progress",
            crate::download::ProgressEvent {
                stage: "mrpack".into(),
                current: i as u64,
                total,
                message: format!("Pack file {}/{}: {}", i + 1, total, f.path),
            },
        );
        let dest = instance_dir.join(&f.path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let url = f
            .downloads
            .first()
            .ok_or_else(|| format!("No download URL for {}", f.path))?;
        download_file(url, &dest, None).await?;
        let _ = f.file_size;
    }

    // Extract overrides
    extract_overrides(&mut archive, &instance_dir, "overrides/")?;
    extract_overrides(&mut archive, &instance_dir, "client-overrides/")?;

    let mut meta = crate::instances::ensure_instance_with_version(&instance_id, &version_id)?;
    meta.name = name.clone();
    meta.notes = index.summary.unwrap_or_default();
    meta.game_version = Some(mc.clone());
    meta.loader = Some(loader_name.clone());
    crate::instances::save_meta(&meta)?;

    let _ = app.emit(
        "install-progress",
        crate::download::ProgressEvent {
            stage: "mrpack".into(),
            current: 1,
            total: 1,
            message: format!("Modpack ready: {name}"),
        },
    );

    Ok(MrPackInstallResult {
        instance_id,
        name,
        game_version: mc,
        loader: loader_name,
    })
}

async fn install_loader_for_pack(
    app: AppHandle,
    mc: &str,
    mc_url: &str,
    deps: &MrPackDeps,
) -> Result<(String, String), String> {
    if let Some(loader) = &deps.fabric_loader {
        let id =
            crate::loaders::install_fabric(app, mc, mc_url, loader).await?;
        return Ok((id, "fabric".into()));
    }
    if let Some(loader) = &deps.quilt_loader {
        let id = crate::loaders::install_quilt(app, mc, mc_url, loader).await?;
        return Ok((id, "quilt".into()));
    }
    if let Some(neo) = &deps.neoforge {
        let id = crate::loaders::install_neoforge(app, mc, mc_url, neo).await?;
        return Ok((id, "neoforge".into()));
    }
    if let Some(forge) = &deps.forge {
        // Forge deps are often just the forge version number; pack may use "47.2.0" or "1.20.1-47.2.0"
        let full = if forge.contains(mc) {
            forge.clone()
        } else {
            format!("{mc}-{forge}")
        };
        let id = crate::loaders::install_forge(app, mc, mc_url, &full).await?;
        return Ok((id, "forge".into()));
    }

    // Vanilla pack
    if !versions_dir()
        .join(mc)
        .join(format!("{mc}.json"))
        .exists()
    {
        install_vanilla(app, mc, mc_url).await?;
    }
    Ok((mc.to_string(), "vanilla".into()))
}

fn extract_overrides(
    archive: &mut ZipArchive<fs::File>,
    instance_dir: &Path,
    prefix: &str,
) -> Result<(), String> {
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if !name.starts_with(prefix) || name.ends_with('/') {
            continue;
        }
        let rel = name.trim_start_matches(prefix);
        if rel.is_empty() {
            continue;
        }
        let dest = instance_dir.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        let _ = out.flush();
    }
    Ok(())
}

fn slugify(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "modpack".into()
    } else {
        s.chars().take(48).collect()
    }
}

fn unique_instance_id(base: &str) -> String {
    let mut id = base.to_string();
    let mut n = 2u32;
    while instances_dir().join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}
