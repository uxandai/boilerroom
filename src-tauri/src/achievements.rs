//! Achievement generation module
//! Handles Steam Web API calls and binary VDF schema file generation
//! Based on SLSah reference implementation

use serde::Deserialize;
use std::io::Write;
use std::path::PathBuf;

// ============================================================================
// Binary VDF Writer (Valve Data Format)
// ============================================================================

/// Binary VDF type constants
mod binary_vdf {
    pub const TYPE_MAP: u8 = 0x00; // Start of map/object
    pub const TYPE_STRING: u8 = 0x01; // Null-terminated string value
    pub const TYPE_INT: u8 = 0x02; // 32-bit little-endian integer
    pub const TYPE_END: u8 = 0x08; // End of map marker
}

/// Write a binary VDF document to bytes
pub struct BinaryVdfWriter {
    buffer: Vec<u8>,
}

impl BinaryVdfWriter {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Start a new map/object with given key name
    pub fn start_map(&mut self, key: &str) {
        self.buffer.push(binary_vdf::TYPE_MAP);
        self.write_null_string(key);
    }

    /// End the current map/object
    pub fn end_map(&mut self) {
        self.buffer.push(binary_vdf::TYPE_END);
    }

    /// Write a string key-value pair
    pub fn write_string(&mut self, key: &str, value: &str) {
        self.buffer.push(binary_vdf::TYPE_STRING);
        self.write_null_string(key);
        self.write_null_string(value);
    }

    /// Write an integer key-value pair
    pub fn write_int(&mut self, key: &str, value: i32) {
        self.buffer.push(binary_vdf::TYPE_INT);
        self.write_null_string(key);
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    /// Helper: Write null-terminated string
    fn write_null_string(&mut self, s: &str) {
        self.buffer.extend_from_slice(s.as_bytes());
        self.buffer.push(0x00);
    }

    /// Finalize and return the binary data
    pub fn finish(mut self) -> Vec<u8> {
        self.buffer.push(binary_vdf::TYPE_END); // Final end marker
        self.buffer
    }
}

// ============================================================================
// Steam Web API Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SteamSchemaResponse {
    pub game: Option<GameSchema>,
}

#[derive(Debug, Deserialize)]
pub struct GameSchema {
    #[serde(rename = "gameName")]
    pub game_name: String,
    #[serde(rename = "gameVersion")]
    pub game_version: String,
    #[serde(rename = "availableGameStats")]
    pub available_game_stats: Option<AvailableGameStats>,
}

#[derive(Debug, Deserialize)]
pub struct AvailableGameStats {
    pub achievements: Option<Vec<AchievementDef>>,
}

#[derive(Debug, Deserialize)]
pub struct AchievementDef {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    pub hidden: i32,
    pub icon: String,
    pub icongray: String,
}

// ============================================================================
// Public API
// ============================================================================

/// Result type for achievement generation
#[derive(Debug, Clone, serde::Serialize)]
pub struct AchievementResult {
    pub success: bool,
    pub message: String,
    pub achievements_count: usize,
}

/// Result type for batch generation
#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchAchievementResult {
    pub processed: usize,
    pub skipped: usize,
    pub errors: usize,
    pub messages: Vec<String>,
}

/// Get the output directory for achievement schemas
pub fn get_schema_output_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    // Try ~/.steam/steam first, fall back to ~/.local/share/Steam
    let primary = home.join(".steam/steam/appcache/stats");
    let fallback = home.join(".local/share/Steam/appcache/stats");

    if primary.exists() {
        Ok(primary)
    } else if fallback.exists() {
        Ok(fallback)
    } else {
        // Create primary path
        std::fs::create_dir_all(&primary)
            .map_err(|e| format!("Failed to create stats dir: {}", e))?;
        Ok(primary)
    }
}

/// Generate achievement schema for a single app
pub async fn generate_achievement_schema(
    app_id: &str,
    steam_api_key: &str,
    steam_user_id: &str,
    language: &str,
) -> Result<AchievementResult, String> {
    // 1. Fetch schema from Steam Web API
    let url = format!(
        "https://api.steampowered.com/ISteamUserStats/GetSchemaForGame/v2/?key={}&appid={}&l={}",
        steam_api_key, app_id, language
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Steam API request failed: {}", e))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Steam API Key is invalid or unauthorized".to_string());
    }

    let schema: SteamSchemaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Steam API response: {}", e))?;

    let game = schema.game.ok_or("No game data in Steam API response")?;

    let achievements = game
        .available_game_stats
        .and_then(|stats| stats.achievements)
        .unwrap_or_default();

    if achievements.is_empty() {
        return Ok(AchievementResult {
            success: true,
            message: format!("{} has no achievements", game.game_name),
            achievements_count: 0,
        });
    }

    // 2. Build binary VDF schema
    let vdf_data = build_achievement_vdf(
        app_id,
        &game.game_name,
        &game.game_version,
        &achievements,
        language,
    );

    // 3. Write schema file
    let output_dir = get_schema_output_dir()?;
    let schema_path = output_dir.join(format!("UserGameStatsSchema_{}.bin", app_id));

    let mut file = std::fs::File::create(&schema_path)
        .map_err(|e| format!("Failed to create schema file: {}", e))?;
    file.write_all(&vdf_data)
        .map_err(|e| format!("Failed to write schema file: {}", e))?;

    // 4. Create empty stats file if it doesn't exist
    let stats_path = output_dir.join(format!("UserGameStats_{}_{}.bin", steam_user_id, app_id));
    if !stats_path.exists() {
        let empty_stats = create_empty_stats_file();
        std::fs::write(&stats_path, &empty_stats)
            .map_err(|e| format!("Failed to create stats file: {}", e))?;
    }

    Ok(AchievementResult {
        success: true,
        message: format!(
            "Generated {} achievements for {}",
            achievements.len(),
            game.game_name
        ),
        achievements_count: achievements.len(),
    })
}

/// Build the binary VDF achievement schema
fn build_achievement_vdf(
    app_id: &str,
    game_name: &str,
    game_version: &str,
    achievements: &[AchievementDef],
    language: &str,
) -> Vec<u8> {
    let mut writer = BinaryVdfWriter::new();

    // Root map with app_id as key
    writer.start_map(app_id);

    writer.write_string("gamename", game_name);
    writer.write_string("version", game_version);

    // Stats section
    writer.start_map("stats");

    // Group achievements into blocks of 32 (matching SLSah format)
    for (i, ach) in achievements.iter().enumerate() {
        let block_id = (i / 32) + 1;
        let bit_id = i % 32;
        let block_key = block_id.to_string();
        let bit_key = bit_id.to_string();

        // Start block if this is first achievement in block
        if bit_id == 0 {
            writer.start_map(&block_key);
            writer.write_string("type", "4");
            writer.write_string("id", &block_key);
            writer.start_map("bits");
        }

        // Write achievement
        writer.start_map(&bit_key);
        writer.write_string("name", &ach.name);
        writer.write_int("bit", bit_id as i32);

        // Display info
        writer.start_map("display");

        // Name with language
        writer.start_map("name");
        writer.write_string(language, &ach.display_name);
        writer.write_string(
            "token",
            &format!("NEW_ACHIEVEMENT_{}_{}_NAME", block_id, bit_id),
        );
        writer.end_map(); // name

        // Description with language
        writer.start_map("desc");
        writer.write_string(language, &ach.description);
        writer.write_string(
            "token",
            &format!("NEW_ACHIEVEMENT_{}_{}_DESC", block_id, bit_id),
        );
        writer.end_map(); // desc

        writer.write_string("hidden", &ach.hidden.to_string());

        // Extract icon filename from URL
        let icon = ach.icon.split('/').last().unwrap_or(&ach.icon);
        let icongray = ach.icongray.split('/').last().unwrap_or(&ach.icongray);
        writer.write_string("icon", icon);
        writer.write_string("icon_gray", icongray);

        writer.end_map(); // display
        writer.end_map(); // bit achievement

        // End block if this is last achievement in block or last achievement overall
        if bit_id == 31 || i == achievements.len() - 1 {
            writer.end_map(); // bits
            writer.end_map(); // block
        }
    }

    writer.end_map(); // stats
    writer.end_map(); // app_id root

    writer.finish()
}

/// Create an empty stats file template
fn create_empty_stats_file() -> Vec<u8> {
    // Minimal binary VDF stats file
    // Based on SLSah template: UserGameStats_steamid_appid.bin
    let mut data = Vec::new();

    // Empty map structure
    data.push(binary_vdf::TYPE_MAP);
    data.extend_from_slice(b"UserGameStats\0");
    data.push(binary_vdf::TYPE_END);
    data.push(binary_vdf::TYPE_END);

    data
}

/// Read AdditionalApps from SLSsteam config
pub fn read_additional_apps() -> Result<Vec<String>, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_path = home.join(".config/SLSsteam/config.yaml");

    if !config_path.exists() {
        return Err("SLSsteam config not found".to_string());
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;

    let yaml: serde_yaml::Value =
        serde_yaml::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;

    let apps = yaml
        .get("AdditionalApps")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| {
                    v.as_i64()
                        .map(|n| n.to_string())
                        .or_else(|| v.as_str().map(|s| s.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(apps)
}

/// Check if schema file exists for an app
pub fn schema_exists(app_id: &str) -> bool {
    if let Ok(output_dir) = get_schema_output_dir() {
        let schema_path = output_dir.join(format!("UserGameStatsSchema_{}.bin", app_id));
        schema_path.exists()
    } else {
        false
    }
}
