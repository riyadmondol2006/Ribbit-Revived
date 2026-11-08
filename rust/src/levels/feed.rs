//! Snack Belt: fruit rides a conveyor towards the hungry frog. Tap the button that matches
//! the fruit at the front of the belt to feed it. Bonus +3 s at the 10th fruit and every 11
//! after. A wrong button just wastes time.
use super::{Ctx, MiniGame};
use crate::art::{self, Fruit, Pose};
use crate::audio::Sfx;
use crate::input::{Phase, Touch};
use crate::math::*;
use crate::render::Renderer;

const KINDS: [Fruit; 5] = [Fruit::Apple, Fruit::Orange, Fruit::Pear, Fruit::Grapes, Fruit::Banana];

pub struct Feed {
    belt: Vec<usize>,
    last: usize,
    shift: f32,
    eat_t: f32,
    flying: Option<(usize, f32)>,
    counter: i32,
    press: [f32; 5],
    wrong: [f32; 5],
    belt_phase: f32,
}

pub fn make(c: &mut Ctx) -> Box<dyn MiniGame> {
    let mut g = Feed { belt: Vec::new(), last: c.rng.below(5), shift: 1.0, eat_t: 9.0, flying: None, counter: 1, press: [9.0; 5], wrong: [9.0; 5], belt_phase: 0.0 };
    for _ in 0..7 {
        let f = g.next_fruit(c);
        g.belt.push(f);
    }
    Box::new(g)
}

impl Feed {
    fn next_fruit(&mut self, c: &mut Ctx) -> usize {
        self.last = (self.last + 1 + c.rng.below(4)) % 5;
        self.last
    }
    fn frog_pos(c: &Ctx) -> V2 {
        v2(c.sx(0.13).max(c.safe_l + 130.0), c.h * 0.42)
    }
    fn pitch(c: &Ctx) -> f32 {
        ((c.safe_r - Self::frog_pos(c).x - 170.0) / 6.5).clamp(120.0, 175.0)
    }
    fn slot_pos(c: &Ctx, i: f32) -> V2 {
        let f = Self::frog_pos(c);
        v2(f.x + 170.0 + i * Self::pitch(c), c.h * 0.44)
    }
    fn button_pos(c: &Ctx, i: usize) -> V2 {
        v2(c.center_x() + (i as f32 - 2.0) * 200.0, c.h - 100.0)
    }
}

impl MiniGame for Feed {
    fn update(&mut self, _c: &mut Ctx, dt: f32) {
        if self.shift < 1.0 {
            self.shift = (self.shift + dt / 0.1).min(1.0);
            self.belt_phase += dt * 600.0;
        }
        self.eat_t += dt;
        for p in self.press.iter_mut().chain(self.wrong.iter_mut()) {
            *p += dt;
        }
        if let Some((_, t)) = &mut self.flying {
            *t += dt;
            if *t > 0.15 {
                self.flying = None;
            }
        }
    }

    fn touch(&mut self, c: &mut Ctx, t: &Touch) {
        if t.phase != Phase::Down || self.shift < 1.0 {
            return;
        }
        let Some(b) = (0..5).find(|&i| t.pos.dist(Self::button_pos(c, i)) < 95.0) else { return };
        self.press[b] = 0.0;
        if self.belt[0] == b {
            let eaten = self.belt.remove(0);
            let f = self.next_fruit(c);
            self.belt.push(f);
            self.shift = 0.0;
            self.flying = Some((eaten, 0.0));
            self.eat_t = 0.0;
            let fp = Self::frog_pos(c);
            c.add_score(1, fp - v2(0.0, 150.0));
            c.sfx_var(Sfx::Eat);
            c.tick();
            self.counter += 1;
            if self.counter > 10 {
                self.counter = 0;
                c.add_time(3.0, fp - v2(-120.0, 200.0));
            }
        } else {
            self.wrong[b] = 0.0;
            c.sfx_ex(Sfx::Wrong, 0.5, 1.2);
            c.reject();
        }
    }

    fn draw(&mut self, r: &mut Renderer, c: &Ctx) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0xFFEFD0), hex(0xF5C98A));
        // kitchen tiles
        for i in 0..((w / 80.0) as i32 + 1) {
            r.rect(i as f32 * 80.0, h * 0.62, 2.0, h * 0.38, Col::WHITE.a(0.25));
        }
        r.rect(0.0, h * 0.62, w, 3.0, Col::WHITE.a(0.3));
        // belt
        let s0 = Self::slot_pos(c, -0.6);
        let s1 = Self::slot_pos(c, 6.6);
        let by = c.h * 0.44 + 52.0;
        r.rrect((s0.x + s1.x) * 0.5, by + 18.0, s1.x - s0.x + 60.0, 58.0, 29.0, hex(0x5A6178), art::INK, 4.0);
        r.rrect((s0.x + s1.x) * 0.5, by, s1.x - s0.x + 60.0, 24.0, 10.0, hex(0x8E97A8), art::INK, 3.0);
        let mut x = s0.x - 10.0 - (self.belt_phase % 40.0);
        while x < s1.x + 20.0 {
            if x > s0.x - 20.0 {
                r.rect(x, by - 10.0, 4.0, 20.0, hex(0x6B7385));
            }
            x += 40.0;
        }
        let mut wx = s0.x;
        while wx < s1.x + 10.0 {
            r.circle(wx, by + 22.0, 18.0, hex(0xC9D2DC), art::INK, 3.0);
            r.disc(wx, by + 22.0, 5.0, art::INK);
            wx += 70.0;
        }
        // fruit on the belt (sliding one slot left while shifting)
        let off = 1.0 - ease_out_cubic(self.shift);
        for (i, &f) in self.belt.iter().enumerate() {
            let p = Self::slot_pos(c, i as f32 + off);
            art::fruit(r, KINDS[f], p.x, p.y, 92.0);
        }
        // frog
        let fp = Self::frog_pos(c);
        let chew = if self.eat_t < 1.2 { (self.eat_t * 14.0).sin().abs() * 0.4 } else { 0.0 };
        let pose = Pose { mouth: if self.eat_t < 0.12 { 1.0 } else { chew }, look: v2(0.8, 0.1), ..Pose::idle(c.anim, 0.0) };
        art::frog(r, fp.x, fp.y, 240.0, art::EMERALD, &pose);
        if let Some((f, t)) = self.flying {
            let k = (t / 0.15).min(1.0);
            let a = Self::slot_pos(c, 0.0);
            let b = fp + v2(0.0, 30.0);
            let p = a.lerp(b, k);
            art::fruit(r, KINDS[f], p.x, p.y - (k * std::f32::consts::PI).sin() * 40.0, 92.0 * (1.0 - k * 0.6));
        }
        // buttons
        for i in 0..5 {
            let p = Self::button_pos(c, i);
            let d = if self.press[i] < 0.1 { 6.0 } else { 0.0 };
            let shake = if self.wrong[i] < 0.3 { (self.wrong[i] * 60.0).sin() * 8.0 * (1.0 - self.wrong[i] / 0.3) } else { 0.0 };
            r.circle(p.x + shake, p.y + 8.0, 78.0, hex(0xC46A12), art::INK, 4.0);
            let col = if self.wrong[i] < 0.3 { hex(0xFF8A80) } else { hex(0xFFF4D6) };
            r.circle(p.x + shake, p.y + d, 78.0, col, art::INK, 4.0);
            art::fruit(r, KINDS[i], p.x + shake, p.y + d, 88.0);
        }
    }
}

pub fn icon(r: &mut Renderer, x: f32, y: f32, s: f32, t: f32) {
    r.rrect(x + s * 0.18, y + s * 0.1, s * 0.75, s * 0.12, 8.0, hex(0x8E97A8), art::INK, 2.0);
    let k = (t * 0.7).fract() * s * 0.2;
    art::fruit(r, Fruit::Apple, x + s * 0.0 + s * 0.2 - k, y - s * 0.02, s * 0.2);
    art::fruit(r, Fruit::Grapes, x + s * 0.4 - k, y - s * 0.02, s * 0.2);
    art::frog(r, x - s * 0.33, y, s * 0.42, art::EMERALD, &Pose { mouth: 0.9, ..Pose::idle(t, 0.0) });
}
