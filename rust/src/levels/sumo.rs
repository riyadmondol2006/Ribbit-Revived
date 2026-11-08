//! Belly Bump (2 players, 10 s): tap your own frog as fast as you can to shove forward.
//! Once the frogs touch, every shove pushes the rival back; push them past the ring edge to
//! win instantly. When time runs out, whoever moved the pair toward the other side wins.
use super::{Ctx, MiniGame, STAGE_W};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const FROG_W: f32 = 420.0;

pub struct Sumo {
    off: [f32; 2],  // displacement from the start position (positive = rightwards)
    shown: [f32; 2],
    push: [bool; 2],
    squash: [f32; 2],
    out: Option<(usize, f32)>,
    cpu_t: f32,
}

pub fn make(_c: &mut Ctx) -> Box<dyn MiniGame> {
    Box::new(Sumo { off: [0.0; 2], shown: [0.0; 2], push: [false; 2], squash: [9.0; 2], out: None, cpu_t: 0.6 })
}

impl Sumo {
    fn start_x(c: &Ctx, side: usize) -> f32 {
        c.center_x() + if side == 0 { -STAGE_W * 0.25 } else { STAGE_W * 0.25 }
    }
    fn x(&self, c: &Ctx, side: usize) -> f32 {
        Self::start_x(c, side) + self.off[side]
    }
    fn frog_y(c: &Ctx) -> f32 {
        c.h * 0.6
    }
}

impl MiniGame for Sumo {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        // computer plays blue: steady shoves at 5-9 per second
        if c.cpu && self.out.is_none() {
            self.cpu_t -= dt;
            if self.cpu_t <= 0.0 {
                self.cpu_t = c.rng.range(0.11, 0.2);
                self.push[1] = true;
                self.squash[1] = 0.0;
                c.sfx_ex(Sfx::Thud, 0.5, 1.05);
            }
        }
        let step = 0.1 * FROG_W;
        let contact = (self.x(c, 0) + FROG_W * 0.5 - 0.09 * FROG_W) >= (self.x(c, 1) - FROG_W * 0.5 + 0.09 * FROG_W);
        let (rp, bp) = (self.push[0], self.push[1]);
        if self.out.is_none() && (rp || bp) {
            if rp && bp {
                // both pushed in the same frame: they cancel out
            } else if rp {
                self.off[0] += step;
                if contact {
                    self.off[1] += step;
                }
            } else {
                self.off[1] -= step;
                if contact {
                    self.off[0] -= step;
                }
            }
        }
        self.push = [false; 2];
        for i in 0..2 {
            self.shown[i] += (self.off[i] - self.shown[i]) * (dt / 0.1).min(1.0);
            self.squash[i] += dt;
        }
        // ring-out
        if self.out.is_none() {
            let rel0 = (self.x(c, 0) - c.x0()) / STAGE_W;
            let rel1 = (self.x(c, 1) - c.x0()) / STAGE_W;
            if rel0 < 0.1 {
                self.out = Some((0, 0.0));
            } else if rel1 > 0.9 {
                self.out = Some((1, 0.0));
            }
            if let Some((loser, _)) = self.out {
                c.sfx(Sfx::Whistle);
                c.heavy();
                c.winner = Some(1 - loser as i8);
            } else {
                let d = self.off[0] + self.off[1];
                c.winner = Some(if d.abs() < 0.01 * FROG_W { -1 } else if d > 0.0 { 0 } else { 1 });
            }
        }
        if let Some((_, t)) = &mut self.out {
            *t += dt;
            if *t > 0.6 {
                c.end_now = true;
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.out.is_some() {
            return;
        }
        let fy = Self::frog_y(c);
        for side in 0..2 {
            if c.cpu && side == 1 {
                continue;
            }
            let x = self.x(c, side);
            if (t.pos.x - x).abs() < FROG_W * 0.5 && (t.pos.y - fy).abs() < FROG_W * 0.55 {
                // prefer the frog on the toucher's half when the sprites overlap
                let half_ok = if side == 0 { t.pos.x < c.center_x() + FROG_W * 0.1 } else { t.pos.x > c.center_x() - FROG_W * 0.1 };
                if !half_ok {
                    continue;
                }
                self.push[side] = true;
                self.squash[side] = 0.0;
                c.sfx_ex(Sfx::Thud, 0.7, if side == 0 { 0.9 } else { 1.05 });
                c.tick();
                break;
            }
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0x3A2A6B), hex(0x1F1840));
        // spotlights
        for (i, x) in [c.sx(0.25), c.sx(0.75)].iter().enumerate() {
            r.glow(*x, h * 0.5, 420.0, [hex(0xFF7A7A), hex(0x7AB8FF)][i].a(0.25));
        }
        // ring (dohyo)
        let ry = Self::frog_y(c) + FROG_W * 0.46;
        r.ellipse(c.center_x(), ry + 40.0, STAGE_W * 0.52, 90.0, hex(0xB08850), art::INK, 5.0);
        r.ellipse(c.center_x(), ry + 18.0, STAGE_W * 0.5, 80.0, hex(0xE8D3A0), art::INK, 5.0);
        r.ellipse(c.center_x(), ry + 18.0, STAGE_W * 0.4, 62.0, Col::CLEAR, hex(0xFFFFFF).a(0.8), 6.0);
        for x in [c.sx(0.1), c.sx(0.9)] {
            r.rrect(x, ry + 18.0, 10.0, 60.0, 4.0, hex(0xFF5D5D), art::INK, 3.0);
        }
        for side in 0..2 {
            let skin = if side == 0 { art::RUBY } else { art::SKY };
            let mut x = Self::start_x(c, side) + self.shown[side];
            let mut y = Self::frog_y(c);
            let mut rot = 0.0;
            let mut alpha = 1.0;
            if let Some((loser, t)) = self.out {
                if loser == side {
                    let k = (t / 0.4).min(1.0);
                    x += if side == 0 { -1.0 } else { 1.0 } * k * 120.0;
                    y += k * k * h * 0.7;
                    rot = if side == 0 { -0.35 } else { 0.35 } * k;
                    alpha = 1.0 - k * 0.3;
                }
            }
            let sq = if self.squash[side] < 0.2 { wobble(self.squash[side], 25.0) * 0.12 } else { 0.0 };
            r.push();
            r.translate(x, y);
            r.rotate(rot);
            r.fade(alpha);
            let pose = Pose { squash: sq, mouth: if self.squash[side] < 0.15 { 0.7 } else { 0.1 }, look: v2(if side == 0 { 0.9 } else { -0.9 }, 0.0), ..Pose::idle(c.anim, side as f32) };
            art::frog(r, 0.0, 0.0, FROG_W * 0.8, skin, &pose);
            // belt and headband
            let band = if side == 0 { hex(0xC0262D) } else { hex(0x1F5FBF) };
            r.rrect(0.0, FROG_W * 0.3, FROG_W * 0.62, 40.0, 16.0, band, art::INK, 4.0);
            r.rrect(0.0, -FROG_W * 0.25, FROG_W * 0.5, 22.0, 10.0, band, art::INK, 3.0);
            r.pop();
        }
        r.text_bold("Tap YOUR frog fast!", c.center_x(), 150.0, 34.0, crate::render::Align::Center, Col::WHITE, art::INK);
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    let k = (t * 4.0).sin() * s * 0.03;
    art::frog(r, x - s * 0.2 + k, y, s * 0.5, art::RUBY, &Pose::idle(t, 0.0));
    art::frog(r, x + s * 0.2 + k, y, s * 0.5, art::SKY, &Pose::idle(t, 1.0));
    r.rrect(x - s * 0.2 + k, y + s * 0.12, s * 0.3, s * 0.06, 4.0, hex(0xC0262D), art::INK, 2.0);
    r.rrect(x + s * 0.2 + k, y + s * 0.12, s * 0.3, s * 0.06, 4.0, hex(0x1F5FBF), art::INK, 2.0);
}
