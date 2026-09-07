use crate::paths::data_dir;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const PACK_ID: &str = "Cubera.zip";
const PACK_ENTRY: &str = "file/Cubera.zip";

/// Lightweight in-game presence: dock branding (macOS) + title-screen Cubera mark.
pub fn prepare_instance_branding(game_dir: &Path) -> Result<(), String> {
    let packs_dir = game_dir.join("resourcepacks");
    fs::create_dir_all(&packs_dir).map_err(|e| e.to_string())?;
    let pack_path = packs_dir.join(PACK_ID);
    write_cubera_resource_pack(&pack_path)?;
    enable_resource_pack(game_dir)?;
    Ok(())
}

pub fn macos_dock_jvm_args() -> Vec<String> {
    let mut args = vec!["-Xdock:name=Cubera".into()];
    if let Some(icon) = dock_icon_path() {
        args.push(format!("-Xdock:icon={}", icon.display()));
    }
    args
}

fn dock_icon_path() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("/Applications/Cubera.app/Contents/Resources/icon.icns"),
        data_dir().join("branding/icon.icns"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

fn write_cubera_resource_pack(dest: &Path) -> Result<(), String> {
    let file = File::create(dest).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Minecraft 26.2 = resource pack 88.0. Do NOT include older formats:
    // declaring min_format < 65 requires a legacy supported_formats field and
    // the game will remove the pack as incompatible.
    let mcmeta = r#"{
  "pack": {
    "description": "Cubera branding",
    "min_format": [88, 0],
    "max_format": [88, 0]
  }
}
"#;
    zip.start_file("pack.mcmeta", opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(mcmeta.as_bytes())
        .map_err(|e| e.to_string())?;

    let splashes = "\
Cubera!\n\
Cubera!\n\
Cubera!\n\
Cubera!\n\
Cubera!\n\
Cubera!\n\
Cubera!\n\
Cubera!\n\
Gelanceerd met Cubera\n\
Gelanceerd met Cubera\n\
Gelanceerd met Cubera\n\
Obsidian & Copper\n\
macOS Minecraft, netjes\n\
Precision launcher!\n\
";
    zip.start_file("assets/minecraft/texts/splashes.txt", opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(splashes.as_bytes())
        .map_err(|e| e.to_string())?;

    let lang = r#"{
  "menu.game": "Cubera",
  "menu.singleplayer": "Singleplayer · Cubera",
  "menu.multiplayer": "Multiplayer · Cubera"
}
"#;
    zip.start_file("assets/minecraft/lang/en_us.json", opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(lang.as_bytes())
        .map_err(|e| e.to_string())?;
    zip.start_file("assets/minecraft/lang/nl_nl.json", opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(lang.as_bytes())
        .map_err(|e| e.to_string())?;

    // Replace the yellow "Java Edition" ribbon with CUBERA.
    if let Some(edition) = branding_asset("edition.png") {
        zip.start_file("assets/minecraft/textures/gui/title/edition.png", opts)
            .map_err(|e| e.to_string())?;
        zip.write_all(&edition).map_err(|e| e.to_string())?;
    }

    let pack_png = branding_asset("pack.png")
        .or_else(|| {
            let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons/128x128.png");
            fs::read(p).ok()
        })
        .unwrap_or_default();
    if !pack_png.is_empty() {
        zip.start_file("pack.png", opts).map_err(|e| e.to_string())?;
        zip.write_all(&pack_png).map_err(|e| e.to_string())?;
    }

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn branding_asset(name: &str) -> Option<Vec<u8>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("branding").join(name);
    fs::read(path).ok()
}

fn enable_resource_pack(game_dir: &Path) -> Result<(), String> {
    let options = game_dir.join("options.txt");
    let mut text = if options.exists() {
        let mut s = String::new();
        File::open(&options)
            .and_then(|mut f| f.read_to_string(&mut s))
            .map_err(|e| e.to_string())?;
        s
    } else {
        String::new()
    };

    // Always force Cubera first so a previous incompatible remove cannot stick.
    let new_line = if let Some(pos) = text.find("resourcePacks:") {
        let line_end = text[pos..]
            .find('\n')
            .map(|i| pos + i)
            .unwrap_or(text.len());
        let line = &text[pos..line_end];
        let rest = if let Some(start) = line.find('[') {
            let end = line.rfind(']').unwrap_or(line.len());
            line[start + 1..end]
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty() && !s.contains("Cubera"))
                .collect::<Vec<_>>()
                .join(",")
        } else {
            String::new()
        };
        let rebuilt = if rest.is_empty() {
            format!("resourcePacks:[\"{PACK_ENTRY}\"]")
        } else {
            format!("resourcePacks:[\"{PACK_ENTRY}\",{rest}]")
        };
        text.replace_range(pos..line_end, &rebuilt);
        None
    } else {
        Some(format!("resourcePacks:[\"{PACK_ENTRY}\"]\n"))
    };

    if let Some(extra) = new_line {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&extra);
    }

    // Clear incompatible list entries for Cubera if present.
    if let Some(pos) = text.find("incompatibleResourcePacks:") {
        let line_end = text[pos..]
            .find('\n')
            .map(|i| pos + i)
            .unwrap_or(text.len());
        let line = &text[pos..line_end];
        if line.contains("Cubera") {
            let cleaned = line
                .replace(&format!("\"{PACK_ENTRY}\""), "")
                .replace(",,", ",")
                .replace("[,", "[")
                .replace(",]", "]");
            text.replace_range(pos..line_end, &cleaned);
        }
    }

    fs::write(&options, text).map_err(|e| e.to_string())?;
    Ok(())
}
