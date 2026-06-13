use std::{
    env::current_exe,
    ffi::{OsStr, c_void},
    mem::transmute,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    thread::JoinHandle,
};

use abi_stable::{
    external_types::RRwLock,
    std_types::{RArc, RHashMap, RStr},
};
use anyhow::{Result, anyhow};
use bunny_ui::{
    input_state::{Input, PointerState},
    paint::paintlist::PaintList,
    response::Response,
    style::Style,
    ui::BunnyUi,
};
use egui::{Id, Rect, Ui};
use rapidhash::fast::RandomState;
use tracing::{error, info, warn};
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
    core::{HSTRING, s},
};

use crate::{
    CONFIG_PATH, PLUGINS_PATH,
    address::{Addresses, MainDllInfo},
    ui::stats::PluginStats,
};

pub static PLUGIN_MANAGER: OnceLock<Mutex<PluginManager>> = OnceLock::new();

#[derive(Debug)]
pub struct PluginManager<'a> {
    pub plugins: Vec<BunnyPlugin<'a>>,
    global_style: Option<Style>,
    pub dirs: PluginDirs,
    pub addresses: Addresses,
}

impl<'a> PluginManager<'a> {
    pub fn new(addresses: Addresses) -> Result<Self> {
        let dirs = PluginDirs::new()?;
        let mut plugins = find_plugins(&dirs)?;
        for plugin in &mut plugins {
            plugin.load(&dirs.configs, addresses.dll_info);
        }
        Ok(Self {
            plugins,
            global_style: None,
            dirs,
            addresses,
        })
    }

    pub fn style(&mut self, ui: &egui::Ui) -> Option<&Style> {
        self.global_style
            .get_or_insert_with(|| Style::from_egui(ui.style()));
        self.global_style.as_ref()
    }

    pub fn refresh(&mut self) {
        if let Ok(new_plugins) = find_plugins(&self.dirs) {
            self.plugins
                .retain(|existing_plugin| new_plugins.contains(existing_plugin));
            for mut plugin in new_plugins {
                if !self.plugins.contains(&plugin) {
                    plugin.load(&self.dirs.configs, self.addresses.dll_info);
                    self.plugins.push(plugin);
                }
            }
        }
    }
}

type FnPluginInit = unsafe extern "C" fn(RStr, MainDllInfo) -> bool;
type FnPluginMenu = unsafe extern "C" fn(&mut BunnyUi);
type FnPluginUi = unsafe extern "C" fn(&mut BunnyUi);
type FnPluginUnload = unsafe extern "C" fn();
type FnPluginSave = unsafe extern "C" fn();

#[derive(Clone, Copy)]
pub struct PluginFuncs {
    pub init: FnPluginInit,
    pub menu_ui: FnPluginMenu,
    pub free_ui: FnPluginUi,
    pub unload: FnPluginUnload,
    pub save: FnPluginSave,
}

impl PluginFuncs {
    fn new(module: HMODULE) -> Result<Self> {
        unsafe {
            let raw_init = GetProcAddress(module, s!("init"))
                .ok_or(anyhow!("plugin function 'init' not found"))?;
            let raw_menu = GetProcAddress(module, s!("menu"))
                .ok_or(anyhow!("plugin function 'menu' not found"))?;
            let raw_ui = GetProcAddress(module, s!("ui"))
                .ok_or(anyhow!("plugin function 'ui' not found"))?;
            let raw_unload = GetProcAddress(module, s!("unload"))
                .ok_or(anyhow!("plugin function 'unload' not found"))?;
            let raw_save = GetProcAddress(module, s!("save"))
                .ok_or(anyhow!("plugin function 'save' not found"))?;

            let init: FnPluginInit = transmute(raw_init);
            let menu_ui: FnPluginMenu = transmute(raw_menu);
            let free_ui: FnPluginUi = transmute(raw_ui);
            let unload: FnPluginUnload = transmute(raw_unload);
            let save: FnPluginSave = transmute(raw_save);

            Ok(Self {
                init,
                menu_ui,
                free_ui,
                unload,
                save,
            })
        }
    }
}

#[derive(Clone)]
pub struct BunnyPlugin<'a> {
    pub name: String,
    pub loaded: bool,
    pub stats: PluginStats,
    funcs: Option<PluginFuncs>,
    handle: Option<usize>,
    plugin_path: PathBuf,
    paint_list: RArc<RRwLock<PaintList<'a>>>,
    menu_responses: Option<RArc<RHashMap<Id, Response, RandomState>>>,
    free_responses: Option<RArc<RHashMap<Id, Response, RandomState>>>,
}

impl BunnyPlugin<'_> {
    fn new(name: String, plugin_path: PathBuf) -> Self {
        Self {
            name,
            loaded: false,
            stats: PluginStats::default(),
            funcs: None,
            handle: None,
            plugin_path,
            paint_list: RArc::new(RRwLock::new(PaintList::new())),
            menu_responses: None,
            free_responses: None,
        }
    }

    pub fn load(&mut self, config_dir_path: impl AsRef<Path>, main_dll_info: MainDllInfo) {
        match unsafe { LoadLibraryW(&HSTRING::from(self.plugin_path.as_path())) } {
            Ok(module) => {
                self.handle = Some(module.0 as usize);
                self.loaded = true;
                info!("{} loaded", &self.name);
                match PluginFuncs::new(module) {
                    Ok(funcs) => match get_plugin_api_version(module) {
                        Ok(plugin_api_ver) => {
                            if plugin_api_ver == bunny_ui::BUNNY_API_VERSION {
                                if unsafe {
                                    (funcs.init)(
                                        config_dir_path.as_ref().to_string_lossy().as_ref().into(),
                                        main_dll_info,
                                    )
                                } {
                                    self.funcs = Some(funcs);
                                } else {
                                    warn!("{} failed to initialize", self.name);
                                }
                            } else {
                                error!(
                                    "{} BunnyUI API version mismatch | Manager API version: {} | Plugin API version: {}",
                                    self.name,
                                    bunny_ui::BUNNY_API_VERSION,
                                    plugin_api_ver
                                )
                            }
                        }
                        Err(e) => error!("{}: {}", self.name, e),
                    },
                    Err(e) => error!("{}: {}", self.name, e),
                }
            }
            Err(e) => {
                error!("Error loading {}: {e}", &self.name);
                self.loaded = false;
            }
        }
    }

    pub fn menu_ui(
        &mut self,
        ui: &mut Ui,
        style: &Style,
        input: Input,
        response_pointerstate: RArc<PointerState>,
        available_space: Rect,
        collect_stats: bool,
    ) {
        if let Some(funcs) = self.funcs {
            let responses = self
                .menu_responses
                .get_or_insert(RArc::new(RHashMap::with_hasher(RandomState::new())));
            let mut bunny_ui = BunnyUi::new(
                Id::new(1),
                responses.clone(),
                input.clone(),
                self.paint_list.clone(),
                available_space,
                ui.pixels_per_point(),
                style.clone(),
            );

            if collect_stats {
                self.stats.menu_timings().start();
            }
            unsafe { (funcs.menu_ui)(&mut bunny_ui) };

            let mut new =
                RHashMap::with_capacity_and_hasher(responses.len() + 64, RandomState::new());
            bunny_ui.ui(ui, &mut new, response_pointerstate);
            self.menu_responses = Some(RArc::new(new));

            if collect_stats {
                self.stats.menu_timings().pre_paint();
            }
            self.process_paint_list(ui);

            if collect_stats {
                self.stats.menu_timings().end();
            }
        }
    }

    pub fn free_ui(
        &mut self,
        ui: &mut Ui,
        style: &Style,
        input: Input,
        response_pointerstate: RArc<PointerState>,
        available_space: Rect,
        collect_stats: bool,
    ) {
        if let Some(funcs) = self.funcs {
            let responses = self
                .free_responses
                .get_or_insert(RArc::new(RHashMap::with_hasher(RandomState::new())));
            let mut bunny_ui = BunnyUi::new(
                Id::new(1),
                responses.clone(),
                input.clone(),
                self.paint_list.clone(),
                available_space,
                ui.pixels_per_point(),
                style.clone(),
            );

            if collect_stats {
                self.stats.ui_timings().start();
            }
            unsafe { (funcs.free_ui)(&mut bunny_ui) };

            let mut new =
                RHashMap::with_capacity_and_hasher(responses.len() + 64, RandomState::new());
            bunny_ui.ui(ui, &mut new, response_pointerstate);
            self.free_responses = Some(RArc::new(new));

            if collect_stats {
                self.stats.ui_timings().pre_paint();
            }
            self.process_paint_list(ui);

            if collect_stats {
                self.stats.ui_timings().end();
            }
        }
    }

    pub fn save(&self) -> Option<JoinHandle<()>> {
        if let Some(funcs) = self.funcs {
            let save = funcs.save;
            Some(std::thread::spawn(move || unsafe { save() }))
        } else {
            None
        }
    }

    pub fn process_paint_list(&mut self, ui: &mut Ui) {
        if self.enabled() {
            self.paint_list.write().ui(ui);
        }
    }

    pub fn unload(&mut self) {
        if let Some(funcs) = self.funcs
            && let Some(handle) = self.handle
        {
            unsafe { (funcs.unload)() };
            if let Some(handle) = self.save()
                && handle.join().is_err()
            {
                error!("Error saving plugin {}", self.name);
            }
            let module = HMODULE(handle as *mut c_void);
            if let Err(e) = unsafe { FreeLibrary(module) } {
                error!("Error unloading plugin {} {e}", self.name);
            } else {
                info!("Unloaded {}", self.name);
            }
        }
        self.funcs = None;
        self.handle = None;
        self.menu_responses = None;
        self.free_responses = None;
        self.paint_list = RArc::new(RRwLock::new(PaintList::new()));
        self.loaded = false;
    }

    pub fn enabled(&self) -> bool {
        self.funcs.is_some()
    }
}

impl PartialEq for BunnyPlugin<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

#[derive(Debug)]
pub struct PluginDirs {
    pub plugins: PathBuf,
    pub configs: PathBuf,
}

impl PluginDirs {
    pub fn new() -> Result<Self> {
        let mut plugins_path = current_exe()?;
        plugins_path.pop();
        let mut config_path = plugins_path.clone();
        plugins_path.push(PLUGINS_PATH);
        config_path.push(CONFIG_PATH);

        if !plugins_path.is_dir() {
            std::fs::create_dir(&plugins_path).map_err(|e| {
                anyhow!("Failed to create plugin dir {} {e}", plugins_path.display())
            })?;
        }
        if !config_path.is_dir() {
            std::fs::create_dir(&config_path).map_err(|e| {
                anyhow!("Failed to create config dir {} {e}", config_path.display())
            })?;
        }
        Ok(Self {
            plugins: plugins_path,
            configs: config_path,
        })
    }
}

fn find_plugins<'a>(plugin_dirs: &PluginDirs) -> Result<Vec<BunnyPlugin<'a>>> {
    let path = &plugin_dirs.plugins;
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
                    let file_name = entry_path
                        .file_stem()
                        .unwrap_or(OsStr::new("?"))
                        .to_string_lossy()
                        .to_string();
                    plugins.push(BunnyPlugin::new(file_name, absolute_path));
                }
                Err(e) => error!(
                    "Failed to convert path at {} to absolute: {e}",
                    entry_path.display()
                ),
            }
        }
    }
    Ok(plugins)
}

fn get_plugin_api_version(module: HMODULE) -> Result<u32> {
    unsafe {
        let raw_ver = GetProcAddress(module, s!("BUNNY_API_VERSION"))
            .ok_or(anyhow!("Plugin bunny API version not found"))?;
        Ok((raw_ver as *const u32).read())
    }
}

impl std::fmt::Debug for BunnyPlugin<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BunnyPlugin")
            .field("name", &self.name)
            .field("loaded", &self.loaded)
            .field("stats", &self.stats)
            .field("handle", &self.handle)
            .field("plugin_path", &self.plugin_path)
            .finish_non_exhaustive()
    }
}
