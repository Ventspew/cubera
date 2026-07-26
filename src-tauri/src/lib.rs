mod auth;
mod branding;
mod download;
mod forge;
mod instances;
mod java_runtime;
mod launch;
mod loaders;
mod manifest;
mod modrinth;
mod mrpack;
mod news;
mod paths;
mod skin;

use paths::{ensure_dirs, load_settings, save_settings, Settings};
use serde::Serialize;
use std::fs;
use std::process::Command;

#[derive(Serialize)]
struct JavaInfo {
    path: Option<String>,
    found: bool,
    managed: Vec<java_runtime::ManagedJavaInfo>,
}

#[tauri::command]
async fn get_version_manifest() -> Result<manifest::VersionManifest, String> {
    ensure_dirs()?;
    manifest::fetch_version_manifest().await
}

#[tauri::command]
fn get_installed_versions() -> Vec<String> {
    download::list_installed_versions()
}

#[tauri::command]
async fn install_vanilla(app: tauri::AppHandle, version_id: String, version_url: String) -> Result<String, String> {
    download::install_vanilla(app, &version_id, &version_url).await
}

#[tauri::command]
async fn install_fabric(
    app: tauri::AppHandle,
    game_version: String,
    game_version_url: String,
    loader_version: String,
) -> Result<String, String> {
    loaders::install_fabric(app, &game_version, &game_version_url, &loader_version).await
}

#[tauri::command]
async fn list_fabric_loaders(game_version: String) -> Result<Vec<loaders::FabricLoaderVersion>, String> {
    loaders::list_fabric_loaders(&game_version).await
}

#[tauri::command]
async fn list_forge_versions(mc_version: Option<String>) -> Result<Vec<loaders::ForgeVersionEntry>, String> {
    loaders::list_forge_versions(mc_version).await
}

#[tauri::command]
async fn install_forge(
    app: tauri::AppHandle,
    mc_version: String,
    mc_version_url: String,
    forge_full: String,
) -> Result<String, String> {
    loaders::install_forge(app, &mc_version, &mc_version_url, &forge_full).await
}

#[tauri::command]
async fn launch_instance(version_id: String) -> Result<String, String> {
    // version_id arg is the instance id (folder name)
    launch::launch_game(&version_id).await
}

#[tauri::command]
fn list_instances() -> Result<Vec<instances::InstanceInfo>, String> {
    instances::list_instances()
}

#[tauri::command]
fn update_instance(meta: instances::InstanceMeta) -> Result<instances::InstanceMeta, String> {
    instances::update_instance(meta)
}

#[tauri::command]
fn duplicate_instance(instance_id: String, new_name: String) -> Result<String, String> {
    instances::duplicate_instance(&instance_id, &new_name)
}

#[tauri::command]
fn kill_instance(instance_id: String) -> Result<(), String> {
    instances::kill_instance(&instance_id)
}

#[tauri::command]
fn is_instance_running(instance_id: String) -> bool {
    instances::is_running(&instance_id)
}

#[tauri::command]
fn open_instance_subfolder(instance_id: String, folder: String) -> Result<(), String> {
    instances::open_instance_subfolder(&instance_id, &folder)
}

#[tauri::command]
async fn fetch_news() -> Result<Vec<news::NewsItem>, String> {
    news::fetch_minecraft_news().await
}

#[tauri::command]
async fn list_quilt_loaders(game_version: String) -> Result<Vec<loaders::FabricLoaderVersion>, String> {
    loaders::list_quilt_loaders(&game_version).await
}

#[tauri::command]
async fn install_quilt(
    app: tauri::AppHandle,
    game_version: String,
    game_version_url: String,
    loader_version: String,
) -> Result<String, String> {
    loaders::install_quilt(app, &game_version, &game_version_url, &loader_version).await
}

#[tauri::command]
async fn list_neoforge_versions(
    mc_version: Option<String>,
) -> Result<Vec<loaders::ForgeVersionEntry>, String> {
    loaders::list_neoforge_versions(mc_version).await
}

#[tauri::command]
async fn install_neoforge(
    app: tauri::AppHandle,
    mc_version: String,
    mc_version_url: String,
    neo_version: String,
) -> Result<String, String> {
    loaders::install_neoforge(app, &mc_version, &mc_version_url, &neo_version).await
}

#[tauri::command]
async fn search_modpacks(
    query: String,
    game_version: Option<String>,
) -> Result<modrinth::ModrinthSearch, String> {
    modrinth::search_modpacks(&query, game_version).await
}

#[tauri::command]
async fn install_mrpack(
    app: tauri::AppHandle,
    file_url: String,
    name: Option<String>,
) -> Result<mrpack::MrPackInstallResult, String> {
    mrpack::install_mrpack_url(app, &file_url, name).await
}

#[tauri::command]
fn list_managed_java() -> Vec<java_runtime::ManagedJavaInfo> {
    java_runtime::list_managed_java()
}

#[tauri::command]
async fn install_managed_java(
    app: tauri::AppHandle,
    component: String,
) -> Result<java_runtime::ManagedJavaInfo, String> {
    java_runtime::install_java_component(app, &component).await
}

#[tauri::command]
async fn start_microsoft_login() -> Result<auth::DeviceCodeResponse, String> {
    auth::start_device_login().await
}

#[tauri::command]
async fn poll_microsoft_login(device_code: String, interval: u64) -> Result<paths::Account, String> {
    auth::poll_device_login(device_code, interval).await
}

#[tauri::command]
fn add_offline_account(name: String) -> Result<paths::Account, String> {
    auth::add_offline_account(name)
}

#[tauri::command]
fn set_active_account(uuid: String) -> Result<(), String> {
    let mut settings = load_settings();
    if !settings.accounts.iter().any(|a| a.uuid == uuid) {
        return Err("Account not found".into());
    }
    settings.active_account = Some(uuid);
    save_settings(&settings)
}

#[tauri::command]
fn remove_account(uuid: String) -> Result<(), String> {
    let mut settings = load_settings();
    settings.accounts.retain(|a| a.uuid != uuid);
    if settings.active_account.as_deref() == Some(&uuid) {
        settings.active_account = settings.accounts.last().map(|a| a.uuid.clone());
    }
    save_settings(&settings)
}

#[tauri::command]
fn get_settings() -> Settings {
    load_settings()
}

#[tauri::command]
fn update_settings(settings: Settings) -> Result<(), String> {
    save_settings(&settings)
}

#[tauri::command]
fn get_java_info() -> JavaInfo {
    let managed = java_runtime::list_managed_java();
    match launch::find_java(load_settings().java_path.as_deref()) {
        Ok(path) => JavaInfo {
            path: Some(path.display().to_string()),
            found: true,
            managed,
        },
        Err(_) => {
            // Prefer managed java if find_java failed
            if let Some(m) = managed.iter().find(|m| m.installed) {
                JavaInfo {
                    path: Some(m.path.clone()),
                    found: true,
                    managed,
                }
            } else {
                JavaInfo {
                    path: None,
                    found: false,
                    managed,
                }
            }
        }
    }
}

#[tauri::command]
fn get_data_dir() -> String {
    paths::data_dir().display().to_string()
}

#[tauri::command]
fn open_instance_folder(instance_id: String) -> Result<(), String> {
    let dir = paths::instances_dir().join(&instance_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Command::new("open")
        .arg(&dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_mod(instance_id: String, filename: String) -> Result<(), String> {
    let path = paths::instances_dir()
        .join(&instance_id)
        .join("mods")
        .join(&filename);
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn delete_instance(instance_id: String) -> Result<(), String> {
    let meta = instances::load_meta(&instance_id).ok();
    let version_id = meta
        .as_ref()
        .map(|m| m.version_id.clone())
        .unwrap_or_else(|| instance_id.clone());

    let instance = paths::instances_dir().join(&instance_id);
    if instance.exists() {
        fs::remove_dir_all(&instance).map_err(|e| e.to_string())?;
    }

    // Only remove the version profile if no other instance still uses it
    let still_used = instances::list_instances()
        .unwrap_or_default()
        .iter()
        .any(|i| i.version_id == version_id);
    if !still_used {
        let version = paths::versions_dir().join(&version_id);
        if version.exists() {
            fs::remove_dir_all(&version).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn search_mods(
    query: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<modrinth::ModrinthSearch, String> {
    modrinth::search_mods(&query, loader, game_version).await
}

#[tauri::command]
async fn search_resourcepacks(
    query: String,
    game_version: Option<String>,
) -> Result<modrinth::ModrinthSearch, String> {
    modrinth::search_resourcepacks(&query, game_version).await
}

#[tauri::command]
async fn search_shaders(
    query: String,
    game_version: Option<String>,
) -> Result<modrinth::ModrinthSearch, String> {
    modrinth::search_shaders(&query, game_version).await
}

#[tauri::command]
async fn get_mod_versions(
    project_id: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> Result<Vec<modrinth::ModrinthVersion>, String> {
    modrinth::get_project_versions(&project_id, game_version, loader).await
}

#[tauri::command]
async fn install_mod(instance_id: String, file_url: String, filename: String) -> Result<String, String> {
    modrinth::install_mod(&instance_id, &file_url, &filename).await
}

#[tauri::command]
async fn install_content(
    instance_id: String,
    folder: String,
    file_url: String,
    filename: String,
) -> Result<String, String> {
    modrinth::install_content(&instance_id, &folder, &file_url, &filename).await
}

#[tauri::command]
fn list_mods(instance_id: String) -> Result<Vec<String>, String> {
    modrinth::list_instance_mods(&instance_id)
}

#[tauri::command]
async fn get_player_skin(uuid: String) -> Result<String, String> {
    skin::get_player_avatar_data_url(&uuid).await
}

#[tauri::command]
fn get_app_info() -> branding::AppInfo {
    branding::app_info()
}

#[tauri::command]
fn get_launch_log(instance_id: String) -> Result<branding::LaunchLog, String> {
    branding::read_launch_log(&instance_id)
}

#[tauri::command]
fn open_data_folder() -> Result<(), String> {
    branding::open_data_folder()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = ensure_dirs();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_version_manifest,
            get_installed_versions,
            install_vanilla,
            install_fabric,
            list_fabric_loaders,
            list_forge_versions,
            install_forge,
            launch_instance,
            start_microsoft_login,
            poll_microsoft_login,
            add_offline_account,
            set_active_account,
            remove_account,
            get_settings,
            update_settings,
            get_java_info,
            get_data_dir,
            open_instance_folder,
            delete_mod,
            delete_instance,
            search_mods,
            search_resourcepacks,
            search_shaders,
            get_mod_versions,
            install_mod,
            install_content,
            list_mods,
            get_player_skin,
            get_app_info,
            get_launch_log,
            open_data_folder,
            list_instances,
            update_instance,
            duplicate_instance,
            kill_instance,
            is_instance_running,
            open_instance_subfolder,
            fetch_news,
            list_quilt_loaders,
            install_quilt,
            list_neoforge_versions,
            install_neoforge,
            search_modpacks,
            install_mrpack,
            list_managed_java,
            install_managed_java,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cubera");
}
