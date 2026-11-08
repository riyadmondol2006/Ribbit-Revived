//! Stump Hop: twelve tree stumps, three treats at a time. Tap a neighbouring stump (diagonals
//! count) to hop there and eat whatever sits on it. Clearing all three treats adds +1 s.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const COLS: [f32; 4] = [-274.0, -92.0, 92.0, 274.0];
const ROWS: [f32; 3] = [205.0, 385.0, 565.0];

pub struct Treat {
    frog: usize,
    from: usize,
    jump: f32,
    candies: Vec<usize>,
    kind: u32,
    wave_t: f32,
    eaten_t: f32,
    wrong: Option<(usize, f32)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Treat { frog: 0, from: 0, jump: 1.0, candies: Vec::new(), kind: 0, wave_t: 0.0, eaten_t: 9.0, wrong: None };
    g.new_wave(c);
    Box::new(g)
}

impl Treat {
    fn pos(c: &Ctx, i: usize) -> V2 {
        v2(c.center_x() + COLS[i % 4], ROWS[i / 4])
    }
    fn new_wave(&mut self, c: &mut Ctx) {
        let mut free: Vec<usize> = (0..12).filter(|&i| i != self.frog).collect();
        c.rng.shuffle(&mut free);
        self.candies = free[..3].to_vec();
        self.kind = (self.kind + 1) % 4;
        self.wave_t = 0.0;
    }
}

impl MiniGame for Treat {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.wave_t += dt;
        self.eaten_t += dt;
        if let Some((_, t)) = &mut self.wrong {
            *t += dt;
            if *t > 0.6 {
                self.wrong = None;
            }
        }
        if self.jump < 1.0 {
            self.jump += dt / 0.1;
            if self.jump >= 1.0 {
                self.jump = 1.0;
                c.sfx_ex(Sfx::Thud, 0.5, 1.4);
                if let Some(k) = self.candies.iter().position(|&i| i == self.frog) {
                    self.candies.remove(k);
                    let p = Self::pos(c, self.frog);
                    c.add_score(1, p - v2(0.0, 150.0));
                    c.sfx_var(Sfx::Eat);
                    self.eaten_t = 0.0;
                    c.fx.burst(p - v2(0.0, 40.0), hex(0xFF5DA2), 10, 300.0);
                    if self.candies.is_empty() {
                        c.add_time(1.0, v2(c.center_x(), c.h * 0.5));
                        self.new_wave(c);
                    }
                }
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.jump < 1.0 {
            return;
        }
        let Some(target) = (0..12).min_by(|&a, &b| Self::pos(c, a).dist(t.pos).total_cmp(&Self::pos(c, b).dist(t.pos))) else { return };
        if Self::pos(c, target).dist(t.pos) > 130.0 {
            return;
        }
        let (fx, fy) = ((self.frog % 4) as f32, (self.frog / 4) as f32);
        let (tx, ty) = ((target % 4) as f32, (target / 4) as f32);
        let d2 = (fx - tx).powi(2) + (fy - ty).powi(2);
        if d2 < 4.0 {
            self.from = self.frog;
            self.frog = target;
            self.jump = 0.0;
            c.sfx_var(Sfx::Jump);
            c.tick();
        } else {
            self.wrong = Some((target, 0.0));
            c.sfx_ex(Sfx::Wrong, 0.6, 1.1);
            c.reject();
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        art::sky(r, hex(0xA6E6FF), hex(0xE6FFE0));
        art::hills(r, 150.0, hex(0x9ED96B), 50.0, 0.0);
        r.grad_rect(0.0, 150.0, w, h - 150.0, hex(0x8BCB5A), hex(0x5E9E3A));
        for i in 0..10 {
            art::tree(r, c.safe_l + i as f32 * 190.0, 175.0, 0.55);
        }
        // reachable hint rings
        if self.jump >= 1.0 {
            let (fx, fy) = ((self.frog % 4) as f32, (self.frog / 4) as f32);
            for i in 0..12 {
                let d2 = (fx - (i % 4) as f32).powi(2) + (fy - (i / 4) as f32).powi(2);
                if i != self.frog && d2 < 4.0 {
                    let p = Self::pos(c, i);
                    r.ellipse(p.x, p.y, 92.0, 30.0, Col::WHITE.a(0.25 + 0.1 * (c.anim * 4.0).sin()), Col::CLEAR, 0.0);
                }
            }
        }
        for i in 0..12 {
            let p = Self::pos(c, i);
            art::stump(r, p.x, p.y, 150.0, 70.0);
            if let Some((wi, t)) = self.wrong {
                if wi == i {
                    let a = 1.0 - t / 0.6;
                    r.text_bold("X", p.x, p.y - 40.0, 80.0 * (1.0 + t), crate::render::Align::Center, hex(0xFF5D5D).a(a), art::INK.a(a));
                }
            }
        }
        let k = ease_out_back((self.wave_t / 0.3).min(1.0));
        for &i in &self.candies {
            let p = Self::pos(c, i);
            art::candy(r, p.x, p.y - 34.0 + (c.anim * 3.0 + i as f32).sin() * 4.0, 72.0 * k, self.kind, 0.2);
        }
        // frog
        let a = Self::pos(c, self.from);
        let b = Self::pos(c, self.frog);
        let j = self.jump.min(1.0);
        let p = a.lerp(b, j) - v2(0.0, (j * std::f32::consts::PI).sin() * 70.0 + 60.0);
        let pose = Pose {
            legs_out: if j < 1.0 { 1.0 } else { 0.0 },
            mouth: if self.eaten_t < 0.4 { (self.eaten_t * 20.0).sin().abs() } else { 0.0 },
            ..Pose::idle(c.anim, 0.0)
        };
        art::frog(r, p.x, p.y, 120.0, art::EMERALD, &pose);
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    art::stump(r, x - s * 0.28, y + s * 0.2, s * 0.34, s * 0.16);
    art::stump(r, x + s * 0.28, y + s * 0.2, s * 0.34, s * 0.16);
    art::candy(r, x + s * 0.28, y + s * 0.08, s * 0.2, 2, 0.0);
    let k = (t * 0.9).fract();
    let fx = x - s * 0.28 + k * s * 0.4;
    art::frog(r, fx, y - s * 0.02 - (k * std::f32::consts::PI).sin() * s * 0.15, s * 0.3, art::EMERALD, &Pose::idle(t, 0.0));
}
