//! Frog Volley (2 players): slide your frog along the sand to keep the ball off your side.
//! Frogs are super-bouncy (restitution 1.6), so a well-timed bump sends the ball flying.
//! When the ball touches the sand, the other side scores. First to 3 wins.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};

const BALL_R: f32 = 55.0;
const FROG_R: f32 = 87.0;
const FROG_HALF: f32 = 97.0;

pub struct Volley {
    frog_x: [f32; 2],
    frog_vx: [f32; 2],
    target: [Option<(i32, f32, f32)>; 2],
    ball: V2,
    vel: V2,
    rot: f32,
    hold: f32,
    score: [u32; 2],
    flash: f32,
    touch_sfx: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let side = c.rng.below(2);
    let mut g = Volley { frog_x: [c.sx(0.3), c.sx(0.7)], frog_vx: [0.0; 2], target: [None, None], ball: v2(0.0, 0.0), vel: v2(0.0, 0.0), rot: 0.0, hold: 0.0, score: [0, 0], flash: 0.0, touch_sfx: 0.0 };
    g.serve(c, side);
    Box::new(g)
}

impl Volley {
    fn floor(c: &Ctx) -> f32 {
        c.h - 0.07 * c.h
    }
    fn frog_y(c: &Ctx) -> f32 {
        Self::floor(c) - FROG_R + 14.0
    }
    fn net_top(c: &Ctx) -> f32 {
        c.h - 206.0
    }
    fn serve(&mut self, c: &Ctx, side: usize) {
        self.ball = v2(if side == 0 { c.sx(0.33) } else { c.sx(0.66) }, c.h * 0.3);
        self.vel = v2(0.0, 0.0);
        self.hold = 2.0;
    }
    fn bump_sfx(&mut self, c: &mut Ctx) {
        if self.touch_sfx <= 0.0 {
            c.sfx_var(Sfx::Bounce);
            self.touch_sfx = 0.08;
        }
    }
}

impl MiniGame for Volley {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.flash = (self.flash - dt).max(0.0);
        self.touch_sfx -= dt;
        if c.cpu {
            // computer plays blue: slide under where the ball will come down, a bit to its right
            let cx = c.center_x();
            let coming = self.ball.x > cx - 40.0 || self.vel.x > 0.0;
            let tx = if coming && self.hold <= 0.0 {
                let fall = ((Self::frog_y(c) - FROG_R - self.ball.y) / 600.0).clamp(0.05, 0.8);
                self.ball.x + self.vel.x * fall + 28.0
            } else {
                cx + (c.safe_r - cx) * 0.5
            };
            self.target[1] = Some((-77, tx.max(cx + 40.0), 0.0));
        }
        // frogs chase their finger x along the sand (max 10 m/s)
        let m = 5.0 / 6.0 * FROG_HALF;
        let cx = c.center_x();
        for i in 0..2 {
            let mut vx = 0.0;
            if let Some((_, tx, off)) = self.target[i] {
                let ratio = (tx - off - self.frog_x[i]) / FROG_HALF;
                vx = if ratio.abs() > 1.0 { 720.0 * ratio.signum() } else if ratio.abs() >= 0.3 { 720.0 * ratio } else { 0.0 };
            }
            self.frog_vx[i] = vx;
            self.frog_x[i] += vx * dt;
            let (lo, hi) = if i == 0 { (c.safe_l + m, cx - m) } else { (cx + m, c.safe_r - m) };
            self.frog_x[i] = self.frog_x[i].clamp(lo, hi);
        }
        if c.end_now {
            return;
        }
        if self.hold > 0.0 {
            self.hold -= dt;
            self.rot += dt;
            return;
        }
        // integrate the ball in small steps
        let steps = 4;
        let h = dt / steps as f32;
        for _ in 0..steps {
            self.vel.y += 432.0 * h;
            self.ball += self.vel * h;
            self.rot += self.vel.x * h / BALL_R;
            // walls and ceiling
            if self.ball.x - BALL_R < c.safe_l {
                self.ball.x = c.safe_l + BALL_R;
                self.vel.x = self.vel.x.abs() * 0.7;
                self.bump_sfx(c);
            } else if self.ball.x + BALL_R > c.safe_r {
                self.ball.x = c.safe_r - BALL_R;
                self.vel.x = -self.vel.x.abs() * 0.7;
                self.bump_sfx(c);
            }
            if self.ball.y - BALL_R < 0.0 {
                self.ball.y = BALL_R;
                self.vel.y = self.vel.y.abs() * 0.7;
            }
            // net (a thin box in the middle)
            let (nl, nr, nt) = (cx - 19.0, cx + 19.0, Self::net_top(c));
            let closest = v2(self.ball.x.clamp(nl, nr), self.ball.y.clamp(nt, c.h));
            let d = self.ball - closest;
            if d.len() < BALL_R {
                let n = if d.len() > 0.01 { d.norm() } else { v2(if self.ball.x < cx { -1.0 } else { 1.0 }, 0.0) };
                self.ball = closest + n * BALL_R;
                let vn = self.vel.dot(n);
                if vn < 0.0 {
                    self.vel -= n * (1.7 * vn);
                }
                self.bump_sfx(c);
            }
            // frogs (kinematic, super-elastic)
            for i in 0..2 {
                let fp = v2(self.frog_x[i], Self::frog_y(c));
                let d = self.ball - fp;
                let dist = d.len();
                if dist < BALL_R + FROG_R && dist > 0.01 {
                    let n = d.norm();
                    self.ball = fp + n * (BALL_R + FROG_R);
                    let rel = self.vel - v2(self.frog_vx[i], 0.0);
                    let vn = rel.dot(n);
                    if vn < 0.0 {
                        self.vel -= n * (2.6 * vn);
                        let sp = self.vel.len();
                        if sp > 1500.0 {
                            self.vel = self.vel * (1500.0 / sp);
                        }
                        self.bump_sfx(c);
                        c.tick();
                    }
                }
            }
            // point when the ball touches the sand
            if self.ball.y + BALL_R >= Self::floor(c) {
                let landed_left = self.ball.x < cx;
                let scorer = if landed_left { 1 } else { 0 };
                self.score[scorer] += 1;
                self.flash = 1.0;
                c.sfx(Sfx::Whistle);
                c.heavy();
                c.fx.splash(v2(self.ball.x, Self::floor(c)), 12);
                if self.score[scorer] >= 3 {
                    c.winner = Some(scorer as i8);
                    c.end_now = true;
                } else {
                    let side = if landed_left { 0 } else { 1 };
                    self.serve(c, side);
                }
                break;
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        match t.phase {
            Phase::Down => {
                let side = if t.pos.x < c.center_x() { 0 } else { 1 };
                if c.cpu && side == 1 {
                    return;
                }
                if self.target[side].is_none() {
                    let off = t.pos.x - self.frog_x[side];
                    // grab near the frog, or anywhere on your half (friendlier on phones)
                    let off = if off.abs() < 1.5 * FROG_HALF { off } else { 0.0 };
                    self.target[side] = Some((t.id, t.pos.x, off));
                }
            }
            Phase::Move => {
                for tg in self.target.iter_mut() {
                    if let Some((id, x, _)) = tg {
                        if *id == t.id {
                            *x = t.pos.x;
                        }
                    }
                }
            }
            Phase::Up | Phase::Cancel => {
                for tg in self.target.iter_mut() {
                    if tg.map(|x| x.0) == Some(t.id) {
                        *tg = None;
                    }
                }
            }
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        art::sky(r, hex(0x6FD3F7), hex(0xD9F5FF));
        art::water(r, h * 0.55, c.anim, hex(0x48B7E0), hex(0x2B8FC9));
        let floor = Self::floor(c);
        r.grad_rect(0.0, floor - 30.0, w, h - floor + 30.0, hex(0xF6DFA4), hex(0xE5C07A));
        let cx = c.center_x();
        // net
        let nt = Self::net_top(c);
        r.rect(cx - 6.0, nt, 12.0, h - nt, hex(0xE8EDF2));
        for k in 0..6 {
            let y = nt + 10.0 + k as f32 * 22.0;
            r.rect(cx - 19.0, y, 38.0, 3.0, art::INK.a(0.6));
        }
        r.rrect(cx, nt, 54.0, 14.0, 6.0, hex(0xF0605D), art::INK, 3.0);
        // score
        art::pill(r, cx, 58.0, 200.0, 76.0, hex(0x23306E));
        r.text_bold(&format!("{} : {}", self.score[0], self.score[1]), cx, 60.0, 50.0, Align::Center, Col::WHITE, art::INK);
        r.disc(cx - 120.0, 58.0, 16.0, hex(0xF0605D));
        r.disc(cx + 120.0, 58.0, 16.0, hex(0x4FA7F2));
        // ball shadow
        let sh = ((self.ball.y) / floor).clamp(0.2, 1.0);
        r.ellipse(self.ball.x, floor - 6.0, BALL_R * sh, 12.0 * sh, art::SHADOW, Col::CLEAR, 0.0);
        for i in 0..2 {
            let skin = if i == 0 { art::RUBY } else { art::SKY };
            let fp = v2(self.frog_x[i], Self::frog_y(c));
            let near = (self.ball.x < cx) == (i == 0);
            let pose = Pose { look: (self.ball - fp).norm(), mouth: if near { 0.4 } else { 0.0 }, ..Pose::idle(c.anim, i as f32) };
            art::frog(r, fp.x, fp.y - 6.0, FROG_R * 2.1, skin, &pose);
        }
        if self.hold > 0.0 {
            r.ring(self.ball.x, self.ball.y, BALL_R + 10.0 + (self.hold * 6.0).sin() * 4.0, 4.0, Col::WHITE.a(0.8));
        }
        art::ball(r, self.ball.x, self.ball.y, BALL_R, self.rot, hex(0xFFE27A));
        if self.flash > 0.0 {
            r.push();
            r.fade(self.flash);
            r.text_bold("POINT!", cx, h * 0.4, 100.0, Align::Center, hex(0xFFD23F), art::INK);
            r.pop();
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rect(x - 2.0, y - s * 0.1, 4.0, s * 0.4, art::INK);
    art::frog(r, x - s * 0.3, y + s * 0.15, s * 0.32, art::RUBY, &Pose::idle(t, 0.0));
    art::frog(r, x + s * 0.3, y + s * 0.15, s * 0.32, art::SKY, &Pose::idle(t, 1.0));
    art::ball(r, x + (t * 1.5).sin() * s * 0.25, y - s * 0.25 + (t * 3.0).sin().abs() * -s * 0.05, s * 0.09, t * 2.0, hex(0xFFE27A));
}
