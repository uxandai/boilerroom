//! VDF (Valve Data Format) parser for config.vdf manipulation
//! This module handles adding DecryptionKey entries to Steam's config.vdf

use std::collections::HashSet;

/// Add decryption keys to config.vdf content
/// Returns the modified content with new keys added (avoiding duplicates)
pub fn add_decryption_keys_to_vdf(content: &str, depot_keys: &[(String, String)]) -> String {
    // Build set of existing depot IDs to avoid duplicates
    let existing_depots = extract_existing_depot_ids(content);

    // Filter out depots that already exist
    let new_keys: Vec<_> = depot_keys
        .iter()
        .filter(|(depot_id, _)| !existing_depots.contains(depot_id))
        .collect();

    if new_keys.is_empty() {
        eprintln!("[config_vdf] All depot keys already exist in config.vdf");
        return content.to_string();
    }

    eprintln!(
        "[config_vdf] Adding {} new depot keys to config.vdf",
        new_keys.len()
    );

    // Build the new depot entries
    let new_entries = build_depot_entries(&new_keys);

    // Find where to insert: look for "depots" section
    if let Some(insert_pos) = find_depots_insert_position(content) {
        let mut result = String::with_capacity(content.len() + new_entries.len());
        result.push_str(&content[..insert_pos]);
        result.push_str(&new_entries);
        result.push_str(&content[insert_pos..]);
        result
    } else {
        // No "depots" section found - need to add full nested structure:
        // InstallConfigStore > Software > Valve > Steam > depots
        if let Some(insert_pos) = find_steam_section_position(content) {
            // Found "Steam" section inside Software > Valve, add depots inside it
            let mut result = String::with_capacity(content.len() + new_entries.len() + 100);
            result.push_str(&content[..insert_pos]);
            result.push_str("\t\t\t\t\"depots\"\n\t\t\t\t{\n");
            result.push_str(&new_entries);
            result.push_str("\t\t\t\t}\n");
            result.push_str(&content[insert_pos..]);
            result
        } else if let Some(insert_pos) = find_install_config_store_position(content) {
            // No Steam section, create full nested structure
            let mut result = String::with_capacity(content.len() + new_entries.len() + 200);
            result.push_str(&content[..insert_pos]);
            result.push_str("\t\"Software\"\n\t{\n");
            result.push_str("\t\t\"Valve\"\n\t\t{\n");
            result.push_str("\t\t\t\"Steam\"\n\t\t\t{\n");
            result.push_str("\t\t\t\t\"depots\"\n\t\t\t\t{\n");
            result.push_str(&new_entries);
            result.push_str("\t\t\t\t}\n");
            result.push_str("\t\t\t}\n");
            result.push_str("\t\t}\n");
            result.push_str("\t}\n");
            result.push_str(&content[insert_pos..]);
            result
        } else {
            // Fallback: append at the end (shouldn't happen with valid config.vdf)
            eprintln!("[config_vdf] Warning: Could not find proper insertion point");
            content.to_string()
        }
    }
}

/// Extract depot IDs that already exist in the config.vdf
fn extract_existing_depot_ids(content: &str) -> HashSet<String> {
    let mut depots = HashSet::new();

    // Look for pattern: "depots" followed by depot ID blocks
    // Format:
    // "depots"
    // {
    //     "1234567"
    //     {

    let mut in_depots_section = false;
    let mut brace_depth = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.contains("\"depots\"") && !trimmed.starts_with("//") {
            in_depots_section = true;
            continue;
        }

        if in_depots_section {
            if trimmed == "{" {
                brace_depth += 1;
            } else if trimmed == "}" {
                brace_depth -= 1;
                if brace_depth == 0 {
                    in_depots_section = false;
                }
            } else if brace_depth == 1 {
                // At depth 1, we should see depot IDs like "1234567"
                if let Some(depot_id) = extract_quoted_string(trimmed) {
                    // Verify it's a numeric depot ID
                    if depot_id.chars().all(|c| c.is_ascii_digit()) {
                        depots.insert(depot_id);
                    }
                }
            }
        }
    }

    depots
}

/// Build VDF entries for the new depot keys
/// Uses 5 levels of indentation to match Steam's config.vdf structure:
/// InstallConfigStore > Software > Valve > Steam > depots > {depot_id}
fn build_depot_entries(keys: &[&(String, String)]) -> String {
    let mut result = String::new();

    for (depot_id, key) in keys {
        // 5 tabs = inside: InstallConfigStore > Software > Valve > Steam > depots
        result.push_str(&format!(
            "\t\t\t\t\t\"{}\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"DecryptionKey\"\t\t\"{}\"\n\t\t\t\t\t}}\n",
            depot_id, key
        ));
    }

    result
}

/// Find the position to insert new depot entries (after "depots" { )
fn find_depots_insert_position(content: &str) -> Option<usize> {
    // Find "depots" line
    let depots_pattern = "\"depots\"";
    let depots_pos = content.find(depots_pattern)?;

    // Find the opening brace after "depots"
    let after_depots = &content[depots_pos..];
    let brace_pos = after_depots.find('{')?;

    // Position is right after the opening brace and its newline
    let absolute_pos = depots_pos + brace_pos + 1;

    // Skip any whitespace/newline after the brace
    let remainder = &content[absolute_pos..];
    let skip = remainder
        .chars()
        .take_while(|c| *c == '\n' || *c == '\r')
        .count();

    Some(absolute_pos + skip)
}

/// Find position after "InstallConfigStore" { to add new "depots" section
fn find_install_config_store_position(content: &str) -> Option<usize> {
    let pattern = "\"InstallConfigStore\"";
    let pos = content.find(pattern)?;

    let after = &content[pos..];
    let brace_pos = after.find('{')?;
    let absolute_pos = pos + brace_pos + 1;

    // Skip newline
    let remainder = &content[absolute_pos..];
    let skip = remainder
        .chars()
        .take_while(|c| *c == '\n' || *c == '\r')
        .count();

    Some(absolute_pos + skip)
}

/// Find position after "Steam" section { inside Software > Valve > Steam
/// This is the correct location for the "depots" section in config.vdf
fn find_steam_section_position(content: &str) -> Option<usize> {
    // Look for "Steam" section that's inside "Valve" (not standalone)
    // The pattern in a real config.vdf is:
    // "Software" > "Valve" > "Steam" > { ... }
    let pattern = "\"Steam\"";

    // Find all occurrences and look for one that's inside Valve section
    let mut search_start = 0;
    while let Some(pos) = content[search_start..].find(pattern) {
        let absolute_pos = search_start + pos;

        // Check if this is likely inside a Valve section by looking at prior content
        let prior = &content[..absolute_pos];
        if prior.contains("\"Valve\"") && prior.contains("\"Software\"") {
            // Found Steam inside Software > Valve structure
            let after = &content[absolute_pos..];
            if let Some(brace_pos) = after.find('{') {
                let insert_pos = absolute_pos + brace_pos + 1;

                // Skip newline after brace
                let remainder = &content[insert_pos..];
                let skip = remainder
                    .chars()
                    .take_while(|c| *c == '\n' || *c == '\r')
                    .count();

                return Some(insert_pos + skip);
            }
        }

        search_start = absolute_pos + pattern.len();
    }

    None
}

/// Extract a quoted string value from a VDF line
fn extract_quoted_string(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

/// Extract depot decryption keys from config.vdf that match an app_id range
/// Steam depots are typically numbered: app_id, app_id+1, app_id+2, etc.
/// This function finds all depot keys where depot_id is within [app_id, app_id+100]
/// Returns a Vec of (depot_id, decryption_key) pairs that were found
#[allow(dead_code)]
pub fn extract_depot_keys_by_app_id(content: &str, app_id: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();

    let app_id_num: u64 = match app_id.parse() {
        Ok(n) => n,
        Err(_) => return result,
    };

    // Also include the app_id itself as a depot (main depot)
    let min_depot = app_id_num;
    let max_depot = app_id_num + 100; // Depots are typically app_id, app_id+1, app_id+2, etc.

    // Parse the depots section looking for matching depot IDs
    let mut in_depots_section = false;
    let mut brace_depth = 0;
    let mut current_depot_id: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.contains("\"depots\"") && !trimmed.starts_with("//") {
            in_depots_section = true;
            continue;
        }

        if in_depots_section {
            if trimmed == "{" {
                brace_depth += 1;
            } else if trimmed == "}" {
                brace_depth -= 1;
                if brace_depth == 1 {
                    current_depot_id = None; // Exiting a depot block
                }
                if brace_depth == 0 {
                    in_depots_section = false;
                }
            } else if brace_depth == 1 {
                // At depth 1, we should see depot IDs like "1234567"
                if let Some(depot_id) = extract_quoted_string(trimmed) {
                    if depot_id.chars().all(|c| c.is_ascii_digit()) {
                        if let Ok(depot_num) = depot_id.parse::<u64>() {
                            if depot_num >= min_depot && depot_num <= max_depot {
                                current_depot_id = Some(depot_id);
                            }
                        }
                    }
                }
            } else if brace_depth == 2 && current_depot_id.is_some() {
                // Inside a depot block, look for DecryptionKey
                if trimmed.contains("\"DecryptionKey\"") {
                    // Extract the key value - it's the second quoted string on this line
                    let parts: Vec<_> = trimmed.split('"').collect();
                    if parts.len() >= 4 {
                        let key = parts[3].to_string();
                        if !key.is_empty() {
                            result.push((current_depot_id.clone().unwrap(), key));
                        }
                    }
                }
            }
        }
    }

    eprintln!(
        "[config_vdf] Found {} depot keys in range [{}, {}] for app {}",
        result.len(),
        min_depot,
        max_depot,
        app_id
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_keys_to_existing_depots() {
        let content = r#""InstallConfigStore"
{
	"depots"
	{
		"228988"
		{
			"DecryptionKey"		"abc123"
		}
	}
}"#;

        let keys = vec![
            ("123456".to_string(), "newkey123".to_string()),
            ("789012".to_string(), "newkey456".to_string()),
        ];

        let result = add_decryption_keys_to_vdf(content, &keys);

        assert!(result.contains("\"123456\""));
        assert!(result.contains("\"newkey123\""));
        assert!(result.contains("\"789012\""));
        assert!(result.contains("\"newkey456\""));
        // Original should still be there
        assert!(result.contains("\"228988\""));
        assert!(result.contains("\"abc123\""));
    }

    #[test]
    fn test_no_duplicates() {
        let content = r#""InstallConfigStore"
{
	"depots"
	{
		"123456"
		{
			"DecryptionKey"		"existingkey"
		}
	}
}"#;

        let keys = vec![
            ("123456".to_string(), "newkey".to_string()), // Should be skipped
            ("789012".to_string(), "anotherkey".to_string()),
        ];

        let result = add_decryption_keys_to_vdf(content, &keys);

        // Original key should be preserved, not replaced
        assert!(result.contains("\"existingkey\""));
        // Only new depot should be added
        assert!(result.contains("\"789012\""));
        assert!(result.contains("\"anotherkey\""));
    }

    #[test]
    fn test_extract_existing_depot_ids() {
        let content = r#""InstallConfigStore"
{
	"depots"
	{
		"228988"
		{
			"DecryptionKey"		"abc"
		}
		"123456"
		{
			"DecryptionKey"		"def"
		}
	}
}"#;

        let depots = extract_existing_depot_ids(content);
        assert!(depots.contains("228988"));
        assert!(depots.contains("123456"));
        assert_eq!(depots.len(), 2);
    }
}
