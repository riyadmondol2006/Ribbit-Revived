//! Shape Sort: the frog holds up a toy; tap its matching silhouette on the shelves. Fill the
//! whole cupboard to score every toy on it and gain +2 s. A wrong tap reshuffles the board.
//! Boards grow from 2 to 10 slots as in the classic progression.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};
use crate::toys;

/// Toy ids in "pool order"; index 6 is the snowman (first seen around board 15).
const POOL: [usize; 12] = [13, 11, 3, 2, 18, 15, toys::SNOWMAN, 1, 6, 17, 0, 9];
/// Slot positions as fractions of the cupboard (x from left, y up from the bottom).
const SLOTS: [(f32, f32); 10] = [(0.40, 0.265), (0.60, 0.265), (0.50, 0.455), (0.50, 0.075), (0.30, 0.455), (0.70, 0.455), (0.30, 0.075), (0.70, 0.075), (0.22, 0.265), (0.78, 0.265)];
const CUP_W: f32 = 820.0;
const CUP_H: f32 = 500.0;

struct Slot {
    pos: V2,
    toy: usize,
    filled: f32,
}

pub struct Sort {
    slots: Vec<Slot>,
    needed: Vec<usize>,
    current: Option<usize>,
    board: usize,
    mistake: bool,
    appear: f32,
    cur_t: f32,
    wrong: Option<(V2, f32)>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Sort { slots: Vec::new(), needed: Vec::new(), current: None, board: 0, mistake: false, appear: 0.0, cur_t: 0.0, wrong: None };
    g.init_board(c);
    Box::new(g)
}

impl Sort {
    fn cup_rect(c: &Ctx) -> (f32, f32) {
        (c.center_x() - CUP_W * 0.5, c.h - CUP_H)
    }
    fn init_board(&mut self, c: &mut Ctx) {
        let b = self.board;
        let n = (2 + b * 8 / 36).min(10);
        let pool_hi = (3 + b * 9 / 40).clamp(n, 12);
        let pool_lo = pool_hi.saturating_sub(n + 2 + b / 20);
        let mut pool: Vec<usize> = (pool_lo..pool_hi).collect();
        c.rng.shuffle(&mut pool);
        let mut slot_ids: Vec<usize> = (0..10).collect();
        c.rng.shuffle(&mut slot_ids);
        let (cx, top) = Self::cup_rect(c);
        self.slots = (0..n)
            .map(|i| {
                let (fx, fy) = SLOTS[slot_ids[i]];
                Slot { pos: v2(cx + fx * CUP_W, top + CUP_H - fy * CUP_H - 46.0), toy: POOL[pool[i]], filled: -1.0 }
            })
            .collect();
        self.needed = self.slots.iter().map(|s| s.toy).collect();
        self.appear = 0.0;
        self.next_toy(c);
    }
    fn next_toy(&mut self, c: &mut Ctx) {
        if self.needed.is_empty() {
            self.current = None;
            return;
        }
        let i = c.rng.below(self.needed.len());
        self.current = Some(self.needed.remove(i));
        self.cur_t = 0.0;
    }
}

impl MiniGame for Sort {
    fn idle(&mut self, _c: &mut Ctx, dt: f32) {
        self.appear += dt;
        self.cur_t += dt;
    }

    fn update(&mut self, _c: &mut Ctx, dt: f32) {
        self.appear += dt;
        self.cur_t += dt;
        for s in self.slots.iter_mut() {
            if s.filled >= 0.0 {
                s.filled += dt;
            }
        }
        if let Some((_, t)) = &mut self.wrong {
            *t += dt;
            if *t > 0.6 {
                self.wrong = None;
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.appear < 0.2 {
            return;
        }
        let Some(cur) = self.current else { return };
        let Some(i) = self.slots.iter().position(|s| s.filled < 0.0 && s.pos.dist(t.pos) < 70.0) else { return };
        if self.slots[i].toy == cur {
            self.slots[i].filled = 0.0;
            c.sfx_var(Sfx::Thud);
            c.tick();
            c.fx.stars(self.slots[i].pos, 5);
            if cur == toys::SNOWMAN && !self.mistake {
                c.special_hit = true;
            }
            if self.slots.iter().all(|s| s.filled >= 0.0) {
                let n = self.slots.len() as i64;
                c.add_score(n, v2(c.center_x(), c.h * 0.42));
                c.add_time(2.0, v2(c.center_x(), c.h * 0.32));
                self.board += 1;
                if self.board >= 76 {
                    self.board = 66 + c.rng.below(10);
                }
                self.init_board(c);
            } else {
                self.next_toy(c);
            }
        } else {
            self.mistake = true;
            self.wrong = Some((self.slots[i].pos, 0.0));
            c.mistake(self.slots[i].pos);
            self.init_board(c);
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0xFFE9D2), hex(0xF2C7A0));
        // wallpaper stripes
        for i in 0..((w / 70.0) as i32 + 1) {
            r.rect(i as f32 * 70.0, 0.0, 26.0, h, Col::WHITE.a(0.18));
        }
        let (cx0, top) = Self::cup_rect(c);
        // cupboard
        r.rrect(cx0 + CUP_W * 0.5, top + CUP_H * 0.5 + 10.0, CUP_W + 30.0, CUP_H + 20.0, 24.0, hex(0x8A5A34), art::INK, 5.0);
        r.rrect(cx0 + CUP_W * 0.5, top + CUP_H * 0.5 + 16.0, CUP_W - 20.0, CUP_H - 20.0, 16.0, hex(0xF3D5AC), art::INK, 4.0);
        for fy in [0.075f32, 0.265, 0.455] {
            let y = top + CUP_H - fy * CUP_H + 2.0;
            r.rrect(cx0 + CUP_W * 0.5, y, CUP_W - 20.0, 14.0, 4.0, hex(0xB07A45), art::INK, 3.0);
        }
        r.rrect(cx0 + CUP_W * 0.5, top + 4.0, CUP_W + 60.0, 26.0, 10.0, hex(0xA86E3F), art::INK, 4.0);
        // silhouettes and placed toys
        let k = ease_out_back((self.appear / 0.3).min(1.0));
        for s in &self.slots {
            if s.filled >= 0.0 {
                let p = ease_out_back((s.filled / 0.15).min(1.0));
                toys::toy(r, s.toy, s.pos.x, s.pos.y, 80.0 * p, None);
            } else {
                toys::toy(r, s.toy, s.pos.x, s.pos.y, 80.0 * k, Some(hex(0x6B4A2B).a(0.75)));
            }
        }
        if let Some((p, t)) = self.wrong {
            let a = (t / 0.6 * std::f32::consts::PI).sin();
            r.text_bold("X", p.x, p.y, 110.0, Align::Center, hex(0xFF5D5D).a(a), art::INK.a(a));
        }
        // frog presenting the current toy on top of the cupboard
        let fp = v2(c.center_x() - 60.0, top - 46.0);
        art::frog(r, fp.x, fp.y, 110.0, art::EMERALD, &Pose { look: v2(0.8, 0.3), ..Pose::idle(c.anim, 0.0) });
        if let Some(cur) = self.current {
            let s = ease_out_back((self.cur_t / 0.3).min(1.0));
            r.glow(c.center_x() + 70.0, top - 50.0, 80.0, Col::WHITE.a(0.7));
            toys::toy(r, cur, c.center_x() + 70.0, top - 50.0, 84.0 * s, None);
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, _t: f32) {
    r.rrect(x, y + s * 0.1, s * 0.9, s * 0.6, 10.0, hex(0xF3D5AC), art::INK, 3.0);
    toys::toy(r, 11, x - s * 0.22, y + s * 0.1, s * 0.25, Some(hex(0x6B4A2B).a(0.75)));
    toys::toy(r, 2, x + s * 0.18, y + s * 0.1, s * 0.25, None);
    toys::toy(r, 11, x, y - s * 0.35, s * 0.25, None);
}
