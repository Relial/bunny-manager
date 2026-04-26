use std::{sync::Once, time::Duration};

use egui::{
    Align, Align2, Button, Color32, Context, CornerRadius, FontId, Image, Key, Layout, Margin,
    Pos2, Rect, Shadow, Shape, SizeHint, Spacing, TextWrapMode, TextureOptions, Ui, UiBuilder,
    Vec2, Visuals,
    emath::GuiRounding,
    epaint::{PathShape, PathStroke},
    include_image,
    load::TexturePoll,
    paint_texture_at,
};
use humantime::format_duration;

use crate::{plugins::PLUGINS, ui::stats::Stats};

pub struct MainWindow {
    display: bool,
    pub stats: Stats,
    opacity: u8,
}

impl MainWindow {
    fn ui(&mut self, ui: &mut Ui) {
        let window_rect = ui.max_rect();
        let title_bar_height = 24.0;
        let title_bar_rect = {
            let mut rect = window_rect;
            rect.max.y = rect.min.y + title_bar_height;
            rect
        };
        self.title_bar(ui, title_bar_rect);

        let content_rect = {
            let mut rect = window_rect;
            rect.min.y = title_bar_rect.max.y;
            rect
        }
        .shrink(4.0);
        ui.scope_builder(UiBuilder::new().max_rect(content_rect), |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.take_available_space();
                self.window_content(ui);
            });
        });
    }

    fn title_bar(&mut self, ui: &mut Ui, rect: Rect) {
        let painter = ui.painter();
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
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                ui.add_space(4.0);
                if ui
                    .add(Button::new("❌").fill(Color32::TRANSPARENT).frame(false))
                    .clicked()
                {
                    self.display = false;
                }
            },
        );
    }

    fn window_content(&mut self, ui: &mut Ui) {
        egui::CollapsingHeader::new("Settings").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Background opacity:");
                ui.add(egui::Slider::new(&mut self.opacity, 0..=100));
            });
        });

        egui::CollapsingHeader::new("Plugins").show(ui, |ui| {
            if let Some(plugins) = PLUGINS.lock().unwrap().as_ref() {
                for plugin in plugins {
                    let plugin_ui = unsafe { (plugin.funcs.ui)() };
                    egui::CollapsingHeader::new(plugin.name.clone()).show(ui, |ui| {
                        plugin_ui.ui(ui);
                    });
                }
            }
        });

        egui::CollapsingHeader::new("Stats").show(ui, |ui| {
            self.stats_display(ui);
        });
    }

    fn stats_display(&mut self, ui: &mut Ui) {
        let stats = &mut self.stats;
        stats.update();
        ui.label(format!("Frame: {}", stats.lifetime_frames()));
        ui.label(format!(
            "FPS: {:.2} (average {:.2})",
            stats.fps(),
            stats.average_fps()
        ));
        ui.label(format!(
            "Uptime: {}",
            format_duration(Duration::from_secs(stats.start_time().elapsed().as_secs()))
        ));
        ui.label(format!(
            "Frametime: {:.2}ms",
            stats.frametime().as_micros() as f64 / 1000.0
        ));
        ui.label(format!(
            "Bunny Manager: {:.2}ms",
            stats.ui_time().as_micros() as f64 / 1000.0
        ));
        ui.label(format!(
            "Game Present: {:.2}ms",
            stats.game_present_time().as_micros() as f64 / 1000.0
        ));
    }
}

pub fn ui(ctx: &Context, state: &mut MainWindow) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        egui_extras::install_image_loaders(ctx);
    });

    if ctx.input(|input| input.key_pressed(Key::Num0)) {
        state.display = !state.display;
    }

    if !state.display {
        return;
    }

    ctx.all_styles_mut(|style| {
        style.animation_time = 0.0;
        style.wrap_mode = Some(TextWrapMode::Extend);
        style.visuals = Visuals {
            window_corner_radius: CornerRadius::ZERO,
            window_shadow: Shadow::NONE,
            window_fill: Color32::from_rgba_unmultiplied(
                12,
                12,
                12,
                (state.opacity as f32 / 100.0 * 255.0) as u8,
            ),
            ..Default::default()
        };
        style.spacing = Spacing {
            window_margin: Margin::ZERO,
            ..Default::default()
        };
    });

    egui::Window::new("Bunny Manager")
        .default_size([300.0, 500.0])
        .resizable([true, true])
        .title_bar(false)
        .scroll(false)
        .show(ctx, |ui| {
            ui.take_available_space();
            state.ui(ui);
        });

    if state.display
        && let Some(pointer_pos) = ctx.pointer_latest_pos()
    {
        paint_cursor(pointer_pos, ctx);
    }
}

fn paint_cursor(pos: Pos2, ctx: &Context) {
    let cursor = Image::new(include_image!("../../assets/pointer_c.svg"));
    let painter = ctx.debug_painter();
    let cursor_rect = Rect {
        min: pos,
        max: pos + Vec2::new(20.0, 20.0),
    };
    let pixels_per_point = ctx.pixels_per_point();
    let rect = cursor_rect.round_to_pixels(pixels_per_point);
    let pixel_size = (pixels_per_point * rect.size()).round();
    let texture = cursor.source(ctx).load(
        ctx,
        TextureOptions::default(),
        SizeHint::Size {
            width: pixel_size.x as _,
            height: pixel_size.y as _,
            maintain_aspect_ratio: false,
        },
    );

    if let Ok(TexturePoll::Ready { texture }) = texture {
        paint_texture_at(&painter, rect, cursor.image_options(), &texture);
    } else {
        let cursor = Shape::Path(PathShape {
            points: vec![
                pos,
                Pos2::new(pos.x, pos.y + 20.0),
                Pos2::new(pos.x + 10.0, pos.y + 15.0),
            ],
            closed: true,
            fill: Color32::WHITE,
            stroke: PathStroke::new(1.0, Color32::BLACK),
        });
        painter.add(cursor);
    }
}

impl Default for MainWindow {
    fn default() -> Self {
        Self {
            display: true,
            stats: Stats::default(),
            opacity: 80,
        }
    }
}
