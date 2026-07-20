use crate::engine::drawable_store::DrawableStore;

/// EngineState — single-threaded core state of the engine.
///
/// This struct is the analogue of Yserver's `ServerState`. It owns a
/// `DrawableStore` and the current surface dimensions. The surface size
/// is mirrored in the store but kept here for direct access from the
/// core loop.
pub struct EngineState {
    pub store: DrawableStore,
}

impl EngineState {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            store: DrawableStore::new(width, height),
        }
    }

    pub fn surface_size(&self) -> (f32, f32) {
        self.store.surface_size()
    }
}