use std::sync::Arc;

/// Unique identifier for a drawable layer.
pub type LayerId = u64;

/// A pointer event within the compositor's coordinate space.
#[derive(Debug, Clone, Copy)]
pub struct PointerEvent {
    pub x: f64,
    pub y: f64,
    pub kind: PointerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Move,
    Down,
    Up,
}

/// A keyboard event (simple char-based).
#[derive(Debug, Clone, Copy)]
pub struct KeyboardEvent {
    pub key: char,
    pub pressed: bool,
}

/// A key event with physical key code and optional text.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key_code: u32,
    pub pressed: bool,
    pub text: Option<String>,
}

/// Layer descriptor used when creating or updating a layer.
#[derive(Debug, Clone, Copy)]
pub struct LayerDescriptor {
    pub rect: [f32; 4],  // [x, y, w, h]
    pub color: [f32; 4], // [r, g, b, a]
    pub z: u32,
}

/// All inbound messages multiplexed onto the core loop.
#[derive(Debug, Clone)]
pub enum Message {
    CreateLayer {
        id: LayerId,
        desc: LayerDescriptor,
    },
    UpdateLayer {
        id: LayerId,
        color: Option<[f32; 4]>,
        rect: Option<[f32; 4]>,
    },
    RemoveLayer {
        id: LayerId,
    },
    SetLayerZ {
        id: LayerId,
        z: u32,
    },
    PointerInput(Arc<PointerEvent>),
    KeyboardInput(Arc<KeyboardEvent>),
        KeyEvent(Arc<KeyEvent>),
    Resize {
        width: u32,
        height: u32,
    },
    Shutdown,
}