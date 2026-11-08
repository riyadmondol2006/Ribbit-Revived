//! Twenty procedurally drawn toys used by Shape Sort and Gift Wrap. Each can be drawn in full
//! colour or as a flat silhouette (for "find the matching shadow" puzzles).
#![allow(clippy::too_many_arguments)]

use crate::art::INK;
use crate::math::*;
use crate::render::{Align, Renderer};
use std::f32::consts::{PI, TAU};

pub const COUNT: usize = 20;
pub const ROCKET: usize = 7;
pub const SNOWMAN: usize = 12;

pub const NAMES: [&str; COUNT] = [
    "teddy", "duck", "ball", "block", "car", "drum", "top", "rocket", "robot", "train", "boat", "star", "snowman", "house", "kite", "tree", "umbrella", "cupcake", "fish",
    "crown",
];

struct P {
    tint: Option<Col>,
}
impl P {
    fn c(&self, base: Col) -> Col {
        self.tint.unwrap_or(base)
    }
    fn l(&self) -> Col {
        if self.tint.is_some() {
            self.tint.unwrap()
        } else {
            INK
        }
    }
}

/// Draws toy `id` centred at (x, y), roughly `s` tall. With `tint`, every part uses that colour.
pub fn toy(r: &mut Renderer, id: usize, x: f32, y: f32, s: f32, tint: Option<Col>) {
    let p = P { tint };
    let k = s / 100.0;
    r.push();
    r.translate(x, y);
    r.scale(k, k);
    let lw = 4.0;
    let l = p.l();
    match id % COUNT {
        0 => {
            // teddy bear
            let b = p.c(hex(0xB5793F));
            r.circle(-26.0, -36.0, 14.0, b, l, lw);
            r.circle(26.0, -36.0, 14.0, b, l, lw);
            r.ellipse(0.0, 22.0, 36.0, 32.0, b, l, lw);
            r.circle(0.0, -18.0, 30.0, b, l, lw);
            r.ellipse(0.0, -8.0, 14.0, 10.0, p.c(hex(0xE8C39A)), p.c(hex(0xE8C39A)), 0.0);
            if tint.is_none() {
                r.disc(-10.0, -24.0, 4.0, INK);
                r.disc(10.0, -24.0, 4.0, INK);
                r.disc(0.0, -11.0, 4.0, INK);
            }
        }
        1 => {
            // rubber duck
            let y_ = p.c(hex(0xFFD23F));
            r.ellipse(4.0, 18.0, 42.0, 28.0, y_, l, lw);
            r.circle(-12.0, -20.0, 24.0, y_, l, lw);
            r.ellipse(-40.0, -14.0, 14.0, 7.0, p.c(hex(0xFF8A2B)), l, 3.0);
            r.ellipse(18.0, 12.0, 18.0, 10.0, p.c(hex(0xF2B92F)), Col::CLEAR, 0.0);
            if tint.is_none() {
                r.disc(-16.0, -26.0, 4.0, INK);
            }
        }
        2 => {
            // beach ball
            r.circle(0.0, 0.0, 44.0, p.c(hex(0xFFFFFF)), l, lw);
            if tint.is_none() {
                r.pie(0.0, 0.0, 40.0, 0.17, hex(0xF0605D));
                r.push();
                r.rotate(TAU / 3.0);
                r.pie(0.0, 0.0, 40.0, 0.17, hex(0x4FA7F2));
                r.rotate(TAU / 3.0);
                r.pie(0.0, 0.0, 40.0, 0.17, hex(0xFFD23F));
                r.pop();
                r.disc(0.0, 0.0, 8.0, Col::WHITE);
            }
        }
        3 => {
            // letter block
            r.rrect(0.0, 0.0, 80.0, 80.0, 10.0, p.c(hex(0x4FA7F2)), l, lw);
            if tint.is_none() {
                r.rrect(0.0, 0.0, 58.0, 58.0, 6.0, hex(0x9FD0FF), Col::CLEAR, 0.0);
                r.text_bold("A", 0.0, 2.0, 46.0, Align::Center, hex(0xF0605D), INK);
            }
        }
        4 => {
            // toy car
            r.rrect(0.0, 6.0, 96.0, 34.0, 12.0, p.c(hex(0xF0605D)), l, lw);
            r.rrect(-6.0, -18.0, 52.0, 30.0, 10.0, p.c(hex(0xF0605D)), l, lw);
            r.rrect(-6.0, -18.0, 40.0, 18.0, 5.0, p.c(hex(0xBFE9FF)), Col::CLEAR, 0.0);
            r.circle(-28.0, 26.0, 14.0, p.c(hex(0x3A3F4E)), l, lw);
            r.circle(28.0, 26.0, 14.0, p.c(hex(0x3A3F4E)), l, lw);
        }
        5 => {
            // drum
            r.rrect(0.0, 10.0, 84.0, 60.0, 8.0, p.c(hex(0xF0605D)), l, lw);
            r.ellipse(0.0, -20.0, 42.0, 14.0, p.c(hex(0xFFF4D6)), l, lw);
            if tint.is_none() {
                for i in 0..4 {
                    let xx = -32.0 + i as f32 * 21.0;
                    r.line(v2(xx, -6.0), v2(xx + 10.0, 36.0), 3.0, hex(0xFFD23F));
                }
            }
            r.line(v2(10.0, -32.0), v2(46.0, -56.0), 5.0, p.c(hex(0xB5793F)));
        }
        6 => {
            // spinning top
            r.tri(v2(-44.0, -6.0), v2(44.0, -6.0), v2(0.0, 44.0), l);
            r.tri(v2(-38.0, -4.0), v2(38.0, -4.0), v2(0.0, 38.0), p.c(hex(0x9A6BEA)));
            r.ellipse(0.0, -8.0, 44.0, 16.0, p.c(hex(0xF27BC4)), l, lw);
            r.rrect(0.0, -32.0, 10.0, 28.0, 4.0, p.c(hex(0xB5793F)), l, 3.0);
        }
        7 => {
            // rocket
            r.rrect(0.0, 0.0, 40.0, 80.0, 20.0, p.c(hex(0xE8EDF2)), l, lw);
            r.tri(v2(-20.0, 20.0), v2(-40.0, 44.0), v2(-20.0, 40.0), p.c(hex(0xF0605D)));
            r.tri(v2(20.0, 20.0), v2(40.0, 44.0), v2(20.0, 40.0), p.c(hex(0xF0605D)));
            r.circle(0.0, -10.0, 10.0, p.c(hex(0x4FA7F2)), l, 3.0);
            r.ellipse(0.0, 48.0, 10.0, 12.0, p.c(hex(0xFFB020)), Col::CLEAR, 0.0);
        }
        8 => {
            // robot
            r.rrect(0.0, 20.0, 60.0, 50.0, 8.0, p.c(hex(0x9AA5B1)), l, lw);
            r.rrect(0.0, -26.0, 50.0, 38.0, 8.0, p.c(hex(0xB8C2D0)), l, lw);
            r.line(v2(0.0, -45.0), v2(0.0, -58.0), 4.0, l);
            r.disc(0.0, -60.0, 6.0, p.c(hex(0xF0605D)));
            if tint.is_none() {
                r.disc(-12.0, -28.0, 6.0, hex(0x5EE6FF));
                r.disc(12.0, -28.0, 6.0, hex(0x5EE6FF));
                r.rrect(0.0, 22.0, 26.0, 12.0, 3.0, hex(0xFFD23F), Col::CLEAR, 0.0);
            }
        }
        9 => {
            // train engine
            r.rrect(-12.0, 4.0, 70.0, 40.0, 8.0, p.c(hex(0x4CC38A)), l, lw);
            r.rrect(26.0, -10.0, 34.0, 56.0, 6.0, p.c(hex(0x3D9B6E)), l, lw);
            r.rrect(-30.0, -26.0, 14.0, 24.0, 4.0, p.c(hex(0x3A3F4E)), l, 3.0);
            r.circle(-30.0, 30.0, 12.0, p.c(hex(0x3A3F4E)), l, lw);
            r.circle(2.0, 30.0, 12.0, p.c(hex(0x3A3F4E)), l, lw);
            r.circle(30.0, 30.0, 12.0, p.c(hex(0x3A3F4E)), l, lw);
        }
        10 => {
            // sailboat
            r.tri(v2(-6.0, 10.0), v2(-6.0, -48.0), v2(-44.0, 10.0), p.c(hex(0xFFFFFF)));
            r.tri(v2(2.0, 10.0), v2(2.0, -40.0), v2(36.0, 10.0), p.c(hex(0xF0605D)));
            r.line(v2(-2.0, 14.0), v2(-2.0, -50.0), 4.0, l);
            r.rrect(0.0, 24.0, 90.0, 22.0, 10.0, p.c(hex(0x4FA7F2)), l, lw);
        }
        11 => r.star(0.0, 4.0, 48.0, 0.0, p.c(hex(0xFFD23F)), l, 6.0),
        12 => {
            // snowman
            r.circle(0.0, 24.0, 30.0, p.c(hex(0xF4F7FA)), l, lw);
            r.circle(0.0, -18.0, 22.0, p.c(hex(0xF4F7FA)), l, lw);
            r.rrect(0.0, -42.0, 34.0, 18.0, 4.0, p.c(hex(0x3A3F4E)), l, 3.0);
            r.rrect(0.0, -34.0, 50.0, 6.0, 3.0, p.c(hex(0x3A3F4E)), l, 2.0);
            if tint.is_none() {
                r.disc(-7.0, -20.0, 3.0, INK);
                r.disc(7.0, -20.0, 3.0, INK);
                r.tri(v2(0.0, -14.0), v2(0.0, -8.0), v2(14.0, -11.0), hex(0xFF8A2B));
                r.rrect(0.0, -2.0, 46.0, 8.0, 4.0, hex(0xF0605D), Col::CLEAR, 0.0);
            }
        }
        13 => {
            // house
            r.rrect(0.0, 16.0, 70.0, 56.0, 4.0, p.c(hex(0xFFE3A8)), l, lw);
            r.tri(v2(-48.0, -10.0), v2(48.0, -10.0), v2(0.0, -50.0), l);
            r.tri(v2(-40.0, -12.0), v2(40.0, -12.0), v2(0.0, -44.0), p.c(hex(0xF0605D)));
            r.rrect(0.0, 28.0, 18.0, 30.0, 3.0, p.c(hex(0xB5793F)), l, 3.0);
        }
        14 => {
            // kite
            r.tri(v2(0.0, -48.0), v2(36.0, -6.0), v2(0.0, 30.0), l);
            r.tri(v2(0.0, -48.0), v2(-36.0, -6.0), v2(0.0, 30.0), l);
            r.tri(v2(0.0, -42.0), v2(30.0, -6.0), v2(0.0, 24.0), p.c(hex(0xF27BC4)));
            r.tri(v2(0.0, -42.0), v2(-30.0, -6.0), v2(0.0, 24.0), p.c(hex(0x9A6BEA)));
            for i in 0..3 {
                let yy = 34.0 + i as f32 * 10.0;
                r.ellipse(if i % 2 == 0 { -6.0 } else { 6.0 }, yy, 6.0, 4.0, p.c(hex(0xFFD23F)), Col::CLEAR, 0.0);
            }
        }
        15 => {
            // pine tree
            for (i, (yy, ww)) in [(-30.0, 30.0), (-6.0, 42.0), (18.0, 54.0)].iter().enumerate() {
                r.tri(v2(-ww - 4.0, yy + 22.0), v2(ww + 4.0, yy + 22.0), v2(0.0, yy - 28.0), l);
                r.tri(v2(-ww, yy + 19.0), v2(*ww, yy + 19.0), v2(0.0, yy - 23.0), p.c(hex(0x3D9B6E).shade(i as f32 * 0.06)));
            }
            r.rrect(0.0, 46.0, 16.0, 14.0, 3.0, p.c(hex(0xB5793F)), l, 3.0);
        }
        16 => {
            // umbrella
            r.push();
            r.rotate(-PI * 0.5);
            r.pie(0.0, 0.0, 50.0, 0.5, l);
            r.pie(0.0, 0.0, 45.0, 0.5, p.c(hex(0x4FA7F2)));
            r.pop();
            r.line(v2(0.0, 0.0), v2(0.0, 40.0), 5.0, l);
            r.ring(-8.0, 40.0, 8.0, 5.0, l);
        }
        17 => {
            // cupcake
            r.tri(v2(-30.0, 0.0), v2(30.0, 0.0), v2(22.0, 40.0), p.c(hex(0x8FD3F4)));
            r.tri(v2(-30.0, 0.0), v2(-22.0, 40.0), v2(22.0, 40.0), p.c(hex(0x8FD3F4)));
            r.circle(0.0, -12.0, 32.0, p.c(hex(0xFFC1E3)), l, lw);
            r.disc(0.0, -46.0, 9.0, p.c(hex(0xF0605D)));
        }
        18 => {
            // fish
            r.ellipse(-6.0, 0.0, 42.0, 28.0, p.c(hex(0xFF9F2E)), l, lw);
            r.tri(v2(30.0, 0.0), v2(52.0, -22.0), v2(52.0, 22.0), l);
            r.tri(v2(32.0, 0.0), v2(48.0, -16.0), v2(48.0, 16.0), p.c(hex(0xFF9F2E)));
            if tint.is_none() {
                r.disc(-28.0, -6.0, 5.0, INK);
            }
        }
        _ => {
            // crown
            r.rrect(0.0, 20.0, 80.0, 26.0, 4.0, p.c(hex(0xFFD23F)), l, lw);
            for i in 0..3 {
                let xx = -30.0 + i as f32 * 30.0;
                r.tri(v2(xx - 16.0, 10.0), v2(xx + 16.0, 10.0), v2(xx, -30.0), l);
                r.tri(v2(xx - 12.0, 9.0), v2(xx + 12.0, 9.0), v2(xx, -24.0), p.c(hex(0xFFD23F)));
                r.disc(xx, -26.0, 6.0, p.c(hex(0xF0605D)));
            }
        }
    }
    r.pop();
}
