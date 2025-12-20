# TonTonDeck

> **⚠️ WORK IN PROGRESS** - No releases yet! You can compile and test yourself, but it's not fully tested. Use at your own risk.

Steam Deck Package Manager - A cross-platform Tauri v2 desktop application for managing installations on Steam Deck using the Morrenus API.

![TonTonDeck Screenshot](screenshot.png)

## Features

- 🔗 **Two Operating Modes**: Local (on Steam Deck) or Remote (PC → Deck via SSH)
- 🔍 **Package Search**: Search the Morrenus API for available manifests
- 📦 **One-Click Install**: Download, patch, and deploy to your Deck
- ⚙️ **SLSsteam Integration**: Automatically configures stuff to run in Gaming Mode
- 📋 **Activity Logs**: Track all operations with exportable logs
- 🔐 **Secure Storage**: API keys stored in system keychain

---

## Quick Start

### On Steam Deck (Local Mode)

1. Download the AppImage and `tontondeck-setup.sh` to your Deck
2. Run the setup script:
   ```bash
   chmod +x tontondeck-setup.sh
   ./tontondeck-setup.sh
   ```
3. Launch TonTonDeck from the application menu

### On PC (Remote Mode)

1. Build or download TonTonDeck for your platform
2. Run `tontondeck-setup.sh` on your Steam Deck
3. Configure SSH connection in TonTonDeck Settings
4. Search and install - they'll be transferred to your Deck

---

## Requirements

### External Tools

TonTonDeck requires these tools to be downloaded and configured in Settings:

| Tool | Purpose | Download |
|------|---------|----------|
| **DepotDownloaderMod** | Downloads depots | [GitHub Releases](https://github.com/SteamAutoCracks/DepotDownloaderMod) |
| **Steamless** (optional) | Removes DRM from executables | [GitHub Releases](https://github.com/atom0s/Steamless) |

> Configure the paths to these tools in **Settings → Paths and Tools**

### Remote Mode (PC → Steam Deck)

When transferring files from PC to Steam Deck, you also need:

- **sshpass** - For automated rsync transfers with password authentication
  ```bash
  # Arch Linux / Steam Deck
  sudo pacman -S sshpass
  
  # Ubuntu/Debian
  sudo apt install sshpass
  
  # macOS
  brew install hudochenkov/sshpass/sshpass
  ```

> Without sshpass, rsync will prompt for password on each file transfer.

### API Key

Get a Morrenus API key from [manifest.morrenus.xyz](https://manifest.morrenus.xyz) (login via Discord). Free tier: 25 manifests/day, key regenerates every 24h.

---

## Steam Deck Setup Script

The `tontondeck-setup.sh` script automates Steam Deck configuration:

| Step | Description |
|------|-------------|
| **Mode Selection** | Choose Local or Remote mode |
| **Password Setup** | Configures `deck` user password for sudo |
| **Read-Only Mode** | Disables SteamOS filesystem protection |
| **SSH** | Enables SSH daemon (Remote mode only) |
| **WebKit Cache** | Clears cache to prevent white screen issues |
| **AppImage Install** | Copies to `~/.local/bin/` with desktop entry |
| **SLSsteam** | Optional: Downloads and configures SLSsteam for Gaming Mode |

---

## Development

### Prerequisites

- Node.js 18+
- Rust ([rustup.rs](https://rustup.rs/))
- Tauri CLI: `cargo install tauri-cli`

**Platform-specific:**
- macOS: Xcode Command Line Tools
- Linux: `webkit2gtk-4.1`, `libayatana-appindicator3-dev`
- Windows: Visual Studio Build Tools with C++ workload

### Build Commands

```bash
npm install          # Install dependencies
npm run tauri dev    # Development mode
npm run tauri build  # Production build
```

---

## Project Structure

```
tontondeck/
├── src/                        # React frontend
│   ├── components/
│   │   ├── SearchPanel.tsx     # Search UI
│   │   ├── LibraryPanel.tsx    # Installed
│   │   ├── SettingsPanel.tsx   # Configuration
│   │   ├── InstallModal.tsx    # Depot selection
│   │   └── InstallProgress.tsx # Download/transfer progress
│   ├── store/useAppStore.ts    # Zustand state
│   └── lib/api.ts              # Tauri command wrappers
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs              # Tauri setup
│   │   ├── commands.rs         # All Tauri commands
│   │   └── install_manager.rs  # Installation pipeline
│   └── Cargo.toml
├── tontondeck-setup.sh         # Steam Deck setup script
├── 7zz                         # Bundled 7-Zip extractor
└── screenshot.png
```

---

## Credits

Special thanks to the creators of the tools that make my fun side project possible:

- **[AceSLS](https://github.com/AceSLS)** - Creator of [SLSsteam](https://github.com/AceSLS/SLSsteam)
- **[atom0s](https://github.com/atom0s)** - Creator of [Steamless](https://github.com/atom0s/Steamless)
- JD Ros 
- Creators of DepotDownloaderMod


