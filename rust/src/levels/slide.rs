//! Fruit Slide: swipe a frog and it slides like on ice until it hits the edge or something
//! in its way. Sliding into fruit of its own colour eats it (+1). Clear the board for +2 s.
//! Boards are generated randomly and kept only if a solver finds a solution.
use super::{Ctx, MiniGame, STAGE_W};
use crate::art::{self, Fruit, Pose, Skin};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;
use std::collections::{HashSet, VecDeque};

const COLS: i32 = 5;
const ROWS: i32 = 3;
const SKINS: [Skin; 5] = [art::SKY, art::EMERALD, art::TANGERINE, art::GRAPE, art::LEMON];
const FRUITS: [Fruit; 5] = [Fruit::Plum, Fruit::Apple, Fruit::Orange, Fruit::Grapes, Fruit::Lemon];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Cell {
    Empty,
    Frog(u8),
    Fruit(u8),
}

type Board = [[Cell; COLS as usize]; ROWS as usize];

/// Ice-slide from (c, r) in direction (dx, dy). Returns the destination and whether fruit was eaten.
fn slide(b: &Board, c: i32, r: i32, dx: i32, dy: i32) -> Option<((i32, i32), bool)> {
    let Cell::Frog(col) = b[r as usize][c as usize] else { return None };
    let (mut x, mut y) = (c, r);
    loop {
        let (nx, ny) = (x + dx, y + dy);
        if !(0..COLS).contains(&nx) || !(0..ROWS).contains(&ny) {
            break;
        }
        match b[ny as usize][nx as usize] {
            Cell::Empty => {
                x = nx;
                y = ny;
            }
            Cell::Fruit(fc) if fc == col => return Some(((nx, ny), true)),
            _ => break,
        }
    }
    if (x, y) == (c, r) {
        None
    } else {
        Some(((x, y), false))
    }
}

fn apply(b: &Board, from: (i32, i32), to: (i32, i32)) -> Board {
    let mut nb = *b;
    nb[to.1 as usize][to.0 as usize] = nb[from.1 as usize][from.0 as usize];
    nb[from.1 as usize][from.0 as usize] = Cell::Empty;
    nb
}

fn solvable(b: &Board, max_depth: usize) -> bool {
    let done = |b: &Board| !b.iter().flatten().any(|c| matches!(c, Cell::Fruit(_)));
    let mut seen: HashSet<Board> = HashSet::new();
    let mut q = VecDeque::new();
    q.push_back((*b, 0usize));
    seen.insert(*b);
    while let Some((s, d)) = q.pop_front() {
        if done(&s) {
            return true;
        }
        if d >= max_depth || seen.len() > 6000 {
            continue;
        }
        for r in 0..ROWS {
            for c in 0..COLS {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    if let Some((to, _)) = slide(&s, c, r, dx, dy) {
                        let ns = apply(&s, (c, r), to);
                        if seen.insert(ns) {
                            q.push_back((ns, d + 1));
                        }
                    }
                }
            }
        }
    }
    false
}

pub struct Slide {
    board: Board,
    level: usize,
    anim: Option<((i32, i32), (i32, i32), f32)>,
    selected: Option<(i32, i32, V2, i32)>,
    appear: f32,
    eaten: Vec<(V2, u8, f32)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Slide { board: [[Cell::Empty; COLS as usize]; ROWS as usize], level: 0, anim: None, selected: None, appear: 0.0, eaten: Vec::new() };
    g.new_board(c);
    Box::new(g)
}

impl Slide {
    fn cell_pos(c: &Ctx, col: f32, row: f32) -> V2 {
        let cw = 0.12 * STAGE_W;
        let ch = 0.21 * c.h;
        v2(c.center_x() - 2.0 * cw + col * cw, c.h * 0.53 - ch + row * ch)
    }
    fn new_board(&mut self, c: &mut Ctx) {
        let pairs = match self.level {
            0..=4 => 1,
            5..=8 => 2,
            9..=13 => 3,
            14..=19 => 4,
            _ => 5,
        };
        for _attempt in 0..200 {
            let mut cells: Vec<(i32, i32)> = (0..ROWS).flat_map(|r| (0..COLS).map(move |cc| (cc, r))).collect();
            c.rng.shuffle(&mut cells);
            let mut b: Board = [[Cell::Empty; COLS as usize]; ROWS as usize];
            let mut cols: Vec<u8> = (0..5).collect();
            c.rng.shuffle(&mut cols);
            for i in 0..pairs {
                let (fc, fr) = cells[i * 2];
                let (gc, gr) = cells[i * 2 + 1];
                b[fr as usize][fc as usize] = Cell::Frog(cols[i]);
                b[gr as usize][gc as usize] = Cell::Fruit(cols[i]);
            }
            if solvable(&b, 4 + pairs * 2) {
                self.board = b;
                self.appear = 0.0;
                return;
            }
        }
        // fallback: frogs directly in line with their fruit
        let mut b: Board = [[Cell::Empty; COLS as usize]; ROWS as usize];
        for i in 0..pairs.min(3) {
            b[i][0] = Cell::Frog(i as u8);
            b[i][4] = Cell::Fruit(i as u8);
        }
        self.board = b;
        self.appear = 0.0;
    }
}

impl MiniGame for Slide {
    fn idle(&mut self, _c: &mut Ctx, dt: f32) {
        self.appear += dt;
    }

    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.appear += dt;
        for e in self.eaten.iter_mut() {
            e.2 += dt;
        }
        self.eaten.retain(|e| e.2 < 0.3);
        if let Some((from, to, t)) = &mut self.anim {
            *t += dt;
            if *t >= 0.1 {
                let _ = (from, to);
                self.anim = None;
                if !self.board.iter().flatten().any(|cl| matches!(cl, Cell::Fruit(_))) {
                    c.add_time(2.0, v2(c.center_x(), c.h * 0.2 + 40.0));
                    self.level += 1;
                    if self.level >= 31 {
                        self.level = 20 + c.rng.below(10);
                    }
                    self.new_board(c);
                }
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        match t.phase {
            Phase::Down => {
                if self.anim.is_some() || self.selected.is_some() {
                    return;
                }
                for r in 0..ROWS {
                    for cc in 0..COLS {
                        if matches!(self.board[r as usize][cc as usize], Cell::Frog(_)) && Self::cell_pos(c, cc as f32, r as f32).dist(t.pos) < 75.0 {
                            self.selected = Some((cc, r, t.pos, t.id));
                            c.sfx_ex(Sfx::Tick, 0.5, 1.3);
                            return;
                        }
                    }
                }
            }
            Phase::Move => {
                let Some((cc, r, start, id)) = self.selected else { return };
                if id != t.id || self.anim.is_some() {
                    return;
                }
                let d = t.pos - start;
                if d.x.abs() < 22.0 && d.y.abs() < 22.0 {
                    return;
                }
                let (dx, dy) = if d.x.abs() >= d.y.abs() { (d.x.signum() as i32, 0) } else { (0, d.y.signum() as i32) };
                self.selected = None;
                if let Some((to, ate)) = slide(&self.board, cc, r, dx, dy) {
                    let Cell::Frog(col) = self.board[r as usize][cc as usize] else { return };
                    self.board = apply(&self.board, (cc, r), to);
                    self.anim = Some(((cc, r), to, 0.0));
                    c.sfx_ex(Sfx::Whoosh, 0.5, 1.5);
                    if ate {
                        let p = Self::cell_pos(c, to.0 as f32, to.1 as f32);
                        self.eaten.push((p, col, 0.0));
                        c.add_score(1, p - v2(0.0, 60.0));
                        c.sfx_var(Sfx::Eat);
                        c.tick();
                    }
                } else {
                    c.sfx_ex(Sfx::Thud, 0.4, 1.4);
                }
            }
            Phase::Up | Phase::Cancel => {
                if self.selected.map(|s| s.3) == Some(t.id) {
                    self.selected = None;
                }
            }
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0xF6E7C1), hex(0xE5C88C));
        let cw = 0.12 * STAGE_W;
        let ch = 0.21 * c.h;
        let tl = Self::cell_pos(c, -0.5, -0.5);
        r.rrect(tl.x + cw * 2.5, tl.y + ch * 1.5, cw * 5.0 + 24.0, ch * 3.0 + 24.0, 20.0, hex(0xC9A060), art::INK, 5.0);
        for rr in 0..ROWS {
            for cc in 0..COLS {
                let p = Self::cell_pos(c, cc as f32, rr as f32);
                r.rrect(p.x, p.y, cw - 8.0, ch - 8.0, 12.0, hex(0xF3DDB0), hex(0xB08850), 2.0);
            }
        }
        let k = if self.appear < 0.2 { 0.0 } else { ease_out_back(((self.appear - 0.2) / 0.2).min(1.0)) };
        for rr in 0..ROWS {
            for cc in 0..COLS {
                let p = Self::cell_pos(c, cc as f32, rr as f32);
                match self.board[rr as usize][cc as usize] {
                    Cell::Fruit(f) => art::fruit(r, FRUITS[f as usize], p.x, p.y, 92.0 * k),
                    Cell::Frog(f) => {
                        let mut pos = p;
                        if let Some((from, to, t)) = self.anim {
                            if to == (cc, rr) {
                                let a = Self::cell_pos(c, from.0 as f32, from.1 as f32);
                                pos = a.lerp(p, (t / 0.1).min(1.0));
                            }
                        }
                        let sel = self.selected.is_some_and(|s| (s.0, s.1) == (cc, rr));
                        if sel {
                            r.glow(pos.x, pos.y, 90.0, Col::WHITE.a(0.8));
                        }
                        art::frog(r, pos.x, pos.y - 6.0, 108.0 * k, SKINS[f as usize], &Pose::idle(c.anim, (cc + rr * 5) as f32));
                    }
                    Cell::Empty => {}
                }
            }
        }
        for (p, col, t) in &self.eaten {
            r.ring(p.x, p.y, 40.0 + t * 200.0, 6.0 * (1.0 - t / 0.3), SKINS[*col as usize].body);
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    let k = (t * 0.8).fract();
    art::fruit(r, Fruit::Lemon, x + s * 0.3, y, s * 0.24);
    art::frog(r, x - s * 0.3 + k * s * 0.4, y, s * 0.3, art::LEMON, &Pose::idle(t, 0.0));
}
