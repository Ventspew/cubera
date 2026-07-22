use png::Encoder;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

const PACK_NAME: &str = "Cubera-Branding";
const PACK_FILE: &str = "Cubera-Branding.zip";

pub fn install_ingame_branding(game_dir: &Path) -> Result<(), String> {
    let packs_dir = game_dir.join("resourcepacks");
    fs::create_dir_all(&packs_dir).map_err(|e| e.to_string())?;

    let zip_path = packs_dir.join(PACK_FILE);
    if !zip_path.exists() {
        write_branding_pack(&zip_path)?;
    }

    enable_resource_pack(game_dir)?;
    Ok(())
}

fn write_branding_pack(zip_path: &Path) -> Result<(), String> {
    let file = fs::File::create(zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mcmeta = r#"{
  "pack": {
    "pack_format": 34,
    "description": "§6Cubera §7— mineral branding"
  }
}
"#;
    zip.start_file("pack.mcmeta", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(mcmeta.as_bytes())
        .map_err(|e| e.to_string())?;

    let splashes = include_str!("../branding/splashes.txt");
    zip.start_file("assets/minecraft/texts/splashes.txt", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(splashes.as_bytes())
        .map_err(|e| e.to_string())?;

    let edition_png = include_bytes!("../branding/edition.png");
    zip.start_file(
        "assets/minecraft/textures/gui/title/edition.png",
        options,
    )
    .map_err(|e| e.to_string())?;
    zip.write_all(edition_png)
        .map_err(|e| e.to_string())?;

    let logo_png = include_bytes!("../branding/title_minceraft.png");
    zip.start_file(
        "assets/minecraft/textures/gui/title/minceraft.png",
        options,
    )
    .map_err(|e| e.to_string())?;
    zip.write_all(logo_png)
        .map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn enable_resource_pack(game_dir: &Path) -> Result<(), String> {
    let options_path = game_dir.join("options.txt");
    let pack_ref = format!("file/{PACK_FILE}");

    if options_path.exists() {
        let raw = fs::read_to_string(&options_path).map_err(|e| e.to_string())?;
        if raw.contains(PACK_FILE) {
            return Ok(());
        }
        let updated = merge_resource_packs_line(&raw, &pack_ref);
        fs::write(&options_path, updated).map_err(|e| e.to_string())?;
    } else {
        let content = format!("resourcePacks:[\"{pack_ref}\"]\n");
        fs::write(&options_path, content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn merge_resource_packs_line(raw: &str, pack_ref: &str) -> String {
    let mut lines: Vec<String> = raw.lines().map(String::from).collect();
    let mut found = false;

    for line in lines.iter_mut() {
        if line.starts_with("resourcePacks:") {
            found = true;
            if line.contains('[') && line.contains(']') {
                let start = line.find('[').unwrap_or(0);
                let end = line.rfind(']').unwrap_or(line.len());
                let inner = &line[start + 1..end];
                let mut entries: Vec<String> = if inner.trim().is_empty() {
                    vec![]
                } else {
                    inner
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                };
                if !entries.iter().any(|e| e.contains(PACK_NAME)) {
                    entries.push(pack_ref.trim_matches('"').to_string());
                }
                *line = format!(
                    "resourcePacks:[{}]",
                    entries
                        .iter()
                        .map(|e| format!("\"{e}\""))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
    }

    if !found {
        lines.push(format!("resourcePacks:[\"{pack_ref}\"]"));
    }

    let mut out = lines.join("\n");
    if raw.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub fn read_launch_log(instance_id: &str) -> Result<LaunchLog, String> {
    let game_dir = crate::paths::instances_dir().join(instance_id);
    let stdout = read_tail(&game_dir.join("cubera-launch.log"), 8000);
    let stderr = read_tail(&game_dir.join("cubera-launch.err.log"), 4000);
    Ok(LaunchLog { stdout, stderr })
}

fn read_tail(path: &Path, max_bytes: usize) -> String {
    if !path.exists() {
        return String::new();
    }
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return String::new();
    }
    if buf.len() <= max_bytes {
        return String::from_utf8_lossy(&buf).to_string();
    }
    String::from_utf8_lossy(&buf[buf.len() - max_bytes..]).to_string()
}

#[derive(serde::Serialize)]
pub struct LaunchLog {
    pub stdout: String,
    pub stderr: String,
}

#[derive(serde::Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tagline: String,
}

pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Cubera".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        tagline: "Precision instrument for macOS".into(),
    }
}

pub fn open_data_folder() -> Result<(), String> {
    let dir = crate::paths::data_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("open")
        .arg(&dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remove_branding_from_options(game_dir: &Path) -> Result<(), String> {
    let options_path = game_dir.join("options.txt");
    if !options_path.exists() {
        return Ok(());
    }
    let raw = fs::read_to_string(&options_path).map_err(|e| e.to_string())?;
    if !raw.contains(PACK_FILE) {
        return Ok(());
    }
    let updated = raw.replace(&format!("\"file/{PACK_FILE}\",", ""), "");
    let updated = updated.replace(&format!(",\"file/{PACK_FILE}\""), "");
    let updated = updated.replace(&format!("\"file/{PACK_FILE}\""), "");
    fs::write(&options_path, updated).map_err(|e| e.to_string())?;
    Ok(())
}
