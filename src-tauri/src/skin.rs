use crate::paths::data_dir;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct SessionProfile {
    properties: Option<Vec<SessionProperty>>,
}

#[derive(Debug, Deserialize)]
struct SessionProperty {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct TexturePayload {
    textures: TextureMap,
}

#[derive(Debug, Deserialize)]
struct TextureMap {
    #[serde(rename = "SKIN")]
    skin: Option<TextureUrl>,
}

#[derive(Debug, Deserialize)]
struct TextureUrl {
    url: String,
}

fn skins_dir() -> PathBuf {
    data_dir().join("skins")
}

fn undash(uuid: &str) -> String {
    uuid.chars().filter(|c| *c != '-').collect()
}

/// Returns a `data:image/png;base64,...` avatar for the player head.
pub async fn get_player_avatar_data_url(uuid: &str) -> Result<String, String> {
    let id = undash(uuid);
    if id.len() != 32 {
        return Err("Ongeldige UUID".into());
    }

    fs::create_dir_all(skins_dir()).map_err(|e| e.to_string())?;
    let cache = skins_dir().join(format!("{id}.png"));

    // Fresh cache for a day
    if cache.exists() {
        if let Ok(meta) = fs::metadata(&cache) {
            if let Ok(modified) = meta.modified() {
                if modified.elapsed().map(|d| d.as_secs() < 86_400).unwrap_or(false) {
                    let bytes = fs::read(&cache).map_err(|e| e.to_string())?;
                    return Ok(to_data_url(&bytes));
                }
            }
        }
    }

    let bytes = download_avatar_bytes(&id).await?;
    let _ = fs::write(&cache, &bytes);
    Ok(to_data_url(&bytes))
}

async fn download_avatar_bytes(undashed: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::builder()
        .user_agent("Cubera/0.1.0 (Minecraft Launcher)")
        .build()
        .map_err(|e| e.to_string())?;

    // 1) Prefer rendered head services (fast, correct crop)
    for url in [
        format!("https://crafthead.net/avatar/{undashed}/128"),
        format!("https://mc-heads.net/avatar/{undashed}/128"),
        format!("https://crafatar.com/avatars/{undashed}?size=128&overlay=true"),
    ] {
        if let Ok(bytes) = fetch_png(&client, &url).await {
            return Ok(bytes);
        }
    }

    // 2) Fallback: Mojang session skin URL → full skin texture (still usable as img)
    if let Ok(skin_url) = fetch_mojang_skin_url(&client, undashed).await {
        if let Ok(bytes) = fetch_png(&client, &skin_url).await {
            return Ok(bytes);
        }
    }

    Err("Skin kon niet worden opgehaald".into())
}

async fn fetch_png(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() < 32 || &bytes[0..4] != b"\x89PNG" {
        return Err("Geen PNG ontvangen".into());
    }
    Ok(bytes.to_vec())
}

async fn fetch_mojang_skin_url(client: &reqwest::Client, undashed: &str) -> Result<String, String> {
    let url = format!("https://sessionserver.mojang.com/session/minecraft/profile/{undashed}");
    let profile: SessionProfile = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let textures = profile
        .properties
        .unwrap_or_default()
        .into_iter()
        .find(|p| p.name == "textures")
        .ok_or_else(|| "Geen textures in profiel".to_string())?;

    let decoded = B64
        .decode(textures.value.trim())
        .map_err(|e| format!("Textures decode mislukt: {e}"))?;
    let payload: TexturePayload =
        serde_json::from_slice(&decoded).map_err(|e| e.to_string())?;
    let skin_url = payload
        .textures
        .skin
        .map(|s| s.url)
        .ok_or_else(|| "Geen skin URL".to_string())?;

    // textures.minecraft.net is often http — upgrade
    Ok(skin_url.replacen("http://", "https://", 1))
}

fn to_data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", B64.encode(bytes))
}
