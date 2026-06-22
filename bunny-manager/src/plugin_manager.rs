use std::{
    borrow::Cow,
    ffi::{OsStr, c_void},
    mem::transmute,
    path::PathBuf,
    thread::JoinHandle,
};

use abi_stable::{
    external_types::RRwLock,
    std_types::{RArc, RHashMap, RString},
};
use anyhow::{Context as _, Result, anyhow};
use bunny_plugin::{
    PluginContext, PluginInfo,
    bunny_ui::{
        self,
        input_state::{Input, PointerState},
        paint::paintlist::PaintList,
        response::Response,
        ui::BunnyUi,
    },
};
use egui::{Checkbox, CollapsingHeader, Id, Rect, Ui};
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
    CONFIG_PATH, EXE_PATH, PLUGINS_PATH, address::Addresses, config::Config, ui::stats::PluginStats,
};

#[derive(Debug)]
pub struct PluginManager<'a> {
    plugins: Vec<BunnyPlugin<'a>>,
    global_style: bunny_ui::style::Style,
    dirs: PluginDirs,
    addresses: Addresses,
    fonts: Vec<String>,
    input: Input,
    response_pointerstate: RArc<PointerState>,
}

impl<'a> PluginManager<'a> {
    pub fn new(addresses: Addresses, fonts: Vec<String>, creation_context: &egui::Context) -> Self {
        let dirs = PluginDirs::new();
        let plugins = find_plugins(&dirs).unwrap_or_else(|e| {
            error!("Error finding plugins: {e:#}");
            Vec::new()
        });
        let global_style = bunny_ui::style::Style::from_egui(&creation_context.global_style());
        Self {
            plugins,
            global_style,
            dirs,
            addresses,
            fonts,
            input: Default::default(),
            response_pointerstate: Default::default(),
        }
    }

    pub fn load_all(&mut self) {
        let context = PluginContext::new(
            self.addresses.mhfo_info,
            self.dirs.configs_str.clone(),
            &self.fonts,
        );
        for plugin in &mut self.plugins {
            plugin.load(context.clone());
        }
    }

    pub fn refresh(&mut self) {
        if let Ok(new_plugins) = find_plugins(&self.dirs) {
            self.plugins
                .retain(|existing_plugin| new_plugins.contains(existing_plugin));
            let context = PluginContext::new(
                self.addresses.mhfo_info,
                self.dirs.configs.to_string_lossy(),
                &self.fonts,
            );
            for mut plugin in new_plugins {
                if !self.plugins.contains(&plugin) {
                    plugin.load(context.clone());
                    self.plugins.push(plugin);
                }
            }
        }
    }

    pub fn update_input(&mut self, ui: &mut Ui) {
        let input_options = ui.options(|o| o.input_options);
        ui.input(|i| {
            self.input.collect(i, input_options.into());
        });

        // Plugins read responses 1 frame late, so they need a copy of the pointerstate that won't get updated.
        self.response_pointerstate = self.input.read(|i| RArc::new(i.pointer.clone()));
    }

    pub fn menu_ui(&mut self, ui: &mut Ui, config: &Config) {
        for plugin in &mut self.plugins {
            ui.horizontal(|ui| {
                let mut temp = plugin.loaded();
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if ui.add(Checkbox::without_text(&mut temp)).clicked() {
                        if plugin.loaded() {
                            plugin.unload();
                        } else {
                            plugin.load(PluginContext::new(
                                self.addresses.mhfo_info,
                                self.dirs.configs_str.as_str(),
                                &self.fonts,
                            ));
                        }
                    }
                });
                if plugin.enabled() {
                    CollapsingHeader::new(plugin.name_version()).show(ui, |ui| {
                        plugin.menu_ui(
                            ui,
                            &self.global_style,
                            self.input.clone(),
                            self.response_pointerstate.clone(),
                            ui.max_rect(),
                            config.collect_stats,
                        );
                        plugin.process_paint_list(ui);
                    });
                } else {
                    ui.scope(|ui| {
                        ui.label(&plugin.file_name);
                    });
                }
            });
        }
    }

    pub fn free_ui(&mut self, ui: &mut Ui, config: &Config) {
        for plugin in &mut self.plugins {
            plugin.free_ui(
                ui,
                &self.global_style,
                self.input.clone(),
                self.response_pointerstate.clone(),
                ui.max_rect(),
                config.collect_stats,
            );
            plugin.process_paint_list(ui);
        }
    }

    pub fn stats_ui(&mut self, ui: &mut Ui) {
        for plugin in &mut self.plugins {
            plugin.stats.update();
            ui.strong(plugin.name());
            ui.indent(&plugin.file_name, |ui| {
                plugin.stats.ui(ui);
            });
        }
    }

    pub fn save(&self) -> Vec<JoinHandle<()>> {
        self.plugins.iter().flat_map(|p| p.save()).collect()
    }

    pub fn unload(&mut self) -> Vec<JoinHandle<()>> {
        self.plugins.iter_mut().flat_map(|p| p.unload()).collect()
    }
}

type FnPluginInit = unsafe extern "C" fn(PluginContext) -> PluginInfo;
type FnPluginMenu = unsafe extern "C" fn(&mut BunnyUi);
type FnPluginUi = unsafe extern "C" fn(&mut BunnyUi);
type FnPluginUnload = unsafe extern "C" fn();
type FnPluginSave = unsafe extern "C" fn();

#[derive(Clone, Copy)]
struct PluginFuncs {
    init: FnPluginInit,
    menu_ui: FnPluginMenu,
    free_ui: FnPluginUi,
    unload: FnPluginUnload,
    save: FnPluginSave,
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

// TODO keep track of save/unload threads finishing so no load/save/unload operation is started while a thread is running

#[derive(Clone)]
struct BunnyPlugin<'a> {
    file_name: String,
    info: Option<PluginInfo>,
    stats: PluginStats,
    funcs: Option<PluginFuncs>,
    module_handle: Option<usize>,
    plugin_path: PathBuf,
    paint_list: RArc<RRwLock<PaintList<'a>>>,
    menu_responses: Option<RArc<RHashMap<Id, Response, RandomState>>>,
    free_responses: Option<RArc<RHashMap<Id, Response, RandomState>>>,
}

impl BunnyPlugin<'_> {
    fn new(file_name: String, plugin_path: PathBuf) -> Self {
        Self {
            file_name,
            info: None,
            stats: PluginStats::default(),
            funcs: None,
            module_handle: None,
            plugin_path,
            paint_list: RArc::new(RRwLock::new(PaintList::new())),
            menu_responses: None,
            free_responses: None,
        }
    }

    fn load(&mut self, plugin_context: PluginContext) {
        match unsafe { LoadLibraryW(&HSTRING::from(self.plugin_path.as_path())) } {
            Ok(module) => {
                self.module_handle = Some(module.0 as usize);
                info!("{} loaded", &self.file_name);
                match PluginFuncs::new(module) {
                    Ok(funcs) => match get_plugin_api_version(module) {
                        Ok(plugin_api_ver) => {
                            if plugin_api_ver == bunny_plugin::BUNNY_API_VERSION {
                                let info = unsafe { (funcs.init)(plugin_context) };
                                if let Err(e) = info.init() {
                                    warn!("{} failed to initialize: {e:#}", info.name());
                                } else {
                                    self.info = Some(info);
                                    self.funcs = Some(funcs);
                                }
                            } else {
                                error!(
                                    "{} BunnyUI API version mismatch | Manager API version: {} | Plugin API version: {}",
                                    self.file_name,
                                    bunny_plugin::BUNNY_API_VERSION,
                                    plugin_api_ver
                                )
                            }
                        }
                        Err(e) => error!("{}: {}", self.file_name, e),
                    },
                    Err(e) => error!("{}: {}", self.file_name, e),
                }
            }
            Err(e) => {
                error!("Error loading {}: {e:#}", &self.file_name);
            }
        }
    }

    fn menu_ui(
        &mut self,
        ui: &mut Ui,
        style: &bunny_ui::style::Style,
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

    fn free_ui(
        &mut self,
        ui: &mut Ui,
        style: &bunny_ui::style::Style,
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

    fn process_paint_list(&mut self, ui: &mut Ui) {
        if self.enabled() {
            self.paint_list.write().ui(ui);
        }
    }

    fn save(&self) -> Option<JoinHandle<()>> {
        self.funcs.map(|f| {
            let save = f.save;
            std::thread::spawn(move || unsafe { save() })
        })
    }

    fn unload(&mut self) -> Option<JoinHandle<()>> {
        let thread_handle = self
            .funcs
            .zip(self.module_handle)
            .map(|(funcs, module_handle)| {
                let file_name = self.file_name.clone();
                let save = funcs.save;
                let unload = funcs.unload;
                std::thread::spawn(move || unsafe {
                    let module = HMODULE(module_handle as *mut c_void);
                    save();
                    unload();
                    if let Err(e) = FreeLibrary(module) {
                        error!("Error unloading {}: {e:#}", file_name);
                    } else {
                        info!("Unloaded {}", file_name);
                    }
                })
            });

        if thread_handle.is_none()
            && let Some(module_handle) = self.module_handle
        {
            let module = HMODULE(module_handle as *mut c_void);
            if let Err(e) = unsafe { FreeLibrary(module) } {
                error!("Error unloading {}: {e:#}", self.file_name);
            } else {
                info!("Unloaded {}", self.file_name);
            }
        }

        self.funcs = None;
        self.module_handle = None;
        self.menu_responses = None;
        self.free_responses = None;
        self.paint_list.write().clear();

        thread_handle
    }

    fn loaded(&self) -> bool {
        self.module_handle.is_some()
    }

    fn enabled(&self) -> bool {
        self.funcs.is_some()
    }

    fn name(&self) -> &str {
        if let Some(plugin_info) = &self.info {
            plugin_info.name()
        } else {
            &self.file_name
        }
    }

    fn name_version<'a>(&'a self) -> Cow<'a, str> {
        if let Some(plugin_info) = &self.info {
            format!("{} - {}", plugin_info.name(), plugin_info.version()).into()
        } else {
            Cow::from(&self.file_name)
        }
    }
}

impl PartialEq for BunnyPlugin<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.file_name == other.file_name
    }
}

#[derive(Debug)]
struct PluginDirs {
    plugins: PathBuf,
    configs: PathBuf,
    configs_str: RString,
}

impl PluginDirs {
    fn new() -> Self {
        let mut base = EXE_PATH
            .get()
            .cloned()
            .expect("EXE_PATH not initialized before plugin manager init");
        base.pop();
        let plugins_path = base.join(PLUGINS_PATH);
        let config_path = base.join(CONFIG_PATH);
        let configs_str = config_path.to_string_lossy().into();
        Self {
            plugins: plugins_path,
            configs: config_path,
            configs_str,
        }
    }
}

fn find_plugins<'a>(plugin_dirs: &PluginDirs) -> Result<Vec<BunnyPlugin<'a>>> {
    let path = &plugin_dirs.plugins;
    let mut plugins = Vec::new();
    for entry in path
        .read_dir()
        .with_context(|| format!("Failed to read plugin dir at {}", path.display()))?
    {
        match entry {
            Ok(entry) => {
                let entry_path = entry.path();
                if let Some(ext) = entry_path.extension()
                    && ext.eq_ignore_ascii_case("dll")
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
                            "Failed to convert path at {} to absolute: {e:#}",
                            entry_path.display()
                        ),
                    }
                }
            }
            Err(e) => {
                error!("Error reading directory entry: {e:#}");
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
            .field("file_name", &self.file_name)
            .finish_non_exhaustive()
    }
}
