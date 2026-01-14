//! Steam CM Protocol Achievement Schema Fetcher
//! 
//! Implements the SLScheevo method: fetches raw achievement schema bytes
//! directly from Steam's CM servers using the ClientGetUserStats message.

use std::path::PathBuf;
use std::io::Write;

/// Top owner IDs - Steam accounts with large game libraries used to fetch schemas
/// These are public profiles that own many games (same as SLScheevo)
#[allow(dead_code)] // Used when CM implementation is complete
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
];

/// Result type for CM-based achievement generation
#[derive(Debug, Clone, serde::Serialize)]
pub struct CmAchievementResult {
    pub success: bool,
    pub message: String,
    pub schema_size: usize,
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

/// Generate achievement schema using Steam CM protocol (SLScheevo method)
/// 
/// This connects to Steam using the provided refresh token and fetches
/// the raw binary schema directly from Steam's servers.
pub async fn generate_achievement_schema_cm(
    app_id: &str,
    steam_user_id: &str,
    _refresh_token: &str,
) -> Result<CmAchievementResult, String> {
    // Note: steam-vent's refresh token login requires more complex setup
    // For now, we'll use a simplified approach that works with the existing infrastructure
    
    // Parse app_id as u32
    let game_id: u32 = app_id.parse()
        .map_err(|_| format!("Invalid app ID: {}", app_id))?;
    
    // Try to fetch schema from Steam servers
    let schema_result = fetch_schema_from_steam(game_id).await?;
    
    match schema_result {
        Some(schema_bytes) => {
            // Write schema file
            let output_dir = get_schema_output_dir()?;
            let schema_path = output_dir.join(format!("UserGameStatsSchema_{}.bin", app_id));
            
            let mut file = std::fs::File::create(&schema_path)
                .map_err(|e| format!("Failed to create schema file: {}", e))?;
            file.write_all(&schema_bytes)
                .map_err(|e| format!("Failed to write schema file: {}", e))?;
            
            // Create empty stats file if it doesn't exist
            let stats_path = output_dir.join(format!("UserGameStats_{}_{}.bin", steam_user_id, app_id));
            if !stats_path.exists() {
                let empty_stats = create_empty_stats_file();
                std::fs::write(&stats_path, &empty_stats)
                    .map_err(|e| format!("Failed to create stats file: {}", e))?;
            }
            
            Ok(CmAchievementResult {
                success: true,
                message: format!("Schema saved ({} bytes)", schema_bytes.len()),
                schema_size: schema_bytes.len(),
            })
        }
        None => {
            Ok(CmAchievementResult {
                success: false,
                message: "No schema found for this game".to_string(),
                schema_size: 0,
            })
        }
    }
}

/// Fetch schema from Steam servers using CM protocol
/// 
/// NOTE: Full implementation pending - steam-vent doesn't expose raw ClientGetUserStats message 
/// as a high-level API. The library focuses on RPC-style service methods.
/// 
/// Current status: Returns an informative error message directing users to use Web API method.
async fn fetch_schema_from_steam(_game_id: u32) -> Result<Option<Vec<u8>>, String> {
    // steam-vent's Connection provides:
    // - anonymous() - anonymous session
    // - access(account, token) - authenticated session with refresh token
    // - service_method() - for RPC-style calls like GetServerList
    // 
    // However, ClientGetUserStats is a raw protocol message (EMsg), not a service method.
    // To implement this properly, we would need to:
    // 1. Use steam-vent-proto to construct CMsgClientGetUserStats
    // 2. Send it as a raw message via the connection
    // 3. Filter responses for CMsgClientGetUserStatsResponse
    // 4. Extract the schema bytes
    //
    // This requires deeper integration with steam-vent's internals.
    // For now, we return an error guiding users to the Web API method.
    
    Err(
        "Steam CM (SLScheevo) method is not yet fully implemented. \
         The steam-vent library doesn't expose ClientGetUserStats as a service method. \
         Please use the Web API (SLSah) method instead, which is fully functional.".to_string()
    )
}

/// Create an empty stats file template
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
}
