//! Berry Bounce: tap the frog's glass globe to kick it upward; touch fruit bubbles to collect
//! them. Custom frame-based physics (k = H/320, one step = 17 ms) mirrors the classic tuning.
use super::{Ctx, MiniGame, STAGE_W};
use crate::art::{self, Fruit, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

/// (count, kind): kind 0 = clock, 1..8 = fruit flavour.
const SEQ: [(u8, u8); 60] = [
    (3, 1), (1, 0), (3, 1), (1, 0), (3, 1), (1, 0), (3, 1), (2, 2), (1, 0), (4, 2), (1, 0), (3, 2), (1, 0), (3, 2), (3, 3), (1, 0), (4, 3), (1, 0), (1, 3), (1, 0),
    (4, 3), (3, 4), (1, 0), (3, 4), (1, 0), (2, 4), (1, 0), (4, 4), (4, 5), (1, 0), (4, 5), (1, 0), (4, 5), (6, 6), (1, 0), (3, 6), (1, 0), (3, 6), (6, 7), (1, 0),
    (6, 7), (4, 8), (1, 0), (12, 8), (1, 0), (12, 8), (1, 0), (1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (1, 6), (1, 7), (1, 8), (1, 0), (20, 8), (1, 0), (20, 8), (1, 0),
];

const R: f32 = 117.0;

struct Target {
    pos: V2,
    kind: u8,
    age: f32,
}

pub struct Bounce {
    x: f32,
    hgt: f32,
    vx: f32,
    vy: f32,
    rot: f32,
    squash: f32,
    squash_t: f32,
    target: Option<Target>,
    seq: usize,
    taken: u8,
    right_side: bool,
    pops: Vec<(V2, f32, u8)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Bounce {
        x: c.center_x(),
        hgt: 0.05 * c.h + R,
        vx: 0.0,
        vy: 0.0,
        rot: 0.0,
        squash: 0.0,
        squash_t: 9.0,
        target: None,
        seq: 0,
        taken: 0,
        right_side: false,
        pops: Vec::new(),
    };
    g.spawn(c);
    Box::new(g)
}

fn fruit_of(kind: u8) -> Fruit {
    art::FRUITS[((kind.max(1) - 1) as usize) % art::FRUITS.len()]
}

impl Bounce {
    fn k(c: &Ctx) -> f32 {
        c.h / 320.0
    }
    fn floor(c: &Ctx) -> f32 {
        0.05 * c.h
    }
    fn spawn(&mut self, c: &mut Ctx) {
        let (count, kind) = SEQ[self.seq];
        let _ = count;
        self.right_side = !self.right_side;
        let fx = if self.right_side { 0.5 + 0.4 * c.rng.f() } else { 0.1 + 0.4 * c.rng.f() };
        let y = c.h * (0.4 + 0.3 * c.rng.f());
        self.target = Some(Target { pos: v2(c.x0() + fx * STAGE_W, y), kind, age: 0.0 });
    }
    fn advance(&mut self) {
        self.taken += 1;
        if self.taken >= SEQ[self.seq].0 {
            self.taken = 0;
            self.seq = (self.seq + 1) % SEQ.len();
        }
    }
    fn ball_pos(&self, c: &Ctx) -> V2 {
        v2(self.x, c.h - self.hgt)
    }
    fn hit_wall(&mut self, c: &mut Ctx) {
        { let pitch = c.rng.range(0.8, 1.0); c.sfx_ex(Sfx::Bounce, 0.6, pitch); }
        self.squash = 0.12;
        self.squash_t = 0.0;
    }
}

impl MiniGame for Bounce {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        let k = Self::k(c);
        let floor = Self::floor(c);
        let (left, right) = (c.safe_l + R, c.safe_r - R);
        // integrate in small slices for stability on slow frames
        let mut remaining = dt;
        while remaining > 0.0 {
            let step = remaining.min(1.0 / 120.0);
            remaining -= step;
            let f = step * 1000.0 / 17.0;
            self.vy -= 1.6 * k * f;
            self.x += self.vx * f / 3.0;
            self.hgt += self.vy * f / 3.0;
            self.rot += self.vx * f / 3.0 * 0.01;
            if self.x > right || self.x < left {
                self.vx = -0.8 * self.vx;
                self.x = self.x.clamp(left, right);
                self.hit_wall(c);
            }
            self.vx -= self.vx * 0.03 * f;
            if self.hgt < R + floor {
                let impact = -self.vy;
                self.vy = -0.8 * self.vy;
                self.hgt = R + floor;
                if impact > 3.0 * k / 1.5 {
                    c.sfx_ex(Sfx::Bounce, (impact / 40.0).clamp(0.2, 0.8), 0.8);
                }
                if impact > 5.0 {
                    self.squash = ((impact - 5.0) / 100.0).min(0.08);
                    self.squash_t = 0.0;
                }
                if impact > 1.0 {
                    c.special = 0; // touching the ground breaks the no-ground streak
                }
                if self.vy.abs() < 1.0 {
                    self.vy = 0.0;
                }
            }
        }
        self.squash_t += dt;
        // pickup
        let bp = self.ball_pos(c);
        if let Some(t) = &mut self.target {
            t.age += dt;
            if t.pos.dist(bp) < R + 30.0 {
                let (pos, kind) = (t.pos, t.kind);
                self.target = None;
                self.pops.push((pos, 0.0, kind));
                if kind == 0 {
                    c.add_time(5.0, pos);
                } else {
                    c.add_score(1, pos - v2(0.0, 50.0));
                    c.sfx_var(Sfx::Eat);
                    c.special += 1;
                    c.fx.burst(pos, art::fruit_color(fruit_of(kind)), 12, 380.0);
                }
                c.fx.ring(pos, Col::WHITE, 70.0);
                self.advance();
                self.spawn(c);
            }
        }
        for p in self.pops.iter_mut() {
            p.1 += dt;
        }
        self.pops.retain(|p| p.1 < 0.3);
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down {
            return;
        }
        let bp = self.ball_pos(c);
        if t.pos.dist(bp) < 1.6 * R {
            self.vx = bp.x - t.pos.x;
            self.vy = 30.0 * Self::k(c);
            self.squash = -0.1;
            self.squash_t = 0.0;
            c.sfx_var(Sfx::Kick);
            c.tick();
            c.fx.ring(t.pos, Col::WHITE, 40.0);
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        art::sky(r, hex(0x9EE3FF), hex(0xE9FFF3));
        art::hills(r, h * 0.62, hex(0xA6DE7E), 60.0, 30.0);
        for i in 0..7 {
            let x = c.safe_l + i as f32 * (w / 6.0) + 40.0;
            art::tree(r, x, h * 0.86, 0.9 + (i % 3) as f32 * 0.15);
        }
        let floor_y = h - Self::floor(c);
        r.grad_rect(0.0, floor_y - 14.0, w, h - floor_y + 14.0, hex(0x7BC95A), hex(0x4E9A37));
        for i in 0..40 {
            let x = i as f32 * 48.0 + 10.0;
            r.tri(v2(x, floor_y - 10.0), v2(x + 8.0, floor_y - 28.0), v2(x + 16.0, floor_y - 10.0), hex(0x8FD86A));
        }
        // target
        if let Some(t) = &self.target {
            let s = (t.age / 0.3).min(1.0).max(0.01);
            let bob = (c.anim * 3.0).sin() * 6.0;
            let p = t.pos + v2(0.0, bob);
            if t.kind == 0 {
                art::clock_icon(r, p.x, p.y, 34.0 * s, 1.0);
            } else {
                art::fruit(r, fruit_of(t.kind), p.x, p.y, 58.0 * s);
            }
            art::bubble(r, p.x, p.y, 52.0 * s, c.anim);
        }
        for (pos, age, _) in &self.pops {
            r.ring(pos.x, pos.y, 52.0 + age * 160.0, 4.0 * (1.0 - age / 0.3), Col::WHITE.a(1.0 - age / 0.3));
        }
        // shadow
        let bp = self.ball_pos(c);
        let hh = (self.hgt - R - Self::floor(c)).max(0.0);
        let ss = (1.1 - hh / h).clamp(0.3, 1.1);
        r.ellipse(bp.x, floor_y + 4.0, R * 0.8 * ss, 16.0 * ss, art::SHADOW, art::SHADOW, 0.0);
        // globe with frog
        let sq = if self.squash_t < 0.3 { self.squash * (1.0 - self.squash_t / 0.3) } else { 0.0 };
        r.push();
        r.translate(bp.x, bp.y);
        r.scale(1.0 + sq, 1.0 - sq);
        r.circle(0.0, 0.0, R, hex(0xCFF2FF).a(0.35), hex(0x2B6F91), 5.0);
        let pose = Pose { look: v2((self.vx / 60.0).clamp(-1.0, 1.0), (-self.vy / 60.0).clamp(-1.0, 1.0)), mouth: if self.vy > 20.0 { 0.6 } else { 0.0 }, ..Pose::idle(c.anim, 3.0) };
        art::frog(r, 0.0, 8.0, 120.0, art::EMERALD, &pose);
        r.push();
        r.rotate(self.rot);
        r.ellipse(0.0, 0.0, R - 6.0, (R - 6.0) * 0.3, Col::CLEAR, Col::WHITE.a(0.18), 3.0);
        r.pop();
        r.ellipse_rot(-R * 0.45, -R * 0.5, R * 0.28, R * 0.13, -0.7, Col::WHITE.a(0.75), Col::CLEAR, 0.0);
        r.disc(R * 0.5, R * 0.45, 7.0, Col::WHITE.a(0.5));
        r.pop();
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    let b = ((t * 2.2).sin().abs()) * s * 0.12;
    r.circle(x - s * 0.1, y + s * 0.05 - b, s * 0.3, hex(0xCFF2FF).a(0.35), hex(0x2B6F91), 3.0);
    art::frog(r, x - s * 0.1, y + s * 0.08 - b, s * 0.32, art::EMERALD, &Pose::idle(t, 0.0));
    art::fruit(r, Fruit::Grapes, x + s * 0.33, y - s * 0.25, s * 0.2);
    art::bubble(r, x + s * 0.33, y - s * 0.25, s * 0.15, t);
}
