//! CloudSync File Watcher
//!
//! Watches remotecache.vdf files for changes and triggers sync operations.
//! Uses the `notify` crate for cross-platform file system monitoring.

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Message sent when a remotecache.vdf file changes
#[derive(Debug, Clone)]
pub struct CloudSyncEvent {
    pub app_id: String,
    pub path: PathBuf,
    #[allow(dead_code)]
    pub timestamp: Instant,
}

/// File watcher for cloud sync
pub struct CloudSyncWatcher {
    watcher: Option<RecommendedWatcher>,
    event_sender: Sender<CloudSyncEvent>,
    event_receiver: Arc<Mutex<Option<Receiver<CloudSyncEvent>>>>,
    watched_paths: Arc<Mutex<Vec<PathBuf>>>,
    is_running: Arc<Mutex<bool>>,
}

impl CloudSyncWatcher {
    /// Create a new watcher instance
    pub fn new() -> Result<Self, String> {
        let (tx, rx) = channel();

        Ok(Self {
            watcher: None,
            event_sender: tx,
            event_receiver: Arc::new(Mutex::new(Some(rx))),
            watched_paths: Arc::new(Mutex::new(Vec::new())),
            is_running: Arc::new(Mutex::new(false)),
        })
    }

    /// Start watching for file changes
    pub fn start(&mut self, app_ids: Vec<String>) -> Result<(), String> {
        let sender = self.event_sender.clone();
        let debounce_duration = Duration::from_secs(2);

        // Create a debouncing event handler
        let last_events: Arc<Mutex<std::collections::HashMap<String, Instant>>> =
            Arc::new(Mutex::new(std::collections::HashMap::new()));

        let last_events_clone = last_events.clone();
        let event_handler = move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Only care about modify and create events
                if !matches!(
                    event.kind,
                    notify::EventKind::Modify(_) | notify::EventKind::Create(_)
                ) {
                    return;
                }

                for path in event.paths {
                    // Extract app_id from path
                    if let Some(app_id) = extract_app_id_from_path(&path) {
                        // Debounce: skip if we've seen this app_id recently
                        let now = Instant::now();
                        let should_send = {
                            let mut last = last_events_clone.lock().unwrap();
                            if let Some(last_time) = last.get(&app_id) {
                                if now.duration_since(*last_time) < debounce_duration {
                                    false
                                } else {
                                    last.insert(app_id.clone(), now);
                                    true
                                }
                            } else {
                                last.insert(app_id.clone(), now);
                                true
                            }
                        };

                        if should_send {
                            let _ = sender.send(CloudSyncEvent {
                                app_id,
                                path: path.clone(),
                                timestamp: now,
                            });
                        }
                    }
                }
            }
        };

        // Create watcher
        let mut watcher = RecommendedWatcher::new(event_handler, Config::default())
            .map_err(|e| format!("Failed to create watcher: {}", e))?;

        // Find and watch remotecache.vdf files
        let paths = find_remotecache_paths(&app_ids);
        let mut watched = self.watched_paths.lock().unwrap();

        for (app_id, path) in &paths {
            // Watch the parent directory of remotecache.vdf
            if let Some(parent) = path.parent() {
                if parent.exists() {
                    if let Err(e) = watcher.watch(parent, RecursiveMode::NonRecursive) {
                        eprintln!("[CloudSync] Warning: Failed to watch {}: {}", parent.display(), e);
                    } else {
                        watched.push(path.clone());
                        eprintln!("[CloudSync] Watching {} for app {}", path.display(), app_id);
                    }
                }
            }
        }

        self.watcher = Some(watcher);
        *self.is_running.lock().unwrap() = true;

        Ok(())
    }

    /// Stop watching for file changes
    pub fn stop(&mut self) {
        self.watcher = None;
        *self.is_running.lock().unwrap() = false;
        self.watched_paths.lock().unwrap().clear();
        eprintln!("[CloudSync] Watcher stopped");
    }

    /// Check if watcher is running
    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    /// Take the event receiver (can only be called once)
    pub fn take_receiver(&self) -> Option<Receiver<CloudSyncEvent>> {
        self.event_receiver.lock().unwrap().take()
    }

    /// Get list of watched paths
    #[allow(dead_code)]
    pub fn watched_paths(&self) -> Vec<PathBuf> {
        self.watched_paths.lock().unwrap().clone()
    }
}

impl Default for CloudSyncWatcher {
    fn default() -> Self {
        Self::new().expect("Failed to create default watcher")
    }
}

/// Extract app_id from a remotecache.vdf path
/// Path format: ~/.local/share/Steam/userdata/[user_id]/[app_id]/remotecache.vdf
fn extract_app_id_from_path(path: &PathBuf) -> Option<String> {
    // Get parent directory (should be the app_id folder)
    let parent = path.parent()?;
    let app_id = parent.file_name()?.to_str()?;
    
    // Verify it looks like an app_id (all digits)
    if app_id.chars().all(|c| c.is_ascii_digit()) {
        Some(app_id.to_string())
    } else {
        None
    }
}

/// Find remotecache.vdf files for given app_ids
fn find_remotecache_paths(app_ids: &[String]) -> Vec<(String, PathBuf)> {
    let mut results = Vec::new();

    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return results,
    };

    let steam_path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Steam")
    } else {
        home.join(".local/share/Steam")
    };

    let userdata_path = steam_path.join("userdata");

    if !userdata_path.exists() {
        return results;
    }

    // Iterate through user directories
    if let Ok(user_dirs) = std::fs::read_dir(&userdata_path) {
        for user_entry in user_dirs.flatten() {
            if !user_entry.path().is_dir() {
                continue;
            }

            // Check each app_id directory
            for app_id in app_ids {
                let remotecache = user_entry.path().join(app_id).join("remotecache.vdf");

                if remotecache.exists() {
                    results.push((app_id.clone(), remotecache));
                }
            }
        }
    }

    results
}

/// Global watcher instance (managed by Tauri state)
pub struct CloudSyncWatcherState {
    watcher: Mutex<Option<CloudSyncWatcher>>,
    event_thread: Mutex<Option<thread::JoinHandle<()>>>,
}

impl CloudSyncWatcherState {
    pub fn new() -> Self {
        Self {
            watcher: Mutex::new(None),
            event_thread: Mutex::new(None),
        }
    }

    /// Start the watcher with a callback for events
    pub fn start<F>(&self, app_ids: Vec<String>, on_event: F) -> Result<(), String>
    where
        F: Fn(CloudSyncEvent) + Send + 'static,
    {
        let mut watcher_guard = self.watcher.lock().unwrap();

        // Stop existing watcher if any
        if let Some(ref mut w) = *watcher_guard {
            w.stop();
        }

        // Create and start new watcher
        let mut watcher = CloudSyncWatcher::new()?;
        watcher.start(app_ids)?;

        // Take the receiver and spawn event processing thread
        if let Some(receiver) = watcher.take_receiver() {
            let handle = thread::spawn(move || {
                for event in receiver {
                    on_event(event);
                }
            });
            *self.event_thread.lock().unwrap() = Some(handle);
        }

        *watcher_guard = Some(watcher);
        Ok(())
    }

    /// Stop the watcher
    pub fn stop(&self) {
        if let Some(ref mut watcher) = *self.watcher.lock().unwrap() {
            watcher.stop();
        }
    }

    /// Check if watcher is running
    pub fn is_running(&self) -> bool {
        self.watcher
            .lock()
            .unwrap()
            .as_ref()
            .map(|w| w.is_running())
            .unwrap_or(false)
    }
}

impl Default for CloudSyncWatcherState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_app_id_from_path() {
        let path = PathBuf::from("/home/user/.local/share/Steam/userdata/12345/730/remotecache.vdf");
        assert_eq!(extract_app_id_from_path(&path), Some("730".to_string()));

        let invalid_path = PathBuf::from("/home/user/documents/file.txt");
        assert_eq!(extract_app_id_from_path(&invalid_path), None);
    }
}
