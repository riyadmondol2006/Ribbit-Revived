//! Pad Catch: candies are flung off the left bank; steer the lily pad under them so they
//! bounce into the frog's mouth. Wave tuning, parabola maths and clock drops follow the
//! reverse-engineered classic (63 waves, u = stage/568 units).
use super::{Ctx, MiniGame, STAGE_W};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;
use crate::ui::{self, Icon};

/// [count, interval s, flight rate, clock bonus s, candy kind, clock at wave end]
const WAVES: [[f32; 6]; 63] = [
    [3., 2.0, 50., 5., 1., 1.], [1., 0.3, 50., 5., 1., 0.], [2., 1.0, 50., 5., 1., 0.], [3., 1.0, 50., 5., 1., 1.],
    [2., 0.7, 50., 5., 1., 0.], [2., 0.6, 50., 5., 1., 1.], [4., 1.8, 50., 5., 1., 1.], [5., 1.5, 50., 5., 1., 1.],
    [7., 1.0, 50., 5., 1., 1.], [10., 0.8, 50., 5., 1., 1.], [8., 0.7, 55., 5., 2., 1.], [3., 1.0, 55., 5., 2., 1.],
    [5., 0.5, 55., 5., 2., 1.], [8., 0.8, 55., 5., 2., 1.], [5., 0.6, 55., 5., 2., 1.], [8., 0.8, 55., 5., 2., 1.],
    [6., 0.6, 55., 5., 2., 1.], [5., 0.4, 55., 5., 2., 1.], [4., 0.6, 55., 5., 2., 0.], [5., 0.3, 55., 5., 2., 1.],
    [2., 2.0, 55., 5., 2., 1.], [8., 0.6, 55., 5., 2., 1.], [5., 0.3, 60., 5., 3., 0.], [3., 1.5, 60., 5., 3., 1.],
    [5., 1.0, 60., 5., 3., 0.], [4., 1.6, 60., 5., 3., 1.], [8., 0.5, 60., 5., 3., 1.], [10., 0.7, 60., 5., 3., 1.],
    [7., 0.4, 60., 5., 3., 1.], [6., 0.8, 60., 5., 3., 1.], [2., 0.3, 60., 5., 3., 0.], [5., 0.5, 60., 5., 3., 1.],
    [10., 0.6, 60., 5., 3., 1.], [3., 0.8, 55., 5., 3., 0.], [7., 0.4, 55., 5., 4., 1.], [8., 0.5, 55., 5., 4., 0.],
    [8., 0.7, 60., 5., 4., 1.], [4., 0.4, 60., 5., 4., 1.], [2., 0.6, 60., 5., 4., 0.], [6., 0.3, 60., 5., 4., 1.],
    [3., 0.8, 60., 5., 4., 0.], [2., 0.4, 60., 5., 4., 0.], [5., 0.7, 60., 5., 4., 1.], [2., 0.8, 60., 5., 4., 0.],
    [5., 0.3, 60., 5., 4., 1.], [5., 0.8, 60., 5., 4., 1.], [8., 0.2, 60., 4., 5., 1.], [10., 0.4, 60., 4., 5., 1.],
    [5., 0.2, 60., 4., 5., 0.], [3., 1.5, 60., 4., 5., 0.], [10., 0.2, 60., 4., 5., 1.], [5., 0.6, 65., 3., 5., 1.],
    [1., 1.0, 30., 3., 1., 0.], [1., 1.0, 40., 3., 2., 0.], [1., 1.0, 50., 3., 3., 0.], [1., 1.0, 55., 3., 4., 0.],
    [1., 1.0, 65., 5., 3., 1.], [10., 0.3, 60., 3., 5., 1.], [5., 0.5, 65., 3., 5., 1.], [15., 0.3, 65., 3., 5., 0.],
    [10., 0.5, 65., 3., 5., 1.], [5., 0.7, 50., 2., 5., 1.], [8., 0.4, 60., 1., 5., 1.],
];

const WU: f32 = 568.0;
const PAD_HALF: f32 = 50.0;
const PAD_H: f32 = 31.0;
const FROG_SIZE: f32 = 230.0;

struct Candy {
    x0: f32,
    h0: f32,
    a: f32,
    t: f32,
    stage: u8,
    live: bool,
    x: f32,
    h: f32,
    kind: u32,
    eaten: f32,
    rot: f32,
}

struct Clock {
    x: f32,
    h: f32,
    live: bool,
    caught: f32,
}

pub struct PadCatch {
    candies: Vec<Candy>,
    clocks: Vec<Clock>,
    pad_x: f32,
    wobble: f32,
    wave: usize,
    wave_count: usize,
    spawn_t: f32,
    walk: f32,
    dropped: bool,
    left_id: Option<i32>,
    right_id: Option<i32>,
    last_dir: f32,
    mouth: f32,
}

pub fn make(_c: &mut Ctx) -> Box<dyn MiniGame> {
    Box::new(PadCatch {
        candies: Vec::new(),
        clocks: Vec::new(),
        pad_x: WU * 0.5,
        wobble: 9.0,
        wave: 0,
        wave_count: 0,
        spawn_t: WAVES[0][1],
        walk: 20.0,
        dropped: false,
        left_id: None,
        right_id: None,
        last_dir: 0.0,
        mouth: 0.0,
    })
}

impl PadCatch {
    fn u() -> f32 {
        STAGE_W / WU
    }
    fn hu(c: &Ctx) -> f32 {
        c.h / Self::u()
    }
    fn sx(c: &Ctx, xu: f32) -> f32 {
        c.x0() + xu * Self::u()
    }
    fn sy(c: &Ctx, hu: f32) -> f32 {
        c.h - hu * Self::u()
    }
    fn mouth_h() -> f32 {
        159.0 + 0.32 * FROG_SIZE / Self::u()
    }
    fn mouth_x() -> f32 {
        WU - 50.0
    }
    fn buttons(c: &Ctx) -> (V2, V2) {
        (v2(c.safe_l + 130.0, c.h - 95.0), v2(c.safe_r - 130.0, c.h - 95.0))
    }
    fn add_candy(&mut self, c: &mut Ctx) {
        self.wave_count += 1;
        let w = WAVES[self.wave];
        if self.wave_count as f32 >= w[0] {
            if w[5] > 0.5 {
                let x = c.rng.range(150.0, WU - 150.0);
                self.clocks.push(Clock { x, h: Self::hu(c) + 39.0, live: true, caught: -1.0 });
            }
            self.wave = (self.wave + 1).min(WAVES.len() - 1);
            self.wave_count = 0;
        }
        let kind = WAVES[self.wave][4] as u32;
        self.candies.push(Candy { x0: -30.0, h0: 238.0, a: -7.39, t: 0.0, stage: 0, live: true, x: -30.0, h: 238.0, kind, eaten: -1.0, rot: c.rng.f() * 6.0 });
    }
}

impl MiniGame for PadCatch {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        let hu = Self::hu(c);
        let pad_plane = hu * 0.25 + PAD_H * 0.5;
        // pad steering: the most recently pressed arrow wins
        if self.left_id.is_some() || self.right_id.is_some() {
            self.pad_x = (self.pad_x + self.last_dir * 300.0 * dt).clamp(WU * 0.3, WU * 0.7);
        }
        self.wobble += dt;
        // spawning
        let w = WAVES[self.wave];
        self.spawn_t += dt;
        if self.spawn_t >= w[1] {
            self.spawn_t = 0.0;
            self.add_candy(c);
        }
        let rate = WAVES[self.wave][2];
        let mut near_mouth = false;
        for k in 0..self.candies.len() {
            let cd = &mut self.candies[k];
            if cd.eaten >= 0.0 {
                cd.eaten += dt;
                continue;
            }
            if !cd.live {
                cd.x += 70.0 / Self::u() * dt;
                cd.h -= 70.0 / Self::u() * dt;
                continue;
            }
            cd.t += dt * rate;
            cd.rot += dt * 4.0;
            cd.x = cd.x0 + 5.0 * cd.t;
            cd.h = cd.h0 + 0.3 * (cd.a * cd.t - (cd.t + cd.t * cd.t) * 0.5);
            if cd.stage == 0 && cd.x >= 30.0 {
                cd.stage = 1;
                cd.x0 = 30.0;
                cd.h0 = 188.0;
                cd.t = 0.0;
                let mut a = self.walk + if c.rng.chance(0.5) { 5.0 } else { -5.0 };
                if a > 30.0 {
                    a -= 10.0;
                } else if a < 10.0 {
                    a += 10.0;
                }
                self.walk = a;
                cd.a = a;
            } else if cd.stage == 1 && cd.h <= pad_plane {
                if (cd.x - self.pad_x).abs() <= PAD_HALF {
                    // bounce: re-aim the arc exactly at the frog's mouth
                    let tt = ((Self::mouth_x() - cd.x) / 5.0).max(1.0);
                    let mh = Self::mouth_h();
                    cd.a = ((mh - cd.h) / 0.3 + (tt + tt * tt) * 0.5) / tt;
                    cd.x0 = cd.x;
                    cd.h0 = cd.h;
                    cd.t = 0.0;
                    cd.stage = 2;
                    self.wobble = 0.0;
                    c.sfx_var(Sfx::Bounce);
                    c.tick();
                    let p = v2(Self::sx(c, cd.x), Self::sy(c, cd.h));
                    c.fx.splash(p, 6);
                } else {
                    cd.live = false;
                    self.dropped = true;
                    c.sfx(Sfx::Splash);
                    let p = v2(Self::sx(c, cd.x), Self::sy(c, hu * 0.25));
                    c.fx.splash(p, 14);
                }
            } else if cd.stage == 2 && cd.x >= Self::mouth_x() {
                cd.eaten = 0.0;
                c.add_score(1, v2(Self::sx(c, Self::mouth_x()) - 30.0, Self::sy(c, Self::mouth_h() + 50.0)));
                c.sfx_var(Sfx::Eat);
                if c.score >= 50 && !self.dropped {
                    c.special_hit = true;
                }
            }
            if cd.live && cd.stage == 2 && v2(cd.x, cd.h).dist(v2(Self::mouth_x(), Self::mouth_h())) < 85.0 {
                near_mouth = true;
            }
        }
        let target = if near_mouth { 1.0 } else { 0.0 };
        self.mouth += (target - self.mouth) * (dt * 14.0).min(1.0);
        self.candies.retain(|cd| cd.eaten < 0.12 && cd.h > -60.0 && cd.x < WU + 80.0);
        // clocks
        for k in 0..self.clocks.len() {
            let cl = &mut self.clocks[k];
            if cl.caught >= 0.0 {
                cl.caught += dt;
                continue;
            }
            if cl.live {
                cl.h -= 180.0 * dt;
                if cl.h <= PAD_H + hu * 0.25 && cl.h > hu * 0.25 - 10.0 && (cl.x - self.pad_x).abs() <= PAD_HALF {
                    cl.caught = 0.0;
                    let bonus = WAVES[self.wave][3];
                    let p = v2(Self::sx(c, cl.x), Self::sy(c, cl.h));
                    c.add_time(bonus, p - v2(0.0, 60.0));
                    self.wobble = 0.0;
                } else if cl.h <= hu * 0.25 {
                    cl.live = false;
                    c.sfx(Sfx::Splash);
                    let p = v2(Self::sx(c, cl.x), Self::sy(c, hu * 0.25));
                    c.fx.splash(p, 10);
                }
            } else {
                cl.h -= 60.0 * dt;
            }
        }
        self.clocks.retain(|cl| cl.caught < 0.15 && cl.h > -50.0);
    }

    fn touch(&mut self, _c: &mut Ctx, t: &Touch) {
        let (lb, rb) = Self::buttons(_c);
        match t.phase {
            Phase::Down => {
                let hit = |b: V2| (t.pos.x - b.x).abs() < 170.0 && (t.pos.y - b.y).abs() < 130.0;
                // generous halves of the lower screen also steer
                let lower = t.pos.y > _c.h * 0.45;
                if hit(lb) || (lower && t.pos.x < _c.center_x() - 150.0) {
                    self.left_id = Some(t.id);
                    self.last_dir = -1.0;
                } else if hit(rb) || (lower && t.pos.x > _c.center_x() + 150.0) {
                    self.right_id = Some(t.id);
                    self.last_dir = 1.0;
                }
            }
            Phase::Up | Phase::Cancel => {
                if self.left_id == Some(t.id) {
                    self.left_id = None;
                    if self.right_id.is_some() {
                        self.last_dir = 1.0;
                    }
                }
                if self.right_id == Some(t.id) {
                    self.right_id = None;
                    if self.left_id.is_some() {
                        self.last_dir = -1.0;
                    }
                }
            }
            Phase::Move => {}
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let u = Self::u();
        let hu = Self::hu(c);
        let (w, h) = (r.w, r.h);
        art::sky(r, hex(0x8FDcF9), hex(0xE6F8FF));
        art::cloud(r, w * 0.3 + (c.anim * 8.0) % 300.0, 110.0, 0.9, Col::WHITE.a(0.9));
        art::cloud(r, w * 0.75 - (c.anim * 5.0) % 200.0, 70.0, 0.7, Col::WHITE.a(0.8));
        // far island
        r.ellipse(c.center_x() + 80.0, Self::sy(c, hu * 0.25), 260.0, 70.0, hex(0x9ED96B), hex(0x5E9E3A), 3.0);
        art::tree(r, c.center_x() + 120.0, Self::sy(c, hu * 0.25) - 30.0, 0.55);
        art::water(r, Self::sy(c, hu * 0.25), c.anim, hex(0x56C2E8), hex(0x1F6FA8));
        // left bank with candy chute
        let lx = Self::sx(c, -10.0);
        let ground = Self::sy(c, 0.05 * hu);
        r.rrect_grad(lx - 40.0, (Self::sy(c, 205.0) + ground) * 0.5 + 60.0, 330.0, ground - Self::sy(c, 205.0) + 120.0, 50.0, hex(0xA8E06A), hex(0x6DA843), hex(0x2F5E1E), 5.0);
        r.rect(c.safe_l.min(lx - 300.0) - 10.0, Self::sy(c, 205.0) + 30.0, lx - 150.0 - c.safe_l.min(lx - 300.0) + 10.0, h, hex(0x8BC958));
        let chute_a = v2(Self::sx(c, -40.0), Self::sy(c, 244.0));
        let chute_b = v2(Self::sx(c, 34.0), Self::sy(c, 186.0));
        r.line_outlined(chute_a, chute_b, 26.0, hex(0xFF8FC8), art::INK, 4.0);
        // candy chute mouth
        r.circle(chute_a.x, chute_a.y, 26.0, hex(0xFF5DA2), art::INK, 4.0);
        r.circle(chute_a.x, chute_a.y, 13.0, hex(0x7A1E4E), Col::CLEAR, 0.0);
        // right bank and the hungry frog
        let rx = Self::sx(c, WU - 60.0);
        r.rrect_grad(rx + 60.0, (Self::sy(c, 160.0) + h) * 0.5 + 10.0, 320.0, h - Self::sy(c, 160.0) + 40.0, 40.0, hex(0xA8E06A), hex(0x6DA843), hex(0x2F5E1E), 5.0);
        r.rect(rx + 200.0, Self::sy(c, 160.0) + 20.0, w, h, hex(0x8BC958));
        let feet = Self::sy(c, 159.0);
        let fx = Self::sx(c, Self::mouth_x());
        let mut pose = Pose::idle(c.anim, 0.5);
        pose.mouth = self.mouth;
        pose.look = v2(-0.8, 0.2);
        art::frog(r, fx, feet - FROG_SIZE * 0.46, FROG_SIZE, art::EMERALD, &pose);
        // lily pad
        let wob = if self.wobble < 1.0 { (-self.wobble * 5.0).exp() * (self.wobble * 30.0).sin() * 8.0 } else { 0.0 };
        let px = Self::sx(c, self.pad_x);
        let py = Self::sy(c, hu * 0.25) + wob;
        art::lily_pad(r, px, py, PAD_HALF * u * 1.08, 0.4);
        // clocks
        for cl in &self.clocks {
            let s = if cl.caught >= 0.0 { 1.0 - cl.caught / 0.15 } else { 1.0 };
            let p = v2(Self::sx(c, cl.x), Self::sy(c, cl.h));
            if !cl.live && cl.caught < 0.0 {
                r.push();
                r.fade(0.5);
                art::clock_icon(r, p.x, p.y, 26.0, 1.0);
                r.pop();
            } else {
                r.glow(p.x, p.y, 60.0 * s, hex(0xFFF3B0).a(0.6));
                art::clock_icon(r, p.x, p.y, 30.0 * s, 1.0);
            }
        }
        // candies
        for cd in &self.candies {
            let p = v2(Self::sx(c, cd.x), Self::sy(c, cd.h));
            let s = if cd.eaten >= 0.0 { 1.0 - cd.eaten / 0.12 } else { 1.0 };
            r.push();
            if !cd.live {
                r.fade(0.45);
            }
            art::candy(r, p.x, p.y, 54.0 * s, cd.kind, cd.rot);
            r.pop();
        }
        // arrow buttons
        let (lb, rb) = Self::buttons(c);
        for (b, held, icon) in [(lb, self.left_id.is_some(), Icon::Left), (rb, self.right_id.is_some(), Icon::Right)] {
            let d = if held { 6.0 } else { 0.0 };
            r.rrect(b.x, b.y + 8.0, 200.0, 130.0, 34.0, hex(0xC46A12), art::INK, 4.0);
            r.rrect_grad(b.x, b.y + d, 200.0, 130.0, 34.0, hex(0xFFC15A), hex(0xFF9420), art::INK, 4.0);
            ui::draw_icon(r, icon, b.x, b.y + d, 70.0, Col::WHITE);
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.ellipse(x, y + s * 0.32, s * 0.5, s * 0.12, hex(0x56C2E8), hex(0x1F6FA8), 3.0);
    art::lily_pad(r, x - s * 0.05, y + s * 0.3, s * 0.24, 0.4);
    let k = (t * 1.5).fract();
    art::candy(r, x - s * 0.3 + k * s * 0.3, y - s * 0.25 + (k - 0.5).powi(2) * s * 0.6, s * 0.2, 0, t * 3.0);
    art::frog(r, x + s * 0.3, y, s * 0.38, art::EMERALD, &Pose { mouth: 0.8, ..Pose::idle(t, 0.0) });
}
