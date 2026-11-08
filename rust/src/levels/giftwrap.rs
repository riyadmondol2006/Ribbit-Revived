//! Gift Wrap: boxes ride the conveyor, each showing the toy it wants. Tap the matching toy
//! from the shelf to pack it. Filling every box in a row earns bonus time (1-3 s) and rolls
//! in the next order. A wrong pick is reshuffled after a short pause.
use super::{Ctx, MiniGame};
use crate::art::{self, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::{Align, Renderer};
use crate::toys;

struct Box_ {
    toy: usize,
    filled: f32,
}

struct Pattern {
    offered: usize,
    boxes: usize,
    bonus: f32,
}

pub struct GiftWrap {
    pattern: usize,
    boxes: Vec<Box_>,
    order: Vec<usize>,
    filled: usize,
    choice: Vec<usize>,
    choice_t: f32,
    lock: f32,
    flying: Option<(usize, V2, usize, f32)>,
    wrong: Option<(V2, f32)>,
    slide: f32,
    old_boxes: Vec<usize>,
    missed: bool,
    bonus: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = GiftWrap { pattern: 0, boxes: Vec::new(), order: Vec::new(), filled: 0, choice: Vec::new(), choice_t: 0.0, lock: 0.0, flying: None, wrong: None, slide: 1.0, old_boxes: Vec::new(), missed: false, bonus: 1.0 };
    g.new_pattern(c, false);
    Box::new(g)
}

impl GiftWrap {
    fn spec(&self, c: &mut Ctx) -> Pattern {
        let p = self.pattern % 91;
        let r = &mut c.rng;
        let (offered, boxes, bonus) = match p {
            0..=8 => (2, 1, 1.0),
            9..=18 => (2, r.irange(2, 3) as usize, r.irange(1, 2) as f32),
            19..=29 => (2, r.irange(1, 2) as usize, 1.0),
            30..=42 => (3, r.irange(1, 4) as usize, r.irange(1, 3) as f32),
            43..=51 => (2, r.irange(1, 3) as usize, r.irange(1, 2) as f32),
            52..=58 => (3, r.irange(2, 4) as usize, 2.0),
            59..=61 => (r.irange(4, 5) as usize, r.irange(4, 5) as usize, 3.0),
            62..=79 => (r.irange(2, 4) as usize, r.irange(1, 5) as usize, r.irange(1, 3) as f32),
            _ => (r.irange(3, 5) as usize, 5, (3.0 - (p - 80) as f32 / 5.0).max(1.0)),
        };
        Pattern { offered, boxes, bonus }
    }
    /// Toys available at this point of the progression (the rocket joins around pattern 46).
    fn pool(&self) -> Vec<usize> {
        let n = (8 + self.pattern / 6).min(toys::COUNT);
        let mut v: Vec<usize> = (0..toys::COUNT).filter(|&t| t != toys::ROCKET).take(n).collect();
        if self.pattern >= 46 {
            v.push(toys::ROCKET);
        }
        v
    }
    fn new_pattern(&mut self, c: &mut Ctx, animate: bool) {
        let sp = self.spec(c);
        let mut pool = self.pool();
        c.rng.shuffle(&mut pool);
        if self.pattern == 46 && !pool[..sp.boxes].contains(&toys::ROCKET) {
            // make sure the rocket's debut box actually shows up
            if let Some(i) = pool.iter().position(|&t| t == toys::ROCKET) {
                pool.swap(0, i);
            }
        }
        self.old_boxes = self.boxes.iter().map(|b| b.toy).collect();
        self.boxes = pool[..sp.boxes].iter().map(|&t| Box_ { toy: t, filled: -1.0 }).collect();
        self.order = (0..sp.boxes).collect();
        c.rng.shuffle(&mut self.order);
        self.filled = 0;
        self.bonus = sp.bonus;
        self.slide = if animate { 0.0 } else { 1.0 };
        self.new_choice(c, sp.offered.max(2));
    }
    fn new_choice(&mut self, c: &mut Ctx, n: usize) {
        let target = self.boxes[self.order[self.filled]].toy;
        let box_toys: Vec<usize> = self.boxes.iter().map(|b| b.toy).collect();
        let mut distract: Vec<usize> = (0..toys::COUNT).filter(|t| !box_toys.contains(t)).collect();
        c.rng.shuffle(&mut distract);
        let correct = c.rng.below(n);
        self.choice = (0..n).map(|k| if k == correct { target } else { distract[k] }).collect();
        self.choice_t = 0.0;
        self.lock = 0.4;
    }
    fn toy_pos(&self, c: &Ctx, k: usize) -> V2 {
        let n = self.choice.len() as f32;
        v2(c.center_x() + 0.2 * super::STAGE_W * (k as f32 - (n - 1.0) * 0.5), c.h * 0.4)
    }
    fn box_pos(&self, c: &Ctx, i: usize, count: usize) -> V2 {
        let n = count as f32;
        let slide = (1.0 - ease_out_cubic(self.slide)) * -900.0;
        v2(c.center_x() + 0.2 * super::STAGE_W * (i as f32 - (n - 1.0) * 0.5) + slide, c.h * 0.8)
    }
}

impl MiniGame for GiftWrap {
    fn idle(&mut self, _c: &mut Ctx, dt: f32) {
        self.choice_t += dt;
    }

    fn update(&mut self, c: &mut Ctx, dt: f32) {
        self.choice_t += dt;
        self.lock = (self.lock - dt).max(0.0);
        if self.slide < 1.0 {
            self.slide = (self.slide + dt / 0.4).min(1.0);
        }
        for b in self.boxes.iter_mut() {
            if b.filled >= 0.0 {
                b.filled += dt;
            }
        }
        if let Some((_, t)) = &mut self.wrong {
            *t += dt;
            if *t > 0.7 {
                self.wrong = None;
            }
        }
        if let Some((toy, from, bi, t)) = &mut self.flying {
            *t += dt;
            if *t >= 0.2 {
                let (toy, _from, bi) = (*toy, *from, *bi);
                self.flying = None;
                self.boxes[bi].filled = 0.0;
                c.sfx_var(Sfx::Thud);
                if toy == toys::ROCKET && !self.missed {
                    c.special_hit = true;
                }
                if self.filled >= self.boxes.len() {
                    let bonus = self.bonus;
                    c.add_time(bonus, v2(c.center_x(), c.h * 0.6));
                    self.pattern += 1;
                    self.new_pattern(c, true);
                } else {
                    let n = self.choice.len();
                    self.new_choice(c, n);
                }
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.lock > 0.0 || self.flying.is_some() || self.wrong.is_some() {
            return;
        }
        let Some(k) = (0..self.choice.len()).find(|&k| self.toy_pos(c, k).dist(t.pos) < 95.0) else { return };
        let toy = self.choice[k];
        let target_box = self.order[self.filled];
        if toy == self.boxes[target_box].toy {
            self.filled += 1;
            let from = self.toy_pos(c, k);
            self.flying = Some((toy, from, target_box, 0.0));
            self.choice[k] = usize::MAX;
            c.add_score(1, from - v2(0.0, 80.0));
            c.sfx(Sfx::Collect);
            c.tick();
        } else {
            self.missed = true;
            let p = self.toy_pos(c, k);
            self.wrong = Some((p, 0.0));
            c.mistake(p);
            let n = self.choice.len();
            self.new_choice(c, n);
            self.lock = 0.7;
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0xC9E8FF), hex(0xF2E4FF));
        // shop shelf behind the offered toys
        r.rrect(c.center_x(), h * 0.48, w + 40.0, 26.0, 8.0, hex(0xB07A45), art::INK, 4.0);
        // conveyor
        let by = h * 0.8;
        r.rrect(c.center_x(), by + 70.0, w + 40.0, 50.0, 20.0, hex(0x5A6178), art::INK, 4.0);
        let mut x = -((c.anim * 80.0) % 60.0);
        while x < w {
            r.circle(x, by + 70.0, 14.0, hex(0xC9D2DC), art::INK, 3.0);
            x += 60.0;
        }
        // packing frog
        let fp = v2(c.safe_l + 130.0, h * 0.62);
        r.rrect(fp.x, h * 0.71, 160.0, 40.0, 10.0, hex(0xF27BC4), art::INK, 4.0);
        art::frog(r, fp.x, fp.y - 20.0, 150.0, art::EMERALD, &Pose { look: v2(0.8, -0.2), ..Pose::idle(c.anim, 0.0) });
        // boxes: the old row slides out, the new row slides in
        if self.slide < 1.0 {
            let n = self.old_boxes.len();
            for (i, &t) in self.old_boxes.iter().enumerate() {
                let p = self.box_pos(c, i, n) + v2(900.0, 0.0);
                art::gift(r, p.x, p.y, 130.0, hex(0xF0605D), hex(0xFFD23F));
                let _ = t;
            }
        }
        let n = self.boxes.len();
        for (i, b) in self.boxes.iter().enumerate() {
            let p = self.box_pos(c, i, n);
            if b.filled >= 0.0 {
                let s = 1.0 + wobble(b.filled, 25.0) * 0.12;
                r.push();
                r.translate(p.x, p.y);
                r.scale(s, 1.0 / s);
                art::gift(r, 0.0, 0.0, 130.0, [hex(0xF0605D), hex(0x4FA7F2), hex(0x9A6BEA), hex(0x4CC38A), hex(0xFF9F2E)][i % 5], hex(0xFFD23F));
                r.pop();
            } else {
                r.rrect(p.x, p.y + 6.0, 130.0, 100.0, 10.0, hex(0xC9935A), art::INK, 4.0);
                r.rrect(p.x, p.y - 44.0, 140.0, 16.0, 6.0, hex(0xB07A45), art::INK, 3.0);
                toys::toy(r, b.toy, p.x, p.y + 4.0, 66.0, None);
                let is_next = self.filled < self.order.len() && self.order[self.filled] == i;
                if is_next {
                    r.ring(p.x, p.y + 4.0, 70.0 + (c.anim * 6.0).sin() * 4.0, 5.0, Col::WHITE.a(0.9));
                }
            }
        }
        // offered toys
        let k = ease_out_back((self.choice_t / 0.4).min(1.0));
        for (i, &t) in self.choice.iter().enumerate() {
            if t == usize::MAX {
                continue;
            }
            let p = self.toy_pos(c, i);
            r.ellipse(p.x, p.y + 56.0, 50.0 * k, 10.0 * k, art::SHADOW, Col::CLEAR, 0.0);
            toys::toy(r, t, p.x, p.y, 110.0 * k, None);
        }
        if let Some((toy, from, bi, t)) = self.flying {
            let to = self.box_pos(c, bi, n);
            let kk = (t / 0.2).min(1.0);
            let p = from.lerp(to, kk) - v2(0.0, (kk * std::f32::consts::PI).sin() * 80.0);
            toys::toy(r, toy, p.x, p.y, 110.0 * (1.0 - kk * 0.4), None);
        }
        if let Some((p, t)) = self.wrong {
            let a = 1.0 - (t / 0.7);
            r.text_bold("X", p.x, p.y, 120.0 * (1.0 + t * 0.5), Align::Center, hex(0xFF5D5D).a(a), art::INK.a(a));
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, _t: f32) {
    art::gift(r, x - s * 0.22, y + s * 0.18, s * 0.34, hex(0xF0605D), hex(0xFFD23F));
    toys::toy(r, 1, x + s * 0.22, y - s * 0.1, s * 0.35, None);
    toys::toy(r, toys::ROCKET, x - s * 0.22, y - s * 0.25, s * 0.25, None);
}
