use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use yserver_engine::engine::compositor::Compositor;
use yserver_engine::engine::core_loop::{channel, run_frame, EngineReceiver, EngineSender};
use yserver_engine::engine::message::{LayerDescriptor, Message};
use yserver_engine::engine::state::EngineState;
use yserver_engine::engine::telemetry::EngineTelemetry;
use yserver_engine::input::translate_window_event;
use yserver_engine::render::wgpu_backend::WgpuBackend;

struct App {
    window: Option<Window>,
    backend: Option<WgpuBackend>,
    state: Option<EngineState>,
    compositor: Option<Compositor>,
    telemetry: Option<EngineTelemetry>,
    tx: Option<EngineSender>,
    rx: Option<EngineReceiver>,
    width: u32,
    height: u32,
    seeded: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = event_loop
            .create_window(
                Window::default_attributes().with_title("Yserver Engine — Phase 2"),
            )
            .expect("failed to create window");

        let width = 800u32;
        let height = 600u32;

        let backend = pollster::block_on(WgpuBackend::new(&window, width, height));
        let state = EngineState::new(width as f32, height as f32);
        let compositor = Compositor::new(width as f32, height as f32);
        let telemetry = EngineTelemetry::new();
        let (tx, rx) = channel();

        log::info!("initialised: window={width}x{height}");

        self.window = Some(window);
        self.backend = Some(backend);
        self.state = Some(state);
        self.compositor = Some(compositor);
        self.telemetry = Some(telemetry);
        self.tx = Some(tx);
        self.rx = Some(rx);
        self.width = width;
        self.height = height;
        self.seeded = false;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // ── Seed demo layers on first frame via messages ──
        if !self.seeded {
            self.seeded = true;
            if let Some(ref tx) = self.tx {
                let _ = tx.send(Message::CreateLayer {
                    id: 1,
                    desc: LayerDescriptor {
                        rect: [50.0, 50.0, 200.0, 150.0],
                        color: [0.2, 0.4, 0.8, 1.0],
                        z: 1,
                    },
                });
                let _ = tx.send(Message::CreateLayer {
                    id: 2,
                    desc: LayerDescriptor {
                        rect: [300.0, 100.0, 250.0, 200.0],
                        color: [0.8, 0.3, 0.2, 0.8],
                        z: 0,
                    },
                });
                let _ = tx.send(Message::CreateLayer {
                    id: 3,
                    desc: LayerDescriptor {
                        rect: [100.0, 300.0, 600.0, 100.0],
                        color: [0.1, 0.7, 0.4, 0.9],
                        z: 2,
                    },
                });
                log::info!("seeded demo layers");
            }
        }

        // ── Translate and forward events ──
        match &event {
            WindowEvent::CloseRequested => {
                if let Some(ref tx) = self.tx {
                    let _ = tx.send(Message::Shutdown);
                }
                event_loop.exit();
                return;
            }
            WindowEvent::RedrawRequested => {
                let state = self.state.as_mut().unwrap();
                let backend = self.backend.as_mut().unwrap();
                let compositor = self.compositor.as_mut().unwrap();
                let telemetry = self.telemetry.as_mut().unwrap();
                let rx = self.rx.as_ref().unwrap();

                let alive = run_frame(state, backend, compositor, telemetry, rx);
                if !alive {
                    event_loop.exit();
                }
                return;
            }
            _ => {}
        }

        // Forward all other events through the input translator
        if let Some(ref tx) = self.tx {
            for msg in translate_window_event(&event) {
                let _ = tx.send(msg);
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App {
        window: None,
        backend: None,
        state: None,
        compositor: None,
        telemetry: None,
        tx: None,
        rx: None,
        width: 800,
        height: 600,
        seeded: false,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("event loop error: {e}");
    }
}
