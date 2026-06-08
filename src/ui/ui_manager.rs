use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use bunny_ui::input_state::Input;
use egui::{
    Image, Pos2, Rect, SizeHint, TextureOptions, Ui, Vec2, emath::GuiRounding as _, include_image,
    load::TexturePoll, paint_texture_at,
};
use tracing::{debug, error, info, warn};

pub static INIT: AtomicBool = AtomicBool::new(false);

use crate::{
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
    pub config: Config,
    pub config_path: Option<PathBuf>,
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

        Self {
            stats: Stats::default(),
            main_window: MainWindow::new(&config),
            paint_cursor: false,
            input: Input::default(),
            config,
            config_path: config_path.ok(),
            last_autosave: Instant::now(),
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        let input_options = ui.options(|o| o.input_options);
        ui.input(|i| {
            self.input.collect(i, input_options.into());
        });

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
        }

        if self.main_window.display {
            self.paint_cursor = true;
            self.main_window.ui(
                ui,
                &mut self.stats,
                &mut plugin_manager,
                self.input.clone(),
                &mut self.config,
            );
        }

        let Some(style) = plugin_manager.style(ui).cloned() else {
            warn!("Something went wrong converting egui style");
            return;
        };
        for plugin in &mut plugin_manager.plugins {
            plugin.free_ui(
                ui,
                &style,
                self.input.clone(),
                ui.max_rect(),
                self.config.collect_stats,
            );
            plugin.process_paint_list(ui);
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
    }

    #[inline(always)]
    pub fn collect_stats(&self) -> bool {
        self.config.collect_stats
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
