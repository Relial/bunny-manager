use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use abi_stable::std_types::RString;
use egui::{
    FontData, FontFamily, Image, Pos2, Rect, SizeHint, TextureOptions, Ui, Vec2,
    emath::GuiRounding as _,
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
    include_image,
    load::TexturePoll,
    paint_texture_at,
};
use tracing::{debug, error, info};

pub static INIT: AtomicBool = AtomicBool::new(false);

use crate::{
    FONTS_DIR_NAME, LOG_LEVEL, MODULE_DIR_PATH,
    address::Addresses,
    config::{Config, get_config_path},
    font::Fonts,
    plugin_manager::PluginManager,
    ui::{main_window::MainWindow, stats::Stats},
};

#[derive(Debug)]
pub struct UiManager<'a> {
    pub stats: Stats,
    main_window: MainWindow,
    paint_cursor: bool,
    pub config: Config,
    pub config_path: PathBuf,
    fonts: Fonts,
    last_autosave: Instant,
    pub plugin_manager: PluginManager<'a>,
}

impl egui_d3d9::App for UiManager<'_> {
    fn ui(&mut self, ui: &mut Ui) {
        if !INIT.load(Ordering::Acquire) {
            // This runs on startup and at D3D9 Reset
            ui_init(ui.ctx(), &self.fonts);
            INIT.store(true, Ordering::Release);
        }
        self.paint_cursor = false;

        self.plugin_manager.update_input(ui);

        ui.input_mut(|i| {
            if i.consume_shortcut(&self.config.toggle_manager_shortcut) {
                self.main_window.open = !self.main_window.open;
            }
        });

        if self.config.autosave_interval_seconds > 0
            && self.last_autosave.elapsed()
                > Duration::from_secs(self.config.autosave_interval_seconds)
        {
            debug!("Autosaving");
            let config_path = self.config_path.clone();
            let config = self.config;
            std::thread::spawn(move || {
                if let Err(e) = config.save(&config_path) {
                    error!("Config save error: {e:#}");
                }
            });
            self.plugin_manager.save();
            self.last_autosave = Instant::now();
        }

        if self.main_window.open {
            let resp_opt = self.main_window.ui(
                ui,
                &mut self.stats,
                &mut self.plugin_manager,
                &mut self.config,
                self.last_autosave,
            );
            if self.config.hide_cursor_outside_manager {
                ui.input(|i| {
                    if let Some((pointer_pos, resp)) = i.pointer.latest_pos().zip(resp_opt)
                        && resp.rect.contains(pointer_pos)
                    {
                        self.paint_cursor = true;
                    }
                });
            } else {
                self.paint_cursor = true;
            }
        }

        self.plugin_manager.free_ui(ui, &self.config);

        if self.paint_cursor
            && let Some(pointer_pos) = ui.pointer_latest_pos()
        {
            paint_cursor(pointer_pos, ui);
        }
    }
}

impl UiManager<'_> {
    pub fn new(creation_context: &egui::Context, addresses: Addresses) -> Self {
        let config_path = get_config_path();
        let config = match Config::load(&config_path) {
            Ok(config) => config,
            Err(e) => {
                error!(
                    "Failed to load Bunny Manager config at {}: {e:#}",
                    config_path.display()
                );
                info!("Using default config");
                let config = Config::default();
                if let Err(e) = config.save(&config_path) {
                    error!("Failed to save new config: {e:#}");
                } else {
                    info!("Succesfully created new config");
                }
                config
            }
        };

        let fonts_path = MODULE_DIR_PATH
            .get()
            .expect("EXE_PATH must be initialized before UI manager init")
            .join(FONTS_DIR_NAME);
        let fonts = Fonts::load(&fonts_path);

        let log_level = LOG_LEVEL
            .get()
            .expect("LOG_LEVEL must be initialized before UI manager init");
        let font_names = fonts.names().map(RString::from).collect();
        let mut plugin_manager =
            PluginManager::new(addresses, *log_level, creation_context, font_names);
        info!("Loading plugins");
        plugin_manager.load_all();
        info!("Loading done");

        ui_init(creation_context, &fonts);
        INIT.store(true, Ordering::Relaxed);

        Self {
            stats: Default::default(),
            main_window: MainWindow::new(&config),
            paint_cursor: false,
            config,
            config_path,
            fonts,
            last_autosave: Instant::now(),
            plugin_manager,
        }
    }

    #[inline(always)]
    pub fn collect_stats(&self) -> bool {
        self.config.collect_stats
    }
}

fn ui_init(ctx: &egui::Context, fonts: &Fonts) {
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

    fonts.add_all(ctx);
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
