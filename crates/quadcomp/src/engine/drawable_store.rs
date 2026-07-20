use std::collections::HashMap;

use crate::engine::message::{LayerDescriptor, LayerId};

/// A drawable surface layer — analogous to a Yserver drawable object.
#[derive(Debug, Clone)]
pub struct Layer {
    pub id: LayerId,
    pub rect: [f32; 4],   // [x, y, w, h]
    pub color: [f32; 4],  // [r, g, b, a]
    pub z: u32,
    pub visible: bool,
    /// Bounding rectangle of the dirty region in logical pixels.
    /// `None` when the layer needs a full redraw.
    pub dirty_rect: Option<[f32; 4]>,
}

impl Layer {
    pub fn from_desc(id: LayerId, desc: LayerDescriptor) -> Self {
        Self {
            id,
            rect: desc.rect,
            color: desc.color,
            z: desc.z,
            visible: true,
            dirty_rect: None,
        }
    }

    /// Mark the entire layer as dirty (full redraw).
    pub fn mark_dirty_full(&mut self) {
        self.dirty_rect = None;
    }

    /// Mark a sub-region as dirty. Merges with any existing dirty rect.
    pub fn mark_dirty_region(&mut self, region: [f32; 4]) {
        let existing = self.dirty_rect.unwrap_or(region);
        let x0 = existing[0].min(region[0]);
        let y0 = existing[1].min(region[1]);
        let x1 = (existing[0] + existing[2]).max(region[0] + region[2]);
        let y1 = (existing[1] + existing[3]).max(region[1] + region[3]);
        self.dirty_rect = Some([x0, y0, x1 - x0, y1 - y0]);
    }
}

/// DrawableStore — owns all layers and tracks which regions are dirty.
///
/// This is the analogue of Yserver's DrawableStore + damage tracking.
/// It provides:
///   - Layer CRUD with automatic dirty marking
///   - Dirty rect collection and merging across all layers
///   - Full-redraw fallback when any layer has `dirty_rect == None`
///   - Look-up helpers for compositing and hit testing
#[derive(Debug)]
pub struct DrawableStore {
    pub layers: HashMap<LayerId, Layer>,
    next_layer_id: LayerId,
    surface_width: f32,
    surface_height: f32,
}

impl DrawableStore {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            layers: HashMap::new(),
            next_layer_id: 1,
            surface_width: width,
            surface_height: height,
        }
    }

    // ── Surface dimensions ──

    pub fn surface_size(&self) -> (f32, f32) {
        (self.surface_width, self.surface_height)
    }

    pub fn set_surface_size(&mut self, w: f32, h: f32) {
        self.surface_width = w;
        self.surface_height = h;
    }

    // ── ID allocation ──

    pub fn allocate_id(&mut self) -> LayerId {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        id
    }

    // ── Layer CRUD ──

    pub fn insert_layer(&mut self, desc: LayerDescriptor) -> LayerId {
        let id = self.allocate_id();
        let mut layer = Layer::from_desc(id, desc);
        layer.mark_dirty_full();
        self.layers.insert(id, layer);
        id
    }

    pub fn remove_layer(&mut self, id: LayerId) {
        self.layers.remove(&id);
    }

    pub fn get_layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(&id)
    }

    pub fn get_layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }

    pub fn update_layer(
        &mut self,
        id: LayerId,
        color: Option<[f32; 4]>,
        rect: Option<[f32; 4]>,
    ) {
        if let Some(layer) = self.layers.get_mut(&id) {
            if let Some(c) = color {
                layer.color = c;
            }
            if let Some(r) = rect {
                layer.rect = r;
            }
            layer.mark_dirty_full();
        }
    }

    // ── Z-order ──

    pub fn set_layer_z(&mut self, id: LayerId, z: u32) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.z = z;
            layer.mark_dirty_full();
        }
    }

    pub fn set_visibility(&mut self, id: LayerId, visible: bool) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.visible = visible;
            layer.mark_dirty_full();
        }
    }

    // ── Iteration / sorting ──

    /// Return layers sorted by z-order for compositing.
    pub fn sorted_layers(&self) -> Vec<&Layer> {
        let mut sorted: Vec<&Layer> = self.layers.values().filter(|l| l.visible).collect();
        sorted.sort_by_key(|l| l.z);
        sorted
    }

    // ── Damage tracking ──

    /// Collect all dirty screen-space rects from all layers, merged.
    /// Returns `None` if any layer needs a full redraw (dirty_rect == None).
    pub fn merged_damage_rects(&self) -> Option<Vec<[f32; 4]>> {
        let mut needs_full = false;
        let mut rects: Vec<[f32; 4]> = Vec::new();
        for layer in self.layers.values() {
            if !layer.visible {
                continue;
            }
            match layer.dirty_rect {
                None => needs_full = true,
                Some(r) => rects.push(r),
            }
        }
        if needs_full { None } else { Some(rects) }
    }

    /// Compute the total bounding box of all damage rects.
    /// Returns `None` if there is no damage or if a full redraw is needed.
    pub fn merged_damage_bounds(&self) -> Option<[f32; 4]> {
        match self.merged_damage_rects() {
            None => None,
            Some(rects) if rects.is_empty() => None,
            Some(rects) => {
                let mut x0 = f32::MAX;
                let mut y0 = f32::MAX;
                let mut x1 = f32::MIN;
                let mut y1 = f32::MIN;
                for r in &rects {
                    x0 = x0.min(r[0]);
                    y0 = y0.min(r[1]);
                    x1 = x1.max(r[0] + r[2]);
                    y1 = y1.max(r[1] + r[3]);
                }
                Some([x0, y0, x1 - x0, y1 - y0])
            }
        }
    }

    /// Clear all dirty flags after compositing.
    pub fn clear_dirty(&mut self) {
        for layer in self.layers.values_mut() {
            layer.dirty_rect = None;
        }
    }
}