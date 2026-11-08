//! Minigame framework: shared per-round context, effects, and the level registry.
#![allow(dead_code)]

use crate::audio::{self, Sfx};
use crate::input::Touch;
use crate::math::*;
use crate::platform::*;
use crate::render::{Align, Renderer};

mod astro;
mod balloon;
mod ballswap;
mod bandit;
mod bounce;
mod build;
mod cake;
mod connect;
mod dive;
mod drive;
mod feed;
mod fly;
mod giftwrap;
mod goal;
mod hop;
mod jet;
mod leap;
mod padcatch;
mod paint;
mod pop;
mod quickdraw;
mod ride;
mod slide;
mod sort;
mod sumo;
mod treat;
mod twin;
mod volley;

// ------------------------------------------------------------------ effects

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PKind {
    Disc,
    Glow,
    Star,
    Confetti,
    Ring,
    Drop,
}

#[derive(Clone)]
struct Particle {
    pos: V2,
    vel: V2,
    life: f32,
    max: f32,
    size: f32,
    col: Col,
    kind: PKind,
    rot: f32,
    vrot: f32,
    grav: f32,
}

#[derive(Clone)]
struct FloatText {
    text: String,
    pos: V2,
    life: f32,
    col: Col,
    size: f32,
}

#[derive(Default)]
pub struct Fx {
    parts: Vec<Particle>,
    texts: Vec<FloatText>,
    rng: Option<Rng>,
}

impl Fx {
    fn rng(&mut self) -> &mut Rng {
        self.rng.get_or_insert_with(|| Rng::new(0xF00D))
    }
    pub fn clear(&mut self) {
        self.parts.clear();
        self.texts.clear();
    }
    pub fn burst(&mut self, at: V2, col: Col, n: usize, speed: f32) {
        for _ in 0..n {
            let rng = self.rng();
            let a = rng.f() * std::f32::consts::TAU;
            let sp = speed * rng.range(0.4, 1.0);
            let size = rng.range(5.0, 11.0);
            let max = rng.range(0.35, 0.7);
            self.parts.push(Particle {
                pos: at,
                vel: v2(a.cos() * sp, a.sin() * sp),
                life: max,
                max,
                size,
                col,
                kind: PKind::Disc,
                rot: 0.0,
                vrot: 0.0,
                grav: 600.0,
            });
        }
    }
    pub fn stars(&mut self, at: V2, n: usize) {
        for _ in 0..n {
            let rng = self.rng();
            let a = rng.f() * std::f32::consts::TAU;
            let sp = rng.range(120.0, 320.0);
            let max = rng.range(0.5, 0.9);
            let rot = rng.f();
            let vrot = rng.range(-6.0, 6.0);
            self.parts.push(Particle {
                pos: at,
                vel: v2(a.cos() * sp, a.sin() * sp - 120.0),
                life: max,
                max,
                size: 14.0,
                col: hex(0xFFD23F),
                kind: PKind::Star,
                rot,
                vrot,
                grav: 500.0,
            });
        }
    }
    pub fn confetti(&mut self, at: V2, n: usize) {
        let cols = [hex(0xFF5D5D), hex(0xFFD23F), hex(0x4CC38A), hex(0x4FA7F2), hex(0xF27BC4), hex(0x9A6BEA)];
        for i in 0..n {
            let rng = self.rng();
            let a = -std::f32::consts::FRAC_PI_2 + rng.range(-1.2, 1.2);
            let sp = rng.range(250.0, 650.0);
            let max = rng.range(1.0, 1.8);
            let rot = rng.f() * 6.0;
            let vrot = rng.range(-10.0, 10.0);
            self.parts.push(Particle {
                pos: at,
                vel: v2(a.cos() * sp, a.sin() * sp),
                life: max,
                max,
                size: 9.0,
                col: cols[i % cols.len()],
                kind: PKind::Confetti,
                rot,
                vrot,
                grav: 520.0,
            });
        }
    }
    pub fn ring(&mut self, at: V2, col: Col, size: f32) {
        self.parts.push(Particle { pos: at, vel: v2(0.0, 0.0), life: 0.35, max: 0.35, size, col, kind: PKind::Ring, rot: 0.0, vrot: 0.0, grav: 0.0 });
    }
    pub fn glow(&mut self, at: V2, col: Col, size: f32) {
        self.parts.push(Particle { pos: at, vel: v2(0.0, 0.0), life: 0.3, max: 0.3, size, col, kind: PKind::Glow, rot: 0.0, vrot: 0.0, grav: 0.0 });
    }
    pub fn splash(&mut self, at: V2, n: usize) {
        for _ in 0..n {
            let rng = self.rng();
            let a = -std::f32::consts::FRAC_PI_2 + rng.range(-0.9, 0.9);
            let sp = rng.range(200.0, 480.0);
            let max = rng.range(0.4, 0.8);
            let size = rng.range(5.0, 10.0);
            self.parts.push(Particle {
                pos: at,
                vel: v2(a.cos() * sp, a.sin() * sp),
                life: max,
                max,
                size,
                col: hex(0xBFE9FF),
                kind: PKind::Drop,
                rot: 0.0,
                vrot: 0.0,
                grav: 1200.0,
            });
        }
    }
    pub fn text(&mut self, s: &str, at: V2, col: Col, size: f32) {
        self.texts.push(FloatText { text: s.to_string(), pos: at, life: 0.9, col, size });
    }

    pub fn update(&mut self, dt: f32) {
        for p in self.parts.iter_mut() {
            p.vel.y += p.grav * dt;
            p.pos += p.vel * dt;
            p.rot += p.vrot * dt;
            p.life -= dt;
        }
        self.parts.retain(|p| p.life > 0.0);
        for t in self.texts.iter_mut() {
            t.pos.y -= 70.0 * dt;
            t.life -= dt;
        }
        self.texts.retain(|t| t.life > 0.0);
    }

    pub fn draw(&self, r: &mut Renderer) {
        for p in &self.parts {
            let k = (p.life / p.max).clamp(0.0, 1.0);
            match p.kind {
                PKind::Disc => r.disc(p.pos.x, p.pos.y, p.size * (0.4 + 0.6 * k), p.col.a(k.min(1.0) * 1.5)),
                PKind::Glow => r.glow(p.pos.x, p.pos.y, p.size * (1.6 - k * 0.6), p.col.a(k)),
                PKind::Star => r.star(p.pos.x, p.pos.y, p.size * (0.5 + 0.5 * k), p.rot, p.col.a(k * 1.4), crate::art::INK.a(k), 2.0),
                PKind::Confetti => {
                    let w = p.size * (0.3 + 0.7 * (p.rot * 2.0).cos().abs());
                    r.rrect_rot(p.pos.x, p.pos.y, w, p.size * 1.6, 1.5, p.rot, p.col.a((k * 3.0).min(1.0)), Col::CLEAR, 0.0)
                }
                PKind::Ring => {
                    let rr = p.size * (1.4 - k * 0.8);
                    r.ring(p.pos.x, p.pos.y, rr, 6.0 * k + 1.0, p.col.a(k));
                }
                PKind::Drop => r.ellipse(p.pos.x, p.pos.y, p.size * 0.7, p.size, p.col.a(k * 1.5), Col::CLEAR, 0.0),
            }
        }
        for t in &self.texts {
            let k = (t.life / 0.9).clamp(0.0, 1.0);
            let pop = 1.0 + (1.0 - ease_out_back(((0.9 - t.life) / 0.2).min(1.0))) * 0.6;
            r.push();
            r.fade((k * 2.0).min(1.0));
            r.text_bold(&t.text, t.pos.x, t.pos.y, t.size * pop, Align::Center, t.col, crate::art::INK);
            r.pop();
        }
    }
}

// ------------------------------------------------------------------ round context

pub struct Ctx {
    pub w: f32,
    pub h: f32,
    /// Horizontal safe bounds (display cutout), in virtual units.
    pub safe_l: f32,
    pub safe_r: f32,
    /// Y below which gameplay should keep important objects clear of the HUD.
    pub hud_h: f32,
    /// Seconds of active play elapsed in this round.
    pub t: f32,
    pub time_left: f32,
    pub score: i64,
    /// Level-maintained counter for the "special" achievement (streaks etc.).
    pub special: i64,
    /// Set by a level when its special achievement condition is met.
    pub special_hit: bool,
    pub rng: Rng,
    pub fx: Fx,
    pub requests: i32,
    pub haptics: bool,
    /// Request to end the round now (2P knockouts, etc.).
    pub end_now: bool,
    /// 2P result: Some(0) left/blue wins, Some(1) right/red wins, Some(-1) draw.
    pub winner: Option<i8>,
    /// Ambient time for animation (runs during intro/countdown too).
    pub anim: f32,
    pub shake: f32,
    /// 1 right after a time bonus, fades to 0 (HUD flashes green).
    pub time_flash: f32,
    /// Two-player games: the right-hand (blue) side is played by the computer.
    pub cpu: bool,
    /// Practice round: no clock, no stars, nothing saved.
    pub practice: bool,
}

impl Ctx {
    pub fn new(w: f32, h: f32, seed: u64) -> Ctx {
        Ctx {
            w,
            h,
            safe_l: 0.0,
            safe_r: w,
            hud_h: 96.0,
            t: 0.0,
            time_left: 20.0,
            score: 0,
            special: 0,
            special_hit: false,
            rng: Rng::new(seed),
            fx: Fx::default(),
            requests: 0,
            haptics: true,
            end_now: false,
            winner: None,
            anim: 0.0,
            shake: 0.0,
            time_flash: 0.0,
            cpu: false,
            practice: false,
        }
    }
    pub fn sfx(&self, s: Sfx) {
        audio::play(s);
    }
    pub fn sfx_ex(&self, s: Sfx, vol: f32, pitch: f32) {
        audio::play_ex(s, vol, pitch, 0.0);
    }
    /// Pitch-varied effect so repeated actions don't sound robotic.
    pub fn sfx_var(&mut self, s: Sfx) {
        let p = self.rng.range(0.92, 1.1);
        audio::play_ex(s, 1.0, p, 0.0);
    }
    pub fn tick(&mut self) {
        if self.haptics {
            self.requests |= REQ_HAPTIC_TICK;
        }
    }
    pub fn confirm(&mut self) {
        if self.haptics {
            self.requests |= REQ_HAPTIC_CONFIRM;
        }
    }
    pub fn reject(&mut self) {
        if self.haptics {
            self.requests |= REQ_HAPTIC_REJECT;
        }
    }
    pub fn heavy(&mut self) {
        if self.haptics {
            self.requests |= REQ_HAPTIC_HEAVY;
        }
    }
    /// Adds score with a floating "+n" label.
    pub fn add_score(&mut self, n: i64, at: V2) {
        self.score += n;
        let label = if n >= 0 { format!("+{n}") } else { format!("{n}") };
        self.fx.text(&label, at, if n >= 0 { hex(0xFFE066) } else { hex(0xFF6B6B) }, 40.0);
    }
    /// Clock pickup: adds bonus seconds to the round with a floating label.
    pub fn add_time(&mut self, secs: f32, at: V2) {
        self.time_left += secs;
        self.time_flash = 1.0;
        self.fx.text(&format!("+{} sec", secs.round() as i32), at, hex(0x8BE04E), 44.0);
        self.fx.stars(at, 6);
        audio::play(Sfx::Clock);
        self.confirm();
    }
    pub fn mistake(&mut self, at: V2) {
        self.sfx(Sfx::Wrong);
        self.reject();
        self.fx.text("X", at, hex(0xFF5D5D), 52.0);
        self.shake = self.shake.max(0.25);
    }
    pub fn center_x(&self) -> f32 {
        (self.safe_l + self.safe_r) * 0.5
    }
    /// Left edge of the centred 1280x720 design stage (the classic 854x480 layout scaled 1.5x).
    pub fn x0(&self) -> f32 {
        self.center_x() - STAGE_W * 0.5
    }
    /// Stage x from a fraction of the stage width.
    pub fn sx(&self, f: f32) -> f32 {
        self.x0() + f * STAGE_W
    }
    /// Stage y from a fraction of the height.
    pub fn sy(&self, f: f32) -> f32 {
        f * self.h
    }
}

/// Width of the reference play stage in virtual units (height is 720).
pub const STAGE_W: f32 = 1280.0;

pub trait MiniGame: Send {
    /// Advance active gameplay (only called while the round clock runs).
    fn update(&mut self, c: &mut Ctx, dt: f32);
    fn draw(&mut self, r: &mut Renderer, c: &Ctx);
    fn touch(&mut self, c: &mut Ctx, t: &Touch);
    /// Ambient animation while the intro card / countdown is showing.
    fn idle(&mut self, _c: &mut Ctx, _dt: f32) {}
    /// Text shown in the score pill (defaults to the raw score).
    fn score_text(&self, c: &Ctx) -> String {
        c.score.to_string()
    }
}

// ------------------------------------------------------------------ registry

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AchKind {
    /// Lifetime total across rounds.
    Total,
    /// Reach in a single round.
    Single,
    /// Score at least `target` within the first `secs` seconds.
    Early(u32),
    /// Level-maintained `ctx.special` counter reaches `target`.
    Streak,
    /// Level sets `ctx.special_hit` itself.
    Flag,
}

#[derive(Clone, Copy, Debug)]
pub struct AchDef {
    pub kind: AchKind,
    pub target: i64,
    pub name: &'static str,
    pub desc: &'static str,
}

pub type Maker = fn(&mut Ctx) -> Box<dyn MiniGame>;
pub type IconFn = fn(&mut Renderer, f32, f32, f32, f32);

pub struct LevelDef {
    pub id: &'static str,
    pub title: &'static str,
    pub tip: &'static str,
    pub unit: &'static str,
    pub thresholds: [i64; 5],
    pub unlock: i32,
    pub duration: f32,
    pub two_player: bool,
    pub color: Col,
    pub ach: &'static [AchDef],
    pub make: Maker,
    pub icon: IconFn,
}

impl LevelDef {
    pub fn stars_for(&self, score: i64) -> u8 {
        self.thresholds.iter().filter(|&&t| score >= t).count() as u8
    }
}

const fn a(kind: AchKind, target: i64, name: &'static str, desc: &'static str) -> AchDef {
    AchDef { kind, target, name, desc }
}

/// All minigames in level-select order. Star thresholds, unlock costs, round lengths and
/// achievement targets follow the reverse-engineered balance of the classic game; names,
/// art, audio and code are new.
pub fn all() -> &'static [LevelDef] {
    &LEVELS
}

macro_rules! lvl {
    ($id:literal, $title:literal, $tip:literal, $unit:literal, $th:expr, $unlock:expr, $dur:expr, $two:expr, $col:expr, $ach:expr, $m:ident) => {
        LevelDef {
            id: $id,
            title: $title,
            tip: $tip,
            unit: $unit,
            thresholds: $th,
            unlock: $unlock,
            duration: $dur,
            two_player: $two,
            color: hex($col),
            ach: $ach,
            make: $m::make,
            icon: $m::icon,
        }
    };
}

use AchKind::*;
const NO_ACH: &[AchDef] = &[];

static LEVELS: [LevelDef; 28] = [
    lvl!("pop", "Pond Pop", "Pop every frog on the board!\nHelmets need extra taps.", "frogs", [20, 40, 70, 100, 150], 0, 20.0, false, 0x4CC38A,
        &[a(Total, 100, "Bubble Wrap", "Pop 100 frogs in total"), a(Total, 1000, "Pop Star", "Pop 1000 frogs in total"),
          a(Single, 170, "Pop Storm", "Pop 170 frogs in one round"), a(Early(5), 20, "Quick Fingers", "Pop 20 frogs in the first 5 seconds")], pop),
    lvl!("hop", "Lily Leap", "Hop as far as you can!\nUse the single and double jumps.", "metres", [50, 100, 150, 200, 250], 0, 20.0, false, 0x4FC1EC,
        &[a(Total, 1000, "Hopper", "Hop 1000 metres in total"), a(Total, 5000, "Marathon Hopper", "Hop 5000 metres in total"),
          a(Single, 300, "Long Legs", "Hop 300 metres in one round"), a(Streak, 200, "Dry Run", "Hop 200 pads without falling in")], hop),
    lvl!("paint", "Color Swap", "Tap frogs to flip their colour.\nMake them all the same!", "frogs", [35, 65, 100, 150, 200], 0, 20.0, false, 0xFF9F2E,
        &[a(Total, 500, "Painter", "Match 500 frogs in total"), a(Total, 2000, "Master Painter", "Match 2000 frogs in total"),
          a(Single, 250, "Speed Painter", "Match 250 frogs in one round"), a(Early(5), 20, "Splashdown", "Match 20 frogs in the first 5 seconds")], paint),
    lvl!("fly", "Bubble Pilot", "Collect the bubbles!\nTap left and right to fly.", "bubbles", [75, 100, 150, 250, 350], 0, 20.0, false, 0x7CC8FF,
        &[a(Total, 1000, "Pilot", "Collect 1000 bubbles in total"), a(Total, 5000, "Ace Pilot", "Collect 5000 bubbles in total"),
          a(Single, 450, "Bubble Storm", "Collect 450 bubbles in one round"), a(Flag, 100, "Moonshot", "Reach the moon with 100 bubbles or fewer")], fly),
    lvl!("astro", "Space Rocks", "Tap the asteroids in order,\nfrom smallest to biggest.", "rocks", [20, 30, 50, 75, 80], 10, 20.0, false, 0x6B5BD8,
        &[a(Total, 300, "Rock Collector", "Collect 300 rocks in total"), a(Total, 1000, "Asteroid Miner", "Collect 1000 rocks in total"),
          a(Single, 100, "Meteor Shower", "Collect 100 rocks in one round"), a(Early(5), 8, "Fast Counter", "Collect 8 rocks in the first 5 seconds")], astro),
    lvl!("feed", "Snack Belt", "Tap the button that matches\nthe fruit at the front of the belt.", "fruit", [20, 30, 40, 50, 70], 15, 20.0, false, 0xE8413C,
        &[a(Total, 500, "Snacker", "Feed 500 fruits in total"), a(Total, 2000, "Gourmet", "Feed 2000 fruits in total"),
          a(Single, 100, "Fast Food", "Feed 100 fruits in one round"), a(Early(1), 5, "Speed Eater", "Feed 5 fruits in the first second")], feed),
    lvl!("build", "Tower Toad", "Tap to drop each floor.\nBuild the tallest tower!", "rooms", [50, 75, 150, 250, 350], 20, 20.0, false, 0x9A6BEA,
        &[a(Total, 1000, "Builder", "Build 1000 rooms in total"), a(Total, 5000, "Architect", "Build 5000 rooms in total"),
          a(Single, 500, "Skyscraper", "Build 500 rooms in one round"), a(Flag, 10, "Perfect Ten", "Build 10 floors that keep all 8 rooms")], build),
    lvl!("treat", "Stump Hop", "Eat the treats!\nTap a nearby stump to hop.", "treats", [20, 40, 60, 100, 150], 25, 20.0, false, 0xA86E3F,
        &[a(Total, 300, "Sweet Tooth", "Eat 300 treats in total"), a(Total, 1500, "Candy Glutton", "Eat 1500 treats in total"),
          a(Single, 170, "Sugar Peak", "Eat 170 treats in one round"), a(Early(3), 9, "Sweet Gobbler", "Eat 9 treats in the first 3 seconds")], treat),
    lvl!("leap", "Sky Spring", "Climb as high as you can!\nTap left or right to leap.", "metres", [20, 40, 60, 100, 150], 30, 20.0, false, 0x8566E0,
        &[a(Total, 1000, "Climber", "Climb 1000 metres in total"), a(Total, 3000, "Cloud Walker", "Climb 3000 metres in total"),
          a(Single, 170, "Sky High", "Climb 170 metres in one round"), a(Streak, 100, "Sure Footed", "Land 100 leaps in a row")], leap),
    lvl!("drive", "Skate Rush", "Skate as far as you can!\nTap up and down to switch lanes.", "metres", [50, 100, 150, 200, 250], 35, 20.0, false, 0x3DB37A,
        &[a(Total, 1000, "Skater", "Skate 1000 metres in total"), a(Total, 5000, "Street Legend", "Skate 5000 metres in total"),
          a(Single, 270, "Zoom", "Skate 270 metres in one round"), a(Flag, 100, "Clean Run", "Skate 100 metres without a crash")], drive),
    lvl!("sumo", "Belly Bump", "Two players! Tap your frog\nto push the other out.", "", [0, 0, 0, 0, 0], 40, 10.0, true, 0xC0262D, NO_ACH, sumo),
    lvl!("twin", "Twin Pop", "Tap a frog, then tap its\ntwin to pop them both!", "frogs", [75, 150, 200, 250, 350], 40, 20.0, false, 0xF2A33D,
        &[a(Total, 1000, "Double Trouble", "Pop 1000 frogs in total"), a(Total, 5000, "Twin Master", "Pop 5000 frogs in total"),
          a(Single, 400, "Double Rainbow", "Pop 400 frogs in one round"), a(Early(5), 42, "Twin Frenzy", "Pop 42 frogs in the first 5 seconds")], twin),
    lvl!("padcatch", "Pad Catch", "Steer the lily pad so the\ncandy bounces to the frog!", "candies", [35, 65, 100, 150, 200], 44, 20.0, false, 0x56C2E8,
        &[a(Total, 1000, "Sweet Catcher", "Feed 1000 candies in total"), a(Total, 3000, "Candy Captain", "Feed 3000 candies in total"),
          a(Single, 300, "Sugar Rush", "Feed 300 candies in one round"), a(Flag, 50, "Dry Feet", "Feed 50 candies without dropping one")], padcatch),
    lvl!("bounce", "Berry Bounce", "Tap the globe to bounce\nand grab the fruit!", "berries", [20, 30, 40, 50, 60], 48, 20.0, false, 0x7BC95A,
        &[a(Total, 1000, "Berry Picker", "Collect 1000 fruits in total"), a(Total, 2000, "Orchard Ace", "Collect 2000 fruits in total"),
          a(Single, 100, "Juicy Round", "Collect 100 fruits in one round"), a(Streak, 30, "Sky Hopper", "30 fruits without touching the ground")], bounce),
    lvl!("jet", "Jet Frog", "Hold to fly up, let go to fall.\nGrab fuel, dodge rocks!", "fuel", [75, 150, 300, 500, 750], 52, 20.0, false, 0x5B4FC4,
        &[a(Total, 5000, "Space Cadet", "Collect 5000 fuel cells in total"), a(Total, 10000, "Star Pilot", "Collect 10000 fuel cells in total"),
          a(Single, 1200, "Fuel Hoarder", "Collect 1200 fuel cells in one round"), a(Flag, 100, "Running on Fumes", "Reach the space station with under 100 fuel")], jet),
    lvl!("balloon", "Balloon Duel", "Two players! Drag down on\nyour side to pump faster!", "", [0, 0, 0, 0, 0], 52, 0.0, true, 0xF0605D, NO_ACH, balloon),
    lvl!("ballswap", "Ball Swap", "Tap frogs to hop onto the free\nball. Match every colour!", "frogs", [10, 25, 50, 75, 100], 56, 20.0, false, 0x3D9BE0,
        &[a(Total, 1000, "Color Matcher", "Match 1000 frogs in total"), a(Total, 2000, "Palette Pro", "Match 2000 frogs in total"),
          a(Single, 175, "Quick Matcher", "Match 175 frogs in one round"), a(Early(5), 10, "Nimble Feet", "Match 10 frogs in the first 5 seconds")], ballswap),
    lvl!("goal", "Frog Soccer", "Two players! Drag your frog\nto kick the ball into the goal.", "", [0, 0, 0, 0, 0], 56, 0.0, true, 0x5DBB55, NO_ACH, goal),
    lvl!("giftwrap", "Gift Wrap", "Tap the toy that the\nhighlighted box is asking for!", "gifts", [35, 50, 70, 90, 120], 60, 20.0, false, 0xF27BC4,
        &[a(Total, 1000, "Gift Wrapper", "Wrap 1000 gifts in total"), a(Total, 3000, "Master Wrapper", "Wrap 3000 gifts in total"),
          a(Single, 175, "Quick Wrapper", "Wrap 175 gifts in one round"), a(Flag, 16, "Rocket Surprise", "Wrap the rocket without a single miss")], giftwrap),
    lvl!("bandit", "Bandit Pop", "Pop the masked bandits!\nLeave the window props alone.", "bandits", [50, 100, 150, 200, 250], 64, 20.0, false, 0xC0262D,
        &[a(Total, 1500, "Sheriff", "Pop 1500 bandits in total"), a(Total, 3000, "Marshal", "Pop 3000 bandits in total"),
          a(Single, 350, "Sharp Shooter", "Pop 350 bandits in one round"), a(Early(5), 15, "Fast Hands", "Pop 15 bandits in the first 5 seconds")], bandit),
    lvl!("dive", "Deep Dive", "Tap to swim and grab the\nsweets. Avoid the pufferfish!", "sweets", [50, 100, 150, 200, 250], 68, 20.0, false, 0x1F78B4,
        &[a(Total, 2000, "Diver", "Catch 2000 sweets in total"), a(Total, 8000, "Deep Diver", "Catch 8000 sweets in total"),
          a(Single, 400, "Reef Raider", "Catch 400 sweets in one round"), a(Flag, 100, "Careful Diver", "Catch 100 sweets without touching the pufferfish")], dive),
    lvl!("volley", "Frog Volley", "Two players! Slide your frog\nto keep the ball off your sand.", "", [0, 0, 0, 0, 0], 72, 0.0, true, 0xE5C07A, NO_ACH, volley),
    lvl!("connect", "Link Up", "Drag through frogs of the\nsame colour to link them!", "frogs", [50, 100, 150, 200, 250], 72, 20.0, false, 0xB07A45,
        &[a(Total, 2000, "Linker", "Link 2000 frogs in total"), a(Total, 4000, "Chain Master", "Link 4000 frogs in total"),
          a(Single, 300, "Long Chains", "Link 300 frogs in one round"), a(Early(5), 20, "Lightning Links", "Link 20 frogs in the first 5 seconds")], connect),
    lvl!("ride", "Wheelie Run", "Tap to jump, tap again to\ndouble jump. Grab the sweets!", "sweets", [75, 100, 150, 200, 250], 75, 20.0, false, 0xFF9FD0,
        &[a(Total, 2000, "Rider", "Collect 2000 sweets in total"), a(Total, 5000, "Road King", "Collect 5000 sweets in total"),
          a(Single, 275, "Sweet Ride", "Collect 275 sweets in one round"), a(Flag, 1, "Candy Land", "Reach Candy Land without falling")], ride),
    lvl!("sort", "Shape Sort", "Tap the shadow that matches\nthe toy the frog is holding.", "toys", [25, 50, 75, 100, 125], 78, 20.0, false, 0xA86E3F,
        &[a(Total, 1000, "Sorter", "Sort 1000 toys in total"), a(Total, 3000, "Neat Freak", "Sort 3000 toys in total"),
          a(Single, 150, "Quick Sorter", "Sort 150 toys in one round"), a(Flag, 1, "Snow Day", "Place the snowman without a single miss")], sort),
    lvl!("slide", "Fruit Slide", "Swipe frogs to slide them\ninto fruit of their colour!", "fruit", [75, 100, 125, 150, 175], 80, 20.0, false, 0xE5C88C,
        &[a(Total, 2000, "Slider", "Eat 2000 fruits in total"), a(Total, 4000, "Ice Master", "Eat 4000 fruits in total"),
          a(Single, 200, "Fruit Rush", "Eat 200 fruits in one round"), a(Early(3), 5, "Happy Sliders", "Eat 5 fruits in the first 3 seconds")], slide),
    lvl!("quickdraw", "Quick Draw", "Two players! When START lights\nup, tap your frog first!", "", [0, 0, 0, 0, 0], 80, 0.0, true, 0x5A2B1F, NO_ACH, quickdraw),
    lvl!("cake", "Cake Duel", "Party game for two! Tap your\nfrog to lick the cake first.", "", [0, 0, 0, 0, 0], 0, 0.0, true, 0xF27BC4, NO_ACH, cake),
];
