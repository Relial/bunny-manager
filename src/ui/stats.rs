use std::time::{Duration, Instant};

use egui::Ui;
use jiff::{RoundMode, SignedDuration, SignedDurationRound, Unit};

const AVERAGE_WEIGHT: f64 = 1.0 / 0.99;
const CURRENT_WEIGHT: f64 = 1.0 / 0.01;

#[derive(Debug)]
pub struct Stats {
    frame_start: Instant,
    ui_end: Instant,
    frame_end: Instant,
    frametime_average: Duration,
    frametime: Duration,
    ui_time_average: Duration,
    ui_time: Duration,
    present_time_average: Duration,
    present_time: Duration,
    total_frames: u64,
    start_time: Instant,
    fps_average: f64,
    fps: f64,
}

impl Stats {
    #[inline(always)]
    pub fn frame_start(&mut self) {
        self.total_frames += 1;
        self.frame_start = Instant::now();
    }

    #[inline(always)]
    pub fn ui_end(&mut self) {
        self.ui_end = Instant::now();
        self.ui_time = self.ui_end - self.frame_start;
    }

    #[inline(always)]
    pub fn frame_end(&mut self) {
        let now = Instant::now();
        self.frametime = now - self.frame_end;
        self.frame_end = now;
        self.present_time = self.frame_end - self.frame_start;
    }

    pub fn update(&mut self) {
        self.fps = 1.0 / self.frametime.as_secs_f64();
        self.fps_average = self.fps_average / AVERAGE_WEIGHT + self.fps / CURRENT_WEIGHT;

        update_average_duration(&mut self.frametime_average, self.frametime);
        update_average_duration(&mut self.ui_time_average, self.ui_time);
        update_average_duration(&mut self.present_time_average, self.present_time);
    }

    #[inline(always)]
    pub fn fps(&self) -> f64 {
        self.fps
    }

    #[inline(always)]
    pub fn fps_average(&self) -> f64 {
        self.fps_average
    }

    #[inline(always)]
    pub fn lifetime_frames(&self) -> u64 {
        self.total_frames
    }

    #[inline(always)]
    pub fn start_time(&self) -> Instant {
        self.start_time
    }

    #[inline(always)]
    pub fn ui_time(&self) -> Duration {
        self.ui_time
    }

    #[inline(always)]
    pub fn ui_time_average(&self) -> Duration {
        self.ui_time_average
    }

    #[inline(always)]
    pub fn frametime(&self) -> Duration {
        self.frametime
    }

    #[inline(always)]
    pub fn frametime_average(&self) -> Duration {
        self.frametime_average
    }

    pub fn ui(&self, ui: &mut Ui) {
        ui.label(format!("Frame: {}", self.lifetime_frames()));
        ui.label(format!(
            "FPS: {:.2} (avg {:.2})",
            self.fps(),
            self.fps_average()
        ));
        let dur: SignedDuration = self.start_time().elapsed().try_into().unwrap();
        let round = SignedDurationRound::new()
            .smallest(Unit::Second)
            .mode(RoundMode::Trunc);
        ui.label(format!("Uptime: {:#}", dur.round(round).unwrap()));
        ui.label(format!(
            "Frametime: {:.2}ms (avg {:.2}ms)",
            self.frametime().as_micros() as f64 / 1000.0,
            self.frametime_average().as_micros() as f64 / 1000.0,
        ));
        ui.label(format!(
            "Bunny Manager: {:.2}ms (avg {:.2}ms)",
            self.ui_time().as_micros() as f64 / 1000.0,
            self.ui_time_average().as_micros() as f64 / 1000.0,
        ));
    }
}

impl Default for Stats {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            frame_start: now,
            ui_end: now,
            frame_end: now,
            frametime_average: Duration::from_secs_f64(1.0 / 30.0),
            frametime: Duration::from_secs_f64(1.0 / 30.0),
            ui_time_average: Duration::default(),
            ui_time: Duration::default(),
            present_time_average: Duration::default(),
            present_time: Duration::default(),
            total_frames: 0,
            start_time: now,
            fps_average: 30.0,
            fps: 30.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Timings {
    start: Instant,
    pre_paint: Instant,
    end: Instant,
}

impl Timings {
    fn new(now: Instant) -> Self {
        Self {
            start: now,
            pre_paint: now,
            end: now,
        }
    }

    #[inline(always)]
    pub fn start(&mut self) {
        self.start = Instant::now()
    }

    #[inline(always)]
    pub fn pre_paint(&mut self) {
        self.pre_paint = Instant::now()
    }

    #[inline(always)]
    pub fn end(&mut self) {
        self.end = Instant::now()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Durations {
    pre_paint: Duration,
    paint: Duration,
    total: Duration,

    pre_paint_average: Duration,
    paint_average: Duration,
    total_average: Duration,
}

impl Durations {
    pub fn update(&mut self, timings: Timings) {
        self.pre_paint = timings.pre_paint - timings.start;
        self.paint = timings.end - timings.pre_paint;
        self.total = timings.end - timings.start;
        update_average_duration(&mut self.pre_paint_average, self.pre_paint);
        update_average_duration(&mut self.paint_average, self.paint);
        update_average_duration(&mut self.total_average, self.total);
    }

    fn display(&self, title: &str) -> String {
        format!(
            "{title}: {:.2}ms (avg {:.2}ms) | Plugin side: {:.2}ms (avg {:.2}ms) | Shape processing: {:.2}ms (avg {:.2}ms)",
            self.total.as_micros() as f64 / 1000.0,
            self.total_average.as_micros() as f64 / 1000.0,
            self.pre_paint.as_micros() as f64 / 1000.0,
            self.pre_paint_average.as_micros() as f64 / 1000.0,
            self.paint.as_micros() as f64 / 1000.0,
            self.paint_average.as_micros() as f64 / 1000.0,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PluginStats {
    menu_timings: Timings,
    menu_durations: Durations,

    ui_timings: Timings,
    ui_durations: Durations,
}

impl Default for PluginStats {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            menu_timings: Timings::new(now),
            menu_durations: Default::default(),
            ui_timings: Timings::new(now),
            ui_durations: Default::default(),
        }
    }
}

impl PluginStats {
    #[inline]
    pub fn menu_timings(&mut self) -> &mut Timings {
        &mut self.menu_timings
    }

    #[inline]
    pub fn ui_timings(&mut self) -> &mut Timings {
        &mut self.ui_timings
    }

    pub fn update(&mut self) {
        self.menu_durations.update(self.menu_timings);
        self.ui_durations.update(self.ui_timings);
    }

    pub fn ui(&self, ui: &mut Ui) {
        ui.label(self.menu_durations.display("Menu"));
        ui.label(self.ui_durations.display("UI"));
    }
}

#[inline]
fn update_average_duration(average: &mut Duration, new_time: Duration) {
    *average = average.div_f64(AVERAGE_WEIGHT) + new_time.div_f64(CURRENT_WEIGHT);
}
