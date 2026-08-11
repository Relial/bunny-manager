use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use abi_stable::std_types::{RArc, RString};
use bunny_plugin::TextureId;
use egui::{
    FontData, FontFamily, Image, Pos2, Rect, SizeHint, TextureOptions, Ui, Vec2,
    emath::GuiRounding as _,
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
    include_image,
    load::TexturePoll,
    paint_texture_at,
};
use shared::texture::{SharedTextures, SizedTexture};
use tracing::{debug, error, info, warn};
use windows::Win32::Graphics::Direct3D9::IDirect3DDevice9;

pub static INIT: AtomicBool = AtomicBool::new(false);

use crate::{
    FONTS_DIR_NAME, LOG_LEVEL, MODULE_DIR_PATH, TEXTURES_DIR_NAME,
    address::Addresses,
    config::{Config, get_config_path},
    font::Fonts,
    plugin_manager::PluginManager,
    texture::TextureLoader,
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
    shared_texture_loader: Option<TextureLoader>,
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
            let config = self.config.clone();
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

    fn free_draw(&mut self) {
        if let Err(e) = self.plugin_manager.free_draw() {
            panic!("Bunny3D draw error: {e:#}");
        }
    }

    fn free_draw_on_top_of_game(&mut self, device: &IDirect3DDevice9, backup_game_state: bool) {
        if let Err(e) = self
            .plugin_manager
            .free_draw_on_top_of_game(device, backup_game_state)
        {
            panic!("Bunny3D draw error: {e:#}");
        }
    }

    fn free_draw_reset(&mut self) {
        self.plugin_manager.free_draw_reset();
    }

    fn get_shared_textures(
        &mut self,
        egui_ctx: &egui::Context,
    ) -> Option<impl Iterator<Item = (egui::TextureId, std::sync::Arc<egui::ColorImage>)>> {
        if let Some(loader) = &mut self.shared_texture_loader
            && let Some(load_results) = loader.load_all(egui_ctx)
        {
            info!("Allocating {} shared textures", load_results.len());
            let mut names_textures = Vec::new();
            let allocations: Vec<_> = load_results
                .into_iter()
                .enumerate()
                .map(|(i, result)| {
                    let id = i as u64;
                    let texture = SizedTexture::new(TextureId::Shared(id), result.data.size);
                    names_textures.push((result.file_name.into(), texture));

                    (egui::TextureId::User(id), result.data)
                })
                .collect();

            if !names_textures.is_empty() {
                let shared_textures = RArc::new(SharedTextures::new(names_textures));
                if let Some(b) = &mut self.plugin_manager.bunny3d {
                    b.add_shared_textures(shared_textures.clone());
                }
                self.plugin_manager.textures = Some(shared_textures);
            }
            self.shared_texture_loader = None;
            Some(allocations.into_iter())
        } else {
            None
        }
    }

    fn add_shared_texture_allocations(
        &mut self,
        textures: Vec<(
            egui::TextureId,
            windows::Win32::Graphics::Direct3D9::IDirect3DTexture9,
        )>,
    ) {
        if let Some(b) = &mut self.plugin_manager.bunny3d {
            b.add_shared_texture_allocations(textures.into_iter().filter_map(
                |(egui_id, handle)| match egui_id {
                    egui::TextureId::Managed(_) => None,
                    egui::TextureId::User(id) => Some((TextureId::Shared(id), handle)),
                },
            ));
        }
    }
}

impl UiManager<'_> {
    pub fn new(
        creation_context: &egui::Context,
        addresses: Addresses,
        device: &IDirect3DDevice9,
    ) -> Self {
        let config_path = get_config_path();
        let mut config = match Config::load(&config_path) {
            Ok(config) => config,
            Err(e) => {
                warn!(
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
            .expect("MODULE_DIR_PATH must be initialized before UI manager init")
            .join(FONTS_DIR_NAME);
        let fonts = Fonts::load(&fonts_path);

        let log_level = LOG_LEVEL
            .get()
            .expect("LOG_LEVEL must be initialized before UI manager init");
        let font_names = fonts.names().map(RString::from).collect();
        let mut plugin_manager =
            PluginManager::new(addresses, *log_level, creation_context, font_names, device);
        info!("Loading plugins");
        plugin_manager.load_all(&config.manually_disabled_plugins);
        info!("Loading done");

        // If a user deletes a plugin and then later adds it back in, they probably want it to be loaded again.
        config.manually_disabled_plugins.retain(|disabled| {
            plugin_manager
                .file_names()
                .any(|file_name| file_name == disabled)
        });

        ui_init(creation_context, &fonts);
        INIT.store(true, Ordering::Relaxed);

        let textures_path = MODULE_DIR_PATH
            .get()
            .expect("MODULE_DIR_PATH must be initialized before UI manager init")
            .join(TEXTURES_DIR_NAME);
        let shared_texture_loader = TextureLoader::new(&textures_path)
            .map_err(|e| error!("Shared texture load error: {e:#}"))
            .ok();

        Self {
            stats: Default::default(),
            main_window: MainWindow::new(&config),
            paint_cursor: false,
            config,
            config_path,
            fonts,
            last_autosave: Instant::now(),
            plugin_manager,
            shared_texture_loader,
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
