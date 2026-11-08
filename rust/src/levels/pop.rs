//! Pond Pop: clear boards of frogs by tapping them. Frogs in a light helmet need two taps,
//! a heavy helmet three; only the final pop scores. Each cleared board adds +1 s.
//! Wave sizes and helmet counts follow the classic 24-wave table.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose, Skin};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

/// (cols, rows, light helmets, heavy helmets)
const WAVES: [(usize, usize, usize, usize); 24] = [
    (2, 2, 0, 0), (3, 2, 0, 0), (4, 2, 0, 0), (3, 3, 0, 0), (5, 2, 1, 0), (4, 3, 2, 0), (7, 2, 3, 0), (5, 3, 3, 0),
    (8, 1, 0, 2), (2, 2, 0, 2), (3, 2, 0, 3), (4, 2, 0, 3), (3, 3, 0, 4), (5, 2, 0, 4), (4, 3, 0, 4), (7, 2, 0, 5),
    (5, 3, 0, 6), (4, 4, 0, 7), (5, 4, 0, 8), (8, 1, 0, 8), (8, 3, 0, 8), (6, 4, 0, 10), (7, 4, 0, 12), (8, 4, 0, 20),
];

const PITCH: f32 = 128.0;
const SKINS: [Skin; 4] = [art::EMERALD, art::SKY, art::TANGERINE, art::BLOSSOM];

struct Frog {
    pos: V2,
    hp: u8,
    max_hp: u8,
    skin: Skin,
    age: f32,
    hit_t: f32,
    pop_t: f32,
    helmet_fly: f32,
    seed: f32,
}

pub struct Pop {
    frogs: Vec<Frog>,
    wave: usize,
    croak_t: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Pop { frogs: Vec::new(), wave: 0, croak_t: 1.0 };
    g.new_wave(c);
    Box::new(g)
}

impl Pop {
    fn new_wave(&mut self, c: &mut Ctx) {
        let (cols, rows, light, heavy) = WAVES[self.wave.min(WAVES.len() - 1)];
        let n = cols * rows;
        let mut hp = vec![1u8; n];
        let mut idx: Vec<usize> = (0..n).collect();
        c.rng.shuffle(&mut idx);
        for (k, &i) in idx.iter().enumerate() {
            if k < light {
                hp[i] = 2;
            } else if k < light + heavy {
                hp[i] = 3;
            }
        }
        let cx = c.center_x();
        let cy = c.h * 0.5 + 45.0;
        self.frogs = (0..n)
            .map(|k| {
                let (col, row) = (k % cols, k / cols);
                let pos = v2(cx + PITCH * (col as f32 - (cols as f32 - 1.0) * 0.5), cy + PITCH * (row as f32 - (rows as f32 - 1.0) * 0.5));
                Frog { pos, hp: hp[k], max_hp: hp[k], skin: *c.rng.pick(&SKINS), age: 0.0, hit_t: 9.0, pop_t: -1.0, helmet_fly: -1.0, seed: c.rng.f() * 9.0 }
            })
            .collect();
    }
}

impl MiniGame for Pop {
    fn idle(&mut self, _c: &mut Ctx, dt: f32) {
        for f in self.frogs.iter_mut() {
            f.age += dt;
        }
    }

    fn update(&mut self, c: &mut Ctx, dt: f32) {
        for f in self.frogs.iter_mut() {
            f.age += dt;
            f.hit_t += dt;
            if f.pop_t >= 0.0 {
                f.pop_t += dt;
            }
            if f.helmet_fly >= 0.0 {
                f.helmet_fly += dt;
            }
        }
        // ambient croaks from a random live frog
        self.croak_t -= dt;
        if self.croak_t <= 0.0 {
            self.croak_t = 0.1 + c.rng.f() * 1.0;
            if c.rng.chance(0.25) {
                let p = c.rng.range(0.9, 1.25);
                c.sfx_ex(Sfx::Ribbit, 0.35, p);
            }
        }
        if self.frogs.iter().all(|f| f.hp == 0 && f.pop_t > 0.12) {
            c.add_time(1.0, v2(c.center_x(), c.h * 0.5));
            self.wave += 1;
            self.new_wave(c);
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down {
            return;
        }
        let half = PITCH * 0.5;
        if let Some(f) = self.frogs.iter_mut().find(|f| f.hp > 0 && (t.pos.x - f.pos.x).abs() < half && (t.pos.y - f.pos.y).abs() < half) {
            f.hp -= 1;
            f.hit_t = 0.0;
            match (f.max_hp, f.hp) {
                (_, 0) => {
                    f.pop_t = 0.0;
                    let (pos, col) = (f.pos, f.skin.body);
                    c.add_score(1, pos - v2(0.0, 70.0));
                    c.sfx_var(Sfx::Pop);
                    c.tick();
                    c.fx.burst(pos, col, 12, 420.0);
                    c.fx.ring(pos, Col::WHITE, 60.0);
                }
                (3, 2) => {
                    c.sfx(Sfx::Crack);
                    c.tick();
                }
                _ => {
                    f.helmet_fly = 0.0;
                    c.sfx_ex(Sfx::Kick, 0.8, 1.4);
                    c.tick();
                }
            }
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0xEAF7D9), hex(0xC6E9A6));
        // soft paper dots
        for i in 0..30 {
            let x = (i as f32 * 173.0) % w;
            let y = (i as f32 * 97.0) % h;
            r.disc(x, y, 18.0 + (i % 4) as f32 * 8.0, Col::WHITE.a(0.25));
        }
        for f in &self.frogs {
            let appear = ease_out_back((f.age / 0.12).min(1.0)) * 0.5 + 0.5;
            if f.pop_t >= 0.0 {
                if f.pop_t < 0.25 {
                    r.push();
                    r.fade(1.0 - f.pop_t / 0.25);
                    art::frog(r, f.pos.x, f.pos.y, 112.0 * (1.0 + f.pop_t * 2.5), f.skin, &Pose { dizzy: true, mouth: 1.0, ..Pose::default() });
                    r.pop();
                }
                continue;
            }
            r.ellipse(f.pos.x, f.pos.y + 52.0, 48.0, 10.0, art::SHADOW, art::SHADOW, 0.0);
            let squash = if f.hit_t < 0.3 { wobble(f.hit_t, 30.0) * 0.18 } else { 0.0 };
            let mut pose = Pose::idle(c.anim, f.seed);
            pose.squash = squash;
            if f.hit_t < 0.25 {
                pose.blink = 1.0;
            }
            art::frog(r, f.pos.x, f.pos.y, 112.0 * appear, f.skin, &pose);
            // helmets
            let hy = f.pos.y - 34.0 * appear;
            match (f.max_hp, f.hp) {
                (2, 2) | (3, 3) | (3, 2) => {
                    let heavy = f.max_hp == 3;
                    let col = if heavy { hex(0x8E9BAA) } else { hex(0xFFC93C) };
                    r.push();
                    r.translate(f.pos.x, hy);
                    r.scale(appear * (1.0 + squash), appear * (1.0 - squash));
                    helmet_dome(r, col);
                    if heavy {
                        for k in 0..3 {
                            r.disc(-24.0 + k as f32 * 24.0, -22.0, 4.0, art::INK.a(0.6));
                        }
                    } else {
                        r.rrect(0.0, -30.0, 12.0, 26.0, 6.0, col.shade(0.2), art::INK, 3.0);
                    }
                    if f.max_hp == 3 && f.hp == 2 {
                        // cracks
                        r.line(v2(-10.0, -40.0), v2(4.0, -22.0), 4.0, art::INK);
                        r.line(v2(4.0, -22.0), v2(-6.0, -8.0), 4.0, art::INK);
                        r.line(v2(4.0, -22.0), v2(22.0, -16.0), 4.0, art::INK);
                    }
                    r.pop();
                }
                _ => {}
            }
            if f.helmet_fly >= 0.0 && f.helmet_fly < 0.4 {
                let k = f.helmet_fly / 0.4;
                let col = if f.max_hp == 3 { hex(0x8E9BAA) } else { hex(0xFFC93C) };
                r.push();
                r.translate(f.pos.x + k * 60.0, hy - k * 120.0);
                r.rotate(k * 2.0);
                r.scale(1.0 + k, 1.0 + k);
                r.fade(1.0 - k);
                helmet_dome(r, col);
                r.pop();
            }
        }
    }
}

/// Half-dome helmet with a brim, drawn around the origin.
fn helmet_dome(r: &mut Renderer, col: Col) {
    r.push();
    r.rotate(-std::f32::consts::FRAC_PI_2);
    r.pie(0.0, 0.0, 54.0, 0.5, art::INK);
    r.pie(0.0, 0.0, 49.5, 0.5, col);
    r.pop();
    r.ellipse_rot(-20.0, -30.0, 13.0, 7.0, -0.6, Col::WHITE.a(0.55), Col::CLEAR, 0.0);
    r.rrect(0.0, 2.0, 120.0, 17.0, 8.5, col.shade(-0.15), art::INK, 4.0);
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    art::frog(r, x - s * 0.2, y + s * 0.05, s * 0.42, art::EMERALD, &Pose::idle(t, 0.0));
    art::frog(r, x + s * 0.22, y + s * 0.05, s * 0.42, art::SKY, &Pose::idle(t, 1.0));
    r.push();
    r.translate(x + s * 0.22, y - s * 0.09);
    r.scale(s * 0.0037, s * 0.0037);
    helmet_dome(r, hex(0xFFC93C));
    r.pop();
    art::sparkle(r, x - s * 0.36, y - s * 0.3, s * 0.06, hex(0xFFE066));
}
