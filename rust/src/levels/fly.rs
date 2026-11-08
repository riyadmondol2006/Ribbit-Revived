//! Bubble Pilot: tap the left/right buttons to puff upward and sideways, following a zig-zag
//! trail of bubbles. Every 30th link is a clock (+4 s). Thrust decays, so keep tapping.
//! Climb past 7000 m of altitude to reach the moon.
use super::{Ctx, MiniGame, STAGE_W};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};
use crate::ui::{self, Icon};

const U: f32 = 2.4; // one classic unit in virtual pixels

struct Link {
    pos: V2, // x screen, y screen
    clock: bool,
    taken: f32,
}

pub struct Fly {
    v: f32,
    alt: f32,
    fx: f32,
    move_from: f32,
    move_to: f32,
    move_t: f32,
    links: Vec<Link>,
    path_x: f32,
    dir: f32,
    seg_left: i32,
    count: usize,
    puff: f32,
    puff_dir: f32,
    btn_t: [f32; 2],
    reached_moon: bool,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let cx = c.center_x();
    let mut g = Fly {
        v: 0.0,
        alt: 0.0,
        fx: cx,
        move_from: cx,
        move_to: cx,
        move_t: 1.0,
        links: Vec::new(),
        path_x: c.sx(0.5),
        dir: 1.0,
        seg_left: 3,
        count: 0,
        puff: 0.0,
        puff_dir: 1.0,
        btn_t: [9.0; 2],
        reached_moon: false,
    };
    let frog_y = c.h * 0.65;
    for i in 0..30 {
        let y = frog_y - 50.0 * U * (i as f32 + 1.0);
        g.push_link(c, y);
    }
    Box::new(g)
}

impl Fly {
    fn push_link(&mut self, c: &mut Ctx, y: f32) {
        let step = (0.06 * STAGE_W).floor();
        self.path_x += self.dir * step;
        let lo = c.x0() + 3.0 * step;
        let hi = c.x0() + STAGE_W - 3.0 * step;
        self.path_x = self.path_x.clamp(lo, hi);
        self.seg_left -= 1;
        if self.seg_left <= 0 {
            self.dir = -self.dir;
            self.seg_left = 2 + c.rng.below(6) as i32;
        }
        self.count += 1;
        let clock = self.count % 30 == 0;
        self.links.push(Link { pos: v2(self.path_x, y), clock, taken: -1.0 });
    }
    fn buttons(c: &Ctx) -> (V2, V2) {
        (v2(c.safe_l + 130.0, c.h - 90.0), v2(c.safe_r - 130.0, c.h - 90.0))
    }
    fn press(&mut self, c: &mut Ctx, side: f32) {
        let mut dir = side;
        let rel = (self.fx - c.x0()) / STAGE_W;
        if rel >= 0.87 {
            dir = -1.0;
        } else if rel <= 0.13 {
            dir = 1.0;
        }
        self.v = (self.v + 100.0).min(350.0);
        self.move_from = self.fx;
        self.move_to = (self.fx + dir * 133.0).clamp(c.safe_l + 80.0, c.safe_r - 80.0);
        self.move_t = 0.0;
        self.puff = 1.0;
        self.puff_dir = dir;
        c.sfx_ex(Sfx::Whoosh, 0.7, 1.3);
        c.tick();
    }
}

impl MiniGame for Fly {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.v = (self.v - 200.0 * dt).clamp(0.0, 350.0);
        self.alt += self.v / 3.2 * dt;
        self.puff = (self.puff - dt * 2.5).max(0.0);
        for b in self.btn_t.iter_mut() {
            *b += dt;
        }
        if self.move_t < 1.0 {
            self.move_t = (self.move_t + dt / 0.5).min(1.0);
            self.fx = lerp(self.move_from, self.move_to, ease_out_cubic(self.move_t));
        }
        let scroll = 0.9 * self.v * U * dt;
        let frog = v2(self.fx, c.h * 0.65);
        let half = 40.0 * U;
        let mut top = f32::MAX;
        for l in self.links.iter_mut() {
            l.pos.y += scroll;
            top = top.min(l.pos.y);
            if l.taken >= 0.0 {
                l.taken += dt;
            } else if self.v > 0.0 && (l.pos.x - frog.x).abs() < half && (l.pos.y - frog.y).abs() < half {
                l.taken = 0.0;
                if l.clock {
                    c.add_time(4.0, l.pos);
                } else {
                    c.add_score(1, l.pos - v2(0.0, 40.0));
                    let pitch = c.rng.range(1.0, 1.3);
                    c.sfx_ex(Sfx::PopHigh, 0.7, pitch);
                }
            }
        }
        // recycle links that fell below the screen to the top of the trail
        let mut recycled = 0;
        let h = c.h;
        self.links.retain(|l| {
            let keep = l.pos.y < 1.1 * h;
            if !keep {
                recycled += 1;
            }
            keep
        });
        for _ in 0..recycled {
            let y = top - 50.0 * U;
            top = y;
            self.push_link(c, y);
        }
        if !self.reached_moon && self.alt >= 7000.0 {
            self.reached_moon = true;
            if c.score <= 100 {
                c.special_hit = true;
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down {
            return;
        }
        let (lb, rb) = Self::buttons(c);
        let near = |b: V2| (t.pos.x - b.x).abs() < 170.0 && (t.pos.y - b.y).abs() < 130.0;
        if near(lb) || t.pos.x < c.center_x() - 200.0 && t.pos.y > c.h * 0.5 {
            self.btn_t[0] = 0.0;
            self.press(c, -1.0);
        } else if near(rb) || t.pos.x > c.center_x() + 200.0 && t.pos.y > c.h * 0.5 {
            self.btn_t[1] = 0.0;
            self.press(c, 1.0);
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        // sky fades into space from 4000 to 8000 units of altitude
        let space = ((self.alt - 4000.0) / 4000.0).clamp(0.0, 1.0);
        let top = hex(0x7FD3FF).mix(hex(0x0E1238), space);
        let bottom = hex(0xD4F4FF).mix(hex(0x2B2370), space);
        r.grad_rect(0.0, 0.0, w, h, top, bottom);
        if space > 0.0 {
            for i in 0..60 {
                let x = (i as f32 * 211.7) % w;
                let y = ((i as f32 * 97.3) + self.alt * 0.3) % h;
                r.disc(x, y, 1.5 + (i % 3) as f32, Col::WHITE.a(space));
            }
        }
        // ground slides away during the first 2700 units
        let g = self.alt * U * 0.5;
        if g < h {
            art::hills(r, h * 0.82 + g, hex(0x8FD06A), 60.0, 0.0);
            for i in 0..6 {
                art::tree(r, c.safe_l + 100.0 + i as f32 * 260.0, h * 0.95 + g, 0.8);
            }
        }
        // drifting scenery: clouds, then planets
        for i in 0..5 {
            let y = ((i as f32 * 260.0 + self.alt * U * 0.35) % (h + 300.0)) - 150.0;
            let x = c.safe_l + ((i * 397) % 1000) as f32 / 1000.0 * (c.safe_r - c.safe_l);
            if space < 0.5 {
                art::cloud(r, x, y, 0.9, Col::WHITE.a(0.8 * (1.0 - space * 2.0)));
            } else {
                r.circle(x, y, 26.0 + (i * 9 % 30) as f32, [hex(0xFF9E6B), hex(0x7CC8FF), hex(0xC59BFF)][i % 3].a(space), Col::CLEAR, 0.0);
            }
        }
        // the moon appears near 7000 units
        let moon_y = h * 0.2 - (7000.0 - self.alt) * U * 0.25;
        if moon_y > -200.0 {
            r.glow(c.center_x(), moon_y, 220.0, hex(0xFFF8D6).a(0.5));
            r.circle(c.center_x(), moon_y, 120.0, hex(0xF2EBD0), hex(0xB8AE8A), 5.0);
            r.circle(c.center_x() - 40.0, moon_y - 20.0, 22.0, hex(0xD9D0AE), Col::CLEAR, 0.0);
            r.circle(c.center_x() + 36.0, moon_y + 34.0, 16.0, hex(0xD9D0AE), Col::CLEAR, 0.0);
        }
        // trail
        for l in &self.links {
            let s = if l.taken >= 0.0 { 1.0 + l.taken * 4.0 } else { 1.0 };
            let a = if l.taken >= 0.0 { (1.0 - l.taken / 0.15).max(0.0) } else { 1.0 };
            if a <= 0.0 {
                continue;
            }
            r.push();
            r.fade(a);
            if l.clock {
                r.glow(l.pos.x, l.pos.y, 70.0, hex(0xFFF3B0).a(0.6));
                art::clock_icon(r, l.pos.x, l.pos.y, 32.0 * s, 1.0);
            } else {
                art::bubble(r, l.pos.x, l.pos.y, 30.0 * s, c.anim + l.pos.x);
            }
            r.pop();
        }
        // flying frog with a propeller cap
        let fy = h * 0.65;
        let tilt = (self.move_to - self.fx) / 400.0;
        r.push();
        r.translate(self.fx, fy);
        r.rotate(tilt.clamp(-0.3, 0.3));
        if self.puff > 0.0 {
            r.push();
            r.fade(self.puff);
            r.circle(-self.puff_dir * 70.0, 50.0, 30.0 * (2.0 - self.puff), Col::WHITE.a(0.8), Col::CLEAR, 0.0);
            r.pop();
        }
        let pose = Pose { mouth: if self.v > 200.0 { 0.5 } else { 0.0 }, look: v2(0.0, -0.8), legs_out: 0.6, ..Pose::idle(c.anim, 0.0) };
        art::frog(r, 0.0, 0.0, 140.0, art::EMERALD, &pose);
        let spin = c.anim * (8.0 + self.v / 20.0);
        r.rrect(0.0, -78.0, 8.0, 26.0, 3.0, art::INK, art::INK, 0.0);
        r.ellipse(0.0, -92.0, 70.0 * spin.sin().abs().max(0.15), 9.0, hex(0xFF5D5D), art::INK, 3.0);
        r.ellipse(0.0, -66.0, 30.0, 14.0, hex(0xFFD23F), art::INK, 3.0);
        r.pop();
        // altitude meter
        let ax = c.safe_l + 50.0;
        r.rrect(ax, h * 0.55, 20.0, 360.0, 10.0, Col(0, 0, 0, 60), Col::WHITE.a(0.5), 2.0);
        let k = (self.alt / 7000.0).clamp(0.0, 1.0);
        r.rrect(ax, h * 0.55 + 180.0 - 180.0 * k, 16.0, 360.0 * k, 8.0, hex(0x8BE04E), Col::CLEAR, 0.0);
        r.circle(ax, h * 0.55 - 190.0, 16.0, hex(0xF2EBD0), art::INK, 3.0);
        r.text_bold(&format!("{}m", self.alt as i32), ax + 26.0, h * 0.55 + 205.0, 22.0, Align::Left, Col::WHITE, art::INK);
        // buttons
        let (lb, rb) = Self::buttons(c);
        for (i, (b, icon)) in [(lb, Icon::Left), (rb, Icon::Right)].iter().enumerate() {
            let d = if self.btn_t[i] < 0.12 { 6.0 } else { 0.0 };
            r.rrect(b.x, b.y + 8.0, 200.0, 124.0, 34.0, hex(0x2B6FB5), art::INK, 4.0);
            r.rrect_grad(b.x, b.y + d, 200.0, 124.0, 34.0, hex(0x7CC8FF), hex(0x3D8FE0), art::INK, 4.0);
            ui::draw_icon(r, *icon, b.x, b.y + d - 4.0, 52.0, Col::WHITE);
            r.disc(b.x + if i == 0 { 46.0 } else { -46.0 }, b.y + d + 26.0, 10.0, Col::WHITE.a(0.8));
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    for i in 0..4 {
        let k = (i as f32 + t * 1.5) % 4.0;
        art::bubble(r, x + ((k as i32 % 2) as f32 - 0.5) * s * 0.2, y - s * 0.4 + k * s * 0.12 - s * 0.1, s * 0.07, t);
    }
    art::frog(r, x, y + s * 0.12, s * 0.38, art::EMERALD, &Pose { look: v2(0.0, -0.8), ..Pose::idle(t, 0.0) });
    r.ellipse(x, y - s * 0.12, s * 0.2 * (t * 12.0).sin().abs().max(0.2), s * 0.03, hex(0xFF5D5D), art::INK, 2.0);
}
