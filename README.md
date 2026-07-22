# Cubera

A modern Minecraft launcher for macOS — vanilla, Fabric, Forge, and Modrinth.

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

- Microsoft sign-in (device code) + offline accounts
- Install vanilla, Fabric & Forge
- Search & install Modrinth mods
- Memory, resolution, fullscreen, and JVM args
- In-game Cubera branding via resource pack
- Data stored in `~/Library/Application Support/Cubera/`

## License

MIT
