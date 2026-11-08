//! Procedural art kit: the Ribbit frog character and every shared prop, drawn from SDF shapes.
#![allow(dead_code, clippy::too_many_arguments)]

use crate::math::*;
use crate::render::{Align, Renderer};
use std::f32::consts::{PI, TAU};

pub const INK: Col = hex(0x1E3A2F);
pub const SHADOW: Col = Col(0, 0, 0, 46);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Skin {
    pub body: Col,
    pub belly: Col,
    pub line: Col,
}

pub const EMERALD: Skin = Skin { body: hex(0x4CC38A), belly: hex(0xC8F2A0), line: hex(0x1E4D3A) };
pub const RUBY: Skin = Skin { body: hex(0xF0605D), belly: hex(0xFFD3C4), line: hex(0x6B1E24) };
pub const TANGERINE: Skin = Skin { body: hex(0xFF9F2E), belly: hex(0xFFE3A8), line: hex(0x6E3A07) };
pub const SKY: Skin = Skin { body: hex(0x4FA7F2), belly: hex(0xC9E8FF), line: hex(0x173E66) };
pub const BLOSSOM: Skin = Skin { body: hex(0xF27BC4), belly: hex(0xFFD6F0), line: hex(0x6A1E4E) };
pub const GRAPE: Skin = Skin { body: hex(0x9A6BEA), belly: hex(0xE2D4FF), line: hex(0x36206B) };
pub const LEMON: Skin = Skin { body: hex(0xF4D03F), belly: hex(0xFFF6C2), line: hex(0x6B5310) };
pub const SLATE: Skin = Skin { body: hex(0x7D8B99), belly: hex(0xD9E1E8), line: hex(0x2C3540) };

pub const RAINBOW: [Skin; 6] = [RUBY, TANGERINE, LEMON, EMERALD, SKY, GRAPE];

static MARKS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Colour-blind helper: when on, every coloured frog (and Ball Swap ball) carries a shape.
pub fn set_color_marks(on: bool) {
    MARKS.store(on, std::sync::atomic::Ordering::Relaxed);
}
pub fn color_marks() -> bool {
    MARKS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Draws the shape that stands for a colour (matched by body colour), centred at (x, y).
pub fn color_mark(r: &mut Renderer, x: f32, y: f32, s: f32, body: Col) {
    let ink = Col(25, 30, 40, 235);
    let halo = Col::WHITE.a(0.9);
    let shapes = [EMERALD.body, RUBY.body, TANGERINE.body, SKY.body, BLOSSOM.body, GRAPE.body, LEMON.body, SLATE.body];
    let idx = shapes.iter().position(|&c| c == body).unwrap_or(8);
    r.circle(x, y, s * 1.15, halo, halo, 0.0);
    match idx {
        0 => r.circle(x, y, s * 0.8, ink, ink, 0.0),
        1 => r.tri(v2(x, y - s * 0.9), v2(x + s * 0.85, y + s * 0.65), v2(x - s * 0.85, y + s * 0.65), ink),
        2 => r.rrect(x, y, s * 1.4, s * 1.4, s * 0.15, ink, ink, 0.0),
        3 => r.rrect_rot(x, y, s * 1.2, s * 1.2, s * 0.1, PI * 0.25, ink, ink, 0.0),
        4 => {
            r.circle(x - s * 0.38, y - s * 0.2, s * 0.45, ink, ink, 0.0);
            r.circle(x + s * 0.38, y - s * 0.2, s * 0.45, ink, ink, 0.0);
            r.tri(v2(x - s * 0.8, y), v2(x + s * 0.8, y), v2(x, y + s * 0.85), ink);
        }
        5 => r.star(x, y + s * 0.05, s * 0.95, 0.0, ink, ink, 0.0),
        6 => {
            r.line(v2(x - s * 0.6, y - s * 0.6), v2(x + s * 0.6, y + s * 0.6), s * 0.38, ink);
            r.line(v2(x + s * 0.6, y - s * 0.6), v2(x - s * 0.6, y + s * 0.6), s * 0.38, ink);
        }
        7 => {
            r.rrect(x, y, s * 1.5, s * 0.42, s * 0.2, ink, ink, 0.0);
            r.rrect(x, y, s * 0.42, s * 1.5, s * 0.2, ink, ink, 0.0);
        }
        _ => r.ring(x, y, s * 0.65, s * 0.35, ink),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    /// 0 open .. 1 closed
    pub blink: f32,
    /// 0 smile .. 1 wide open
    pub mouth: f32,
    /// vertical squash (-0.3 stretched .. 0.3 squashed)
    pub squash: f32,
    /// pupil direction, unit-ish
    pub look: V2,
    pub dizzy: bool,
    pub happy_eyes: bool,
    pub sprout: bool,
    pub legs_out: f32,
}

impl Default for Pose {
    fn default() -> Pose {
        Pose { blink: 0.0, mouth: 0.0, squash: 0.0, look: v2(0.0, 0.0), dizzy: false, happy_eyes: false, sprout: false, legs_out: 0.0 }
    }
}

impl Pose {
    pub fn idle(t: f32, seed: f32) -> Pose {
        let cycle = (t + seed * 3.7) % 3.4;
        let blink = if cycle < 0.14 { (cycle / 0.07 * PI * 0.5).sin().abs().min(1.0) } else { 0.0 };
        let look = v2(((t * 0.6 + seed).sin() * 0.6).clamp(-1.0, 1.0), ((t * 0.43 + seed * 2.0).cos() * 0.3).clamp(-1.0, 1.0));
        Pose { blink, look, squash: (t * 3.0 + seed).sin() * 0.025, ..Pose::default() }
    }
}

/// Draws a frog whose body is ~`size` wide, feet resting on (x, y + size*0.46).
pub fn frog(r: &mut Renderer, x: f32, y: f32, size: f32, skin: Skin, pose: &Pose) {
    let s = size / 100.0;
    r.push();
    r.translate(x, y);
    r.scale(s, s);
    let lw = 4.0;
    let sq = pose.squash;
    // shadow
    r.ellipse(0.0, 47.0, 40.0 + sq * 30.0, 8.0, SHADOW, SHADOW, 0.0);
    r.push();
    r.translate(0.0, 46.0);
    r.scale(1.0 + sq, 1.0 - sq);
    r.translate(0.0, -46.0);
    let dark = skin.body.shade(-0.18);
    // back legs
    let lo = pose.legs_out * 18.0;
    r.ellipse(-30.0 - lo, 40.0, 17.0, 9.0, dark, skin.line, lw);
    r.ellipse(30.0 + lo, 40.0, 17.0, 9.0, dark, skin.line, lw);
    // body
    r.ellipse_grad(0.0, 12.0, 46.0, 37.0, skin.body.shade(0.08), skin.body.shade(-0.06), skin.line, lw);
    // belly
    r.ellipse(0.0, 24.0, 28.0, 19.0, skin.belly, skin.belly, 0.0);
    if color_marks() {
        color_mark(r, 0.0, 26.0, 14.0, skin.body);
    }
    // little arms
    r.ellipse_rot(-24.0, 36.0, 9.0, 6.0, -0.4, skin.body, skin.line, 3.5);
    r.ellipse_rot(24.0, 36.0, 9.0, 6.0, 0.4, skin.body, skin.line, 3.5);
    // cheeks
    r.ellipse(-31.0, 15.0, 7.5, 4.5, hex(0xFF8FA3).a(0.65), Col::CLEAR, 0.0);
    r.ellipse(31.0, 15.0, 7.5, 4.5, hex(0xFF8FA3).a(0.65), Col::CLEAR, 0.0);
    // mouth
    if pose.mouth > 0.05 {
        let h = 4.0 + pose.mouth * 12.0;
        r.ellipse(0.0, 14.0 + h * 0.3, 13.0 + pose.mouth * 4.0, h, hex(0x5A1A22), skin.line, 3.0);
        r.ellipse(0.0, 14.0 + h * 0.8, 8.0, h * 0.45, hex(0xFF6F8B), Col::CLEAR, 0.0);
    } else {
        r.ellipse(0.0, 12.0, 13.0, 7.0, skin.line, skin.line, 0.0);
        r.ellipse(0.0, 8.5, 14.5, 6.5, skin.body.shade(0.03), skin.body, 0.0);
    }
    // eyes
    for side in [-1.0f32, 1.0] {
        let ex = side * 22.0;
        let ey = -20.0;
        r.circle(ex, ey, 17.0, skin.body.shade(0.08), skin.line, lw);
        if pose.happy_eyes {
            // closed "^" eyes
            r.line(v2(ex - 7.0, ey + 2.0), v2(ex, ey - 4.0), 4.0, skin.line);
            r.line(v2(ex, ey - 4.0), v2(ex + 7.0, ey + 2.0), 4.0, skin.line);
            continue;
        }
        r.circle(ex, ey, 12.0, Col::WHITE, Col::WHITE, 0.0);
        if pose.dizzy {
            r.ring(ex, ey, 6.0, 2.5, INK);
            r.disc(ex, ey, 2.0, INK);
        } else {
            let px = ex + pose.look.x * 4.0;
            let py = ey + pose.look.y * 4.0 + 1.0;
            r.circle(px, py, 7.5, hex(0x16261F), Col::CLEAR, 0.0);
            r.disc(px - 2.5, py - 3.0, 2.6, Col::WHITE);
        }
        if pose.blink > 0.02 {
            let lid = pose.blink.min(1.0);
            r.push();
            r.translate(ex, ey);
            r.ellipse(0.0, -12.0 + lid * 12.0, 13.5, 1.0 + lid * 12.5, skin.body.shade(0.04), skin.body.shade(0.04), 0.0);
            r.pop();
        }
    }
    if pose.sprout {
        r.line(v2(0.0, -24.0), v2(1.5, -40.0), 3.5, skin.line);
        r.ellipse_rot(10.0, -43.0, 10.0, 5.0, -0.5, hex(0x8BE04E), skin.line, 2.5);
        r.ellipse_rot(-7.0, -40.0, 8.0, 4.2, 0.5, hex(0xB4F07A), skin.line, 2.5);
    }
    r.pop();
    r.pop();
}

/// Frog seen from directly above/behind (used by runners and riders): simplified silhouette.
pub fn frog_head(r: &mut Renderer, x: f32, y: f32, size: f32, skin: Skin, pose: &Pose) {
    let s = size / 100.0;
    r.push();
    r.translate(x, y);
    r.scale(s, s);
    r.ellipse_grad(0.0, 8.0, 46.0, 30.0, skin.body.shade(0.08), skin.body.shade(-0.05), skin.line, 4.0);
    r.ellipse(-30.0, 12.0, 7.0, 4.0, hex(0xFF8FA3).a(0.6), Col::CLEAR, 0.0);
    r.ellipse(30.0, 12.0, 7.0, 4.0, hex(0xFF8FA3).a(0.6), Col::CLEAR, 0.0);
    if pose.mouth > 0.05 {
        r.ellipse(0.0, 16.0, 12.0, 4.0 + pose.mouth * 8.0, hex(0x5A1A22), skin.line, 3.0);
    } else {
        r.ellipse(0.0, 12.0, 12.0, 6.0, skin.line, skin.line, 0.0);
        r.ellipse(0.0, 9.0, 13.5, 5.5, skin.body.shade(0.03), skin.body, 0.0);
    }
    for side in [-1.0f32, 1.0] {
        let ex = side * 21.0;
        r.circle(ex, -14.0, 16.0, skin.body.shade(0.08), skin.line, 4.0);
        r.circle(ex, -14.0, 11.0, Col::WHITE, Col::WHITE, 0.0);
        r.circle(ex + pose.look.x * 3.5, -13.0 + pose.look.y * 3.5, 7.0, hex(0x16261F), Col::CLEAR, 0.0);
        r.disc(ex + pose.look.x * 3.5 - 2.3, -16.0 + pose.look.y * 3.5, 2.4, Col::WHITE);
        if pose.blink > 0.02 {
            r.ellipse(ex, -26.0 + pose.blink * 12.0, 12.5, 1.0 + pose.blink * 11.5, skin.body.shade(0.04), skin.body.shade(0.04), 0.0);
        }
    }
    r.pop();
}

// ------------------------------------------------------------------ scenery

pub fn sky(r: &mut Renderer, top: Col, bottom: Col) {
    let (w, h) = (r.w, r.h);
    r.grad_rect(0.0, 0.0, w, h, top, bottom);
}

pub fn cloud(r: &mut Renderer, x: f32, y: f32, s: f32, c: Col) {
    r.ellipse(x, y + 8.0 * s, 70.0 * s, 22.0 * s, c, c, 0.0);
    r.circle(x - 28.0 * s, y, 26.0 * s, c, c, 0.0);
    r.circle(x + 6.0 * s, y - 12.0 * s, 34.0 * s, c, c, 0.0);
    r.circle(x + 38.0 * s, y + 2.0 * s, 24.0 * s, c, c, 0.0);
}

pub fn hills(r: &mut Renderer, base_y: f32, c: Col, amp: f32, phase: f32) {
    let w = r.w;
    let n = (w / 160.0).ceil() as i32 + 2;
    for i in -1..n {
        let x = i as f32 * 160.0 + (phase % 160.0);
        let hh = amp * (0.7 + 0.3 * ((i as f32 + (phase / 160.0).floor()) * 1.7).sin().abs());
        r.ellipse(x, base_y, 120.0, hh, c, c, 0.0);
    }
    r.rect(0.0, base_y, w, r.h - base_y, c);
}

pub fn water(r: &mut Renderer, y: f32, t: f32, top: Col, bottom: Col) {
    let w = r.w;
    let h = r.h;
    r.grad_rect(0.0, y, w, h - y, top, bottom);
    let hl = Col::WHITE.a(0.25);
    let mut x = -60.0 + (t * 25.0) % 120.0;
    while x < w + 60.0 {
        let yy = y + 6.0 + (x * 0.05 + t * 2.0).sin() * 3.0;
        r.rrect(x, yy, 46.0, 5.0, 2.5, hl, hl, 0.0);
        x += 120.0;
    }
}

pub fn lily_pad(r: &mut Renderer, x: f32, y: f32, rx: f32, rot: f32) {
    let ry = rx * 0.38;
    let c = hex(0x3DAA5C);
    r.ellipse(x, y + ry * 0.25, rx, ry, hex(0x1F6B3A), hex(0x1F6B3A), 0.0);
    r.ellipse(x, y, rx, ry, c, hex(0x1F6B3A), 3.0);
    // notch
    let a = rot;
    let p0 = v2(x, y);
    let p1 = v2(x + (a - 0.18).cos() * rx * 1.05, y + (a - 0.18).sin() * ry * 1.05);
    let p2 = v2(x + (a + 0.18).cos() * rx * 1.05, y + (a + 0.18).sin() * ry * 1.05);
    r.tri(p0, p1, p2, hex(0x2B7FB8).a(0.0));
    r.line(v2(x, y), v2(x + (a + 0.9).cos() * rx * 0.7, y + (a + 0.9).sin() * ry * 0.7), 2.0, hex(0x2E8B4A));
    r.line(v2(x, y), v2(x + (a - 1.4).cos() * rx * 0.6, y + (a - 1.4).sin() * ry * 0.6), 2.0, hex(0x2E8B4A));
}

pub fn tree(r: &mut Renderer, x: f32, y: f32, s: f32) {
    r.rrect(x, y - 40.0 * s, 18.0 * s, 80.0 * s, 6.0 * s, hex(0x8A5A34), hex(0x4E3019), 3.0);
    let g = hex(0x58B94F);
    let l = hex(0x2E6B2A);
    r.circle(x - 26.0 * s, y - 90.0 * s, 36.0 * s, g, l, 3.0);
    r.circle(x + 26.0 * s, y - 92.0 * s, 34.0 * s, g, l, 3.0);
    r.circle(x, y - 122.0 * s, 40.0 * s, g.shade(0.1), l, 3.0);
}

pub fn stump(r: &mut Renderer, x: f32, y: f32, w: f32, h: f32) {
    r.rrect_grad(x, y + h * 0.5, w, h, 8.0, hex(0xA86E3F), hex(0x7A4A25), hex(0x4A2B14), 4.0);
    r.ellipse(x, y, w * 0.5, w * 0.16, hex(0xE7B985), hex(0x4A2B14), 4.0);
    r.ellipse(x, y, w * 0.3, w * 0.09, Col::CLEAR, hex(0xB57F4B), 2.0);
}

// ------------------------------------------------------------------ items

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fruit {
    Apple,
    Orange,
    Pear,
    Grapes,
    Banana,
    Melon,
    Lemon,
    Plum,
}
pub const FRUITS: [Fruit; 8] = [Fruit::Apple, Fruit::Orange, Fruit::Pear, Fruit::Grapes, Fruit::Banana, Fruit::Melon, Fruit::Lemon, Fruit::Plum];

pub fn fruit_color(f: Fruit) -> Col {
    match f {
        Fruit::Apple => hex(0xE8413C),
        Fruit::Orange => hex(0xFF9A1F),
        Fruit::Pear => hex(0xB6D94C),
        Fruit::Grapes => hex(0x8E4FC9),
        Fruit::Banana => hex(0xF7D64A),
        Fruit::Melon => hex(0x4DBB5B),
        Fruit::Lemon => hex(0xFFE95C),
        Fruit::Plum => hex(0x5B4BC4),
    }
}

pub fn fruit(r: &mut Renderer, f: Fruit, x: f32, y: f32, size: f32) {
    let s = size / 60.0;
    r.push();
    r.translate(x, y);
    r.scale(s, s);
    let c = fruit_color(f);
    let l = c.shade(-0.6);
    match f {
        Fruit::Apple => {
            r.circle(-9.0, 2.0, 21.0, c, l, 3.0);
            r.circle(9.0, 2.0, 21.0, c, l, 3.0);
            r.ellipse(0.0, 4.0, 24.0, 20.0, c, c, 0.0);
            r.line(v2(0.0, -14.0), v2(3.0, -26.0), 4.0, hex(0x6B3E1E));
            r.ellipse_rot(10.0, -22.0, 9.0, 4.5, -0.5, hex(0x5DBB45), hex(0x2D6B22), 2.0);
            r.ellipse(-10.0, -6.0, 5.0, 7.0, Col::WHITE.a(0.5), Col::CLEAR, 0.0);
        }
        Fruit::Orange | Fruit::Lemon => {
            if f == Fruit::Lemon {
                r.ellipse(0.0, 2.0, 27.0, 21.0, c, l, 3.0);
                r.disc(-27.0, 2.0, 4.0, c.shade(-0.1));
                r.disc(27.0, 2.0, 4.0, c.shade(-0.1));
            } else {
                r.circle(0.0, 2.0, 24.0, c, l, 3.0);
                r.ellipse_rot(5.0, -22.0, 8.0, 4.0, -0.4, hex(0x5DBB45), hex(0x2D6B22), 2.0);
            }
            r.ellipse(-8.0, -6.0, 5.0, 7.0, Col::WHITE.a(0.5), Col::CLEAR, 0.0);
        }
        Fruit::Pear => {
            r.circle(0.0, 9.0, 21.0, c, l, 3.0);
            r.circle(0.0, -9.0, 13.0, c, l, 3.0);
            r.ellipse(0.0, 2.0, 15.0, 16.0, c, c, 0.0);
            r.line(v2(0.0, -20.0), v2(2.0, -30.0), 4.0, hex(0x6B3E1E));
            r.ellipse(-7.0, 2.0, 4.0, 7.0, Col::WHITE.a(0.45), Col::CLEAR, 0.0);
        }
        Fruit::Grapes => {
            let pts = [(-12.0, -10.0), (0.0, -12.0), (12.0, -10.0), (-6.0, 1.0), (6.0, 1.0), (-12.0, 4.0), (12.0, 4.0), (0.0, 13.0), (-6.0, 22.0), (6.0, 22.0)];
            for (px, py) in pts {
                r.circle(px, py, 9.0, c, l, 2.5);
                r.disc(px - 3.0, py - 3.0, 2.2, Col::WHITE.a(0.5));
            }
            r.line(v2(0.0, -20.0), v2(-2.0, -30.0), 4.0, hex(0x6B3E1E));
            r.ellipse_rot(9.0, -26.0, 9.0, 4.5, -0.3, hex(0x5DBB45), hex(0x2D6B22), 2.0);
        }
        Fruit::Banana => {
            for i in 0..7 {
                let a = PI * (0.15 + i as f32 * 0.1);
                let px = a.cos() * -22.0;
                let py = a.sin() * 18.0 - 10.0;
                r.circle(px, py, 10.0, c, l, 3.0);
            }
            for i in 0..7 {
                let a = PI * (0.15 + i as f32 * 0.1);
                r.disc(a.cos() * -22.0, a.sin() * 18.0 - 10.0, 7.0, c);
            }
            r.rrect(18.0, -16.0, 6.0, 8.0, 2.0, hex(0x6B3E1E), hex(0x6B3E1E), 0.0);
        }
        Fruit::Melon => {
            r.pie(0.0, 10.0, 30.0, 0.5, hex(0x2E8B3E));
            r.push();
            r.rotate(PI * 0.5);
            r.pop();
            r.ellipse(0.0, 4.0, 30.0, 20.0, hex(0x2E8B3E), hex(0x1C5226), 3.0);
            r.ellipse(0.0, 2.0, 25.0, 15.0, hex(0xF35B5B), Col::CLEAR, 0.0);
            for i in 0..5 {
                let px = -14.0 + i as f32 * 7.0;
                r.ellipse(px, 2.0 + (i % 2) as f32 * 4.0, 1.6, 2.6, INK, INK, 0.0);
            }
        }
        Fruit::Plum => {
            r.ellipse(0.0, 2.0, 22.0, 24.0, c, l, 3.0);
            r.line(v2(0.0, -22.0), v2(0.0, -6.0), 2.0, l.a(0.5));
            r.line(v2(0.0, -20.0), v2(3.0, -30.0), 4.0, hex(0x6B3E1E));
            r.ellipse(-8.0, -6.0, 4.0, 7.0, Col::WHITE.a(0.45), Col::CLEAR, 0.0);
        }
    }
    r.pop();
}

pub fn candy(r: &mut Renderer, x: f32, y: f32, size: f32, kind: u32, rot: f32) {
    let s = size / 60.0;
    r.push();
    r.translate(x, y);
    r.rotate(rot);
    r.scale(s, s);
    match kind % 4 {
        0 => {
            // wrapped sweet
            let c = hex(0xFF5DA2);
            r.tri(v2(-16.0, 0.0), v2(-32.0, -14.0), v2(-32.0, 14.0), c.shade(-0.1));
            r.tri(v2(16.0, 0.0), v2(32.0, -14.0), v2(32.0, 14.0), c.shade(-0.1));
            r.circle(0.0, 0.0, 18.0, c, c.shade(-0.6), 3.0);
            r.line(v2(-8.0, -12.0), v2(8.0, 12.0), 4.0, Col::WHITE.a(0.7));
        }
        1 => {
            // lollipop swirl
            r.line(v2(0.0, 10.0), v2(0.0, 34.0), 5.0, hex(0xF2E6D0));
            r.circle(0.0, -4.0, 22.0, hex(0xFFE066), hex(0x8A5A00), 3.0);
            for i in 0..4 {
                r.ring(0.0, -4.0, 5.0 + i as f32 * 4.5, 2.5, hex(0xFF5D5D).a(if i % 2 == 0 { 0.9 } else { 0.0 }));
            }
        }
        2 => {
            // cupcake
            r.rrect(0.0, 12.0, 34.0, 20.0, 4.0, hex(0x8FD3F4), hex(0x2B6F91), 3.0);
            r.circle(0.0, -4.0, 18.0, hex(0xFFC1E3), hex(0x9C3D73), 3.0);
            r.disc(0.0, -20.0, 5.0, hex(0xE8413C));
        }
        _ => {
            // donut
            r.circle(0.0, 0.0, 22.0, hex(0xD9965B), hex(0x6E4220), 3.0);
            r.circle(0.0, -2.0, 18.0, hex(0xA06CE0), Col::CLEAR, 0.0);
            r.circle(0.0, 0.0, 7.0, hex(0x2B7FB8).a(0.0), hex(0x6E4220), 3.0);
            for i in 0..6 {
                let a = i as f32 * 1.1;
                r.rrect_rot(a.cos() * 12.0, a.sin() * 12.0 - 2.0, 6.0, 2.5, 1.2, a, [hex(0xFFFFFF), hex(0xFFE066), hex(0x66E0FF)][i % 3], Col::CLEAR, 0.0);
            }
        }
    }
    r.pop();
}

pub fn bubble(r: &mut Renderer, x: f32, y: f32, rad: f32, t: f32) {
    let wob = 1.0 + (t * 5.0).sin() * 0.04;
    r.ellipse(x, y, rad * wob, rad / wob, hex(0xBFE9FF).a(0.25), hex(0xE8F7FF).a(0.9), 2.5);
    r.ellipse_rot(x - rad * 0.4, y - rad * 0.4, rad * 0.3, rad * 0.15, -0.7, Col::WHITE.a(0.8), Col::CLEAR, 0.0);
    r.disc(x + rad * 0.35, y + rad * 0.4, rad * 0.08, Col::WHITE.a(0.6));
}

pub fn rock(r: &mut Renderer, x: f32, y: f32, rad: f32, seed: u32, base: Col) {
    // lumpy asteroid built from overlapping circles
    let mut rng = Rng::new(seed as u64 + 7);
    let line = base.shade(-0.6);
    for i in 0..6 {
        let a = i as f32 / 6.0 * TAU + rng.f();
        let d = rad * 0.35;
        r.circle(x + a.cos() * d, y + a.sin() * d, rad * rng.range(0.6, 0.75), base, line, 3.0);
    }
    for i in 0..6 {
        let a = i as f32 / 6.0 * TAU + 0.3;
        r.disc(x + a.cos() * rad * 0.33, y + a.sin() * rad * 0.33, rad * 0.55, base);
    }
    r.disc(x, y, rad * 0.6, base);
    for _ in 0..3 {
        let a = rng.f() * TAU;
        let d = rng.range(0.1, 0.5) * rad;
        let cr = rng.range(0.12, 0.22) * rad;
        r.circle(x + a.cos() * d, y + a.sin() * d, cr, base.shade(-0.2), base.shade(-0.35), 1.5);
    }
    r.ellipse(x - rad * 0.3, y - rad * 0.35, rad * 0.2, rad * 0.12, Col::WHITE.a(0.3), Col::CLEAR, 0.0);
}

pub fn clock_icon(r: &mut Renderer, x: f32, y: f32, rad: f32, frac: f32) {
    r.circle(x, y, rad, hex(0xFFFFFF), INK, rad * 0.16);
    if frac < 1.0 {
        r.pie(x, y, rad * 0.78, 1.0 - frac, hex(0xFFB020).a(0.85));
    }
    r.line(v2(x, y), v2(x, y - rad * 0.6), rad * 0.13, INK);
    r.line(v2(x, y), v2(x + rad * 0.4, y), rad * 0.13, INK);
    r.disc(x, y, rad * 0.12, INK);
    r.rrect(x, y - rad * 1.12, rad * 0.5, rad * 0.25, rad * 0.08, INK, INK, 0.0);
}

pub fn star_icon(r: &mut Renderer, x: f32, y: f32, rad: f32, on: bool) {
    if on {
        r.star(x, y + rad * 0.08, rad, 0.0, hex(0xFFD23F), hex(0x8A5A00), rad * 0.16);
        r.ellipse(x - rad * 0.2, y - rad * 0.2, rad * 0.18, rad * 0.1, Col::WHITE.a(0.7), Col::CLEAR, 0.0);
    } else {
        r.star(x, y + rad * 0.08, rad, 0.0, hex(0x000000).a(0.25), Col::WHITE.a(0.35), rad * 0.12);
    }
}

pub fn gift(r: &mut Renderer, x: f32, y: f32, size: f32, color: Col, ribbon: Col) {
    let h = size * 0.85;
    r.rrect(x, y, size, h, size * 0.08, color, color.shade(-0.6), 3.0);
    r.rect(x - size * 0.08, y - h * 0.5 + 2.0, size * 0.16, h - 4.0, ribbon);
    r.rect(x - size * 0.5 + 2.0, y - size * 0.08, size - 4.0, size * 0.16, ribbon);
    r.ellipse_rot(x - size * 0.16, y - h * 0.55, size * 0.17, size * 0.09, 0.4, ribbon, ribbon.shade(-0.5), 2.0);
    r.ellipse_rot(x + size * 0.16, y - h * 0.55, size * 0.17, size * 0.09, -0.4, ribbon, ribbon.shade(-0.5), 2.0);
}

pub fn balloon(r: &mut Renderer, x: f32, y: f32, rad: f32, c: Col) {
    r.line(v2(x, y + rad), v2(x + 4.0, y + rad * 2.0), 2.0, INK.a(0.6));
    r.tri(v2(x, y + rad * 0.95), v2(x - rad * 0.12, y + rad * 1.12), v2(x + rad * 0.12, y + rad * 1.12), c.shade(-0.2));
    r.ellipse(x, y, rad * 0.88, rad, c, c.shade(-0.55), 3.0);
    r.ellipse_rot(x - rad * 0.35, y - rad * 0.4, rad * 0.18, rad * 0.32, 0.5, Col::WHITE.a(0.55), Col::CLEAR, 0.0);
}

pub fn ball(r: &mut Renderer, x: f32, y: f32, rad: f32, rot: f32, c: Col) {
    r.circle(x, y, rad, c, c.shade(-0.6), rad * 0.1);
    r.push();
    r.translate(x, y);
    r.rotate(rot);
    r.ellipse(0.0, 0.0, rad * 0.35, rad * 0.9, Col::CLEAR, Col::WHITE.a(0.8), rad * 0.1);
    r.pop();
    r.ellipse(x - rad * 0.35, y - rad * 0.4, rad * 0.22, rad * 0.14, Col::WHITE.a(0.6), Col::CLEAR, 0.0);
}

pub fn sparkle(r: &mut Renderer, x: f32, y: f32, s: f32, c: Col) {
    r.glow(x, y, s * 1.6, c.a(0.5));
    r.rrect(x, y, s * 0.35, s * 2.0, s * 0.17, c, c, 0.0);
    r.rrect(x, y, s * 2.0, s * 0.35, s * 0.17, c, c, 0.0);
}

/// Score / label pill used by every HUD.
pub fn pill(r: &mut Renderer, x: f32, y: f32, w: f32, h: f32, fill: Col) {
    r.rrect(x, y + 4.0, w, h, h * 0.5, Col(0, 0, 0, 60), Col::CLEAR, 0.0);
    r.rrect_grad(x, y, w, h, h * 0.5, fill.shade(0.15), fill.shade(-0.1), INK, 4.0);
}

pub fn label(r: &mut Renderer, s: &str, x: f32, y: f32, size: f32) {
    r.text_bold(s, x, y, size, Align::Center, Col::WHITE, INK);
}
