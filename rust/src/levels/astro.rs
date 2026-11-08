//! Space Rocks: numbered asteroids float around the astronaut frog; tap them from the smallest
//! number to the biggest. A wrong tap reshuffles the wave; finishing a wave adds +3 s.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};

/// Asteroid slots in the classic 480x320 space (y measured from the bottom).
const SLOTS: [(f32, f32); 12] = [
    (50.0, 90.0), (50.0, 190.0), (140.0, 50.0), (130.0, 130.0), (140.0, 230.0), (250.0, 40.0),
    (250.0, 255.0), (330.0, 50.0), (340.0, 130.0), (340.0, 230.0), (430.0, 90.0), (430.0, 190.0),
];
const SIZES: [usize; 9] = [3, 4, 5, 6, 7, 8, 9, 10, 11];

struct Rock {
    pos: V2,
    num: i32,
    rot: f32,
    spin: f32,
    taken: f32,
    seed: u32,
}

pub struct Astro {
    rocks: Vec<Rock>,
    wave: usize,
    plays: usize,
    next: usize,
    appear: f32,
    regen: f32,
    marks: Vec<(V2, f32, bool)>,
    stars: Vec<(f32, f32)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let stars = (0..80).map(|_| (c.rng.f(), c.rng.f())).collect();
    let mut g = Astro { rocks: Vec::new(), wave: 0, plays: 0, next: 0, appear: 0.0, regen: -1.0, marks: Vec::new(), stars };
    g.generate(c);
    Box::new(g)
}

impl Astro {
    fn generate(&mut self, c: &mut Ctx) {
        let w = self.wave.min(8);
        let n = SIZES[w];
        let mut nums = Vec::with_capacity(n);
        if w < 6 {
            nums.push(1 + c.rng.below(9) as i32);
            for i in 1..n {
                nums.push(10 * i as i32 + c.rng.below(10) as i32);
            }
        } else {
            let q = (89 / n).max(1);
            let mut v = 1 + c.rng.below(9) as i32;
            nums.push(v);
            v = 10 + c.rng.below(q) as i32;
            nums.push(v);
            for _ in 2..n {
                v += 1 + c.rng.below(q) as i32;
                nums.push(v);
            }
        }
        let mut slots: Vec<usize> = (0..12).collect();
        c.rng.shuffle(&mut slots);
        let sx = super::STAGE_W / 480.0;
        let sy = c.h / 320.0;
        self.rocks = nums
            .iter()
            .enumerate()
            .map(|(i, &num)| {
                let (px, py) = SLOTS[slots[i]];
                Rock {
                    pos: v2(c.x0() + px * sx, (c.h - py * sy).max(150.0)),
                    num,
                    rot: c.rng.f() * 6.0,
                    spin: c.rng.range(-40.0, 40.0).to_radians(),
                    taken: -1.0,
                    seed: c.rng.next_u64() as u32,
                }
            })
            .collect();
        self.next = 0;
        self.appear = -0.3;
    }
}

impl MiniGame for Astro {
    fn idle(&mut self, _c: &mut Ctx, dt: f32) {
        self.appear += dt;
    }

    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.appear += dt;
        for r in self.rocks.iter_mut() {
            r.rot += r.spin * dt;
            if r.taken >= 0.0 {
                r.taken += dt;
            }
        }
        for m in self.marks.iter_mut() {
            m.1 += dt;
        }
        self.marks.retain(|m| m.1 < 0.4);
        if self.regen >= 0.0 {
            self.regen += dt;
            if self.regen > 0.2 {
                self.regen = -1.0;
                self.generate(c);
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.appear < 0.0 || self.regen >= 0.0 {
            return;
        }
        let hit = self.rocks.iter().position(|r| r.taken < 0.0 && r.pos.dist(t.pos) < 100.0);
        let Some(i) = hit else { return };
        // the correct rock is the smallest number still in play
        let want = self.rocks.iter().enumerate().filter(|(_, r)| r.taken < 0.0).min_by_key(|(_, r)| r.num).map(|(k, _)| k);
        if Some(i) == want {
            let pos = self.rocks[i].pos;
            self.rocks[i].taken = 0.0;
            self.next += 1;
            c.add_score(1, pos - v2(0.0, 80.0));
            let pitch = 0.9 + (self.next as f32) * 0.05;
            c.sfx_ex(Sfx::Collect, 0.9, pitch);
            c.tick();
            c.fx.burst(pos, hex(0xB9A8FF), 10, 360.0);
            self.marks.push((pos, 0.0, true));
            if self.rocks.iter().all(|r| r.taken >= 0.0) {
                c.add_time(3.0, v2(c.center_x(), c.h * 0.5));
                self.plays += 1;
                if self.wave < 8 && self.plays >= 2 {
                    self.wave += 1;
                    self.plays = 0;
                }
                self.regen = 0.0;
            }
        } else {
            let pos = self.rocks[i].pos;
            c.mistake(pos);
            self.marks.push((pos, 0.0, false));
            for r in self.rocks.iter_mut() {
                if r.taken < 0.0 {
                    r.taken = 0.0;
                }
            }
            self.regen = 0.0;
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0x10173D), hex(0x30236B));
        for (i, &(x, y)) in self.stars.iter().enumerate() {
            let tw = 0.5 + 0.5 * (c.anim * 1.7 + i as f32).sin();
            r.disc(x * w, y * h, 1.5 + (i % 3) as f32, Col::WHITE.a(0.3 + 0.7 * tw));
        }
        r.glow(c.center_x(), h * 0.55, 260.0, hex(0x6B5BFF).a(0.25));
        // astronaut frog in the middle
        let bob = 0.05 * h * (1.0 + (c.anim * std::f32::consts::PI / 4.0).sin()) - 0.05 * h;
        let fp = v2(c.center_x() + 0.0, h - 130.0 * h / 320.0 + bob);
        art::frog(r, fp.x, fp.y, 130.0, art::EMERALD, &Pose { look: v2(0.0, -0.5), ..Pose::idle(c.anim, 1.0) });
        r.circle(fp.x, fp.y - 12.0, 84.0, hex(0xCFF2FF).a(0.2), hex(0xE8F7FF).a(0.85), 4.0);
        r.ellipse_rot(fp.x - 36.0, fp.y - 56.0, 22.0, 9.0, -0.7, Col::WHITE.a(0.7), Col::CLEAR, 0.0);
        let k = ease_out_back((self.appear / 0.2).clamp(0.0, 1.0));
        for rk in &self.rocks {
            let s = if rk.taken >= 0.0 { (1.0 - rk.taken / 0.2).max(0.0) } else { k };
            if s <= 0.01 {
                continue;
            }
            r.push();
            r.translate(rk.pos.x, rk.pos.y);
            r.scale(s, s);
            r.push();
            r.rotate(rk.rot);
            art::rock(r, 0.0, 0.0, 62.0, rk.seed, hex(0x9B8FB8));
            r.pop();
            r.text_bold(&rk.num.to_string(), 0.0, 2.0, 50.0, Align::Center, Col::WHITE, art::INK);
            r.pop();
        }
        for (p, t, ok) in &self.marks {
            let a = 1.0 - t / 0.4;
            if *ok {
                r.ring(p.x, p.y, 70.0 + t * 80.0, 6.0 * a, hex(0x8BE04E).a(a));
            }
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rrect(x, y, s * 1.2, s * 0.95, 16.0, hex(0x1C1A4F), Col::CLEAR, 0.0);
    for (i, (dx, dy, n)) in [(-0.32, -0.18, "3"), (0.3, -0.2, "12"), (0.05, 0.22, "27")].iter().enumerate() {
        r.push();
        r.translate(x + dx * s, y + dy * s);
        r.rotate(t * 0.3 + i as f32);
        art::rock(r, 0.0, 0.0, s * 0.15, i as u32 + 3, hex(0x9B8FB8));
        r.pop();
        r.text_bold(n, x + dx * s, y + dy * s, s * 0.14, Align::Center, Col::WHITE, art::INK);
    }
}
