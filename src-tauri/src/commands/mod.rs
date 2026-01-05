//! Commands module - Re-exports all command submodules
//!
//! This module follows the directory-based module pattern.
//! Each submodule contains related Tauri commands grouped by domain.

pub mod api;
pub mod connection;
pub mod depot;
pub mod depot_keys;
pub mod installation;
pub mod library;
pub mod settings;
pub mod slssteam;
pub mod steam_fixes;
pub mod steamcmd;
pub mod steamless_commands;
pub mod tools;
pub mod transfer;

// Re-export all public items for backward compatibility with lib.rs
pub use api::*;
pub use connection::*;
pub use depot::*;
pub use depot_keys::*;
pub use installation::*;
pub use library::*;
pub use settings::*;
pub use slssteam::*;
pub use steam_fixes::*;
pub use steamcmd::*;
pub use steamless_commands::*;
pub use tools::*;
pub use transfer::*;
