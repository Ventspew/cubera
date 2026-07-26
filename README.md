# Cubera

A modern Minecraft launcher for macOS — built to compete with Prism, MultiMC, and the Modrinth App.

Built with **Tauri 2 + React + TypeScript**.

## Install (macOS)

1. Download the latest **`.dmg`** from [Releases](https://github.com/Ventspew/cubera/releases).
2. Open the disk image.
3. Drag **Cubera** to **Applications**.
4. If macOS blocks the app or moves it to Trash:

```bash
xattr -cr /Applications/Cubera.app
open /Applications/Cubera.app
```

Or right-click → **Open**.

Java 17+ is required to play:

```bash
brew install --cask temurin
```

## Development

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

The installer appears in:

```
src-tauri/target/release/bundle/dmg/
```

## Features

### Instances (first-class)
- Named instances with notes, last played, play count
- Per-instance memory & JVM args (override globals)
- Duplicate instances (shared version profile)
- Launch / kill process tracking
- Quick folders: mods, saves, screenshots

### Install
- Vanilla, Fabric, Quilt, Forge
- Release / snapshot / all version filters

### Content
- Modrinth mods, resource packs, and shaders
- Filtered by instance game version & loader

### Account & launch
- Microsoft device-code login with **token refresh**
- Offline accounts with correct Mojang offline UUIDs
- In-game Cubera branding (resource pack + splashes)
- Minecraft news feed
- Launch log viewer

### Data

Stored in `~/Library/Application Support/Cubera/`.

## License

MIT
