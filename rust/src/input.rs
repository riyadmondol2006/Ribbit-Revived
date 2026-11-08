//! Touch event plumbing. Kotlin forwards raw pointer events; the engine converts them to
//! virtual-canvas coordinates and dispatches to the active scene.
use crate::math::V2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Clone, Copy, Debug)]
pub struct Touch {
    pub phase: Phase,
    pub id: i32,
    pub pos: V2,
}

impl Phase {
    pub fn from_i32(a: i32) -> Phase {
        match a {
            0 => Phase::Down,
            1 => Phase::Move,
            2 => Phase::Up,
            _ => Phase::Cancel,
        }
    }
}
