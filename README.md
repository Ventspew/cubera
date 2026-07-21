# Cubera

Een moderne Minecraft-launcher voor macOS — vanilla, Fabric, Forge en Modrinth.

Gebouwd met **Tauri 2 + React + TypeScript**.

## Installeren (macOS)

1. Download de nieuwste **`.dmg`** van [Releases](https://github.com/Ventspew/cubera/releases).
2. Open de disk image.
3. Sleep **Cubera** naar **Applications**.
4. Start Cubera (bij eerste start: Systeeminstellingen → Privacy → openen toestaan als macOS dat vraagt).

Java 17+ is nodig om te spelen:

```bash
brew install --cask temurin
```

## Ontwikkelen

```bash
npm install
npm run tauri dev
```

## Bouwen

```bash
npm run tauri build
```

De installer verschijnt in:

```
src-tauri/target/release/bundle/dmg/
```

## Features

- Microsoft-login (device code) + offline accounts
- Vanilla, Fabric & Forge installeren
- Modrinth mods zoeken & installeren
- Geheugen, resolutie, fullscreen en JVM-args
- Data in `~/Library/Application Support/Cubera/`

## Licentie

MIT
