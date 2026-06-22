use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
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
    EXE_PATH, FONTS_PATH,
    address::Addresses,
    config::{Config, get_config_path},
    plugin_manager::PluginManager,
    ui::{main_window::MainWindow, stats::Stats},
};

#[derive(Debug)]
pub struct UiManager<'a> {
    pub stats: Stats,
    main_window: MainWindow,
    paint_cursor: bool,
    pub config: Config,
    pub config_path: Option<PathBuf>,
    fonts_path: PathBuf,
    last_autosave: Instant,
    pub plugin_manager: PluginManager<'a>,
}

impl UiManager<'_> {
    pub fn new(creation_context: &egui::Context, addresses: Addresses) -> Self {
        let config_path = get_config_path();
        let config = match &config_path {
            Ok(path) => match Config::load(path) {
                Ok(config) => config,
                Err(e) => {
                    error!(
                        "Failed to load Bunny Manager config at {}: {e:#}",
                        path.display()
                    );
                    info!("Using default config");
                    let config = Config::default();
                    if let Err(e) = config.save(path) {
                        error!("Failed to save new config: {e:#}");
                    } else {
                        info!("Succesfully created new config");
                    }
                    config
                }
            },
            Err(e) => {
                warn!("Failed to get Bunny Manager config path: {e:#}. Using default config.");
                Config::default()
            }
        };

        let mut fonts_path = EXE_PATH
            .get()
            .cloned()
            .expect("EXE_PATH not initialized before UI manager init");
        fonts_path.pop();
        fonts_path.push(FONTS_PATH);

        let fonts = ui_init(creation_context, &fonts_path);
        let mut plugin_manager = PluginManager::new(addresses, fonts, creation_context);
        info!("Loading plugins");
        plugin_manager.load_all();
        info!("Loading done");

        Self {
            stats: Default::default(),
            main_window: MainWindow::new(&config),
            paint_cursor: false,
            config,
            config_path: config_path.ok(),
            fonts_path,
            last_autosave: Instant::now(),
            plugin_manager,
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        self.plugin_manager.update_input(ui);

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
                    error!("Config save error: {e:#}");
                }
            });
            self.plugin_manager.save();
            self.last_autosave = Instant::now();
        }

        if self.main_window.display {
            self.paint_cursor = true;
            self.main_window.ui(
                ui,
                &mut self.stats,
                &mut self.plugin_manager,
                &mut self.config,
            );
        }

        self.plugin_manager.free_ui(ui, &self.config);

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

    #[inline(always)]
    pub fn collect_stats(&self) -> bool {
        self.config.collect_stats
    }
}

fn ui_init(ctx: &egui::Context, fonts_path: impl AsRef<Path>) -> Vec<String> {
    egui_extras::install_image_loaders(ctx);
    ctx.disable_accesskit();
    ctx.global_style_mut(|s| s.interaction.tooltip_delay = 0.1);

    // egui default font doesn't support JP, this is the fallback
    ctx.add_font(FontInsert::new(
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

    add_fonts(fonts_path, ctx).unwrap_or_else(|e| {
        error!("Error adding fonts: {e:#}");
        Vec::new()
    })
}

pub fn ui(ui: &mut Ui, manager: &mut UiManager) {
    if !INIT.load(Ordering::Relaxed) {
        // This runs on startup and at D3D9 Reset
        ui_init(ui.ctx(), &manager.fonts_path);
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

fn add_fonts(path: impl AsRef<Path>, ctx: &egui::Context) -> Result<Vec<String>> {
    let mut fonts = Vec::new();
    let path = path.as_ref();
    for entry in path
        .read_dir()
        .with_context(|| format!("Failed to read fonts dir at {}", path.display()))?
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
                                ctx.add_font(FontInsert::new(
                                    &n,
                                    FontData::from_owned(font_bytes),
                                    vec![InsertFontFamily {
                                        family: FontFamily::Name(n.clone().into()),
                                        priority: FontPriority::Highest,
                                    }],
                                ));
                                fonts.push(n.into());
                            } else {
                                error!(
                                    "Failed to extract file name from path {}",
                                    entry_path.display()
                                );
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to read font file at {}: {e:#}",
                                entry_path.display()
                            );
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error reading directory entry: {e:#}");
            }
        }
    }
    Ok(fonts)
}
