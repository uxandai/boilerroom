# TonTon - Decky Loader Plugin

Sync TonTonDeck-installed games from your PC to Steam Deck.

## Features

- 🎮 **List TonTonDeck games** - Shows only games installed via TonTonDeck/ACCELA (games with `.DepotDownloader` marker)
- 🔄 **Rsync transfer** - Fast, resumable file transfer using rsync
- 📁 **ACF support** - Automatically syncs `.acf` manifest files so games appear in Steam

## Installation

1. Install [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader) on your Steam Deck
2. Download the latest `TonTon-vX.X.X.zip` from releases
3. In Decky → Settings → Install from ZIP
4. Select the TonTon ZIP file

## Requirements

- **SSH on PC**: Enable SSH server on your PC (OpenSSH)
- **sshpass**: Should be available on Steam Deck by default
- **rsync**: Should be available on Steam Deck by default

## Usage

1. Open Quick Access Menu (•••) → Decky → TonTon
2. Go to **Settings** tab
3. Enter your PC's IP address, username, and password
4. Click **Test Connection** to verify
5. Go to **Games** tab
6. Click **Refresh** to load your TonTonDeck games
7. Click **Sync** next to any game to transfer it

## PC Setup

Your PC needs SSH access. On Linux/macOS:

```bash
# Enable SSH (if not already)
sudo systemctl enable --now sshd

# Find your IP
ip addr  # or: hostname -I
```

On Windows, enable OpenSSH Server via Settings → Apps → Optional Features.

## License

BSD 3-Clause License
