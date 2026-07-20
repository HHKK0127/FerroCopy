//! Input abstraction layer for QuadComp.
//!
//! Translates winit input events into the engine's message protocol.
//! Supports keyboard, mouse, and touch input sources.

use std::sync::Arc;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::PhysicalKey;

use crate::engine::message::{
    KeyboardEvent, KeyEvent, Message, PointerEvent, PointerKind,
};

/// Translate a winit `WindowEvent` into zero or more `Message`s.
pub fn translate_window_event(event: &WindowEvent) -> Vec<Message> {
    match event {
        WindowEvent::CursorMoved { position, .. } => {
            vec![Message::PointerInput(Arc::new(PointerEvent {
                x: position.x,
                y: position.y,
                kind: PointerKind::Move,
            }))]
        }
        WindowEvent::MouseInput { state, button, .. } => {
            let kind = match (state, button) {
                (ElementState::Pressed, MouseButton::Left) => PointerKind::Down,
                (ElementState::Released, MouseButton::Left) => PointerKind::Up,
                _ => return vec![],
            };
            vec![Message::PointerInput(Arc::new(PointerEvent {
                x: 0.0,
                y: 0.0,
                kind,
            }))]
        }
        WindowEvent::KeyboardInput {
            event,
            is_synthetic: false,
            ..
        } => {
            let key_code_val = match event.physical_key {
                PhysicalKey::Code(kc) => kc as u32,
                _ => 0,
            };
            let pressed = event.state == ElementState::Pressed;
            let key_char = key_code_to_char(key_code_val);

            vec![
                Message::KeyboardInput(Arc::new(KeyboardEvent {
                    key: key_char.unwrap_or('\0'),
                    pressed,
                })),
                Message::KeyEvent(Arc::new(KeyEvent {
                    key_code: key_code_val,
                    pressed,
                    text: key_char.map(|c| c.to_string()),
                })),
            ]
        }
        WindowEvent::CloseRequested => {
            vec![Message::Shutdown]
        }
        WindowEvent::Resized(size) => {
            vec![Message::Resize {
                width: size.width,
                height: size.height,
            }]
        }
        _ => vec![],
    }
}

/// Best-effort mapping from physical key code to a printable character.
fn key_code_to_char(code: u32) -> Option<char> {
    match code {
        0x2C => Some(' '),  // Space
        0x04..=0x1D => char::from_u32(b'a' as u32 + (code - 0x04) as u32),
        0x1E..=0x27 => char::from_u32(b'0' as u32 + (code - 0x1E) as u32),
        _ => None,
    }
}