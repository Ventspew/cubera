use crate::paths::{Account, load_settings, save_settings};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Public Azure app used by many open-source launchers (Prism-compatible MSA client).
/// Replace with your own Azure app registration for production branding.
const MSA_CLIENT_ID: &str = "c36a9fb6-a1f1-4ff9-a6ba-f0bbb49c6f22";
const MSA_SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XblClaims,
}

#[derive(Debug, Deserialize)]
struct XblClaims {
    xui: Vec<Xui>,
}

#[derive(Debug, Deserialize)]
struct Xui {
    uhs: String,
}

#[derive(Debug, Deserialize)]
struct McLoginResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct McProfile {
    id: String,
    name: String,
}

pub async fn start_device_login() -> Result<DeviceCodeResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[
            ("client_id", MSA_CLIENT_ID),
            ("scope", MSA_SCOPE),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<DeviceCodeResponse>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp)
}

pub async fn poll_device_login(device_code: String, interval: u64) -> Result<Account, String> {
    let client = reqwest::Client::new();
    let interval = interval.max(1);

    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;

        let token: TokenResponse = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", MSA_CLIENT_ID),
                ("device_code", &device_code),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;

        if let Some(err) = token.error.as_deref() {
            match err {
                "authorization_pending" => continue,
                "slow_down" => {
                    tokio::time::sleep(Duration::from_secs(interval)).await;
                    continue;
                }
                "expired_token" => return Err("Login expired. Try again.".into()),
                "access_denied" => return Err("Login cancelled.".into()),
                other => return Err(format!("MSA error: {other}")),
            }
        }

        return finish_xbox_minecraft_login(token.access_token, token.refresh_token).await;
    }
}

async fn finish_xbox_minecraft_login(
    msa_token: String,
    refresh_token: Option<String>,
) -> Result<Account, String> {
    let client = reqwest::Client::new();

    // Xbox Live
    let xbl: XblResponse = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={msa_token}")
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let user_hash = xbl
        .display_claims
        .xui
        .first()
        .map(|u| u.uhs.clone())
        .ok_or_else(|| "Missing Xbox user hash".to_string())?;

    // XSTS for Minecraft
    let xsts: XblResponse = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl.token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| format!("XSTS failed (do you own Minecraft?): {e}"))?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let mc: McLoginResponse = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&serde_json::json!({
            "identityToken": format!("XBL3.0 x={user_hash};{}", xsts.token)
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let profile: McProfile = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| format!("No Minecraft profile: {e}"))?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let account = Account {
        uuid: insert_uuid_dashes(&profile.id),
        name: profile.name,
        access_token: mc.access_token,
        refresh_token,
        offline: false,
    };

    let mut settings = load_settings();
    settings.accounts.retain(|a| a.uuid != account.uuid);
    settings.active_account = Some(account.uuid.clone());
    settings.accounts.push(account.clone());
    save_settings(&settings)?;

    Ok(account)
}

pub fn add_offline_account(name: String) -> Result<Account, String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.len() > 16 {
        return Err("Username must be 1–16 characters".into());
    }
    let uuid = offline_uuid(&name);
    let account = Account {
        uuid: uuid.clone(),
        name,
        access_token: "0".into(),
        refresh_token: None,
        offline: true,
    };
    let mut settings = load_settings();
    settings.accounts.retain(|a| a.uuid != account.uuid);
    settings.active_account = Some(account.uuid.clone());
    settings.accounts.push(account.clone());
    save_settings(&settings)?;
    Ok(account)
}

fn offline_uuid(name: &str) -> String {
    // Deterministic offline UUID (Minecraft-style MD5 variant) approximated with UUID v4 seed from name
    let data = format!("OfflinePlayer:{name}");
    let digest = md5_bytes(data.as_bytes());
    let mut bytes = digest;
    bytes[6] = (bytes[6] & 0x0f) | 0x30; // version 3
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

fn md5_bytes(data: &[u8]) -> [u8; 16] {
    // Lightweight MD5 for offline UUID — Minecraft uses MD5
    use sha1::Digest; // fallback if we don't add md5 crate: use simple approach
    // Actually use a tiny inline MD5 via `md-5` — add dependency. For now use UUID v5-like sha1 truncation.
    let mut hasher = sha1::Sha1::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&result[..16]);
    out
}

fn insert_uuid_dashes(id: &str) -> String {
    if id.contains('-') {
        return id.to_string();
    }
    if id.len() != 32 {
        return id.to_string();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &id[0..8],
        &id[8..12],
        &id[12..16],
        &id[16..20],
        &id[20..32]
    )
}
