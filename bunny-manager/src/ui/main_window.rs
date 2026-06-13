use abi_stable::std_types::RArc;
use bunny_ui::input_state::{Input, PointerState};
use egui::{
    Align2, Checkbox, CollapsingHeader, Color32, CornerRadius, FontId, Frame, Id, Sense, Shadow,
    TextWrapMode, Ui, scroll_area::ScrollSource, vec2,
};
use tracing::warn;

use crate::{
    config::Config,
    plugin_manager::PluginManager,
    ui::{
        license::{BUNNY_LICENSE, D3D8TO9_LICENSE, EGUI_D3D9_LICENSE},
        shortcut_button::ShortcutButton,
        stats::Stats,
    },
};

#[derive(Debug)]
pub struct MainWindow {
    pub display: bool,
}

impl MainWindow {
    pub fn new(config: &Config) -> Self {
        Self {
            display: config.open_on_startup,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        stats: &mut Stats,
        manager: &mut PluginManager,
        input: Input,
        response_pointerstate: RArc<PointerState>,
        config: &mut Config,
    ) {
        let frame = Frame::new()
            .corner_radius(CornerRadius::ZERO)
            .fill(Color32::from_rgba_unmultiplied(
                12,
                12,
                12,
                (config.opacity as f32 / 100.0 * 255.0) as u8,
            ))
            .shadow(Shadow::NONE)
            .stroke(ui.visuals().window_stroke);
        egui::Window::new("Bunny Manager")
            .default_size([300.0, 500.0])
            .resizable([true, true])
            .frame(frame)
            .title_bar(false)
            .scroll(false)
            .show(ui, |ui| {
                ui.take_available_space();
                let style = ui.style_mut();
                style.animation_time = 0.0;
                style.wrap_mode = Some(TextWrapMode::Extend);
                style.interaction.selectable_labels = false;

                self.title_bar(ui);

                egui::ScrollArea::both()
                    .scroll_source(ScrollSource::MOUSE_WHEEL | ScrollSource::SCROLL_BAR)
                    .show(ui, |ui| {
                        ui.take_available_space();
                        self.window_content(
                            ui,
                            manager,
                            stats,
                            input,
                            response_pointerstate,
                            config,
                        );
                    });
            });
    }

    fn title_bar(&mut self, ui: &mut Ui) {
        let title_bar_height = 24.0;
        let rect = {
            let mut rect = ui.max_rect();
            rect.max.y = rect.min.y + title_bar_height;
            rect
        };
        let painter = ui.painter();
        let id = Id::new("close button");
        let widget_state = ui
            .read_response(id)
            .map(|r| r.widget_state())
            .unwrap_or_default();
        let close_color = ui.visuals().widgets.state(widget_state).fg_stroke.color;
        let close_rect = painter.text(
            rect.right_center() - vec2(4.0, 0.0),
            Align2::RIGHT_CENTER,
            "❌",
            FontId::proportional(14.0),
            close_color,
        );
        if ui.interact(close_rect, id, Sense::click()).clicked() {
            self.display = false;
        }

        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Bunny Manager",
            FontId::proportional(16.0),
            ui.visuals().text_color(),
        );
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            ui.visuals().widgets.noninteractive.bg_stroke,
        );

        ui.allocate_rect(rect, Sense::empty());
    }

    fn window_content(
        &mut self,
        ui: &mut Ui,
        plugin_manager: &mut PluginManager,
        stats: &mut Stats,
        input: Input,
        response_pointerstate: RArc<PointerState>,
        config: &mut Config,
    ) {
        ui.collapsing("Settings", |ui| {
            ui.label("Background opacity");
            ui.add(egui::Slider::new(&mut config.opacity, 0..=100));

            ui.separator();

            ui.label("Config autosave interval seconds (0 to disable)");
            ui.add(egui::Slider::new(
                &mut config.config_autosave_interval_seconds,
                0..=600,
            ));

            ui.separator();

            ui.checkbox(&mut config.collect_stats, "Collect stats");
            ui.checkbox(
                &mut config.open_on_startup,
                "Open manager window on startup",
            );

            ui.separator();

            ui.label("Manager window toggle keybind");
            ui.add(ShortcutButton::new(
                &mut config.toggle_manager_shortcut,
                "manager toggle shortcut",
            ));
        });

        let dll_info = plugin_manager.addresses.dll_info;

        ui.collapsing("Plugins", |ui| {
            if ui.button("Refresh").clicked() {
                plugin_manager.refresh();
            }

            ui.separator();

            let Some(style) = plugin_manager.style(ui).cloned() else {
                warn!("Something went wrong converting egui style");
                return;
            };

            for plugin in &mut plugin_manager.plugins {
                ui.horizontal(|ui| {
                    let mut temp = plugin.loaded;
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if ui.add(Checkbox::without_text(&mut temp)).clicked() {
                            if plugin.loaded {
                                plugin.unload();
                            } else {
                                plugin.load(&plugin_manager.dirs.configs, dll_info);
                            }
                        }
                    });
                    if plugin.enabled() {
                        CollapsingHeader::new(&plugin.name).show(ui, |ui| {
                            plugin.menu_ui(
                                ui,
                                &style,
                                input.clone(),
                                response_pointerstate.clone(),
                                ui.max_rect(),
                                config.collect_stats,
                            );
                            plugin.process_paint_list(ui);
                        });
                    } else {
                        ui.scope(|ui| {
                            ui.label(&plugin.name);
                        });
                    }
                });
            }
        });

        if config.collect_stats {
            ui.collapsing("Stats", |ui| {
                stats.update();
                stats.ui(ui);
            });

            ui.collapsing("Plugin stats", |ui| {
                for plugin in &mut plugin_manager.plugins {
                    plugin.stats.update();
                    ui.strong(&plugin.name);
                    ui.indent(&plugin.name, |ui| {
                        plugin.stats.ui(ui);
                    });
                }
            });
        }

        ui.collapsing("License", |ui| {
            ui.style_mut().wrap_mode = Some(TextWrapMode::Wrap);
            ui.label("Bunny Manager license:");
            ui.label(BUNNY_LICENSE);

            ui.separator();

            ui.label("d3d8to9 license:");
            ui.label(D3D8TO9_LICENSE);

            ui.separator();

            ui.label("egui-d3d9 license:");
            ui.label(EGUI_D3D9_LICENSE);
        });
    }
}
