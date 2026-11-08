//! Scene flow: splash, main menu, level select, play (intro, countdown, run, time up,
//! bonus time), results, achievements, options and credits.
#![allow(clippy::too_many_arguments)]

use crate::art::{self, Pose, INK};
use crate::audio::{self, Sfx, Track};
use crate::input::{Phase, Touch};
use crate::levels::{self, AchKind, Ctx, LevelDef, MiniGame};
use crate::math::*;
use crate::platform::*;
use crate::profile::Profile;
use crate::render::{Align, Renderer};
use crate::ui::{self, Button, Icon};
use std::f32::consts::{PI, TAU};

pub const RANKS: [(i64, &str); 10] = [
    (0, "Tadpole"),
    (20, "Froglet"),
    (60, "Puddle Jumper"),
    (120, "Pond Scout"),
    (200, "Lily Ranger"),
    (300, "Marsh Knight"),
    (450, "Reed Ninja"),
    (600, "Bog Captain"),
    (800, "Swamp Sage"),
    (1200, "Pond Legend"),
];

pub fn rank_of(xp: i64) -> usize {
    RANKS.iter().rposition(|(t, _)| xp >= *t).unwrap_or(0)
}
/// Progress 0..1 towards the next rank.
pub fn rank_progress(xp: i64) -> f32 {
    let i = rank_of(xp);
    if i + 1 >= RANKS.len() {
        return 1.0;
    }
    let (a, b) = (RANKS[i].0, RANKS[i + 1].0);
    ((xp - a) as f32 / (b - a) as f32).clamp(0.0, 1.0)
}

const GREEN: Col = hex(0x4CC38A);
const ORANGE: Col = hex(0xFF9F2E);
const BLUE: Col = hex(0x4FA7F2);
const RED: Col = hex(0xF0605D);
const PURPLE: Col = hex(0x9A6BEA);
const GOLD: Col = hex(0xFFD23F);
const PANEL: Col = hex(0xFFF4D6);

// ------------------------------------------------------------------ shared state

pub struct Toast {
    title: String,
    text: String,
    life: f32,
    icon: Icon,
}

pub struct Shared {
    pub profile: Profile,
    pub time: f32,
    pub w: f32,
    pub h: f32,
    pub safe_l: f32,
    pub safe_r: f32,
    pub requests: i32,
    pub rng: Rng,
    toasts: Vec<Toast>,
    /// UTC day number, used by the daily bonus and the daily challenge.
    pub today: i64,
    /// Installed version name, from Kotlin.
    pub version: String,
    /// A newer release found on GitHub, if any.
    pub update: Option<String>,
}

impl Shared {
    fn toast(&mut self, title: &str, text: &str, icon: Icon) {
        self.toasts.push(Toast { title: title.into(), text: text.into(), life: 3.2, icon });
    }
    fn haptic(&mut self, flag: i32) {
        if self.profile.haptics {
            self.requests |= flag;
        }
    }
    fn levels(&self) -> &'static [LevelDef] {
        levels::all()
    }
    fn unlocked(&self, idx: usize) -> bool {
        self.profile.total_stars() >= self.levels()[idx].unlock
    }
    /// Today's challenge: one unlocked single-player game and a target (2nd..4th star).
    fn daily(&self) -> Option<(usize, i64)> {
        let pool: Vec<usize> = (0..self.levels().len()).filter(|&i| !self.levels()[i].two_player && self.unlocked(i)).collect();
        if pool.is_empty() {
            return None;
        }
        let mut rng = Rng::new((self.today as u64).wrapping_mul(7919) + 13);
        rng.next_u64();
        let idx = pool[rng.below(pool.len())];
        let def = &self.levels()[idx];
        Some((idx, def.thresholds[1 + (self.today.rem_euclid(3)) as usize]))
    }
    fn daily_done_today(&self) -> bool {
        self.profile.daily_done == self.today
    }
}

/// How a round is played.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Normal,
    /// No clock, no stars, nothing saved.
    Practice,
    /// Two-player game where the computer plays blue.
    Cpu,
    /// Today's daily challenge with its target score.
    Daily(i64),
}

#[derive(Clone)]
enum Nav {
    Menu,
    Select(Option<usize>),
    /// level index, mode, skip the intro card
    Play(usize, Mode, bool),
    Score(RoundResult),
    Achievements,
    Options,
    Credits,
}

#[derive(Clone, Copy, PartialEq)]
enum ModalAct {
    Close,
    Exit,
    Reset,
}

struct Modal {
    title: String,
    text: String,
    buttons: Vec<(Button, ModalAct)>,
    t: f32,
}

enum Scene {
    Splash(f32),
    Menu(MenuScene),
    Select(SelectScene),
    Play(Box<PlayScene>),
    Score(Box<ScoreScene>),
    Achievements(AchScene),
    Options(OptionsScene),
    Credits(CreditsScene),
}

pub struct Game {
    pub r: Renderer,
    pub s: Shared,
    scene: Scene,
    pending: Option<Nav>,
    fade: f32,
    fading_out: bool,
    modal: Option<Modal>,
    last_ns: i64,
    gl_ok: bool,
    insets_px: (i32, i32),
}

impl Game {
    pub fn new(r: Renderer, files_dir: &str) -> Game {
        let mut profile = Profile::load(files_dir);
        let w = r.w;
        let h = r.h;
        audio::set_music_enabled(profile.music);
        audio::set_sfx_enabled(profile.sfx);
        art::set_color_marks(profile.color_marks);
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
        let mut toasts = Vec::new();
        // daily visit bonus: streak days give 1..5 clocks
        let day = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) / 86400) as i64;
        if day != profile.last_day {
            profile.streak = if day == profile.last_day + 1 { (profile.streak + 1).min(5) } else { 1 };
            if profile.last_day != 0 {
                let gift = profile.streak;
                profile.clocks += gift;
                toasts.push(Toast {
                    title: format!("Day {} bonus!", profile.streak),
                    text: format!("+{gift} clock{}", if gift > 1 { "s" } else { "" }),
                    life: 4.0,
                    icon: Icon::Clock,
                });
            }
            profile.last_day = day;
            profile.save();
        }
        Game {
            r,
            s: Shared { profile, time: 0.0, w, h, safe_l: 0.0, safe_r: w, requests: 0, rng: Rng::new(seed), toasts, today: day, version: String::new(), update: None },
            scene: Scene::Splash(0.0),
            pending: None,
            fade: 0.0,
            fading_out: false,
            modal: None,
            last_ns: 0,
            gl_ok: false,
            insets_px: (0, 0),
        }
    }

    pub fn gl_created(&mut self) {
        match self.r.init_gl() {
            Ok(()) => self.gl_ok = true,
            Err(e) => {
                crate::logi!("GL init failed: {e}");
                self.gl_ok = false;
            }
        }
    }

    pub fn resize(&mut self, w: i32, h: i32) {
        self.r.resize(w, h);
        self.s.w = self.r.w;
        self.s.h = self.r.h;
        self.apply_insets();
    }

    /// Insets arrive in physical pixels and may come before or after the surface size,
    /// so the raw values are kept and converted whenever either changes.
    pub fn set_insets(&mut self, l: i32, _t: i32, r: i32, _b: i32) {
        self.insets_px = (l.max(0), r.max(0));
        self.apply_insets();
    }

    fn apply_insets(&mut self) {
        let k = self.r.h / self.r.px_h.max(1.0);
        let (l, r) = self.insets_px;
        self.s.safe_l = l as f32 * k;
        self.s.safe_r = self.r.w - r as f32 * k;
        if self.s.safe_r - self.s.safe_l < self.r.w * 0.5 {
            self.s.safe_l = 0.0;
            self.s.safe_r = self.r.w;
        }
        self.layout();
    }

    /// Recompute widget positions after a size or inset change.
    fn layout(&mut self) {
        let s = &self.s;
        match &mut self.scene {
            Scene::Menu(m) => m.layout(s),
            Scene::Select(m) => m.layout(s),
            Scene::Play(p) => p.layout(s),
            Scene::Score(p) => p.layout(s),
            Scene::Achievements(a) => a.layout(s),
            Scene::Options(o) => o.layout(s),
            Scene::Credits(c) => c.layout(s),
            Scene::Splash(_) => {}
        }
    }

    pub fn pause(&mut self) {
        if let Scene::Play(p) = &mut self.scene {
            p.app_paused();
        }
        audio::music(None);
        self.s.profile.save();
    }

    pub fn set_version(&mut self, v: &str) {
        self.s.version = v.to_string();
    }

    /// Kotlin reports a newer release (empty string = none).
    pub fn set_update(&mut self, v: &str) {
        self.s.update = if v.is_empty() || !self.s.profile.update_check { None } else { Some(v.to_string()) };
    }

    pub fn resume(&mut self) {
        self.last_ns = 0;
        if self.s.profile.update_check {
            self.s.requests |= REQ_CHECK_UPDATE;
        }
        let track = match &self.scene {
            Scene::Play(_) => Some(Track::Level),
            Scene::Splash(_) => None,
            _ => Some(Track::Menu),
        };
        audio::music(track);
    }

    pub fn save(&mut self) {
        self.s.profile.save();
    }

    /// Debug hook: open a minigame directly (ignores locks).
    pub fn debug_start(&mut self, spec: &str) {
        let (id, mode) = spec.split_once(':').unwrap_or((spec, ""));
        if let Some(i) = self.s.levels().iter().position(|d| d.id == id) {
            self.modal = None;
            self.pending = None;
            self.fading_out = false;
            let (mode, skip) = match mode {
                "practice" => (Mode::Practice, true),
                "cpu" => (Mode::Cpu, true),
                "daily" => (Mode::Daily(self.s.levels()[i].thresholds[1]), false),
                _ => (Mode::Normal, false),
            };
            self.enter(Nav::Play(i, mode, skip));
        }
    }

    pub fn back(&mut self) {
        if let Some(m) = &self.modal {
            if m.buttons.iter().any(|(_, a)| *a == ModalAct::Close) {
                self.modal = None;
            }
            return;
        }
        if self.pending.is_some() {
            return;
        }
        let nav = match &mut self.scene {
            Scene::Splash(_) => None,
            Scene::Menu(_) => {
                self.modal = Some(make_modal(
                    &self.s,
                    "Leaving already?",
                    "The frogs will miss you.\nQuit Ribbit Revived?",
                    vec![("Stay", Icon::None, GREEN, ModalAct::Close), ("Quit", Icon::None, RED, ModalAct::Exit)],
                ));
                None
            }
            Scene::Select(_) | Scene::Achievements(_) | Scene::Options(_) => Some(Nav::Menu),
            Scene::Credits(_) => Some(Nav::Options),
            Scene::Play(p) => p.back(&mut self.s),
            Scene::Score(sc) => Some(Nav::Select(Some(sc.res.idx))),
        };
        if let Some(n) = nav {
            self.go(n);
        }
    }

    fn go(&mut self, n: Nav) {
        if self.pending.is_none() {
            self.pending = Some(n);
            self.fading_out = true;
        }
    }

    fn enter(&mut self, n: Nav) {
        let s = &mut self.s;
        self.scene = match n {
            Nav::Menu => {
                audio::music(Some(Track::Menu));
                Scene::Menu(MenuScene::new(s))
            }
            Nav::Select(focus) => {
                audio::music(Some(Track::Menu));
                Scene::Select(SelectScene::new(s, focus))
            }
            Nav::Play(i, mode, skip) => {
                audio::music(Some(Track::Level));
                Scene::Play(Box::new(PlayScene::new(s, i, mode, skip)))
            }
            Nav::Score(res) => {
                audio::music(Some(Track::Menu));
                Scene::Score(Box::new(ScoreScene::new(s, res)))
            }
            Nav::Achievements => Scene::Achievements(AchScene::new(s)),
            Nav::Options => Scene::Options(OptionsScene::new(s)),
            Nav::Credits => Scene::Credits(CreditsScene::new(s)),
        };
    }

    pub fn touch(&mut self, phase: i32, id: i32, x: f32, y: f32) {
        let t = Touch { phase: Phase::from_i32(phase), id, pos: self.r.to_virtual(x, y) };
        if self.pending.is_some() {
            return;
        }
        if let Some(m) = &mut self.modal {
            let mut act = None;
            for (b, a) in m.buttons.iter_mut() {
                if b.touch(&t) {
                    act = Some(*a);
                }
            }
            if let Some(a) = act {
                self.modal = None;
                match a {
                    ModalAct::Close => {}
                    ModalAct::Exit => {
                        self.s.profile.save();
                        self.s.requests |= REQ_EXIT;
                    }
                    ModalAct::Reset => {
                        self.s.profile.reset();
                        self.s.profile.save();
                        self.s.toast("Fresh start!", "All progress was reset", Icon::Retry);
                        self.go(Nav::Menu);
                    }
                }
            }
            return;
        }
        let nav = match &mut self.scene {
            Scene::Splash(t0) => {
                if t.phase == Phase::Down && *t0 > 0.6 {
                    *t0 = 99.0;
                }
                None
            }
            Scene::Menu(m) => m.touch(&mut self.s, &t),
            Scene::Select(m) => m.touch(&mut self.s, &t),
            Scene::Play(p) => p.touch(&mut self.s, &t),
            Scene::Score(p) => p.touch(&mut self.s, &t),
            Scene::Achievements(a) => a.touch(&mut self.s, &t),
            Scene::Options(o) => {
                let (nav, reset) = o.touch(&mut self.s, &t);
                if reset {
                    self.modal = Some(make_modal(
                        &self.s,
                        "Reset progress?",
                        "Stars, best scores, ranks and\nachievements will be erased.",
                        vec![("Keep", Icon::None, GREEN, ModalAct::Close), ("Reset", Icon::None, RED, ModalAct::Reset)],
                    ));
                }
                nav
            }
            Scene::Credits(c) => c.touch(&mut self.s, &t),
        };
        if let Some(n) = nav {
            self.go(n);
        }
    }

    pub fn frame(&mut self, now_ns: i64) -> i32 {
        let dt = if self.last_ns == 0 { 1.0 / 60.0 } else { ((now_ns - self.last_ns) as f64 / 1e9) as f32 };
        self.last_ns = now_ns;
        let dt = dt.clamp(0.0, 0.05);
        self.update(dt);
        if self.gl_ok {
            self.draw();
        }
        let req = self.s.requests;
        self.s.requests = 0;
        if self.s.profile.dirty {
            // saves are cheap (a few KB) but avoid doing them every frame
            if let Scene::Play(p) = &self.scene {
                if p.running() {
                    return req;
                }
            }
            self.s.profile.save();
        }
        req
    }

    fn update(&mut self, dt: f32) {
        self.s.time += dt;
        for t in self.s.toasts.iter_mut().take(1) {
            t.life -= dt;
        }
        self.s.toasts.retain(|t| t.life > 0.0);
        if let Some(m) = &mut self.modal {
            m.t += dt;
            for (b, _) in m.buttons.iter_mut() {
                b.update(dt);
            }
        }
        // transitions
        if self.fading_out {
            self.fade += dt / 0.2;
            if self.fade >= 1.0 {
                self.fade = 1.0;
                self.fading_out = false;
                if let Some(n) = self.pending.take() {
                    self.enter(n);
                }
            }
        } else if self.fade > 0.0 {
            self.fade = (self.fade - dt / 0.25).max(0.0);
        }
        let nav = match &mut self.scene {
            Scene::Splash(t) => {
                *t += dt;
                if *t > 2.4 {
                    Some(Nav::Menu)
                } else {
                    None
                }
            }
            Scene::Menu(m) => m.update(&mut self.s, dt),
            Scene::Select(m) => m.update(&mut self.s, dt),
            Scene::Play(p) => {
                let modal_open = self.modal.is_some();
                if modal_open {
                    None
                } else {
                    p.update(&mut self.s, dt)
                }
            }
            Scene::Score(p) => p.update(&mut self.s, dt),
            Scene::Achievements(a) => a.update(&mut self.s, dt),
            Scene::Options(o) => o.update(&mut self.s, dt),
            Scene::Credits(c) => c.update(&mut self.s, dt),
        };
        if let Some(n) = nav {
            self.go(n);
        }
    }

    fn draw(&mut self) {
        let r = &mut self.r;
        let s = &self.s;
        r.begin(hex(0x1F7A5A));
        match &mut self.scene {
            Scene::Splash(t) => draw_splash(r, *t),
            Scene::Menu(m) => m.draw(r, s),
            Scene::Select(m) => m.draw(r, s),
            Scene::Play(p) => p.draw(r, s),
            Scene::Score(p) => p.draw(r, s),
            Scene::Achievements(a) => a.draw(r, s),
            Scene::Options(o) => o.draw(r, s),
            Scene::Credits(c) => c.draw(r, s),
        }
        // toasts
        if let Some(t) = s.toasts.first() {
            let k = ease_out_back(((3.2 - t.life) / 0.35).min(1.0)) * ((t.life / 0.3).min(1.0));
            let y = -60.0 + k * 130.0;
            let x = (s.safe_l + s.safe_r) * 0.5;
            let w = (r.text_width(&t.title, 30.0).max(r.text_width(&t.text, 24.0)) + 150.0).max(380.0);
            r.rrect(x, y + 5.0, w, 96.0, 30.0, Col(0, 0, 0, 70), Col::CLEAR, 0.0);
            r.rrect_grad(x, y, w, 96.0, 30.0, hex(0x2F3F8F), hex(0x23306E), INK, 4.0);
            ui::draw_icon(r, t.icon, x - w * 0.5 + 52.0, y, 52.0, GOLD);
            r.text_bold(&t.title, x + 30.0, y - 16.0, 30.0, Align::Center, GOLD, INK);
            r.text(&t.text, x + 30.0, y + 20.0, 24.0, Align::Center, Col::WHITE);
        }
        if let Some(m) = &self.modal {
            draw_modal(r, s, m);
        }
        if self.fade > 0.0 {
            draw_wipe(r, self.fade);
        }
        r.end();
    }
}

// ------------------------------------------------------------------ helpers

fn make_modal(s: &Shared, title: &str, text: &str, btns: Vec<(&str, Icon, Col, ModalAct)>) -> Modal {
    let cx = (s.safe_l + s.safe_r) * 0.5;
    let n = btns.len() as f32;
    let bw = 220.0;
    let gap = 30.0;
    let total = n * bw + (n - 1.0) * gap;
    let buttons = btns
        .into_iter()
        .enumerate()
        .map(|(i, (l, ic, c, a))| (Button::new(cx - total * 0.5 + bw * 0.5 + i as f32 * (bw + gap), s.h * 0.5 + 110.0, bw, 84.0, l, ic, c), a))
        .collect();
    Modal { title: title.into(), text: text.into(), buttons, t: 0.0 }
}

fn draw_modal(r: &mut Renderer, s: &Shared, m: &Modal) {
    let k = ease_out_back((m.t / 0.3).min(1.0));
    ui::dim(r, 0.55 * (m.t / 0.2).min(1.0));
    let cx = (s.safe_l + s.safe_r) * 0.5;
    let cy = s.h * 0.5;
    r.push();
    r.translate(cx, cy);
    r.scale(k, k);
    r.translate(-cx, -cy);
    ui::panel(r, cx, cy, 700.0, 420.0, PANEL);
    r.text_bold(&m.title, cx, cy - 130.0, 50.0, Align::Center, ORANGE, INK);
    r.text(&m.text, cx, cy - 20.0, 32.0, Align::Center, INK);
    for (b, _) in &m.buttons {
        b.draw(r);
    }
    r.pop();
}

/// Screen transition: a dark pond-green iris closing to a frog silhouette and opening again.
fn draw_wipe(r: &mut Renderer, k: f32) {
    let (w, h) = (r.w, r.h);
    let a = ease_in_out(k);
    r.rect(0.0, 0.0, w, h, hex(0x12382A).a(a));
    if a > 0.6 {
        let s = (a - 0.6) / 0.4;
        r.push();
        r.fade(s);
        art::frog(r, w * 0.5, h * 0.5 - 10.0, 110.0, art::EMERALD, &Pose { happy_eyes: true, sprout: true, ..Pose::default() });
        r.pop();
    }
}

fn draw_splash(r: &mut Renderer, t: f32) {
    let (w, h) = (r.w, r.h);
    r.grad_rect(0.0, 0.0, w, h, hex(0x1F7A5A), hex(0x12382A));
    let k = ease_out_elastic((t / 1.2).min(1.0));
    for i in 0..3 {
        let rr = 120.0 + i as f32 * 90.0 + (t * 60.0) % 90.0;
        r.ring(w * 0.5, h * 0.45, rr, 4.0, Col::WHITE.a(0.08 * (1.0 - (rr - 120.0) / 270.0)));
    }
    let pose = Pose { blink: if (1.4..1.55).contains(&t) { 1.0 } else { 0.0 }, sprout: true, mouth: if t > 1.0 && t < 1.4 { 0.6 } else { 0.0 }, ..Pose::default() };
    art::frog(r, w * 0.5, h * 0.42, 220.0 * k, art::EMERALD, &pose);
    let a = ((t - 0.6) / 0.4).clamp(0.0, 1.0);
    r.push();
    r.fade(a);
    r.text_bold("RIBBIT REVIVED", w * 0.5, h * 0.76, 64.0, Align::Center, hex(0xB4F07A), INK);
    r.text("made by riyadm", w * 0.5, h * 0.86, 28.0, Align::Center, Col::WHITE.a(0.75));
    r.pop();
}

/// The pond backdrop shared by the menu screens.
fn draw_pond_bg(r: &mut Renderer, t: f32) {
    let (w, h) = (r.w, r.h);
    art::sky(r, hex(0x6FD3F7), hex(0xCFF3FF));
    r.glow(w * 0.5, 96.0, 160.0, hex(0xFFF3B0).a(0.9));
    r.circle(w * 0.5, 96.0, 50.0, hex(0xFFE27A), hex(0xFFC93C), 4.0);
    for i in 0..4 {
        let x = ((i as f32 * 420.0 + t * (12.0 + i as f32 * 4.0)) % (w + 400.0)) - 200.0;
        art::cloud(r, x, 90.0 + (i % 2) as f32 * 70.0, 0.8 + (i % 3) as f32 * 0.2, Col::WHITE.a(0.9));
    }
    art::hills(r, h * 0.62, hex(0x8BD66B), 70.0, t * 4.0);
    art::hills(r, h * 0.68, hex(0x5DBB55), 50.0, 80.0 - t * 7.0);
    art::water(r, h * 0.74, t, hex(0x48B7E0), hex(0x1E78B4));
}

fn star_row(r: &mut Renderer, x: f32, y: f32, n: u8, size: f32, gap: f32) {
    for i in 0..5 {
        art::star_icon(r, x + (i as f32 - 2.0) * gap, y, size, i < n as usize);
    }
}

fn back_button(s: &Shared) -> Button {
    Button::round(s.safe_l + 70.0, 66.0, 92.0, Icon::Back, ORANGE)
}

// ------------------------------------------------------------------ main menu

struct MenuScene {
    play: Button,
    daily: Button,
    trophy: Button,
    gear: Button,
    update: Button,
    frog_tap: f32,
    frog_mood: f32,
    hearts: Vec<(V2, f32)>,
    t: f32,
}

impl MenuScene {
    fn new(s: &Shared) -> MenuScene {
        let mut m = MenuScene {
            play: Button::new(0.0, 0.0, 330.0, 110.0, "PLAY", Icon::Play, ORANGE),
            daily: Button::round(0.0, 0.0, 96.0, Icon::Calendar, GREEN),
            trophy: Button::round(0.0, 0.0, 96.0, Icon::Trophy, PURPLE),
            gear: Button::round(0.0, 0.0, 96.0, Icon::Gear, BLUE),
            update: Button::new(0.0, 0.0, 330.0, 84.0, "Update", Icon::Down, GREEN),
            frog_tap: 9.0,
            frog_mood: 0.0,
            hearts: Vec::new(),
            t: 0.0,
        };
        m.play.text_size = 48.0;
        m.update.text_size = 30.0;
        m.update.visible = s.update.is_some();
        m.layout(s);
        m
    }
    fn layout(&mut self, s: &Shared) {
        let cx = (s.safe_l + s.safe_r) * 0.5;
        let right = cx + (s.safe_r - s.safe_l) * 0.22;
        self.play.pos = v2(right, s.h * 0.56);
        self.trophy.pos = v2(right - 115.0, s.h * 0.56 + 140.0);
        self.daily.pos = v2(right, s.h * 0.56 + 140.0);
        self.gear.pos = v2(right + 115.0, s.h * 0.56 + 140.0);
        self.update.pos = v2(s.safe_l + 200.0, 64.0);
    }
    fn frog_pos(&self, s: &Shared) -> V2 {
        v2(s.safe_l + (s.safe_r - s.safe_l) * 0.27, s.h * 0.62)
    }
    fn touch(&mut self, s: &mut Shared, t: &Touch) -> Option<Nav> {
        if self.play.touch(t) {
            return Some(Nav::Select(None));
        }
        if self.trophy.touch(t) {
            return Some(Nav::Achievements);
        }
        if self.daily.touch(t) {
            if let Some((i, target)) = s.daily() {
                if s.daily_done_today() {
                    s.toasts.clear();
                    s.toast("Done for today!", "Replay it for fun; a new one comes tomorrow", Icon::Calendar);
                }
                return Some(Nav::Play(i, Mode::Daily(target), false));
            }
        }
        if self.gear.touch(t) {
            return Some(Nav::Options);
        }
        if self.update.touch(t) {
            s.requests |= REQ_OPEN_UPDATE;
            s.haptic(REQ_HAPTIC_CONFIRM);
        }
        if t.phase == Phase::Down && t.pos.dist(self.frog_pos(s) - v2(0.0, 80.0)) < 150.0 {
            self.frog_tap = 0.0;
            self.frog_mood = (self.frog_mood + 0.25).min(1.0);
            audio::play_ex(Sfx::Ribbit, 1.0, s.rng.range(0.9, 1.15), 0.0);
            s.haptic(REQ_HAPTIC_TICK);
            let p = self.frog_pos(s) + v2(s.rng.range(-60.0, 60.0), -170.0);
            self.hearts.push((p, 0.0));
        }
        None
    }
    fn update(&mut self, s: &mut Shared, dt: f32) -> Option<Nav> {
        self.t += dt;
        self.frog_tap += dt;
        self.update.visible = s.update.is_some();
        if let Some(v) = &s.update {
            self.update.label = format!("Update {v}");
        }
        self.frog_mood = (self.frog_mood - dt * 0.08).max(0.0);
        for b in [&mut self.play, &mut self.daily, &mut self.trophy, &mut self.gear, &mut self.update] {
            b.update(dt);
        }
        for h in self.hearts.iter_mut() {
            h.1 += dt;
            h.0.y -= 90.0 * dt;
        }
        self.hearts.retain(|h| h.1 < 1.2);
        None
    }
    fn draw(&mut self, r: &mut Renderer, s: &Shared) {
        draw_pond_bg(r, self.t);
        let fp = self.frog_pos(s);
        art::lily_pad(r, fp.x, fp.y + 75.0, 190.0, 0.3);
        let jump = if self.frog_tap < 0.45 { (self.frog_tap / 0.45 * PI).sin() * 60.0 } else { 0.0 };
        let sq = if self.frog_tap < 0.6 { -wobble(self.frog_tap, 22.0) * 0.25 } else { 0.0 };
        let mut pose = Pose::idle(self.t, 0.3);
        pose.squash = sq + pose.squash;
        pose.sprout = true;
        if self.frog_tap < 0.5 {
            pose.mouth = 0.8;
            pose.happy_eyes = self.frog_mood > 0.5;
        }
        art::frog(r, fp.x, fp.y - jump, 260.0, art::EMERALD, &pose);
        for (p, a) in &self.hearts {
            heart(r, p.x + (a * 6.0).sin() * 10.0, p.y, 22.0, hex(0xFF5D8F).a(1.0 - a / 1.2));
        }
        if self.t > 3.0 && self.frog_tap > 5.0 {
            let bob = (self.t * 3.0).sin() * 4.0;
            r.rrect(fp.x + 120.0, fp.y - 230.0 + bob, 200.0, 56.0, 22.0, Col::WHITE, INK, 3.0);
            r.tri(v2(fp.x + 60.0, fp.y - 205.0 + bob), v2(fp.x + 90.0, fp.y - 205.0 + bob), v2(fp.x + 50.0, fp.y - 185.0 + bob), INK);
            r.text("Tap me!", fp.x + 120.0, fp.y - 230.0 + bob, 26.0, Align::Center, INK);
        }
        // logo
        let lx = self.play.pos.x;
        let wob = (self.t * 2.0).sin() * 0.03;
        r.push();
        r.translate(lx, 150.0);
        r.rotate(wob);
        r.text_bold("Ribbit", 0.0, -20.0, 104.0, Align::Center, hex(0x8BE04E), INK);
        r.rrect_rot(0.0, 62.0, 290.0, 54.0, 20.0, -0.04, ORANGE, INK, 4.0);
        r.text_bold("REVIVED", 0.0, 62.0, 40.0, Align::Center, Col::WHITE, INK);
        r.ellipse_rot(150.0, -78.0, 26.0, 12.0, -0.5, hex(0x8BE04E), INK, 3.0);
        r.ellipse_rot(118.0, -72.0, 20.0, 10.0, 0.5, hex(0xB4F07A), INK, 3.0);
        r.pop();
        self.play.draw(r);
        self.trophy.draw(r);
        self.daily.draw(r);
        self.gear.draw(r);
        if self.update.visible {
            self.update.draw(r);
            let b = self.update.pos + v2(self.update.w * 0.5 - 8.0, -self.update.h * 0.5 + 6.0);
            let k = 1.0 + (self.t * 5.0).sin().abs() * 0.12;
            r.circle(b.x, b.y, 16.0 * k, RED, INK, 3.0);
            r.text_bold("!", b.x, b.y + 1.0, 22.0, Align::Center, Col::WHITE, INK);
        }
        if !s.daily_done_today() {
            let b = self.daily.pos + v2(34.0, -36.0);
            let k = 1.0 + (self.t * 5.0).sin().abs() * 0.12;
            r.circle(b.x, b.y, 16.0 * k, RED, INK, 3.0);
            r.text_bold("!", b.x, b.y + 1.0, 22.0, Align::Center, Col::WHITE, INK);
        }
        // rank + stars strip
        draw_rank_strip(r, s, s.safe_l + 30.0, s.h - 52.0, 1.0);
    }
}

fn heart(r: &mut Renderer, x: f32, y: f32, s: f32, c: Col) {
    r.circle(x - s * 0.5, y, s * 0.6, c, c, 0.0);
    r.circle(x + s * 0.5, y, s * 0.6, c, c, 0.0);
    r.tri(v2(x - s * 1.08, y + s * 0.15), v2(x + s * 1.08, y + s * 0.15), v2(x, y + s * 1.25), c);
}

fn draw_rank_strip(r: &mut Renderer, s: &Shared, x: f32, y: f32, prog_override: f32) {
    let xp = s.profile.xp;
    let rk = rank_of(xp);
    let w = 420.0;
    r.rrect(x + w * 0.5, y + 4.0, w, 64.0, 32.0, Col(0, 0, 0, 60), Col::CLEAR, 0.0);
    r.rrect(x + w * 0.5, y, w, 64.0, 32.0, hex(0x23306E), INK, 4.0);
    rank_badge(r, x + 34.0, y, 30.0, rk);
    r.text_bold(RANKS[rk].1, x + 74.0, y - 12.0, 26.0, Align::Left, GOLD, INK);
    let p = rank_progress(xp) * prog_override;
    r.rrect(x + 74.0 + 110.0, y + 16.0, 220.0, 14.0, 7.0, Col(0, 0, 0, 90), Col::CLEAR, 0.0);
    if p > 0.0 {
        r.rrect(x + 74.0 + 110.0 * p, y + 16.0, 220.0 * p, 14.0, 7.0, hex(0x8BE04E), Col::CLEAR, 0.0);
    }
    let stars = s.profile.total_stars();
    art::star_icon(r, x + w - 72.0, y - 2.0, 20.0, true);
    r.text_bold(&stars.to_string(), x + w - 46.0, y, 28.0, Align::Left, Col::WHITE, INK);
    // clocks
    art::clock_icon(r, x + w + 50.0, y, 22.0, 1.0);
    r.text_bold(&s.profile.clocks.to_string(), x + w + 80.0, y, 28.0, Align::Left, Col::WHITE, INK);
}

fn rank_badge(r: &mut Renderer, x: f32, y: f32, rad: f32, rank: usize) {
    let cols = [hex(0xB9C4C9), hex(0x8BE04E), hex(0x4CC38A), hex(0x4FA7F2), hex(0x3D7BE0), hex(0x9A6BEA), hex(0xF27BC4), hex(0xFF9F2E), hex(0xF0605D), hex(0xFFD23F)];
    let c = cols[rank.min(9)];
    r.circle(x, y, rad, c, INK, 3.5);
    let pips = rank + 1;
    if pips <= 5 {
        for i in 0..pips {
            let a = -PI * 0.5 + i as f32 * TAU / pips as f32;
            r.disc(x + a.cos() * rad * 0.45 * if pips > 1 { 1.0 } else { 0.0 }, y + a.sin() * rad * 0.45 * if pips > 1 { 1.0 } else { 0.0 }, rad * 0.18, Col::WHITE);
        }
    } else {
        r.star(x, y + 1.0, rad * 0.62, 0.0, Col::WHITE, INK, 2.5);
        r.text_bold(&(rank - 4).to_string(), x, y + 2.0, rad * 0.6, Align::Center, c, INK);
    }
}

// ------------------------------------------------------------------ level select

struct SelectScene {
    back: Button,
    scroll: f32,
    vel: f32,
    drag: Option<(i32, f32, f32, f32)>,
    moved: bool,
    shake: Vec<f32>,
    t: f32,
    press: Option<usize>,
}

const CARD_W: f32 = 300.0;
const CARD_H: f32 = 250.0;
const CARD_GAP: f32 = 34.0;

impl SelectScene {
    fn new(s: &Shared, focus: Option<usize>) -> SelectScene {
        let n = s.levels().len();
        let mut sc = SelectScene { back: back_button(s), scroll: 0.0, vel: 0.0, drag: None, moved: false, shake: vec![0.0; n], t: 0.0, press: None };
        if let Some(f) = focus {
            let col = (f / 2) as f32;
            sc.scroll = (col * (CARD_W + CARD_GAP) - (s.safe_r - s.safe_l) * 0.35).max(0.0);
            sc.clamp(s);
        }
        sc
    }
    fn layout(&mut self, s: &Shared) {
        self.back = back_button(s);
        self.clamp(s);
    }
    fn max_scroll(&self, s: &Shared) -> f32 {
        let cols = s.levels().len().div_ceil(2) as f32;
        let content = cols * (CARD_W + CARD_GAP) - CARD_GAP + 120.0;
        (content - (s.safe_r - s.safe_l)).max(0.0)
    }
    fn clamp(&mut self, s: &Shared) {
        let m = self.max_scroll(s);
        self.scroll = self.scroll.clamp(0.0, m);
    }
    fn card_pos(&self, s: &Shared, i: usize) -> V2 {
        let col = (i / 2) as f32;
        let row = (i % 2) as f32;
        v2(s.safe_l + 60.0 + CARD_W * 0.5 + col * (CARD_W + CARD_GAP) - self.scroll, 250.0 + row * (CARD_H + 36.0))
    }
    fn card_at(&self, s: &Shared, p: V2) -> Option<usize> {
        (0..s.levels().len()).find(|&i| {
            let c = self.card_pos(s, i);
            (p.x - c.x).abs() < CARD_W * 0.5 && (p.y - c.y).abs() < CARD_H * 0.5
        })
    }
    fn touch(&mut self, s: &mut Shared, t: &Touch) -> Option<Nav> {
        if self.back.touch(t) {
            return Some(Nav::Menu);
        }
        match t.phase {
            Phase::Down => {
                if self.drag.is_none() && t.pos.y > 110.0 {
                    self.drag = Some((t.id, t.pos.x, self.scroll, s.time));
                    self.moved = false;
                    self.vel = 0.0;
                    self.press = self.card_at(s, t.pos);
                }
            }
            Phase::Move => {
                if let Some((id, x0, s0, _)) = self.drag {
                    if id == t.id {
                        let dx = t.pos.x - x0;
                        if dx.abs() > 14.0 {
                            self.moved = true;
                            self.press = None;
                        }
                        if self.moved {
                            let prev = self.scroll;
                            self.scroll = s0 - dx;
                            let m = self.max_scroll(s);
                            if self.scroll < 0.0 {
                                self.scroll *= 0.35;
                            } else if self.scroll > m {
                                self.scroll = m + (self.scroll - m) * 0.35;
                            }
                            self.vel = (self.scroll - prev) * 60.0;
                        }
                    }
                }
            }
            Phase::Up | Phase::Cancel => {
                if let Some((id, ..)) = self.drag {
                    if id == t.id {
                        self.drag = None;
                        if !self.moved && t.phase == Phase::Up {
                            if let Some(i) = self.card_at(s, t.pos) {
                                if s.unlocked(i) {
                                    audio::play(Sfx::Click);
                                    return Some(Nav::Play(i, Mode::Normal, false));
                                }
                                self.shake[i] = 0.5;
                                audio::play(Sfx::Wrong);
                                s.haptic(REQ_HAPTIC_REJECT);
                                let need = s.levels()[i].unlock - s.profile.total_stars();
                                s.toasts.clear();
                                s.toast("Locked", &format!("Collect {need} more star{} to play", if need == 1 { "" } else { "s" }), Icon::Lock);
                            }
                        }
                        self.press = None;
                    }
                }
            }
        }
        None
    }
    fn update(&mut self, s: &mut Shared, dt: f32) -> Option<Nav> {
        self.t += dt;
        self.back.update(dt);
        for v in self.shake.iter_mut() {
            *v = (*v - dt).max(0.0);
        }
        if self.drag.is_none() {
            self.scroll += self.vel * dt;
            self.vel *= (1.0 - dt * 4.0).max(0.0);
            let m = self.max_scroll(s);
            if self.scroll < 0.0 {
                self.scroll += (0.0 - self.scroll) * (dt * 12.0).min(1.0);
                self.vel = 0.0;
            } else if self.scroll > m {
                self.scroll += (m - self.scroll) * (dt * 12.0).min(1.0);
                self.vel = 0.0;
            }
        }
        None
    }
    fn draw(&mut self, r: &mut Renderer, s: &Shared) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0x7FD8F5), hex(0x2E9F6E));
        // soft dots pattern
        for i in 0..14 {
            let x = (i as f32 * 157.0 - self.scroll * 0.3).rem_euclid(w + 100.0) - 50.0;
            r.circle(x, 120.0 + (i * 53 % 500) as f32, 30.0 + (i % 3) as f32 * 14.0, Col::WHITE.a(0.07), Col::CLEAR, 0.0);
        }
        let stars = s.profile.total_stars();
        for (i, def) in s.levels().iter().enumerate() {
            let p = self.card_pos(s, i);
            if p.x < -CARD_W || p.x > w + CARD_W {
                continue;
            }
            let shake = (self.shake[i] * 40.0).sin() * self.shake[i] * 24.0;
            let pressed = self.press == Some(i);
            let sc = if pressed { 0.95 } else { 1.0 };
            r.push();
            r.translate(p.x + shake, p.y);
            r.scale(sc, sc);
            let unlocked = stars >= def.unlock;
            draw_card(r, s, def, i, unlocked, self.t);
            r.pop();
        }
        // header
        r.grad_rect(0.0, 0.0, w, 118.0, hex(0x1F7A5A).a(0.95), hex(0x1F7A5A).a(0.0));
        let cx = (s.safe_l + s.safe_r) * 0.5;
        r.text_bold("Choose a game", cx, 62.0, 54.0, Align::Center, Col::WHITE, INK);
        let sx = s.safe_r - 120.0;
        art::pill(r, sx, 64.0, 170.0, 70.0, hex(0x23306E));
        art::star_icon(r, sx - 45.0, 62.0, 24.0, true);
        r.text_bold(&stars.to_string(), sx + 20.0, 64.0, 34.0, Align::Center, Col::WHITE, INK);
        self.back.draw(r);
        // scroll hint
        let m = self.max_scroll(s);
        if m > 0.0 {
            let bw = (s.safe_r - s.safe_l) * 0.4;
            let bx = cx - bw * 0.5;
            r.rrect(cx, h - 28.0, bw, 10.0, 5.0, Col(0, 0, 0, 60), Col::CLEAR, 0.0);
            let k = (self.scroll / m).clamp(0.0, 1.0);
            r.rrect(bx + 40.0 + k * (bw - 80.0), h - 28.0, 80.0, 10.0, 5.0, Col::WHITE.a(0.8), Col::CLEAR, 0.0);
        }
    }
}

fn draw_card(r: &mut Renderer, s: &Shared, def: &LevelDef, _i: usize, unlocked: bool, t: f32) {
    let (cw, ch) = (CARD_W, CARD_H);
    r.rrect(0.0, 8.0, cw, ch, 30.0, Col(0, 0, 0, 70), Col::CLEAR, 0.0);
    let base = if unlocked { def.color } else { hex(0x8C9A94) };
    r.rrect_grad(0.0, 0.0, cw, ch, 30.0, base.shade(0.35), base.shade(0.05), INK, 5.0);
    // art window
    r.rrect(0.0, -30.0, cw - 30.0, ch - 96.0, 20.0, base.shade(0.55), Col::WHITE.a(0.6), 3.0);
    r.push();
    if !unlocked {
        r.fade(0.45);
    }
    (def.icon)(r, 0.0, -30.0, 150.0, t);
    r.pop();
    r.text_bold(def.title, 0.0, ch * 0.5 - 52.0, 32.0, Align::Center, Col::WHITE, INK);
    if unlocked {
        if def.two_player {
            r.rrect(0.0, ch * 0.5 - 18.0, 150.0, 30.0, 15.0, hex(0x23306E), INK, 3.0);
            r.text("2 PLAYERS", 0.0, ch * 0.5 - 18.0, 20.0, Align::Center, GOLD);
        } else {
            let rec = s.profile.level(def.id);
            star_row(r, 0.0, ch * 0.5 - 18.0, rec.stars, 13.0, 30.0);
        }
    } else {
        r.circle(0.0, -30.0, 54.0, hex(0x23306E), INK, 4.0);
        ui::draw_icon(r, Icon::Lock, 0.0, -34.0, 60.0, GOLD);
        r.rrect(0.0, ch * 0.5 - 18.0, 130.0, 34.0, 17.0, hex(0x23306E), INK, 3.0);
        art::star_icon(r, -32.0, ch * 0.5 - 19.0, 13.0, true);
        r.text_bold(&def.unlock.to_string(), 14.0, ch * 0.5 - 18.0, 24.0, Align::Center, Col::WHITE, INK);
    }
}

// ------------------------------------------------------------------ play

#[derive(Clone, Copy, PartialEq, Debug)]
enum PlayPhase {
    Intro,
    Countdown,
    Run,
    TimeUp,
    MoreTime,
    Finish,
}

#[derive(Clone)]
pub struct RoundResult {
    idx: usize,
    score: i64,
    winner: Option<i8>,
    new_ach: Vec<usize>,
    mode: Mode,
}

struct PlayScene {
    idx: usize,
    def: &'static LevelDef,
    game: Box<dyn MiniGame>,
    ctx: Ctx,
    phase: PlayPhase,
    pt: f32,
    paused: bool,
    pause_t: f32,
    used_bonus: bool,
    last_sec: i32,
    ach_hit: Vec<bool>,
    new_ach: Vec<usize>,
    btn_pause: Button,
    btn_help: Button,
    btn_main: Button,
    help: bool,
    mode: Mode,
    btn_start: Button,
    btn_alt: Button,
    btn_resume: Button,
    btn_restart: Button,
    btn_levels: Button,
    btn_use: Button,
    btn_skip: Button,
    hud_pop: f32,
    last_score: i64,
    prev_total: i64,
    best: i64,
    toast_t: f32,
    toast_name: String,
}

impl PlayScene {
    fn new(s: &mut Shared, idx: usize, mode: Mode, skip_intro: bool) -> PlayScene {
        let def = &s.levels()[idx];
        let seed = s.rng.next_u64();
        let mut ctx = Ctx::new(s.w, s.h, seed);
        ctx.safe_l = s.safe_l;
        ctx.safe_r = s.safe_r;
        ctx.time_left = def.duration;
        ctx.haptics = s.profile.haptics;
        ctx.cpu = mode == Mode::Cpu;
        ctx.practice = mode == Mode::Practice;
        let game = (def.make)(&mut ctx);
        let rec = s.profile.level(def.id);
        let ach_hit: Vec<bool> = (0..def.ach.len()).map(|k| s.profile.has_ach(&ach_key(def.id, k))).collect();
        let mut p = PlayScene {
            idx,
            def,
            game,
            ctx,
            phase: PlayPhase::Intro,
            pt: 0.0,
            paused: false,
            pause_t: 0.0,
            used_bonus: false,
            last_sec: 99,
            ach_hit,
            new_ach: Vec::new(),
            btn_pause: Button::round(0.0, 0.0, 84.0, Icon::Pause, ORANGE),
            btn_help: Button::round(0.0, 0.0, 70.0, Icon::Info, BLUE),
            btn_main: Button::new(0.0, 0.0, 330.0, 84.0, "Main menu", Icon::Back, PURPLE),
            help: false,
            mode,
            btn_start: Button::new(0.0, 0.0, 300.0, 96.0, if def.two_player { "2 PLAYERS" } else { "START!" }, Icon::Play, GREEN),
            btn_alt: Button::new(0.0, 0.0, 300.0, 96.0, if def.two_player { "VS CPU" } else { "PRACTICE" }, if def.two_player { Icon::Star } else { Icon::Info }, PURPLE),
            btn_resume: Button::new(0.0, 0.0, 330.0, 84.0, "Continue", Icon::Play, GREEN),
            btn_restart: Button::new(0.0, 0.0, 330.0, 84.0, "Restart", Icon::Retry, ORANGE),
            btn_levels: Button::new(0.0, 0.0, 330.0, 84.0, "Choose game", Icon::Menu, BLUE),
            btn_use: Button::new(0.0, 0.0, 290.0, 90.0, "+5 sec", Icon::Clock, GREEN),
            btn_skip: Button::new(0.0, 0.0, 250.0, 90.0, "No thanks", Icon::None, RED),
            hud_pop: 0.0,
            last_score: 0,
            prev_total: rec.total,
            best: rec.best,
            toast_t: 0.0,
            toast_name: String::new(),
        };
        p.btn_start.text_size = 36.0;
        p.btn_alt.text_size = 34.0;
        p.btn_alt.visible = !matches!(mode, Mode::Daily(_)) && !skip_intro;
        if skip_intro {
            p.phase = PlayPhase::Countdown;
        }
        p.layout(s);
        p
    }

    /// Leaves the intro card and starts the 3-2-1 countdown in the chosen mode.
    fn begin(&mut self, s: &mut Shared, mode: Mode) {
        self.mode = mode;
        self.ctx.cpu = mode == Mode::Cpu;
        self.ctx.practice = mode == Mode::Practice;
        self.phase = PlayPhase::Countdown;
        self.pt = 0.0;
        self.btn_alt.visible = false;
        self.layout(s);
        s.profile.tips_seen.insert(self.def.id.to_string(), true);
    }

    fn timed(&self) -> bool {
        self.def.duration > 0.0 && !self.ctx.practice
    }

    fn layout(&mut self, s: &Shared) {
        let cx = (s.safe_l + s.safe_r) * 0.5;
        self.ctx.safe_l = s.safe_l;
        self.ctx.safe_r = s.safe_r;
        self.ctx.w = s.w;
        self.ctx.h = s.h;
        self.btn_pause.pos = v2(s.safe_l + 62.0, 58.0);
        self.btn_help.pos = v2(s.safe_l + 150.0, 58.0);
        if self.btn_alt.visible {
            self.btn_start.pos = v2(cx - 170.0, s.h * 0.5 + 200.0);
            self.btn_alt.pos = v2(cx + 170.0, s.h * 0.5 + 200.0);
        } else {
            self.btn_start.pos = v2(cx, s.h * 0.5 + 200.0);
        }
        self.btn_resume.pos = v2(cx, s.h * 0.5 - 105.0);
        self.btn_restart.pos = v2(cx, s.h * 0.5 - 5.0);
        self.btn_levels.pos = v2(cx, s.h * 0.5 + 95.0);
        self.btn_main.pos = v2(cx, s.h * 0.5 + 195.0);
        for b in [&mut self.btn_resume, &mut self.btn_restart, &mut self.btn_levels, &mut self.btn_main] {
            b.h = 84.0;
        }
        self.btn_use.pos = v2(cx - 150.0, s.h * 0.5 + 110.0);
        self.btn_skip.pos = v2(cx + 160.0, s.h * 0.5 + 110.0);
    }

    fn running(&self) -> bool {
        self.phase == PlayPhase::Run && !self.paused
    }

    fn app_paused(&mut self) {
        if matches!(self.phase, PlayPhase::Run | PlayPhase::Countdown) {
            self.paused = true;
        }
    }

    fn back(&mut self, _s: &mut Shared) -> Option<Nav> {
        match self.phase {
            PlayPhase::Intro => Some(Nav::Select(Some(self.idx))),
            PlayPhase::Run | PlayPhase::Countdown => {
                self.paused = !self.paused;
                self.pause_t = 0.0;
                None
            }
            _ => None,
        }
    }

    fn finish(&mut self) -> Nav {
        Nav::Score(RoundResult { idx: self.idx, score: self.ctx.score, winner: self.ctx.winner, new_ach: self.new_ach.clone(), mode: self.mode })
    }

    fn touch(&mut self, s: &mut Shared, t: &Touch) -> Option<Nav> {
        if self.help {
            if self.btn_start.touch(t) {
                self.help = false;
                self.paused = false;
            }
            return None;
        }
        if self.paused {
            if self.btn_resume.touch(t) {
                self.paused = false;
            } else if self.btn_restart.touch(t) {
                return Some(Nav::Play(self.idx, self.mode, true));
            } else if self.btn_levels.touch(t) {
                return Some(Nav::Select(Some(self.idx)));
            } else if self.btn_main.touch(t) {
                return Some(Nav::Menu);
            }
            return None;
        }
        match self.phase {
            PlayPhase::Intro => {
                if self.btn_start.touch(t) {
                    let m = self.mode;
                    self.begin(s, m);
                } else if self.btn_alt.touch(t) {
                    let m = if self.def.two_player { Mode::Cpu } else { Mode::Practice };
                    self.begin(s, m);
                }
            }
            PlayPhase::Countdown => {
                self.btn_pause.touch(t);
            }
            PlayPhase::Run => {
                if self.btn_pause.touch(t) {
                    self.paused = true;
                    self.pause_t = 0.0;
                    return None;
                }
                if self.btn_help.touch(t) {
                    self.paused = true;
                    self.help = true;
                    self.btn_start.label = "CONTINUE".into();
                    return None;
                }
                if self.btn_pause.is_pressed() || self.btn_help.is_pressed() {
                    return None;
                }
                self.game.touch(&mut self.ctx, t);
            }
            PlayPhase::MoreTime => {
                if self.btn_use.touch(t) && s.profile.clocks > 0 {
                    s.profile.clocks -= 1;
                    s.profile.dirty = true;
                    self.used_bonus = true;
                    self.ctx.time_left = 5.0;
                    self.last_sec = 99;
                    self.phase = PlayPhase::Countdown;
                    self.pt = 1.4; // short "1, GO!" restart
                    audio::play(Sfx::Clock);
                } else if self.btn_skip.touch(t) {
                    self.phase = PlayPhase::Finish;
                    return Some(self.finish());
                }
            }
            _ => {}
        }
        None
    }

    fn check_achievements(&mut self, s: &mut Shared) {
        for k in 0..self.def.ach.len() {
            if self.ach_hit[k] {
                continue;
            }
            let a = &self.def.ach[k];
            let c = &self.ctx;
            let ok = match a.kind {
                AchKind::Total => self.prev_total + c.score >= a.target,
                AchKind::Single => c.score >= a.target,
                AchKind::Early(secs) => c.t <= secs as f32 && c.score >= a.target,
                AchKind::Streak => c.special >= a.target,
                AchKind::Flag => c.special_hit,
            };
            if ok {
                self.ach_hit[k] = true;
                self.new_ach.push(k);
                s.profile.achievements.insert(ach_key(self.def.id, k), true);
                s.profile.dirty = true;
                self.toast_t = 2.6;
                self.toast_name = a.name.to_string();
                audio::play(Sfx::Achievement);
                s.haptic(REQ_HAPTIC_CONFIRM);
            }
        }
    }

    fn update(&mut self, s: &mut Shared, dt: f32) -> Option<Nav> {
        self.btn_pause.update(dt);
        self.btn_help.update(dt);
        self.ctx.time_flash = (self.ctx.time_flash - dt * 1.5).max(0.0);
        for b in [&mut self.btn_start, &mut self.btn_alt, &mut self.btn_resume, &mut self.btn_restart, &mut self.btn_levels, &mut self.btn_main, &mut self.btn_use, &mut self.btn_skip] {
            b.update(dt);
        }
        self.ctx.anim += dt;
        self.toast_t = (self.toast_t - dt).max(0.0);
        if self.paused {
            self.pause_t += dt;
            return None;
        }
        self.pt += dt;
        self.ctx.shake = (self.ctx.shake - dt).max(0.0);
        self.ctx.fx.update(dt);
        self.hud_pop = (self.hud_pop - dt * 4.0).max(0.0);
        match self.phase {
            PlayPhase::Intro => {
                self.game.idle(&mut self.ctx, dt);
            }
            PlayPhase::Countdown => {
                self.game.idle(&mut self.ctx, dt);
                // 3 beats of 0.7 s then GO
                let beat = (self.pt / 0.7) as i32;
                if beat != self.last_sec {
                    self.last_sec = beat;
                    if beat < 3 {
                        audio::play(Sfx::Beep);
                    } else {
                        audio::play(Sfx::Go);
                        self.phase = PlayPhase::Run;
                        self.pt = 0.0;
                        self.last_sec = 99;
                    }
                }
            }
            PlayPhase::Run => {
                self.ctx.t += dt;
                if self.timed() {
                    self.ctx.time_left -= dt;
                }
                self.game.update(&mut self.ctx, dt);
                if self.ctx.score != self.last_score {
                    self.hud_pop = 1.0;
                    self.last_score = self.ctx.score;
                }
                // last-seconds warning ticks
                let sec = self.ctx.time_left.ceil() as i32;
                if self.timed() && (1..=4).contains(&sec) && sec != self.last_sec {
                    self.last_sec = sec;
                    audio::play(Sfx::Tick);
                }
                if !self.ctx.practice {
                    self.check_achievements(s);
                }
                s.requests |= self.ctx.requests;
                self.ctx.requests = 0;
                let timed_out = self.timed() && self.ctx.time_left <= 0.0;
                if timed_out || self.ctx.end_now {
                    self.ctx.time_left = self.ctx.time_left.max(0.0);
                    self.phase = PlayPhase::TimeUp;
                    self.pt = 0.0;
                    audio::play(Sfx::TimeUp);
                    s.haptic(REQ_HAPTIC_HEAVY);
                }
            }
            PlayPhase::TimeUp => {
                if self.pt > 1.3 {
                    let can_bonus = !self.def.two_player && !self.used_bonus && !self.ctx.end_now && s.profile.clocks > 0;
                    if can_bonus {
                        self.phase = PlayPhase::MoreTime;
                        self.pt = 0.0;
                    } else {
                        self.phase = PlayPhase::Finish;
                        return Some(self.finish());
                    }
                }
            }
            PlayPhase::MoreTime => {
                if self.pt > 6.0 {
                    self.phase = PlayPhase::Finish;
                    return Some(self.finish());
                }
            }
            PlayPhase::Finish => {}
        }
        s.requests |= self.ctx.requests;
        self.ctx.requests = 0;
        None
    }

    fn draw(&mut self, r: &mut Renderer, s: &Shared) {
        let shake = if self.ctx.shake > 0.0 { (self.ctx.anim * 90.0).sin() * self.ctx.shake * 18.0 } else { 0.0 };
        r.push();
        r.translate(shake, 0.0);
        self.game.draw(r, &self.ctx);
        self.ctx.fx.draw(r);
        r.pop();
        self.draw_hud(r, s);
        let cx = (s.safe_l + s.safe_r) * 0.5;
        let cy = s.h * 0.5;
        match self.phase {
            PlayPhase::Intro => self.draw_intro(r, s),
            PlayPhase::Countdown => {
                let beat = (self.pt / 0.7) as i32;
                let f = (self.pt % 0.7) / 0.7;
                let label = match beat {
                    0 => "3",
                    1 => "2",
                    2 => "1",
                    _ => "GO!",
                };
                let k = ease_out_back((f / 0.4).min(1.0));
                r.push();
                r.translate(cx, cy);
                r.scale(k * 1.0, k * 1.0);
                r.fade(1.0 - ((f - 0.7) / 0.3).max(0.0));
                r.glow(0.0, 0.0, 200.0, Col::WHITE.a(0.35));
                r.text_bold(label, 0.0, 0.0, 190.0, Align::Center, GOLD, INK);
                r.pop();
            }
            PlayPhase::Run if self.pt < 0.6 => {
                let f = self.pt / 0.6;
                r.push();
                r.translate(cx, cy);
                let k = 1.0 + f * 0.6;
                r.scale(k, k);
                r.fade(1.0 - f);
                r.text_bold("GO!", 0.0, 0.0, 190.0, Align::Center, GOLD, INK);
                r.pop();
            }
            PlayPhase::TimeUp => {
                let k = ease_out_back((self.pt / 0.4).min(1.0));
                ui::dim(r, 0.35 * (self.pt / 0.3).min(1.0));
                let label = if self.def.two_player {
                    match (self.ctx.winner, self.ctx.cpu) {
                        (Some(0), true) => "YOU WIN!",
                        (Some(1), true) => "CPU WINS!",
                        (Some(0), false) => "RED WINS!",
                        (Some(1), false) => "BLUE WINS!",
                        _ => "DRAW!",
                    }
                } else {
                    "TIME UP!"
                };
                r.push();
                r.translate(cx, cy - 300.0 + 300.0 * k);
                r.rotate(-0.05);
                r.rrect(0.0, 6.0, 640.0, 150.0, 40.0, Col(0, 0, 0, 80), Col::CLEAR, 0.0);
                r.rrect_grad(0.0, 0.0, 640.0, 150.0, 40.0, hex(0xFF7A59), hex(0xE24A3B), INK, 6.0);
                r.text_bold(label, 0.0, 0.0, 96.0, Align::Center, Col::WHITE, INK);
                r.pop();
            }
            PlayPhase::MoreTime => {
                ui::dim(r, 0.55);
                let k = ease_out_back((self.pt / 0.3).min(1.0));
                r.push();
                r.translate(cx, cy);
                r.scale(k, k);
                r.translate(-cx, -cy);
                ui::panel(r, cx, cy, 720.0, 430.0, PANEL);
                r.text_bold("Need more time?", cx, cy - 150.0, 54.0, Align::Center, ORANGE, INK);
                art::clock_icon(r, cx - 200.0, cy - 30.0, 54.0, 1.0 - self.pt / 6.0);
                r.text(&format!("Spend 1 clock for\n+5 seconds of play."), cx + 60.0, cy - 50.0, 34.0, Align::Center, INK);
                r.text(&format!("You have {} clock{}", s.profile.clocks, if s.profile.clocks == 1 { "" } else { "s" }), cx + 60.0, cy + 20.0, 28.0, Align::Center, hex(0x2F6F4F));
                self.btn_use.draw(r);
                self.btn_skip.draw(r);
                r.pop();
            }
            _ => {}
        }
        // in-round achievement toast
        if self.toast_t > 0.0 {
            let k = ease_out_back(((2.6 - self.toast_t) / 0.3).min(1.0)) * (self.toast_t / 0.3).min(1.0);
            let y = s.h + 60.0 - k * 140.0;
            r.rrect(cx, y, 520.0, 84.0, 30.0, hex(0x23306E), INK, 4.0);
            ui::draw_icon(r, Icon::Trophy, cx - 210.0, y, 54.0, GOLD);
            r.text_bold("Achievement!", cx + 30.0, y - 16.0, 26.0, Align::Center, GOLD, INK);
            r.text(&self.toast_name, cx + 30.0, y + 18.0, 26.0, Align::Center, Col::WHITE);
        }
        if self.help {
            self.draw_intro(r, s);
        } else if self.paused {
            ui::dim(r, 0.6);
            ui::panel(r, cx, cy + 20.0, 460.0, 610.0, PANEL);
            r.text_bold("Paused", cx, cy - 210.0, 58.0, Align::Center, ORANGE, INK);
            self.btn_resume.draw(r);
            self.btn_restart.draw(r);
            self.btn_levels.draw(r);
            self.btn_main.draw(r);
        }
    }

    fn draw_hud(&self, r: &mut Renderer, s: &Shared) {
        let cx = (s.safe_l + s.safe_r) * 0.5;
        // timer (untimed 2-player duels show only the pause button)
        if self.def.duration <= 0.0 {
            if matches!(self.phase, PlayPhase::Run | PlayPhase::Countdown) {
                self.btn_pause.draw(r);
            }
            return;
        }
        if self.ctx.practice {
            art::pill(r, cx, 58.0, 250.0, 76.0, PURPLE);
            r.text_bold("PRACTICE", cx, 60.0, 38.0, Align::Center, Col::WHITE, INK);
            if matches!(self.phase, PlayPhase::Run | PlayPhase::Countdown) {
                self.btn_pause.draw(r);
            }
            let sx = s.safe_r - 150.0;
            art::pill(r, sx, 58.0, 230.0, 76.0, hex(0x23306E));
            let txt = self.game.score_text(&self.ctx);
            r.text_bold(&txt, sx - 10.0, 54.0, 42.0, Align::Center, GOLD, INK);
            r.text(self.def.unit, sx - 10.0, 84.0, 18.0, Align::Center, Col::WHITE.a(0.8));
            return;
        }
        let tl = self.ctx.time_left.max(0.0);
        let urgent = tl <= 4.0 && self.phase == PlayPhase::Run;
        let pulse = if urgent { 1.0 + (1.0 - tl.fract()) * 0.15 } else { 1.0 };
        r.push();
        r.translate(cx, 58.0);
        r.scale(pulse, pulse);
        let base = if urgent { RED } else { hex(0x23306E) };
        art::pill(r, 0.0, 0.0, 230.0, 76.0, base.mix(hex(0x3DBB5A), self.ctx.time_flash));
        art::clock_icon(r, -72.0, 2.0, 26.0, (tl / self.def.duration).min(1.0));
        let shown = if tl >= 10.0 { format!("{:.1}", tl) } else { format!("{:.2}", tl) };
        let tc = if self.ctx.time_flash > 0.0 { Col::WHITE.mix(hex(0xB4F07A), self.ctx.time_flash) } else { Col::WHITE };
        r.text_bold(&shown, 26.0, 2.0, 44.0, Align::Center, tc, INK);
        r.pop();
        // pause + help
        if matches!(self.phase, PlayPhase::Run | PlayPhase::Countdown) {
            self.btn_pause.draw(r);
            if self.phase == PlayPhase::Run {
                self.btn_help.draw(r);
            }
        }
        if self.def.two_player {
            return;
        }
        // score
        let sx = s.safe_r - 150.0;
        let pop = 1.0 + self.hud_pop * 0.12;
        r.push();
        r.translate(sx, 58.0);
        r.scale(pop, pop);
        art::pill(r, 0.0, 0.0, 230.0, 76.0, hex(0x23306E));
        let txt = self.game.score_text(&self.ctx);
        r.text_bold(&txt, -10.0, -4.0, 42.0, Align::Center, GOLD, INK);
        r.text(self.def.unit, -10.0, 26.0, 18.0, Align::Center, Col::WHITE.a(0.8));
        r.pop();
        // star progress strip under the score: each star fills as you approach its threshold
        let th = &self.def.thresholds;
        for i in 0..5 {
            let lo = if i == 0 { 0 } else { th[i - 1] };
            let fill = ((self.ctx.score - lo) as f32 / (th[i] - lo).max(1) as f32).clamp(0.0, 1.0);
            let x = sx - 72.0 + i as f32 * 36.0;
            let y = 116.0;
            art::star_icon(r, x, y, 15.0, false);
            if fill > 0.0 {
                let k = if fill >= 1.0 { 1.0 + self.hud_pop * 0.25 } else { 0.35 + 0.55 * fill };
                r.push();
                r.translate(x, y);
                r.scale(k, k);
                r.fade(if fill >= 1.0 { 1.0 } else { 0.55 });
                art::star_icon(r, 0.0, 0.0, 15.0, true);
                r.pop();
            }
        }
        if let Mode::Daily(target) = self.mode {
            let done = self.ctx.score >= target;
            r.rrect(sx, 154.0, 200.0, 34.0, 17.0, if done { GREEN } else { hex(0x23306E) }, INK, 3.0);
            r.text_bold(&format!("Goal {target}"), sx, 155.0, 22.0, Align::Center, if done { Col::WHITE } else { GOLD }, INK);
        }
    }

    fn draw_intro(&self, r: &mut Renderer, s: &Shared) {
        let cx = (s.safe_l + s.safe_r) * 0.5;
        let cy = s.h * 0.5;
        let k = ease_out_back((self.pt / 0.35).min(1.0));
        ui::dim(r, 0.5 * (self.pt / 0.25).min(1.0));
        r.push();
        r.translate(cx, cy);
        r.scale(k, k);
        r.translate(-cx, -cy);
        ui::panel(r, cx, cy + 10.0, 820.0, 560.0, PANEL);
        r.text_bold(self.def.title, cx, cy - 205.0, 60.0, Align::Center, self.def.color.shade(-0.1), INK);
        // illustration
        r.rrect(cx - 200.0, cy - 30.0, 300.0, 230.0, 26.0, self.def.color.shade(0.5), self.def.color.shade(-0.3), 4.0);
        (self.def.icon)(r, cx - 200.0, cy - 30.0, 190.0, self.ctx.anim);
        r.text(self.def.tip, cx + 160.0, cy - 80.0, 32.0, Align::Center, INK);
        if self.def.two_player {
            r.text("Left side: RED    Right side: BLUE", cx + 160.0, cy + 40.0, 24.0, Align::Center, hex(0x2F6F4F));
            r.text("Alone? Play VS CPU as RED.", cx + 160.0, cy + 80.0, 24.0, Align::Center, hex(0x6A3DB8));
        } else {
            for (i, &th) in self.def.thresholds.iter().enumerate() {
                let y = cy - 2.0 + (i / 3) as f32 * 48.0;
                let x = cx + 50.0 + (i % 3) as f32 * 112.0;
                art::star_icon(r, x - 26.0, y, 16.0, true);
                r.text_bold(&th.to_string(), x + 14.0, y, 28.0, Align::Center, Col::WHITE, INK);
            }
            if let Mode::Daily(target) = self.mode {
                r.text_bold(&format!("Score {target} for +3 clocks"), cx + 160.0, cy + 110.0, 28.0, Align::Center, ORANGE, INK);
            } else if self.best > 0 {
                r.text(&format!("Your best: {}", self.best), cx + 160.0, cy + 110.0, 28.0, Align::Center, hex(0x2F6F4F));
            }
        }
        if matches!(self.mode, Mode::Daily(_)) {
            r.push();
            r.translate(cx + 300.0, cy - 250.0);
            r.rotate(0.12);
            r.rrect(0.0, 0.0, 250.0, 54.0, 20.0, ORANGE, INK, 4.0);
            r.text_bold("DAILY CHALLENGE", 0.0, 2.0, 26.0, Align::Center, Col::WHITE, INK);
            r.pop();
        }
        self.btn_start.draw(r);
        self.btn_alt.draw(r);
        r.pop();
    }
}

fn ach_key(id: &str, k: usize) -> String {
    format!("{id}.{k}")
}

// ------------------------------------------------------------------ results

struct ScoreScene {
    res: RoundResult,
    def: &'static LevelDef,
    t: f32,
    stars: u8,
    prev_best: i64,
    new_best: bool,
    xp_before: i64,
    xp_after: i64,
    rank_up: bool,
    unlocked_now: Vec<usize>,
    clocks_won: i32,
    btn_menu: Button,
    btn_retry: Button,
    btn_next: Button,
    star_fired: u8,
    fx: levels::Fx,
    daily_won: bool,
}

impl ScoreScene {
    fn new(s: &mut Shared, res: RoundResult) -> ScoreScene {
        let def = &s.levels()[res.idx];
        let stars_before_total = s.profile.total_stars();
        let rec = s.profile.level(def.id);
        let stars = if def.two_player { 0 } else { def.stars_for(res.score) };
        let xp_before = s.profile.xp;
        let new_best = !def.two_player && res.score > rec.best;
        let mut clocks_won = 0;
        {
            let r = s.profile.level_mut(def.id);
            r.plays += 1;
            if !def.two_player {
                r.total += res.score.max(0);
                if res.score > r.best {
                    r.best = res.score;
                }
                if stars > r.stars {
                    clocks_won += (stars - r.stars) as i32;
                    r.stars = stars;
                }
            }
        }
        // rank progress grows by the stars earned this round (2P games give 1 for playing)
        let gain = if def.two_player { 1 } else { stars as i64 };
        let rank_before = rank_of(xp_before);
        s.profile.xp += gain;
        let rank_up = rank_of(s.profile.xp) > rank_before;
        if rank_up {
            clocks_won += 3;
        }
        s.profile.clocks += clocks_won;
        // daily challenge: the first success of the day pays 3 clocks
        let mut daily_won = false;
        if let Mode::Daily(target) = res.mode {
            if res.score >= target && !s.daily_done_today() {
                s.profile.daily_done = s.today;
                s.profile.daily_total += 1;
                s.profile.clocks += 3;
                daily_won = true;
            }
        }
        let total_after = s.profile.total_stars();
        let unlocked_now: Vec<usize> = s
            .levels()
            .iter()
            .enumerate()
            .filter(|(_, d)| d.unlock > stars_before_total && d.unlock <= total_after)
            .map(|(i, _)| i)
            .collect();
        s.profile.save();
        let mut sc = ScoreScene {
            def,
            t: 0.0,
            stars,
            prev_best: rec.best,
            new_best,
            xp_before,
            xp_after: s.profile.xp,
            rank_up,
            unlocked_now,
            clocks_won,
            btn_menu: Button::round(0.0, 0.0, 100.0, Icon::Menu, BLUE),
            btn_retry: Button::round(0.0, 0.0, 120.0, Icon::Retry, ORANGE),
            btn_next: Button::round(0.0, 0.0, 100.0, Icon::Next, GREEN),
            star_fired: 0,
            fx: levels::Fx::default(),
            daily_won,
            res,
        };
        sc.layout(s);
        if def.two_player {
            audio::play(Sfx::Win);
        }
        sc
    }
    fn next_idx(&self, s: &Shared) -> Option<usize> {
        let n = self.res.idx + 1;
        (n < s.levels().len() && s.unlocked(n)).then_some(n)
    }
    fn layout(&mut self, s: &Shared) {
        let cx = (s.safe_l + s.safe_r) * 0.5;
        let by = s.h * 0.5 + 240.0;
        self.btn_menu.pos = v2(cx - 170.0, by);
        self.btn_retry.pos = v2(cx, by - 8.0);
        self.btn_next.pos = v2(cx + 170.0, by);
        self.btn_next.visible = self.next_idx(s).is_some();
    }
    fn touch(&mut self, s: &mut Shared, t: &Touch) -> Option<Nav> {
        if self.t < 0.5 {
            return None;
        }
        if self.btn_menu.touch(t) {
            return Some(Nav::Select(Some(self.res.idx)));
        }
        if self.btn_retry.touch(t) {
            return Some(Nav::Play(self.res.idx, self.res.mode, true));
        }
        if self.btn_next.touch(t) {
            if let Some(n) = self.next_idx(s) {
                return Some(Nav::Play(n, Mode::Normal, false));
            }
        }
        None
    }
    fn update(&mut self, s: &mut Shared, dt: f32) -> Option<Nav> {
        self.t += dt;
        self.fx.update(dt);
        for b in [&mut self.btn_menu, &mut self.btn_retry, &mut self.btn_next] {
            b.update(dt);
        }
        let cx = (s.safe_l + s.safe_r) * 0.5;
        // stars pop one by one after the count-up
        let start = 1.1;
        while self.star_fired < self.stars && self.t > start + self.star_fired as f32 * 0.35 {
            let i = self.star_fired;
            self.star_fired += 1;
            audio::play_ex(Sfx::Star, 1.0, 1.0 + i as f32 * 0.08, 0.0);
            s.haptic(REQ_HAPTIC_TICK);
            self.fx.stars(v2(cx + (i as f32 - 2.0) * 84.0, s.h * 0.5 - 30.0), 8);
            if self.star_fired == 5 {
                self.fx.confetti(v2(cx, s.h * 0.3), 60);
                audio::play(Sfx::Win);
            }
        }
        if self.t > start + 1.9 && self.t - dt <= start + 1.9 {
            if self.rank_up {
                audio::play(Sfx::Rank);
                s.toast("New rank!", RANKS[rank_of(self.xp_after)].1, Icon::Star);
            }
            for &u in &self.unlocked_now {
                audio::play(Sfx::Unlock);
                s.toast("New game unlocked!", s.levels()[u].title, Icon::Play);
            }
            if self.daily_won {
                audio::play(Sfx::Achievement);
                s.toast("Daily challenge complete!", "+3 clocks. See you tomorrow!", Icon::Calendar);
            }
            if self.clocks_won > 0 {
                s.toast(&format!("+{} clock{}", self.clocks_won, if self.clocks_won > 1 { "s" } else { "" }), "Use clocks for bonus time", Icon::Clock);
            }
            self.layout(s);
        }
        None
    }
    fn draw(&mut self, r: &mut Renderer, s: &Shared) {
        draw_pond_bg(r, self.t + 10.0);
        ui::dim(r, 0.3);
        let cx = (s.safe_l + s.safe_r) * 0.5;
        let cy = s.h * 0.5;
        let k = ease_out_back((self.t / 0.4).min(1.0));
        r.push();
        r.translate(cx, cy);
        r.scale(k, k);
        r.translate(-cx, -cy);
        ui::panel(r, cx, cy - 10.0, 860.0, 600.0, PANEL);
        r.text_bold(self.def.title, cx, cy - 250.0, 52.0, Align::Center, self.def.color.shade(-0.1), INK);
        if self.def.two_player {
            let cpu = self.res.mode == Mode::Cpu;
            let (label, col, skin) = match self.res.winner {
                Some(0) if cpu => ("YOU WIN!", RED, art::RUBY),
                Some(1) if cpu => ("CPU WINS!", BLUE, art::SKY),
                Some(0) => ("RED WINS!", RED, art::RUBY),
                Some(1) => ("BLUE WINS!", BLUE, art::SKY),
                _ => ("DRAW!", ORANGE, art::EMERALD),
            };
            r.text_bold(label, cx, cy - 130.0, 90.0, Align::Center, col, INK);
            let pose = Pose { happy_eyes: true, mouth: 0.7, sprout: true, squash: (self.t * 8.0).sin() * 0.05, ..Pose::default() };
            art::frog(r, cx, cy + 20.0, 180.0, skin, &pose);
        } else {
            let count = ((self.t - 0.3) / 0.8).clamp(0.0, 1.0);
            let shown = (self.res.score as f32 * ease_out_cubic(count)).round() as i64;
            r.text_bold(&shown.to_string(), cx, cy - 150.0, 96.0, Align::Center, GOLD, INK);
            r.text(self.def.unit, cx, cy - 92.0, 28.0, Align::Center, hex(0x2F6F4F));
            for i in 0..5 {
                let x = cx + (i as f32 - 2.0) * 84.0;
                let lit = i < self.star_fired as usize;
                let st = 1.1 + i as f32 * 0.35;
                let sc = if lit { 1.0 + wobble(self.t - st, 18.0) * 0.4 } else { 1.0 };
                r.push();
                r.translate(x, cy - 30.0);
                r.scale(sc, sc);
                art::star_icon(r, 0.0, 0.0, 36.0, lit);
                r.pop();
                r.text(&self.def.thresholds[i].to_string(), x, cy + 18.0, 20.0, Align::Center, hex(0x2F6F4F).a(0.8));
            }
            if self.new_best && self.t > 1.0 {
                let b = ease_out_back(((self.t - 1.0) / 0.3).min(1.0));
                r.push();
                r.translate(cx + 300.0, cy - 175.0);
                r.rotate(0.25);
                r.scale(b, b);
                r.rrect(0.0, 0.0, 200.0, 56.0, 20.0, RED, INK, 4.0);
                r.text_bold("NEW BEST!", 0.0, 0.0, 30.0, Align::Center, Col::WHITE, INK);
                r.pop();
            } else if self.prev_best > 0 {
                r.text(&format!("Best: {}", self.prev_best.max(self.res.score)), cx, cy + 62.0, 28.0, Align::Center, hex(0x2F6F4F));
            }
        }
        // rank bar
        let rp = ((self.t - 1.2) / 1.2).clamp(0.0, 1.0);
        let xp_now = self.xp_before + ((self.xp_after - self.xp_before) as f32 * ease_in_out(rp)).round() as i64;
        let rk = rank_of(xp_now);
        let by = cy + 120.0;
        rank_badge(r, cx - 250.0, by, 30.0, rk);
        r.text_bold(RANKS[rk].1, cx - 205.0, by - 14.0, 26.0, Align::Left, hex(0x23306E), Col::WHITE);
        let p = rank_progress(xp_now);
        r.rrect(cx + 30.0, by + 14.0, 470.0, 18.0, 9.0, Col(0, 0, 0, 50), Col::CLEAR, 0.0);
        if p > 0.0 {
            r.rrect(cx - 205.0 + 235.0 * p, by + 14.0, 470.0 * p, 18.0, 9.0, hex(0x4CC38A), Col::CLEAR, 0.0);
        }
        if self.xp_after > self.xp_before {
            r.text_bold(&format!("+{}", self.xp_after - self.xp_before), cx + 290.0, by - 12.0, 28.0, Align::Center, GREEN, INK);
        }
        if !self.res.new_ach.is_empty() {
            let names: Vec<&str> = self.res.new_ach.iter().map(|&k| self.def.ach[k].name).collect();
            ui::draw_icon(r, Icon::Trophy, cx - 250.0, cy + 180.0, 40.0, GOLD);
            r.text(&names.join(", "), cx + 20.0, cy + 180.0, 24.0, Align::Center, hex(0x6A3DB8));
        }
        r.pop();
        self.btn_menu.draw(r);
        self.btn_retry.draw(r);
        self.btn_next.draw(r);
        self.fx.draw(r);
    }
}

// ------------------------------------------------------------------ achievements

struct AchScene {
    back: Button,
    scroll: f32,
    vel: f32,
    drag: Option<(i32, f32, f32)>,
    t: f32,
}

impl AchScene {
    fn new(s: &Shared) -> AchScene {
        AchScene { back: back_button(s), scroll: 0.0, vel: 0.0, drag: None, t: 0.0 }
    }
    fn layout(&mut self, s: &Shared) {
        self.back = back_button(s);
    }
    fn rows(s: &Shared) -> usize {
        s.levels().iter().filter(|d| !d.ach.is_empty()).count()
    }
    fn max_scroll(&self, s: &Shared) -> f32 {
        (Self::rows(s) as f32 * 190.0 + 150.0 - s.h).max(0.0)
    }
    fn touch(&mut self, s: &mut Shared, t: &Touch) -> Option<Nav> {
        if self.back.touch(t) {
            return Some(Nav::Menu);
        }
        match t.phase {
            Phase::Down => {
                if self.drag.is_none() {
                    self.drag = Some((t.id, t.pos.y, self.scroll));
                    self.vel = 0.0;
                }
            }
            Phase::Move => {
                if let Some((id, y0, s0)) = self.drag {
                    if id == t.id {
                        let prev = self.scroll;
                        self.scroll = (s0 - (t.pos.y - y0)).clamp(-80.0, self.max_scroll(s) + 80.0);
                        self.vel = (self.scroll - prev) * 60.0;
                    }
                }
            }
            _ => {
                if self.drag.map(|d| d.0) == Some(t.id) {
                    self.drag = None;
                }
            }
        }
        None
    }
    fn update(&mut self, s: &mut Shared, dt: f32) -> Option<Nav> {
        self.t += dt;
        self.back.update(dt);
        if self.drag.is_none() {
            self.scroll += self.vel * dt;
            self.vel *= (1.0 - dt * 4.0).max(0.0);
            let m = self.max_scroll(s);
            if self.scroll < 0.0 {
                self.scroll += -self.scroll * (dt * 12.0).min(1.0);
            } else if self.scroll > m {
                self.scroll += (m - self.scroll) * (dt * 12.0).min(1.0);
            }
        }
        None
    }
    fn draw(&mut self, r: &mut Renderer, s: &Shared) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0x5B3FA8), hex(0x2B2370));
        let cx = (s.safe_l + s.safe_r) * 0.5;
        let width = (s.safe_r - s.safe_l - 140.0).min(1300.0);
        let mut done = 0;
        let mut total = 0;
        for (i, def) in s.levels().iter().filter(|d| !d.ach.is_empty()).enumerate() {
            let y = 150.0 + i as f32 * 190.0 - self.scroll + 60.0;
            for k in 0..def.ach.len() {
                total += 1;
                if s.profile.has_ach(&ach_key(def.id, k)) {
                    done += 1;
                }
            }
            if y < -120.0 || y > h + 120.0 {
                continue;
            }
            r.rrect(cx, y, width, 170.0, 26.0, Col::WHITE.a(0.1), Col::WHITE.a(0.25), 3.0);
            r.text_bold(def.title, cx - width * 0.5 + 30.0, y - 58.0, 30.0, Align::Left, def.color.shade(0.3), INK);
            let tw = (width - 60.0) / 4.0;
            for k in 0..def.ach.len() {
                let a = &def.ach[k];
                let got = s.profile.has_ach(&ach_key(def.id, k));
                let x = cx - width * 0.5 + 30.0 + tw * k as f32 + tw * 0.5;
                let ty = y + 22.0;
                r.rrect(x, ty, tw - 16.0, 104.0, 18.0, if got { hex(0xFFF4D6) } else { Col(0, 0, 0, 60) }, if got { GOLD } else { Col::WHITE.a(0.2) }, 3.0);
                r.push();
                if !got {
                    r.fade(0.45);
                }
                ui::draw_icon(r, Icon::Trophy, x - tw * 0.5 + 44.0, ty - 8.0, 46.0, if got { GOLD } else { hex(0xB9C4C9) });
                r.pop();
                let tc = if got { INK } else { Col::WHITE.a(0.85) };
                r.text(a.name, x + 22.0, ty - 28.0, 21.0, Align::Center, tc);
                r.text(a.desc, x + 22.0, ty + 12.0, 15.0, Align::Center, if got { hex(0x2F6F4F) } else { Col::WHITE.a(0.6) });
                if !got && a.kind == AchKind::Total {
                    let have = s.profile.level(def.id).total.min(a.target);
                    r.text(&format!("{have}/{}", a.target), x + 22.0, ty + 40.0, 16.0, Align::Center, GOLD);
                }
            }
        }
        r.grad_rect(0.0, 0.0, w, 130.0, hex(0x5B3FA8), hex(0x5B3FA8).a(0.0));
        r.text_bold("Achievements", cx, 62.0, 54.0, Align::Center, GOLD, INK);
        r.text_bold(&format!("{done}/{total}"), s.safe_r - 100.0, 62.0, 34.0, Align::Center, Col::WHITE, INK);
        self.back.draw(r);
    }
}

// ------------------------------------------------------------------ options

struct OptionsScene {
    back: Button,
    music: Button,
    sound: Button,
    haptic: Button,
    marks: Button,
    updates: Button,
    reset: Button,
    credits: Button,
    t: f32,
}

impl OptionsScene {
    fn new(s: &Shared) -> OptionsScene {
        let mut o = OptionsScene {
            back: back_button(s),
            music: Button::new(0.0, 0.0, 420.0, 90.0, "", Icon::Music(true), GREEN),
            sound: Button::new(0.0, 0.0, 420.0, 90.0, "", Icon::Sound(true), GREEN),
            haptic: Button::new(0.0, 0.0, 420.0, 90.0, "", Icon::Haptic(true), GREEN),
            marks: Button::new(0.0, 0.0, 420.0, 90.0, "", Icon::Star, GREEN),
            updates: Button::new(0.0, 0.0, 420.0, 90.0, "", Icon::Down, GREEN),
            reset: Button::new(0.0, 0.0, 420.0, 90.0, "Reset progress", Icon::Retry, RED),
            credits: Button::new(0.0, 0.0, 420.0, 90.0, "Credits", Icon::Info, PURPLE),
            t: 0.0,
        };
        o.layout(s);
        o.refresh(s);
        o
    }
    fn layout(&mut self, s: &Shared) {
        self.back = back_button(s);
        let cx = (s.safe_l + s.safe_r) * 0.5;
        let lx = cx - 240.0;
        let rx = cx + 240.0;
        let row = |i: usize| 225.0 + i as f32 * 97.0;
        for b in [&mut self.music, &mut self.sound, &mut self.haptic, &mut self.marks, &mut self.updates, &mut self.reset, &mut self.credits] {
            b.h = 86.0;
        }
        self.music.pos = v2(lx, row(0));
        self.sound.pos = v2(lx, row(1));
        self.haptic.pos = v2(lx, row(2));
        self.marks.pos = v2(lx, row(3));
        self.updates.pos = v2(rx, row(0));
        self.credits.pos = v2(rx, row(1));
        self.reset.pos = v2(rx, row(2));
    }
    fn refresh(&mut self, s: &Shared) {
        let p = &s.profile;
        let set = |b: &mut Button, on: bool, name: &str, icon: Icon| {
            b.label = format!("{name}: {}", if on { "On" } else { "Off" });
            b.icon = icon;
            b.color = if on { GREEN } else { hex(0x8C9A94) };
        };
        set(&mut self.music, p.music, "Music", Icon::Music(p.music));
        set(&mut self.sound, p.sfx, "Sound", Icon::Sound(p.sfx));
        set(&mut self.haptic, p.haptics, "Vibration", Icon::Haptic(p.haptics));
        set(&mut self.marks, p.color_marks, "Color marks", Icon::Star);
        set(&mut self.updates, p.update_check, "Updates", Icon::Down);
    }
    fn touch(&mut self, s: &mut Shared, t: &Touch) -> (Option<Nav>, bool) {
        if self.back.touch(t) {
            return (Some(Nav::Menu), false);
        }
        if self.music.touch(t) {
            s.profile.music = !s.profile.music;
            audio::set_music_enabled(s.profile.music);
            s.profile.dirty = true;
        }
        if self.sound.touch(t) {
            s.profile.sfx = !s.profile.sfx;
            audio::set_sfx_enabled(s.profile.sfx);
            s.profile.dirty = true;
        }
        if self.haptic.touch(t) {
            s.profile.haptics = !s.profile.haptics;
            s.profile.dirty = true;
            s.haptic(REQ_HAPTIC_CONFIRM);
        }
        if self.marks.touch(t) {
            s.profile.color_marks = !s.profile.color_marks;
            art::set_color_marks(s.profile.color_marks);
            s.profile.dirty = true;
        }
        if self.updates.touch(t) {
            s.profile.update_check = !s.profile.update_check;
            s.profile.dirty = true;
            if s.profile.update_check {
                s.requests |= REQ_CHECK_UPDATE;
            } else {
                s.update = None;
            }
        }
        if self.credits.touch(t) {
            return (Some(Nav::Credits), false);
        }
        let reset = self.reset.touch(t);
        self.refresh(s);
        (None, reset)
    }
    fn update(&mut self, _s: &mut Shared, dt: f32) -> Option<Nav> {
        self.t += dt;
        for b in [&mut self.back, &mut self.music, &mut self.sound, &mut self.haptic, &mut self.marks, &mut self.updates, &mut self.reset, &mut self.credits] {
            b.update(dt);
        }
        None
    }
    fn draw(&mut self, r: &mut Renderer, s: &Shared) {
        draw_pond_bg(r, self.t);
        ui::dim(r, 0.25);
        let cx = (s.safe_l + s.safe_r) * 0.5;
        ui::panel(r, cx, 395.0, 1060.0, 540.0, PANEL);
        r.text_bold("Options", cx, 66.0, 60.0, Align::Center, Col::WHITE, INK);
        for b in [&self.music, &self.sound, &self.haptic, &self.marks, &self.updates, &self.reset, &self.credits] {
            b.draw(r);
        }
        r.text("Color marks add a shape to every frog colour.", cx, 588.0, 22.0, Align::Center, hex(0x2F6F4F));
        r.text("Updates asks GitHub now and then whether a new version is out.", cx, 614.0, 22.0, Align::Center, hex(0x2F6F4F));
        let ver = if s.version.is_empty() { "Ribbit Revived".to_string() } else { format!("Version {}", s.version) };
        r.text(&ver, cx, 640.0, 20.0, Align::Center, hex(0x2F6F4F).a(0.7));
        self.back.draw(r);
    }
}

// ------------------------------------------------------------------ credits

struct CreditsScene {
    back: Button,
    t: f32,
}

impl CreditsScene {
    fn new(s: &Shared) -> CreditsScene {
        CreditsScene { back: back_button(s), t: 0.0 }
    }
    fn layout(&mut self, s: &Shared) {
        self.back = back_button(s);
    }
    fn touch(&mut self, _s: &mut Shared, t: &Touch) -> Option<Nav> {
        if self.back.touch(t) {
            return Some(Nav::Options);
        }
        None
    }
    fn update(&mut self, _s: &mut Shared, dt: f32) -> Option<Nav> {
        self.t += dt;
        self.back.update(dt);
        None
    }
    fn draw(&mut self, r: &mut Renderer, s: &Shared) {
        let (w, h) = (r.w, r.h);
        r.grad_rect(0.0, 0.0, w, h, hex(0x1F7A5A), hex(0x12382A));
        let cx = (s.safe_l + s.safe_r) * 0.5;
        let lines = [
            ("RIBBIT REVIVED", 64.0, hex(0xB4F07A)),
            ("", 20.0, Col::WHITE),
            ("Game design & code", 26.0, GOLD),
            ("riyadm", 40.0, Col::WHITE),
            ("", 16.0, Col::WHITE),
            ("Engine", 26.0, GOLD),
            ("Rust native core  +  Kotlin Android shell", 30.0, Col::WHITE),
            ("", 16.0, Col::WHITE),
            ("Art & audio", 26.0, GOLD),
            ("Every frog, sprite, sound and tune is generated in code", 26.0, Col::WHITE),
            ("", 16.0, Col::WHITE),
            ("Thanks for playing!", 34.0, hex(0xFF9F2E)),
        ];
        let mut y = 120.0;
        for (txt, size, col) in lines {
            if !txt.is_empty() {
                r.text_bold(txt, cx, y, size, Align::Center, col, INK);
            }
            y += size * 1.35;
        }
        let pose = Pose::idle(self.t, 1.0);
        art::frog(r, s.safe_r - 160.0, h - 120.0, 160.0, art::EMERALD, &Pose { sprout: true, ..pose });
        art::frog(r, s.safe_l + 170.0, h - 110.0, 120.0, art::BLOSSOM, &Pose::idle(self.t, 2.0));
        self.back.draw(r);
    }
}
