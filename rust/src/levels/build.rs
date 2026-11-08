//! Tower Toad: a floor of rooms swings across the sky; tap to drop it onto the tower.
//! Only rooms overlapping the floor below stay on, so the tower narrows with sloppy drops.
//! Score = rooms placed; +1 s every second floor. The swing speeds up as the tower grows.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const ROOM: f32 = 87.0;

#[derive(Clone, Copy)]
struct Room {
    x: f32,
    style: u8,
}

struct Falling {
    rooms: Vec<Room>,
    y: f32,
    vy: f32,
}

struct Dropped {
    room: Room,
    y: f32,
    vy: f32,
    rot: f32,
    vx: f32,
}

pub struct Build {
    floors: Vec<Vec<Room>>,
    n: usize,
    swing_x: f32,
    dir: f32,
    falling: Option<Falling>,
    dropped: Vec<Dropped>,
    cam: f32,
    base_y: f32,
    land_t: f32,
    next_styles: Vec<u8>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let base_y = c.h * 0.5 + 114.0 + ROOM * 0.5;
    let cx = c.center_x();
    let ground: Vec<Room> = (0..8).map(|i| Room { x: cx + (i as f32 - 3.5) * ROOM, style: c.rng.below(22) as u8 }).collect();
    let dir = if c.rng.chance(0.5) { 1.0 } else { -1.0 };
    let mut g = Build { floors: vec![ground], n: 8, swing_x: 0.0, dir, falling: None, dropped: Vec::new(), cam: 0.0, base_y, land_t: 9.0, next_styles: Vec::new() };
    g.swing_x = cx - dir * (c.safe_r - c.safe_l) * 0.5;
    g.roll_styles(c);
    Box::new(g)
}

fn room_col(style: u8) -> Col {
    const C: [u32; 8] = [0xF0605D, 0xFF9F2E, 0xF4D03F, 0x4CC38A, 0x4FA7F2, 0x9A6BEA, 0xF27BC4, 0x5ED1C5];
    hex(C[style as usize % 8])
}

fn draw_room(r: &mut Renderer, x: f32, y: f32, style: u8) {
    let col = room_col(style);
    r.rrect(x, y, ROOM, ROOM, 8.0, col, art::INK, 4.0);
    r.rect(x - ROOM * 0.5 + 6.0, y - ROOM * 0.5 + 6.0, ROOM - 12.0, 10.0, col.shade(0.2));
    match style % 3 {
        0 => {
            r.rrect(x, y + 8.0, 40.0, 38.0, 6.0, hex(0xBFE9FF), art::INK, 3.0);
            r.rect(x - 1.5, y - 11.0, 3.0, 38.0, art::INK);
            r.rect(x - 20.0, y + 6.5, 40.0, 3.0, art::INK);
        }
        1 => {
            r.rrect(x - 16.0, y + 8.0, 22.0, 30.0, 6.0, hex(0xFFF0B3), art::INK, 3.0);
            r.rrect(x + 16.0, y + 8.0, 22.0, 30.0, 6.0, hex(0xFFF0B3), art::INK, 3.0);
        }
        _ => {
            r.circle(x, y + 6.0, 18.0, hex(0xBFE9FF), art::INK, 3.0);
            r.rrect(x, y + 34.0, 30.0, 8.0, 3.0, col.shade(-0.3), Col::CLEAR, 0.0);
        }
    }
}

impl Build {
    fn roll_styles(&mut self, c: &mut Ctx) {
        self.next_styles = (0..8).map(|_| c.rng.below(22) as u8).collect();
    }
    fn hook_y(&self) -> f32 {
        180.0
    }
    fn sweep_time(&self) -> f32 {
        (2.0 * 0.95f32.powi(self.floors.len() as i32)).max(0.7)
    }
    fn floor_y(&self, k: usize) -> f32 {
        self.base_y - k as f32 * ROOM
    }
    fn swing_rooms(&self) -> Vec<Room> {
        (0..self.n).map(|i| Room { x: self.swing_x + (i as f32 - (self.n as f32 - 1.0) * 0.5) * ROOM, style: self.next_styles[i] }).collect()
    }
}

impl MiniGame for Build {
    fn idle(&mut self, c: &mut Ctx, dt: f32) {
        self.update_swing(c, dt);
    }

    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.land_t += dt;
        let target_cam = ((self.floors.len() as f32 - 2.0) * ROOM).max(0.0);
        self.cam += (target_cam - self.cam) * (dt * 7.0).min(1.0);
        if self.falling.is_none() {
            self.update_swing(c, dt);
        }
        let top_k = self.floors.len() - 1;
        let land_y = self.floor_y(top_k) - ROOM;
        if let Some(f) = &mut self.falling {
            f.vy += c.h / 0.1 * dt;
            f.y += f.vy * dt;
            if f.y >= land_y {
                let below = &self.floors[top_k];
                let l = below.iter().map(|r| r.x).fold(f32::MAX, f32::min);
                let rgt = below.iter().map(|r| r.x).fold(f32::MIN, f32::max);
                let rooms = std::mem::take(&mut f.rooms);
                let mut kept = Vec::new();
                for rm in rooms {
                    if rm.x > l - ROOM && rm.x < rgt + ROOM {
                        kept.push(rm);
                    } else {
                        let side = if rm.x < l { -1.0 } else { 1.0 };
                        self.dropped.push(Dropped { room: rm, y: land_y, vy: f.vy * 0.3, rot: 0.0, vx: side * 160.0 });
                    }
                }
                self.falling = None;
                if kept.is_empty() {
                    c.sfx(Sfx::Whoosh);
                    c.reject();
                } else {
                    let n = kept.len();
                    let mid = kept.iter().map(|r| r.x).sum::<f32>() / n as f32;
                    for rm in &kept {
                        c.fx.burst(v2(rm.x, land_y + ROOM * 0.5 + self.cam), hex(0xE8DCC8), 4, 160.0);
                    }
                    self.floors.push(kept);
                    self.n = n;
                    c.add_score(n as i64, v2(mid, land_y + self.cam - 80.0));
                    c.sfx_var(Sfx::Thud);
                    c.tick();
                    self.land_t = 0.0;
                    let floors = self.floors.len();
                    if floors % 2 == 1 {
                        c.add_time(1.0, v2(mid, land_y + self.cam - 150.0));
                    }
                    if floors >= 10 && n == 8 {
                        c.special_hit = true;
                    }
                }
                self.roll_styles(c);
                // next floor enters from a random side
                self.dir = if c.rng.chance(0.5) { 1.0 } else { -1.0 };
                self.swing_x = c.center_x() - self.dir * (c.safe_r - c.safe_l) * 0.5;
            }
        }
        for d in self.dropped.iter_mut() {
            d.vy += 2400.0 * dt;
            d.y += d.vy * dt;
            d.x_move(dt);
        }
        let limit = c.h + 400.0 + self.cam;
        self.dropped.retain(|d| d.y < limit);
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.falling.is_some() {
            return;
        }
        let y = self.hook_y() + 40.0 - self.cam;
        self.falling = Some(Falling { rooms: self.swing_rooms(), y, vy: 0.0 });
        c.sfx_ex(Sfx::Whoosh, 0.5, 1.5);
    }

    fn score_text(&self, c: &Ctx) -> String {
        c.score.to_string()
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        let height = (self.floors.len() as f32 / 40.0).min(1.0);
        r.grad_rect(0.0, 0.0, w, h, hex(0x7FD3FF).mix(hex(0x3B3F9E), height), hex(0xDDF5FF).mix(hex(0xFFB89A), height));
        for i in 0..4 {
            let y = (i as f32 * 210.0 + self.cam * 0.4) % (h + 200.0) - 100.0;
            art::cloud(r, c.safe_l + ((i * 431) % 1000) as f32 / 1000.0 * (c.safe_r - c.safe_l), y, 0.8, Col::WHITE.a(0.8));
        }
        let ground_y = self.floor_y(0) + ROOM * 0.5 + self.cam;
        if ground_y < h + 10.0 {
            art::hills(r, ground_y - 20.0, hex(0x8FD06A), 40.0, 0.0);
            r.rect(0.0, ground_y, w, h - ground_y + 1.0, hex(0x6DA843));
        }
        for (k, fl) in self.floors.iter().enumerate() {
            let y = self.floor_y(k) + self.cam;
            if y < -ROOM || y > h + ROOM {
                continue;
            }
            let bounce = if k + 1 == self.floors.len() && self.land_t < 0.3 { wobble(self.land_t, 30.0) * 6.0 } else { 0.0 };
            for rm in fl {
                draw_room(r, rm.x, y + bounce, rm.style);
            }
        }
        for d in &self.dropped {
            r.push();
            r.translate(d.room.x, d.y + self.cam);
            r.rotate(d.rot);
            draw_room(r, 0.0, 0.0, d.room.style);
            r.pop();
        }
        // crane hook + swinging floor
        let hook = self.hook_y();
        match &self.falling {
            None => {
                r.line(v2(self.swing_x, -10.0), v2(self.swing_x, hook - 40.0), 6.0, hex(0x5A6178));
                r.ring(self.swing_x, hook - 26.0, 14.0, 5.0, hex(0x5A6178));
                let y = hook + 30.0;
                r.rrect(self.swing_x, y - ROOM * 0.5 - 6.0, self.n as f32 * ROOM, 12.0, 4.0, hex(0xFFC93C), art::INK, 3.0);
                for rm in self.swing_rooms() {
                    draw_room(r, rm.x, y + 10.0, rm.style);
                }
                let pose = Pose { look: v2(0.0, 0.8), ..Pose::idle(c.anim, 0.0) };
                art::frog(r, self.swing_x, y - ROOM * 0.5 - 52.0, 80.0, art::EMERALD, &pose);
                r.ellipse(self.swing_x, y - ROOM * 0.5 - 76.0, 34.0, 14.0, hex(0xFFC93C), art::INK, 3.0);
            }
            Some(f) => {
                for rm in &f.rooms {
                    draw_room(r, rm.x, f.y + self.cam, rm.style);
                }
            }
        }
        let _ = c;
    }
}

impl Dropped {
    fn x_move(&mut self, dt: f32) {
        self.room.x += self.vx * dt;
        self.rot += self.vx.signum() * 4.0 * dt;
    }
}

impl Build {
    fn update_swing(&mut self, c: &mut Ctx, dt: f32) {
        let span = c.safe_r - c.safe_l;
        let speed = span / self.sweep_time();
        self.swing_x += self.dir * speed * dt;
        let lo = c.center_x() - span * 0.5;
        let hi = c.center_x() + span * 0.5;
        if self.swing_x > hi {
            self.swing_x = hi;
            self.dir = -1.0;
        } else if self.swing_x < lo {
            self.swing_x = lo;
            self.dir = 1.0;
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.push();
    r.translate(x, y + s * 0.18);
    r.scale(s / 300.0, s / 300.0);
    for k in 0..3 {
        for i in 0..(3 - k) {
            draw_room(r, (i as f32 - (2 - k) as f32 * 0.5) * ROOM, -(k as f32) * ROOM, (k * 3 + i) as u8);
        }
    }
    let sw = (t * 2.0).sin() * 60.0;
    draw_room(r, sw, -3.0 * ROOM - 50.0, 4);
    r.pop();
}
