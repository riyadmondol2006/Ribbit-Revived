//! Link Up: drag through neighbouring frogs of the same colour (diagonals allowed, slide back
//! to undo). Release a chain of 3+ to score its length, gain +1 s and get a fresh board.
//! Each link plays the next note of the scale.
use super::{Ctx, MiniGame, STAGE_W};
use crate::art::{self, Pose, Skin};
use crate::audio::{self, Sfx};
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const COLORS: [Skin; 7] = [art::GRAPE, art::BLOSSOM, art::LEMON, art::TANGERINE, art::EMERALD, art::SKY, art::SLATE];
const COLS: usize = 6;
const ROWS: usize = 4;

pub struct Connect {
    grid: [[Option<usize>; COLS]; ROWS],
    chain: Vec<(usize, usize)>,
    pointer: Option<i32>,
    board: usize,
    appear: f32,
    splash: Vec<(V2, Col, f32)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Connect { grid: [[None; COLS]; ROWS], chain: Vec::new(), pointer: None, board: 0, appear: 0.0, splash: Vec::new() };
    g.new_board(c);
    Box::new(g)
}

impl Connect {
    fn cell_pos(c: &Ctx, col: usize, row: usize) -> V2 {
        v2(c.center_x() + (col as f32 - 2.5) * 0.11 * STAGE_W, c.h * 0.26 + row as f32 * 0.18 * c.h)
    }
    fn new_board(&mut self, c: &mut Ctx) {
        let b = self.board;
        let (frogs, colors) = match b {
            0..=4 => (8, 2),
            5..=24 => (10 + (b - 5) * 14 / 20, 3 + (b - 5) / 8),
            25..=44 => (16 + c.rng.below(9), 4 + c.rng.below(4)),
            _ => (18 + c.rng.below(7), 5 + c.rng.below(3)),
        };
        let frogs = frogs.min(24);
        let colors = colors.min(7);
        let mut palette: Vec<usize> = (0..7).collect();
        c.rng.shuffle(&mut palette);
        self.grid = [[None; COLS]; ROWS];
        // carve one long same-colour snake so a good chain always exists
        let snake_len = (frogs / colors).clamp(3, 9) + c.rng.below(3);
        let mut col = c.rng.below(COLS) as i32;
        let mut row = c.rng.below(ROWS) as i32;
        let mut placed = 0;
        for _ in 0..snake_len {
            if self.grid[row as usize][col as usize].is_none() {
                self.grid[row as usize][col as usize] = Some(palette[0]);
                placed += 1;
            }
            let mut opts = Vec::new();
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (col + dx, row + dy);
                    if (dx != 0 || dy != 0) && (0..COLS as i32).contains(&nx) && (0..ROWS as i32).contains(&ny) && self.grid[ny as usize][nx as usize].is_none() {
                        opts.push((nx, ny));
                    }
                }
            }
            if opts.is_empty() {
                break;
            }
            let (nx, ny) = opts[c.rng.below(opts.len())];
            col = nx;
            row = ny;
        }
        // fill the rest with the other colours
        let mut cells: Vec<(usize, usize)> = (0..ROWS).flat_map(|r| (0..COLS).map(move |cc| (cc, r))).filter(|&(cc, r)| self.grid[r][cc].is_none()).collect();
        c.rng.shuffle(&mut cells);
        for (k, (cc, r)) in cells.into_iter().enumerate() {
            if placed >= frogs {
                break;
            }
            let colr = if colors > 1 { palette[1 + k % (colors - 1)] } else { palette[0] };
            self.grid[r][cc] = Some(colr);
            placed += 1;
        }
        self.chain.clear();
        self.appear = 0.0;
    }
    fn cell_at(&self, c: &Ctx, p: V2) -> Option<(usize, usize)> {
        for r in 0..ROWS {
            for cc in 0..COLS {
                if self.grid[r][cc].is_some() && Self::cell_pos(c, cc, r).dist(p) < 62.0 {
                    return Some((cc, r));
                }
            }
        }
        None
    }
    fn note(&self, c: &mut Ctx, len: usize) {
        let pitch = audio::SCALE[len.min(7)];
        c.sfx_ex(Sfx::Note, 0.8, pitch);
    }
}

impl MiniGame for Connect {
    fn idle(&mut self, _c: &mut Ctx, dt: f32) {
        self.appear += dt;
    }

    fn update(&mut self, _c: &mut Ctx, dt: f32) {
        self.appear += dt;
        for s in self.splash.iter_mut() {
            s.2 += dt;
        }
        self.splash.retain(|s| s.2 < 0.4);
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        match t.phase {
            Phase::Down => {
                if self.pointer.is_none() {
                    if let Some(cell) = self.cell_at(c, t.pos) {
                        self.pointer = Some(t.id);
                        self.chain = vec![cell];
                        self.note(c, 0);
                        c.tick();
                    }
                }
            }
            Phase::Move => {
                if self.pointer != Some(t.id) {
                    return;
                }
                let Some(cell) = self.cell_at(c, t.pos) else { return };
                let last = *self.chain.last().unwrap();
                if cell == last {
                    return;
                }
                if self.chain.len() >= 2 && self.chain[self.chain.len() - 2] == cell {
                    let len = self.chain.len();
                    self.chain.pop();
                    self.note(c, len - 1);
                    return;
                }
                let colr = self.grid[last.1][last.0];
                let adjacent = (cell.0 as i32 - last.0 as i32).abs() <= 1 && (cell.1 as i32 - last.1 as i32).abs() <= 1;
                if adjacent && self.grid[cell.1][cell.0] == colr && !self.chain.contains(&cell) {
                    let len = self.chain.len();
                    self.chain.push(cell);
                    self.note(c, len);
                    c.tick();
                }
            }
            Phase::Up | Phase::Cancel => {
                if self.pointer != Some(t.id) {
                    return;
                }
                self.pointer = None;
                if self.chain.len() >= 3 && t.phase == Phase::Up {
                    let n = self.chain.len() as i64;
                    let colr = self.grid[self.chain[0].1][self.chain[0].0].unwrap_or(0);
                    for &(cc, r) in &self.chain {
                        let p = Self::cell_pos(c, cc, r);
                        self.splash.push((p, COLORS[colr].body, 0.0));
                        c.fx.burst(p, COLORS[colr].body, 6, 260.0);
                    }
                    let mid = Self::cell_pos(c, self.chain[n as usize / 2].0, self.chain[n as usize / 2].1);
                    c.add_score(n, mid - v2(0.0, 70.0));
                    c.add_time(1.0, v2(c.center_x(), c.h * 0.15 + 40.0));
                    c.sfx(Sfx::Collect);
                    self.board += 1;
                    self.new_board(c);
                } else {
                    self.chain.clear();
                }
            }
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0xD9A86A), hex(0xB07A45));
        for i in 0..((h / 60.0) as i32 + 1) {
            r.rect(0.0, i as f32 * 60.0, w, 2.0, hex(0x8A5A34).a(0.35));
        }
        let k = ease_out_back((self.appear / 0.3).min(1.0));
        // chain glow + connectors
        if let Some(&(c0, r0)) = self.chain.first() {
            if let Some(colr) = self.grid[r0][c0] {
                let col = COLORS[colr].body;
                for &(cc, rr) in &self.chain {
                    let p = Self::cell_pos(c, cc, rr);
                    r.circle(p.x, p.y, 66.0, col.a(0.45), col.shade(-0.3), 4.0);
                }
                for win in self.chain.windows(2) {
                    let a = Self::cell_pos(c, win[0].0, win[0].1);
                    let b = Self::cell_pos(c, win[1].0, win[1].1);
                    r.line_outlined(a, b, 22.0, col, art::INK, 4.0);
                }
            }
        }
        for s in &self.splash {
            let a = 1.0 - s.2 / 0.4;
            for j in 0..7 {
                let ang = j as f32 * 0.9;
                r.disc(s.0.x + ang.cos() * 50.0 * (1.0 + s.2 * 2.0), s.0.y + ang.sin() * 50.0 * (1.0 + s.2 * 2.0), 16.0 * a, s.1.a(a));
            }
        }
        for rr in 0..ROWS {
            for cc in 0..COLS {
                let Some(colr) = self.grid[rr][cc] else { continue };
                let p = Self::cell_pos(c, cc, rr);
                let in_chain = self.chain.contains(&(cc, rr));
                let pose = Pose { happy_eyes: in_chain, mouth: if in_chain { 0.4 } else { 0.0 }, ..Pose::idle(c.anim, (rr * 6 + cc) as f32) };
                art::frog(r, p.x, p.y - 4.0, 104.0 * k * if in_chain { 1.08 } else { 1.0 }, COLORS[colr], &pose);
            }
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    let pts = [v2(x - s * 0.3, y + s * 0.15), v2(x, y - s * 0.12), v2(x + s * 0.3, y + s * 0.15)];
    r.line_outlined(pts[0], pts[1], s * 0.06, art::SKY.body, art::INK, 2.0);
    r.line_outlined(pts[1], pts[2], s * 0.06, art::SKY.body, art::INK, 2.0);
    for (i, p) in pts.iter().enumerate() {
        art::frog(r, p.x, p.y, s * 0.3, art::SKY, &Pose::idle(t, i as f32));
    }
}
