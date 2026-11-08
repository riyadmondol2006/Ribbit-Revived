//! Twin Pop: tap a frog to select it, then tap its same-coloured twin to pop both.
//! Holding one frog and tapping the twin with a second finger works too.
//! Boards are generated with the classic progression (1 colour -> up to 8 colours, 2..24 frogs);
//! clearing a board adds +1 s.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose, Skin};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const SKINS: [Skin; 8] = [art::EMERALD, art::SKY, art::BLOSSOM, art::LEMON, art::TANGERINE, art::Skin { body: hex(0x9BE15D), belly: hex(0xE6FFC2), line: hex(0x2F5E14) }, art::RUBY, art::GRAPE];

struct Frog {
    pos: V2,
    color: usize,
    alive: bool,
    pop_t: f32,
    seed: f32,
}

pub struct Twin {
    frogs: Vec<Frog>,
    board: usize,
    appear: f32,
    selected: Option<usize>,
    holder: Option<i32>,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Twin { frogs: Vec::new(), board: 0, appear: 0.0, selected: None, holder: None };
    g.new_board(c);
    Box::new(g)
}

impl Twin {
    fn new_board(&mut self, c: &mut Ctx) {
        let b = self.board;
        let (colors, count) = match b {
            0..=3 => (1, 2 + 2 * c.rng.below(2)),
            4..=11 => (2, 4 + 2 * c.rng.below(3)),
            12..=23 => (3, (6 + (b - 12)).min(20) & !1),
            24..=35 => (3 + c.rng.below(4), (12 + (b - 24) / 2 * 2).min(20)),
            _ => (5 + c.rng.below(4), 20 + 2 * c.rng.below(3)),
        };
        let colors = colors.min(count / 2).max(1);
        // even number of every colour
        let mut list: Vec<usize> = Vec::new();
        let palette = {
            let mut p: Vec<usize> = (0..8).collect();
            c.rng.shuffle(&mut p);
            p
        };
        let pairs = count / 2;
        for i in 0..pairs {
            let col = palette[i % colors];
            list.push(col);
            list.push(col);
        }
        c.rng.shuffle(&mut list);
        // rows of up to 6, centred; each row centred independently like the classic layout
        let per_row = match count {
            0..=4 => count,
            5..=8 => count.div_ceil(2),
            9..=12 => count.div_ceil(2).min(6),
            _ => 6,
        };
        let rows = count.div_ceil(per_row);
        let dx = 0.14 * super::STAGE_W;
        let dy = 0.18 * c.h;
        let cy = c.h * 0.5 + 40.0;
        self.frogs.clear();
        let mut k = 0;
        for r in 0..rows {
            let n = (count - k).min(per_row);
            let y = cy - (rows as f32 - 1.0) * dy * 0.5 + r as f32 * dy;
            for i in 0..n {
                let x = c.center_x() - (n as f32 - 1.0) * dx * 0.5 + i as f32 * dx;
                self.frogs.push(Frog { pos: v2(x, y), color: list[k], alive: true, pop_t: 0.0, seed: c.rng.f() * 10.0 });
                k += 1;
            }
        }
        self.appear = 0.0;
        self.selected = None;
        self.holder = None;
    }

    fn frog_at(&self, p: V2) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, f) in self.frogs.iter().enumerate() {
            if !f.alive {
                continue;
            }
            let d = p.dist(f.pos - v2(0.0, 10.0));
            if d < 80.0 && best.is_none_or(|b| d < b.1) {
                best = Some((i, d));
            }
        }
        best.map(|b| b.0)
    }
}

impl MiniGame for Twin {
    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.appear += dt;
        for f in self.frogs.iter_mut() {
            if !f.alive {
                f.pop_t += dt;
            }
        }
        if self.frogs.iter().all(|f| !f.alive) && self.frogs.iter().all(|f| f.pop_t > 0.15) {
            c.add_time(1.0, v2(c.center_x(), c.h * 0.5));
            self.board += 1;
            self.new_board(c);
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        match t.phase {
            Phase::Down => {
                let hit = self.frog_at(t.pos);
                let sel = self.selected.filter(|&a| self.frogs[a].alive);
                match (sel, hit) {
                    (_, None) => {
                        if self.holder.is_none() {
                            self.selected = None;
                        }
                    }
                    (Some(a), Some(b)) if a == b => {
                        // tapping the selected frog again lets go of it
                        if self.holder.is_none() {
                            self.selected = None;
                        }
                    }
                    (Some(a), Some(b)) if self.frogs[b].color == self.frogs[a].color => {
                        for i in [a, b] {
                            self.frogs[i].alive = false;
                            self.frogs[i].pop_t = 0.0;
                            let col = SKINS[self.frogs[i].color].body;
                            c.fx.burst(self.frogs[i].pos, col, 10, 380.0);
                            c.fx.ring(self.frogs[i].pos, Col::WHITE, 60.0);
                        }
                        let mid = self.frogs[a].pos.lerp(self.frogs[b].pos, 0.5);
                        c.add_score(2, mid - v2(0.0, 60.0));
                        c.sfx_var(Sfx::Pop);
                        c.tick();
                        self.selected = None;
                        self.holder = None;
                    }
                    (Some(_), Some(b)) => {
                        // different colour: switch the selection to the new frog
                        self.selected = Some(b);
                        self.holder = Some(t.id);
                        c.sfx_ex(Sfx::Swap, 0.5, 0.7);
                    }
                    (None, Some(b)) => {
                        self.selected = Some(b);
                        self.holder = Some(t.id);
                        c.sfx_ex(Sfx::Tick, 0.6, 1.4);
                    }
                }
            }
            Phase::Move => {
                if self.holder == Some(t.id) {
                    if let Some(f) = self.frog_at(t.pos) {
                        self.selected = Some(f);
                    }
                }
            }
            Phase::Up | Phase::Cancel => {
                if self.holder == Some(t.id) {
                    self.holder = None; // the selection stays until the twin is tapped
                }
            }
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0xFFE7A8), hex(0xF7B967));
        // sunburst
        r.push();
        r.translate(c.center_x(), h * 0.55);
        r.rotate(c.anim * 0.05);
        for i in 0..16 {
            let a = i as f32 * std::f32::consts::TAU / 16.0;
            let b = a + 0.12;
            r.tri(v2(0.0, 0.0), v2(a.cos() * 1400.0, a.sin() * 1400.0), v2(b.cos() * 1400.0, b.sin() * 1400.0), Col::WHITE.a(0.12));
        }
        r.pop();
        let k = ease_out_back((self.appear / 0.3).min(1.0));
        let sel = self.selected;
        for (i, f) in self.frogs.iter().enumerate() {
            let skin = SKINS[f.color];
            if f.alive {
                r.ellipse(f.pos.x, f.pos.y + 48.0 * k, 52.0 * k, 12.0 * k, art::SHADOW, art::SHADOW, 0.0);
                if sel == Some(i) {
                    r.glow(f.pos.x, f.pos.y, 110.0, Col::WHITE.a(0.8));
                    r.ring(f.pos.x, f.pos.y + 4.0, 70.0, 6.0, hex(0xFFFFFF));
                }
                let mut pose = Pose::idle(c.anim, f.seed);
                if sel == Some(i) {
                    pose.mouth = 0.5;
                    pose.squash = -0.06;
                }
                art::frog(r, f.pos.x, f.pos.y - 6.0, 118.0 * k, skin, &pose);
            } else if f.pop_t < 0.25 {
                let s = 1.0 + f.pop_t * 3.0;
                r.push();
                r.fade(1.0 - f.pop_t / 0.25);
                art::frog(r, f.pos.x, f.pos.y - 6.0, 118.0 * s, skin, &Pose { dizzy: true, mouth: 1.0, ..Pose::default() });
                r.pop();
            }
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    let p = Pose::idle(t, 0.0);
    art::frog(r, x - s * 0.24, y - s * 0.1, s * 0.42, art::SKY, &p);
    art::frog(r, x + s * 0.24, y - s * 0.1, s * 0.42, art::SKY, &Pose::idle(t, 1.0));
    art::frog(r, x, y + s * 0.16, s * 0.42, art::BLOSSOM, &Pose::idle(t, 2.0));
    r.ring(x - s * 0.24, y - s * 0.1, s * 0.25, 3.0, Col::WHITE);
}
