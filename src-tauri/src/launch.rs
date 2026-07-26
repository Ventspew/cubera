use crate::download::{natives_dir_for, resolve_version_chain};
use crate::manifest::{rule_allows, rule_allows_with, Argument, ArgumentValue, Library, VersionJson};
use crate::paths::{assets_dir, libraries_dir, load_settings, versions_dir, Account};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::AppHandle;

fn launch_features(custom_resolution: bool) -> HashMap<String, bool> {
    let mut features = HashMap::new();
    features.insert("is_demo_user".into(), false);
    features.insert("has_custom_resolution".into(), custom_resolution);
    features.insert("has_quick_plays_support".into(), false);
    features.insert("is_quick_play_singleplayer".into(), false);
    features.insert("is_quick_play_multiplayer".into(), false);
    features.insert("is_quick_play_realms".into(), false);
    features
}

async fn resolve_java_or_install(
    app: &AppHandle,
    preferred: Option<&str>,
    required: Option<&crate::manifest::JavaVersion>,
) -> Result<PathBuf, String> {
    if let Some(p) = preferred {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        }
    }
    if let Some(req) = required {
        if let Some(bin) = crate::java_runtime::managed_java_bin(&req.component) {
            return Ok(bin);
        }
        let comp = crate::java_runtime::component_for_major(req.major_version);
        if let Some(bin) = crate::java_runtime::managed_java_bin(comp) {
            return Ok(bin);
        }
        // Download the Mojang JRE required by this version
        match crate::java_runtime::install_java_component(app.clone(), &req.component).await {
            Ok(info) => return Ok(PathBuf::from(info.path)),
            Err(primary) => {
                if comp != req.component.as_str() {
                    if let Ok(info) =
                        crate::java_runtime::install_java_component(app.clone(), comp).await
                    {
                        return Ok(PathBuf::from(info.path));
                    }
                }
                // Fall through to system Java, but keep the install error for later
                if let Ok(sys) = find_java(None) {
                    return Ok(sys);
                }
                return Err(format!(
                    "Java {} required but not installed ({primary}). Use Settings → Install managed Java.",
                    req.major_version
                ));
            }
        }
    }
    for comp in ["java-runtime-delta", "java-runtime-gamma", "java-runtime-epsilon"] {
        if let Some(bin) = crate::java_runtime::managed_java_bin(comp) {
            return Ok(bin);
        }
    }
    match find_java(None) {
        Ok(path) => Ok(path),
        Err(_) => {
            let info = crate::java_runtime::install_java_component(app.clone(), "java-runtime-delta")
                .await
                .map_err(|e| {
                    format!("Java not found and managed download failed ({e}). Install Temurin 21 or use Settings → Install managed Java.")
                })?;
            Ok(PathBuf::from(info.path))
        }
    }
}

pub fn find_java(preferred: Option<&str>) -> Result<PathBuf, String> {
    if let Some(p) = preferred {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        }
    }

    let candidates = [
        "/usr/libexec/java_home",
        "/opt/homebrew/opt/openjdk/bin/java",
        "/opt/homebrew/opt/openjdk@21/bin/java",
        "/opt/homebrew/opt/openjdk@17/bin/java",
        "/usr/bin/java",
    ];

    // macOS java_home
    if let Ok(output) = Command::new("/usr/libexec/java_home").arg("-v").arg("21").output() {
        if output.status.success() {
            let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let bin = PathBuf::from(&home).join("bin/java");
            if bin.exists() {
                return Ok(bin);
            }
        }
    }
    if let Ok(output) = Command::new("/usr/libexec/java_home").arg("-v").arg("17").output() {
        if output.status.success() {
            let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let bin = PathBuf::from(&home).join("bin/java");
            if bin.exists() {
                return Ok(bin);
            }
        }
    }
    if let Ok(output) = Command::new("/usr/libexec/java_home").output() {
        if output.status.success() {
            let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let bin = PathBuf::from(&home).join("bin/java");
            if bin.exists() {
                return Ok(bin);
            }
        }
    }

    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() && c.ends_with("java") {
            return Ok(p);
        }
    }

    if let Ok(path) = which("java") {
        return Ok(path);
    }

    Err("Java not found. Install Temurin 21 via Homebrew: brew install --cask temurin".into())
}

fn which(bin: &str) -> Result<PathBuf, String> {
    let output = Command::new("which")
        .arg(bin)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("not found".into());
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

pub async fn launch_game(app: AppHandle, instance_id: &str) -> Result<String, String> {
    let settings = load_settings();
    let account = active_account(&settings.accounts, settings.active_account.as_deref())
        .ok_or_else(|| "No account selected. Log in first.".to_string())?;
    let account = crate::auth::ensure_fresh_account(account).await?;

    let meta = crate::instances::ensure_instance(instance_id)?;
    let version_id = if meta.version_id.is_empty() {
        instance_id.to_string()
    } else {
        meta.version_id.clone()
    };

    let memory = meta.memory_mb.unwrap_or(settings.memory_mb).max(512);
    let chain = resolve_version_chain(&version_id)?;
    let merged = merge_versions(&chain)?;

    let java_pref = meta
        .java_path
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(settings.java_path.as_deref());
    let java = resolve_java_or_install(&app, java_pref, merged.java_version.as_ref()).await?;

    let game_dir = crate::paths::instances_dir().join(instance_id);
    fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(game_dir.join("mods")).map_err(|e| e.to_string())?;
    let _ = crate::instances::ensure_instance_with_version(instance_id, &version_id);

    if settings.ingame_branding {
        crate::branding::install_ingame_branding(&game_dir)?;
    } else {
        let _ = crate::branding::remove_branding_from_options(&game_dir);
    }

    let natives = natives_dir_for(&version_id);
    fs::create_dir_all(&natives).map_err(|e| e.to_string())?;
    extract_natives(&merged.libraries, &natives)?;

    let classpath = build_classpath(&version_id, &merged.libraries)?;
    let asset_index = merged
        .asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .or(merged.assets.clone())
        .unwrap_or_else(|| "legacy".into());

    let width = settings.width.max(640);
    let height = settings.height.max(480);
    let features = launch_features(!settings.fullscreen);
    let arg_ctx = ArgContext {
        classpath: &classpath,
        natives: &natives,
        account: &account,
        version_id: &version_id,
        game_dir: &game_dir,
        asset_index: &asset_index,
        width,
        height,
        features: &features,
    };

    let mut jvm_args = vec![
        format!("-Xmx{}M", memory),
        format!("-Xms{}M", (memory / 4).max(512)),
        format!("-Djava.library.path={}", natives.display()),
        "-Dminecraft.launcher.brand=Cubera".into(),
        "-Dminecraft.launcher.version=0.2.4".into(),
    ];

    let extra_jvm = meta
        .jvm_args
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(settings.jvm_args.as_str());
    if !extra_jvm.trim().is_empty() {
        jvm_args.extend(extra_jvm.split_whitespace().map(|s| s.to_string()));
    }

    if let Some(args) = merged.arguments.as_ref().and_then(|a| a.jvm.as_ref()) {
        jvm_args.extend(expand_args(args, &arg_ctx));
    } else {
        jvm_args.push("-cp".into());
        jvm_args.push(classpath.clone());
    }

    if !jvm_args.iter().any(|a| a == "-cp" || a == "-classpath") {
        jvm_args.push("-cp".into());
        jvm_args.push(classpath.clone());
    }

    jvm_args.push(merged.main_class.clone());

    let mut game_args = Vec::new();
    if let Some(args) = merged.arguments.as_ref().and_then(|a| a.game.as_ref()) {
        game_args.extend(expand_args(args, &arg_ctx));
    } else if let Some(legacy) = &merged.minecraft_arguments {
        game_args.extend(
            legacy
                .split_whitespace()
                .map(|s| replace_tokens(s, &arg_ctx)),
        );
    } else {
        game_args.extend([
            "--username".into(),
            account.name.clone(),
            "--version".into(),
            version_id.clone(),
            "--gameDir".into(),
            game_dir.display().to_string(),
            "--assetsDir".into(),
            assets_dir().display().to_string(),
            "--assetIndex".into(),
            asset_index.clone(),
            "--uuid".into(),
            account.uuid.clone(),
            "--accessToken".into(),
            account.access_token.clone(),
            "--userType".into(),
            if account.offline {
                "legacy".into()
            } else {
                "msa".into()
            },
            "--versionType".into(),
            "Cubera".into(),
        ]);
    }

    // Never launch in demo unless explicitly requested via features
    game_args.retain(|a| a != "--demo");

    if settings.fullscreen {
        if !game_args.iter().any(|a| a == "--fullscreen") {
            game_args.push("--fullscreen".into());
        }
    } else {
        game_args.retain(|a| a != "--fullscreen");
        if !game_args.iter().any(|a| a == "--width") {
            game_args.push("--width".into());
            game_args.push(width.to_string());
            game_args.push("--height".into());
            game_args.push(height.to_string());
        }
    }

    // Drop any args that still contain unsubstituted placeholders
    game_args.retain(|a| !a.contains("${"));
    jvm_args.retain(|a| !a.contains("${"));

    let log_path = game_dir.join("cubera-launch.log");
    let err_path = game_dir.join("cubera-launch.err.log");
    let log_file = fs::File::create(&log_path).ok();
    let err_file = fs::File::create(&err_path).ok();

    let mut cmd = Command::new(&java);
    cmd.args(&jvm_args)
        .args(&game_args)
        .current_dir(&game_dir)
        .stdin(Stdio::null());

    if let Some(out) = log_file {
        cmd.stdout(Stdio::from(out));
    } else {
        cmd.stdout(Stdio::null());
    }
    if let Some(err) = err_file {
        cmd.stderr(Stdio::from(err));
    } else {
        cmd.stderr(Stdio::null());
    }

    let child = cmd.spawn().map_err(|e| format!("Failed to launch: {e}"))?;
    let pid = child.id();
    // Detach: we track the PID ourselves; do not wait/kill on Drop.
    std::mem::forget(child);
    crate::instances::register_running(instance_id, pid);
    let _ = crate::instances::mark_played(instance_id);

    // Catch immediate JVM/Minecraft crashes so the UI shows a real error
    tokio::time::sleep(Duration::from_millis(1800)).await;
    if !crate::instances::is_running(instance_id) {
        let log = crate::branding::read_launch_log(instance_id).unwrap_or_default();
        let detail = if !log.stderr.trim().is_empty() {
            log.stderr.trim().to_string()
        } else if !log.stdout.trim().is_empty() {
            log.stdout.trim().to_string()
        } else {
            "No log output. Check that the version is fully installed and Java matches the game.".into()
        };
        let tail: String = detail.chars().rev().take(1200).collect::<String>().chars().rev().collect();
        return Err(format!(
            "Minecraft exited immediately for {version_id}.\n{tail}"
        ));
    }

    Ok(format!(
        "Launched {} ({version_id}) as {}",
        meta.name, account.name
    ))
}

fn active_account<'a>(accounts: &'a [Account], active: Option<&str>) -> Option<&'a Account> {
    if let Some(id) = active {
        if let Some(a) = accounts.iter().find(|a| a.uuid == id) {
            return Some(a);
        }
    }
    accounts.last()
}

fn merge_versions(chain: &[Value]) -> Result<VersionJson, String> {
    // chain[0] is child (forge/fabric), last is vanilla base
    let mut merged = Value::Object(serde_json::Map::new());
    for json in chain.iter().rev() {
        deep_merge(&mut merged, json);
    }
    // Libraries should be concatenated child-first then parent
    let mut libs = Vec::new();
    for json in chain {
        if let Some(arr) = json.get("libraries").and_then(|v| v.as_array()) {
            libs.extend(arr.iter().cloned());
        }
    }
    if let Some(obj) = merged.as_object_mut() {
        obj.insert("libraries".into(), Value::Array(libs));
    }

    serde_json::from_value(merged).map_err(|e| e.to_string())
}

fn deep_merge(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(over_map)) => {
            for (k, v) in over_map {
                if k == "libraries" {
                    continue;
                }
                // Fabric/Quilt/Forge often ship `"game": []` — replacing would wipe
                // vanilla auth/gameDir args and the game would exit immediately.
                if k == "arguments" {
                    merge_arguments(base_map.entry(k.clone()).or_insert(Value::Null), v);
                    continue;
                }
                deep_merge(base_map.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (base, overlay) => *base = overlay.clone(),
    }
}

fn merge_arguments(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(over_map)) => {
            for (k, v) in over_map {
                if (k == "game" || k == "jvm") && v.is_array() {
                    let entry = base_map
                        .entry(k.clone())
                        .or_insert_with(|| Value::Array(Vec::new()));
                    if let (Value::Array(base_arr), Value::Array(over_arr)) = (entry, v) {
                        base_arr.extend(over_arr.iter().cloned());
                    }
                    continue;
                }
                deep_merge(base_map.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (base, overlay) => *base = overlay.clone(),
    }
}

fn build_classpath(version_id: &str, libraries: &[Library]) -> Result<String, String> {
    let mut entries = Vec::new();
    let mut missing = 0u32;
    for lib in libraries {
        if !rule_allows(&lib.rules) {
            continue;
        }
        if lib.natives.is_some() {
            // natives jars go to natives dir, not always classpath — skip classifier-only
            if lib
                .downloads
                .as_ref()
                .and_then(|d| d.artifact.as_ref())
                .is_none()
            {
                continue;
            }
        }
        let path = if let Some(artifact) = lib
            .downloads
            .as_ref()
            .and_then(|d| d.artifact.as_ref())
        {
            libraries_dir().join(&artifact.path)
        } else {
            libraries_dir().join(crate::manifest::maven_path(&lib.name))
        };
        if path.exists() {
            entries.push(path.display().to_string());
        } else {
            missing += 1;
        }
    }

    let client_jar = versions_dir()
        .join(version_id)
        .join(format!("{version_id}.jar"));
    let mut has_client = false;
    // For inherited versions, client jar is on the vanilla id
    if client_jar.exists() {
        entries.push(client_jar.display().to_string());
        has_client = true;
    } else if let Ok(chain) = resolve_version_chain(version_id) {
        for json in chain.iter().rev() {
            if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                let jar = versions_dir().join(id).join(format!("{id}.jar"));
                if jar.exists() {
                    entries.push(jar.display().to_string());
                    has_client = true;
                    break;
                }
            }
        }
    }

    if !has_client {
        return Err(format!(
            "Client jar missing for `{version_id}`. Reinstall this version from the Install tab."
        ));
    }
    if missing > 12 {
        return Err(format!(
            "Classpath incomplete ({missing} libraries missing) for `{version_id}`. Reinstall this version."
        ));
    }

    Ok(entries.join(":"))
}

fn extract_natives(libraries: &[Library], natives_dir: &Path) -> Result<(), String> {
    for lib in libraries {
        if !rule_allows(&lib.rules) {
            continue;
        }
        let Some(downloads) = &lib.downloads else {
            continue;
        };
        let Some(classifiers) = &downloads.classifiers else {
            continue;
        };
        for (key, artifact) in classifiers {
            let is_mac = key.contains("natives-osx")
                || key.contains("natives-macos")
                || key.contains("natives-macos-arm64")
                || key.contains("natives-osx-arm64");
            if !is_mac {
                continue;
            }
            if cfg!(target_arch = "aarch64") && key.contains("x86") {
                continue;
            }
            let jar = libraries_dir().join(&artifact.path);
            if !jar.exists() {
                continue;
            }
            extract_zip(&jar, natives_dir)?;
        }
    }
    Ok(())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if name.ends_with('/') || name.contains("META-INF") {
            continue;
        }
        let out_path = dest.join(Path::new(&name).file_name().unwrap_or_default());
        let mut outfile = fs::File::create(out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
    }
    Ok(())
}

struct ArgContext<'a> {
    classpath: &'a str,
    natives: &'a Path,
    account: &'a Account,
    version_id: &'a str,
    game_dir: &'a Path,
    asset_index: &'a str,
    width: u32,
    height: u32,
    features: &'a HashMap<String, bool>,
}

fn expand_args(args: &[Argument], ctx: &ArgContext<'_>) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::String(s) => out.push(replace_tokens(s, ctx)),
            Argument::Object { rules, value } => {
                if rule_allows_with(rules, ctx.features) {
                    match value {
                        ArgumentValue::Single(s) => out.push(replace_tokens(s, ctx)),
                        ArgumentValue::Multiple(list) => {
                            for s in list {
                                out.push(replace_tokens(s, ctx));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn replace_tokens(s: &str, ctx: &ArgContext<'_>) -> String {
    s.replace("${auth_player_name}", &ctx.account.name)
        .replace("${version_name}", ctx.version_id)
        .replace("${game_directory}", &ctx.game_dir.display().to_string())
        .replace("${assets_root}", &assets_dir().display().to_string())
        .replace("${assets_index_name}", ctx.asset_index)
        .replace("${auth_uuid}", &ctx.account.uuid)
        .replace("${auth_access_token}", &ctx.account.access_token)
        .replace("${clientid}", "cubera")
        .replace("${auth_xuid}", "0")
        .replace(
            "${user_type}",
            if ctx.account.offline { "legacy" } else { "msa" },
        )
        .replace("${version_type}", "Cubera")
        .replace("${natives_directory}", &ctx.natives.display().to_string())
        .replace("${launcher_name}", "Cubera")
        .replace("${launcher_version}", "0.2.4")
        .replace("${classpath}", ctx.classpath)
        .replace(
            "${library_directory}",
            &libraries_dir().display().to_string(),
        )
        .replace("${classpath_separator}", ":")
        .replace("${resolution_width}", &ctx.width.to_string())
        .replace("${resolution_height}", &ctx.height.to_string())
}
