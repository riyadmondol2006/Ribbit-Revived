//! Wheelie Run: the frog rides a unicycle across floating platforms. Tap to jump, tap again
//! in the air to double jump. Collect sweets, grab clocks (+4 s), avoid bumps that stop you.
//! Falling only costs time. Past 200 m the ground turns to caramel: Candy Land.
//! Physics runs on fixed 10 ms ticks like the classic, with jumps scaled by speed.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const FW: f32 = 90.0;
const FH: f32 = 140.0;

#[derive(Clone, Copy, PartialEq)]
enum Surface {
    Grass,
    Dirt,
    Stone,
    Caramel,
}

struct Platform {
    x: f32,
    w: f32,
    top: f32,
    surface: Surface,
}

struct Thing {
    x: f32,
    h: f32,
    kind: u8, // 0 candy, 1 clock, 2 bump
    alive: bool,
}

pub struct Ride {
    u: f32,
    m: f32,
    plats: Vec<Platform>,
    things: Vec<Thing>,
    scroll: f32,
    next_x: f32,
    prev_h: f32,
    prev_size: usize,
    since_wide: f32,
    since_bump: f32,
    since_clock: f32,
    clocks: u32,
    arc_count: u32,
    speed: f32,
    old_speed: f32,
    recovering: bool,
    fy: f32,
    v: f32,
    grounded: bool,
    rising: bool,
    jumps: u32,
    running: bool,
    fell: bool,
    acc: f32,
    first_caramel: Option<f32>,
    last_plat: usize,
    land_t: f32,
    wheel: f32,
    blocked_sfx: bool,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let u = 0.003 * c.h;
    let m = 142.0 * u;
    let mut g = Ride {
        u,
        m,
        plats: Vec::new(),
        things: Vec::new(),
        scroll: 0.0,
        next_x: -400.0,
        prev_h: 50.0 * u,
        prev_size: 2,
        since_wide: 0.0,
        since_bump: 0.0,
        since_clock: 0.0,
        clocks: 0,
        arc_count: 0,
        speed: 0.05 * m,
        old_speed: 0.05 * m,
        recovering: false,
        fy: 50.0 * u,
        v: 1.0,
        grounded: true,
        rising: false,
        jumps: 0,
        running: true,
        fell: false,
        acc: 0.0,
        first_caramel: None,
        last_plat: 0,
        land_t: 9.0,
        wheel: 0.0,
        blocked_sfx: false,
    };
    // a long first platform
    g.plats.push(Platform { x: -600.0, w: 1500.0, top: 50.0 * u, surface: Surface::Grass });
    g.next_x = 900.0 + 220.0;
    g.generate(c);
    Box::new(g)
}

impl Ride {
    fn frog_sx(c: &Ctx) -> f32 {
        c.sx(0.2).max(c.safe_l + 140.0)
    }
    fn meters(&self) -> f32 {
        self.scroll / self.m
    }
    fn generate(&mut self, c: &mut Ctx) {
        let u = self.u;
        while self.next_x < self.scroll + c.w * 1.6 {
            let mut h;
            loop {
                h = 50.0 * u + c.rng.f() * 100.0 * u;
                if (h - self.prev_h).abs() <= 30.0 * u {
                    break;
                }
            }
            let mut size = c.rng.below(3);
            if size == self.prev_size {
                size = (size + 1 + c.rng.below(2)) % 3;
            }
            let w = [225.0, 390.0, 795.0][size];
            let d = self.meters() + (self.next_x - self.scroll) / self.m;
            let surface = if d > 200.0 {
                Surface::Caramel
            } else if d > 150.0 {
                Surface::Stone
            } else if d > 100.0 {
                Surface::Dirt
            } else {
                Surface::Grass
            };
            let x = self.next_x;
            if surface == Surface::Caramel && self.first_caramel.is_none() {
                self.first_caramel = Some(x);
            }
            self.plats.push(Platform { x, w, top: h, surface });
            let mut gap = self.speed * 100.0 / 7.0;
            self.since_wide += w / u;
            if self.since_wide > 3800.0 {
                self.since_wide = 0.0;
                gap *= 1.8;
            }
            // bumps on broad platforms, clocks elsewhere
            let mut has_bump = false;
            let mut has_clock = false;
            self.since_bump += (w + gap) / u;
            self.since_clock += (w + gap) / u;
            if size == 2 && self.since_bump > 1500.0 && x > 1000.0 {
                self.since_bump = 0.0;
                has_bump = true;
                self.things.push(Thing { x: x + w * 0.5, h, kind: 2, alive: true });
            }
            if !has_bump && self.since_clock >= 1800.0 + 20.0 * self.clocks as f32 {
                self.since_clock = 0.0;
                self.clocks += 1;
                has_clock = true;
                self.things.push(Thing { x: x + w * 0.5, h: h + 40.0, kind: 1, alive: true });
            }
            // sweets: over-gap arcs and on-platform clusters
            let pitch = 28.0 * u;
            self.arc_count += 1;
            if self.arc_count > 2 || (self.arc_count == 2 && c.rng.chance(0.5)) {
                self.arc_count = 0;
                let mid = x + w + gap * 0.5;
                let base = h + 0.2 * 310.0 * (c.rng.f() * 0.99 + 0.5) + pitch;
                let n = 3 + c.rng.below(4);
                for i in 0..n {
                    let f = i as f32 / (n as f32 - 1.0) - 0.5;
                    self.things.push(Thing { x: mid + f * pitch * (n as f32 - 1.0), h: base + (1.0 - 4.0 * f * f) * pitch * 1.6, kind: 0, alive: true });
                }
            }
            if !has_bump && !has_clock && c.rng.below(5) != 4 {
                let n = 3 + c.rng.below(4);
                let base = h + 0.25 * 310.0 * 0.5;
                for i in 0..n {
                    let col = (i % 3) as f32 - 1.0;
                    let row = (i / 3) as f32;
                    self.things.push(Thing { x: x + w * 0.5 + col * pitch, h: base + row * pitch, kind: 0, alive: true });
                }
            }
            self.prev_h = h;
            self.prev_size = size;
            self.next_x = x + w + gap;
        }
        let cutoff = self.scroll - c.w;
        self.plats.retain(|p| p.x + p.w > cutoff);
        self.things.retain(|t| t.x > cutoff);
    }

    fn support(&self, x0: f32, x1: f32) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in self.plats.iter().enumerate() {
            if x1 > p.x && x0 < p.x + p.w && best.is_none_or(|b| p.top > b.1) {
                best = Some((i, p.top));
            }
        }
        for t in &self.things {
            if t.kind == 2 && x1 > t.x - 47.0 && x0 < t.x + 47.0 {
                let top = t.h + 95.0;
                if best.is_none_or(|b| top > b.1) {
                    best = Some((usize::MAX, top));
                }
            }
        }
        best
    }

    fn tick(&mut self, c: &mut Ctx) {
        if !self.running {
            return;
        }
        // speed
        if self.recovering {
            self.speed += 0.027;
            if self.speed >= self.old_speed {
                self.speed = self.old_speed;
                self.recovering = false;
            }
        } else {
            self.speed += 0.00027;
        }
        let step = 0.6 * self.speed;
        // the frog's world x is the scroll position
        let fx = self.scroll;
        let front0 = fx + FW * 0.5;
        // front hit: something taller than the feet right ahead
        let mut blocked = false;
        let mut bump_hit = false;
        for p in &self.plats {
            if p.x >= front0 - 1.0 && p.x < front0 + step && p.top > self.fy + 0.05 * FH {
                blocked = true;
            }
        }
        for t in &self.things {
            if t.kind == 2 && t.x - 47.0 >= front0 - 1.0 && t.x - 47.0 < front0 + step && t.h + 95.0 > self.fy + 0.05 * FH {
                blocked = true;
                bump_hit = true;
            }
        }
        if blocked {
            if !self.recovering {
                self.old_speed = self.speed.max(0.05 * self.m);
            }
            self.speed = 0.05 * self.m;
            self.recovering = true;
            if bump_hit && !self.blocked_sfx {
                c.sfx(Sfx::Thud);
                c.reject();
            }
            self.blocked_sfx = true;
        } else {
            self.scroll += step;
            self.blocked_sfx = false;
            self.wheel += step / 30.0;
        }
        // vertical motion
        let x0 = self.scroll - FW * 0.45;
        let x1 = self.scroll + FW * 0.45;
        if self.rising {
            self.fy += step * self.v;
            self.v -= 0.042;
            if self.v <= 0.0 {
                self.rising = false;
                self.v = 0.0;
            }
        } else if self.grounded {
            match self.support(x0, x1) {
                Some((_, top)) if (top - self.fy).abs() < 4.0 => self.fy = top,
                _ => {
                    self.grounded = false;
                    self.v = 1.0;
                }
            }
        } else {
            let prev = self.fy;
            self.fy -= step * self.v;
            self.v += 0.042;
            if let Some((idx, top)) = self.support(x0, x1) {
                if prev >= top - 2.0 && self.fy <= top {
                    self.fy = top;
                    self.grounded = true;
                    if self.jumps > 0 {
                        c.sfx_ex(Sfx::Thud, 0.4, 1.4);
                        self.land_t = 0.0;
                    }
                    self.jumps = 0;
                    self.v = 1.0;
                    if idx != usize::MAX {
                        self.last_plat = idx;
                        let p = &self.plats[idx];
                        if !self.fell && self.first_caramel.is_some_and(|fc| p.x + p.w > fc) {
                            c.special_hit = true;
                        }
                    }
                }
            }
        }
        if self.fy < -FH {
            // fell: put the frog back on the last platform it stood on; tap to ride again
            self.fell = true;
            self.running = false;
            self.speed = 0.05 * self.m;
            self.recovering = false;
            c.sfx(Sfx::Splash);
            c.reject();
            if let Some(p) = self.plats.get(self.last_plat).or(self.plats.first()) {
                self.scroll = (p.x + p.w - FW).max(p.x + FW * 0.6);
                self.fy = p.top;
            }
            self.grounded = true;
            self.rising = false;
            self.jumps = 0;
            self.v = 1.0;
        }
        // pickups
        let fr_lo = self.fy;
        let fr_hi = self.fy + FH;
        for t in self.things.iter_mut() {
            if !t.alive || t.kind == 2 {
                continue;
            }
            let (hw, hh) = if t.kind == 0 { (30.0, 30.0) } else { (40.0, 40.0) };
            if (t.x - self.scroll).abs() < hw + FW * 0.5 && t.h + hh > fr_lo && t.h - hh < fr_hi {
                t.alive = false;
                let sp = v2(Self::frog_sx(c) + (t.x - self.scroll), c.h - t.h);
                if t.kind == 0 {
                    c.add_score(1, sp - v2(0.0, 30.0));
                    c.sfx_var(Sfx::Eat);
                    c.fx.glow(sp, hex(0xFFD6F0), 40.0);
                } else {
                    c.add_time(4.0, sp);
                }
            }
        }
    }
}

impl MiniGame for Ride {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.acc += dt;
        self.land_t += dt;
        while self.acc >= 0.01 {
            self.acc -= 0.01;
            self.tick(c);
        }
        self.generate(c);
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down {
            return;
        }
        if !self.running {
            self.running = true;
            self.fell_resume(c);
            return;
        }
        if self.jumps < 2 {
            self.v = if self.jumps == 0 { 1.2 } else { 1.0 };
            self.rising = true;
            self.grounded = false;
            c.sfx_ex(if self.jumps == 0 { Sfx::Jump } else { Sfx::DoubleJump }, 0.8, 1.0);
            c.tick();
            self.jumps += 1;
        }
    }

    fn score_text(&self, c: &Ctx) -> String {
        c.score.to_string()
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        let d = self.meters();
        let (top, bottom) = if d > 200.0 {
            (hex(0xFFC1E3), hex(0xFFF0D6))
        } else if d > 150.0 {
            (hex(0x8FB8E8), hex(0xDDE9F7))
        } else {
            (hex(0x86D6FF), hex(0xE3F8FF))
        };
        r.grad_rect(0.0, 0.0, w, h, top, bottom);
        for i in 0..4 {
            let x = (i as f32 * 450.0 - self.scroll * 0.75 * 0.3).rem_euclid(w + 300.0) - 150.0;
            art::cloud(r, x, 110.0 + (i % 2) as f32 * 60.0, 0.9, Col::WHITE.a(0.85));
        }
        let mcol = if d > 200.0 { hex(0xF7A8D0) } else if d > 150.0 { hex(0x9FAEC4) } else { hex(0xA6DE7E) };
        art::hills(r, h * 0.62, mcol, 80.0, -self.scroll * 0.5 * 0.3);
        let fx = Self::frog_sx(c);
        let to_sx = |x: f32| fx + (x - self.scroll);
        for p in &self.plats {
            let x = to_sx(p.x);
            if x > w + 50.0 || x + p.w < -50.0 {
                continue;
            }
            let ytop = h - p.top;
            let (grass, body) = match p.surface {
                Surface::Grass => (hex(0x7BC95A), hex(0xA0703F)),
                Surface::Dirt => (hex(0xC9935A), hex(0x8A5A34)),
                Surface::Stone => (hex(0xB8C2D0), hex(0x7D8B99)),
                Surface::Caramel => (hex(0xFF9FD0), hex(0xD9965B)),
            };
            r.rrect(x + p.w * 0.5, ytop + 200.0, p.w, 400.0, 20.0, body, art::INK, 4.0);
            r.rrect(x + p.w * 0.5, ytop + 10.0, p.w + 8.0, 28.0, 14.0, grass, art::INK, 4.0);
            if p.surface == Surface::Caramel {
                for k in 0..((p.w / 60.0) as i32) {
                    r.ellipse(x + 30.0 + k as f32 * 60.0, ytop + 30.0, 14.0, 18.0, hex(0xFF9FD0), Col::CLEAR, 0.0);
                }
            }
        }
        for t in &self.things {
            if !t.alive {
                continue;
            }
            let x = to_sx(t.x);
            if !(-80.0..=w + 80.0).contains(&x) {
                continue;
            }
            let y = h - t.h;
            match t.kind {
                0 => art::candy(r, x, y, 44.0, 2, 0.0),
                1 => art::clock_icon(r, x, y - 10.0 + (c.anim * 4.0).sin() * 4.0, 26.0, 1.0),
                _ => {
                    r.ellipse(x, y - 40.0, 47.0, 48.0, hex(0x8A5A34), art::INK, 4.0);
                    r.ellipse(x - 12.0, y - 58.0, 12.0, 8.0, Col::WHITE.a(0.3), Col::CLEAR, 0.0);
                }
            }
        }
        // frog on a unicycle
        let fy = h - self.fy;
        r.circle(fx, fy - 26.0, 26.0, hex(0x3A3F4E), art::INK, 4.0);
        r.push();
        r.translate(fx, fy - 26.0);
        r.rotate(self.wheel);
        r.line(v2(-20.0, 0.0), v2(20.0, 0.0), 4.0, hex(0xC9D2DC));
        r.line(v2(0.0, -20.0), v2(0.0, 20.0), 4.0, hex(0xC9D2DC));
        r.pop();
        r.line(v2(fx, fy - 26.0), v2(fx, fy - 64.0), 6.0, art::INK);
        let lean = if self.rising { -0.1 } else if !self.grounded { 0.08 } else { 0.0 };
        r.push();
        r.translate(fx, fy - 112.0);
        r.rotate(lean);
        let pose = Pose { look: v2(0.8, 0.0), mouth: if !self.grounded { 0.5 } else { 0.0 }, squash: if self.land_t < 0.2 { wobble(self.land_t, 30.0) * 0.15 } else { 0.0 }, ..Pose::idle(c.anim, 0.0) };
        art::frog(r, 0.0, 0.0, 110.0, art::EMERALD, &pose);
        r.pop();
        if !self.running {
            r.text_bold("Tap to ride again!", c.center_x(), h * 0.3, 44.0, crate::render::Align::Center, Col::WHITE, art::INK);
        }
        if d > 195.0 && d < 215.0 {
            r.text_bold("CANDY LAND!", c.center_x(), h * 0.22, 60.0, crate::render::Align::Center, hex(0xFF5DA2), art::INK);
        }
    }
}

impl Ride {
    fn fell_resume(&mut self, c: &mut Ctx) {
        c.sfx_ex(Sfx::Jump, 0.6, 1.0);
        self.v = 1.2;
        self.rising = true;
        self.grounded = false;
        self.jumps = 1;
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rrect(x - s * 0.25, y + s * 0.32, s * 0.5, s * 0.2, 8.0, hex(0xA0703F), art::INK, 2.0);
    r.rrect(x + s * 0.4, y + s * 0.26, s * 0.3, s * 0.2, 8.0, hex(0xA0703F), art::INK, 2.0);
    r.circle(x - s * 0.15, y + s * 0.15, s * 0.07, hex(0x3A3F4E), art::INK, 2.0);
    let b = (t * 2.0).sin().abs() * s * 0.08;
    art::frog(r, x - s * 0.15, y - s * 0.08 - b, s * 0.3, art::EMERALD, &Pose::idle(t, 0.0));
    art::candy(r, x + s * 0.2, y - s * 0.25, s * 0.14, 2, 0.0);
}
