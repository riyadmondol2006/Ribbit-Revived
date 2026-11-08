//! Lily Leap: hop along a row of floating pads with the single (+1) and double (+2) jump
//! buttons. Gaps are always one cell wide, so a double jump clears them and a single jump
//! means a splash. Clock pads give +2 s. Score = pads travelled (metres).
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};

const PITCH: f32 = 138.0;
const CELLS: usize = 1124;

#[derive(Clone, Copy, PartialEq)]
enum State {
    Idle,
    Jump { from: usize, to: usize, t: f32 },
    Fall { at: usize, t: f32 },
    Back { from: usize, to: usize, t: f32 },
}

pub struct Hop {
    pad: Vec<bool>,
    clock: Vec<bool>,
    pos: usize,
    cam: f32,
    state: State,
    btn_t: [f32; 2],
    ripples: Vec<(f32, f32)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut pad = vec![false; CELLS];
    let mut clock = vec![false; CELLS];
    let mut i = 0;
    while i < CELLS {
        let mut run = 1;
        while run < 4 && c.rng.chance(2.0 / 3.0) {
            run += 1;
        }
        for k in 0..run {
            if i + k < CELLS {
                pad[i + k] = true;
            }
        }
        i += run + 1; // exactly one water cell between runs
    }
    let mut clocks: Vec<usize> = vec![6, 12, 18, 24, 30, 36, 48, 56, 64, 72, 80, 90, 100, 110, 120, 133, 146, 159, 180, 200, 240];
    let mut k = 280;
    while k <= 600 {
        clocks.push(k);
        k += 40;
    }
    for &cpos in &clocks {
        if cpos < CELLS {
            pad[cpos] = true;
            clock[cpos] = true;
        }
    }
    pad[0] = true;
    pad[1] = true;
    Box::new(Hop { pad, clock, pos: 0, cam: 0.0, state: State::Idle, btn_t: [9.0; 2], ripples: Vec::new() })
}

impl Hop {
    fn frog_x(c: &Ctx) -> f32 {
        c.sx(0.32)
    }
    fn water_y(c: &Ctx) -> f32 {
        c.h * 0.62
    }
    fn cell_x(&self, c: &Ctx, i: usize) -> f32 {
        Self::frog_x(c) + i as f32 * PITCH - self.cam
    }
    fn buttons(c: &Ctx) -> (V2, V2) {
        (v2(c.safe_l + 140.0, c.h - 88.0), v2(c.safe_r - 140.0, c.h - 88.0))
    }
    fn jump(&mut self, c: &mut Ctx, n: usize) {
        if self.state != State::Idle {
            return;
        }
        let to = (self.pos + n).min(CELLS - 1);
        self.state = State::Jump { from: self.pos, to, t: 0.0 };
        c.score = to as i64;
        c.sfx_ex(if n == 1 { Sfx::Jump } else { Sfx::DoubleJump }, 0.8, 1.0);
        c.tick();
    }
    fn tier(i: usize) -> usize {
        (i / 40).min(5)
    }
    fn land(&mut self, c: &mut Ctx, at: usize, counted: bool) {
        self.pos = at;
        self.state = State::Idle;
        self.ripples.push((at as f32, 0.0));
        let tier = Self::tier(at) as f32;
        c.sfx_ex(Sfx::Splat, 0.4, 1.2 + tier * 0.08);
        if self.clock[at] {
            self.clock[at] = false;
            let p = v2(self.cell_x(c, at), Self::water_y(c) - 120.0);
            c.add_time(2.0, p);
        }
        let _ = counted;
    }
}

impl MiniGame for Hop {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        for b in self.btn_t.iter_mut() {
            *b += dt;
        }
        for r in self.ripples.iter_mut() {
            r.1 += dt;
        }
        self.ripples.retain(|r| r.1 < 0.6);
        // camera follows the frog's pad
        let target = self.pos as f32 * PITCH;
        let target = match self.state {
            State::Jump { from, to, t } | State::Back { from, to, t } => lerp(from as f32, to as f32, (t / 0.18).min(1.0)) * PITCH,
            _ => target,
        };
        self.cam += (target - self.cam) * (dt * 14.0).min(1.0);
        match self.state {
            State::Idle => {}
            State::Jump { from, to, t } => {
                let t = t + dt;
                if t >= 0.2 {
                    if self.pad[to] {
                        c.special += (to - from) as i64;
                        self.land(c, to, true);
                    } else {
                        self.state = State::Fall { at: to, t: 0.0 };
                        c.special = 0;
                        c.sfx(Sfx::Splash);
                        c.reject();
                        let p = v2(self.cell_x(c, to), Self::water_y(c));
                        c.fx.splash(p, 18);
                    }
                } else {
                    self.state = State::Jump { from, to, t };
                }
            }
            State::Fall { at, t } => {
                let t = t + dt;
                if t >= 1.0 {
                    self.state = State::Back { from: at, to: at - 1, t: 0.0 };
                    c.sfx_ex(Sfx::Jump, 0.6, 0.8);
                } else {
                    self.state = State::Fall { at, t };
                }
            }
            State::Back { from, to, t } => {
                let t = t + dt;
                if t >= 0.25 {
                    c.score = to as i64;
                    self.land(c, to, false);
                } else {
                    self.state = State::Back { from, to, t };
                }
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down {
            return;
        }
        let (lb, rb) = Self::buttons(c);
        let near = |b: V2| (t.pos.x - b.x).abs() < 180.0 && (t.pos.y - b.y).abs() < 130.0;
        if near(lb) || (t.pos.x < c.center_x() && t.pos.y > c.h * 0.45) {
            self.btn_t[0] = 0.0;
            self.jump(c, 1);
        } else if near(rb) || (t.pos.x >= c.center_x() && t.pos.y > c.h * 0.45) {
            self.btn_t[1] = 0.0;
            self.jump(c, 2);
        }
    }

    fn score_text(&self, c: &Ctx) -> String {
        format!("{} m", c.score)
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let w = r.w;
        let wy = Self::water_y(c);
        art::sky(r, hex(0x7ED6FF), hex(0xDFF7FF));
        // parallax backdrop: far dunes, mid hills
        let far = self.cam * 5.0 / 70.0;
        let mid = self.cam * 50.0 / 70.0;
        art::cloud(r, (w * 0.3 - far * 0.5).rem_euclid(w + 300.0) - 150.0, 100.0, 1.0, Col::WHITE.a(0.9));
        art::cloud(r, (w * 0.8 - far * 0.5).rem_euclid(w + 300.0) - 150.0, 160.0, 0.7, Col::WHITE.a(0.8));
        art::hills(r, wy - 110.0, hex(0xF6C35B), 70.0, -far);
        art::hills(r, wy - 40.0, hex(0xE9A43F), 50.0, -mid * 0.4);
        for i in 0..6 {
            let x = (i as f32 * 340.0 - mid * 0.4).rem_euclid(w + 400.0) - 200.0;
            art::tree(r, x, wy - 20.0, 0.6);
        }
        art::water(r, wy, c.anim, hex(0x4FC1EC), hex(0x1C76B5));
        // pads
        let first = ((self.cam - Self::frog_x(c)) / PITCH).floor().max(0.0) as usize;
        let last = (first + (w / PITCH) as usize + 3).min(CELLS - 1);
        for i in first..=last {
            let x = self.cell_x(c, i);
            if !self.pad[i] {
                continue;
            }
            let bob = (c.anim * 2.0 + i as f32).sin() * 3.0;
            let y = wy + 30.0 + bob;
            match Self::tier(i) {
                0 => art::lily_pad(r, x, y, 62.0, 0.5 + i as f32),
                1 => {
                    r.rrect(x, y + 6.0, 112.0, 30.0, 6.0, hex(0xA9743F), art::INK, 3.0);
                    r.rrect(x, y - 2.0, 112.0, 26.0, 6.0, hex(0xD9A35F), art::INK, 3.0);
                    r.line(v2(x - 40.0, y - 2.0), v2(x + 40.0, y - 2.0), 2.0, hex(0xA9743F));
                }
                2 => {
                    r.ellipse(x, y, 58.0, 26.0, hex(0x3E8A5B), art::INK, 3.0);
                    for k in 0..3 {
                        r.ellipse(x - 26.0 + k as f32 * 26.0, y - 4.0, 12.0, 8.0, hex(0x6CBF7E), Col::CLEAR, 0.0);
                    }
                    r.circle(x + 58.0, y + 2.0, 13.0, hex(0x8FD39B), art::INK, 3.0);
                }
                3 => {
                    r.ellipse(x, y, 60.0, 22.0, hex(0x9AA5B1), art::INK, 3.0);
                    r.ellipse(x - 14.0, y - 6.0, 16.0, 6.0, Col::WHITE.a(0.35), Col::CLEAR, 0.0);
                }
                4 => {
                    r.ellipse_rot(x, y, 64.0, 20.0, 0.05, hex(0xF29F3C), art::INK, 3.0);
                    r.line(v2(x - 50.0, y), v2(x + 50.0, y - 3.0), 2.5, hex(0xC9752A));
                }
                _ => {
                    r.ellipse(x, y, 62.0, 22.0, hex(0xE4F6FF), hex(0x6FA8C9), 3.0);
                    r.ellipse(x + 10.0, y - 5.0, 20.0, 6.0, Col::WHITE, Col::CLEAR, 0.0);
                }
            }
            if self.clock[i] {
                let cb = (c.anim * 3.0 + i as f32).sin() * 6.0;
                art::clock_icon(r, x, y - 60.0 + cb, 26.0, 1.0);
            }
            if i % 10 == 0 && i > 0 {
                r.text_bold(&format!("{i}"), x, y + 50.0, 22.0, Align::Center, Col::WHITE, art::INK);
            }
        }
        for (cell, t) in &self.ripples {
            let x = Self::frog_x(c) + cell * PITCH - self.cam;
            r.ellipse(x, wy + 36.0, 60.0 + t * 80.0, 14.0 + t * 16.0, Col::CLEAR, Col::WHITE.a(0.6 * (1.0 - t / 0.6)), 3.0);
        }
        // frog
        let fx0 = Self::frog_x(c);
        let base_y = wy + 30.0 - 46.0;
        let (fx, fy, pose, alpha) = match self.state {
            State::Idle => (fx0 + self.pos as f32 * PITCH - self.cam, base_y, Pose::idle(c.anim, 0.0), 1.0),
            State::Jump { from, to, t } => {
                let k = (t / 0.2).min(1.0);
                let arc = (k * std::f32::consts::PI).sin() * (if to - from == 1 { 90.0 } else { 150.0 });
                let x = fx0 + lerp(from as f32, to as f32, k) * PITCH - self.cam;
                (x, base_y - arc, Pose { legs_out: 1.0, mouth: 0.3, ..Pose::default() }, 1.0)
            }
            State::Fall { at, t } => {
                let x = fx0 + at as f32 * PITCH - self.cam;
                let sink = (t / 0.3).min(1.0) * 70.0;
                (x, base_y + sink, Pose { dizzy: true, mouth: 0.8, ..Pose::default() }, (1.0 - t / 0.35).max(0.0))
            }
            State::Back { from, to, t } => {
                let k = (t / 0.25).min(1.0);
                let x = fx0 + lerp(from as f32, to as f32, k) * PITCH - self.cam;
                (x, base_y - (k * std::f32::consts::PI).sin() * 90.0 + (1.0 - k) * 40.0, Pose { legs_out: 1.0, ..Pose::default() }, k.max(0.3))
            }
        };
        r.push();
        r.fade(alpha);
        art::frog(r, fx, fy, 120.0, art::EMERALD, &pose);
        r.pop();
        // jump buttons
        let (lb, rb) = Self::buttons(c);
        for (i, b) in [lb, rb].iter().enumerate() {
            let d = if self.btn_t[i] < 0.12 { 6.0 } else { 0.0 };
            r.rrect(b.x, b.y + 8.0, 220.0, 124.0, 34.0, hex(0xC46A12), art::INK, 4.0);
            r.rrect_grad(b.x, b.y + d, 220.0, 124.0, 34.0, hex(0xFFC15A), hex(0xFF9420), art::INK, 4.0);
            let hops = i + 1;
            let span = if hops == 1 { 60.0 } else { 120.0 };
            let y0 = b.y + d + 22.0;
            for k in 0..=hops {
                r.ellipse(b.x - span * 0.5 + k as f32 * span / hops as f32, y0, 16.0, 7.0, hex(0x3DAA5C), art::INK, 2.5);
            }
            for k in 0..hops {
                let x0 = b.x - span * 0.5 + k as f32 * span / hops as f32;
                let x1 = x0 + span / hops as f32;
                let steps = 6;
                for s in 0..steps {
                    let a0 = s as f32 / steps as f32;
                    let a1 = (s + 1) as f32 / steps as f32;
                    let p0 = v2(lerp(x0, x1, a0), y0 - 10.0 - (a0 * std::f32::consts::PI).sin() * 36.0);
                    let p1 = v2(lerp(x0, x1, a1), y0 - 10.0 - (a1 * std::f32::consts::PI).sin() * 36.0);
                    r.line(p0, p1, 6.0, Col::WHITE);
                }
            }
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rrect(x, y + s * 0.28, s * 1.2, s * 0.3, 10.0, hex(0x4FC1EC), Col::CLEAR, 0.0);
    for i in 0..3 {
        if i == 1 {
            continue;
        }
        art::lily_pad(r, x - s * 0.36 + i as f32 * s * 0.36, y + s * 0.22, s * 0.15, 0.4);
    }
    let k = (t * 0.8).fract();
    let fx = x - s * 0.36 + k * s * 0.72;
    let fy = y + s * 0.05 - (k * std::f32::consts::PI).sin() * s * 0.3;
    art::frog(r, fx, fy, s * 0.3, art::EMERALD, &Pose { legs_out: 1.0, ..Pose::idle(t, 0.0) });
}
