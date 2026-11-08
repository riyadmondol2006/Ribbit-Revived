//! Skate Rush: the frog skates down a three-lane street. Tap up/down to change lanes, dodge
//! cones, grab speed-ups (permanent +3% and a temporary burst) and clocks (+4 s).
//! Distance in metres is the score. Segments get denser in five difficulty tiers.
use super::{Ctx, MiniGame, STAGE_W};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};
use crate::ui::{self, Icon};

const METER: f32 = STAGE_W / 4.0;
const LANE_GAP: f32 = 96.0;

#[derive(Clone, Copy, PartialEq)]
enum Obj {
    Cone(u8),
    Speed,
    Clock,
}

struct Item {
    row: f32, // object-space row (1 row = 1 metre of object space)
    lane: i32,
    kind: Obj,
    hit: f32,
}

pub struct Drive {
    items: Vec<Item>,
    dist: f32,       // metres travelled
    obj_scroll: f32, // object-space rows scrolled (2 per metre)
    base: f32,
    slow_t: f32,
    boost_left: f32,
    lane: i32,
    lane_from: i32,
    lane_t: f32,
    crashed: bool,
    next_row: f32,
    segments_done: usize,
    level: usize,
    btn_t: [f32; 2],
    decor: Vec<(f32, u8)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Drive {
        items: Vec::new(),
        dist: 0.0,
        obj_scroll: 0.0,
        base: 1.8,
        slow_t: 9.0,
        boost_left: 0.0,
        lane: 1,
        lane_from: 1,
        lane_t: 1.0,
        crashed: false,
        next_row: 4.0,
        segments_done: 0,
        level: 0,
        btn_t: [9.0; 2],
        decor: Vec::new(),
    };
    g.ensure(c);
    Box::new(g)
}

impl Drive {
    fn frog_x(c: &Ctx) -> f32 {
        c.sx(0.2).max(c.safe_l + 220.0)
    }
    fn lane_y(c: &Ctx, lane: f32) -> f32 {
        c.h * 0.56 + lane * LANE_GAP
    }
    fn gen_segment(&mut self, c: &mut Ctx) {
        let rows = if self.level >= 4 { 48 } else { 24 };
        let cones_per_seg = [7, 6, 9, 9, 24][self.level.min(4)];
        let start = self.next_row;
        let mut taken: Vec<(i32, i32)> = Vec::new();
        // speed-up and clock share a block, three rows apart
        let blk = c.rng.irange(4, rows as i32 - 6);
        let (l1, l2) = (c.rng.irange(0, 2), c.rng.irange(0, 2));
        self.items.push(Item { row: start + blk as f32, lane: l1, kind: Obj::Clock, hit: -1.0 });
        self.items.push(Item { row: start + blk as f32 + 3.0, lane: l2, kind: Obj::Speed, hit: -1.0 });
        taken.push((blk, l1));
        taken.push((blk + 3, l2));
        let mut placed = 0;
        let mut guard = 0;
        while placed < cones_per_seg && guard < 400 {
            guard += 1;
            let row = c.rng.irange(1, rows as i32 - 1);
            let lane = c.rng.irange(0, 2);
            if taken.iter().any(|&(r, l)| (r - row).abs() <= 1 && (l == lane || (r - row).abs() == 0)) {
                continue;
            }
            // never block all three lanes in neighbouring rows
            let row_count = taken.iter().filter(|&&(r, _)| (r - row).abs() <= 1).count();
            if row_count >= if self.level >= 2 { 2 } else { 1 } {
                continue;
            }
            taken.push((row, lane));
            let cone = (self.level.min(4) + 1) as u8;
            self.items.push(Item { row: start + row as f32, lane, kind: Obj::Cone(cone), hit: -1.0 });
            placed += 1;
        }
        self.next_row = start + rows as f32;
        self.segments_done += 1;
        if self.segments_done % 5 == 0 {
            self.level = (self.level + 1).min(4);
        }
    }
    fn ensure(&mut self, c: &mut Ctx) {
        while self.next_row < self.obj_scroll + 30.0 {
            self.gen_segment(c);
        }
        while self.decor.last().map(|d| d.0).unwrap_or(0.0) < self.obj_scroll + 30.0 {
            let r = self.decor.last().map(|d| d.0).unwrap_or(0.0) + 2.5;
            let k = c.rng.below(4) as u8;
            self.decor.push((r, k));
        }
    }
    fn speed(&self) -> f32 {
        let mut s = self.base;
        if self.boost_left > 0.0 {
            s += 1.2 * (self.boost_left / (5.5 * 0.8)).min(1.0);
        }
        if self.slow_t < 1.0 {
            let slow = 1.8 * 0.55;
            s = lerp(slow, s, self.slow_t);
        }
        s
    }
    fn buttons(c: &Ctx) -> (V2, V2) {
        (v2(c.safe_l + 130.0, c.h - 88.0), v2(c.safe_r - 130.0, c.h - 88.0))
    }
    fn row_x(&self, c: &Ctx, row: f32) -> f32 {
        Self::frog_x(c) + (row - self.obj_scroll) * METER
    }
}

impl MiniGame for Drive {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        for b in self.btn_t.iter_mut() {
            *b += dt;
        }
        self.slow_t += dt;
        if self.lane_t < 1.0 {
            self.lane_t = (self.lane_t + dt / 0.1).min(1.0);
        }
        let v = self.speed();
        let d = v * dt;
        self.dist += d;
        self.obj_scroll += 2.0 * d;
        if self.boost_left > 0.0 {
            self.boost_left = (self.boost_left - d).max(0.0);
        }
        c.score = self.dist as i64;
        self.ensure(c);
        let lane_now = if self.lane_t >= 0.5 { self.lane } else { self.lane_from };
        let fx = Self::frog_x(c);
        let window = 0.15 * 150.0 / METER;
        for i in 0..self.items.len() {
            if self.items[i].hit >= 0.0 {
                self.items[i].hit += dt;
                continue;
            }
            let rel = self.items[i].row - self.obj_scroll;
            // crossing test so fast scrolls can never skip the narrow hit window
            if self.items[i].lane == lane_now && rel <= window && rel >= -window - 2.0 * d {
                self.items[i].hit = 0.0;
                let p = v2(fx, Self::lane_y(c, self.items[i].lane as f32) - 40.0);
                match self.items[i].kind {
                    Obj::Cone(_) => {
                        self.crashed = true;
                        self.boost_left = 0.0;
                        self.slow_t = 0.0;
                        c.sfx(Sfx::Thud);
                        c.heavy();
                        c.shake = 0.2;
                    }
                    Obj::Speed => {
                        self.base *= 1.033;
                        self.boost_left = 5.5;
                        c.sfx(Sfx::Whoosh);
                        c.confirm();
                        c.fx.stars(p, 6);
                    }
                    Obj::Clock => c.add_time(4.0, p - v2(0.0, 40.0)),
                }
            }
        }
        let scroll = self.obj_scroll;
        self.items.retain(|it| it.row > scroll - 6.0);
        self.decor.retain(|d| d.0 > scroll - 6.0);
        if !self.crashed && c.score >= 100 {
            c.special_hit = true;
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down {
            return;
        }
        let dir = if t.pos.x < c.center_x() { -1 } else { 1 };
        self.btn_t[if dir < 0 { 0 } else { 1 }] = 0.0;
        let nl = (self.lane + dir).clamp(0, 2);
        if nl != self.lane {
            self.lane_from = self.lane;
            self.lane = nl;
            self.lane_t = 0.0;
            c.sfx_ex(Sfx::Swap, 0.6, 1.0 + dir as f32 * 0.1);
            c.tick();
        }
    }

    fn score_text(&self, c: &Ctx) -> String {
        format!("{} m", c.score)
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        art::sky(r, hex(0x8FDCFF), hex(0xE0F7FF));
        let far = self.obj_scroll * METER * 0.05;
        art::hills(r, h * 0.36, hex(0xA6DE7E), 50.0, -far);
        // city silhouettes
        for i in 0..12 {
            let x = (i as f32 * 180.0 - self.obj_scroll * METER * 0.15).rem_euclid(w + 200.0) - 100.0;
            let bh = 80.0 + ((i * 37) % 90) as f32;
            r.rrect(x, h * 0.42 - bh * 0.5, 120.0, bh, 8.0, hex(0x9DB4D9), Col::CLEAR, 0.0);
            for wy in 0..3 {
                r.rect(x - 30.0, h * 0.42 - bh + 18.0 + wy as f32 * 24.0, 16.0, 12.0, hex(0xE6F0FF));
                r.rect(x + 14.0, h * 0.42 - bh + 18.0 + wy as f32 * 24.0, 16.0, 12.0, hex(0xE6F0FF));
            }
        }
        r.rect(0.0, h * 0.42, w, h * 0.06, hex(0x8BC958));
        // road
        let top = Self::lane_y(c, -0.6);
        let bot = Self::lane_y(c, 2.6);
        r.grad_rect(0.0, top, w, bot - top, hex(0x5A6178), hex(0x454B5E));
        r.rect(0.0, top - 10.0, w, 10.0, hex(0xC9D2DC));
        r.rect(0.0, bot, w, h - bot, hex(0xC9D2DC));
        for lane in [0.5f32, 1.5] {
            let y = Self::lane_y(c, lane);
            let off = (self.obj_scroll * METER) % 120.0;
            let mut x = -off;
            while x < w {
                r.rrect(x + 30.0, y, 60.0, 8.0, 4.0, Col::WHITE.a(0.8), Col::CLEAR, 0.0);
                x += 120.0;
            }
        }
        // roadside decor
        for (row, k) in &self.decor {
            let x = self.row_x(c, *row);
            if x < -100.0 || x > w + 100.0 {
                continue;
            }
            match k {
                0 => art::tree(r, x, top - 10.0, 0.45),
                1 => {
                    r.rect(x - 4.0, top - 120.0, 8.0, 110.0, hex(0x5A6178));
                    r.circle(x, top - 124.0, 14.0, hex(0xFFE27A), art::INK, 3.0);
                }
                _ => {}
            }
        }
        // items, sorted by lane so lower lanes draw on top
        for lane in 0..3 {
            for it in self.items.iter().filter(|it| it.lane == lane) {
                let x = self.row_x(c, it.row);
                if x < -120.0 || x > w + 120.0 {
                    continue;
                }
                let y = Self::lane_y(c, lane as f32);
                match it.kind {
                    Obj::Cone(k) => {
                        if it.hit >= 0.0 {
                            let t = it.hit;
                            if t > 1.0 {
                                continue;
                            }
                            r.push();
                            r.translate(x - t * 300.0, y - 30.0 - t * 400.0 + t * t * 500.0);
                            r.rotate(-t * 25.0);
                            draw_cone(r, 0.0, 0.0, k);
                            r.pop();
                        } else {
                            draw_cone(r, x, y - 30.0, k);
                        }
                    }
                    Obj::Speed if it.hit < 0.0 => {
                        r.rrect(x, y + 4.0, 90.0, 30.0, 10.0, hex(0xFFD23F), art::INK, 3.0);
                        for k in 0..3 {
                            let dx = -26.0 + k as f32 * 22.0;
                            r.line(v2(x + dx - 6.0, y - 4.0), v2(x + dx + 6.0, y + 4.0), 4.0, art::INK);
                            r.line(v2(x + dx + 6.0, y + 4.0), v2(x + dx - 6.0, y + 12.0), 4.0, art::INK);
                        }
                    }
                    Obj::Clock if it.hit < 0.0 => {
                        art::clock_icon(r, x, y - 40.0 + (c.anim * 4.0).sin() * 5.0, 26.0, 1.0);
                    }
                    _ => {}
                }
            }
            // the frog rides in its lane
            let lane_f = lerp(self.lane_from as f32, self.lane as f32, ease_out_cubic(self.lane_t));
            if (lane_f.round() as i32) == lane {
                let fx = Self::frog_x(c);
                let fy = Self::lane_y(c, lane_f);
                let bump = (c.anim * 20.0).sin().abs() * 3.0;
                r.rrect(fx, fy + 10.0, 150.0, 16.0, 8.0, hex(0xF0605D), art::INK, 3.0);
                r.circle(fx - 46.0, fy + 26.0, 13.0, hex(0xFFE27A), art::INK, 3.0);
                r.circle(fx + 46.0, fy + 26.0, 13.0, hex(0xFFE27A), art::INK, 3.0);
                let crash = self.slow_t < 0.5;
                let pose = Pose { dizzy: crash, mouth: if self.boost_left > 0.0 { 0.6 } else { 0.0 }, look: v2(0.8, 0.0), ..Pose::idle(c.anim, 0.0) };
                art::frog(r, fx, fy - 46.0 - bump, 110.0, art::EMERALD, &pose);
                if self.boost_left > 0.0 {
                    for k in 0..4 {
                        let y = fy - 70.0 + k as f32 * 22.0;
                        r.rrect(fx - 120.0 - (c.anim * 900.0 + k as f32 * 37.0) % 60.0, y, 70.0, 5.0, 2.5, Col::WHITE.a(0.6), Col::CLEAR, 0.0);
                    }
                }
            }
        }
        r.text_bold(&format!("{:.0} km/h", self.speed() * 12.0), Self::frog_x(c), Self::lane_y(c, -0.6) - 40.0, 24.0, Align::Center, Col::WHITE, art::INK);
        // buttons
        let (ub, db) = Self::buttons(c);
        for (i, (b, icon)) in [(ub, Icon::Up), (db, Icon::Down)].iter().enumerate() {
            let d = if self.btn_t[i] < 0.12 { 6.0 } else { 0.0 };
            r.rrect(b.x, b.y + 8.0, 200.0, 124.0, 34.0, hex(0x1F7A5A), art::INK, 4.0);
            r.rrect_grad(b.x, b.y + d, 200.0, 124.0, 34.0, hex(0x7EE0A8), hex(0x3DB37A), art::INK, 4.0);
            ui::draw_icon(r, *icon, b.x, b.y + d, 60.0, Col::WHITE);
        }
    }
}

fn draw_cone(r: &mut Renderer, x: f32, y: f32, kind: u8) {
    let col = [hex(0xFF8A2B), hex(0xFF5D5D), hex(0xB66BFF), hex(0x4FA7F2), hex(0xFFD23F)][(kind as usize).saturating_sub(1) % 5];
    r.rrect(x, y + 34.0, 70.0, 12.0, 4.0, col.shade(-0.3), art::INK, 3.0);
    r.tri(v2(x - 28.0, y + 30.0), v2(x + 28.0, y + 30.0), v2(x, y - 38.0), art::INK);
    r.tri(v2(x - 23.0, y + 28.0), v2(x + 23.0, y + 28.0), v2(x, y - 32.0), col);
    r.rrect(x, y + 4.0, 34.0, 9.0, 3.0, Col::WHITE, Col::CLEAR, 0.0);
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rrect(x, y + s * 0.2, s * 1.2, s * 0.32, 8.0, hex(0x5A6178), Col::CLEAR, 0.0);
    r.push();
    r.translate(x + s * 0.32 - (t * 0.6).fract() * s * 0.4, y + s * 0.12);
    r.scale(s / 250.0, s / 250.0);
    draw_cone(r, 0.0, 0.0, 1);
    r.pop();
    r.rrect(x - s * 0.2, y + s * 0.24, s * 0.4, s * 0.05, 4.0, hex(0xF0605D), art::INK, 2.0);
    art::frog(r, x - s * 0.2, y + s * 0.06, s * 0.34, art::EMERALD, &Pose::idle(t, 0.0));
}
