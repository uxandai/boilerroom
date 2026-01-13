use crate::commands::SshConfig;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::io::{BufRead, BufReader, Write};
use regex::Regex;

use tauri::{AppHandle, Emitter};
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Clone, Serialize, Debug)]
pub struct InstallProgress {
    pub state: String, // "downloading", "steamless", "transferring", "configuring", "finished", "error", "cancelled"
    pub message: String,
    pub download_percent: f64,
    pub download_speed: String,  // e.g. "12.5 MB/s"
    pub eta: String,             // e.g. "2m 30s" or "calculating..."
    pub files_total: usize,
    pub files_transferred: usize,
    pub bytes_total: u64,
    pub bytes_transferred: u64,
    pub transfer_speed: String, // e.g. "45.2 MB/s"
}

#[derive(Clone, Serialize, Debug)]
pub struct DepotDownloadArg {
    pub depot_id: String,
    pub manifest_id: String,
    pub manifest_file: String,
}

#[derive(Clone)]
pub struct InstallManager {
    app_handle: AppHandle,
    progress: Arc<Mutex<InstallProgress>>,
    cancelled: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    child_process: Arc<Mutex<Option<u32>>>, // Store child PID for killing
    last_emit: Arc<Mutex<std::time::Instant>>, // Throttling
}

impl InstallManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            progress: Arc::new(Mutex::new(InstallProgress {
                state: "idle".to_string(),
                message: "Waiting to start...".to_string(),
                download_percent: 0.0,
                download_speed: String::new(),
                eta: String::new(),
                files_total: 0,
                files_transferred: 0,
                bytes_total: 0,
                bytes_transferred: 0,
                transfer_speed: String::new(),
            })),
            cancelled: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            child_process: Arc::new(Mutex::new(None)),
            last_emit: Arc::new(Mutex::new(std::time::Instant::now())),
        }
    }
    
    /// Reset state for new installation
    pub fn reset(&self) {
        eprintln!("[InstallManager] Resetting state for new installation");
        self.cancelled.store(false, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        *self.child_process.lock().unwrap() = None;
        
        let mut p = self.progress.lock().unwrap();
        p.state = "idle".to_string();
        p.message = "Starting...".to_string();
        p.download_percent = 0.0;
        p.download_speed = String::new();
        p.eta = "calculating...".to_string();
        p.files_total = 0;
        p.files_transferred = 0;
        p.bytes_total = 0;
        p.bytes_transferred = 0;
        p.transfer_speed = String::new();
    }
    
    /// Pause the current installation
    pub fn pause(&self) {
        eprintln!("[InstallManager] Pause requested!");
        self.paused.store(true, Ordering::SeqCst);
        
        // Kill current download process - it will be resumed with -resume flag
        if let Some(pid) = *self.child_process.lock().unwrap() {
            eprintln!("[InstallManager] Stopping child process PID: {} for pause", pid);
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .args([&pid.to_string()])
                    .status();
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string()])
                    .status();
            }
        }
        
        self.update_status("paused", "Installation paused");
    }
    
    /// Resume the paused installation
    pub fn resume(&self) {
        eprintln!("[InstallManager] Resume requested!");
        self.paused.store(false, Ordering::SeqCst);
        self.update_status("downloading", "Resuming download...");
    }
    
    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
    
    pub fn cancel(&self) {
        eprintln!("[InstallManager] Cancel requested!");
        self.cancelled.store(true, Ordering::SeqCst);
        
        // Try to kill active child process - graceful first, then force
        if let Some(pid) = *self.child_process.lock().unwrap() {
            eprintln!("[InstallManager] Killing child process PID: {} (graceful)", pid);
            
            #[cfg(unix)]
            {
                // First try SIGTERM (graceful)
                let _ = std::process::Command::new("kill")
                    .args([&pid.to_string()])
                    .status();
                
                // Wait a bit for graceful shutdown
                std::thread::sleep(std::time::Duration::from_millis(500));
                
                // Then SIGKILL if still running
                let _ = std::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .status();
                eprintln!("[InstallManager] Child process killed (SIGKILL sent)");
            }
            
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .status();
                eprintln!("[InstallManager] Child process killed (taskkill)");
            }
        }
        
        self.update_status("cancelled", "Installation cancelled by user");
    }
    
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
    
    fn set_child_pid(&self, pid: Option<u32>) {
        *self.child_process.lock().unwrap() = pid;
    }

    fn emit_progress(&self, force: bool) {
        // Throttling: only emit if enough time passed or forced (state change)
        if !force {
            let mut last = self.last_emit.lock().unwrap();
            if last.elapsed() < std::time::Duration::from_millis(100) {
                return;
            }
            *last = std::time::Instant::now();
        } else {
            // Update timestamp even if forced, to keep spacing
            let mut last = self.last_emit.lock().unwrap();
            *last = std::time::Instant::now();
        }

        let progress = self.progress.lock().unwrap().clone();
        let _ = self.app_handle.emit("install-progress", progress);
    }

    fn update_status(&self, state: &str, msg: &str) {
        let mut p = self.progress.lock().unwrap();
        p.state = state.to_string();
        p.message = msg.to_string();
        drop(p);
        self.emit_progress(true); // Always force state changes
    }

    fn update_download_percent(&self, percent: f64) {
        let mut p = self.progress.lock().unwrap();
        p.download_percent = percent;
        drop(p);
        self.emit_progress(false); // Throttle progress updates
    }
    
    fn update_download_speed_eta(&self, speed: &str, eta: &str) {
        let mut p = self.progress.lock().unwrap();
        p.download_speed = speed.to_string();
        p.eta = eta.to_string();
        drop(p);
        self.emit_progress(false); // Throttle speed/eta updates
    }

    fn update_transfer_progress(&self, files_done: usize, files_total: usize, speed: &str) {
        let mut p = self.progress.lock().unwrap();
        p.files_transferred = files_done;
        p.files_total = files_total;
        p.transfer_speed = speed.to_string();
        // Scale transfer progress to 50-100% (download was 0-50%)
        if files_total > 0 {
            let transfer_pct = (files_done as f64 / files_total as f64) * 50.0;
            p.download_percent = 50.0 + transfer_pct;
        }
        drop(p);
        self.emit_progress(false); // Throttle transfer updates
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_pipeline(
        &self,
        app_id: String,
        game_name: String,
        depots: Vec<DepotDownloadArg>,
        keys_file: PathBuf,
        depot_keys: Vec<(String, String)>,
        depot_downloader_path: String,
        _steamless_path: String,
        ssh_config: SshConfig,
        target_directory: String,
        app_token: Option<String>, // Optional app token from LUA addtoken()
    ) -> Result<(), String> {
        // Reset state before starting new installation
        self.reset();
        
        let is_local = ssh_config.is_local;
        
        // Sanitize game name for folder - remove special chars
        let folder_name: String = game_name.chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
            .collect();
        let folder_name = folder_name.trim().to_string();
        let folder_name = if folder_name.is_empty() { app_id.clone() } else { folder_name };
        
        // For LOCAL mode: download directly to Steam library
        // For REMOTE mode: download to temp, then rsync
        let download_dir = if is_local {
            let path = PathBuf::from(&target_directory).join(&folder_name);
            std::fs::create_dir_all(&path).map_err(|e| format!("Failed to create game directory: {}", e))?;
            path
        } else {
            // Use /var/tmp instead of /tmp - /var/tmp is always persistent disk, not tmpfs RAM
            let temp = PathBuf::from("/var/tmp").join(format!("boilerroom_install_{}", app_id));
            if temp.exists() {
                std::fs::remove_dir_all(&temp).map_err(|e| e.to_string())?;
            }
            std::fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
            temp
        };

        let m = self.clone();
        let app_id_clone = app_id.clone();
        let game_name_clone = game_name.clone();
        let folder_name_clone = folder_name.clone();
        let target_dir = target_directory.clone();
        let download_dir_clone = download_dir.clone();
        let depot_keys_clone = depot_keys.clone();
        let app_token_clone = app_token.clone();

        thread::spawn(move || {
            // ========================================
            // PHASE 1: DOWNLOAD (DepotDownloaderMod)
            // ========================================
            m.update_status("downloading", if is_local { "Downloading to Steam library..." } else { "Downloading to temp..." });
            
            // Regex to match various percentage formats: 50%, 50.00%, 50.5%
            let percent_re = Regex::new(r"(\d{1,3}(?:\.\d{1,2})?)%").unwrap();
            let total_depots = depots.len();
            
            let speed_re = Regex::new(r"(\d+\.?\d*)\s*(KB|MB|GB)/s").unwrap();
            for (depot_idx, depot) in depots.iter().enumerate() {
                // Check for cancellation before each depot
                if m.is_cancelled() {
                    eprintln!("[DDMod] Installation cancelled by user");
                    return;
                }
                
                m.update_status("downloading", &format!("Downloading depot {}/{} (ID: {})", depot_idx+1, total_depots, depot.depot_id));
                
                let mut cmd = Command::new(&depot_downloader_path);
                cmd.args([
                    "-app", &app_id_clone,
                    "-depot", &depot.depot_id,
                    "-manifest", &depot.manifest_id,
                    "-manifestfile", &depot.manifest_file,
                    "-depotkeys", keys_file.to_string_lossy().as_ref(),
                    "-max-downloads", "25",
                    "-dir", download_dir_clone.to_str().unwrap(),
                    "-validate",
                ]);
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());

                let mut child = match cmd.spawn() {
                    Ok(c) => c,
                    Err(e) => {
                        m.update_status("error", &format!("DepotDownloader not found: {}", e));
                        return;
                    }
                };
                
                // Store child PID for cancellation
                m.set_child_pid(Some(child.id()));

                // Parse stdout for progress
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    let start_time = std::time::Instant::now();
                    
                    for line in reader.lines().map_while(Result::ok) {
                        // Check for cancellation while reading
                        if m.is_cancelled() {
                            eprintln!("[DDMod] Cancellation detected during download");
                            let _ = child.kill();
                            return;
                        }
                        
                        eprintln!("[DDMod] {}", line);
                        
                        // Parse progress percentage and calculate ETA
                        if let Some(caps) = percent_re.captures(&line) {
                            if let Some(pct_match) = caps.get(1) {
                                if let Ok(depot_pct) = pct_match.as_str().parse::<f64>() {
                                    // Calculate overall progress: (completed_depots * 100 + current_depot_pct) / total_depots
                                    let raw_pct = ((depot_idx as f64 * 100.0) + depot_pct) / (total_depots as f64);
                                    // For local installs: download is 100% of progress (no rsync)
                                    // For remote installs: download is 0-50%, rsync is 50-100%
                                    let overall_pct = if is_local { raw_pct } else { raw_pct * 0.5 };
                                    eprintln!("[DDMod] Depot {} progress: {:.1}%, Overall: {:.1}%", depot.depot_id, depot_pct, overall_pct);
                                    m.update_download_percent(overall_pct);
                                    
                                    // Calculate ETA based on elapsed time and progress
                                    let elapsed = start_time.elapsed().as_secs_f64();
                                    let eta = if overall_pct > 0.5 && elapsed > 1.0 {
                                        let total_time = elapsed * 100.0 / overall_pct;
                                        let remaining = total_time - elapsed;
                                        if remaining > 3600.0 {
                                            format!("{:.0}h {:.0}m", remaining / 3600.0, (remaining % 3600.0) / 60.0)
                                        } else if remaining > 60.0 {
                                            format!("{:.0}m {:.0}s", remaining / 60.0, remaining % 60.0)
                                        } else {
                                            format!("{:.0}s", remaining.max(1.0))
                                        }
                                    } else {
                                        "calculating...".to_string()
                                    };
                                    
                                    // Speed not available from DepotDownloaderMod output
                                    // (it doesn't print speed info, only percent + filename)
                                    let speed = String::new();
                                    
                                    m.update_download_speed_eta(&speed, &eta);
                                }
                            }
                        }
                        
                        // Also try to parse speed from line if present (backup)
                        if let Some(caps) = speed_re.captures(&line) {
                            if let (Some(num), Some(unit)) = (caps.get(1), caps.get(2)) {
                                let speed = format!("{} {}/s", num.as_str(), unit.as_str());
                                let p = m.progress.lock().unwrap();
                                let current_eta = p.eta.clone();
                                drop(p);
                                m.update_download_speed_eta(&speed, &current_eta);
                            }
                        }
                    }
                }
                
                m.set_child_pid(None);

                let status = child.wait();
                if let Ok(s) = status {
                    if !s.success() {
                        eprintln!("[DDMod] Depot {} failed with exit code {:?}", depot.depot_id, s.code());
                        // Don't return on failure - try next depot
                    }
                }
                
                // Check cancellation after depot completes
                if m.is_cancelled() {
                    return;
                }
            }

            m.update_status("downloading", "Download complete!");
            // For local installs: download complete = 100%, for remote: 50% (rsync will be 50-100%)
            m.update_download_percent(if is_local { 100.0 } else { 50.0 });

            // ========================================
            // PHASE 2b: COPY MANIFESTS TO DEPOTCACHE
            // ========================================
            // Steam needs manifest files in steamapps/depotcache/ to recognize installed depots
            // Without this, users need to "verify game files" for games to work
            m.update_status("configuring", "Copying manifest files to depotcache...");
            
            // Calculate depotcache path: target_dir is steamapps/common, so parent is steamapps
            let depotcache_dir = if is_local {
                PathBuf::from(&target_dir).parent()
                    .map(|p| p.join("depotcache"))
                    .unwrap_or_else(|| PathBuf::from(&target_dir).join("../depotcache"))
            } else {
                // For remote, we'll handle via SSH later with ACF creation
                PathBuf::new()
            };
            
            if is_local && !depotcache_dir.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(&depotcache_dir) {
                    eprintln!("[Manifests] Warning: Failed to create depotcache dir: {}", e);
                }
                
                for depot in &depots {
                    let src = PathBuf::from(&depot.manifest_file);
                    if src.exists() {
                        if let Some(filename) = src.file_name() {
                            let dest = depotcache_dir.join(filename);
                            match std::fs::copy(&src, &dest) {
                                Ok(_) => eprintln!("[Manifests] Copied {:?} to depotcache", filename),
                                Err(e) => eprintln!("[Manifests] Failed to copy {:?}: {}", filename, e),
                            }
                        }
                    } else {
                        eprintln!("[Manifests] Source manifest not found: {:?}", src);
                    }
                }
            }

            // NOTE: Steamless phase removed - users can run it manually from Settings if needed
            // This avoids the complexity of Wine/.NET installation during game downloads

            // ========================================
            // PHASE 3: COUNT FILES (for progress display)
            // ========================================
            let file_count: usize = WalkDir::new(&download_dir_clone)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .count();
            
            {
                let mut p = m.progress.lock().unwrap();
                p.files_total = file_count;
            }
            m.emit_progress(true); // Force update with file count

            // ========================================
            // PHASE 4: TRANSFER (only for REMOTE mode)
            // ========================================
            if is_local {
                // LOCAL: files already in place, no copy needed!
                m.update_status("transferring", &format!("Installed {} files directly", file_count));
                m.update_transfer_progress(file_count, file_count, "direct");
            } else {
                // Check cancellation before rsync
                if m.is_cancelled() {
                    eprintln!("[rsync] Installation cancelled before transfer");
                    return;
                }
                
                // REMOTE: use rsync via SSH
                let remote_path = format!("{}/{}", target_dir, folder_name_clone);
                m.update_status("transferring", &format!("rsync {} files to Deck...", file_count));
                
                let src_path = format!("{}/", download_dir_clone.to_string_lossy());
                // Don't escape - use -s/--protect-args flag in rsync instead
                let dst_path = format!("{}@{}:{}", ssh_config.username, ssh_config.ip, remote_path);
                
                eprintln!("[rsync] Starting transfer to {}", dst_path);
                eprintln!("[rsync] Source: {}", src_path);
                
                // First try: use sshpass if password is provided
                let use_sshpass = !ssh_config.password.is_empty();
                let has_sshpass = if use_sshpass {
                    Command::new("which").arg("sshpass").output()
                        .map(|o| o.status.success())
                        .unwrap_or(false)
                } else {
                    false
                };
                
                eprintln!("[rsync] Password auth: {}, sshpass available: {}", use_sshpass, has_sshpass);
                
                // Detect rsync version to enable --info=progress2 (requires rsync 3.1.0+)
                // First check Homebrew paths (macOS), then fall back to system rsync
                let rsync_path = if std::path::Path::new("/opt/homebrew/bin/rsync").exists() {
                    "/opt/homebrew/bin/rsync"
                } else if std::path::Path::new("/usr/local/bin/rsync").exists() {
                    "/usr/local/bin/rsync"
                } else {
                    "rsync"
                };
                
                // Check if rsync supports --info=progress2 (version 3.1.0+)
                let use_info_progress = Command::new(rsync_path)
                    .arg("--version")
                    .output()
                    .map(|out| {
                        let version_str = String::from_utf8_lossy(&out.stdout);
                        // Parse version like "rsync  version 3.2.7  protocol version 31"
                        if let Some(ver_line) = version_str.lines().next() {
                            if let Some(ver_part) = ver_line.split("version").nth(1) {
                                let ver_num = ver_part.split_whitespace().next().unwrap_or("");
                                let parts: Vec<&str> = ver_num.split('.').collect();
                                if parts.len() >= 2 {
                                    let major = parts[0].parse::<u32>().unwrap_or(0);
                                    let minor = parts[1].parse::<u32>().unwrap_or(0);
                                    // --info=progress2 was added in rsync 3.1.0
                                    return major > 3 || (major == 3 && minor >= 1);
                                }
                            }
                        }
                        false
                    })
                    .unwrap_or(false);
                
                if use_info_progress {
                    eprintln!("[rsync] Using {} with --info=progress2", rsync_path);
                } else {
                    eprintln!("[rsync] Using {} with --progress (older version)", rsync_path);
                }
                
                let mut cmd = Command::new(rsync_path);
                // -s (--protect-args) allows spaces in paths without escaping
                if use_info_progress {
                    cmd.args(["-avzs", "--info=progress2", "--no-inc-recursive"]);
                } else {
                    cmd.args(["-avzs", "--progress"]);
                }
                
                if has_sshpass {
                    // Use sshpass to wrap ssh inside rsync's -e option
                    // The env var SSHPASS must be set, and sshpass -e tells it to read from env
                    let ssh_cmd = format!(
                        "sshpass -e ssh -p {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ServerAliveInterval=30 -o ServerAliveCountMax=10",
                        ssh_config.port
                    );
                    cmd.env("SSHPASS", &ssh_config.password);
                    cmd.args(["-e", &ssh_cmd]);
                    eprintln!("[rsync] Using: rsync -e 'sshpass -e ssh ...'");
                } else if !ssh_config.private_key_path.is_empty() {
                    // Use SSH key
                    let ssh_cmd = format!(
                        "ssh -p {} -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ServerAliveInterval=30 -o ServerAliveCountMax=10",
                        ssh_config.port, ssh_config.private_key_path
                    );
                    cmd.args(["-e", &ssh_cmd]);
                    eprintln!("[rsync] Using SSH key: {}", ssh_config.private_key_path);
                } else {
                    // Default SSH - will use ssh-agent or prompt for password
                    let ssh_cmd = format!(
                        "ssh -p {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ServerAliveInterval=30 -o ServerAliveCountMax=10",
                        ssh_config.port
                    );
                    cmd.args(["-e", &ssh_cmd]);
                    eprintln!("[rsync] Using default SSH (ssh-agent or key-based)");
                }
                
                cmd.args([&src_path, &dst_path]);
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());

                match cmd.spawn() {
                    Ok(mut child) => {
                        // Store PID for cancellation
                        m.set_child_pid(Some(child.id()));
                        
                        // Spawn thread to capture stderr
                        let stderr = child.stderr.take();
                        let stderr_handle = std::thread::spawn(move || {
                            let mut stderr_output = String::new();
                            if let Some(stderr) = stderr {
                                let reader = BufReader::new(stderr);
                                for line in reader.lines().map_while(Result::ok) {
                                    eprintln!("[rsync stderr] {}", line);
                                    stderr_output.push_str(&line);
                                    stderr_output.push('\n');
                                }
                            }
                            stderr_output
                        });
                        
                        // Parse rsync stdout - it uses \r for in-place updates
                        // Read byte by byte and split on \r or \n
                        let m_clone = m.clone();
                        let _file_count_clone = file_count;
                        let stdout = child.stdout.take();
                        
                        let progress_handle = std::thread::spawn(move || {
                            use std::io::Read;
                            
                            if let Some(mut stdout) = stdout {
                                let mut buffer = [0u8; 1];
                                let mut line = String::new();
                                
                                // Regex for parsing rsync --progress output
                                // Format: "filename  1234567 100%  1.23MB/s  0:00:01 (xfr#1, to-chk=259/261)"
                                let files_re = Regex::new(r"to-chk=(\d+)/(\d+)").unwrap();
                                let speed_re = Regex::new(r"(\d+\.?\d*[KMG]?B/s)").unwrap();
                                
                                while let Ok(1) = stdout.read(&mut buffer) {
                                    let ch = buffer[0] as char;
                                    
                                    if ch == '\r' || ch == '\n' {
                                        // Process line
                                        if !line.is_empty() {
                                            // Check for cancellation
                                            if m_clone.is_cancelled() {
                                                break;
                                            }
                                            
                                            // Parse to-chk for file progress
                                            if let Some(caps) = files_re.captures(&line) {
                                                if let (Some(remaining), Some(total)) = (caps.get(1), caps.get(2)) {
                                                    if let (Ok(r), Ok(t)) = (remaining.as_str().parse::<usize>(), total.as_str().parse::<usize>()) {
                                                        let done = t.saturating_sub(r);
                                                        // Get speed if available
                                                        let speed = speed_re.captures(&line)
                                                            .and_then(|c| c.get(1))
                                                            .map(|m| m.as_str())
                                                            .unwrap_or("");
                                                        m_clone.update_transfer_progress(done, t, speed);
                                                        eprintln!("[rsync] Files: {}/{} {}", done, t, speed);
                                                    }
                                                }
                                            }
                                            line.clear();
                                        }
                                    } else {
                                        line.push(ch);
                                    }
                                }
                            }
                        });
                        
                        m.set_child_pid(None);
                        
                        // Get stderr output
                        let stderr_output = stderr_handle.join().unwrap_or_default();
                        
                        // Wait for progress thread
                        let _ = progress_handle.join();

                        let status = child.wait();
                        
                        // Update progress to 100% 
                        m.update_transfer_progress(file_count, file_count, "done");
                        
                        if let Ok(s) = status {
                            if !s.success() {
                                let exit_code = s.code().unwrap_or(-1);
                                let error_msg = match exit_code {
                                    1 => format!("rsync syntax/usage error or sshpass issue. {}",
                                        if use_sshpass && !has_sshpass { 
                                            "Install sshpass: brew install esolitos/ipa/sshpass" 
                                        } else { 
                                            "Check SSH connection manually." 
                                        }),
                                    2 => "rsync protocol incompatibility".to_string(),
                                    3 => "Errors selecting input/output files".to_string(),
                                    5 => "rsync: error starting client-server protocol".to_string(),
                                    10 => "rsync: connection unexpectedly closed".to_string(),
                                    11 => "rsync: error in file I/O".to_string(),
                                    12 => "rsync: problem with rsync protocol data stream".to_string(),
                                    23 => "rsync: partial transfer (some files transferred)".to_string(),
                                    24 => "rsync: partial transfer (vanished source files)".to_string(),
                                    255 => format!("SSH connection failed. Check: 1) Deck is on, 2) SSH enabled, 3) IP correct ({})", ssh_config.ip),
                                    _ => format!("rsync error code {}", exit_code),
                                };
                                eprintln!("[rsync] FAILED: {}", error_msg);
                                if !stderr_output.is_empty() {
                                    eprintln!("[rsync] stderr: {}", stderr_output.trim());
                                }
                                m.update_status("error", &error_msg);
                                return;
                            }
                        }
                        eprintln!("[rsync] Transfer complete!");
                    }
                    Err(e) => {
                        eprintln!("[rsync] Failed to spawn: {}", e);
                        m.update_status("error", &format!("rsync not found or failed: {}", e));
                        return;
                    }
                }
                
                m.update_transfer_progress(file_count, file_count, "done");
            }

            // ========================================
            // PHASE 5: UPDATE SLSsteam CONFIG
            // ========================================
            m.update_status("configuring", "Updating SLSsteam config...");
            
            if ssh_config.is_local {
                if let Some(home) = dirs::home_dir() {
                    let config_path = home.join(".config/SLSsteam/config.yaml");
                    if config_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&config_path) {
                            let new_config = add_app_to_config_yaml(&content, &app_id_clone, &game_name_clone);
                            let _ = std::fs::write(config_path, new_config);
                        }
                    }
                }
            } else {
                // Remote: use native SSH (ssh2 library) to update config - no sshpass needed
                use std::net::TcpStream;
                use std::time::Duration;
                use std::io::Read;
                
                let addr = format!("{}:{}", ssh_config.ip, ssh_config.port);
                if let Ok(tcp) = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(10)) {
                    if let Ok(mut sess) = ssh2::Session::new() {
                        sess.set_tcp_stream(tcp);
                        if sess.handshake().is_ok() && sess.userauth_password(&ssh_config.username, &ssh_config.password).is_ok() {
                            // Read existing config
                            let mut content = String::new();
                            if let Ok(mut channel) = sess.channel_session() {
                                if channel.exec("cat ~/.config/SLSsteam/config.yaml 2>/dev/null || echo ''").is_ok() {
                                    let _ = channel.read_to_string(&mut content);
                                    let _ = channel.wait_close();
                                }
                            }
                            
                            let new_config = add_app_to_config_yaml(&content, &app_id_clone, &game_name_clone);
                            
                            // Write back via new channel
                            if let Ok(mut channel) = sess.channel_session() {
                                if channel.exec("mkdir -p ~/.config/SLSsteam && cat > ~/.config/SLSsteam/config.yaml").is_ok() {
                                    let _ = channel.write_all(new_config.as_bytes());
                                    let _ = channel.send_eof();
                                    let _ = channel.wait_close();
                                    eprintln!("[SLSsteam] Config updated on remote");
                                }
                            }
                        }
                    }
                }
            }

            // ========================================
            // PHASE 5b: ADD DECRYPTION KEYS TO config.vdf
            // ========================================
            if !depot_keys_clone.is_empty() {
                use crate::config_vdf;
                
                if ssh_config.is_local {
                    // Local: update config.vdf directly
                    if let Some(home) = dirs::home_dir() {
                        // Try common Steam paths
                        let steam_paths = [
                            home.join(".steam/steam/config/config.vdf"),
                            home.join(".local/share/Steam/config/config.vdf"),
                        ];
                        
                        for config_path in steam_paths {
                            if config_path.exists() {
                                if let Ok(content) = std::fs::read_to_string(&config_path) {
                                    let new_config = config_vdf::add_decryption_keys_to_vdf(&content, &depot_keys_clone);
                                    if std::fs::write(&config_path, &new_config).is_ok() {
                                        eprintln!("[config.vdf] Added {} decryption keys to {:?}", depot_keys_clone.len(), config_path);
                                    }
                                }
                                break; // Only update one config.vdf
                            }
                        }
                    }
                } else {
                    // Remote: update config.vdf via SSH
                    use std::net::TcpStream;
                    use std::time::Duration;
                    use std::io::Read;
                    
                    let addr = format!("{}:{}", ssh_config.ip, ssh_config.port);
                    if let Ok(tcp) = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(10)) {
                        if let Ok(mut sess) = ssh2::Session::new() {
                            sess.set_tcp_stream(tcp);
                            if sess.handshake().is_ok() && sess.userauth_password(&ssh_config.username, &ssh_config.password).is_ok() {
                                // Read existing config.vdf
                                let mut content = String::new();
                                if let Ok(mut channel) = sess.channel_session() {
                                    if channel.exec("cat ~/.steam/steam/config/config.vdf 2>/dev/null || echo ''").is_ok() {
                                        let _ = channel.read_to_string(&mut content);
                                        let _ = channel.wait_close();
                                    }
                                }
                                
                                let new_config = config_vdf::add_decryption_keys_to_vdf(&content, &depot_keys_clone);
                                
                                // Write back
                                if let Ok(mut channel) = sess.channel_session() {
                                    if channel.exec("cat > ~/.steam/steam/config/config.vdf").is_ok() {
                                        let _ = channel.write_all(new_config.as_bytes());
                                        let _ = channel.send_eof();
                                        let _ = channel.wait_close();
                                        eprintln!("[config.vdf] Added {} decryption keys on remote", depot_keys_clone.len());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ========================================
            // PHASE 5c: ADD APP TOKEN TO SLSsteam CONFIG (DISABLED - not needed)
            // ========================================
            // NOTE: AppTokens functionality disabled - not needed for current workflow
            // if let Some(ref token) = app_token_clone {
            //     eprintln!("[AppToken] Adding app token for {} (len={})", app_id_clone, token.len());
            //     
            //     if ssh_config.is_local {
            //         if let Some(home) = dirs::home_dir() {
            //             let config_path = home.join(".config/SLSsteam/config.yaml");
            //             if config_path.exists() {
            //                 if let Ok(content) = std::fs::read_to_string(&config_path) {
            //                     let new_config = crate::commands::modify_slssteam_config_section(
            //                         &content, "AppTokens", &app_id_clone, token
            //                     );
            //                     if std::fs::write(&config_path, &new_config).is_ok() {
            //                         eprintln!("[AppToken] Added token for {} to local config", app_id_clone);
            //                     }
            //                 }
            //             }
            //         }
            //     } else {
            //         // Remote: update via SSH
            //         use std::net::TcpStream;
            //         use std::time::Duration;
            //         use std::io::Read;
            //         
            //         let addr = format!("{}:{}", ssh_config.ip, ssh_config.port);
            //         if let Ok(tcp) = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(10)) {
            //             if let Ok(mut sess) = ssh2::Session::new() {
            //                 sess.set_tcp_stream(tcp);
            //                 if sess.handshake().is_ok() && sess.userauth_password(&ssh_config.username, &ssh_config.password).is_ok() {
            //                     let mut content = String::new();
            //                     if let Ok(mut channel) = sess.channel_session() {
            //                         if channel.exec("cat ~/.config/SLSsteam/config.yaml 2>/dev/null || echo ''").is_ok() {
            //                             let _ = channel.read_to_string(&mut content);
            //                             let _ = channel.wait_close();
            //                         }
            //                     }
            //                     
            //                     let new_config = crate::commands::modify_slssteam_config_section(
            //                         &content, "AppTokens", &app_id_clone, token
            //                     );
            //                     
            //                     if let Ok(mut channel) = sess.channel_session() {
            //                         if channel.exec("cat > ~/.config/SLSsteam/config.yaml").is_ok() {
            //                             let _ = channel.write_all(new_config.as_bytes());
            //                             let _ = channel.send_eof();
            //                             let _ = channel.wait_close();
            //                             eprintln!("[AppToken] Added token for {} on remote", app_id_clone);
            //                         }
            //                     }
            //                 }
            //             }
            //         }
            //     }
            // }
            let _ = app_token_clone; // Suppress unused variable warning

            // ========================================
            // PHASE 6: CREATE STEAM ACF MANIFEST
            // ========================================
            m.update_status("configuring", "Creating Steam manifest...");
            
            // Calculate size on disk from file count (estimate)
            let size_on_disk = file_count * 1024 * 1024; // Rough estimate
            
            let acf_content = format!(
                r#""AppState"
{{
	"appid"		"{app_id}"
	"Universe"		"1"
	"name"		"{game_name}"
	"StateFlags"		"4"
	"installdir"		"{folder_name}"
	"SizeOnDisk"		"{size_on_disk}"
	"buildid"		"0"
	"InstalledDepots"
	{{
	}}
	"UserConfig"
	{{
		"platform_override_dest"		"linux"
		"platform_override_source"		"windows"
	}}
	"MountedConfig"
	{{
		"platform_override_dest"		"linux"
		"platform_override_source"		"windows"
	}}
}}"#,
                app_id = app_id_clone,
                game_name = game_name_clone,
                folder_name = folder_name_clone,
                size_on_disk = size_on_disk
            );
            
            let acf_filename = format!("appmanifest_{}.acf", app_id_clone);
            
            if ssh_config.is_local {
                // Local: write directly
                let steamapps_path = std::path::PathBuf::from(&target_dir);
                if let Some(parent) = steamapps_path.parent() {
                    let acf_path = parent.join(&acf_filename);
                    if let Err(e) = std::fs::write(&acf_path, &acf_content) {
                        eprintln!("[ACF] Failed to write {}: {}", acf_path.display(), e);
                    } else {
                        eprintln!("[ACF] Created {}", acf_path.display());
                    }
                }
            } else {
                // Remote: write via native SSH (ssh2 library) - no sshpass needed
                use std::net::TcpStream;
                use std::time::Duration;
                
                // ACF goes in steamapps/ directory (parent of common/)
                // target_dir is like "/home/deck/.steam/steam/steamapps/common"
                let steamapps_dir = target_dir.trim_end_matches('/').trim_end_matches("/common");
                let acf_remote_path = format!("{}/{}", steamapps_dir, acf_filename);
                
                eprintln!("[ACF] Creating {} on remote via ssh2", acf_remote_path);
                
                let addr = format!("{}:{}", ssh_config.ip, ssh_config.port);
                match TcpStream::connect_timeout(
                    &addr.parse().unwrap(),
                    Duration::from_secs(10)
                ) {
                    Ok(tcp) => {
                        match ssh2::Session::new() {
                            Ok(mut sess) => {
                                sess.set_tcp_stream(tcp);
                                if sess.handshake().is_ok() {
                                    if sess.userauth_password(&ssh_config.username, &ssh_config.password).is_ok() {
                                        // Use exec channel to write file
                                        let cmd = format!("cat > \"{}\"", acf_remote_path);
                                        match sess.channel_session() {
                                            Ok(mut channel) => {
                                                if channel.exec(&cmd).is_ok() {
                                                    let _ = channel.write_all(acf_content.as_bytes());
                                                    let _ = channel.send_eof();
                                                    let _ = channel.wait_close();
                                                    let exit = channel.exit_status().unwrap_or(-1);
                                                    if exit == 0 {
                                                        eprintln!("[ACF] Created {} on remote successfully", acf_remote_path);
                                                    } else {
                                                        eprintln!("[ACF] Remote command failed with exit code {}", exit);
                                                    }
                                                }
                                            }
                                            Err(e) => eprintln!("[ACF] Failed to open SSH channel: {}", e),
                                        }
                                    } else {
                                        eprintln!("[ACF] SSH auth failed");
                                    }
                                } else {
                                    eprintln!("[ACF] SSH handshake failed");
                                }
                            }
                            Err(e) => eprintln!("[ACF] Failed to create SSH session: {}", e),
                        }
                    }
                    Err(e) => eprintln!("[ACF] Failed to connect: {}", e),
                }
            }

            // ========================================
            // PHASE 6b: COPY MANIFESTS TO REMOTE DEPOTCACHE
            // ========================================
            if !ssh_config.is_local {
                use std::net::TcpStream;
                use std::time::Duration;
                
                let steamapps_dir = target_dir.trim_end_matches('/').trim_end_matches("/common");
                let depotcache_remote_path = format!("{}/depotcache", steamapps_dir);
                
                eprintln!("[Manifests] Copying manifests to remote depotcache: {}", depotcache_remote_path);
                
                let addr = format!("{}:{}", ssh_config.ip, ssh_config.port);
                if let Ok(tcp) = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(10)) {
                    if let Ok(mut sess) = ssh2::Session::new() {
                        sess.set_tcp_stream(tcp);
                        if sess.handshake().is_ok() && sess.userauth_password(&ssh_config.username, &ssh_config.password).is_ok() {
                            // Create depotcache directory
                            if let Ok(mut channel) = sess.channel_session() {
                                let mkdir_cmd = format!("mkdir -p \"{}\"", depotcache_remote_path);
                                if channel.exec(&mkdir_cmd).is_ok() {
                                    let _ = channel.wait_close();
                                }
                            }
                            
                            // Copy each manifest file via SFTP or exec+cat
                            for depot in &depots {
                                let src = PathBuf::from(&depot.manifest_file);
                                if src.exists() {
                                    if let Some(filename) = src.file_name() {
                                        let remote_path = format!("{}/{}", depotcache_remote_path, filename.to_string_lossy());
                                        if let Ok(content) = std::fs::read(&src) {
                                            let cmd = format!("cat > \"{}\"", remote_path);
                                            if let Ok(mut channel) = sess.channel_session() {
                                                if channel.exec(&cmd).is_ok() {
                                                    let _ = channel.write_all(&content);
                                                    let _ = channel.send_eof();
                                                    let _ = channel.wait_close();
                                                    eprintln!("[Manifests] Copied {:?} to remote depotcache", filename);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ========================================
            // PHASE 7: CLEANUP
            // ========================================
            m.update_status("finished", "Installation complete!");
            
            // Cleanup temp directory (only for REMOTE mode - LOCAL downloaded directly)
            if !is_local {
                let _ = std::fs::remove_dir_all(&download_dir_clone);
            }
        });

        Ok(())
    }
}

/// Helper function to add AppID to SLSsteam config.yaml
/// Uses text-based insertion to preserve comments
pub fn add_app_to_config_yaml(content: &str, app_id: &str, game_name: &str) -> String {
    // Check if app_id already exists
    if content.contains(&format!("- {}", app_id)) {
        return content.to_string();
    }
    
    // Find AdditionalApps section
    if let Some(idx) = content.find("AdditionalApps:") {
        // Find the end of AdditionalApps line
        let after_key = &content[idx..];
        if let Some(newline_idx) = after_key.find('\n') {
            let insert_pos = idx + newline_idx + 1;
            
            // Insert the new entry with comment (no indentation needed for YAML list items)
            let new_entry = format!("# {}\n- {}\n", game_name, app_id);
            
            let mut result = String::with_capacity(content.len() + new_entry.len());
            result.push_str(&content[..insert_pos]);
            result.push_str(&new_entry);
            result.push_str(&content[insert_pos..]);
            return result;
        }
    }
    
    // No AdditionalApps section - append it
    let mut result = content.to_string();
    if !result.ends_with('\n') && !result.is_empty() {
        result.push('\n');
    }
    result.push_str("\nAdditionalApps:\n");
    result.push_str(&format!("# {}\n", game_name));
    result.push_str(&format!("- {}\n", app_id));
    result
}
