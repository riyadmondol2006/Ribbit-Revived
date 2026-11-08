//! Sky Spring: climb a sky of planks with diagonal hops (up-left / up-right buttons).
//! Cracked planks break under the frog; you drop back onto the same row after a short fall.
//! Every block of ten rows hides one clock (+2 s). Rows are generated so a path always exists.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};
use crate::ui::{self, Icon};

const PITCH_X: f32 = 129.5;
const PITCH_Y: f32 = 190.0;

#[derive(Clone, Copy, PartialEq)]
enum Cell {
    Empty,
    Broken,
    Solid,
    Clock,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Idle,
    Jump { from: (i32, i32), t: f32 },
    Fall { t: f32 },
    Respawn { t: f32 },
}

pub struct Leap {
    rows: Vec<[Cell; 7]>,
    path_col: i32,
    col: i32,
    row: i32,
    cam: f32,
    state: State,
    btn_t: [f32; 2],
    fall_x: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Leap { rows: Vec::new(), path_col: 3, col: 3, row: -1, cam: -1.0, state: State::Idle, btn_t: [9.0; 2], fall_x: 0.0 };
    g.extend(c, 40);
    Box::new(g)
}

impl Leap {
    fn extend(&mut self, c: &mut Ctx, upto: usize) {
        while self.rows.len() < upto {
            let r = self.rows.len();
            let parity = (r % 2) as i32; // row 0 uses even columns
            // the guaranteed path takes a diagonal step each row
            let step = if self.path_col <= 0 {
                1
            } else if self.path_col >= 6 {
                -1
            } else if c.rng.chance(0.5) {
                1
            } else {
                -1
            };
            self.path_col += step;
            let mut row = [Cell::Empty; 7];
            let separator = r % 10 == 9;
            let solid_p = 0.3 + if r < 10 { 0.25 } else { 0.0 };
            for (col, cell) in row.iter_mut().enumerate() {
                if col as i32 % 2 != parity {
                    continue;
                }
                *cell = if separator || col as i32 == self.path_col || c.rng.chance(solid_p) { Cell::Solid } else { Cell::Broken };
            }
            if r % 10 == 4 {
                row[self.path_col as usize] = Cell::Clock;
            }
            self.rows.push(row);
        }
    }
    fn cell(&self, col: i32, row: i32) -> Cell {
        if row < 0 || !(0..7).contains(&col) {
            return Cell::Empty;
        }
        self.rows.get(row as usize).map(|r| r[col as usize]).unwrap_or(Cell::Empty)
    }
    fn screen(&self, c: &Ctx, col: f32, row: f32) -> V2 {
        v2(c.center_x() + (col - 3.0) * PITCH_X, c.h * 0.78 - (row - self.cam) * PITCH_Y)
    }
    fn buttons(c: &Ctx) -> (V2, V2) {
        (v2(c.safe_l + 140.0, c.h - 88.0), v2(c.safe_r - 140.0, c.h - 88.0))
    }
    fn hop(&mut self, c: &mut Ctx, dir: i32) {
        if self.state != State::Idle {
            return;
        }
        let nc = self.col + dir;
        if !(0..7).contains(&nc) {
            return;
        }
        self.state = State::Jump { from: (self.col, self.row), t: 0.0 };
        self.col = nc;
        self.row += 1;
        c.sfx_var(Sfx::Jump);
        c.tick();
        let need = self.row as usize + 12;
        self.extend(c, need);
    }
}

impl MiniGame for Leap {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        for b in self.btn_t.iter_mut() {
            *b += dt;
        }
        let target = self.row as f32;
        self.cam += (target - self.cam) * (dt / 0.15).min(1.0);
        match self.state {
            State::Idle => {}
            State::Jump { from, t } => {
                let t = t + dt;
                if t < 0.2 {
                    self.state = State::Jump { from, t };
                } else {
                    match self.cell(self.col, self.row) {
                        Cell::Solid | Cell::Clock => {
                            let is_clock = self.cell(self.col, self.row) == Cell::Clock;
                            let p = self.screen(c, self.col as f32, self.row as f32);
                            c.add_score(1, p - v2(0.0, 120.0));
                            c.special += 1;
                            c.sfx_ex(Sfx::Bounce, 0.5, 1.3);
                            if is_clock {
                                self.rows[self.row as usize][self.col as usize] = Cell::Solid;
                                c.add_time(2.0, p - v2(0.0, 160.0));
                            }
                            self.state = State::Idle;
                        }
                        _ => {
                            self.state = State::Fall { t: 0.0 };
                            self.fall_x = self.screen(c, self.col as f32, 0.0).x;
                            c.special = 0;
                            c.sfx(Sfx::Crack);
                            c.reject();
                        }
                    }
                }
            }
            State::Fall { t } => {
                let t = t + dt;
                if t < 0.7 {
                    self.state = State::Fall { t };
                } else {
                    // reappear on the first solid plank of the same row
                    let row = self.row;
                    if let Some(col) = (0..7).find(|&cc| matches!(self.cell(cc, row), Cell::Solid | Cell::Clock)) {
                        self.col = col;
                    }
                    self.state = State::Respawn { t: 0.0 };
                    c.sfx_ex(Sfx::Swap, 0.5, 1.0);
                }
            }
            State::Respawn { t } => {
                let t = t + dt;
                self.state = if t < 0.2 { State::Respawn { t } } else { State::Idle };
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down {
            return;
        }
        if t.pos.x < c.center_x() {
            self.btn_t[0] = 0.0;
            self.hop(c, -1);
        } else {
            self.btn_t[1] = 0.0;
            self.hop(c, 1);
        }
    }

    fn score_text(&self, c: &Ctx) -> String {
        format!("{} m", c.score)
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        let hgt = (self.cam / 120.0).clamp(0.0, 1.0);
        r.grad_rect(0.0, 0.0, w, h, hex(0x86D6FF).mix(hex(0x5B6CD8), hgt), hex(0xE3F8FF).mix(hex(0xF5B3D8), hgt));
        for i in 0..5 {
            let y = ((i as f32 * 190.0) + self.cam * 40.0) % (h + 200.0) - 100.0;
            art::cloud(r, c.safe_l + ((i * 613) % 1000) as f32 / 1000.0 * (c.safe_r - c.safe_l), y, 1.0, Col::WHITE.a(0.7));
        }
        // ground at the start
        let gy = self.screen(c, 3.0, -1.0).y + 40.0;
        if gy < h + 60.0 {
            art::hills(r, gy, hex(0x8FD06A), 40.0, 0.0);
        }
        let lo = (self.cam - 2.0).floor().max(0.0) as i32;
        let hi = (self.cam + 5.0).ceil() as i32;
        for row in lo..=hi {
            for col in 0..7 {
                let cell = self.cell(col, row);
                if cell == Cell::Empty {
                    continue;
                }
                let p = self.screen(c, col as f32, row as f32);
                let falling_here = matches!(self.state, State::Fall { .. }) && row == self.row && col == self.col;
                if falling_here {
                    if let State::Fall { t } = self.state {
                        let drop = 0.5 * 10.0 * h * t * t;
                        r.push();
                        r.translate(p.x, p.y + drop);
                        r.rotate(t * 2.0);
                        r.rrect(-30.0, 0.0, 54.0, 20.0, 6.0, hex(0xB07A45), art::INK, 3.0);
                        r.rrect(30.0, 0.0, 54.0, 20.0, 6.0, hex(0xB07A45), art::INK, 3.0);
                        r.pop();
                    }
                    continue;
                }
                match cell {
                    Cell::Broken => {
                        r.rrect(p.x, p.y, 120.0, 22.0, 8.0, hex(0xC9935A), art::INK, 3.0);
                        r.line(v2(p.x - 6.0, p.y - 10.0), v2(p.x + 4.0, p.y), 3.0, art::INK);
                        r.line(v2(p.x + 4.0, p.y), v2(p.x - 2.0, p.y + 10.0), 3.0, art::INK);
                    }
                    _ => {
                        let col_ = if row > 100 { hex(0xC59BFF) } else if row > 50 { hex(0x7CC8FF) } else { hex(0x8BE04E) };
                        r.rrect(p.x, p.y, 120.0, 22.0, 8.0, col_, art::INK, 3.0);
                        r.rrect(p.x, p.y - 4.0, 104.0, 6.0, 3.0, Col::WHITE.a(0.35), Col::CLEAR, 0.0);
                        if cell == Cell::Clock {
                            art::clock_icon(r, p.x, p.y - 50.0 + (c.anim * 3.0).sin() * 4.0, 24.0, 1.0);
                        }
                    }
                }
            }
            if row % 10 == 0 && row > 0 {
                let p = self.screen(c, 6.6, row as f32);
                r.text_bold(&format!("{row}"), p.x + 20.0, p.y, 24.0, Align::Left, Col::WHITE, art::INK);
            }
        }
        // frog
        let (pos, pose, alpha) = match self.state {
            State::Idle => (self.screen(c, self.col as f32, self.row as f32) - v2(0.0, 58.0), Pose::idle(c.anim, 0.0), 1.0),
            State::Jump { from, t } => {
                let k = (t / 0.2).min(1.0);
                let a = self.screen(c, from.0 as f32, from.1 as f32);
                let b = self.screen(c, self.col as f32, self.row as f32);
                let p = a.lerp(b, k) - v2(0.0, 58.0 + (k * std::f32::consts::PI).sin() * 60.0);
                (p, Pose { legs_out: 1.0, mouth: 0.3, ..Pose::default() }, 1.0)
            }
            State::Fall { t } => {
                let p = self.screen(c, self.col as f32, self.row as f32) - v2(0.0, 58.0) + v2(0.0, 0.5 * 10.0 * h * t * t);
                (p, Pose { dizzy: true, mouth: 0.8, ..Pose::default() }, 1.0)
            }
            State::Respawn { t } => (self.screen(c, self.col as f32, self.row as f32) - v2(0.0, 58.0), Pose::idle(c.anim, 0.0), t / 0.2),
        };
        r.push();
        r.fade(alpha);
        art::frog(r, pos.x, pos.y, 110.0, art::EMERALD, &pose);
        r.pop();
        let _ = self.fall_x;
        // buttons
        let (lb, rb) = Self::buttons(c);
        for (i, (b, icon)) in [(lb, Icon::Left), (rb, Icon::Right)].iter().enumerate() {
            let d = if self.btn_t[i] < 0.12 { 6.0 } else { 0.0 };
            r.rrect(b.x, b.y + 8.0, 210.0, 124.0, 34.0, hex(0x5B3FA8), art::INK, 4.0);
            r.rrect_grad(b.x, b.y + d, 210.0, 124.0, 34.0, hex(0xB89CFF), hex(0x8566E0), art::INK, 4.0);
            r.push();
            r.translate(b.x, b.y + d);
            r.rotate(if i == 0 { 0.6 } else { -0.6 });
            ui::draw_icon(r, Icon::Up, 0.0, 0.0, 60.0, Col::WHITE);
            r.pop();
            let _ = icon;
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    for (i, (dx, dy)) in [(-0.3, 0.3), (0.0, 0.05), (0.3, -0.2), (-0.1, -0.35)].iter().enumerate() {
        r.rrect(x + dx * s, y + dy * s, s * 0.3, s * 0.06, 4.0, if i == 3 { hex(0xC9935A) } else { hex(0x8BE04E) }, art::INK, 2.0);
    }
    let k = (t * 0.8).fract();
    art::frog(r, x + lerp(-0.3, 0.0, k) * s, y + lerp(0.2, -0.05, k) * s - (k * std::f32::consts::PI).sin() * s * 0.15, s * 0.28, art::EMERALD, &Pose::idle(t, 0.0));
}
