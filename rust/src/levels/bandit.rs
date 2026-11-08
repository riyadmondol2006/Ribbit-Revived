//! Bandit Pop: masked bandit frogs peek out of the building's windows. Pop them all to clear
//! the facade (+1 s, +3 s for hopper rounds). Don't tap the innocent window props!
//! Bandit types: plain, hopper (jumps to another window), hat (pops matching neighbours),
//! sombrero (calls in helpers) and the boss (only poppable once every guard is gone).
use super::{Ctx, MiniGame, STAGE_W};
use crate::art::{self, Pose, Skin};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};

const COLS: usize = 6;
const ROWS: usize = 4;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Who {
    Empty,
    Decoy(u8),
    Plain,
    Hopper,
    Hat,
    Sombrero,
    Helper,
    Guard,
    Boss,
}

#[derive(Clone, Copy)]
struct Win {
    who: Who,
    alive: bool,
    open: f32,
    pop: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase_ {
    Play,
    Closing(f32, bool),
    Opening(f32),
}

pub struct Bandit {
    grid: [[Win; COLS]; ROWS],
    record: usize,
    phase: Phase_,
    wrong: Option<(V2, f32)>,
    hopper_grid: bool,
    hint: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Bandit { grid: [[Win { who: Who::Empty, alive: false, open: 0.0, pop: -1.0 }; COLS]; ROWS], record: 0, phase: Phase_::Opening(0.0), wrong: None, hopper_grid: false, hint: 9.0 };
    g.generate(c);
    Box::new(g)
}

impl Bandit {
    /// (rows, cols, bandit type 1..5, bandit count, decoys) for each record of the progression.
    fn record_spec(&self, c: &mut Ctx) -> (usize, usize, u8, usize, usize) {
        let r = self.record % 81;
        match r {
            0..=11 => {
                let sizes = [(2, 2, 1), (2, 2, 2), (2, 4, 2), (2, 4, 3), (2, 4, 4), (4, 4, 4), (4, 4, 5), (4, 4, 6), (4, 6, 6), (4, 6, 7), (4, 6, 8), (4, 6, 10)];
                let (rr, cc, n) = sizes[r];
                (rr, cc, 1, n, (r / 3).min(3))
            }
            12..=14 | 29..=31 | 46..=48 => (4, 6, 2, 1, 1 + c.rng.below(3)),
            15..=28 => (4, if r < 21 { 4 } else { 6 }, 3, 4 + (r - 15) / 2, 1 + c.rng.below(3)),
            32..=45 => (4, 6, 4, 1, 2 + (r - 32) * 9 / 13),
            49..=62 => (4, 6, 5, 1 + (r - 49) * 8 / 13, 1 + c.rng.below(3)),
            _ => {
                let t = 1 + c.rng.below(5) as u8;
                let n = match t {
                    1 => 12 + c.rng.below(12),
                    3 => 8 + c.rng.below(5),
                    5 => 6 + c.rng.below(10),
                    _ => 1,
                };
                (4, 6, t, n, 1 + c.rng.below(3))
            }
        }
    }

    fn generate(&mut self, c: &mut Ctx) {
        let (rows, cols, kind, count, decoys) = self.record_spec(c);
        self.hopper_grid = kind == 2;
        let mut list: Vec<Who> = Vec::new();
        match kind {
            1 => list.extend(std::iter::repeat_n(Who::Plain, count)),
            2 => list.push(Who::Hopper),
            3 => list.extend(std::iter::repeat_n(Who::Hat, count)),
            4 => list.push(Who::Sombrero),
            _ => {
                list.push(Who::Boss);
                list.extend(std::iter::repeat_n(Who::Guard, count));
            }
        }
        for _ in 0..decoys {
            list.push(Who::Decoy(c.rng.below(8) as u8));
        }
        let cells = rows * cols;
        list.truncate(cells);
        while list.len() < cells {
            list.push(Who::Empty);
        }
        c.rng.shuffle(&mut list);
        let c0 = (COLS - cols) / 2;
        let r0 = (ROWS - rows) / 2;
        for row in self.grid.iter_mut() {
            for w in row.iter_mut() {
                *w = Win { who: Who::Empty, alive: false, open: 0.0, pop: -1.0 };
            }
        }
        let mut k = 0;
        for col in 0..cols {
            for row in 0..rows {
                let who = list[k];
                k += 1;
                self.grid[r0 + row][c0 + col] = Win { who, alive: !matches!(who, Who::Empty | Who::Decoy(_)), open: 0.0, pop: -1.0 };
            }
        }
        self.phase = Phase_::Opening(0.0);
        c.sfx_ex(Sfx::Swap, 0.5, 0.9);
    }

    fn pos(c: &Ctx, col: usize, row: usize) -> V2 {
        v2(c.center_x() + 0.15 * STAGE_W * (col as f32 - 2.5), c.h * 0.5 + 0.17 * c.h * (row as f32 - 1.5) + 0.06 * c.h)
    }

    fn pop_at(&mut self, c: &mut Ctx, col: usize, row: usize, score: bool) {
        let w = &mut self.grid[row][col];
        w.alive = false;
        w.pop = 0.0;
        if score {
            let p = Self::pos(c, col, row);
            c.add_score(1, p - v2(0.0, 70.0));
            c.fx.burst(p, hex(0x7D8B99), 10, 360.0);
        }
    }

    fn any_bandits(&self) -> bool {
        self.grid.iter().flatten().any(|w| w.alive)
    }
}

impl MiniGame for Bandit {
    fn idle(&mut self, _c: &mut Ctx, dt: f32) {
        if let Phase_::Opening(t) = self.phase {
            self.phase = Phase_::Opening(t + dt);
        }
    }

    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.hint += dt;
        for w in self.grid.iter_mut().flatten() {
            if w.pop >= 0.0 {
                w.pop += dt;
            }
        }
        if let Some((_, t)) = &mut self.wrong {
            *t += dt;
            if *t > 0.6 {
                self.wrong = None;
            }
        }
        match self.phase {
            Phase_::Opening(t) => {
                if t + dt >= 0.13 {
                    self.phase = Phase_::Play;
                } else {
                    self.phase = Phase_::Opening(t + dt);
                }
            }
            Phase_::Closing(t, cleared) => {
                let t = t + dt;
                if t >= if cleared { 0.18 } else { 0.5 } {
                    if cleared {
                        let bonus = if self.hopper_grid { 3.0 } else { 1.0 };
                        c.add_time(bonus, v2(c.center_x(), c.h * 0.2 + 40.0));
                        self.record += 1;
                    }
                    self.generate(c);
                } else {
                    self.phase = Phase_::Closing(t, cleared);
                }
            }
            Phase_::Play => {}
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.phase != Phase_::Play {
            return;
        }
        let mut hit = None;
        for row in 0..ROWS {
            for col in 0..COLS {
                let p = Self::pos(c, col, row);
                if (t.pos.x - p.x).abs() < 80.0 && (t.pos.y - p.y).abs() < 66.0 {
                    hit = Some((col, row));
                }
            }
        }
        let Some((col, row)) = hit else { return };
        let w = self.grid[row][col];
        match w.who {
            Who::Empty | Who::Decoy(255) => {}
            Who::Decoy(_) => {
                let p = Self::pos(c, col, row);
                self.wrong = Some((p, 0.0));
                c.mistake(p);
                self.phase = Phase_::Closing(0.0, false);
            }
            _ if !w.alive => {}
            Who::Hopper => {
                let empties: Vec<(usize, usize)> = (0..ROWS).flat_map(|r| (0..COLS).map(move |cc| (cc, r))).filter(|&(cc, r)| self.grid[r][cc].who == Who::Empty).collect();
                if empties.is_empty() {
                    self.pop_at(c, col, row, true);
                    c.sfx_var(Sfx::Pop);
                } else {
                    let (nc, nr) = empties[c.rng.below(empties.len())];
                    self.pop_at(c, col, row, false);
                    self.grid[row][col].who = Who::Decoy(255); // the dead spot stays closed
                    self.grid[nr][nc] = Win { who: Who::Hopper, alive: true, open: 0.0, pop: -1.0 };
                    c.sfx_ex(Sfx::Jump, 0.8, 1.2);
                }
            }
            Who::Hat => {
                self.pop_at(c, col, row, true);
                c.sfx(Sfx::Burst);
                let neigh = [(col as i32 - 1, row as i32), (col as i32 + 1, row as i32), (col as i32, row as i32 - 1), (col as i32, row as i32 + 1)];
                for (nc, nr) in neigh {
                    if (0..COLS as i32).contains(&nc) && (0..ROWS as i32).contains(&nr) {
                        let n = self.grid[nr as usize][nc as usize];
                        if n.who == Who::Hat && n.alive {
                            self.pop_at(c, nc as usize, nr as usize, true);
                        }
                    }
                }
            }
            Who::Sombrero => {
                self.pop_at(c, col, row, true);
                c.sfx_var(Sfx::Pop);
                for dr in -1i32..=1 {
                    for dc in -1i32..=1 {
                        let (nc, nr) = (col as i32 + dc, row as i32 + dr);
                        if (0..COLS as i32).contains(&nc) && (0..ROWS as i32).contains(&nr) && self.grid[nr as usize][nc as usize].who == Who::Empty {
                            self.grid[nr as usize][nc as usize] = Win { who: Who::Helper, alive: true, open: 0.0, pop: -1.0 };
                        }
                    }
                }
                c.sfx_ex(Sfx::Swap, 0.6, 0.8);
            }
            Who::Boss => {
                let guards = self.grid.iter().flatten().any(|g| g.who == Who::Guard && g.alive);
                if guards {
                    c.sfx_ex(Sfx::Wrong, 0.5, 1.4);
                    self.hint = 0.0;
                } else {
                    self.pop_at(c, col, row, true);
                    c.sfx(Sfx::Burst);
                }
            }
            _ => {
                self.pop_at(c, col, row, true);
                c.sfx_var(Sfx::Pop);
            }
        }
        c.tick();
        if self.phase == Phase_::Play && !self.any_bandits() {
            self.phase = Phase_::Closing(0.0, true);
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        art::sky(r, hex(0x9ED8FF), hex(0xE3F6FF));
        // building facade
        let left = Self::pos(c, 0, 0).x - 130.0;
        let right = Self::pos(c, COLS - 1, 0).x + 130.0;
        let top = Self::pos(c, 0, 0).y - 110.0;
        r.rrect((left + right) * 0.5, (top + h) * 0.5 + 20.0, right - left, h - top + 40.0, 20.0, hex(0xF2C38B), art::INK, 5.0);
        for i in 0..14 {
            let y = top + 20.0 + i as f32 * 44.0;
            r.rect(left + 6.0, y, right - left - 12.0, 3.0, hex(0xD9A066).a(0.6));
        }
        r.rrect((left + right) * 0.5, top, right - left + 40.0, 30.0, 10.0, hex(0xC0262D), art::INK, 4.0);
        let _ = w;
        let closing = match self.phase {
            Phase_::Closing(t, _) => (t / 0.13).min(1.0),
            Phase_::Opening(t) => 1.0 - (t / 0.13).min(1.0),
            Phase_::Play => 0.0,
        };
        for row in 0..ROWS {
            for col in 0..COLS {
                let p = Self::pos(c, col, row);
                let win = self.grid[row][col];
                let occupied = !matches!(win.who, Who::Empty | Who::Decoy(255));
                let open = if occupied { 1.0 - closing } else { 0.0 };
                // interior
                r.rrect(p.x, p.y, 132.0, 112.0, 10.0, hex(0x3A2A4E), art::INK, 4.0);
                if open > 0.05 {
                    r.push();
                    r.fade(open);
                    match win.who {
                        Who::Decoy(d) => draw_decoy(r, p, d, c.anim),
                        Who::Empty => {}
                        who => {
                            let shake = if who == Who::Guard && self.hint < 0.5 { (self.hint * 40.0).sin() * 6.0 } else { 0.0 };
                            if win.alive {
                                draw_bandit(r, p + v2(shake, 0.0), who, c.anim);
                            } else if win.pop >= 0.0 && win.pop < 0.25 {
                                r.push();
                                r.fade(1.0 - win.pop / 0.25);
                                draw_bandit(r, p, who, c.anim);
                                r.pop();
                            }
                        }
                    }
                    r.pop();
                }
                // shutters swing closed over the window
                let shut = 1.0 - open;
                let sw = 66.0 * shut.max(0.0);
                if sw > 1.0 {
                    r.rrect(p.x - 66.0 + sw * 0.5, p.y, sw, 112.0, 6.0, hex(0x4FA7F2), art::INK, 3.0);
                    r.rrect(p.x + 66.0 - sw * 0.5, p.y, sw, 112.0, 6.0, hex(0x4FA7F2), art::INK, 3.0);
                }
                r.rrect(p.x, p.y + 64.0, 150.0, 16.0, 6.0, hex(0xE8DCC8), art::INK, 3.0);
            }
        }
        if let Some((p, t)) = self.wrong {
            let a = (t / 0.6 * std::f32::consts::PI).sin();
            r.text_bold("X", p.x, p.y, 120.0, Align::Center, hex(0xFF5D5D).a(a), art::INK.a(a));
        }
    }
}

fn draw_bandit(r: &mut Renderer, p: V2, who: Who, t: f32) {
    let skin: Skin = match who {
        Who::Plain => art::TANGERINE,
        Who::Hopper => art::SLATE,
        Who::Hat => art::EMERALD,
        Who::Sombrero => art::RUBY,
        Who::Helper => art::SKY,
        Who::Guard => art::SKY,
        _ => art::GRAPE,
    };
    let pose = Pose { look: v2((t * 1.3 + p.x).sin() * 0.8, 0.2), ..Pose::idle(t, p.x) };
    art::frog(r, p.x, p.y + 6.0, 96.0, skin, &pose);
    // bandit mask
    r.rrect(p.x, p.y - 14.0, 92.0, 18.0, 9.0, art::INK.a(0.85), Col::CLEAR, 0.0);
    match who {
        Who::Hat => {
            r.ellipse(p.x, p.y - 40.0, 54.0, 10.0, hex(0x9A6BEA), art::INK, 3.0);
            r.rrect(p.x, p.y - 56.0, 52.0, 30.0, 8.0, hex(0x9A6BEA), art::INK, 3.0);
        }
        Who::Sombrero | Who::Helper => {
            let col = if who == Who::Sombrero { hex(0xFFD23F) } else { hex(0xFF9F2E) };
            r.ellipse(p.x, p.y - 38.0, 70.0, 14.0, col, art::INK, 3.0);
            r.ellipse(p.x, p.y - 52.0, 28.0, 22.0, col, art::INK, 3.0);
        }
        Who::Boss => {
            r.ellipse(p.x, p.y - 40.0, 54.0, 10.0, art::INK, art::INK, 0.0);
            r.rrect(p.x, p.y - 58.0, 46.0, 34.0, 6.0, art::INK, art::INK, 0.0);
            r.rect(p.x - 23.0, p.y - 50.0, 46.0, 6.0, hex(0xF0605D));
        }
        Who::Guard => {
            r.rrect(p.x, p.y - 42.0, 70.0, 14.0, 7.0, hex(0x1F5FBF), art::INK, 3.0);
        }
        Who::Hopper => {
            r.line(v2(p.x - 30.0, p.y - 50.0), v2(p.x - 10.0, p.y - 36.0), 4.0, art::INK);
            r.line(v2(p.x + 30.0, p.y - 50.0), v2(p.x + 10.0, p.y - 36.0), 4.0, art::INK);
        }
        _ => {}
    }
}

fn draw_decoy(r: &mut Renderer, p: V2, d: u8, t: f32) {
    match d % 8 {
        0 => {
            // flower pot
            r.rrect(p.x, p.y + 30.0, 46.0, 36.0, 6.0, hex(0xC9752A), art::INK, 3.0);
            for k in 0..3 {
                let x = p.x - 18.0 + k as f32 * 18.0;
                r.line(v2(x, p.y + 12.0), v2(x, p.y - 12.0), 3.0, hex(0x3D9B6E));
                r.circle(x, p.y - 18.0, 10.0, [hex(0xF27BC4), hex(0xFFD23F), hex(0xF0605D)][k], art::INK, 2.0);
            }
        }
        1 => {
            // teapot
            r.ellipse(p.x, p.y + 16.0, 34.0, 28.0, hex(0x4FA7F2), art::INK, 3.0);
            r.line(v2(p.x + 30.0, p.y + 10.0), v2(p.x + 48.0, p.y - 4.0), 6.0, hex(0x4FA7F2));
            r.disc(p.x, p.y - 14.0, 6.0, art::INK);
            r.ring(p.x - 4.0 + (t * 2.0).sin() * 4.0, p.y - 34.0, 8.0, 2.0, Col::WHITE.a(0.6));
        }
        2 => {
            // curtains
            r.tri(v2(p.x - 62.0, p.y - 52.0), v2(p.x - 10.0, p.y - 52.0), v2(p.x - 62.0, p.y + 50.0), hex(0xF0605D));
            r.tri(v2(p.x + 62.0, p.y - 52.0), v2(p.x + 10.0, p.y - 52.0), v2(p.x + 62.0, p.y + 50.0), hex(0xF0605D));
        }
        3 => {
            // cat napping
            r.ellipse(p.x, p.y + 26.0, 40.0, 20.0, hex(0xB8C2D0), art::INK, 3.0);
            r.circle(p.x - 28.0, p.y + 14.0, 16.0, hex(0xB8C2D0), art::INK, 3.0);
            r.line(v2(p.x - 34.0, p.y + 12.0), v2(p.x - 26.0, p.y + 12.0), 2.0, art::INK);
        }
        4 => {
            // fruit bowl
            r.pie(p.x, p.y + 20.0, 36.0, 0.5, hex(0xE8EDF2));
            art::fruit(r, art::Fruit::Apple, p.x - 14.0, p.y + 8.0, 30.0);
            art::fruit(r, art::Fruit::Orange, p.x + 14.0, p.y + 8.0, 30.0);
        }
        5 => {
            // lamp
            r.line(v2(p.x, p.y + 44.0), v2(p.x, p.y - 10.0), 4.0, art::INK);
            r.tri(v2(p.x - 26.0, p.y - 4.0), v2(p.x + 26.0, p.y - 4.0), v2(p.x, p.y - 40.0), hex(0xFFE27A));
            r.glow(p.x, p.y - 10.0, 50.0, hex(0xFFE27A).a(0.4));
        }
        6 => {
            // cactus
            r.rrect(p.x, p.y + 8.0, 22.0, 70.0, 11.0, hex(0x5DBB55), art::INK, 3.0);
            r.rrect(p.x - 20.0, p.y, 14.0, 30.0, 7.0, hex(0x5DBB55), art::INK, 3.0);
        }
        _ => {
            // book stack
            for k in 0..3 {
                r.rrect(p.x + (k as f32 - 1.0) * 4.0, p.y + 34.0 - k as f32 * 16.0, 60.0, 14.0, 3.0, [hex(0x9A6BEA), hex(0x4CC38A), hex(0xFF9F2E)][k], art::INK, 2.0);
            }
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rrect(x, y, s * 0.6, s * 0.55, 8.0, hex(0x3A2A4E), art::INK, 3.0);
    r.push();
    r.translate(x, y);
    r.scale(s / 160.0, s / 160.0);
    draw_bandit(r, v2(0.0, 0.0), Who::Plain, t);
    r.pop();
}
