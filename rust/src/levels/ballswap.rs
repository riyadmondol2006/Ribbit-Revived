//! Ball Swap: coloured frogs stand on coloured floating balls with one spare ball. Tap a frog
//! to hop it onto the free ball; solve the board by putting every frog on its own colour.
//! Board sizes follow the classic 73-step progression (1 -> 8 frogs).
use super::{Ctx, MiniGame};
use crate::art::{self, Pose, Skin};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const SKINS: [Skin; 8] = [
    art::RUBY,
    art::SKY,
    art::BLOSSOM,
    art::EMERALD,
    art::GRAPE,
    art::LEMON,
    art::TANGERINE,
    art::SLATE,
];

/// Frog count per step (derived from the classic file-order table).
const STEPS: [u8; 73] = [
    1, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 1, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 1, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 5, 5, 1, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 5, 5, 1, 2, 2, 2, 3, 3,
    3, 3, 4, 4, 4, 5, 5, 6, 7, 8, 6, 7, 8, 6, 7, 8,
];

struct Ball {
    pos: V2,
    color: Option<usize>, // None = neutral
    bob: f32,
}

struct Frog {
    color: usize,
    ball: usize,
    from: usize,
    jump: f32,
    popped: bool,
}

pub struct BallSwap {
    balls: Vec<Ball>,
    frogs: Vec<Frog>,
    step: usize,
    solved_t: f32,
    appear: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = BallSwap { balls: Vec::new(), frogs: Vec::new(), step: 0, solved_t: -1.0, appear: 0.0 };
    g.new_board(c);
    Box::new(g)
}

impl BallSwap {
    fn new_board(&mut self, c: &mut Ctx) {
        let n = STEPS[self.step % STEPS.len()] as usize;
        let total = n + 1;
        // colours for this board
        let mut cols: Vec<usize> = (0..8).collect();
        c.rng.shuffle(&mut cols);
        cols.truncate(n);
        // ball layout: up to 4 per row, rows staggered
        let per_row = match total {
            0..=4 => total,
            5..=6 => 3,
            _ => 3,
        };
        let rows = total.div_ceil(per_row);
        let dx = 220.0;
        let dy = 160.0;
        let cx = c.center_x();
        let cy = c.h * 0.56;
        let mut slots: Vec<V2> = Vec::new();
        let mut k = 0;
        for r in 0..rows {
            let cnt = (total - k).min(per_row);
            let y = cy - (rows as f32 - 1.0) * dy * 0.5 + r as f32 * dy;
            let stagger = if r % 2 == 1 { 40.0 } else { -20.0 };
            for i in 0..cnt {
                let x = cx - (cnt as f32 - 1.0) * dx * 0.5 + i as f32 * dx + stagger * (rows > 1) as i32 as f32;
                slots.push(v2(x, y));
                k += 1;
            }
        }
        c.rng.shuffle(&mut slots);
        self.balls.clear();
        for (i, p) in slots.iter().enumerate() {
            let color = if i < n { Some(cols[i]) } else { None };
            self.balls.push(Ball { pos: *p, color, bob: c.rng.f() * 6.0 });
        }
        // place frogs on a random arrangement that is not already solved
        loop {
            let mut spots: Vec<usize> = (0..total).collect();
            c.rng.shuffle(&mut spots);
            self.frogs = (0..n).map(|i| Frog { color: cols[i], ball: spots[i], from: spots[i], jump: 1.0, popped: false }).collect();
            if !self.is_solved() {
                break;
            }
        }
        self.solved_t = -1.0;
        self.appear = 0.0;
    }

    fn is_solved(&self) -> bool {
        self.frogs.iter().all(|f| self.balls[f.ball].color == Some(f.color))
    }

    fn empty_ball(&self) -> Option<usize> {
        (0..self.balls.len()).find(|&b| !self.frogs.iter().any(|f| f.ball == b))
    }

    fn frog_pos(&self, f: &Frog, t: f32) -> V2 {
        let a = self.balls[f.from].pos;
        let b = self.balls[f.ball].pos;
        let k = f.jump.min(1.0);
        let mut p = a.lerp(b, k);
        p.y -= (k * std::f32::consts::PI).sin() * 90.0;
        let bob = (t * 2.0 + self.balls[f.ball].bob).sin() * 4.0 * (k >= 1.0) as i32 as f32;
        p.y += bob - 70.0;
        p
    }
}

impl MiniGame for BallSwap {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.appear += dt;
        let mut landed = false;
        for f in self.frogs.iter_mut() {
            if f.jump < 1.0 {
                f.jump += dt / 0.12;
                if f.jump >= 1.0 {
                    f.jump = 1.0;
                    landed = true;
                }
            }
        }
        if landed {
            c.sfx_ex(Sfx::Splat, 0.5, 1.3);
            if self.solved_t < 0.0 && self.is_solved() {
                self.solved_t = 0.0;
                let n = self.frogs.len() as i64;
                c.add_score(n, v2(c.center_x(), c.h * 0.3));
                c.sfx(Sfx::Collect);
                c.confirm();
                for f in self.frogs.iter_mut() {
                    f.popped = true;
                }
                let positions: Vec<(V2, Col)> = self.frogs.iter().map(|f| (self.balls[f.ball].pos - v2(0.0, 70.0), SKINS[f.color].body)).collect();
                for (p, col) in positions {
                    c.fx.burst(p, col, 10, 360.0);
                    c.fx.ring(p, Col::WHITE, 60.0);
                }
            }
        }
        if self.solved_t >= 0.0 {
            self.solved_t += dt;
            if self.solved_t > 0.35 {
                let bonus = if self.frogs.len() >= 5 { 2.0 } else { 1.0 };
                c.add_time(bonus, v2(c.center_x(), c.h * 0.42));
                self.step += 1;
                self.new_board(c);
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.solved_t >= 0.0 {
            return;
        }
        let mut best: Option<(usize, f32)> = None;
        for (i, f) in self.frogs.iter().enumerate() {
            let p = self.frog_pos(f, c.anim);
            let d = p.dist(t.pos);
            if d < 95.0 && best.is_none_or(|b| d < b.1) {
                best = Some((i, d));
            }
        }
        if let (Some((i, _)), Some(e)) = (best, self.empty_ball()) {
            if self.frogs[i].jump < 1.0 {
                return;
            }
            let f = &mut self.frogs[i];
            f.from = f.ball;
            f.ball = e;
            f.jump = 0.0;
            c.sfx_var(Sfx::Jump);
            c.tick();
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let h = r.h;
        art::sky(r, hex(0x9FE3FF), hex(0xD8F6FF));
        art::hills(r, h * 0.26, hex(0x8FD06A), 40.0, 10.0);
        art::water(r, h * 0.3, c.anim, hex(0x4FB6E2), hex(0x1D5F99));
        let k = ease_out_back((self.appear / 0.3).min(1.0));
        for b in &self.balls {
            let bob = (c.anim * 2.0 + b.bob).sin() * 4.0;
            let p = b.pos + v2(0.0, bob);
            r.ellipse(p.x, p.y + 44.0 * k, 70.0 * k, 14.0 * k, hex(0x1D5F99).a(0.5), Col::CLEAR, 0.0);
            let (fill, ring) = match b.color {
                Some(ci) => (SKINS[ci].body, SKINS[ci].line),
                None => (hex(0xF4F7FA), hex(0x8A97A3)),
            };
            r.circle(p.x, p.y, 58.0 * k, fill, ring, 5.0);
            r.ellipse(p.x, p.y, 58.0 * k, 16.0 * k, Col::CLEAR, Col::WHITE.a(0.5), 4.0);
            r.ellipse_rot(p.x - 22.0 * k, p.y - 26.0 * k, 16.0 * k, 9.0 * k, -0.6, Col::WHITE.a(0.7), Col::CLEAR, 0.0);
            if art::color_marks() {
                if let Some(ci) = b.color {
                    art::color_mark(r, p.x + 30.0 * k, p.y + 18.0 * k, 12.0 * k, SKINS[ci].body);
                }
            }
        }
        for f in &self.frogs {
            let p = self.frog_pos(f, c.anim);
            let skin = SKINS[f.color];
            let mut pose = Pose::idle(c.anim, f.color as f32);
            if f.jump < 1.0 {
                pose.legs_out = 1.0;
                pose.mouth = 0.4;
            }
            let ok = self.balls[f.ball].color == Some(f.color) && f.jump >= 1.0;
            if ok {
                pose.happy_eyes = true;
            }
            if f.popped && self.solved_t >= 0.0 {
                let s = 1.0 + self.solved_t * 2.0;
                r.push();
                r.fade(1.0 - self.solved_t / 0.35);
                art::frog(r, p.x, p.y, 110.0 * s, skin, &pose);
                r.pop();
            } else {
                art::frog(r, p.x, p.y, 110.0 * k, skin, &pose);
            }
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.circle(x - s * 0.24, y + s * 0.2, s * 0.17, art::SKY.body, art::SKY.line, 3.0);
    r.circle(x + s * 0.24, y + s * 0.2, s * 0.17, art::RUBY.body, art::RUBY.line, 3.0);
    let k = (t * 1.2).fract();
    let jx = x - s * 0.24 + k * s * 0.48;
    let jy = y - s * 0.02 - (k * std::f32::consts::PI).sin() * s * 0.2;
    art::frog(r, jx, jy, s * 0.3, art::RUBY, &Pose::idle(t, 0.0));
}
