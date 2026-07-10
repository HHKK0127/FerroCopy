//! FerroCopy Dot Design — particle-based UI components
//!
//! All UI elements are represented as collections of dots (particles),
//! inspired by the concept of "dots as the fundamental unit of visual design".
//! The space theme evokes a starry night sky.

#![allow(dead_code)]

use eframe::egui;

// ── Color Palette (Space Theme) ──────────────────────────────────────

pub const VOID: egui::Color32 = egui::Color32::from_rgb(8, 8, 16);
pub const DEEP_SPACE: egui::Color32 = egui::Color32::from_rgb(16, 16, 28);
pub const NEBULA: egui::Color32 = egui::Color32::from_rgb(24, 24, 40);
pub const STAR_DUST: egui::Color32 = egui::Color32::from_rgb(36, 36, 56);
pub const STAR_WHITE: egui::Color32 = egui::Color32::from_rgb(220, 230, 255);
pub const STAR_BLUE: egui::Color32 = egui::Color32::from_rgb(100, 180, 255);
pub const STAR_GOLD: egui::Color32 = egui::Color32::from_rgb(255, 200, 80);
pub const STAR_GREEN: egui::Color32 = egui::Color32::from_rgb(80, 220, 160);
pub const STAR_RED: egui::Color32 = egui::Color32::from_rgb(255, 100, 120);

// ── Simple deterministic pseudo-random (no extra deps) ────────────────

fn prng(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 33) as f32) / (u32::MAX as f32)
}

// ── Background Particle System ───────────────────────────────────────

pub struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    radius: f32,
    alpha: f32,
    alpha_speed: f32,
}

impl Particle {
    fn new(id: usize) -> Self {
        let mut seed = (id as u64).wrapping_mul(2654435761).wrapping_add(12345);
        let x = prng(&mut seed);
        let y = prng(&mut seed);
        Self {
            x: x * 800.0,
            y: y * 600.0,
            vx: (prng(&mut seed) - 0.5) * 0.4,
            vy: (prng(&mut seed) - 0.5) * 0.4,
            radius: prng(&mut seed) * 2.0 + 0.8,
            alpha: prng(&mut seed) * 0.4 + 0.1,
            alpha_speed: (prng(&mut seed) - 0.5) * 0.6,
        }
    }

    fn update(&mut self, dt: f32, w: f32, h: f32) {
        let s = dt * 60.0;
        self.x += self.vx * s;
        self.y += self.vy * s;
        self.alpha += self.alpha_speed * dt;
        if self.alpha <= 0.08 || self.alpha >= 0.5 {
            self.alpha_speed = -self.alpha_speed;
        }
        self.alpha = self.alpha.clamp(0.05, 0.55);
        if self.x < 0.0 {
            self.x = w;
        }
        if self.x > w {
            self.x = 0.0;
        }
        if self.y < 0.0 {
            self.y = h;
        }
        if self.y > h {
            self.y = 0.0;
        }
    }

    fn paint(&self, painter: &egui::Painter, rect: &egui::Rect) {
        let px = rect.left() + self.x * rect.width() / 800.0;
        let py = rect.top() + self.y * rect.height() / 600.0;
        let pos = egui::pos2(px, py);
        let color = egui::Color32::from_white_alpha((self.alpha * 255.0) as u8);
        painter.circle_filled(pos, self.radius, color);
    }
}

pub struct ParticleSystem {
    particles: Vec<Particle>,
    initialized: bool,
}

impl ParticleSystem {
    pub fn new(count: usize) -> Self {
        let particles = (0..count).map(Particle::new).collect();
        Self {
            particles,
            initialized: false,
        }
    }

    pub fn update(&mut self, dt: f32, rect: &egui::Rect) {
        if !self.initialized {
            let w = rect.width().max(800.0);
            let h = rect.height().max(600.0);
            for p in &mut self.particles {
                p.x *= w / 800.0;
                p.y *= h / 600.0;
            }
            self.initialized = true;
        }
        let w = rect.width().max(100.0);
        let h = rect.height().max(100.0);
        for p in &mut self.particles {
            p.update(dt, w, h);
        }
    }

    pub fn paint(&self, painter: &egui::Painter, rect: &egui::Rect) {
        for p in &self.particles {
            p.paint(painter, rect);
        }
    }
}

// ── Dot Progress Bar ─────────────────────────────────────────────────

/// A progress bar rendered as a row of dots.
/// Filled dots glow blue, current dot pulses, empty dots are dim.
pub fn dot_progress_bar(ui: &mut egui::Ui, ratio: f32, width: f32, paused: bool, time: f64) {
    let dot_count = 40;
    let dot_spacing = width / dot_count as f32;
    let dot_radius = dot_spacing * 0.35;
    let filled = (ratio * dot_count as f32) as usize;

    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width, dot_radius * 3.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);

    let pulse: f32 = if paused {
        0.0
    } else {
        ((time * 4.0).sin() * 0.3 + 0.7) as f32
    };

    for i in 0..dot_count {
        let x = rect.left() + (i as f32 + 0.5) * dot_spacing;
        let y = rect.center().y;
        let pos = egui::pos2(x, y);

        let (radius, color): (f32, egui::Color32) = if i < filled {
            let t = i as f32 / filled.max(1) as f32;
            let r = (100.0 + t * 120.0) as u8;
            let g = (180.0 + t * 50.0) as u8;
            let b = 255u8;
            (dot_radius, egui::Color32::from_rgb(r, g, b))
        } else if i == filled && filled < dot_count {
            let r = dot_radius * (pulse * 0.5 + 0.8);
            (r, egui::Color32::from_rgb(100, 200, 255))
        } else {
            (dot_radius * 0.6, egui::Color32::from_rgb(40, 50, 70))
        };

        let alpha: f32 = if i < filled {
            220.0
        } else if i == filled {
            180.0 + pulse * 75.0
        } else {
            80.0
        };
        let c = color.linear_multiply(alpha / 255.0);
        painter.circle_filled(pos, radius, c);
    }
}

// ── Dot Separator ────────────────────────────────────────────────────

/// A decorative line made of dots that shimmer subtly.
pub fn dot_separator(ui: &mut egui::Ui, time: f64) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 6.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);

    let dot_count = 60;
    let spacing = rect.width() / dot_count as f32;

    for i in 0..dot_count {
        let x = rect.left() + (i as f32 + 0.5) * spacing;
        let y = rect.center().y;
        let pos = egui::pos2(x, y);

        let phase: f32 = ((time * 2.0 + i as f64 * 0.15).sin() * 0.5 + 0.5) as f32;
        let alpha: u8 = (40.0 + phase * 80.0) as u8;
        let radius: f32 = 1.0 + phase * 0.8;

        painter.circle_filled(pos, radius, egui::Color32::from_white_alpha(alpha));
    }
}

// ── File Icon Dots ───────────────────────────────────────────────────

fn paint_dot(painter: &egui::Painter, x: f32, y: f32, r: f32, color: egui::Color32) {
    painter.circle_filled(egui::pos2(x, y), r, color.linear_multiply(0.8));
}

fn paint_dots(
    painter: &egui::Painter,
    x: f32,
    y: f32,
    pts: &[(f32, f32)],
    pulse: f32,
    color: egui::Color32,
) {
    for &(dx, dy) in pts {
        let r = 2.0 * pulse;
        painter.circle_filled(
            egui::pos2(x + dx * pulse, y + dy * pulse),
            r,
            color.linear_multiply(0.8),
        );
    }
}

/// Determine the dot pattern for a file based on its extension.
/// Renders a constellation at (x, y) with the appropriate color.
pub fn file_icon_dots(painter: &egui::Painter, name: &str, x: f32, y: f32, time: f64) {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let pulse: f32 = ((time * 3.0).sin() * 0.15 + 1.0) as f32;

    match ext.as_str() {
        "pdf" => {
            paint_dot(painter, x, y, 2.0 * pulse, STAR_RED);
        }
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => {
            let pts = [
                (0.0, -6.0),
                (6.0, -2.0),
                (4.0, 5.0),
                (-4.0, 5.0),
                (-6.0, -2.0),
            ];
            paint_dots(painter, x, y, &pts, pulse, STAR_BLUE);
        }
        "mp4" | "avi" | "mkv" | "mov" => {
            let angle = time * 2.0;
            let pts: [(f32, f32); 6] = std::array::from_fn(|i| {
                let a = angle + i as f64 * std::f64::consts::PI / 3.0;
                ((a.cos() as f32) * 6.0, (a.sin() as f32) * 6.0)
            });
            paint_dots(painter, x, y, &pts, pulse, STAR_GOLD);
        }
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => {
            let pts = [(-6.0, 4.0), (-3.0, 0.0), (0.0, -4.0), (3.0, 0.0), (6.0, 4.0)];
            paint_dots(painter, x, y, &pts, pulse, STAR_GREEN);
        }
        "zip" | "7z" | "rar" | "tar" | "gz" => {
            let mut pts = [(0.0, 0.0); 9];
            for (i, p) in pts.iter_mut().enumerate() {
                let row = (i as f32 / 3.0).floor() - 1.0;
                let col = (i as f32 % 3.0) - 1.0;
                *p = (col * 4.0, row * 4.0);
            }
            paint_dots(painter, x, y, &pts, pulse, STAR_RED);
        }
        "ppt" | "pptx" => {
            let pts = [
                (0.0, -7.0),
                (5.0, -5.0),
                (7.0, 0.0),
                (5.0, 5.0),
                (0.0, 7.0),
                (-5.0, 5.0),
                (-7.0, 0.0),
                (-5.0, -5.0),
            ];
            paint_dots(painter, x, y, &pts, pulse, STAR_GOLD);
        }
        "xls" | "xlsx" | "csv" => {
            let pts = [
                (-6.0, 0.0),
                (-2.0, 0.0),
                (2.0, 0.0),
                (6.0, 0.0),
                (0.0, -6.0),
                (0.0, -2.0),
                (0.0, 2.0),
                (0.0, 6.0),
            ];
            paint_dots(painter, x, y, &pts, pulse, STAR_GREEN);
        }
        "exe" | "dll" | "msi" => {
            let pts = [
                (-6.0, -6.0),
                (6.0, -6.0),
                (6.0, 6.0),
                (-6.0, 6.0),
                (-3.0, -3.0),
                (3.0, -3.0),
                (3.0, 3.0),
                (-3.0, 3.0),
            ];
            paint_dots(painter, x, y, &pts, pulse, STAR_BLUE);
        }
        _ => {
            paint_dot(painter, x, y, 2.0 * pulse, STAR_WHITE);
        }
    }
}

// ── Dot Checkbox ─────────────────────────────────────────────────────

/// A checkbox rendered as dot patterns.
/// Unchecked: 4 dots in a square. Checked: radial star burst.
pub fn dot_checkbox(ui: &mut egui::Ui, checked: &mut bool, time: f64) -> bool {
    let size = egui::vec2(20.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter_at(rect);

    if response.clicked() {
        *checked = !*checked;
        return true;
    }

    let cx = rect.center().x;
    let cy = rect.center().y;

    if *checked {
        let count = 8;
        let pulse: f32 = ((time * 5.0).sin() * 0.3 + 0.7) as f32;
        for i in 0..count {
            let angle = i as f64 * std::f64::consts::PI * 2.0 / count as f64;
            let r: f32 = (5.0 + pulse * 3.0) * (0.8 + (i as f32 / count as f32) * 0.2);
            let x = cx + (angle.cos() as f32) * r;
            let y = cy + (angle.sin() as f32) * r;
            let alpha: u8 = (120.0 + pulse * 80.0) as u8;
            let color = egui::Color32::from_rgba_premultiplied(100, 180, 255, alpha);
            painter.circle_filled(egui::pos2(x, y), 1.8, color);
        }
        painter.circle_filled(egui::pos2(cx, cy), 2.5, STAR_BLUE);
        for i in 0..6 {
            let angle = time * 3.0 + i as f64 * std::f64::consts::PI / 3.0;
            let r = 9.0_f32;
            let x = cx + (angle.cos() as f32) * r;
            let y = cy + (angle.sin() as f32) * r;
            painter.circle_filled(egui::pos2(x, y), 1.0, STAR_BLUE.linear_multiply(0.4));
        }
    } else {
        let gap = 4.0_f32;
        let dots = [(-gap, -gap), (gap, -gap), (-gap, gap), (gap, gap)];
        for &(dx, dy) in &dots {
            painter.circle_filled(
                egui::pos2(cx + dx, cy + dy),
                1.5,
                egui::Color32::from_rgb(80, 100, 130),
            );
        }
    }

    false
}

// ── Dot Button ───────────────────────────────────────────────────────

/// A button rendered as a grid of dots that brightens and densifies on hover.
/// Returns true if clicked.
pub fn dot_button(ui: &mut egui::Ui, label: &str, enabled: bool, time: f64) -> bool {
    let text_color = if enabled {
        STAR_WHITE
    } else {
        egui::Color32::from_rgb(80, 80, 100)
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(110.0, 30.0), egui::Sense::click());
    let painter = ui.painter_at(rect);
    let hovered = response.hovered() && enabled;

    // Background dot grid
    let cols = 18;
    let rows = 5;
    let spacing_x = rect.width() / cols as f32;
    let spacing_y = rect.height() / rows as f32;

    for row in 0..rows {
        for col in 0..cols {
            let x = rect.left() + (col as f32 + 0.5) * spacing_x;
            let y = rect.top() + (row as f32 + 0.5) * spacing_y;
            let pos = egui::pos2(x, y);

            let (radius, alpha): (f32, u8) = if hovered {
                let pulse: f32 =
                    ((time * 6.0 + (row * cols + col) as f64 * 0.3).sin() * 0.3 + 0.7) as f32;
                (1.5 + pulse * 0.8, (80.0 + pulse * 120.0) as u8)
            } else {
                (1.0, 30u8)
            };

            let color = egui::Color32::from_rgba_premultiplied(100, 180, 255, alpha);
            painter.circle_filled(pos, radius, color);
        }
    }

    // Label text on top
    let galley = painter.layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(13.0),
        text_color,
    );
    let text_pos = egui::pos2(
        rect.center().x - galley.size().x / 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_pos, galley, text_color);

    response.clicked() && enabled
}
