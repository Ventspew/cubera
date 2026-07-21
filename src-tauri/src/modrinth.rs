use serde::{Deserialize, Serialize};

const MODRINTH_API: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub categories: Vec<String>,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub project_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthSearch {
    pub hits: Vec<ModrinthHit>,
    pub total_hits: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<ModrinthFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
}

pub async fn search_mods(query: &str, loader: Option<String>, game_version: Option<String>) -> Result<ModrinthSearch, String> {
    let mut facets: Vec<Vec<String>> = vec![vec!["project_type:mod".into()]];
    if let Some(l) = loader {
        if !l.is_empty() {
            facets.push(vec![format!("categories:{l}")]);
        }
    }
    if let Some(v) = game_version {
        if !v.is_empty() {
            facets.push(vec![format!("versions:{v}")]);
        }
    }

    let facets_json = serde_json::to_string(&facets).map_err(|e| e.to_string())?;
    let url = format!(
        "{MODRINTH_API}/search?query={}&limit=20&facets={}",
        urlencoding::encode(query),
        urlencoding::encode(&facets_json)
    );

    let client = reqwest::Client::new();
    client
        .get(&url)
        .header("User-Agent", "Cubera/0.1.0 (Minecraft Launcher)")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_project_versions(
    project_id: &str,
    game_version: Option<String>,
    loader: Option<String>,
) -> Result<Vec<ModrinthVersion>, String> {
    let mut url = format!("{MODRINTH_API}/project/{project_id}/version?");
    if let Some(v) = game_version {
        url.push_str(&format!("game_versions={}", urlencoding::encode(&format!("[\"{v}\"]"))));
    }
    if let Some(l) = loader {
        if !url.ends_with('?') {
            url.push('&');
        }
        url.push_str(&format!("loaders={}", urlencoding::encode(&format!("[\"{l}\"]"))));
    }

    let client = reqwest::Client::new();
    client
        .get(&url)
        .header("User-Agent", "Cubera/0.1.0 (Minecraft Launcher)")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn install_mod(instance_id: &str, file_url: &str, filename: &str) -> Result<String, String> {
    let mods_dir = crate::paths::instances_dir().join(instance_id).join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;
    let dest = mods_dir.join(filename);
    crate::download::download_file(file_url, &dest, None).await?;
    Ok(dest.display().to_string())
}

pub fn list_instance_mods(instance_id: &str) -> Result<Vec<String>, String> {
    let mods_dir = crate::paths::instances_dir().join(instance_id).join("mods");
    let Ok(entries) = std::fs::read_dir(mods_dir) else {
        return Ok(vec![]);
    };
    let mut mods = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".jar") {
            mods.push(name);
        }
    }
    mods.sort();
    Ok(mods)
}
