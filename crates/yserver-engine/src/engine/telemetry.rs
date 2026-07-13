use std::time::{Duration, Instant};

/// Per-frame telemetry — cf. Yserver's `LoopTelemetry`.
#[derive(Debug, Default, Clone)]
pub struct FrameStats {
    pub frame_count: u64,
    pub total_frame_time_ms: f64,
    pub compose_time_ms: f64,
    pub submit_time_ms: f64,
    pub dirty_rect_count: u32,
    pub layer_count: usize,
    pub full_redraw: bool,
}

/// Rolling diagnostics over a measurement window.
pub struct EngineTelemetry {
    enabled: bool,
    last_emit: Option<Instant>,
    window_frames: u64,
    window_frame_time: f64,
    window_compose_time: f64,
    window_submit_time: f64,
    window_full_redraws: u64,
    max_frame_time: f64,
}

impl EngineTelemetry {
    pub fn new() -> Self {
        Self {
            enabled: std::env::var_os("YSERVER_ENGINE_TELEMETRY").is_some(),
            last_emit: None,
            window_frames: 0,
            window_frame_time: 0.0,
            window_compose_time: 0.0,
            window_submit_time: 0.0,
            window_full_redraws: 0,
            max_frame_time: 0.0,
        }
    }

    pub fn record_frame(&mut self, stats: &FrameStats, now: Instant) {
        if !self.enabled {
            return;
        }
        self.window_frames += 1;
        self.window_frame_time += stats.total_frame_time_ms;
        self.window_compose_time += stats.compose_time_ms;
        self.window_submit_time += stats.submit_time_ms;
        if stats.full_redraw {
            self.window_full_redraws += 1;
        }
        if stats.total_frame_time_ms > self.max_frame_time {
            self.max_frame_time = stats.total_frame_time_ms;
        }

        if let Some(last) = self.last_emit {
            let elapsed = now.saturating_duration_since(last);
            if elapsed >= Duration::from_secs(1) {
                let secs = elapsed.as_secs_f64().max(1e-6);
                log::info!(
                    "engine telemetry [{:.2}s]: fps={:.0} avg_frame={:.1}ms max_frame={:.1}ms \
                     compose={:.1}ms submit={:.1}ms full_redraws={}",
                    secs,
                    self.window_frames as f64 / secs,
                    self.window_frame_time / self.window_frames as f64,
                    self.max_frame_time,
                    self.window_compose_time / self.window_frames as f64,
                    self.window_submit_time / self.window_frames as f64,
                    self.window_full_redraws,
                );
                self.window_frames = 0;
                self.window_frame_time = 0.0;
                self.window_compose_time = 0.0;
                self.window_submit_time = 0.0;
                self.window_full_redraws = 0;
                self.max_frame_time = 0.0;
                self.last_emit = Some(now);
            }
        } else {
            self.last_emit = Some(now);
        }
    }
}