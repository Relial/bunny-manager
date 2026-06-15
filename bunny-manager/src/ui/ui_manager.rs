use std::{
    env::current_exe,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use abi_stable::std_types::RArc;
use anyhow::{Result, anyhow};
use bunny_ui::input_state::{Input, PointerState};
use egui::{
    FontData, FontFamily, Image, Pos2, Rect, SizeHint, TextureOptions, Ui, Vec2,
    emath::GuiRounding as _,
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
    include_image,
    load::TexturePoll,
    paint_texture_at,
};
use tracing::{debug, error, info, warn};

pub static INIT: AtomicBool = AtomicBool::new(false);

use crate::{
    FONTS_PATH,
    config::{Config, get_config_path},
    plugin_manager::PLUGIN_MANAGER,
    ui::{main_window::MainWindow, stats::Stats},
};

#[derive(Debug)]
pub struct UiManager {
    pub stats: Stats,
    main_window: MainWindow,
    paint_cursor: bool,
    input: Input,
    response_pointerstate: RArc<PointerState>,
    pub config: Config,
    pub config_path: Option<PathBuf>,
    fonts_path: Option<PathBuf>,
    last_autosave: Instant,
}

impl UiManager {
    pub fn new() -> Self {
        let config_path = get_config_path();
        let config = match &config_path {
            Ok(path) => match Config::load(path) {
                Ok(config) => config,
                Err(e) => {
                    error!(
                        "Failed to load Bunny Manager config at {}: {e}. Using default config.",
                        path.display()
                    );
                    let config = Config::default();
                    if let Err(e) = config.save(path) {
                        error!("Failed to save new config: {e}");
                    } else {
                        info!("Succesfully created new config");
                    }
                    config
                }
            },
            Err(e) => {
                warn!("Failed to get Bunny Manager config path: {e}. Using default config.");
                Config::default()
            }
        };

        let fonts_path = match get_fonts_path() {
            Ok(path) => Some(path),
            Err(e) => {
                error!("Failed to get fonts path: {e}");
                None
            }
        };

        Self {
            stats: Default::default(),
            main_window: MainWindow::new(&config),
            paint_cursor: false,
            input: Default::default(),
            response_pointerstate: Default::default(),
            config,
            config_path: config_path.ok(),
            fonts_path,
            last_autosave: Instant::now(),
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        self.update_input(ui);

        let mut plugin_manager = PLUGIN_MANAGER
            .get()
            .expect("Manager should be initialized before d3d9 is hooked and egui gets called.")
            .lock()
            .unwrap();

        if self.config.config_autosave_interval_seconds > 0
            && self.last_autosave.elapsed()
                > Duration::from_secs(self.config.config_autosave_interval_seconds)
        {
            debug!("Autosaving");
            let config_path = self.config_path.clone();
            let config = self.config;
            std::thread::spawn(move || {
                if let Some(path) = &config_path
                    && let Err(e) = config.save(path)
                {
                    error!("Config save error: {e}");
                }
            });
            for plugin in &plugin_manager.plugins {
                plugin.save();
            }
            self.last_autosave = Instant::now();
        }

        if self.main_window.display {
            self.paint_cursor = true;
            self.main_window.ui(
                ui,
                &mut self.stats,
                &mut plugin_manager,
                self.input.clone(),
                self.response_pointerstate.clone(),
                &mut self.config,
            );
        }

        if let Some(style) = plugin_manager.style(ui).cloned() {
            for plugin in &mut plugin_manager.plugins {
                plugin.free_ui(
                    ui,
                    &style,
                    self.input.clone(),
                    self.response_pointerstate.clone(),
                    ui.max_rect(),
                    self.config.collect_stats,
                );
                plugin.process_paint_list(ui);
            }
        } else {
            warn!("Something went wrong converting egui style");
        }

        ui.input_mut(|i| {
            if i.consume_shortcut(&self.config.toggle_manager_shortcut) {
                self.main_window.display = !self.main_window.display;
            }
        });

        if self.paint_cursor
            && let Some(pointer_pos) = ui.pointer_latest_pos()
        {
            paint_cursor(pointer_pos, ui);
        }
    }

    fn init(&mut self, ui: &mut Ui) {
        egui_extras::install_image_loaders(ui);
        ui.disable_accesskit();
        ui.style_mut().interaction.tooltip_delay = 0.1;

        // egui default font doesn't support JP, this is the fallback
        ui.add_font(FontInsert::new(
            "NotoSansJP-Regular",
            FontData::from_static(include_bytes!("../../assets/NotoSansJP-Regular.ttf")),
            vec![
                InsertFontFamily {
                    family: FontFamily::Proportional,
                    priority: FontPriority::Lowest,
                },
                InsertFontFamily {
                    family: FontFamily::Monospace,
                    priority: FontPriority::Lowest,
                },
            ],
        ));

        if let Some(fonts_path) = &self.fonts_path
            && let Err(e) = add_fonts(fonts_path, ui)
        {
            error!("Error adding fonts: {e}");
        }
    }

    #[inline(always)]
    pub fn collect_stats(&self) -> bool {
        self.config.collect_stats
    }

    fn update_input(&mut self, ui: &mut Ui) {
        let input_options = ui.options(|o| o.input_options);
        ui.input(|i| {
            self.input.collect(i, input_options.into());
        });

        // Plugins read responses 1 frame late, so they need a copy of the pointerstate that won't get updated.
        self.response_pointerstate = self.input.read(|i| RArc::new(i.pointer.clone()));
    }
}

pub fn ui(ui: &mut Ui, manager: &mut UiManager) {
    if !INIT.load(Ordering::Relaxed) {
        // This runs on startup and at D3D9 Reset
        manager.init(ui);
        INIT.store(true, Ordering::Relaxed);
    }
    manager.paint_cursor = false;

    manager.ui(ui);
}

fn paint_cursor(pos: Pos2, ui: &Ui) {
    let cursor = Image::new(include_image!("../../assets/pointer_c.svg"));
    let painter = ui.debug_painter();
    let pixels_per_point = ui.pixels_per_point();
    let rect =
        Rect::from_min_size(pos, Vec2 { x: 20.0, y: 20.0 }).round_to_pixels(pixels_per_point);
    let pixel_size = (pixels_per_point * rect.size()).round();
    let texture = cursor.source(ui).load(
        ui,
        TextureOptions::default(),
        SizeHint::Size {
            width: pixel_size.x as _,
            height: pixel_size.y as _,
            maintain_aspect_ratio: false,
        },
    );

    if let Ok(TexturePoll::Ready { texture }) = texture {
        paint_texture_at(&painter, rect, cursor.image_options(), &texture);
    }
}

fn get_fonts_path() -> Result<PathBuf> {
    let mut path = current_exe()?;
    path.pop();
    path.push(FONTS_PATH);
    Ok(path)
}

fn add_fonts(path: impl AsRef<Path>, ui: &mut Ui) -> Result<()> {
    let path = path.as_ref();
    for entry in path
        .read_dir()
        .map_err(|e| anyhow!("Failed to read fonts dir at {} {e}", path.display()))?
    {
        match entry {
            Ok(entry) => {
                let entry_path = entry.path();
                if let Some(ext) = entry_path.extension()
                    && (ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf"))
                {
                    match std::fs::read(&entry_path) {
                        Ok(font_bytes) => {
                            if let Some(file_name) = entry_path.file_stem() {
                                let n = file_name.to_string_lossy();
                                ui.add_font(FontInsert::new(
                                    &n,
                                    FontData::from_owned(font_bytes),
                                    vec![InsertFontFamily {
                                        family: FontFamily::Name(n.clone().into()),
                                        priority: FontPriority::Highest,
                                    }],
                                ));
                            } else {
                                error!(
                                    "Failed to extract file name from path {}",
                                    entry_path.display()
                                );
                            }
                        }
                        Err(e) => {
                            error!("Failed to read font file at {}: {e}", entry_path.display());
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error reading directory entry: {e}");
            }
        }
    }
    Ok(())
}
