# BoilerRoom

> **⚠️ WORK IN PROGRESS** - No releases yet! You can compile and test yourself, but it's not fully tested. Use at your own risk. Code written with assistance of AI (Claude/Gemini).

Steam Deck Game Manager - A Tauri v2 desktop application for managing and syncing your Steam library on Steam Deck/Linux.

![BoilerRoom Screenshot](screenshot.png)

---

## ⚠️ Important Notice

This software is intended for **educational and personal use**. Users are responsible for:

- Respecting Steam Terms of Service
- Only downloading content they legally own
- Only managing content from your **own Steam library**
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
- 🔍 **Game Search**: Search and browse available games
- 📦 **One-Click Install**: Download and configure games for your Deck
- ⚙️ **SLSsteam Integration**: Automatically configures games to run in Gaming Mode
- 📋 **Activity Logs**: Track all operations with exportable logs
- 🔑 **Secure Storage**: API keys stored in system keychain
- 🎮 **Library Management**: List, uninstall, and manage installed games
- 🖼️ **SteamGridDB Artwork**: Fetch and cache game artwork
- 🎵 **OST Support**: Download soundtrack files

---

## How It Works

BoilerRoom helps you manage games on your Steam Deck:

1. **Search** - Find games via the search interface
2. **Download** - Fetch game files to your PC or directly to Deck
3. **Transfer** - Sync files to Steam Deck via SSH (remote mode)
4. **Configure** - Set up SLSsteam so games appear in Gaming Mode

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

### Installation

```bash
# Clone and run the installer
git clone https://github.com/uxandai/boilerroom.git
cd boilerroom
chmod +x install.sh
./install.sh
```

The installer will:
1. Install BoilerRoom binary (from `compiled/` folder)
2. Download and configure SLSsteam
3. Patch Steam for Gaming Mode support
4. Create config files if needed

**Important:** Already configured? The installer detects existing setups and won't overwrite your configs.

### After Installation

1. **Restart Steam** for SLSsteam to take effect
2. Launch BoilerRoom from application menu or run `boilerroom`
3. Configure your Depot Provider API key in Settings

### Troubleshooting

**App shows white screen or doesn't start?**

On Steam Deck / Arch Linux, you may need to initialize keyrings and install WebKit:

```bash
# Initialize pacman keyrings
sudo pacman-key --init
sudo pacman-key --populate archlinux
sudo pacman-key --populate holo  # Steam Deck only

# Install WebKit dependency
sudo pacman -S webkit2gtk-4.1
```

The installer already adds `WEBKIT_DISABLE_COMPOSITING_MODE=1` to the desktop entry to prevent rendering issues.

**SLSsteam install didn't work?**

Use [headcrab.sh](https://github.com/Deadboy666/h3adcr-b) instead:

```bash
curl -fsSL "https://raw.githubusercontent.com/Deadboy666/h3adcr-b/refs/heads/main/headcrab.sh" | bash
```

---

## Requirements

### External Tools

| Tool | Purpose | Required | Download |
|------|---------|----------|----------|
| **DepotDownloaderMod** | Downloads game files | ✅ Yes | [GitHub](https://github.com/SteamAutoCracks/DepotDownloaderMod) |

> Configure paths in **Settings → Paths and Tools**

### Remote Mode Dependencies

Install `sshpass` for automated rsync transfers:

```bash
# Arch Linux / Steam Deck
sudo pacman -S sshpass
```
---

## Use Cases

### 1. Sync Games to Steam Deck

Have games on your PC? Transfer them to your Steam Deck via rsync. BoilerRoom handles:
- File transfer with progress reporting
- Steam library configuration
- SLSsteam setup for Gaming Mode

### 2. Manage Your Library

- View installed games with size information
- Uninstall games cleanly
- Check for updates

---

## Technical Architecture

### Backend (Rust/Tauri)

The backend is organized into **13 command modules**:

| Module | Purpose |
|--------|---------|
| `api.rs` | Depot Provider API, SteamGridDB, artwork caching |
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

## Configuration

### BoilerRoom Configuration

**Location:** `~/.local/share/com.boilerroom.app/settings.json`

BoilerRoom stores its settings in a JSON file managed by Tauri's store plugin:

```json
{
  "connectionMode": "local",
  "api_key": "your-api-key",
  "depot_downloader_path": "/path/to/DepotDownloaderMod",
  "steamless_path": "/path/to/Steamless.CLI.exe",
  "steamgriddb_api_key": "optional-steamgriddb-key",
  "steam_api_key": "optional-steam-web-api-key",
  "steam_user_id": "your-steam-id",
  "achievement_method": "web_api"
}
```

**Key settings:**
- `connectionMode`: `"local"` or `"remote"`
- `api_key`: Depot Provider API key (from manifest.morrenus.xyz)
- `depot_downloader_path`: Path to DepotDownloaderMod executable
- `steamless_path`: Path to Steamless CLI (optional, for DRM removal)

---

## Development

### Prerequisites

-   **Node.js** 18+
-   **Rust** ([rustup.rs](https://rustup.rs/))
-   **Tauri CLI**: `cargo install tauri-cli`

**Platform-specific:**
-   **Linux**: `webkit2gtk-4.1`, `libayatana-appindicator3-dev`

### Build Commands

```bash
npm install                        # Install dependencies
npm run tauri dev                  # Development mode
npm run tauri build -- --no-bundle # Production build
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
│   │   │   ├── api.rs            # Depot Provider API
│   │   │   ├── depot.rs          # Manifest parsing
│   │   │   ├── depot_keys.rs     # Depot keys only install
│   │   │   ├── installation.rs   # Install pipeline
│   │   │   ├── library.rs        # Library management
│   │   │   └── ...
│   │   ├── install_manager.rs    # Pipelined installation
│   │   ├── steamless.rs          # DRM removal helpers
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
- **[DepotDownloaderMod](https://github.com/SteamAutoCracks/DepotDownloaderMod)** - Depot downloader
- **[Deadboy666/h3adcr-b](https://github.com/Deadboy666/h3adcr-b)** - Cool bash scripts provider
- **[xamionex/SLScheevo](https://github.com/xamionex/SLScheevo)** - Achievement unlocker
- **[niwia/SLSah](https://github.com/niwia/SLSah)** - Achievement unlocker

---

## License

This project is for educational purposes. Use responsibly and respect game developers' rights.
