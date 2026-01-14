//! Steam CM Protocol Achievement Schema Fetcher
//! 
//! Implements the SLScheevo method: fetches raw achievement schema bytes
//! directly from Steam's CM servers using the ClientGetUserStats message.
//!
//! This is a native Rust implementation based on:
//! https://github.com/xamionex/SLScheevo

use std::path::PathBuf;
use std::io::Write;
use std::time::Duration;

use steam_vent::{Connection, ServerList, ConnectionTrait};
use steam_vent::auth::{DeviceConfirmationHandler, FileGuardDataStore};
use steam_vent::proto::steammessages_clientserver_userstats::{
    CMsgClientGetUserStats, CMsgClientGetUserStatsResponse,
};

/// Top owner IDs - Steam accounts with large game libraries used to fetch schemas
const TOP_OWNER_IDS: &[u64] = &[
    76561198028121353, 76561197979911851, 76561198017975643, 76561197993544755,
    76561198355953202, 76561198001237877, 76561198237402290, 76561198152618007,
    76561198355625888, 76561198213148949, 76561197969050296, 76561198217186687,
    76561198037867621, 76561198094227663, 76561198019712127, 76561197963550511,
    76561198134044398, 76561198001678750, 76561197973009892, 76561198044596404,
    76561197976597747, 76561197969810632, 76561198095049646, 76561198085065107,
    76561198864213876, 76561197962473290, 76561198388522904, 76561198033715344,
    76561197995070100, 76561198313790296, 76561198063574735, 76561197996432822,
    76561197976968076, 76561198281128349, 76561198154462478, 76561198027233260,
    76561198842864763, 76561198010615256, 76561198035900006, 76561198122859224,
    76561198235911884, 76561198027214426, 76561197970825215, 76561197968410781,
    76561198104323854, 76561198001221571, 76561198256917957, 76561198008181611,
    76561198407953371, 76561198062901118,
];

/// Maximum consecutive "no schema" responses before giving up
const MAX_NO_SCHEMA_STREAK: usize = 5;

/// Result type for CM-based achievement generation
#[derive(Debug, Clone, serde::Serialize)]
pub struct CmAchievementResult {
    pub success: bool,
    pub message: String,
    pub schema_size: usize,
}

/// Get the data directory for storing Steam guard tokens
fn get_steam_guard_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let guard_dir = home.join(".cache/boilerroom/steam_guard");
    std::fs::create_dir_all(&guard_dir)
        .map_err(|e| format!("Failed to create guard dir: {}", e))?;
    Ok(guard_dir)
}

/// Get the output directory for achievement schemas  
fn get_schema_output_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let primary = home.join(".steam/steam/appcache/stats");
    let fallback = home.join(".local/share/Steam/appcache/stats");

    if primary.exists() {
        Ok(primary)
    } else if fallback.exists() {
        Ok(fallback)
    } else {
        std::fs::create_dir_all(&primary)
            .map_err(|e| format!("Failed to create stats dir: {}", e))?;
        Ok(primary)
    }
}

/// Login to Steam with username/password
/// Uses DeviceConfirmationHandler - user must approve on mobile app
pub async fn steam_login(
    username: &str,
    password: &str,
) -> Result<Connection, String> {
    eprintln!("[Steam CM] Discovering Steam servers...");
    let server_list = ServerList::discover().await
        .map_err(|e| format!("Failed to discover Steam servers: {}", e))?;
    
    let guard_dir = get_steam_guard_dir()?;
    let guard_store = FileGuardDataStore::new(guard_dir);
    
    eprintln!("[Steam CM] Logging in as {}... (approve on Steam mobile app)", username);
    
    // Use DeviceConfirmationHandler - waits for mobile app approval
    let connection = Connection::login(
        &server_list,
        username,
        password,
        guard_store,
        DeviceConfirmationHandler,
    ).await.map_err(|e| format!("Login failed: {}. Check credentials or approve on mobile app.", e))?;
    
    eprintln!("[Steam CM] Login successful!");
    Ok(connection)
}

/// Generate achievement schema using Steam CM protocol (SLScheevo method)
/// 
/// Requires valid Steam connection (must be logged in)
pub async fn generate_achievement_schema_cm(
    app_id: &str,
    steam_user_id: &str,
) -> Result<CmAchievementResult, String> {
    // For now, return error - login flow needs to be called first
    // In future: use stored refresh token to auto-reconnect
    
    let game_id: u64 = app_id.parse()
        .map_err(|_| format!("Invalid app ID: {}", app_id))?;
    
    eprintln!("[Steam CM] Schema request for game {} (user: {})", game_id, steam_user_id);
    
    Err(
        "Steam CM method requires login. Use 'Steam Login' in settings first, \
         then the connection can be used to fetch schemas.".to_string()
    )
}

/// Fetch schema with an existing connection
pub async fn fetch_schema_with_connection(
    connection: &Connection,
    app_id: &str,
) -> Result<CmAchievementResult, String> {
    let game_id: u64 = app_id.parse()
        .map_err(|_| format!("Invalid app ID: {}", app_id))?;
    
    eprintln!("[Steam CM] Fetching schema for game {} from {} owners...", game_id, TOP_OWNER_IDS.len());
    
    let mut no_schema_streak = 0;
    
    // Try each owner ID until we find one with the schema
    for (i, &owner_id) in TOP_OWNER_IDS.iter().enumerate() {
        match try_get_schema(connection, game_id, owner_id).await {
            Ok(SchemaResult::Found(schema)) => {
                eprintln!("[Steam CM] Found schema from owner {} ({}/{}) - {} bytes", 
                    owner_id, i + 1, TOP_OWNER_IDS.len(), schema.len());
                
                // Write schema file only - stats file is created separately by Steam
                let output_dir = get_schema_output_dir()?;
                let schema_path = output_dir.join(format!("UserGameStatsSchema_{}.bin", app_id));
                
                let mut file = std::fs::File::create(&schema_path)
                    .map_err(|e| format!("Failed to create schema file: {}", e))?;
                file.write_all(&schema)
                    .map_err(|e| format!("Failed to write schema file: {}", e))?;
                
                return Ok(CmAchievementResult {
                    success: true,
                    message: format!("Schema saved ({} bytes)", schema.len()),
                    schema_size: schema.len(),
                });
            }
            Ok(SchemaResult::NoSchema) => {
                no_schema_streak += 1;
                if no_schema_streak >= MAX_NO_SCHEMA_STREAK {
                    eprintln!("[Steam CM] {} consecutive 'no schema' - game likely has no achievements", no_schema_streak);
                    return Ok(CmAchievementResult {
                        success: false,
                        message: "No schema found (game may not have achievements)".to_string(),
                        schema_size: 0,
                    });
                }
            }
            Ok(SchemaResult::NotFound) => {
                no_schema_streak = 0;
            }
            Err(e) => {
                eprintln!("[Steam CM] Error checking owner {}: {}", owner_id, e);
                no_schema_streak = 0;
            }
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    Ok(CmAchievementResult {
        success: false,
        message: "No schema found after checking all owners".to_string(),
        schema_size: 0,
    })
}

/// Result of trying to get schema from a single owner
enum SchemaResult {
    Found(Vec<u8>),
    NoSchema,
    NotFound,
}

/// Try to get schema from a specific owner
async fn try_get_schema(
    connection: &Connection,
    game_id: u64,
    owner_id: u64,
) -> Result<SchemaResult, String> {
    let mut request = CMsgClientGetUserStats::new();
    request.set_game_id(game_id);
    request.set_schema_local_version(-1);
    request.set_crc_stats(0);
    request.set_steam_id_for_user(owner_id);
    
    let response: CMsgClientGetUserStatsResponse = tokio::time::timeout(
        Duration::from_secs(5),
        connection.job(request)
    )
    .await
    .map_err(|_| "Request timed out")?
    .map_err(|e| format!("Network error: {}", e))?;
    
    if response.has_schema() && !response.schema().is_empty() {
        return Ok(SchemaResult::Found(response.schema().to_vec()));
    }
    
    if response.eresult() == 2 && response.crc_stats() == 0 {
        return Ok(SchemaResult::NoSchema);
    }
    
    Ok(SchemaResult::NotFound)
}

/// Create an empty stats file template (binary VDF format)
fn create_empty_stats_file() -> Vec<u8> {
    let mut data = Vec::new();
    data.push(0x00); // TYPE_MAP 
    data.extend_from_slice(b"UserGameStats\0");
    data.push(0x08); // TYPE_END
    data.push(0x08); // TYPE_END
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_owner_ids_count() {
        assert!(TOP_OWNER_IDS.len() >= 30);
    }
    
    #[test]
    fn test_empty_stats_file() {
        let stats = create_empty_stats_file();
        assert!(!stats.is_empty());
        assert!(stats.len() > 10);
    }
}
