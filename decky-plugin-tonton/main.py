"""
TonTon - Sync TonTonDeck games from PC to Steam Deck
"""
import os
import subprocess
import asyncio
import json
import logging
import re
from settings import SettingsManager

# Set up logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("TonTon")

# Settings manager for persistent config
settings_dir = os.environ.get("DECKY_PLUGIN_SETTINGS_DIR", "/tmp/tonton")
settings = SettingsManager(name="settings", settings_directory=settings_dir)
settings.read()

# Global sync state for progress tracking
sync_state = {
    "syncing": False,
    "current_game": None,
    "current_file": None,
    "files_done": 0,
    "files_total": 0,
    "bytes_done": 0,
    "bytes_total": 0,
}


class Plugin:
    """Main plugin class for TonTon."""

    async def _main(self):
        """Called when plugin loads."""
        logger.info("TonTon plugin loaded")

    async def _unload(self):
        """Called when plugin unloads."""
        logger.info("TonTon plugin unloaded")

    # =========================================================================
    # Settings Management
    # =========================================================================

    async def get_settings(self) -> dict:
        """Get all settings."""
        return {
            "pc_ip": settings.getSetting("pc_ip", ""),
            "pc_user": settings.getSetting("pc_user", ""),
            "pc_password": settings.getSetting("pc_password", ""),
            "steam_path": settings.getSetting("steam_path", "~/.steam/steam"),
        }

    async def save_settings(
        self, pc_ip: str, pc_user: str, pc_password: str, steam_path: str = "~/.steam/steam"
    ) -> bool:
        """Save PC connection settings."""
        try:
            settings.setSetting("pc_ip", pc_ip)
            settings.setSetting("pc_user", pc_user)
            settings.setSetting("pc_password", pc_password)
            settings.setSetting("steam_path", steam_path)
            settings.commit()
            logger.info(f"Settings saved for PC: {pc_ip}")
            return True
        except Exception as e:
            logger.error(f"Failed to save settings: {e}")
            return False

    async def test_connection(self) -> dict:
        """Test SSH connection to PC."""
        pc_ip = settings.getSetting("pc_ip", "")
        pc_user = settings.getSetting("pc_user", "")
        pc_password = settings.getSetting("pc_password", "")

        if not pc_ip or not pc_user:
            return {"success": False, "error": "PC IP and username required"}

        try:
            # Use sshpass to test connection
            cmd = [
                "sshpass", "-p", pc_password,
                "ssh", "-o", "StrictHostKeyChecking=no",
                "-o", "ConnectTimeout=5",
                f"{pc_user}@{pc_ip}",
                "echo ok"
            ]
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=10)

            if result.returncode == 0 and "ok" in result.stdout:
                return {"success": True}
            else:
                return {"success": False, "error": result.stderr or "Connection failed"}
        except subprocess.TimeoutExpired:
            return {"success": False, "error": "Connection timeout"}
        except Exception as e:
            return {"success": False, "error": str(e)}

    # =========================================================================
    # Game List Management
    # =========================================================================

    async def get_pc_games(self) -> dict:
        """
        Fetch list of TonTonDeck-installed games from PC.
        Only returns games that have the .DepotDownloader marker folder.
        """
        pc_ip = settings.getSetting("pc_ip", "")
        pc_user = settings.getSetting("pc_user", "")
        pc_password = settings.getSetting("pc_password", "")
        steam_path = settings.getSetting("steam_path", "~/.steam/steam")

        if not pc_ip or not pc_user:
            return {"success": False, "error": "Configure PC connection first", "games": []}

        try:
            # SSH command to find TonTonDeck games
            # Uses Python on remote for proper JSON generation
            remote_script = f'''
import os
import json

steam_path = os.path.expanduser("{steam_path}")
steamapps = os.path.join(steam_path, "steamapps")
common = os.path.join(steamapps, "common")

# Build installdir to appid map from ACF files
appid_map = {{}}
if os.path.isdir(steamapps):
    for f in os.listdir(steamapps):
        if f.startswith("appmanifest_") and f.endswith(".acf"):
            try:
                with open(os.path.join(steamapps, f), "r") as acf:
                    content = acf.read()
                    appid = None
                    installdir = None
                    for line in content.split("\\n"):
                        if '"appid"' in line:
                            parts = line.split('"')
                            if len(parts) >= 4:
                                appid = parts[3]
                        if '"installdir"' in line:
                            parts = line.split('"')
                            if len(parts) >= 4:
                                installdir = parts[3]
                    if appid and installdir:
                        appid_map[installdir] = appid
            except:
                pass

# Find games with .DepotDownloader marker
games = []
if os.path.isdir(common):
    for game_name in os.listdir(common):
        game_path = os.path.join(common, game_name)
        marker_path = os.path.join(game_path, ".DepotDownloader")
        if os.path.isdir(marker_path):
            # Calculate size
            size = 0
            try:
                for root, dirs, files in os.walk(game_path):
                    for f in files:
                        try:
                            size += os.path.getsize(os.path.join(root, f))
                        except:
                            pass
            except:
                pass
            
            games.append({{
                "name": game_name,
                "app_id": appid_map.get(game_name, "unknown"),
                "size_bytes": size
            }})

print(json.dumps(games))
'''
            cmd = [
                "sshpass", "-p", pc_password,
                "ssh", "-o", "StrictHostKeyChecking=no",
                "-o", "ConnectTimeout=10",
                f"{pc_user}@{pc_ip}",
                "python3", "-c", remote_script
            ]

            result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)

            if result.returncode != 0:
                logger.error(f"SSH error: {result.stderr}")
                return {"success": False, "error": result.stderr or "Failed to fetch games", "games": []}

            # Parse JSON output
            try:
                games = json.loads(result.stdout.strip())
                logger.info(f"Found {len(games)} TonTonDeck games on PC")
                return {"success": True, "games": games}
            except json.JSONDecodeError as e:
                logger.error(f"Failed to parse game list: {e}")
                logger.error(f"Output was: {result.stdout}")
                return {"success": False, "error": "Failed to parse game list", "games": []}

        except subprocess.TimeoutExpired:
            return {"success": False, "error": "Timeout fetching games", "games": []}
        except Exception as e:
            logger.error(f"Error fetching games: {e}")
            return {"success": False, "error": str(e), "games": []}

    # =========================================================================
    # Game Sync (rsync)
    # =========================================================================

    async def sync_game(self, game_name: str, app_id: str) -> dict:
        """
        Sync a game from PC to Steam Deck using rsync.
        Copies:
        - Game folder from steamapps/common/{game_name}/
        - ACF file from steamapps/appmanifest_{app_id}.acf
        """
        global sync_state
        
        pc_ip = settings.getSetting("pc_ip", "")
        pc_user = settings.getSetting("pc_user", "")
        pc_password = settings.getSetting("pc_password", "")
        pc_steam_path = settings.getSetting("steam_path", "~/.steam/steam")

        if not pc_ip or not pc_user:
            return {"success": False, "error": "Configure PC connection first"}

        # Local Steam path on Steam Deck
        local_steam_path = os.path.expanduser("~/.steam/steam")
        local_steamapps = os.path.join(local_steam_path, "steamapps")
        local_common = os.path.join(local_steamapps, "common")

        # Ensure local directories exist
        os.makedirs(local_common, exist_ok=True)

        # Update sync state
        sync_state["syncing"] = True
        sync_state["current_game"] = game_name
        sync_state["current_file"] = None
        sync_state["files_done"] = 0
        sync_state["files_total"] = 0

        try:
            # 1. Sync game folder with rsync - using --info=progress2 for better progress
            logger.info(f"=== Starting sync for game: {game_name} ===")

            game_src = f"{pc_user}@{pc_ip}:{pc_steam_path}/steamapps/common/{game_name}/"
            game_dst = os.path.join(local_common, game_name)

            # Use itemize-changes to see each file
            rsync_game_cmd = [
                "sshpass", "-p", pc_password,
                "rsync", "-avz", "--itemize-changes",
                "-e", "ssh -o StrictHostKeyChecking=no",
                game_src,
                game_dst + "/"
            ]

            logger.info(f"Running rsync: {game_src} -> {game_dst}")
            
            # Run rsync with real-time output logging
            process = subprocess.Popen(
                rsync_game_cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1
            )
            
            files_synced = 0
            current_file = None
            
            # Read output line by line
            for line in process.stdout:
                line = line.strip()
                if not line:
                    continue
                
                # Parse rsync itemize output: >f..t...... filename
                # The format is: YXcstpoguax filename
                # Where Y is the update type (> = receiving)
                if line.startswith('>f') or line.startswith('<f') or line.startswith('cf'):
                    # This is a file being transferred
                    parts = line.split(' ', 1)
                    if len(parts) >= 2:
                        current_file = parts[1]
                        sync_state["current_file"] = current_file
                        files_synced += 1
                        sync_state["files_done"] = files_synced
                        logger.info(f"[{files_synced}] Syncing: {current_file}")
                elif line.startswith('>d') or line.startswith('cd'):
                    # Directory
                    parts = line.split(' ', 1)
                    if len(parts) >= 2:
                        logger.info(f"Directory: {parts[1]}")
            
            # Wait for process to complete
            process.wait()
            
            if process.returncode != 0:
                stderr = process.stderr.read()
                logger.error(f"rsync failed: {stderr}")
                sync_state["syncing"] = False
                return {"success": False, "error": f"rsync failed: {stderr}"}

            logger.info(f"=== Game sync complete: {files_synced} files transferred ===")

            # 2. Sync ACF file
            if app_id and app_id != "unknown":
                logger.info(f"Syncing ACF file for app {app_id}")
                sync_state["current_file"] = f"appmanifest_{app_id}.acf"

                acf_src = f"{pc_user}@{pc_ip}:{pc_steam_path}/steamapps/appmanifest_{app_id}.acf"
                acf_dst = os.path.join(local_steamapps, f"appmanifest_{app_id}.acf")

                rsync_acf_cmd = [
                    "sshpass", "-p", pc_password,
                    "rsync", "-avz",
                    "-e", "ssh -o StrictHostKeyChecking=no",
                    acf_src,
                    acf_dst
                ]

                acf_result = subprocess.run(rsync_acf_cmd, capture_output=True, text=True, timeout=60)

                if acf_result.returncode != 0:
                    logger.warning(f"Failed to sync ACF: {acf_result.stderr}")
                    # Continue anyway - game files are more important
                else:
                    logger.info(f"ACF file synced: appmanifest_{app_id}.acf")

            sync_state["syncing"] = False
            sync_state["current_file"] = None
            logger.info(f"=== Successfully synced game: {game_name} ===")
            return {"success": True, "message": f"Synced {game_name} ({files_synced} files)"}

        except subprocess.TimeoutExpired:
            sync_state["syncing"] = False
            return {"success": False, "error": "Sync timeout"}
        except Exception as e:
            sync_state["syncing"] = False
            logger.error(f"Sync error: {e}")
            return {"success": False, "error": str(e)}

    async def get_sync_status(self) -> dict:
        """Get current sync status (for progress tracking)."""
        global sync_state
        return {
            "syncing": sync_state["syncing"],
            "current_game": sync_state["current_game"],
            "current_file": sync_state["current_file"],
            "files_done": sync_state["files_done"],
            "files_total": sync_state["files_total"],
        }

