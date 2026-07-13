use crate::engine::drawable_store::DrawableStore;
use crate::engine::telemetry::FrameStats;

/// Result of the compositing pass for one frame.
pub struct CompositorOutput {
    /// List of (layer_id, screen_rect, color) to render, in back-to-front order.
    pub quads: Vec<(u64, [f32; 4], [f32; 4])>,
    /// Dirty rects in screen space, or empty if full redraw.
    pub damage_rects: Option<Vec<[f32; 4]>>,
    pub full_redraw: bool,
}

/// Minimal scene compositor (cf. Yserver SceneCompositor).
///
/// Walks the layer list in z-order, collects visible quads, and
/// determines what regions need repainting.
///
/// Phase 4 additions:
///   - Opacity blending (layer color alpha is already handled by
///     the renderer's alpha blending)
///   - Visible region culling: layers outside the surface are skipped.
///   - Clipping: each quad is clipped against the surface bounds.
pub struct Compositor {
        surface_width: f32,
        surface_height: f32,
}

impl Compositor {
        pub fn new(width: f32, height: f32) -> Self {
            Self {
                surface_width: width,
                surface_height: height,
            }
        }

        /// Update the compositor's surface dimensions.
        pub fn resize(&mut self, w: f32, h: f32) {
            self.surface_width = w;
            self.surface_height = h;
    }

        /// Build the draw list for the current frame.
        ///
        /// Applies:
        ///   - Z-order sort (already done by DrawableStore::sorted_layers)
        ///   - Visible region culling (skip layers fully outside surface)
        ///   - Boundary clipping (clamp quad to surface bounds)
        ///   - Opacity pass-through (color alpha is used by the shader)
        pub fn compose(&self, store: &DrawableStore) -> CompositorOutput {
            let layers = store.sorted_layers();
            let damage = store.merged_damage_rects();

            let mut quads = Vec::with_capacity(layers.len());
            for layer in &layers {
                // ── Visible region culling ──
                let clipped = self.clip_rect(layer.rect);
                if clipped.is_none() {
                    continue;
                }
                quads.push((layer.id, clipped.unwrap(), layer.color));
            }

            let full_redraw = damage.is_none();
            CompositorOutput {
                quads,
                damage_rects: damage,
                full_redraw,
            }
        }

        /// Clip a rectangle against the surface bounds.
        /// Returns `None` if the rect is completely outside the surface.
        fn clip_rect(&self, rect: [f32; 4]) -> Option<[f32; 4]> {
            let x0 = rect[0].max(0.0);
            let y0 = rect[1].max(0.0);
            let x1 = (rect[0] + rect[2]).min(self.surface_width);
            let y1 = (rect[1] + rect[3]).min(self.surface_height);
            if x0 >= x1 || y0 >= y1 {
                None
            } else {
                Some([x0, y0, x1 - x0, y1 - y0])
            }
        }

        /// Gather stats for telemetry.
        pub fn collect_stats(&self, output: &CompositorOutput, _store: &DrawableStore) -> FrameStats {
            FrameStats {
                frame_count: 1,
                layer_count: output.quads.len(),
                dirty_rect_count: output
                    .damage_rects
                    .as_ref()
                    .map_or(0, |v| v.len() as u32),
                full_redraw: output.full_redraw,
                ..FrameStats::default()
            }
        }
}

#[cfg(test)]
mod tests {
        use super::*;

        #[test]
        fn test_clip_rect_fully_inside() {
            let comp = Compositor::new(800.0, 600.0);
            let r = comp.clip_rect([10.0, 10.0, 100.0, 100.0]);
            assert_eq!(r, Some([10.0, 10.0, 100.0, 100.0]));
        }

        #[test]
        fn test_clip_rect_partial() {
            let comp = Compositor::new(800.0, 600.0);
            let r = comp.clip_rect([-50.0, 10.0, 200.0, 100.0]);
            assert_eq!(r, Some([0.0, 10.0, 150.0, 100.0]));
        }

        #[test]
        fn test_clip_rect_fully_outside() {
            let comp = Compositor::new(800.0, 600.0);
            let r = comp.clip_rect([-100.0, 10.0, 50.0, 100.0]);
            assert_eq!(r, None);
        }
}