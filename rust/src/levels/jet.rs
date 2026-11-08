//! Jet Frog: hold anywhere to fire the jetpack, release to fall. Collect fuel cells, grab
//! speed boosts and clocks, dodge space rocks. World units follow the classic 480x330 field;
//! item patterns are generated procedurally with the same density ramp.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const FIELD_W: f32 = 480.0;
const FIELD_H: f32 = 330.0;
const PATTERN_W: f32 = 875.0;
const FROG_W: f32 = 70.0; // in field units
const FROG_H: f32 = 84.0;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Fuel,
    Boost,
    Rock(f32, f32),
    Clock,
    Station,
}

struct Item {
    x: f32, // field units, world space
    h: f32, // field units above the floor
    kind: Kind,
    alive: bool,
    hide_t: f32,
    seed: u32,
}

struct Debris {
    x: f32,
    h: f32,
    kind: Kind,
    seed: u32,
    rot: f32,
}

pub struct Jet {
    items: Vec<Item>,
    debris: Vec<Debris>,
    world: f32,
    next_pattern: usize,
    v: f32,
    h: f32,
    vy: f32,
    thrust: bool,
    fingers: Vec<i32>,
    boost_t: f32,
    station_done: bool,
    stars: Vec<(f32, f32, f32)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut stars = Vec::new();
    for _ in 0..90 {
        stars.push((c.rng.f(), c.rng.f() * 0.8, c.rng.f() * 6.0));
    }
    let mut g = Jet {
        items: Vec::new(),
        debris: Vec::new(),
        world: 0.0,
        next_pattern: 0,
        v: 300.0,
        h: FIELD_H * 0.5,
        vy: 0.0,
        thrust: false,
        fingers: Vec::new(),
        boost_t: 0.0,
        station_done: false,
        stars,
    };
    g.ensure_patterns(c);
    Box::new(g)
}

impl Jet {
    fn sx(_c: &Ctx) -> f32 {
        super::STAGE_W / FIELD_W
    }
    fn sy(c: &Ctx) -> f32 {
        c.h / FIELD_H
    }
    /// The classic frog sits 140 vertical units from the left edge (~115 field units).
    fn frog_x() -> f32 {
        115.0
    }
    /// Screen position of a field point (world x relative to the camera).
    fn to_screen(&self, c: &Ctx, x: f32, h: f32) -> V2 {
        v2(c.x0() + (x - self.world) * Self::sx(c), c.h - h * Self::sy(c))
    }

    fn generate(&mut self, c: &mut Ctx, p: usize) {
        let base = 100.0 + p as f32 * PATTERN_W;
        let cell = |col: f32, row: f32| (base + col * 25.0, 25.0 * (13.0 - row) - 10.0 + 18.0);
        let rng = &mut c.rng;
        let density = (p as f32 / 30.0).min(1.0);
        // fuel formations
        let forms = 2 + (density * 3.0) as usize + rng.below(2);
        let mut used: Vec<(i32, i32)> = Vec::new();
        for f in 0..forms {
            let col0 = (f as f32 * 35.0 / forms as f32) as i32 + rng.irange(0, 3);
            let len = rng.irange(4, 8);
            let shape = rng.below(3);
            let row0 = rng.irange(4, 10) as f32;
            for i in 0..len {
                let col = col0 + i;
                if col >= 35 {
                    break;
                }
                let row = match shape {
                    0 => row0,
                    1 => row0 + ((i as f32) * 0.8).sin() * 2.5,
                    _ => row0 - i as f32 * 0.6,
                }
                .clamp(3.0, 11.0);
                let (x, h) = cell(col as f32, row);
                used.push((col, row as i32));
                self.items.push(Item { x, h, kind: Kind::Fuel, alive: true, hide_t: 0.0, seed: 0 });
            }
        }
        // rocks (none in the very first pattern)
        let rocks = if p == 0 { 0 } else { (1.0 + density * 6.0) as usize + rng.below(2) };
        for _ in 0..rocks {
            for _try in 0..8 {
                let col = rng.irange(3, 33);
                let row = rng.irange(3, 11);
                if used.iter().any(|&(uc, ur)| (uc - col).abs() <= 2 && (ur - row).abs() <= 2) {
                    continue;
                }
                let (x, h) = cell(col as f32, row as f32);
                let (rw, rh) = *rng.pick(&[(36.0, 36.0), (75.0, 36.0), (42.0, 75.0), (85.0, 42.0), (42.0, 85.0)]);
                used.push((col, row));
                self.items.push(Item { x, h, kind: Kind::Rock(rw, rh), alive: true, hide_t: 0.0, seed: rng.next_u64() as u32 });
                break;
            }
        }
        // speed boost every other pattern, clocks about 2 of every 3 patterns
        let place = |kind: Kind, rng: &mut Rng, used: &mut Vec<(i32, i32)>, items: &mut Vec<Item>| {
            for _try in 0..10 {
                let col = rng.irange(5, 30);
                let row = rng.irange(4, 10);
                if used.iter().any(|&(uc, ur)| (uc - col).abs() <= 1 && (ur - row).abs() <= 1) {
                    continue;
                }
                let (x, h) = cell(col as f32, row as f32);
                used.push((col, row));
                items.push(Item { x, h, kind, alive: true, hide_t: 0.0, seed: 0 });
                return;
            }
        };
        if p % 2 == 1 {
            place(Kind::Boost, rng, &mut used, &mut self.items);
        }
        if p % 3 != 2 || p < 2 {
            place(Kind::Clock, rng, &mut used, &mut self.items);
        }
        if p == 23 {
            let (x, h) = cell(15.0, 4.0);
            self.items.push(Item { x, h, kind: Kind::Station, alive: true, hide_t: 0.0, seed: 0 });
        }
    }

    fn ensure_patterns(&mut self, c: &mut Ctx) {
        let view_end = self.world + (c.w / Self::sx(c)) + 200.0;
        while 100.0 + self.next_pattern as f32 * PATTERN_W < view_end + PATTERN_W {
            let p = self.next_pattern;
            self.generate(c, p);
            self.next_pattern += 1;
        }
    }

    fn rect(kind: Kind) -> (f32, f32) {
        match kind {
            Kind::Fuel => (24.0, 23.0),
            Kind::Boost | Kind::Clock => (30.0, 30.0),
            Kind::Rock(w, h) => (w, h),
            Kind::Station => (66.0, 62.0),
        }
    }
}

impl MiniGame for Jet {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        // world speed eases back to the 300 u/s cruise
        if self.v < 300.0 {
            self.v = (self.v + 100.0 * dt).min(300.0);
        } else if self.v > 300.0 {
            self.v = (self.v - 300.0 * dt).max(300.0);
        }
        self.world += 0.7 * self.v * dt;
        self.boost_t = (self.boost_t - dt).max(0.0);
        // vertical motion
        let a = if self.thrust { 800.0 } else { -450.0 };
        self.vy += a * dt;
        self.h += self.vy * dt;
        if self.h < FROG_H * 2.0 / 3.0 {
            self.h = FROG_H * 2.0 / 3.0;
            self.vy = 200.0;
        }
        // keep the frog below the HUD band
        if self.h > FIELD_H - 55.0 - FROG_H * 0.5 {
            self.h = FIELD_H - 55.0 - FROG_H * 0.5;
            self.vy = 0.0;
        }
        self.ensure_patterns(c);
        // collisions (frog fixed at field x = 140 relative to the camera)
        let fx = self.world + Self::frog_x();
        let fh = self.h;
        let station_line = self.world + FIELD_W * 0.3;
        for it in self.items.iter_mut() {
            if !it.alive {
                it.hide_t += dt;
                continue;
            }
            if it.kind == Kind::Station && !self.station_done && it.x <= station_line {
                self.station_done = true;
                if c.score < 100 {
                    c.special_hit = true;
                }
            }
            let (w, h) = Self::rect(it.kind);
            if (it.x - fx).abs() < (w + FROG_W) * 0.5 && (it.h - fh).abs() < (h + FROG_H) * 0.5 {
                let sp = v2(c.x0() + (it.x - self.world) * Self::sx(c), c.h - it.h * Self::sy(c));
                match it.kind {
                    Kind::Fuel => {
                        it.alive = false;
                        c.add_score(1, sp - v2(0.0, 30.0));
                        { let pitch = c.rng.range(0.95, 1.1); c.sfx_ex(Sfx::Coin, 0.6, pitch); }
                        c.fx.glow(sp, hex(0x7CF3FF), 50.0);
                    }
                    Kind::Boost => {
                        it.alive = false;
                        self.v = 590.0;
                        self.boost_t = 1.5;
                        c.sfx(Sfx::Whoosh);
                        c.confirm();
                        c.fx.stars(sp, 8);
                    }
                    Kind::Clock => {
                        it.alive = false;
                        c.add_time(6.0, sp);
                    }
                    Kind::Rock(..) | Kind::Station => {
                        it.alive = false;
                        self.v = 100.0;
                        c.sfx(Sfx::Thud);
                        c.heavy();
                        c.shake = 0.25;
                        self.debris.push(Debris { x: it.x, h: it.h, kind: it.kind, seed: it.seed, rot: 0.0 });
                    }
                }
            }
        }
        let cam = self.world;
        self.items.retain(|it| it.x > cam - 200.0 && (it.alive || it.hide_t < 0.15));
        for d in self.debris.iter_mut() {
            d.x += 300.0 * dt;
            d.rot += dt * 5.0;
        }
        let limit = cam + FIELD_W * 3.0;
        self.debris.retain(|d| d.x < limit);
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        match t.phase {
            Phase::Down => {
                if self.fingers.is_empty() {
                    self.vy = 0.0;
                    c.sfx_ex(Sfx::Whoosh, 0.5, 1.4);
                }
                self.fingers.push(t.id);
                self.thrust = true;
            }
            Phase::Up | Phase::Cancel => {
                self.fingers.retain(|&f| f != t.id);
                self.thrust = !self.fingers.is_empty();
            }
            Phase::Move => {}
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0x0B1033), hex(0x2B2370));
        // stars (parallax)
        for (i, &(sx, sy, ph)) in self.stars.iter().enumerate() {
            let x = (sx * w * 1.2 - self.world * 0.06 * (1.0 + (i % 3) as f32 * 0.5)).rem_euclid(w * 1.2) - w * 0.1;
            let tw = 0.5 + 0.5 * (c.anim * 2.0 + ph).sin();
            r.disc(x, sy * h, 1.5 + (i % 3) as f32, Col::WHITE.a(0.4 + 0.6 * tw));
        }
        // planet
        let px = w * 0.8 - self.world * 0.04;
        r.circle(px, h * 0.3, 90.0, hex(0xFF9E6B), hex(0xB4513A), 4.0);
        r.ellipse_rot(px, h * 0.3, 150.0, 22.0, -0.3, Col::CLEAR, hex(0xFFD8A8).a(0.8), 6.0);
        r.circle(px - 30.0, h * 0.3 - 20.0, 18.0, hex(0xE07A4F).a(0.6), Col::CLEAR, 0.0);
        // moon ground strip
        let gy = h - 26.0;
        r.grad_rect(0.0, gy - 10.0, w, h - gy + 10.0, hex(0x7A7FA6), hex(0x4C5078));
        let off = (self.world * Self::sx(c)) % 180.0;
        let mut x = -off;
        while x < w + 180.0 {
            r.ellipse(x, gy, 40.0, 9.0, hex(0x5C6087), Col::CLEAR, 0.0);
            x += 180.0;
        }
        // items
        for it in &self.items {
            let p = self.to_screen(c, it.x, it.h);
            if p.x < -200.0 || p.x > w + 200.0 {
                continue;
            }
            let s = if it.alive { 1.0 } else { 1.0 - it.hide_t / 0.15 };
            if s <= 0.0 {
                continue;
            }
            match it.kind {
                Kind::Fuel => {
                    r.glow(p.x, p.y, 40.0 * s, hex(0x7CF3FF).a(0.5));
                    r.rrect(p.x, p.y, 30.0 * s, 42.0 * s, 8.0, hex(0x5EE6FF), hex(0x1B4F72), 3.0);
                    r.rrect(p.x, p.y - 2.0, 14.0 * s, 24.0 * s, 4.0, hex(0xC9FBFF), Col::CLEAR, 0.0);
                    r.rrect(p.x, p.y - 24.0 * s, 14.0 * s, 8.0 * s, 2.0, hex(0x1B4F72), Col::CLEAR, 0.0);
                }
                Kind::Boost => {
                    r.circle(p.x, p.y, 38.0 * s, hex(0xFFD23F), art::INK, 4.0);
                    for k in 0..2 {
                        let dx = -10.0 + k as f32 * 16.0;
                        r.line(v2(p.x + dx - 8.0, p.y - 14.0), v2(p.x + dx + 6.0, p.y), 6.0, art::INK);
                        r.line(v2(p.x + dx + 6.0, p.y), v2(p.x + dx - 8.0, p.y + 14.0), 6.0, art::INK);
                    }
                }
                Kind::Clock => {
                    r.glow(p.x, p.y, 70.0, hex(0xFFF3B0).a(0.4));
                    art::clock_icon(r, p.x, p.y, 32.0 * s, 1.0);
                }
                Kind::Rock(rw, rh) => {
                    let rad = rw.max(rh) * 0.5 * Self::sx(c) * 0.75;
                    art::rock(r, p.x, p.y, rad, it.seed, hex(0x8D8BA8));
                }
                Kind::Station => draw_station(r, p.x, p.y, 1.2),
            }
        }
        for d in &self.debris {
            let p = self.to_screen(c, d.x, d.h);
            r.push();
            r.translate(p.x, p.y);
            r.rotate(d.rot);
            r.fade(0.7);
            match d.kind {
                Kind::Rock(rw, rh) => art::rock(r, 0.0, 0.0, rw.max(rh) * 0.5 * Self::sx(c) * 0.6, d.seed, hex(0x8D8BA8)),
                _ => draw_station(r, 0.0, 0.0, 1.0),
            }
            r.pop();
        }
        // speed streaks
        if self.boost_t > 0.0 {
            for i in 0..14 {
                let y = (i as f32 * 53.0 + c.anim * 900.0) % h;
                let x = (i as f32 * 211.0 - c.anim * 2600.0).rem_euclid(w);
                r.rrect(x, y, 160.0, 4.0, 2.0, Col::WHITE.a(0.4 * self.boost_t / 1.5), Col::CLEAR, 0.0);
            }
        }
        // frog with jetpack
        let fp = self.to_screen(c, self.world + Self::frog_x(), self.h);
        let shadow_s = (0.3 + 0.7 * (1.0 - self.h / FIELD_H)).clamp(0.2, 1.0);
        r.ellipse(fp.x, gy, 50.0 * shadow_s, 8.0 * shadow_s, Col(0, 0, 0, 70), Col::CLEAR, 0.0);
        let tilt = (self.vy / 900.0).clamp(-0.3, 0.3);
        r.push();
        r.translate(fp.x, fp.y);
        r.rotate(-tilt);
        if self.thrust {
            let fl = 1.0 + (c.anim * 40.0).sin() * 0.15;
            r.glow(-38.0, 70.0, 60.0, hex(0xFFB020).a(0.8));
            r.ellipse(-38.0, 62.0 + 20.0 * fl, 12.0, 30.0 * fl, hex(0xFFD23F), hex(0xFF6B2B), 4.0);
        }
        r.rrect(-38.0, 14.0, 34.0, 70.0, 12.0, hex(0xB8C2D9), art::INK, 4.0);
        r.rrect(-38.0, 50.0, 22.0, 14.0, 4.0, hex(0x5A6178), art::INK, 3.0);
        let pose = Pose { mouth: if self.thrust { 0.5 } else { 0.0 }, look: v2(0.7, (-self.vy / 400.0).clamp(-1.0, 1.0)), ..Pose::idle(c.anim, 2.0) };
        art::frog(r, 0.0, 0.0, 120.0, art::EMERALD, &pose);
        r.circle(0.0, -10.0, 78.0, hex(0xCFF2FF).a(0.22), hex(0xE8F7FF).a(0.85), 4.0);
        r.ellipse_rot(-34.0, -50.0, 22.0, 9.0, -0.7, Col::WHITE.a(0.7), Col::CLEAR, 0.0);
        r.pop();
    }
}

fn draw_station(r: &mut Renderer, x: f32, y: f32, s: f32) {
    r.rrect(x - 70.0 * s, y, 70.0 * s, 26.0 * s, 4.0, hex(0x3D7BE0), art::INK, 3.0);
    r.rrect(x + 70.0 * s, y, 70.0 * s, 26.0 * s, 4.0, hex(0x3D7BE0), art::INK, 3.0);
    r.circle(x, y, 34.0 * s, hex(0xD9DEE8), art::INK, 4.0);
    r.circle(x, y, 14.0 * s, hex(0x7CF3FF), art::INK, 3.0);
    r.line(v2(x, y - 34.0 * s), v2(x + 10.0 * s, y - 60.0 * s), 4.0, art::INK);
    r.disc(x + 10.0 * s, y - 62.0 * s, 6.0 * s, hex(0xFF5D5D));
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rrect(x, y, s * 1.2, s * 0.95, 16.0, hex(0x2B2370), Col::CLEAR, 0.0);
    for i in 0..9 {
        r.disc(x - s * 0.5 + (i as f32 * 0.37 % 1.0) * s, y - s * 0.4 + (i as f32 * 0.61 % 1.0) * s * 0.7, 2.0, Col::WHITE);
    }
    let b = (t * 2.0).sin() * s * 0.05;
    r.glow(x - s * 0.17, y + s * 0.25 + b, s * 0.15, hex(0xFFB020).a(0.8));
    art::frog(r, x - s * 0.05, y + b, s * 0.4, art::EMERALD, &Pose::idle(t, 1.0));
    r.circle(x - s * 0.05, y - s * 0.04 + b, s * 0.26, hex(0xCFF2FF).a(0.2), Col::WHITE.a(0.8), 3.0);
    r.rrect(x + s * 0.35, y - s * 0.2, s * 0.1, s * 0.14, 4.0, hex(0x5EE6FF), hex(0x1B4F72), 2.0);
}
