use log::{info, warn};
use serde::Deserialize;
use std::path::PathBuf;

const API_BASE_URL: &str = "https://www.pcgamingwiki.com/w/api.php";

#[derive(Debug, Deserialize)]
struct CargoResponse {
    cargoquery: Vec<CargoEntry>,
}

#[derive(Debug, Deserialize)]
struct CargoEntry {
    title: CargoTitle,
}

#[derive(Debug, Deserialize)]
struct CargoTitle {
    #[serde(rename = "PageName")]
    page_name: String,
}

#[derive(Debug, Deserialize)]
struct ParseSectionResponse {
    parse: ParseSections,
}

#[derive(Debug, Deserialize)]
struct ParseSections {
    sections: Vec<WikiSection>,
}

#[derive(Debug, Deserialize)]
struct WikiSection {
    line: String,
    index: String,
}

#[derive(Debug, Deserialize)]
struct ParseWikitextResponse {
    parse: ParseWikitext,
}

#[derive(Debug, Deserialize)]
struct ParseWikitext {
    wikitext: WikitextContent,
}

#[derive(Debug, Deserialize)]
struct WikitextContent {
    #[serde(rename = "*")]
    content: String,
}

/// Entry point to find save locations for a Steam AppID
pub async fn find_save_locations(app_id: &str, steam_user_id: Option<&str>) -> Result<Vec<PathBuf>, String> {
    // 1. Find PageName
    let page_name = get_page_name(app_id).await?;
    info!("PCGamingWiki: Found page '{}' for AppID {}", page_name, app_id);

    // 2. Find Save Data Section
    let section_idx = get_save_data_section_index(&page_name).await?;
    
    // 3. Get Wikitext
    let wikitext = get_section_wikitext(&page_name, &section_idx).await?;
    
    // 4. Parse & Resolve Paths
    let paths = parse_and_resolve_paths(&wikitext, app_id, steam_user_id);
    
    Ok(paths)
}

async fn get_page_name(app_id: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client.get(API_BASE_URL)
        .query(&[
            ("action", "cargoquery"),
            ("tables", "Infobox_game"),
            ("fields", "_pageName=PageName"),
            ("where", &format!("Infobox_game.Steam_AppID HOLDS '{}'", app_id)),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let data: CargoResponse = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    
    data.cargoquery.first()
        .map(|entry| entry.title.page_name.clone())
        .ok_or_else(|| "Game not found on PCGamingWiki".to_string())
}

async fn get_save_data_section_index(page_name: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client.get(API_BASE_URL)
        .query(&[
            ("action", "parse"),
            ("page", page_name),
            ("prop", "sections"),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let data: ParseSectionResponse = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    
    // Look for "Save game data location" or fallback to "Game data"
    for section in &data.parse.sections {
        if section.line.to_lowercase().contains("save game data location") {
            return Ok(section.index.clone());
        }
    }
    
    // Fallback pass
    for section in &data.parse.sections {
         if section.line.to_lowercase() == "game data" {
            return Ok(section.index.clone());
        }
    }

    Err("Save data section not found".to_string())
}

async fn get_section_wikitext(page_name: &str, section_index: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client.get(API_BASE_URL)
        .query(&[
            ("action", "parse"),
            ("page", page_name),
            ("section", section_index),
            ("prop", "wikitext"),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let data: ParseWikitextResponse = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    Ok(data.parse.wikitext.content)
}

use regex::Regex;

fn parse_and_resolve_paths(wikitext: &str, app_id: &str, steam_user_id: Option<&str>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    
    // Regex to find Game data/saves templates
    // Matches: {{Game data/saves|Type|Path|...}}
    // note: PCGamingWiki templates can be messy. We look for pipe-separated values.
    let re = Regex::new(r"\{\{Game data/saves\|[^|]*\|([^}|]+)").unwrap();
    
    // Also look for straightforward Windows paths if not in template, but that's riskier.
    // Let's stick to the template for now.
    
    for cap in re.captures_iter(wikitext) {
        if let Some(raw_path_match) = cap.get(1) {
            let raw_path = raw_path_match.as_str().trim();
            if !raw_path.is_empty() {
                if let Some(resolved) = resolve_windows_path_to_linux(raw_path, app_id, steam_user_id) {
                    paths.push(resolved);
                }
            }
        }
    }

    // Also handle second/third args in the template if they exist (some games have multiple paths)
    // A more robust parser would tokenise the template, but let's iterate on lines for now as wikicode often breaks lines
    
    paths
}

fn resolve_windows_path_to_linux(win_path: &str, app_id: &str, steam_user_id: Option<&str>) -> Option<PathBuf> {
    let mut path_str = win_path.to_string();

    // 1. Replace Template Variables
    // {{p|game}} -> install dir (hard to know exactly, but usually we can find it via library.rs logic if we had access)
    // For now, let's focus on Proton prefix paths
    
    // {{p|uid}} -> steam user id
    if let Some(uid) = steam_user_id {
        path_str = path_str.replace("{{p|uid}}", uid);
    }
    
    // {{p|steam}} -> ~/.local/share/Steam
    let steam_root = if cfg!(target_os = "macos") {
         match dirs::home_dir() {
             Some(h) => h.join("Library/Application Support/Steam"),
             None => return None,
         }
    } else {
         match dirs::home_dir() {
             Some(h) => h.join(".local/share/Steam"),
             None => return None,
         }
    };
    
    path_str = path_str.replace("{{p|steam}}", &steam_root.to_string_lossy());
    path_str = path_str.replace("{{P|steam}}", &steam_root.to_string_lossy());

    // Proton Prefix Base: ~/.local/share/Steam/steamapps/compatdata/[APPID]/pfx/drive_c/
    let compat_data = steam_root.join("steamapps/compatdata").join(app_id).join("pfx/drive_c");
    
    // User Profile Base: .../users/steamuser/
    let user_profile = compat_data.join("users/steamuser");

    // Replace common variables
    if path_str.contains("{{p|user}}") || path_str.contains("{{P|user}}") {
        path_str = path_str.replace("{{p|user}}", &user_profile.to_string_lossy())
                           .replace("{{P|user}}", &user_profile.to_string_lossy());
    }
    
    // AppData
    let appdata = user_profile.join("AppData/Roaming");
    if path_str.contains("{{p|appdata}}") || path_str.contains("{{P|appdata}}") {
        path_str = path_str.replace("{{p|appdata}}", &appdata.to_string_lossy())
                           .replace("{{P|appdata}}", &appdata.to_string_lossy());
    }

    // LocalAppData
    let localappdata = user_profile.join("AppData/Local");
    if path_str.contains("{{p|localappdata}}") || path_str.contains("{{P|localappdata}}") {
        path_str = path_str.replace("{{p|localappdata}}", &localappdata.to_string_lossy())
                           .replace("{{P|localappdata}}", &localappdata.to_string_lossy());
    }
    
    // Saved Games
    let saved_games = user_profile.join("Saved Games");
     if path_str.contains("{{p|userprofile}}\\Saved Games") { // Common raw string
         path_str = path_str.replace("{{p|userprofile}}\\Saved Games", &saved_games.to_string_lossy());
    }

    // Documents
    let documents = user_profile.join("Documents");
     if path_str.contains("{{p|userprofile}}\\Documents") {
         path_str = path_str.replace("{{p|userprofile}}\\Documents", &documents.to_string_lossy());
    }

    
    // Normalize slashes: Linux uses forward slash
    let normalized = path_str.replace("\\", "/");
    
    // If it looks like a relative path starting with game dir {{p|game}}, we might need the install dir.
    // If we can't resolve {{p|game}}, we might return None or try to guess.
    if normalized.contains("{{p|game}}") || normalized.contains("{{P|game}}") {
        // We need the actual game install path here.
        // For now, let's log and skip, or we need to pass the install path to this function.
        warn!("Skipping path with unresolved {{p|game}}: {}", normalized);
        return None;
    }

    Some(PathBuf::from(normalized))
}
