//! GUI command implementation - spawn the Tauri desktop app with optional DB

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn execute(db: Option<PathBuf>) -> Result<()> {
    log::info!("Launching DOTx GUI (Tauri)");

    // Try to locate the dotx-gui directory relative to current dir
    let candidates = [
        PathBuf::from("dotx-gui"),
        PathBuf::from("../dotx-gui"),
        PathBuf::from("../../dotx-gui"),
    ];

    let gui_dir = candidates
        .into_iter()
        .find(|p| p.join("tauri.conf.json").exists())
        .ok_or_else(|| anyhow!(
            "dotx-gui not found. Ensure the 'dotx-gui/' directory exists at the repo root \
             and that Tauri is installed. Try: 'make setup' (installs tauri-cli) or \
             'cargo install tauri-cli'. You can also run 'make run-ui'."
        ))?;

    // Build command: cargo tauri dev [-- --db path]
    let mut cmd = Command::new("cargo");
    cmd.arg("tauri").arg("dev");

    if let Some(db_path) = db.as_ref() {
        cmd.arg("--");
        cmd.arg("--db");
        cmd.arg(db_path);
    }

    cmd.current_dir(&gui_dir);
    cmd.env("RUST_LOG", std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()));
    log::debug!("Running: {:?} (cwd={})", cmd, gui_dir.display());

    let status = cmd.status().context("Failed to spawn Tauri dev server")?;
    if !status.success() {
        return Err(anyhow!("Tauri exited with status: {status}"));
    }

    Ok(())
}
