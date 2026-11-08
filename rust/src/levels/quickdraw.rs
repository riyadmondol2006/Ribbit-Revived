//! Quick Draw (2 players): wait for the START sign to light up, then tap your own frog first
//! to shove your rival out. Fake-outs (a buzzing fly, sparks, flickering letters) try to bait
//! a false start: tapping early hands the win to the other player.
use super::{Ctx, MiniGame, STAGE_W};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};

#[derive(Clone, Copy, PartialEq)]
enum Fake {
    None,
    Fly(f32, f32),
    Sparks(f32, f32),
    Letter(usize, f32),
    Glow(f32),
}

pub struct QuickDraw {
    t: f32,
    t_on: f32,
    next_fake: f32,
    fake: Fake,
    lit: bool,
    off: [f32; 2],
    shown: [f32; 2],
    squash: [f32; 2],
    over: Option<f32>,
    loser: Option<usize>,
    wrong: Option<(V2, f32)>,
    cpu_react: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let t_on = c.rng.range(5.0, 20.0);
    let next_fake = c.rng.range(2.0, 8.0);
    Box::new(QuickDraw { t: 0.0, t_on, next_fake, fake: Fake::None, lit: false, off: [0.0; 2], shown: [0.0; 2], squash: [9.0; 2], over: None, loser: None, wrong: None, cpu_react: c.rng.range(0.32, 0.6) })
}

impl QuickDraw {
    fn frog_pos(&self, c: &Ctx, side: usize) -> V2 {
        let base = if side == 0 { c.sx(0.3) } else { c.sx(0.7) };
        v2(base + self.shown[side], c.h * 0.66)
    }
    fn hit_frog(&self, c: &Ctx, side: usize, p: V2) -> bool {
        let f = self.frog_pos(c, side);
        (p.x - f.x).abs() < 150.0 && (p.y - f.y).abs() < 170.0
    }
    fn finish(&mut self, c: &mut Ctx, winner: i8) {
        if self.over.is_none() {
            self.over = Some(0.0);
            c.winner = Some(winner);
        }
    }
}

impl MiniGame for QuickDraw {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        for i in 0..2 {
            self.shown[i] += (self.off[i] - self.shown[i]) * (dt / 0.1).min(1.0);
            self.squash[i] += dt;
        }
        if let Some((_, t)) = &mut self.wrong {
            *t += dt;
        }
        if let Some(t) = &mut self.over {
            *t += dt;
            if *t > 1.0 {
                c.end_now = true;
            }
            return;
        }
        if !self.lit && self.t >= self.t_on {
            self.lit = true;
            self.fake = Fake::None;
            c.sfx(Sfx::Go);
            c.heavy();
        }
        if self.lit && self.t >= self.t_on + 10.0 {
            self.finish(c, -1);
        }
        // computer (blue) never false-starts and reacts after 0.32-0.6 s
        if c.cpu && self.lit && self.over.is_none() && self.t >= self.t_on + self.cpu_react {
            self.cpu_react = f32::MAX;
            self.squash[1] = 0.0;
            c.sfx(Sfx::Kick);
            let push = 0.6 * STAGE_W;
            self.off[0] -= push;
            self.off[1] -= push;
        }
        // fake-outs while the sign is still dark
        if !self.lit && self.t >= self.next_fake {
            self.next_fake = self.t + c.rng.range(3.0, 18.0);
            self.fake = match c.rng.below(5) {
                0 => {
                    c.sfx_ex(Sfx::Whistle, 0.25, 2.2);
                    Fake::Fly(0.0, c.rng.range(1.0, 3.0))
                }
                1 => Fake::None,
                2 => {
                    c.sfx(Sfx::Burst);
                    let xs = [0.09, 0.16, 0.23, 0.77, 0.84, 0.91];
                    Fake::Sparks(*c.rng.pick(&xs), 0.0)
                }
                3 => {
                    c.sfx_ex(Sfx::Tick, 0.6, 0.8);
                    Fake::Letter(c.rng.below(5), 0.0)
                }
                _ => {
                    c.sfx_ex(Sfx::Tick, 0.6, 1.2);
                    Fake::Glow(0.0)
                }
            };
        }
        self.fake = match self.fake {
            Fake::Fly(t, d) if t + dt < d => Fake::Fly(t + dt, d),
            Fake::Sparks(x, t) if t + dt < 0.8 => Fake::Sparks(x, t + dt),
            Fake::Letter(k, t) if t + dt < 1.0 => Fake::Letter(k, t + dt),
            Fake::Glow(t) if t + dt < 1.0 => Fake::Glow(t + dt),
            _ => Fake::None,
        };
        // knock-out
        let rel0 = (self.frog_pos(c, 0).x - c.x0()) / STAGE_W;
        let rel1 = (self.frog_pos(c, 1).x - c.x0()) / STAGE_W;
        if rel0 < 0.1 {
            self.loser = Some(0);
            c.sfx(Sfx::Whistle);
            self.finish(c, 1);
        } else if rel1 > 0.9 {
            self.loser = Some(1);
            c.sfx(Sfx::Whistle);
            self.finish(c, 0);
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.over.is_some() {
            return;
        }
        let red = self.hit_frog(c, 0, t.pos);
        let blue = self.hit_frog(c, 1, t.pos) && !c.cpu;
        if !self.lit {
            // false start
            self.wrong = Some((t.pos, 0.0));
            c.mistake(t.pos);
            let w = if red { 1 } else if blue { 0 } else { -1 };
            self.finish(c, w);
            return;
        }
        let side = if red { 0 } else if blue { 1 } else { return };
        self.squash[side] = 0.0;
        c.sfx(Sfx::Kick);
        c.tick();
        let push = 0.6 * STAGE_W;
        if side == 0 {
            self.off[0] += push;
            self.off[1] += push;
        } else {
            self.off[0] -= push;
            self.off[1] -= push;
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0x5A2B1F), hex(0x2B1410));
        // wooden stage
        r.grad_rect(0.0, h * 0.8, w, h * 0.2, hex(0xB07A45), hex(0x7A4A25));
        for i in 0..((w / 140.0) as i32 + 1) {
            r.rect(i as f32 * 140.0, h * 0.8, 3.0, h * 0.2, hex(0x5A3418));
        }
        // sign
        let sx = c.center_x();
        let sy = h * 0.27;
        r.rrect(sx, sy, 560.0, 150.0, 24.0, hex(0x3A2A6B), art::INK, 6.0);
        let glow = match self.fake {
            Fake::Glow(t) => ((t / 0.05) as i32 % 2) == 0,
            _ => false,
        };
        if self.lit || glow {
            r.glow(sx, sy, 380.0, hex(0xFFE27A).a(if self.lit { 0.8 } else { 0.4 }));
        }
        for (i, ch) in ["S", "T", "A", "R", "T"].iter().enumerate() {
            let lit_letter = self.lit || matches!(self.fake, Fake::Letter(k, t) if k == i && ((t / 0.05) as i32 % 2) == 0);
            let col = if lit_letter { hex(0xFFE27A) } else { hex(0x6B5A8F) };
            r.text_bold(ch, sx - 200.0 + i as f32 * 100.0, sy, 100.0, Align::Center, col, art::INK);
        }
        for i in 0..14 {
            let a = i as f32 / 14.0 * std::f32::consts::TAU;
            let on = self.lit && ((self.t * 8.0) as i32 + i) % 2 == 0;
            r.disc(sx + a.cos() * 300.0, sy + a.sin() * 92.0, 8.0, if on { hex(0xFFE27A) } else { hex(0x8A7A55) });
        }
        // fakes
        match self.fake {
            Fake::Fly(t, d) => {
                let x = -40.0 + (w + 80.0) * (t / d);
                let y = h * 0.45 + (t * 14.0).sin() * 20.0;
                r.ellipse(x, y, 10.0, 7.0, art::INK, art::INK, 0.0);
                r.ellipse(x - 4.0, y - 8.0, 8.0, 4.0, Col::WHITE.a(0.7), Col::CLEAR, 0.0);
            }
            Fake::Sparks(fx, t) => {
                let p = v2(c.sx(fx), h * 0.12);
                for k in 0..10 {
                    let a = k as f32 * 0.63;
                    r.disc(p.x + a.cos() * t * 160.0, p.y + a.sin() * t * 160.0, 6.0 * (1.0 - t / 0.8), hex(0xFFE27A));
                }
            }
            _ => {}
        }
        // frogs
        for side in 0..2 {
            let skin = if side == 0 { art::RUBY } else { art::SKY };
            let mut p = self.frog_pos(c, side);
            let mut rot = 0.0;
            if self.loser == Some(side) {
                if let Some(t) = self.over {
                    let k = (t / 0.4).min(1.0);
                    p.y += k * h * 0.7;
                    rot = if side == 0 { -0.35 } else { 0.35 } * k;
                }
            }
            let sq = if self.squash[side] < 0.2 { (self.squash[side] / 0.2 * std::f32::consts::PI).sin() * 0.1 } else { 0.0 };
            r.push();
            r.translate(p.x, p.y);
            r.rotate(rot);
            let pose = Pose { squash: -sq, mouth: if self.lit { 0.6 } else { 0.0 }, look: v2(if side == 0 { 0.6 } else { -0.6 }, -0.5), ..Pose::idle(c.anim, side as f32 * 3.0) };
            art::frog(r, 0.0, 0.0, 260.0, skin, &pose);
            r.pop();
        }
        if let Some((p, t)) = self.wrong {
            if t < 0.6 {
                let a = (t / 0.6 * std::f32::consts::PI).sin();
                r.text_bold("X", p.x, p.y, 130.0, Align::Center, hex(0xFF5D5D).a(a), art::INK.a(a));
            }
        }
        let hint = if self.lit { "TAP YOUR FROG NOW!" } else { "Wait for the light..." };
        r.text_bold(hint, sx, h * 0.47, 36.0, Align::Center, if self.lit { hex(0xFFE27A) } else { Col::WHITE }, art::INK);
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    let lit = (t * 1.5).fract() > 0.5;
    r.rrect(x, y - s * 0.28, s * 0.8, s * 0.22, 8.0, hex(0x3A2A6B), art::INK, 3.0);
    r.text_bold("START", x, y - s * 0.28, s * 0.15, Align::Center, if lit { hex(0xFFE27A) } else { hex(0x6B5A8F) }, art::INK);
    art::frog(r, x - s * 0.24, y + s * 0.15, s * 0.36, art::RUBY, &Pose::idle(t, 0.0));
    art::frog(r, x + s * 0.24, y + s * 0.15, s * 0.36, art::SKY, &Pose::idle(t, 1.0));
}
