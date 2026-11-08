//! Deep Dive: tap anywhere and the frog swims there (always in half a second). Sweep up the
//! sweets in each pattern; clearing a pattern adds +3 s (+2 s later on). A pufferfish loops a
//! rose curve around the reef and knocks the frog back on contact.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;
use std::f32::consts::{PI, TAU};

/// Sweets per pattern (difficulty ramp of the classic 52 sub-levels).
const COUNTS: [usize; 52] = [
    1, 2, 2, 2, 3, 4, 5, 6, 6, 8, 8, 7, 4, 8, 7, 5, 10, 10, 7, 5, 23, 14, 12, 13, 10, 20, 17, 15, 12, 12, 14, 22, 18, 30, 14, 24, 13, 14, 15, 17, 20, 16, 15, 20,
    16, 18, 20, 14, 22, 18, 16, 11,
];

struct Candy {
    pos: V2,
    kind: u32,
    age: f32,
}

pub struct Dive {
    candies: Vec<Candy>,
    layer: usize,
    frog: V2,
    start: V2,
    target: V2,
    swim_t: f32,
    angle: f32,
    stun: f32,
    knock_from: V2,
    knock_to: V2,
    theta: f32,
    k: f32,
    puff: f32,
    hit: bool,
    bonus: f32,
    bubbles: Vec<(V2, f32)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let frog = v2(c.sx(0.1).max(c.safe_l + 90.0), c.h * 0.5);
    let mut g = Dive {
        candies: Vec::new(),
        layer: 0,
        frog,
        start: frog,
        target: frog,
        swim_t: 9.0,
        angle: 0.0,
        stun: 0.0,
        knock_from: frog,
        knock_to: frog,
        theta: c.rng.f() * 6.0,
        k: (2 + c.rng.below(4)) as f32,
        puff: 9.0,
        hit: false,
        bonus: 3.0,
        bubbles: Vec::new(),
    };
    g.place_layer(c);
    Box::new(g)
}

impl Dive {
    fn grid(c: &Ctx, col: f32, row: f32) -> V2 {
        v2(c.sx(0.1 + 0.8 * col / 42.0), c.h * (0.16 + 0.76 * row / 28.0))
    }
    /// Lays out `n` sweets as a symmetric shape on the classic 43x29 grid.
    fn place_layer(&mut self, c: &mut Ctx) {
        let n = COUNTS[self.layer % COUNTS.len()];
        let kind = ((self.layer / 8) % 4) as u32;
        let (cx, cy) = (21.0 + c.rng.range(-6.0, 6.0), 13.0 + c.rng.range(-3.0, 3.0));
        let shape = if n <= 2 { 0 } else { c.rng.below(5) };
        let mut pts: Vec<(f32, f32)> = Vec::new();
        match shape {
            0 => {
                // horizontal row
                let gap = (30.0 / n as f32).min(5.0);
                for i in 0..n {
                    pts.push((cx + (i as f32 - (n as f32 - 1.0) * 0.5) * gap, cy));
                }
            }
            1 => {
                // ring
                let rad = 3.0 + n as f32 * 0.45;
                for i in 0..n {
                    let a = i as f32 / n as f32 * TAU;
                    pts.push((cx + a.cos() * rad * 1.4, cy + a.sin() * rad));
                }
            }
            2 => {
                // diamond outline
                for i in 0..n {
                    let t = i as f32 / n as f32 * 4.0;
                    let seg = t.floor();
                    let f = t - seg;
                    let corners = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];
                    let a = corners[seg as usize];
                    let b = corners[seg as usize + 1];
                    let s = 4.0 + n as f32 * 0.35;
                    pts.push((cx + (a.0 + (b.0 - a.0) * f) * s * 1.5, cy + (a.1 + (b.1 - a.1) * f) * s));
                }
            }
            3 => {
                // block grid
                let cols = (n as f32).sqrt().ceil() as usize;
                for i in 0..n {
                    let (col, row) = (i % cols, i / cols);
                    pts.push((cx + (col as f32 - (cols as f32 - 1.0) * 0.5) * 3.5, cy + (row as f32 - (n / cols) as f32 * 0.5) * 3.0));
                }
            }
            _ => {
                // wave
                for i in 0..n {
                    let x = (i as f32 - (n as f32 - 1.0) * 0.5) * (32.0 / n as f32).min(4.0);
                    pts.push((cx + x, cy + (x * 0.5).sin() * 5.0));
                }
            }
        }
        self.candies = pts
            .into_iter()
            .map(|(col, row)| Candy { pos: Self::grid(c, col.clamp(2.0, 40.0), row.clamp(1.0, 27.0)), kind, age: 0.0 })
            .collect();
    }
    fn urchin_pos(&self, c: &Ctx) -> V2 {
        let a = 0.4 * c.h;
        let r = a * (self.k * self.theta).cos();
        v2(c.center_x() + r * self.theta.cos() * 1.5, c.h * 0.55 + r * self.theta.sin())
    }
}

impl MiniGame for Dive {
    fn idle(&mut self, c: &mut Ctx, dt: f32) {
        self.theta += 0.3 * dt;
        for cd in self.candies.iter_mut() {
            cd.age += dt;
        }
        let _ = c;
    }

    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.theta += 0.3 * dt;
        self.puff += dt;
        for cd in self.candies.iter_mut() {
            cd.age += dt;
        }
        for b in self.bubbles.iter_mut() {
            b.1 += dt;
            b.0.y -= 120.0 * dt;
        }
        self.bubbles.retain(|b| b.1 < 1.0);
        // movement
        if self.stun > 0.0 {
            self.stun -= dt;
            let k = ease_in_out(1.0 - (self.stun / 0.2).max(0.0));
            self.frog = self.knock_from.lerp(self.knock_to, k);
        } else if self.swim_t < 1.1 {
            self.swim_t += dt;
            let st = self.swim_t - 0.1;
            if st > 0.0 {
                let k = (st / 0.5).min(1.0);
                let e = if k < 0.5 { 2.0 * k * k } else { 1.0 - (-2.0 * k + 2.0).powi(2) / 2.0 };
                self.frog = self.start.lerp(self.target, e);
                if c.rng.chance(dt * 12.0) {
                    self.bubbles.push((self.frog + v2(c.rng.range(-20.0, 20.0), 30.0), 0.0));
                }
            }
        }
        if self.swim_t > 0.6 {
            self.angle *= (1.0 - dt * 6.0).max(0.0);
        }
        // pufferfish
        let u = self.urchin_pos(c);
        if self.stun <= 0.0 && u.dist(self.frog) < 0.13 * c.h {
            self.hit = true;
            self.stun = 0.2;
            self.puff = 0.0;
            self.swim_t = 9.0;
            let away = (self.frog - u).norm();
            self.knock_from = self.frog;
            self.knock_to = self.frog + away * 0.8 * 130.0;
            c.sfx(Sfx::Thud);
            c.heavy();
            c.shake = 0.2;
        }
        // collection
        let f = self.frog;
        let before = self.candies.len();
        let mut got = Vec::new();
        self.candies.retain(|cd| {
            let hit = (cd.pos.x - f.x).abs() < 44.0 * 1.5 * 0.5 + 20.0 && (cd.pos.y - f.y).abs() < 44.0 * 1.5 * 0.5 + 20.0;
            if hit {
                got.push(cd.pos);
            }
            !hit
        });
        for p in got {
            c.add_score(1, p - v2(0.0, 40.0));
            c.sfx_ex(Sfx::Coin, 0.55, 1.1);
            c.fx.burst(p, hex(0xBFE9FF), 8, 220.0);
            if !self.hit && c.score >= 100 {
                c.special_hit = true;
            }
        }
        if before > 0 && self.candies.is_empty() {
            if self.layer >= 43 {
                self.bonus = 2.0;
            }
            c.add_time(self.bonus, v2(c.center_x(), c.h * 0.4));
            self.layer += 1;
            self.place_layer(c);
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.stun > 0.0 {
            return;
        }
        self.start = self.frog;
        self.target = v2(t.pos.x.clamp(c.safe_l + 50.0, c.safe_r - 50.0), t.pos.y.clamp(120.0, c.h - 50.0));
        self.swim_t = 0.0;
        let d = self.target - self.start;
        self.angle = d.x.atan2(-d.y).clamp(-PI, PI) * 0.6;
        c.sfx_ex(Sfx::Whoosh, 0.4, 1.6);
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0x3AB8E8), hex(0x0E3F7A));
        // light rays
        for i in 0..6 {
            let x = c.safe_l + i as f32 * 300.0 + (c.anim * 20.0).sin() * 30.0;
            r.tri(v2(x, 0.0), v2(x + 120.0, 0.0), v2(x + 260.0, h), Col::WHITE.a(0.05));
        }
        // seabed
        r.grad_rect(0.0, h - 70.0, w, 70.0, hex(0xE8C98A), hex(0xC9A060));
        for i in 0..16 {
            let x = c.safe_l + i as f32 * 120.0;
            let sway = (c.anim * 1.5 + i as f32).sin() * 8.0;
            let col = [hex(0xFF7A9C), hex(0x8BE04E), hex(0xFFB347)][i % 3];
            r.line_outlined(v2(x, h - 60.0), v2(x + sway, h - 130.0 - (i % 4) as f32 * 20.0), 16.0, col, col.shade(-0.4), 3.0);
        }
        for b in &self.bubbles {
            r.ring(b.0.x, b.0.y, 6.0 + b.1 * 4.0, 2.0, Col::WHITE.a(0.7 * (1.0 - b.1)));
        }
        for cd in &self.candies {
            let s = ease_out_back((cd.age / 0.4).min(1.0));
            let bob = ((cd.age / 1.5) % 2.0 - 1.0).abs() * 6.0;
            art::candy(r, cd.pos.x, cd.pos.y - bob, 58.0 * s, cd.kind, 0.0);
        }
        // pufferfish
        let u = self.urchin_pos(c);
        let ps = if self.puff < 0.04 { 1.0 + self.puff / 0.04 * 0.7 } else if self.puff < 0.64 { 1.7 - (self.puff - 0.04) / 0.6 * 0.7 } else { 1.0 };
        r.push();
        r.translate(u.x, u.y);
        r.scale(ps, ps);
        for i in 0..12 {
            let a = i as f32 / 12.0 * TAU + c.anim;
            r.tri(v2(a.cos() * 40.0, a.sin() * 40.0), v2((a + 0.2).cos() * 40.0, (a + 0.2).sin() * 40.0), v2((a + 0.1).cos() * 62.0, (a + 0.1).sin() * 62.0), hex(0xC9752A));
        }
        r.circle(0.0, 0.0, 46.0, hex(0xFFB347), art::INK, 4.0);
        r.circle(-14.0, -10.0, 9.0, Col::WHITE, art::INK, 3.0);
        r.circle(14.0, -10.0, 9.0, Col::WHITE, art::INK, 3.0);
        r.disc(-14.0, -9.0, 4.0, art::INK);
        r.disc(14.0, -9.0, 4.0, art::INK);
        r.ellipse(0.0, 14.0, 10.0, 6.0, hex(0x7A3A12), Col::CLEAR, 0.0);
        r.pop();
        // frog
        r.push();
        r.translate(self.frog.x, self.frog.y);
        r.rotate(self.angle);
        let pose = Pose { legs_out: if self.swim_t < 0.6 { 1.0 } else { 0.3 }, dizzy: self.stun > 0.0, mouth: 0.2, ..Pose::idle(c.anim, 0.0) };
        art::frog(r, 0.0, 0.0, 110.0, art::EMERALD, &pose);
        r.pop();
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rrect(x, y, s * 1.2, s * 0.95, 16.0, hex(0x1F78B4), Col::CLEAR, 0.0);
    art::candy(r, x + s * 0.3, y - s * 0.2, s * 0.18, 0, 0.0);
    art::candy(r, x + s * 0.32, y + s * 0.1, s * 0.18, 1, 0.0);
    let k = (t * 1.2).sin() * s * 0.05;
    art::frog(r, x - s * 0.15 + k, y, s * 0.38, art::EMERALD, &Pose { legs_out: 1.0, ..Pose::idle(t, 0.0) });
    r.ring(x - s * 0.4, y - s * 0.3, s * 0.03, 2.0, Col::WHITE);
}
