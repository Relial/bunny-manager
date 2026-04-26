use std::{
    env::current_exe,
    ffi::OsStr,
    mem::transmute,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    thread,
};

use abi_stable::std_types::RString;
use anyhow::{Result, anyhow};
use mhfz_bunny_gui::ui::BunnyUi;
use tracing::{error, info};
use windows::{
    Win32::{
        Foundation::HMODULE,
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
    core::{HSTRING, s},
};

use crate::address::GameMode;

pub static PLUGINS: LazyLock<Mutex<Option<Vec<BunnyPlugin>>>> = LazyLock::new(|| Mutex::new(None));

type FnPluginInit = unsafe extern "C" fn(RString, usize, GameMode);
type FnPluginUi = unsafe extern "C" fn() -> BunnyUi;
type FnPluginUnload = unsafe extern "C" fn();

pub struct PluginFuncs {
    pub init: FnPluginInit,
    pub ui: FnPluginUi,
    pub unload: FnPluginUnload,
}

impl PluginFuncs {
    fn new(module: HMODULE) -> Option<Self> {
        unsafe {
            let raw_init = GetProcAddress(module, s!("init"))?;
            let raw_ui = GetProcAddress(module, s!("ui"))?;
            let raw_unload = GetProcAddress(module, s!("unload"))?;

            let init: FnPluginInit = transmute(raw_init);
            let ui: FnPluginUi = transmute(raw_ui);
            let unload: FnPluginUnload = transmute(raw_unload);

            Some(Self { init, ui, unload })
        }
    }
}

pub struct BunnyPlugin {
    pub name: String,
    pub funcs: PluginFuncs,
}

impl BunnyPlugin {
    fn new(name: String, funcs: PluginFuncs) -> Self {
        Self { name, funcs }
    }
}

pub struct PluginDirs {
    pub plugins: PathBuf,
    pub configs: PathBuf,
}

impl PluginDirs {
    pub fn new() -> Result<Self> {
        let exe = current_exe()?;
        let plugins_dir = exe
            .parent()
            .ok_or(anyhow!("Failed to get exe parent dir"))?
            .join("plugins");
        let bunny_dir = plugins_dir.join("bunny_plugins");
        let config_dir = plugins_dir.join("bunny_config");
        if !bunny_dir.is_dir() {
            std::fs::create_dir(&bunny_dir)
                .map_err(|e| anyhow!("Failed to create plugin dir {}  {e}", bunny_dir.display()))?;
        }
        if !config_dir.is_dir() {
            std::fs::create_dir(&config_dir).map_err(|e| {
                anyhow!("Failed to create config dir {}  {e}", config_dir.display())
            })?;
        }
        Ok(Self {
            plugins: bunny_dir,
            configs: config_dir,
        })
    }
}

pub fn load_plugins(plugins_dir: impl AsRef<Path>) -> Result<()> {
    let path = plugins_dir.as_ref();
    let mut plugins = Vec::new();
    for entry in path
        .read_dir()
        .map_err(|e| anyhow!("Failed to read plugin dir at {} {e}", path.display()))?
        .flatten()
    {
        let entry_path = entry.path();
        if let Some(ext) = entry_path.extension()
            && ext == "dll"
        {
            match entry_path.canonicalize() {
                Ok(absolute_path) => {
                    let file_name = entry_path.file_stem().unwrap_or(OsStr::new("?"));
                    unsafe {
                        match LoadLibraryW(&HSTRING::from(absolute_path.as_path())) {
                            Ok(module) => {
                                if let Some(funcs) = PluginFuncs::new(module) {
                                    let plugin = BunnyPlugin::new(
                                        file_name.to_string_lossy().to_string(),
                                        funcs,
                                    );
                                    plugins.push(plugin);
                                } else {
                                    error!(
                                        "Plugin {} was loaded, but not all plugin functions were found.",
                                        file_name.display()
                                    );
                                }
                            }
                            Err(e) => {
                                error!("Error loading {}, skipping: {e}", file_name.display())
                            }
                        }
                    };
                }
                Err(e) => error!(
                    "Failed to convert path at {} to absolute: {e}",
                    entry_path.display()
                ),
            }
        }
    }
    *PLUGINS.lock().unwrap() = Some(plugins);
    Ok(())
}

pub fn initialize_plugins(configs_dir: impl AsRef<Path>, dll_address: usize, game_mode: GameMode) {
    if let Some(plugins) = PLUGINS.lock().unwrap().as_ref() {
        let path = configs_dir.as_ref();
        for plugin in plugins {
            let config_path = path.join(format!("{}.toml", plugin.name));
            match config_path.canonicalize() {
                Ok(absolute_path) => {
                    let func = plugin.funcs.init;
                    let config_path: RString = absolute_path.to_string_lossy().into();
                    thread::spawn(move || unsafe { func(config_path, dll_address, game_mode) });
                }
                Err(e) => {
                    error!(
                        "Failed to convert path at {} to absolute: {e}",
                        config_path.display()
                    );
                }
            };
        }
    }
}
