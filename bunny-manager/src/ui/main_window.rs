use std::time::{Duration, Instant};

use egui::{
    Align2, Color32, CornerRadius, FontId, Frame, Id, ProgressBar, Response, Sense, Shadow,
    TextWrapMode, Ui, scroll_area::ScrollSource, vec2,
};

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
    pub open: bool,
}

impl MainWindow {
    pub fn new(config: &Config) -> Self {
        Self {
            open: config.open_on_startup,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        stats: &mut Stats,
        manager: &mut PluginManager,
        config: &mut Config,
        last_autosave: Instant,
    ) -> Option<Response> {
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
                    .wheel_scroll_multiplier(vec2(1.0, 2.5))
                    .show(ui, |ui| {
                        ui.take_available_space();
                        self.window_content(ui, manager, stats, config, last_autosave);
                    });
            })
            .map(|inner| inner.response)
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
            self.open = false;
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
        config: &mut Config,
        last_autosave: Instant,
    ) {
        ui.collapsing("Settings", |ui| {
            ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
            ui.label("Background opacity");
            ui.add(egui::Slider::new(&mut config.opacity, 0..=100));

            ui.separator();

            ui.label("Config autosave interval seconds (0 to disable)");
            let slider_resp = ui.add(egui::Slider::new(
                &mut config.autosave_interval_seconds,
                0..=600,
            ));
            let autosave_interval = Duration::from_secs(config.autosave_interval_seconds);
            let save_progress = 1.0 - (last_autosave.elapsed().div_duration_f32(autosave_interval));
            ui.add(
                ProgressBar::new(save_progress)
                    .desired_height(5.0)
                    .desired_width(slider_resp.rect.width()),
            );

            ui.separator();

            ui.checkbox(&mut config.collect_stats, "Collect stats");
            ui.checkbox(
                &mut config.open_on_startup,
                "Open manager window on startup",
            );
            ui.checkbox(
                &mut config.hide_cursor_outside_manager,
                "Hide cursor outside manager window",
            );

            ui.separator();

            ui.label("Manager window toggle keybind");
            ui.add(ShortcutButton::new(
                &mut config.toggle_manager_shortcut,
                "manager toggle shortcut",
            ));
        });

        ui.collapsing("Plugins", |ui| {
            if ui.button("Refresh").clicked() {
                plugin_manager.refresh();
            }

            ui.separator();

            plugin_manager.menu_ui(ui, config);
        });

        if config.collect_stats {
            ui.collapsing("Stats", |ui| {
                stats.update();
                stats.ui(ui);
            });

            ui.collapsing("Plugin stats", |ui| {
                plugin_manager.stats_ui(ui);
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
