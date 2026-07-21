use crate::download::{natives_dir_for, resolve_version_chain};
use crate::manifest::{rule_allows, Argument, ArgumentValue, Library, VersionJson};
use crate::paths::{assets_dir, libraries_dir, load_settings, versions_dir, Account};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

pub async fn launch_game(version_id: &str) -> Result<String, String> {
    let settings = load_settings();
    let account = active_account(&settings.accounts, settings.active_account.as_deref())
        .ok_or_else(|| "No account selected. Log in first.".to_string())?;

    let java = find_java(settings.java_path.as_deref())?;
    let chain = resolve_version_chain(version_id)?;
    let merged = merge_versions(&chain)?;

    let game_dir = crate::paths::instances_dir().join(version_id);
    fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(game_dir.join("mods")).map_err(|e| e.to_string())?;

    let natives = natives_dir_for(version_id);
    fs::create_dir_all(&natives).map_err(|e| e.to_string())?;
    extract_natives(&merged.libraries, &natives)?;

    let classpath = build_classpath(version_id, &merged.libraries)?;
    let asset_index = merged
        .asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .or(merged.assets.clone())
        .unwrap_or_else(|| "legacy".into());

    let mut jvm_args = vec![
        format!("-Xmx{}M", settings.memory_mb),
        format!("-Xms{}M", (settings.memory_mb / 4).max(512)),
        format!("-Djava.library.path={}", natives.display()),
        "-Dminecraft.launcher.brand=Cubera".into(),
        "-Dminecraft.launcher.version=0.1.0".into(),
    ];

    if !settings.jvm_args.trim().is_empty() {
        jvm_args.extend(
            settings
                .jvm_args
                .split_whitespace()
                .map(|s| s.to_string()),
        );
    }

    if let Some(args) = merged.arguments.as_ref().and_then(|a| a.jvm.as_ref()) {
        jvm_args.extend(expand_args(args, &classpath, &natives, &account, version_id, &game_dir, &asset_index));
    } else {
        jvm_args.push("-cp".into());
        jvm_args.push(classpath.clone());
    }

    // Ensure -cp is present for modern args that use ${classpath}
    if !jvm_args.iter().any(|a| a == "-cp" || a == "-classpath") {
        jvm_args.push("-cp".into());
        jvm_args.push(classpath.clone());
    }

    jvm_args.push(merged.main_class.clone());

    let mut game_args = Vec::new();
    if let Some(args) = merged.arguments.as_ref().and_then(|a| a.game.as_ref()) {
        game_args.extend(expand_args(
            args,
            &classpath,
            &natives,
            &account,
            version_id,
            &game_dir,
            &asset_index,
        ));
    } else if let Some(legacy) = &merged.minecraft_arguments {
        game_args.extend(legacy.split_whitespace().map(|s| {
            replace_tokens(
                s,
                &classpath,
                &natives,
                &account,
                version_id,
                &game_dir,
                &asset_index,
            )
        }));
    } else {
        game_args.extend([
            "--username".into(),
            account.name.clone(),
            "--version".into(),
            version_id.into(),
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

    // Window / fullscreen
    if settings.fullscreen {
        if !game_args.iter().any(|a| a == "--fullscreen") {
            game_args.push("--fullscreen".into());
        }
    } else {
        game_args.retain(|a| a != "--fullscreen");
        if !game_args.iter().any(|a| a == "--width") {
            game_args.push("--width".into());
            game_args.push(settings.width.max(640).to_string());
            game_args.push("--height".into());
            game_args.push(settings.height.max(480).to_string());
        }
    }

    // Log file
    let log_path = game_dir.join("cubera-launch.log");
    let log_file = fs::File::create(&log_path).ok();
    let err_file = fs::File::create(game_dir.join("cubera-launch.err.log")).ok();

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

    cmd.spawn().map_err(|e| format!("Failed to launch: {e}"))?;
    Ok(format!("Launched {version_id} as {}", account.name))
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
                deep_merge(base_map.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (base, overlay) => *base = overlay.clone(),
    }
}

fn build_classpath(version_id: &str, libraries: &[Library]) -> Result<String, String> {
    let mut entries = Vec::new();
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
        }
    }

    let client_jar = versions_dir()
        .join(version_id)
        .join(format!("{version_id}.jar"));
    // For inherited versions, client jar is on the vanilla id
    if client_jar.exists() {
        entries.push(client_jar.display().to_string());
    } else {
        // walk inherits
        if let Ok(chain) = resolve_version_chain(version_id) {
            for json in chain.iter().rev() {
                if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                    let jar = versions_dir().join(id).join(format!("{id}.jar"));
                    if jar.exists() {
                        entries.push(jar.display().to_string());
                        break;
                    }
                }
            }
        }
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

fn expand_args(
    args: &[Argument],
    classpath: &str,
    natives: &Path,
    account: &Account,
    version_id: &str,
    game_dir: &Path,
    asset_index: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::String(s) => out.push(replace_tokens(
                s, classpath, natives, account, version_id, game_dir, asset_index,
            )),
            Argument::Object { rules, value } => {
                if crate::manifest::rule_allows(rules) {
                    match value {
                        ArgumentValue::Single(s) => out.push(replace_tokens(
                            s, classpath, natives, account, version_id, game_dir, asset_index,
                        )),
                        ArgumentValue::Multiple(list) => {
                            for s in list {
                                out.push(replace_tokens(
                                    s, classpath, natives, account, version_id, game_dir,
                                    asset_index,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn replace_tokens(
    s: &str,
    classpath: &str,
    natives: &Path,
    account: &Account,
    version_id: &str,
    game_dir: &Path,
    asset_index: &str,
) -> String {
    s.replace("${auth_player_name}", &account.name)
        .replace("${version_name}", version_id)
        .replace("${game_directory}", &game_dir.display().to_string())
        .replace("${assets_root}", &assets_dir().display().to_string())
        .replace("${assets_index_name}", asset_index)
        .replace("${auth_uuid}", &account.uuid)
        .replace("${auth_access_token}", &account.access_token)
        .replace("${clientid}", "cubera")
        .replace("${auth_xuid}", "0")
        .replace(
            "${user_type}",
            if account.offline { "legacy" } else { "msa" },
        )
        .replace("${version_type}", "Cubera")
        .replace("${natives_directory}", &natives.display().to_string())
        .replace("${launcher_name}", "Cubera")
        .replace("${launcher_version}", "0.1.0")
        .replace("${classpath}", classpath)
        .replace(
            "${library_directory}",
            &libraries_dir().display().to_string(),
        )
        .replace("${classpath_separator}", ":")
}
