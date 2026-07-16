use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::Result;
use egui::{Key, KeyboardShortcut, Modifiers, Pos2, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};

use crate::{CONFIG_DIR_NAME, MODULE_DIR_PATH};

const DEFAULT_WINDOW_SIZE: Vec2 = vec2(400.0, 500.0);
const DEFAULT_WINDOW_POSITION: Pos2 = pos2(20.0, 20.0);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub opacity: u8,
    pub open_on_startup: bool,
    pub toggle_manager_shortcut: KeyboardShortcut,
    pub collect_stats: bool,
    pub autosave_interval_seconds: u64,
    pub hide_cursor_outside_manager: bool,
    pub manually_disabled_plugins: HashSet<String>,
    pub remember_window_size_position: bool,
    pub window_size: Vec2,
    pub window_position: Pos2,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            opacity: 80,
            open_on_startup: true,
            toggle_manager_shortcut: KeyboardShortcut::new(Modifiers::CTRL, Key::Num0),
            collect_stats: false,
            autosave_interval_seconds: 60,
            hide_cursor_outside_manager: false,
            manually_disabled_plugins: HashSet::new(),
            remember_window_size_position: true,
            window_size: DEFAULT_WINDOW_SIZE,
            window_position: DEFAULT_WINDOW_POSITION,
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

    pub fn reset_saved_size_pos(&mut self) {
        self.window_size = DEFAULT_WINDOW_SIZE;
        self.window_position = DEFAULT_WINDOW_POSITION;
    }
}

pub fn get_config_path() -> PathBuf {
    let base = MODULE_DIR_PATH
        .get()
        .expect("MODULE_DIR_PATH not initialized before config init");
    let mut config_path = base.join(CONFIG_DIR_NAME);
    config_path.push("bunny_manager.toml");
    config_path
}
