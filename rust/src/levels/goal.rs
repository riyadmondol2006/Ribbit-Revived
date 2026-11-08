//! Frog Soccer (2 players): drag your frog around your half and bump the ball into the
//! opposite goal (the whole side edge). First to 3 goals wins. Contact pushes the ball with a
//! force that grows with the overlap squared, like the classic tuning.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};

const FW: f32 = 207.0;
const FH: f32 = 243.0;

pub struct Goal {
    frogs: [V2; 2],
    target: [V2; 2],
    owner: [Option<(i32, V2)>; 2],
    ball: V2,
    vel: V2,
    rot: f32,
    in_contact: bool,
    goals: [u32; 2],
    spawn_t: f32,
    flash: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let cx = c.center_x();
    let w = c.safe_r - c.safe_l;
    let frogs = [v2(cx - w * 0.5 + FW * 0.6, c.h * 0.6), v2(cx + w * 0.5 - FW * 0.6, c.h * 0.6)];
    Box::new(Goal { frogs, target: frogs, owner: [None, None], ball: v2(cx, c.h * 0.55), vel: v2(0.0, 0.0), rot: 0.0, in_contact: false, goals: [0, 0], spawn_t: 0.0, flash: 0.0 })
}

impl Goal {
    fn frog_r(c: &Ctx) -> f32 {
        0.13 * c.h
    }
    fn ball_r(c: &Ctx) -> f32 {
        0.066 * c.h
    }
    fn clamp_frog(c: &Ctx, side: usize, p: V2) -> V2 {
        let cx = c.center_x();
        let half = (c.safe_r - c.safe_l) * 0.5;
        let (lo, hi) = if side == 0 { (cx - half + 0.4 * FW, cx - 0.4 * FW) } else { (cx + 0.4 * FW, cx + half - 0.4 * FW) };
        v2(p.x.clamp(lo, hi), p.y.clamp(c.h * 0.2 + 0.4 * FH * 0.5, c.h - 0.4 * FH))
    }
    fn reset_ball(&mut self, c: &Ctx) {
        self.ball = v2(c.center_x(), c.h * 0.55);
        self.vel = v2(0.0, 0.0);
        self.spawn_t = 0.0;
        for f in self.frogs {
            if f.dist(self.ball) < Self::frog_r(c) + Self::ball_r(c) {
                self.ball.y = if f.y > self.ball.y { c.h * 0.3 } else { c.h * 0.85 };
            }
        }
    }
}

impl Goal {
    /// Computer plays blue: get behind the ball and shove it toward red's goal,
    /// otherwise guard the right-hand goal at the ball's height.
    fn cpu_think(&mut self, c: &mut Ctx) {
        let (fr, br) = (Self::frog_r(c), Self::ball_r(c));
        let own_half = self.ball.x > c.center_x() - fr * 0.5;
        let target = if own_half {
            let aim = v2(c.safe_l, c.h * 0.6);
            let dir = (self.ball - aim).norm();
            // approach from behind; once lined up, drive through the ball
            let behind = self.ball + dir * (fr + br - 12.0);
            if self.frogs[1].dist(behind) < 30.0 { self.ball - dir * 40.0 } else { behind }
        } else {
            v2(c.safe_r - (c.safe_r - c.center_x()) * 0.35, self.ball.y)
        };
        // a little slower than a human finger so it can be beaten
        let cur = self.frogs[1];
        let step = (target - cur) * 0.85;
        self.owner[1] = Some((-77, v2(0.0, 0.0)));
        self.target[1] = cur + step;
    }
}

impl MiniGame for Goal {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        let ms = dt * 1000.0;
        self.spawn_t += dt;
        if c.cpu {
            self.cpu_think(c);
        }
        self.flash = (self.flash - dt).max(0.0);
        // frogs chase their finger targets
        let max_step = c.h / 400.0 * ms;
        for i in 0..2 {
            if self.owner[i].is_some() {
                let d = self.target[i] - self.frogs[i];
                let len = d.len();
                self.frogs[i] = if len <= max_step { self.target[i] } else { self.frogs[i] + d * (max_step / len) };
            }
            self.frogs[i] = Self::clamp_frog(c, i, self.frogs[i]);
        }
        if c.end_now {
            return;
        }
        let (fr, br) = (Self::frog_r(c), Self::ball_r(c));
        let mut push = v2(0.0, 0.0);
        let mut contact = false;
        for f in self.frogs {
            let d = f.dist(self.ball);
            if d <= fr + br {
                contact = true;
                let ov = (fr + br - d) / 1.5; // overlap in classic pixels
                push += (self.ball - f).norm() * ((ov * ov + 1.0).floor() * 1.5);
            }
        }
        if !contact && self.in_contact {
            c.sfx_var(Sfx::Kick);
        }
        self.in_contact = contact;
        if self.vel.x * push.x < 0.0 {
            self.vel.x -= self.vel.x * (ms / 200.0).min(1.0);
        }
        if self.vel.y * push.y < 0.0 {
            self.vel.y -= self.vel.y * (ms / 200.0).min(1.0);
        }
        self.vel += push * (ms / 100.0);
        self.vel = self.vel * (1.0 - ms / 4000.0);
        let cap = 1.5 * c.h;
        if !contact && self.vel.len() > cap {
            self.vel = self.vel.norm() * cap;
        }
        self.ball += self.vel * dt;
        self.rot += self.vel.x * dt / br;
        let top = c.h * 0.2;
        let bottom = c.h - 20.0;
        let mut wall = false;
        if self.ball.y - br < top {
            self.ball.y = top + br;
            self.vel.y = self.vel.y.abs();
            wall = true;
        } else if self.ball.y + br > bottom {
            self.ball.y = bottom - br;
            self.vel.y = -self.vel.y.abs();
            wall = true;
        }
        if wall && !contact {
            c.sfx_var(Sfx::Kick);
            let (a, b) = (c.rng.range(0.95, 1.04), c.rng.range(0.95, 1.04));
            self.vel.x *= a;
            self.vel.y *= b;
        }
        // goals: the whole side edge
        let scorer = if self.ball.x < c.safe_l - br {
            Some(1)
        } else if self.ball.x > c.safe_r + br {
            Some(0)
        } else {
            None
        };
        if let Some(s) = scorer {
            self.goals[s] += 1;
            self.flash = 1.2;
            c.sfx(Sfx::Whistle);
            c.heavy();
            c.fx.confetti(v2(c.center_x(), c.h * 0.4), 30);
            if self.goals[s] >= 3 {
                c.winner = Some(s as i8);
                c.end_now = true;
            } else {
                self.reset_ball(c);
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        match t.phase {
            Phase::Down => {
                let side = if t.pos.x >= c.center_x() { 1 } else { 0 };
                if c.cpu && side == 1 {
                    return;
                }
                if self.owner[side].is_none() && t.pos.dist(self.frogs[side]) < Self::frog_r(c) * 1.6 {
                    let off = t.pos - self.frogs[side];
                    self.owner[side] = Some((t.id, off));
                    self.target[side] = self.frogs[side];
                }
            }
            Phase::Move => {
                for i in 0..2 {
                    if let Some((id, off)) = self.owner[i] {
                        if id == t.id {
                            self.target[i] = t.pos - off;
                        }
                    }
                }
            }
            Phase::Up | Phase::Cancel => {
                for o in self.owner.iter_mut() {
                    if o.map(|x| x.0) == Some(t.id) {
                        *o = None;
                    }
                }
            }
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        // pitch stripes
        for i in 0..((w / 120.0) as i32 + 1) {
            let col = if i % 2 == 0 { hex(0x5DBB55) } else { hex(0x4FA84A) };
            r.rect(i as f32 * 120.0, 0.0, 120.0, h, col);
        }
        let cx = c.center_x();
        let top = h * 0.2;
        r.rect(0.0, top - 6.0, w, 6.0, Col::WHITE.a(0.8));
        r.rect(cx - 3.0, top, 6.0, h - top, Col::WHITE.a(0.8));
        r.ring(cx, (top + h) * 0.5, 110.0, 6.0, Col::WHITE.a(0.8));
        // goals
        for (x, col) in [(c.safe_l, hex(0xF0605D)), (c.safe_r, hex(0x4FA7F2))] {
            r.rrect(x, (top + h) * 0.5, 30.0, (h - top) * 0.8, 10.0, col, art::INK, 4.0);
            for k in 0..8 {
                let y = top + (h - top) * 0.1 + k as f32 * (h - top) * 0.1;
                r.rect(x - 15.0, y, 30.0, 3.0, Col::WHITE.a(0.6));
            }
        }
        // score board
        art::pill(r, cx, 58.0, 200.0, 76.0, hex(0x23306E));
        r.text_bold(&format!("{} : {}", self.goals[0], self.goals[1]), cx, 60.0, 50.0, Align::Center, Col::WHITE, art::INK);
        r.disc(cx - 120.0, 58.0, 16.0, hex(0xF0605D));
        r.disc(cx + 120.0, 58.0, 16.0, hex(0x4FA7F2));
        // ball shadow + ball
        let br = Self::ball_r(c);
        let s = ease_out_back((self.spawn_t / 0.2).min(1.0));
        r.ellipse(self.ball.x + 8.0, self.ball.y + br * 0.9, br * s, br * 0.3 * s, art::SHADOW, art::SHADOW, 0.0);
        // frogs
        for i in 0..2 {
            let skin = if i == 0 { art::RUBY } else { art::SKY };
            let f = self.frogs[i];
            let near = self.ball.dist(f) < 260.0;
            let pose = Pose { look: (self.ball - f).norm(), mouth: if near { 0.5 } else { 0.0 }, ..Pose::idle(c.anim, i as f32) };
            if self.owner[i].is_some() {
                r.ring(f.x, f.y + 40.0, Self::frog_r(c), 4.0, Col::WHITE.a(0.7));
            }
            art::frog(r, f.x, f.y, FW * 0.85, skin, &pose);
        }
        art::ball(r, self.ball.x, self.ball.y, br * s, self.rot, hex(0xFFFFFF));
        if self.flash > 0.0 {
            r.push();
            r.fade(self.flash.min(1.0));
            r.text_bold("GOAL!", cx, h * 0.45, 120.0, Align::Center, hex(0xFFD23F), art::INK);
            r.pop();
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rrect(x, y, s * 1.2, s * 0.95, 16.0, hex(0x5DBB55), Col::CLEAR, 0.0);
    art::frog(r, x - s * 0.3, y + s * 0.05, s * 0.36, art::RUBY, &Pose::idle(t, 0.0));
    art::frog(r, x + s * 0.3, y + s * 0.05, s * 0.36, art::SKY, &Pose::idle(t, 1.0));
    art::ball(r, x + (t * 2.0).sin() * s * 0.1, y + s * 0.2, s * 0.09, t * 3.0, Col::WHITE);
}
