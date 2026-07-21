use crate::paths::{load_settings, save_settings, Account};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Default MSA client: Prism Launcher's public Azure app.
/// Microsoft's consent screen therefore shows "Prism Launcher".
/// Cubera does **not** ship Prism's codebase — only this public client ID,
/// because a custom Azure app must be registered (+ Minecraft API permission)
/// to display "Cubera" instead. Override via settings later if needed.
const MSA_CLIENT_ID: &str = "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb";
/// Scope used by Prism / MultiMC-family launchers.
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
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
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
struct XstsErrorBody {
    #[serde(rename = "XErr")]
    xerr: Option<u64>,
    #[serde(rename = "Message")]
    message: Option<String>,
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
        .form(&[("client_id", MSA_CLIENT_ID), ("scope", MSA_SCOPE)])
        .send()
        .await
        .map_err(|e| format!("Kan Microsoft niet bereiken: {e}"))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Microsoft device-login mislukt ({status}): {text}"));
    }

    serde_json::from_str(&text).map_err(|e| format!("Ongeldig device-code antwoord: {e} — {text}"))
}

pub async fn poll_device_login(device_code: String, interval: u64) -> Result<Account, String> {
    let client = reqwest::Client::new();
    let interval = interval.max(1);
    let deadline = std::time::Instant::now() + Duration::from_secs(15 * 60);

    loop {
        if std::time::Instant::now() > deadline {
            return Err("Login verlopen. Probeer opnieuw.".into());
        }

        tokio::time::sleep(Duration::from_secs(interval)).await;

        let resp = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", MSA_CLIENT_ID),
                ("device_code", device_code.as_str()),
            ])
            .send()
            .await
            .map_err(|e| format!("Token-request mislukt: {e}"))?;

        let text = resp.text().await.map_err(|e| e.to_string())?;
        let token: TokenResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Ongeldig token-antwoord: {e} — {text}"))?;

        if let Some(err) = token.error.as_deref() {
            match err {
                "authorization_pending" => continue,
                "slow_down" => {
                    tokio::time::sleep(Duration::from_secs(interval)).await;
                    continue;
                }
                "expired_token" => return Err("Login verlopen. Probeer opnieuw.".into()),
                "access_denied" => return Err("Login geannuleerd.".into()),
                other => {
                    let detail = token.error_description.unwrap_or_default();
                    return Err(format!("Microsoft-fout ({other}): {detail}"));
                }
            }
        }

        let access = token
            .access_token
            .ok_or_else(|| format!("Geen access token ontvangen: {text}"))?;
        return finish_xbox_minecraft_login(access, token.refresh_token).await;
    }
}

async fn finish_xbox_minecraft_login(
    msa_token: String,
    refresh_token: Option<String>,
) -> Result<Account, String> {
    let client = reqwest::Client::new();

    let xbl_resp = client
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
        .map_err(|e| format!("Xbox Live auth mislukt: {e}"))?;

    let xbl_status = xbl_resp.status();
    let xbl_text = xbl_resp.text().await.map_err(|e| e.to_string())?;
    if !xbl_status.is_success() {
        return Err(format!("Xbox Live weigerde login ({xbl_status}): {xbl_text}"));
    }
    let xbl: XblResponse = serde_json::from_str(&xbl_text)
        .map_err(|e| format!("Xbox Live antwoord ongeldig: {e}"))?;

    let user_hash = xbl
        .display_claims
        .xui
        .first()
        .map(|u| u.uhs.clone())
        .ok_or_else(|| "Geen Xbox user hash ontvangen".to_string())?;

    let xsts_resp = client
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
        .map_err(|e| format!("XSTS mislukt: {e}"))?;

    let xsts_status = xsts_resp.status();
    let xsts_text = xsts_resp.text().await.map_err(|e| e.to_string())?;
    if !xsts_status.is_success() {
        let hint = if let Ok(body) = serde_json::from_str::<XstsErrorBody>(&xsts_text) {
            match body.xerr {
                Some(2148916233) => " Dit Microsoft-account heeft geen Xbox-profiel. Maak er een aan op xbox.com.".to_string(),
                Some(2148916238) => " Dit is een kinderaccount — voeg het toe aan een Microsoft Family.".to_string(),
                Some(_) => body.message.unwrap_or_default(),
                None => body.message.unwrap_or_default(),
            }
        } else {
            String::new()
        };
        return Err(format!(
            "XSTS mislukt ({xsts_status}). Heb je Minecraft Java?{hint} {xsts_text}"
        ));
    }
    let xsts: XblResponse = serde_json::from_str(&xsts_text)
        .map_err(|e| format!("XSTS antwoord ongeldig: {e}"))?;

    let mc_resp = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&serde_json::json!({
            "identityToken": format!("XBL3.0 x={user_hash};{}", xsts.token)
        }))
        .send()
        .await
        .map_err(|e| format!("Minecraft login mislukt: {e}"))?;

    let mc_status = mc_resp.status();
    let mc_text = mc_resp.text().await.map_err(|e| e.to_string())?;
    if !mc_status.is_success() {
        return Err(format!(
            "Minecraft-services weigerden login ({mc_status}). Check of Minecraft Java gekocht is. {mc_text}"
        ));
    }
    let mc: McLoginResponse = serde_json::from_str(&mc_text)
        .map_err(|e| format!("Minecraft login-antwoord ongeldig: {e}"))?;

    let profile_resp = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc.access_token)
        .send()
        .await
        .map_err(|e| format!("Profiel ophalen mislukt: {e}"))?;

    let profile_status = profile_resp.status();
    let profile_text = profile_resp.text().await.map_err(|e| e.to_string())?;
    if !profile_status.is_success() {
        return Err(format!(
            "Geen Minecraft-profiel ({profile_status}). Heb je Java Edition? {profile_text}"
        ));
    }
    let profile: McProfile = serde_json::from_str(&profile_text)
        .map_err(|e| format!("Profiel ongeldig: {e}"))?;

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
        return Err("Gebruikersnaam moet 1–16 tekens zijn".into());
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
    let data = format!("OfflinePlayer:{name}");
    let digest = md5_bytes(data.as_bytes());
    let mut bytes = digest;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

fn md5_bytes(data: &[u8]) -> [u8; 16] {
    use sha1::Digest;
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
