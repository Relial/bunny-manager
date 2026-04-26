use std::time::{Duration, Instant};

const FPS_UPDATE_INTERVAL: Duration = Duration::from_millis(500);
const FPS_ALPHA: f64 = 0.9;

pub struct Stats {
    frame_start: Instant,
    ui_end: Instant,
    frame_end: Instant,
    frametime: Duration,
    ui_time: Duration,
    present_time: Duration,
    total_frames: u64,
    frames_since_last_fps_update: u64,
    last_update: Option<Instant>,
    start_time: Instant,
    average_fps: f64,
    fps: f64,
}

impl Stats {
    #[inline(always)]
    pub fn frame_start(&mut self) {
        self.total_frames += 1;
        self.frames_since_last_fps_update += 1;
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
        if let Some(last_update) = self.last_update {
            let elapsed = last_update.elapsed();
            if elapsed < FPS_UPDATE_INTERVAL {
                return;
            }
            self.fps = self.frames_since_last_fps_update as f64 / elapsed.as_secs_f64();
            self.average_fps = FPS_ALPHA * self.average_fps + (1.0 - FPS_ALPHA) * self.fps;
        }
        self.last_update = Some(Instant::now());
        self.frames_since_last_fps_update = 0;
    }

    #[inline(always)]
    pub fn fps(&self) -> f64 {
        self.fps
    }

    #[inline(always)]
    pub fn average_fps(&self) -> f64 {
        self.average_fps
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
    pub fn game_present_time(&self) -> Duration {
        self.present_time - self.ui_time
    }

    #[inline(always)]
    pub fn frametime(&self) -> Duration {
        self.frametime
    }
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            frame_start: Instant::now(),
            ui_end: Instant::now(),
            frame_end: Instant::now(),
            frametime: Duration::default(),
            ui_time: Duration::default(),
            present_time: Duration::default(),
            total_frames: 0,
            frames_since_last_fps_update: 0,
            last_update: None,
            start_time: Instant::now(),
            average_fps: 30.0,
            fps: 30.0,
        }
    }
}
