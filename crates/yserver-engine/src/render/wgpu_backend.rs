use std::mem::size_of;

use wgpu::{
    Adapter, Backends, Device, Instance, Queue, SurfaceConfiguration,
    DeviceDescriptor, Features, Limits, PowerPreference, RequestAdapterOptions,
    VertexBufferLayout, VertexAttribute, VertexFormat, VertexStepMode,
    BufferUsages, RenderPipeline, Buffer, ShaderModuleDescriptor,
    ShaderSource,
};

use crate::engine::compositor::CompositorOutput;

/// Max quads we can draw per frame without reallocating the vertex buffer.
const MAX_QUADS: usize = 1024;
/// Size of one vertex in bytes: position (vec2) + color (vec4) = 6 × f32 = 24.
const VERTEX_SIZE: u64 = size_of::<[f32; 6]>() as u64;
/// Vertices per quad (two triangles).
const VERTS_PER_QUAD: u64 = 6;

/// Minimal wgpu-based render backend (cf. Yserver RenderEngine + KMS backend).
///
/// SAFETY: The surface borrows from a Window that MUST outlive this struct.
/// The caller guarantees this via field declaration order (Window before Backend).
pub struct WgpuBackend {
    pub surface: Option<wgpu::Surface<'static>>,
    pub adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
    pub config: Option<SurfaceConfiguration>,
    pub width: u32,
    pub height: u32,
    clear_color: wgpu::Color,
    surface_format: wgpu::TextureFormat,
    pipeline: Option<RenderPipeline>,
    vertex_buf: Buffer,
}

impl WgpuBackend {
    /// Create the backend via a raw window handle.
    ///
    /// # Safety
    ///
    /// `window` must outlive this backend. The typical pattern is to store
    /// the Window before WgpuBackend in the parent struct (Rust drops fields
    /// in declaration order, so Window is dropped last).
    pub async fn new(window: &winit::window::Window, width: u32, height: u32) -> Self {
        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends: Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window).ok();

        // SAFETY: The caller guarantees the window outlives this backend
        // by declaring it as a sibling field that is dropped first.
        let surface: Option<wgpu::Surface<'static>> = surface.map(|s| unsafe {
            std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(s)
        });

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: surface.as_ref(),
            })
            .await
            .expect("no suitable wgpu adapter");

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                    label: None,
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("failed to create wgpu device");

        let vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad vertex buffer"),
            size: MAX_QUADS as u64 * VERTS_PER_QUAD * VERTEX_SIZE,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut backend = Self {
            surface,
            adapter,
            device,
            queue,
            config: None,
            width,
            height,
            clear_color: wgpu::Color::BLACK,
            surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            pipeline: None,
            vertex_buf,
        };

        backend.configure_surface(width, height);
        backend
    }

    fn rebuild_pipeline(&mut self) {
        let format = self.surface_format;
        let shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("quad shader"),
            source: ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                include_str!("../shaders.wgsl"),
            )),
        });

        let layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad pipeline layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[VertexBufferLayout {
                    array_stride: VERTEX_SIZE,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            offset: 0,
                            format: VertexFormat::Float32x2,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            offset: 8,
                            format: VertexFormat::Float32x4,
                            shader_location: 1,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.pipeline = Some(pipeline);
    }

    fn configure_surface(&mut self, width: u32, height: u32) {
        let Some(ref surface) = self.surface else {
            return;
        };
        let caps = surface.get_capabilities(&self.adapter);
        let format = if caps.formats.contains(&wgpu::TextureFormat::Bgra8UnormSrgb) {
            wgpu::TextureFormat::Bgra8UnormSrgb
        } else {
            caps.formats[0]
        };
        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &config);
        self.config = Some(config);
        self.surface_format = format;
        self.width = width;
        self.height = height;

        self.rebuild_pipeline();
    }

    /// Reconfigure after a resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        if let Some(ref surface) = self.surface {
            if let Some(ref config) = self.config {
                let mut new_config = config.clone();
                new_config.width = width.max(1);
                new_config.height = height.max(1);
                surface.configure(&self.device, &new_config);
                self.config = Some(new_config);
            }
        }
    }

    /// Convert a screen-space rect [x, y, w, h] to 6 vertex records.
        /// Each vertex is [pos_x, pos_y, r, g, b, a].
        fn quad_vertices(rect: &[f32; 4], color: &[f32; 4], w: f32, h: f32) -> [[f32; 6]; 6] {
            let (x, y, qw, qh) = (rect[0], rect[1], rect[2], rect[3]);
            // Screen → clip space: x maps to [-1, 1], y inverts because WGSL clip-space is Y-up
            let x0 = (x / w) * 2.0 - 1.0;
            let y0 = -((y / h) * 2.0 - 1.0);
            let x1 = ((x + qw) / w) * 2.0 - 1.0;
            let y1 = -((y + qh) / h) * 2.0 - 1.0;
            let (r, g, b, a) = (color[0], color[1], color[2], color[3]);
            [
                [x0, y0, r, g, b, a],
                [x0, y1, r, g, b, a],
                [x1, y0, r, g, b, a],
                [x1, y0, r, g, b, a],
                [x0, y1, r, g, b, a],
                [x1, y1, r, g, b, a],
            ]
        }

        /// Render one composited frame.
        pub fn render_frame(&self, output: &CompositorOutput) -> Result<(), wgpu::SurfaceError> {
            let Some(ref surface) = self.surface else {
                return Ok(());
            };
            let Some(ref config) = self.config else {
                return Ok(());
            };
            let Some(ref pipeline) = self.pipeline else {
                return Ok(());
            };

            let frame = surface.get_current_texture()?;
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("frame encoder"),
                });

            // ── Build vertex data from compositor output ──
            let w = config.width as f32;
            let h = config.height as f32;
            let mut vertex_data: Vec<[f32; 6]> = Vec::with_capacity(output.quads.len() * 6);

            for &(_layer_id, ref rect, ref color) in &output.quads {
                vertex_data.extend_from_slice(&Self::quad_vertices(rect, color, w, h));
            }

            // Write vertex data to GPU buffer
                        if !vertex_data.is_empty() {
                            let bytes = unsafe {
                                std::slice::from_raw_parts(
                                    vertex_data.as_ptr() as *const u8,
                                    vertex_data.len() * size_of::<[f32; 6]>(),
                                )
                            };
                            self.queue.write_buffer(&self.vertex_buf, 0, bytes);
                        }

                        // ── Determine scissor rect for partial redraw ──
                        let (scissor_x, scissor_y, scissor_w, scissor_h) = if output.full_redraw {
                            (0, 0, config.width, config.height)
                        } else if let Some(ref damage) = output.damage_rects {
                            // Merge all damage rects into one bounding box for scissor
                            let mut x0 = f32::MAX;
                            let mut y0 = f32::MAX;
                            let mut x1 = f32::MIN;
                            let mut y1 = f32::MIN;
                            for r in damage {
                                x0 = x0.min(r[0]);
                                y0 = y0.min(r[1]);
                                x1 = x1.max(r[0] + r[2]);
                                y1 = y1.max(r[1] + r[3]);
                            }
                            // Clamp to surface bounds
                            let sx = x0.max(0.0) as u32;
                            let sy = y0.max(0.0) as u32;
                            let sw = (x1 - x0).max(1.0).min(config.width as f32) as u32;
                            let sh = (y1 - y0).max(1.0).min(config.height as f32) as u32;
                            (sx, sy, sw, sh)
                        } else {
                            // No damage at all — skip rendering entirely
                            return Ok(());
                        };

                        // ── Render with scissor rect for partial redraw ──
                        let total_verts = vertex_data.len() as u32;
                        {
                            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("composite pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(self.clear_color),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: None,
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });
                            // Apply scissor rect for partial redraw
                            rpass.set_scissor_rect(scissor_x, scissor_y, scissor_w, scissor_h);
                            if total_verts > 0 {
                                rpass.set_pipeline(pipeline);
                                rpass.set_vertex_buffer(0, self.vertex_buf.slice(..));
                                rpass.draw(0..total_verts, 0..1);
                            }
                        }

            self.queue.submit(std::iter::once(encoder.finish()));
            frame.present();
            Ok(())
        }
    }