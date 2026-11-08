//! Balloon Duel (2 players): drag down on your half of the screen to work your pump. Each full
//! stroke inflates your balloon a little (0.4 -> 1.0, 31 pumps); the first full balloon floats
//! its frog away and wins. No clock.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const STROKE: f32 = 202.0;

#[derive(Default)]
struct Side {
    handle: f32,
    fill: f32,
    win_t: f32,
    puff: f32,
}

pub struct Balloon {
    sides: [Side; 2],
    fingers: Vec<(i32, usize, f32)>,
    winner: Option<usize>,
    clouds: Vec<(f32, f32, f32)>,
    cpu_t: f32,
    cpu_phase: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let clouds = (0..5).map(|i| (c.rng.f(), 60.0 + i as f32 * 45.0, c.rng.range(0.6, 1.1))).collect();
    let mk = || Side { handle: 1.0, fill: 0.4, win_t: 0.0, puff: 0.0 };
    Box::new(Balloon { sides: [mk(), mk()], fingers: Vec::new(), winner: None, clouds, cpu_t: 0.5, cpu_phase: 0.0 })
}

impl Balloon {
    fn frog_x(c: &Ctx, side: usize) -> f32 {
        if side == 0 {
            c.sx(0.27)
        } else {
            c.sx(0.73)
        }
    }
    fn pump_x(c: &Ctx, side: usize) -> f32 {
        if side == 0 {
            c.sx(0.08)
        } else {
            c.sx(0.92)
        }
    }
}

impl MiniGame for Balloon {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        // computer pumps blue at a steady, beatable pace (~0.38 s per stroke)
        if c.cpu && self.winner.is_none() && self.sides[1].fill < 1.0 {
            self.cpu_phase += dt;
            self.cpu_t -= dt;
            let s = &mut self.sides[1];
            s.handle = 0.5 + 0.5 * (self.cpu_phase * std::f32::consts::TAU / 0.38).cos();
            if self.cpu_t <= 0.0 {
                self.cpu_t = c.rng.range(0.33, 0.42);
                s.fill = (s.fill + 0.02).min(1.0);
                s.puff = 1.0;
                c.sfx_ex(Sfx::Pump, 0.6, 1.05);
                if s.fill >= 0.999 {
                    self.winner = Some(1);
                    c.sfx(Sfx::Win);
                }
            }
        }
        for (i, s) in self.sides.iter_mut().enumerate() {
            s.puff = (s.puff - dt * 3.0).max(0.0);
            if self.winner == Some(i) {
                s.win_t += dt;
            }
        }
        if let Some(wn) = self.winner {
            if self.sides[wn].win_t > 2.6 && !c.end_now {
                c.winner = Some(wn as i8);
                c.end_now = true;
            }
        }
        for cl in self.clouds.iter_mut() {
            cl.0 -= dt * 0.02 * cl.2;
            if cl.0 < -0.2 {
                cl.0 = 1.2;
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        match t.phase {
            Phase::Down => {
                let side = if t.pos.x < c.center_x() { 0 } else { 1 };
                if c.cpu && side == 1 {
                    return;
                }
                self.fingers.push((t.id, side, t.pos.y));
            }
            Phase::Move => {
                let Some(f) = self.fingers.iter_mut().find(|f| f.0 == t.id) else { return };
                let side = f.1;
                let dy = t.pos.y - f.2;
                f.2 = t.pos.y;
                if self.winner == Some(side) {
                    return;
                }
                let s = &mut self.sides[side];
                if s.fill >= 1.0 {
                    return;
                }
                if dy > 0.0 && s.handle > 0.0 {
                    s.handle -= dy / STROKE;
                    if s.handle < 0.0 {
                        s.handle = 0.0;
                        s.fill = (s.fill + 0.02).min(1.0);
                        s.puff = 1.0;
                        audio_pump(c, side);
                        if s.fill >= 0.999 && self.winner.is_none() {
                            self.winner = Some(side);
                            c.sfx(Sfx::Win);
                            c.heavy();
                        }
                    }
                } else if dy < 0.0 && s.handle < 1.0 {
                    s.handle += -dy / STROKE;
                    if s.handle > 1.0 {
                        s.handle = 1.0;
                        c.sfx_ex(Sfx::Swap, 0.4, if side == 0 { 0.8 } else { 0.9 });
                    }
                }
            }
            Phase::Up | Phase::Cancel => self.fingers.retain(|f| f.0 != t.id),
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        art::sky(r, hex(0x8FD8FF), hex(0xDDF6FF));
        for cl in &self.clouds {
            art::cloud(r, cl.0 * w, cl.1, cl.2, Col::WHITE.a(0.85));
        }
        art::hills(r, h * 0.78, hex(0x9BD86E), 40.0, 0.0);
        r.grad_rect(0.0, h * 0.86, w, h * 0.14, hex(0xD9B77A), hex(0xB08850));
        // centre line
        r.rect(c.center_x() - 3.0, h * 0.2, 6.0, h * 0.8, Col::WHITE.a(0.35));
        for side in 0..2 {
            let s = &self.sides[side];
            let skin = if side == 0 { art::RUBY } else { art::SKY };
            let fx = Self::frog_x(c, side);
            let ground = h * 0.9;
            let lift = if self.winner == Some(side) { ease_in_out(s.win_t / 2.6) * h * 0.9 } else { 0.0 };
            let sway = if self.winner == Some(side) { (s.win_t * 2.0).sin() * 0.2 } else { 0.0 };
            // pump
            let px = Self::pump_x(c, side);
            let top = ground - 150.0 - s.handle * 0.9 * 150.0 * 0.7;
            r.rrect(px, ground - 70.0, 60.0, 140.0, 10.0, hex(0x5A6178), art::INK, 4.0);
            r.rect(px - 5.0, top, 10.0, ground - 140.0 - top, hex(0x9AA5B1));
            r.rrect(px, top, 120.0, 22.0, 10.0, hex(0xC9D2DC), art::INK, 4.0);
            // hose to balloon
            let bx = fx + if side == 0 { 40.0 } else { -40.0 };
            let rad = 40.0 + s.fill * 110.0 + s.puff * 6.0;
            let by = ground - 230.0 - rad - lift;
            r.line(v2(px, ground - 20.0), v2((px + fx) * 0.5, ground - 10.0), 8.0, hex(0x2B2B2B));
            r.line(v2((px + fx) * 0.5, ground - 10.0), v2(bx, by + rad), 6.0, art::INK.a(0.7));
            // frog (floats away on a win)
            r.push();
            r.translate(fx, ground - 60.0 - lift);
            r.rotate(sway);
            let pose = Pose {
                mouth: if s.puff > 0.0 { 0.9 } else { 0.2 },
                happy_eyes: self.winner == Some(side),
                squash: s.puff * 0.08,
                ..Pose::idle(c.anim, side as f32)
            };
            art::frog(r, 0.0, 0.0, 170.0, skin, &pose);
            r.pop();
            art::balloon(r, bx, by, rad, skin.body);
            // progress meter
            let mx = if side == 0 { c.sx(0.42) } else { c.sx(0.58) };
            r.rrect(mx, h * 0.5, 34.0, 300.0, 17.0, Col(0, 0, 0, 60), art::INK, 3.0);
            let fillh = 300.0 * ((s.fill - 0.4) / 0.6).clamp(0.0, 1.0);
            if fillh > 4.0 {
                r.rrect(mx, h * 0.5 + 150.0 - fillh * 0.5, 26.0, fillh - 4.0, 13.0, skin.body, Col::CLEAR, 0.0);
            }
        }
        r.text_bold("Drag DOWN on your side to pump!", c.center_x(), 150.0, 30.0, crate::render::Align::Center, Col::WHITE, art::INK);
    }
}

fn audio_pump(c: &mut Ctx, side: usize) {
    c.sfx_ex(Sfx::Pump, 0.9, if side == 0 { 0.95 } else { 1.05 });
    c.tick();
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    let k = (t * 1.5).sin() * 0.05;
    art::balloon(r, x - s * 0.25, y - s * 0.18, s * (0.18 + k), art::RUBY.body);
    art::balloon(r, x + s * 0.25, y - s * 0.22, s * (0.22 - k), art::SKY.body);
    art::frog(r, x - s * 0.25, y + s * 0.22, s * 0.3, art::RUBY, &Pose::idle(t, 0.0));
    art::frog(r, x + s * 0.25, y + s * 0.22, s * 0.3, art::SKY, &Pose::idle(t, 1.0));
}
