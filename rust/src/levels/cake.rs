//! Cake Duel (2 players, party bonus game): a cake pops up between two hungry frogs. Tap your
//! frog to flick its tongue; whoever licks a ready cake first scores it. First to 3 cakes wins.
//! Licking too early just misses, so spamming is allowed.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

#[derive(Clone, Copy, PartialEq)]
enum CakeState {
    Appearing(f32),
    Ready,
    Grabbed(usize, f32),
    Recharge(f32),
}

pub struct Cake {
    state: CakeState,
    licks: [f32; 2],
    lick_len: [f32; 2],
    score: [u32; 2],
    done: Option<f32>,
    clouds: Vec<(f32, f32, f32)>,
    cpu_wait: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let clouds = (0..5).map(|_| (c.rng.f(), c.rng.range(0.1, 0.35), c.rng.range(0.6, 1.1))).collect();
    Box::new(Cake { state: CakeState::Appearing(0.0), licks: [9.0; 2], lick_len: [0.1; 2], score: [0, 0], done: None, clouds, cpu_wait: -1.0 })
}

impl Cake {
    fn frog_pos(c: &Ctx, side: usize) -> V2 {
        v2(if side == 0 { c.sx(0.25) } else { c.sx(0.75) }, c.h * 0.66)
    }
    fn cake_pos(c: &Ctx) -> V2 {
        v2(c.center_x(), c.h * 0.5)
    }
}

impl MiniGame for Cake {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        for l in self.licks.iter_mut() {
            *l += dt;
        }
        for cl in self.clouds.iter_mut() {
            cl.0 -= dt * 0.03 * cl.2;
            if cl.0 < -0.2 {
                cl.0 = 1.2;
            }
        }
        if let Some(t) = &mut self.done {
            *t += dt;
            if *t > 1.0 {
                c.end_now = true;
            }
            return;
        }
        // computer (blue) licks 0.35-0.75 s after a cake is ready
        if c.cpu {
            if self.state == CakeState::Ready {
                if self.cpu_wait < 0.0 {
                    self.cpu_wait = c.rng.range(0.35, 0.75);
                }
                self.cpu_wait -= dt;
                if self.cpu_wait <= 0.0 && self.licks[1] >= self.lick_len[1] {
                    self.state = CakeState::Grabbed(1, 0.0);
                    self.licks[1] = 0.0;
                    self.lick_len[1] = 0.2;
                    c.sfx(Sfx::Eat);
                    self.cpu_wait = -1.0;
                }
            } else {
                self.cpu_wait = -1.0;
            }
        }
        self.state = match self.state {
            CakeState::Appearing(t) if t + dt >= 0.2 => CakeState::Ready,
            CakeState::Appearing(t) => CakeState::Appearing(t + dt),
            CakeState::Grabbed(side, t) if t + dt >= 0.2 => {
                self.score[side] += 1;
                c.sfx(Sfx::Collect);
                c.confirm();
                c.fx.stars(Self::frog_pos(c, side) - v2(0.0, 200.0), 8);
                if self.score[side] >= 3 {
                    c.winner = Some(side as i8);
                    self.done = Some(0.0);
                    c.sfx(Sfx::Win);
                }
                CakeState::Recharge(c.rng.range(1.0, 4.0))
            }
            CakeState::Grabbed(side, t) => CakeState::Grabbed(side, t + dt),
            CakeState::Recharge(t) if t - dt <= 0.0 => CakeState::Appearing(0.0),
            CakeState::Recharge(t) => CakeState::Recharge(t - dt),
            s => s,
        };
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.done.is_some() {
            return;
        }
        let side = if t.pos.x < c.center_x() { 0 } else { 1 };
        if c.cpu && side == 1 {
            return;
        }
        let f = Self::frog_pos(c, side);
        if (t.pos.x - f.x).abs() > 200.0 || (t.pos.y - f.y).abs() > 220.0 {
            return;
        }
        if self.licks[side] < self.lick_len[side] {
            return;
        }
        match self.state {
            CakeState::Ready => {
                self.state = CakeState::Grabbed(side, 0.0);
                self.licks[side] = 0.0;
                self.lick_len[side] = 0.2;
                c.sfx(Sfx::Eat);
                c.tick();
            }
            CakeState::Grabbed(..) => {}
            _ => {
                self.licks[side] = 0.0;
                self.lick_len[side] = 0.1;
                c.sfx_ex(Sfx::Swap, 0.6, 1.5);
            }
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        art::sky(r, hex(0xFFC1E3), hex(0xFFF3D6));
        for cl in &self.clouds {
            art::cloud(r, cl.0 * w, cl.1 * h, cl.2, Col::WHITE.a(0.85));
        }
        // bunting
        for i in 0..((w / 80.0) as i32 + 1) {
            let x = i as f32 * 80.0;
            let col = [hex(0xF0605D), hex(0xFFD23F), hex(0x4FA7F2), hex(0x4CC38A)][i as usize % 4];
            r.tri(v2(x, 0.0), v2(x + 70.0, 0.0), v2(x + 35.0, 50.0), col);
        }
        r.grad_rect(0.0, h * 0.82, w, h * 0.18, hex(0x9BD86E), hex(0x6DA843));
        // stand
        let cp = Self::cake_pos(c);
        r.rrect(cp.x, cp.y + 150.0, 40.0, 180.0, 10.0, hex(0xE8EDF2), art::INK, 4.0);
        r.ellipse(cp.x, cp.y + 64.0, 120.0, 22.0, hex(0xE8EDF2), art::INK, 4.0);
        // the cake
        let (scale, pos) = match self.state {
            CakeState::Appearing(t) => (ease_out_back(t / 0.2), cp),
            CakeState::Ready => (1.0, cp),
            CakeState::Grabbed(side, t) => {
                let k = ((t - 0.1) / 0.1).clamp(0.0, 1.0);
                (1.0 - k * 0.6, cp.lerp(Self::frog_pos(c, side) - v2(0.0, 40.0), k))
            }
            CakeState::Recharge(_) => (0.0, cp),
        };
        if scale > 0.01 {
            r.push();
            r.translate(pos.x, pos.y);
            r.scale(scale, scale);
            r.rrect(0.0, 20.0, 150.0, 70.0, 14.0, hex(0xFFC1E3), art::INK, 4.0);
            r.rrect(0.0, -30.0, 110.0, 50.0, 14.0, hex(0xFFF4D6), art::INK, 4.0);
            for i in 0..5 {
                r.disc(-48.0 + i as f32 * 24.0, -6.0, 9.0, hex(0xF27BC4));
            }
            for i in 0..3 {
                let x = -30.0 + i as f32 * 30.0;
                r.rrect(x, -72.0, 10.0, 34.0, 4.0, [hex(0x4FA7F2), hex(0xFFD23F), hex(0x4CC38A)][i], art::INK, 2.0);
                r.ellipse(x, -96.0 + (c.anim * 10.0 + i as f32).sin() * 2.0, 6.0, 10.0, hex(0xFFB020), Col::CLEAR, 0.0);
            }
            r.pop();
        }
        // frogs with tongues
        for side in 0..2 {
            let skin = if side == 0 { art::RUBY } else { art::SKY };
            let f = Self::frog_pos(c, side);
            let lick = self.licks[side] < self.lick_len[side];
            if lick {
                let k = self.licks[side] / self.lick_len[side];
                let ext = if k < 0.5 { k * 2.0 } else { 2.0 - k * 2.0 };
                let mouth = f + v2(0.0, 30.0);
                let tip = mouth.lerp(cp, ext * 0.92);
                r.line_outlined(mouth, tip, 22.0, hex(0xFF6F8B), art::INK, 3.0);
            }
            let pose = Pose { mouth: if lick { 1.0 } else { 0.0 }, look: if side == 0 { v2(0.7, -0.4) } else { v2(-0.7, -0.4) }, ..Pose::idle(c.anim, side as f32) };
            art::frog(r, f.x, f.y, 230.0, skin, &pose);
            // cake counters
            for i in 0..3 {
                let x = f.x + (i as f32 - 1.0) * 66.0;
                let y = h * 0.22;
                let full = (i as u32) < self.score[side];
                r.rrect(x, y, 52.0, 40.0, 10.0, if full { hex(0xFFC1E3) } else { Col::WHITE.a(0.3) }, art::INK, 3.0);
                if full {
                    r.disc(x, y - 22.0, 6.0, hex(0xF0605D));
                }
            }
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rrect(x, y - s * 0.05, s * 0.3, s * 0.16, 6.0, hex(0xFFC1E3), art::INK, 2.0);
    r.rrect(x, y - s * 0.17, s * 0.2, s * 0.1, 6.0, hex(0xFFF4D6), art::INK, 2.0);
    art::frog(r, x - s * 0.33, y + s * 0.15, s * 0.3, art::RUBY, &Pose { mouth: 0.8, ..Pose::idle(t, 0.0) });
    art::frog(r, x + s * 0.33, y + s * 0.15, s * 0.3, art::SKY, &Pose::idle(t, 1.0));
}
