//! Small math toolkit: vectors, colours, easing and a deterministic RNG.
#![allow(dead_code)]

use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

pub const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 {
            v2(self.x / l, self.y / l)
        } else {
            v2(0.0, 0.0)
        }
    }
    pub fn lerp(self, o: V2, t: f32) -> V2 {
        v2(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn rot(self, a: f32) -> V2 {
        let (s, c) = a.sin_cos();
        v2(self.x * c - self.y * s, self.x * s + self.y * c)
    }
}
impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        v2(self.x * s, self.y * s)
    }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl SubAssign for V2 {
    fn sub_assign(&mut self, o: V2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}

/// Straight (non-premultiplied) RGBA colour, 8 bits per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Col(pub u8, pub u8, pub u8, pub u8);

pub const fn hex(c: u32) -> Col {
    Col((c >> 16) as u8, (c >> 8) as u8, c as u8, 255)
}

impl Col {
    pub const WHITE: Col = Col(255, 255, 255, 255);
    pub const BLACK: Col = Col(0, 0, 0, 255);
    pub const CLEAR: Col = Col(0, 0, 0, 0);
    pub fn a(self, a: f32) -> Col {
        Col(self.0, self.1, self.2, (self.3 as f32 * a.clamp(0.0, 1.0)) as u8)
    }
    pub fn mix(self, o: Col, t: f32) -> Col {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Col(m(self.0, o.0), m(self.1, o.1), m(self.2, o.2), m(self.3, o.3))
    }
    pub fn shade(self, f: f32) -> Col {
        if f >= 0.0 {
            self.mix(Col(255, 255, 255, self.3), f)
        } else {
            self.mix(Col(0, 0, 0, self.3), -f)
        }
    }
}

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = clamp01(t) - 1.0;
    t * t * t + 1.0
}
pub fn ease_in_out(t: f32) -> f32 {
    let t = clamp01(t);
    t * t * (3.0 - 2.0 * t)
}
pub fn ease_out_back(t: f32) -> f32 {
    let t = clamp01(t);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}
pub fn ease_out_elastic(t: f32) -> f32 {
    let t = clamp01(t);
    if t == 0.0 || t == 1.0 {
        return t;
    }
    let c4 = std::f32::consts::TAU / 3.0;
    2f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
}
/// Damped "boing" used for squash and stretch on taps: 1 at t=0 decaying to 0.
pub fn wobble(t: f32, freq: f32) -> f32 {
    if t < 0.0 {
        return 0.0;
    }
    (-(t * 6.0)).exp() * (t * freq).sin()
}

/// xorshift64* - fast, deterministic, good enough for gameplay.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1) ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in [0, 1).
    pub fn f(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    /// Uniform integer in [0, n).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
    pub fn irange(&mut self, a: i32, b: i32) -> i32 {
        a + self.below((b - a + 1).max(1) as usize) as i32
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
    pub fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len())]
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}
