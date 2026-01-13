# BoilerRoom

> **⚠️ WORK IN PROGRESS** - No releases yet! You can compile and test yourself, but it's not fully tested. Use at your own risk.

Steam Deck Package Manager - A cross-platform Tauri v2 desktop application for managing game installations on Steam Deck using the Morrenus API.

![BoilerRoom Screenshot](screenshot.png)

---

## ⚠️ Important Notice

This software is intended for **educational and personal use**. Users are responsible for:

- Respecting Steam Terms of Service
- Only downloading content they legally own
- Not distributing copyrighted content
- **Linux-only** — Will not work on Windows or macOS due to SLSsteam dependencies

---

## Table of Contents

- [Features](#features)
- [How It Works](#how-it-works)
- [Operating Modes](#operating-modes)
- [Quick Start](#quick-start)
- [Requirements](#requirements)
- [Use Cases](#use-cases)
- [Technical Architecture](#technical-architecture)
- [Steam Configuration Files](#steam-configuration-files)
- [Development](#development)
- [Project Structure](#project-structure)
- [Credits](#credits)

---

## Features

- 🔗 **Two Operating Modes**: Local (on Steam Deck) or Remote (PC → Deck via SSH)
- 🔍 **Package Search**: Search for available game manifests
- 📦 **One-Click Install**: Download depots, configure Steam, and deploy to your Deck/PC
- ⚙️ **SLSsteam Integration**: Automatically configures games to run in Gaming Mode
- 🔐 **VDF Manipulation**: Injects depot decryption keys into Steam's `config.vdf`
- 📋 **Activity Logs**: Track all operations with exportable logs
- 🔑 **Secure Storage**: API keys stored in system keychain
- 🎮 **Steam Library Management**: List, uninstall, and manage installed games
- 🖼️ **SteamGridDB Artwork**: Fetch and cache game artwork
- 🛠️ **Steamless Integration**: Optional DRM removal tools
- 📥 **Depot Keys Mode**: Configure Steam to download games directly

---

## How It Works

BoilerRoom orchestrates game installations through a multi-stage pipeline:

### Installation Pipeline

```
┌─────────────────┐
│  1. Search API  │  ─→ Query Morrenus API for game manifests
└────────┬────────┘
         ▼
┌─────────────────┐
│ 2. Download ZIP │  ─→ Fetch manifest bundle (.zip with LUA/manifests)
└────────┬────────┘
         ▼
┌─────────────────┐
│  3. Parse LUA   │  ─→ Extract app IDs, depot keys, manifest IDs
└────────┬────────┘
         ▼
┌─────────────────┐
│ 4. DepotDownload│  ─→ Download game files using DepotDownloaderMod
└────────┬────────┘
         ▼
┌─────────────────┐
│ 5. Transfer     │  ─→ rsync files to Steam Deck (remote mode only)
└────────┬────────┘
         ▼
┌─────────────────┐
│ 6. Configure    │  ─→ Update SLSsteam, config.vdf, create ACF
└─────────────────┘
```

### Alternative: Depot Keys Only Mode

For users who want Steam to handle the download:

1. **Inject decryption keys** into `config.vdf`
2. **Copy manifest files** to `depotcache/`
3. **Create ACF** with `StateFlags=6` (Update Required)
4. **Trigger** `steam://install/{appid}` - Steam downloads the game

---

## Operating Modes

### Local Mode (Running on Steam Deck)

- Downloads game files **directly** to Steam library
- No file transfer needed - files go straight to `steamapps/common/`
- Progress: 0-100% = download only
- Best for: Steam Deck Desktop Mode or SteamOS-compatible Linux distros

### Remote Mode (PC → Steam Deck)

- Downloads to **temporary directory** on PC
- Transfers via **rsync over SSH** to Steam Deck
- Progress scaled: 0-50% = download, 50-100% = transfer
- Requires: SSH enabled on Deck, `sshpass` on PC for password auth

---

## Quick Start

### On Steam Deck (Local Mode)

1. Download the AppImage and `boilerroom-setup.sh` to your Deck
2. Run the setup script:
   ```bash
   chmod +x boilerroom-setup.sh
   ./boilerroom-setup.sh
   ```
3. Launch BoilerRoom from the application menu

### On PC (Remote Mode)

1. Build or download BoilerRoom for your platform
2. Run `boilerroom-setup.sh` on your Steam Deck (enables SSH, installs SLSsteam)
3. Configure SSH connection in BoilerRoom Settings
4. Search and install - games are transferred automatically

---

## Requirements

### External Tools

| Tool | Purpose | Required | Download |
|------|---------|----------|----------|
| **DepotDownloaderMod** | Downloads Steam depots | ✅ Yes | [GitHub](https://github.com/SteamAutoCracks/DepotDownloaderMod) |
| **Steamless** | Removes DRM from executables | ❌ Optional | [GitHub](https://github.com/atom0s/Steamless) |

> Configure paths in **Settings → Paths and Tools**

### Remote Mode (PC → Steam Deck)

Install `sshpass` for automated rsync transfers:

```bash
# Arch Linux / Steam Deck
sudo pacman -S sshpass

# Ubuntu/Debian
sudo apt install sshpass

# macOS
brew install hudochenkov/sshpass/sshpass
```

### API Key

Get a Morrenus API key from [manifest.morrenus.xyz](https://manifest.morrenus.xyz) (Discord login required).
- Free tier: 25 manifests/day
- Key regenerates every 24h

---

## Use Cases

### 1. Install Games Not in Your Library

Search for any game, download the manifests and files, deploy to Steam Deck. SLSsteam makes them appear in your library while in Gaming Mode.

### 2. Transfer Locally Installed Games

Already have games on your PC? Use the Library tab to copy them to your Steam Deck via rsync. BoilerRoom handles:
- File transfer with progress reporting
- ACF manifest creation
- SLSsteam configuration

### 3. Depot Keys Only (Steam Downloads)

For games where you want Steam to handle the download:
1. Injects depot decryption keys into Steam's `config.vdf`
2. Copies manifest files to `depotcache/`
3. Creates an ACF with `StateFlags=6`
4. Opens `steam://install/{appid}` - Steam does the rest

### 4. Manage Your Library

- View installed games with size information
- Uninstall games (removes files, ACF, and SLSsteam entries)
- Check for updates against API manifests

---

## Technical Architecture

### Backend (Rust/Tauri)

The backend is organized into **13 command modules**:

| Module | Purpose |
|--------|---------|
| `api.rs` | Morrenus API, SteamGridDB, artwork caching |
| `connection.rs` | SSH connection, Steam Deck status checks |
| `depot.rs` | Manifest ZIP extraction, LUA parsing, DepotDownloaderMod |
| `depot_keys.rs` | Depot keys only installation, ACF creation |
| `installation.rs` | Pipelined installation orchestration |
| `library.rs` | Installed games listing, uninstall, library paths |
| `settings.rs` | API keys, SLSsteam cache management |
| `slssteam.rs` | SLSsteam installation/verification (local & remote) |
| `steam_fixes.rs` | Steam update disable, libcurl32 fix |
| `steamcmd.rs` | SteamCMD app info integration |
| `steamless_commands.rs` | DRM removal via Steamless |
| `tools.rs` | Steamless GUI launcher, SLSah integration |
| `transfer.rs` | rsync game copy to remote Steam Deck |

### Core Components

| File | Description |
|------|-------------|
| `install_manager.rs` | Pipelined download/transfer with progress events |
| `config_vdf.rs` | VDF parser for `config.vdf` manipulation |
| `steamless.rs` | Steamless execution helpers |

### Key Libraries

| Library | Purpose |
|---------|---------|
| `tauri` v2 | Desktop app framework |
| `ssh2` | Native SSH connections (no sshpass needed for commands) |
| `reqwest` | HTTP client for API calls |
| `regex` | LUA manifest parsing |
| `serde` + `serde_yaml` | Config serialization |
| `zip` | Manifest ZIP extraction |
| `sevenz-rust` | 7z archive extraction (SLSsteam releases) |
| `walkdir` | Recursive file operations |

---

## Steam Configuration Files

BoilerRoom manipulates several Steam configuration files:

### `config.vdf` - Depot Decryption Keys

Steam's main configuration file stores depot decryption keys:

```vdf
"InstallConfigStore"
{
    "depots"
    {
        "228988"
        {
            "DecryptionKey"     "abc123def456..."
        }
    }
}
```

**What BoilerRoom does:**
- Parses existing `config.vdf`
- Injects new depot keys without duplicates
- Finds the `"depots"` section or creates it

### `appmanifest_{appid}.acf` - Game Manifest

Steam uses ACF files to track installed games:

```vdf
"AppState"
{
    "appid"         "123456"
    "name"          "Game Name"
    "StateFlags"    "4"
    "installdir"    "GameFolder"
    "UserConfig"
    {
        "platform_override_dest"    "linux"
        "platform_override_source"  "windows"
    }
}
```

**StateFlags values:**
- `4` = Fully Installed
- `6` = Update Required (triggers Steam download)

### `libraryfolders.vdf` - Steam Library Paths

Parsed to find all Steam library locations:

```vdf
"libraryfolders"
{
    "0"
    {
        "path"      "/home/deck/.steam/steam"
    }
    "1"
    {
        "path"      "/run/media/mmcblk0p1"
    }
}
```

### SLSsteam `config.yaml`

BoilerRoom manages the SLSsteam configuration:

```yaml
PlayNotOwnedGames: yes
SafeMode: yes
AdditionalApps:
  - 480
  - 123456
```

**Injected per-game:**
- App ID added to `AdditionalApps` list
- Optional `FakeAppIds` and `AppTokens` sections

---

## Development

### Prerequisites

- **Node.js** 18+
- **Rust** ([rustup.rs](https://rustup.rs/))
- **Tauri CLI**: `cargo install tauri-cli`

**Platform-specific:**
- **macOS**: Xcode Command Line Tools
- **Linux**: `webkit2gtk-4.1`, `libayatana-appindicator3-dev`
- **Windows**: Visual Studio Build Tools with C++ workload

### Build Commands

```bash
npm install          # Install dependencies
npm run tauri dev    # Development mode
npm run tauri build  # Production build
```

### Linux (Ubuntu/Debian)

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget \
  libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

---

## Project Structure

```
boilerroom/
├── src/                          # React frontend
│   ├── components/
│   │   ├── SearchPanel.tsx       # Game search UI
│   │   ├── LibraryPanel.tsx      # Installed games
│   │   ├── SettingsPanel.tsx     # Configuration
│   │   ├── InstallModal.tsx      # Depot selection
│   │   └── InstallProgress.tsx   # Progress display
│   ├── store/useAppStore.ts      # Zustand state management
│   └── lib/api.ts                # Tauri command wrappers
│
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs                # Tauri setup & plugin registration
│   │   ├── commands/             # Command modules (13 files)
│   │   │   ├── mod.rs            # Module re-exports
│   │   │   ├── api.rs            # Morrenus API
│   │   │   ├── depot.rs          # Manifest parsing
│   │   │   ├── depot_keys.rs     # Depot keys only install
│   │   │   ├── installation.rs   # Install pipeline
│   │   │   ├── library.rs        # Library management
│   │   │   └── ...
│   │   ├── install_manager.rs    # Pipelined installation
│   │   ├── config_vdf.rs         # VDF parser
│   │   └── depots.ini            # Known depot names (5.7MB)
│   └── Cargo.toml
│
├── boilerroom-setup.sh           # Steam Deck setup script
├── 7zz                           # Bundled 7-Zip extractor
└── deps/                         # Bundled dependencies
```

---

## Steam Deck Setup Script

The `boilerroom-setup.sh` script automates Steam Deck configuration:

| Step | Description |
|------|-------------|
| **Mode Selection** | Choose Local or Remote mode |
| **Password Setup** | Configures `deck` user password for sudo |
| **Read-Only Mode** | Disables SteamOS filesystem protection |
| **SSH** | Enables SSH daemon (Remote mode only) |
| **WebKit Cache** | Clears cache to prevent white screen issues |
| **AppImage Install** | Copies to `~/.local/bin/` with desktop entry |
| **SLSsteam** | Downloads and installs latest SLSsteam |

---

## SLSsteam Integration

[SLSsteam](https://github.com/AceSLS/SLSsteam) is a Linux library that enables running games not in your Steam library:

### How It Works

1. **Library Injection**: `LD_AUDIT` environment variable loads `SLSsteam.so`
2. **Ownership Spoofing**: Fakes game ownership checks
3. **Gaming Mode**: Games appear in Steam's Gaming Mode UI

### BoilerRoom's SLSsteam Features

- **Auto-fetch**: Downloads latest release from GitHub
- **Auto-install**: Copies to `~/.local/share/SLSsteam/`
- **Patches**: Modifies `steam.desktop` and `steam-jupiter`
- **Config Management**: Adds game IDs to `config.yaml`

---

## Credits

Special thanks to the creators of the tools that make this project possible:

- **[AceSLS](https://github.com/AceSLS)** - Creator of [SLSsteam](https://github.com/AceSLS/SLSsteam)
- **[atom0s](https://github.com/atom0s)** - Creator of [Steamless](https://github.com/atom0s/Steamless)
- **JD Ros** and the DepotDownloaderMod team
- **Morrenus** - Manifest API provider

---

## License

This project is for educational purposes. Use responsibly and respect game developers' rights.
