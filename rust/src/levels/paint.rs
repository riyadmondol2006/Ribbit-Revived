//! Color Swap: frogs come in two colours; tap a frog to flip it. When every frog matches,
//! you score the number of frogs that already had that colour (so flip the minority!) and
//! gain +1 s. Board sizes follow the classic 34-wave table.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose, Skin};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const WAVES: [(usize, usize); 34] = [
    (2, 2), (3, 2), (4, 2), (2, 4), (8, 1), (4, 2), (2, 4), (3, 3), (8, 1), (5, 2), (7, 3), (6, 2), (7, 2), (5, 3), (2, 4), (4, 4), (8, 2),
    (6, 3), (3, 2), (4, 2), (2, 4), (8, 1), (4, 2), (5, 4), (4, 4), (7, 3), (8, 3), (6, 4), (4, 4), (7, 4), (3, 2), (4, 2), (2, 4), (8, 4),
];
const COLORS: [Skin; 4] = [art::SKY, art::EMERALD, art::TANGERINE, art::RUBY];
const PX: f32 = 124.0;
const PY: f32 = 126.0;

struct Frog {
    pos: V2,
    is_b: bool,
    flip_t: f32,
    seed: f32,
}

pub struct Paint {
    frogs: Vec<Frog>,
    a: usize,
    b: usize,
    cnt_a: i64,
    cnt_b: i64,
    wave: usize,
    appear: f32,
    splats: Vec<(V2, f32, f32, Col)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Paint { frogs: Vec::new(), a: 0, b: 1, cnt_a: 0, cnt_b: 0, wave: 0, appear: 0.0, splats: Vec::new() };
    g.new_wave(c);
    Box::new(g)
}

impl Paint {
    fn new_wave(&mut self, c: &mut Ctx) {
        let (cols, rows) = WAVES[self.wave.min(WAVES.len() - 1)];
        let n = cols * rows;
        self.a = c.rng.below(4);
        self.b = (self.a + 1 + c.rng.below(3)) % 4;
        let cx = c.center_x();
        let cy = c.h * 0.5 + 45.0;
        self.frogs = (0..n)
            .map(|k| {
                let (col, row) = (k % cols, k / cols);
                Frog {
                    pos: v2(cx + PX * (col as f32 - (cols as f32 - 1.0) * 0.5), cy + PY * (row as f32 - (rows as f32 - 1.0) * 0.5)),
                    is_b: false,
                    flip_t: 9.0,
                    seed: c.rng.f() * 9.0,
                }
            })
            .collect();
        // N/2 picks with replacement become colour B; A stays the majority
        for _ in 0..n / 2 {
            let i = c.rng.below(n);
            self.frogs[i].is_b = true;
        }
        if self.frogs.iter().all(|f| !f.is_b) {
            self.frogs[0].is_b = true;
        }
        self.cnt_b = self.frogs.iter().filter(|f| f.is_b).count() as i64;
        self.cnt_a = n as i64 - self.cnt_b;
        self.appear = 0.0;
    }
    fn skin(&self, f: &Frog) -> Skin {
        COLORS[if f.is_b { self.b } else { self.a }]
    }
}

impl MiniGame for Paint {
    fn idle(&mut self, _c: &mut Ctx, dt: f32) {
        self.appear += dt;
    }

    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.appear += dt;
        for f in self.frogs.iter_mut() {
            f.flip_t += dt;
        }
        for s in self.splats.iter_mut() {
            s.1 += dt;
        }
        self.splats.retain(|s| s.1 < 0.3);
        let all_b = self.frogs.iter().all(|f| f.is_b);
        let all_a = self.frogs.iter().all(|f| !f.is_b);
        if (all_a || all_b) && self.frogs.iter().all(|f| f.flip_t > 0.12) {
            let gain = if all_a { self.cnt_a } else { self.cnt_b };
            c.add_score(gain, v2(c.center_x(), c.h * 0.3));
            c.add_time(1.0, v2(c.center_x(), c.h * 0.42));
            c.confirm();
            self.wave += 1;
            self.new_wave(c);
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down {
            return;
        }
        let (a, b) = (self.a, self.b);
        if let Some(f) = self.frogs.iter_mut().find(|f| (t.pos.x - f.pos.x).abs() < PX * 0.5 && (t.pos.y - f.pos.y).abs() < PY * 0.5) {
            f.is_b = !f.is_b;
            f.flip_t = 0.0;
            let col = COLORS[if f.is_b { b } else { a }].body;
            let rot = c.rng.f() * 6.28;
            self.splats.push((f.pos, 0.0, rot, col));
            c.fx.burst(f.pos, col, 8, 300.0);
            c.sfx_var(Sfx::Splat);
            c.tick();
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0xFFF6E3), hex(0xF5DDB4));
        // paint pots on the sides
        for (i, x) in [c.safe_l + 90.0, c.safe_r - 90.0].iter().enumerate() {
            let col = COLORS[if i == 0 { self.a } else { self.b }].body;
            r.rrect(*x, h - 90.0, 120.0, 110.0, 16.0, hex(0xC9D2DC), art::INK, 4.0);
            r.ellipse(*x, h - 145.0, 60.0, 16.0, col, art::INK, 4.0);
            r.ellipse(*x - 10.0, h - 120.0, 14.0, 24.0, col, Col::CLEAR, 0.0);
        }
        let k = ease_out_back((self.appear / 0.3).min(1.0));
        for s in &self.splats {
            let a = 1.0 - s.1 / 0.3;
            r.push();
            r.translate(s.0.x, s.0.y);
            r.rotate(s.2);
            r.scale(1.0 + s.1 * 2.0, 1.0 + s.1 * 2.0);
            for j in 0..6 {
                let ang = j as f32 * 1.05;
                r.disc(ang.cos() * 40.0, ang.sin() * 40.0, 14.0, s.3.a(a * 0.8));
            }
            r.disc(0.0, 0.0, 42.0, s.3.a(a * 0.6));
            r.pop();
        }
        for f in &self.frogs {
            let flip = if f.flip_t < 0.25 { wobble(f.flip_t, 30.0) * 0.2 } else { 0.0 };
            r.ellipse(f.pos.x, f.pos.y + 52.0 * k, 46.0 * k, 10.0 * k, art::SHADOW, art::SHADOW, 0.0);
            let mut pose = Pose::idle(c.anim, f.seed);
            pose.squash = flip;
            if f.flip_t < 0.3 {
                pose.happy_eyes = true;
            }
            art::frog(r, f.pos.x, f.pos.y, 108.0 * k, self.skin(f), &pose);
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    art::frog(r, x - s * 0.22, y - s * 0.12, s * 0.34, art::EMERALD, &Pose::idle(t, 0.0));
    art::frog(r, x + s * 0.22, y - s * 0.12, s * 0.34, art::EMERALD, &Pose::idle(t, 1.0));
    art::frog(r, x - s * 0.22, y + s * 0.24, s * 0.34, art::TANGERINE, &Pose::idle(t, 2.0));
    art::frog(r, x + s * 0.22, y + s * 0.24, s * 0.34, art::EMERALD, &Pose::idle(t, 3.0));
    r.disc(x - s * 0.1, y + s * 0.12, s * 0.06, hex(0xFF9F2E).a(0.8));
}
