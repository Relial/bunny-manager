use std::{
    env::current_exe,
    path::{Path, PathBuf},
};

use anyhow::Result;
use egui::{Key, KeyboardShortcut, Modifiers};
use serde::{Deserialize, Serialize};

use crate::CONFIG_PATH;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Config {
    pub opacity: u8,
    pub open_on_startup: bool,
    pub toggle_manager_shortcut: KeyboardShortcut,
    pub collect_stats: bool,
    pub config_autosave_interval_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            opacity: 80,
            open_on_startup: true,
            toggle_manager_shortcut: KeyboardShortcut::new(Modifiers::CTRL, Key::Num0),
            collect_stats: false,
            config_autosave_interval_seconds: 60,
        }
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        let config = toml::from_slice(&bytes)?;
        Ok(config)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let contents = toml::to_string(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

pub fn get_config_path() -> Result<PathBuf> {
    let mut path = current_exe()?;
    path.pop();
    path.push(CONFIG_PATH);
    path.push("bunny_manager.toml");
    Ok(path)
}
