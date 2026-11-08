//! UI widgets: chunky cartoon buttons with press animation and vector icons.
#![allow(dead_code)]

use crate::art::{self, INK};
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};
use std::f32::consts::PI;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    None,
    Play,
    Pause,
    Back,
    Retry,
    Next,
    Menu,
    Gear,
    Trophy,
    Music(bool),
    Sound(bool),
    Haptic(bool),
    Close,
    Check,
    Left,
    Right,
    Up,
    Down,
    Clock,
    Lock,
    Star,
    Info,
    Calendar,
}

#[derive(Clone, Debug)]
pub struct Button {
    pub pos: V2,
    pub w: f32,
    pub h: f32,
    pub label: String,
    pub icon: Icon,
    pub color: Col,
    pub enabled: bool,
    pub visible: bool,
    pressed_by: Option<i32>,
    press_t: f32,
    pub text_size: f32,
}

impl Button {
    pub fn new(x: f32, y: f32, w: f32, h: f32, label: &str, icon: Icon, color: Col) -> Button {
        Button {
            pos: v2(x, y),
            w,
            h,
            label: label.to_string(),
            icon,
            color,
            enabled: true,
            visible: true,
            pressed_by: None,
            press_t: 0.0,
            text_size: (h * 0.42).min(40.0),
        }
    }
    pub fn round(x: f32, y: f32, d: f32, icon: Icon, color: Col) -> Button {
        Button::new(x, y, d, d, "", icon, color)
    }
    pub fn at(mut self, x: f32, y: f32) -> Button {
        self.pos = v2(x, y);
        self
    }
    pub fn hit(&self, p: V2) -> bool {
        let pad = 10.0;
        (p.x - self.pos.x).abs() <= self.w * 0.5 + pad && (p.y - self.pos.y).abs() <= self.h * 0.5 + pad
    }
    pub fn is_pressed(&self) -> bool {
        self.pressed_by.is_some()
    }
    pub fn reset(&mut self) {
        self.pressed_by = None;
    }

    /// Returns true when a press is released over the button (a click).
    pub fn touch(&mut self, t: &Touch) -> bool {
        if !self.enabled || !self.visible {
            self.pressed_by = None;
            return false;
        }
        match t.phase {
            Phase::Down => {
                if self.pressed_by.is_none() && self.hit(t.pos) {
                    self.pressed_by = Some(t.id);
                    crate::audio::play(crate::audio::Sfx::Click);
                    return false;
                }
            }
            Phase::Move => {
                if self.pressed_by == Some(t.id) && !self.hit(t.pos) {
                    self.pressed_by = None;
                }
            }
            Phase::Up => {
                if self.pressed_by == Some(t.id) {
                    self.pressed_by = None;
                    return self.hit(t.pos);
                }
            }
            Phase::Cancel => {
                if self.pressed_by == Some(t.id) {
                    self.pressed_by = None;
                }
            }
        }
        false
    }

    /// Hold-style button (e.g. thrust): true while a finger is down on it.
    pub fn touch_hold(&mut self, t: &Touch) -> bool {
        match t.phase {
            Phase::Down => {
                if self.hit(t.pos) {
                    self.pressed_by = Some(t.id);
                    return true;
                }
            }
            Phase::Up | Phase::Cancel => {
                if self.pressed_by == Some(t.id) {
                    self.pressed_by = None;
                }
            }
            Phase::Move => {}
        }
        false
    }

    pub fn update(&mut self, dt: f32) {
        let target = if self.pressed_by.is_some() { 1.0 } else { 0.0 };
        self.press_t += (target - self.press_t) * (dt * 25.0).min(1.0);
    }

    pub fn draw(&self, r: &mut Renderer) {
        if !self.visible {
            return;
        }
        let p = self.press_t;
        let depth = 7.0;
        let (x, y) = (self.pos.x, self.pos.y + p * depth * 0.7);
        let col = if self.enabled { self.color } else { hex(0x9AA5A0) };
        let rad = (self.h * 0.5).min(self.w * 0.5).min(36.0);
        let rad = if self.w == self.h { self.w * 0.5 } else { rad };
        // base / 3D edge
        r.rrect(self.pos.x, self.pos.y + depth, self.w, self.h, rad, col.shade(-0.45), INK, 4.0);
        r.rrect_grad(x, y, self.w, self.h, rad, col.shade(0.18), col.shade(-0.05), INK, 4.0);
        // gloss
        r.rrect(x, y - self.h * 0.22, self.w - 18.0, self.h * 0.32, rad * 0.6, Col::WHITE.a(0.22), Col::CLEAR, 0.0);
        let has_label = !self.label.is_empty();
        let icon_size = if has_label { self.h * 0.42 } else { self.h * 0.5 };
        let mut lx = x;
        if self.icon != Icon::None {
            let ix = if has_label { x - self.w * 0.5 + self.h * 0.5 } else { x };
            draw_icon(r, self.icon, ix, y, icon_size, Col::WHITE);
            if has_label {
                lx = x + self.h * 0.22;
            }
        }
        if has_label {
            r.text_bold(&self.label, lx, y, self.text_size, Align::Center, Col::WHITE, INK);
        }
    }
}

/// Draws a white glyph with a dark outline for the given icon, centred at (x, y).
pub fn draw_icon(r: &mut Renderer, icon: Icon, x: f32, y: f32, s: f32, c: Col) {
    let o = INK;
    let lw = s * 0.12;
    let thick = |r: &mut Renderer, a: V2, b: V2, w: f32| {
        r.line(a, b, w + lw * 2.0, o);
    };
    let fill = |r: &mut Renderer, a: V2, b: V2, w: f32| {
        r.line(a, b, w, c);
    };
    match icon {
        Icon::None => {}
        Icon::Play | Icon::Next | Icon::Right | Icon::Left | Icon::Up | Icon::Down | Icon::Back => {
            let ang = match icon {
                Icon::Left => PI,
                Icon::Up => -PI * 0.5,
                Icon::Down => PI * 0.5,
                _ => 0.0,
            };
            r.push();
            r.translate(x, y);
            r.rotate(ang);
            let k = s * 0.5;
            if icon == Icon::Back {
                let a = v2(-k * 0.9, 0.0);
                thick(r, a, v2(k * 0.9, 0.0), s * 0.16);
                thick(r, a, v2(-k * 0.1, -k * 0.75), s * 0.16);
                thick(r, a, v2(-k * 0.1, k * 0.75), s * 0.16);
                fill(r, a, v2(k * 0.9, 0.0), s * 0.16);
                fill(r, a, v2(-k * 0.1, -k * 0.75), s * 0.16);
                fill(r, a, v2(-k * 0.1, k * 0.75), s * 0.16);
            } else {
                let (p0, p1, p2) = (v2(-k * 0.6, -k * 0.85), v2(k * 0.85, 0.0), v2(-k * 0.6, k * 0.85));
                let grow = lw * 1.6;
                r.tri(p0 + v2(-grow * 0.7, -grow), p1 + v2(grow * 1.4, 0.0), p2 + v2(-grow * 0.7, grow), o);
                r.tri(p0, p1, p2, c);
                if icon == Icon::Next {
                    r.rrect(k * 0.95, 0.0, s * 0.16 + lw * 2.0, s * 0.85 + lw * 2.0, s * 0.08, o, o, 0.0);
                    r.rrect(k * 0.95, 0.0, s * 0.16, s * 0.85, s * 0.08, c, c, 0.0);
                }
            }
            r.pop();
        }
        Icon::Pause => {
            for dx in [-s * 0.2, s * 0.2] {
                r.rrect(x + dx, y, s * 0.24, s * 0.8, s * 0.08, c, o, lw);
            }
        }
        Icon::Retry => {
            r.ring(x, y, s * 0.38, s * 0.15 + lw * 2.0, o);
            r.ring(x, y - 0.0, s * 0.38 - lw, s * 0.15, c);
            r.rrect(x + s * 0.12, y - s * 0.38, s * 0.3, s * 0.2, 2.0, art::INK.a(0.0), Col::CLEAR, 0.0);
            let tip = v2(x + s * 0.38, y - s * 0.02);
            r.tri(tip + v2(-s * 0.28, -s * 0.02), tip + v2(s * 0.28, -s * 0.02), tip + v2(0.0, s * 0.3), o);
            r.tri(tip + v2(-s * 0.2, 0.0), tip + v2(s * 0.2, 0.0), tip + v2(0.0, s * 0.22), c);
        }
        Icon::Menu => {
            for i in 0..3 {
                for j in 0..2 {
                    r.rrect(x - s * 0.22 + j as f32 * s * 0.44, y - s * 0.28 + i as f32 * s * 0.28, s * 0.36, s * 0.2, s * 0.06, c, o, lw * 0.8);
                }
            }
        }
        Icon::Gear => {
            for i in 0..8 {
                let a = i as f32 * PI / 4.0;
                r.rrect_rot(x + a.cos() * s * 0.36, y + a.sin() * s * 0.36, s * 0.2, s * 0.24, s * 0.05, a, c, o, lw);
            }
            r.circle(x, y, s * 0.36, c, o, lw);
            r.circle(x, y, s * 0.14, o, o, 0.0);
        }
        Icon::Trophy => {
            r.rrect(x, y + s * 0.38, s * 0.56, s * 0.14, s * 0.05, c, o, lw);
            r.rrect(x, y + s * 0.22, s * 0.14, s * 0.24, 2.0, c, o, lw);
            r.ring(x - s * 0.3, y - s * 0.12, s * 0.15, lw * 1.5, o);
            r.ring(x + s * 0.3, y - s * 0.12, s * 0.15, lw * 1.5, o);
            r.ellipse(x, y - s * 0.1, s * 0.3, s * 0.32, c, o, lw);
            r.rect(x - s * 0.3, y - s * 0.45, s * 0.6, s * 0.1, c);
        }
        Icon::Music(on) => {
            r.ellipse(x - s * 0.22, y + s * 0.26, s * 0.16, s * 0.12, c, o, lw);
            r.ellipse(x + s * 0.24, y + s * 0.18, s * 0.16, s * 0.12, c, o, lw);
            r.line_outlined(v2(x - s * 0.1, y + s * 0.24), v2(x - s * 0.1, y - s * 0.34), s * 0.1, c, o, lw * 0.7);
            r.line_outlined(v2(x + s * 0.36, y + s * 0.16), v2(x + s * 0.36, y - s * 0.42), s * 0.1, c, o, lw * 0.7);
            r.line_outlined(v2(x - s * 0.1, y - s * 0.34), v2(x + s * 0.36, y - s * 0.42), s * 0.14, c, o, lw * 0.7);
            if !on {
                cross(r, x, y, s);
            }
        }
        Icon::Sound(on) => {
            r.rrect(x - s * 0.28, y, s * 0.22, s * 0.3, s * 0.04, c, o, lw);
            r.tri(v2(x - s * 0.25, y), v2(x + s * 0.06, y - s * 0.36), v2(x + s * 0.06, y + s * 0.36), o);
            r.tri(v2(x - s * 0.2, y), v2(x + s * 0.02, y - s * 0.28), v2(x + s * 0.02, y + s * 0.28), c);
            if on {
                r.ring(x + s * 0.08, y, s * 0.24, lw * 1.2, c);
                r.ring(x + s * 0.08, y, s * 0.38, lw * 1.2, c);
                r.rect(x - s * 0.4, y - s * 0.5, s * 0.46, s, Col::CLEAR);
            } else {
                cross(r, x + s * 0.2, y, s * 0.6);
            }
        }
        Icon::Haptic(on) => {
            r.rrect(x, y, s * 0.36, s * 0.66, s * 0.08, c, o, lw);
            r.rrect(x, y, s * 0.2, s * 0.42, s * 0.04, o.a(0.6), Col::CLEAR, 0.0);
            for side in [-1.0, 1.0] {
                r.line(v2(x + side * s * 0.3, y - s * 0.16), v2(x + side * s * 0.38, y - s * 0.04), lw * 1.2, c);
                r.line(v2(x + side * s * 0.38, y - s * 0.04), v2(x + side * s * 0.3, y + s * 0.08), lw * 1.2, c);
            }
            if !on {
                cross(r, x, y, s);
            }
        }
        Icon::Close => cross_c(r, x, y, s, c),
        Icon::Check => {
            let a = v2(x - s * 0.3, y);
            let b = v2(x - s * 0.08, y + s * 0.24);
            let d = v2(x + s * 0.34, y - s * 0.26);
            thick(r, a, b, s * 0.16);
            thick(r, b, d, s * 0.16);
            fill(r, a, b, s * 0.16);
            fill(r, b, d, s * 0.16);
        }
        Icon::Clock => art::clock_icon(r, x, y + s * 0.05, s * 0.4, 1.0),
        Icon::Lock => {
            r.ring(x, y - s * 0.12, s * 0.22, s * 0.1, o);
            r.ring(x, y - s * 0.12, s * 0.2, s * 0.06, c.shade(-0.2));
            r.rrect(x, y + s * 0.14, s * 0.6, s * 0.44, s * 0.08, c, o, lw);
            r.disc(x, y + s * 0.1, s * 0.06, o);
            r.rect(x - s * 0.02, y + s * 0.1, s * 0.04, s * 0.12, o);
        }
        Icon::Star => art::star_icon(r, x, y, s * 0.5, true),
        Icon::Calendar => {
            r.rrect(x, y + s * 0.06, s * 0.8, s * 0.72, s * 0.1, c, o, lw);
            r.rrect(x, y - s * 0.2, s * 0.8, s * 0.2, s * 0.08, hex(0xF0605D), o, lw);
            for dx in [-0.22f32, 0.22] {
                r.rrect(x + dx * s, y - s * 0.34, s * 0.09, s * 0.2, s * 0.04, o, o, 0.0);
            }
            for i in 0..3 {
                for j in 0..2 {
                    r.disc(x - s * 0.2 + i as f32 * s * 0.2, y + s * 0.06 + j as f32 * s * 0.18, s * 0.055, o);
                }
            }
        }
        Icon::Info => {
            r.circle(x, y, s * 0.42, c, o, lw);
            r.text("i", x, y, s * 0.6, Align::Center, o);
        }
    }
}

fn cross(r: &mut Renderer, x: f32, y: f32, s: f32) {
    r.line(v2(x - s * 0.36, y - s * 0.36), v2(x + s * 0.36, y + s * 0.36), s * 0.14, hex(0xE8413C));
}
fn cross_c(r: &mut Renderer, x: f32, y: f32, s: f32, c: Col) {
    let k = s * 0.3;
    for (a, b) in [(v2(x - k, y - k), v2(x + k, y + k)), (v2(x + k, y - k), v2(x - k, y + k))] {
        r.line(a, b, s * 0.18 + 8.0, INK);
    }
    for (a, b) in [(v2(x - k, y - k), v2(x + k, y + k)), (v2(x + k, y - k), v2(x - k, y + k))] {
        r.line(a, b, s * 0.18, c);
    }
}

/// Rounded "card" panel used by dialogs.
pub fn panel(r: &mut Renderer, cx: f32, cy: f32, w: f32, h: f32, fill: Col) {
    r.rrect(cx, cy + 10.0, w, h, 36.0, Col(0, 0, 0, 70), Col::CLEAR, 0.0);
    r.rrect_grad(cx, cy, w, h, 36.0, fill.shade(0.12), fill.shade(-0.06), INK, 5.0);
    r.rrect(cx, cy, w - 26.0, h - 26.0, 26.0, Col::CLEAR, Col::WHITE.a(0.35), 3.0);
}

/// Dim the whole screen behind a modal.
pub fn dim(r: &mut Renderer, a: f32) {
    let (w, h) = (r.w, r.h);
    r.rect(0.0, 0.0, w, h, Col(8, 24, 18, (a * 255.0) as u8));
}
