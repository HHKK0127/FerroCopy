use std::time::Instant;

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::engine::compositor::Compositor;
use crate::engine::message::Message;
use crate::engine::state::EngineState;
use crate::engine::telemetry::EngineTelemetry;
use crate::render::wgpu_backend::WgpuBackend;

/// Waker-style channel pair (cf. Yserver CoreSender/CoreReceiver).
/// The sender half can be cloned and handed to producer threads.
pub type EngineSender = Sender<Message>;
pub type EngineReceiver = Receiver<Message>;

/// Create a channel pair for the engine loop.
pub fn channel() -> (EngineSender, EngineReceiver) {
    unbounded()
}

/// Run one frame of the core loop. Returns `false` when the engine
/// should shut down.
pub fn run_frame(
    state: &mut EngineState,
    backend: &mut WgpuBackend,
    compositor: &mut Compositor,
    telemetry: &mut EngineTelemetry,
    rx: &EngineReceiver,
) -> bool {
    let frame_start = Instant::now();

    // ── 1. Drain pending messages ──
        let mut needs_redraw = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Message::CreateLayer { desc, .. } => {
                    state.store.insert_layer(desc);
                    needs_redraw = true;
                }
                Message::UpdateLayer { id, color, rect } => {
                    state.store.update_layer(id, color, rect);
                    needs_redraw = true;
                }
                Message::RemoveLayer { id } => {
                    state.store.remove_layer(id);
                    needs_redraw = true;
                }
                Message::SetLayerZ { id, z } => {
                    state.store.set_layer_z(id, z);
                    needs_redraw = true;
                }
                Message::Resize { width, height } => {
                    state.store.set_surface_size(width as f32, height as f32);
                    compositor.resize(width as f32, height as f32);
                    backend.resize(width, height);
                    needs_redraw = true;
                }
            Message::Shutdown => return false,
            Message::PointerInput(_) | Message::KeyboardInput(_) | Message::KeyEvent(_) => {
                // Phase 1: no interactive input handling
            }
        }
    }

    // ── 2. Compose scene ──
    let compose_start = Instant::now();
        let output = compositor.compose(&state.store);
    let compose_time = compose_start.elapsed();

    // ── 3. Submit render commands ──
    let submit_start = Instant::now();
    if needs_redraw || output.full_redraw {
        let _ = backend.render_frame(&output);
    }
    let submit_time = submit_start.elapsed();

    // ── 4. Clear dirty flags ──
        state.store.clear_dirty();

    // ── 5. Telemetry ──
        let mut stats = compositor.collect_stats(&output, &state.store);
    let elapsed = frame_start.elapsed();
    stats.total_frame_time_ms = elapsed.as_secs_f64() * 1000.0;
    stats.compose_time_ms = compose_time.as_secs_f64() * 1000.0;
    stats.submit_time_ms = submit_time.as_secs_f64() * 1000.0;
    telemetry.record_frame(&stats, Instant::now());

    true
}