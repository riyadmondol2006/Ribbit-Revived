//! Software audio: procedurally synthesised sound effects plus a tiny chiptune sequencer
//! for the music. The Kotlin audio thread pulls mixed 16-bit stereo frames via `render`.
#![allow(dead_code)]

use std::f32::consts::TAU;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Sfx {
    Pop,
    PopHigh,
    Ribbit,
    Jump,
    DoubleJump,
    Collect,
    Coin,
    Wrong,
    Click,
    Beep,
    Go,
    Star,
    Win,
    TimeUp,
    Splash,
    Whoosh,
    Unlock,
    Achievement,
    Eat,
    Thud,
    Bounce,
    Pump,
    Kick,
    Whistle,
    Splat,
    Crack,
    Swap,
    Rank,
    Clock,
    Tick,
    Burst,
    Note,
}
const SFX_COUNT: usize = Sfx::Note as usize + 1;

/// C-major scale steps (do re mi fa sol la si do) as playback-rate multipliers for `Sfx::Note`.
pub const SCALE: [f32; 8] = [1.0, 1.122_462, 1.259_921, 1.334_84, 1.498_307, 1.681_793, 1.887_749, 2.0];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Track {
    Menu,
    Level,
}

enum Cmd {
    Play(Sfx, f32, f32, f32),
    Music(Option<Track>),
    MusicOn(bool),
    SfxOn(bool),
}

static QUEUE: Mutex<Vec<Cmd>> = Mutex::new(Vec::new());
static MIXER: Mutex<Option<Mixer>> = Mutex::new(None);

pub fn init(sample_rate: u32) {
    let m = Mixer::new(sample_rate.max(8000));
    *MIXER.lock().unwrap() = Some(m);
}

fn push(c: Cmd) {
    if let Ok(mut q) = QUEUE.lock() {
        if q.len() < 256 {
            q.push(c);
        }
    }
}

pub fn play(s: Sfx) {
    push(Cmd::Play(s, 1.0, 1.0, 0.0));
}
/// Plays with volume, pitch multiplier and stereo pan (-1 left .. 1 right).
pub fn play_ex(s: Sfx, vol: f32, pitch: f32, pan: f32) {
    push(Cmd::Play(s, vol, pitch, pan));
}
pub fn music(t: Option<Track>) {
    push(Cmd::Music(t));
}
pub fn set_music_enabled(on: bool) {
    push(Cmd::MusicOn(on));
}
pub fn set_sfx_enabled(on: bool) {
    push(Cmd::SfxOn(on));
}

/// Called from the audio thread.
pub fn render(out: &mut [i16], frames: usize) {
    let cmds: Vec<Cmd> = QUEUE.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
    let mut guard = MIXER.lock().unwrap();
    match guard.as_mut() {
        Some(m) => {
            for c in cmds {
                m.apply(c);
            }
            m.render(out, frames);
        }
        None => out.iter_mut().for_each(|s| *s = 0),
    }
}

// ------------------------------------------------------------------ mixer

struct Voice {
    data: Arc<Vec<f32>>,
    pos: f32,
    rate: f32,
    vol: f32,
    pan: f32,
}

struct Mixer {
    sr: f32,
    bank: Vec<Arc<Vec<f32>>>,
    voices: Vec<Voice>,
    music_on: bool,
    sfx_on: bool,
    seq: Sequencer,
    music_gain: f32,
    music_target: f32,
}

impl Mixer {
    fn new(sr: u32) -> Mixer {
        let sr = sr as f32;
        let bank = (0..SFX_COUNT).map(|i| Arc::new(synth_sfx(i, sr))).collect();
        Mixer {
            sr,
            bank,
            voices: Vec::with_capacity(32),
            music_on: true,
            sfx_on: true,
            seq: Sequencer::new(sr),
            music_gain: 0.0,
            music_target: 0.0,
        }
    }

    fn apply(&mut self, c: Cmd) {
        match c {
            Cmd::Play(s, vol, pitch, pan) => {
                if !self.sfx_on {
                    return;
                }
                if self.voices.len() >= 28 {
                    // drop the oldest voice to keep the mix clean
                    self.voices.remove(0);
                }
                self.voices.push(Voice {
                    data: self.bank[s as usize].clone(),
                    pos: 0.0,
                    rate: pitch.clamp(0.25, 4.0),
                    vol,
                    pan: pan.clamp(-1.0, 1.0),
                });
            }
            Cmd::Music(t) => {
                if t != self.seq.track {
                    self.seq.set_track(t);
                    self.music_gain = 0.0;
                }
                self.music_target = if t.is_some() { 1.0 } else { 0.0 };
            }
            Cmd::MusicOn(on) => self.music_on = on,
            Cmd::SfxOn(on) => {
                self.sfx_on = on;
                if !on {
                    self.voices.clear();
                }
            }
        }
    }

    fn render(&mut self, out: &mut [i16], frames: usize) {
        let fade = 1.0 / (self.sr * 0.6);
        for i in 0..frames {
            let mut l = 0.0f32;
            let mut r = 0.0f32;
            // music
            let target = if self.music_on { self.music_target } else { 0.0 };
            if self.music_gain < target {
                self.music_gain = (self.music_gain + fade).min(target);
            } else if self.music_gain > target {
                self.music_gain = (self.music_gain - fade).max(target);
            }
            if self.music_gain > 0.0 {
                let (ml, mr) = self.seq.next();
                l += ml * 0.30 * self.music_gain;
                r += mr * 0.30 * self.music_gain;
            }
            // effects
            for v in self.voices.iter_mut() {
                let idx = v.pos as usize;
                if idx + 1 >= v.data.len() {
                    v.vol = -1.0;
                    continue;
                }
                let frac = v.pos - idx as f32;
                let s = v.data[idx] + (v.data[idx + 1] - v.data[idx]) * frac;
                let s = s * v.vol * 0.55;
                l += s * (1.0 - v.pan.max(0.0));
                r += s * (1.0 + v.pan.min(0.0));
                v.pos += v.rate;
            }
            // soft clip
            let l = soft_clip(l);
            let r = soft_clip(r);
            out[i * 2] = (l * 32000.0) as i16;
            out[i * 2 + 1] = (r * 32000.0) as i16;
        }
        self.voices.retain(|v| v.vol >= 0.0);
    }
}

fn soft_clip(x: f32) -> f32 {
    if x.abs() < 0.6 {
        x
    } else {
        x.signum() * (0.6 + 0.4 * ((x.abs() - 0.6) / 0.4).tanh())
    }
}

// ------------------------------------------------------------------ synthesis helpers

struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn square(phase: f32, duty: f32) -> f32 {
    if phase.fract() < duty {
        1.0
    } else {
        -1.0
    }
}
fn tri(phase: f32) -> f32 {
    let p = phase.fract();
    if p < 0.5 {
        4.0 * p - 1.0
    } else {
        3.0 - 4.0 * p
    }
}
fn saw(phase: f32) -> f32 {
    2.0 * phase.fract() - 1.0
}
fn midi_hz(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

/// Generates `dur` seconds by calling `f(t, dt)` per sample; `f` keeps its own phase state.
fn gen(sr: f32, dur: f32, mut f: impl FnMut(f32, f32) -> f32) -> Vec<f32> {
    let n = (sr * dur) as usize;
    let dt = 1.0 / sr;
    let mut v = Vec::with_capacity(n + 2);
    for i in 0..n {
        let t = i as f32 * dt;
        // short fade in/out avoids clicks
        let edge = ((i as f32 / (sr * 0.002)).min(1.0)) * (((n - i) as f32 / (sr * 0.004)).min(1.0));
        v.push(f(t, dt) * edge);
    }
    v.push(0.0);
    v.push(0.0);
    v
}

/// Mixes several generated clips with start offsets (seconds).
fn layer(sr: f32, parts: Vec<(f32, Vec<f32>)>) -> Vec<f32> {
    let len = parts.iter().map(|(o, p)| (o * sr) as usize + p.len()).max().unwrap_or(0);
    let mut out = vec![0.0; len];
    for (o, p) in parts {
        let off = (o * sr) as usize;
        for (i, s) in p.iter().enumerate() {
            out[off + i] += s;
        }
    }
    out
}

fn tone(sr: f32, hz0: f32, hz1: f32, dur: f32, decay: f32, wave: fn(f32) -> f32, vol: f32) -> Vec<f32> {
    let mut ph = 0.0;
    gen(sr, dur, move |t, dt| {
        let k = t / dur;
        let hz = hz0 * (hz1 / hz0).powf(k);
        ph += hz * dt;
        wave(ph) * (-t * decay).exp() * vol
    })
}
fn sine(p: f32) -> f32 {
    (p * TAU).sin()
}
fn sq50(p: f32) -> f32 {
    square(p, 0.5) * 0.6
}
fn sq25(p: f32) -> f32 {
    square(p, 0.25) * 0.6
}

fn synth_sfx(id: usize, sr: f32) -> Vec<f32> {
    let mut nz = Noise(0x1234_5678 ^ (id as u32 * 7919 + 1));
    match id {
        x if x == Sfx::Pop as usize => {
            let body = tone(sr, 950.0, 180.0, 0.12, 28.0, sine, 0.9);
            let click = gen(sr, 0.015, |t, _| nz.next() * (-t * 300.0).exp() * 0.5);
            layer(sr, vec![(0.0, body), (0.0, click)])
        }
        x if x == Sfx::PopHigh as usize => {
            let body = tone(sr, 1500.0, 420.0, 0.1, 30.0, sine, 0.85);
            let click = gen(sr, 0.012, |t, _| nz.next() * (-t * 300.0).exp() * 0.45);
            layer(sr, vec![(0.0, body), (0.0, click)])
        }
        x if x == Sfx::Ribbit as usize => {
            // two croak syllables: buzzy saw through a resonant band-pass, amplitude-pulsed
            let syll = |sr: f32, base: f32, dur: f32| {
                let mut ph = 0.0;
                let (mut lp1, mut bp) = (0.0f32, 0.0f32);
                gen(sr, dur, move |t, dt| {
                    let hz = base * (1.0 + 0.25 * (t / dur));
                    ph += hz * dt;
                    let x = saw(ph);
                    // state-variable filter around 750 Hz
                    let f = 2.0 * (std::f32::consts::PI * 750.0 / sr).sin();
                    let hp = x - lp1 - 0.35 * bp;
                    bp += f * hp;
                    lp1 += f * bp;
                    let am = 0.55 + 0.45 * (t * 38.0 * TAU).sin();
                    let env = (t / 0.015).min(1.0) * (1.0 - t / dur).max(0.0).powf(0.6);
                    bp * am * env * 0.9
                })
            };
            layer(sr, vec![(0.0, syll(sr, 150.0, 0.13)), (0.17, syll(sr, 165.0, 0.17))])
        }
        x if x == Sfx::Jump as usize => tone(sr, 280.0, 820.0, 0.16, 9.0, sq25, 0.6),
        x if x == Sfx::DoubleJump as usize => {
            layer(sr, vec![(0.0, tone(sr, 420.0, 1100.0, 0.12, 10.0, sq25, 0.55)), (0.06, tone(sr, 700.0, 1600.0, 0.12, 12.0, sine, 0.4))])
        }
        x if x == Sfx::Collect as usize => layer(
            sr,
            vec![(0.0, tone(sr, 1318.5, 1318.5, 0.09, 18.0, sine, 0.7)), (0.06, tone(sr, 1975.5, 1975.5, 0.16, 14.0, sine, 0.6))],
        ),
        x if x == Sfx::Coin as usize => layer(
            sr,
            vec![(0.0, tone(sr, 987.8, 987.8, 0.07, 10.0, sq50, 0.5)), (0.07, tone(sr, 1318.5, 1318.5, 0.22, 9.0, sq50, 0.5))],
        ),
        x if x == Sfx::Wrong as usize => {
            let mut p1 = 0.0;
            let mut p2 = 0.0;
            gen(sr, 0.28, move |t, dt| {
                p1 += 140.0 * dt;
                p2 += 148.0 * dt;
                (saw(p1) + saw(p2)) * 0.35 * (1.0 - t / 0.28)
            })
        }
        x if x == Sfx::Click as usize => {
            let a = gen(sr, 0.03, |t, _| nz.next() * (-t * 220.0).exp() * 0.4);
            layer(sr, vec![(0.0, a), (0.0, tone(sr, 1800.0, 1200.0, 0.04, 60.0, sine, 0.5))])
        }
        x if x == Sfx::Beep as usize => tone(sr, 880.0, 880.0, 0.16, 12.0, sq25, 0.55),
        x if x == Sfx::Go as usize => layer(sr, vec![(0.0, tone(sr, 1318.5, 1318.5, 0.36, 5.0, sq25, 0.5)), (0.0, tone(sr, 659.3, 659.3, 0.36, 5.0, sine, 0.4))]),
        x if x == Sfx::Star as usize => {
            let notes = [1046.5, 1318.5, 1568.0, 2093.0];
            layer(sr, notes.iter().enumerate().map(|(i, &h)| (i as f32 * 0.05, tone(sr, h, h, 0.6, 6.0, sine, 0.45))).collect())
        }
        x if x == Sfx::Win as usize => {
            let notes = [523.3, 659.3, 784.0, 1046.5];
            let mut parts: Vec<(f32, Vec<f32>)> =
                notes.iter().enumerate().map(|(i, &h)| (i as f32 * 0.1, tone(sr, h, h, 0.18, 8.0, sq25, 0.45))).collect();
            parts.push((0.4, tone(sr, 1046.5, 1046.5, 0.6, 3.5, sq50, 0.4)));
            parts.push((0.4, tone(sr, 1318.5, 1318.5, 0.6, 3.5, sine, 0.35)));
            layer(sr, parts)
        }
        x if x == Sfx::TimeUp as usize => layer(
            sr,
            vec![(0.0, tone(sr, 900.0, 900.0, 0.12, 4.0, sq25, 0.5)), (0.14, tone(sr, 700.0, 700.0, 0.12, 4.0, sq25, 0.5)), (0.28, tone(sr, 520.0, 260.0, 0.45, 4.0, sq25, 0.5))],
        ),
        x if x == Sfx::Splash as usize => {
            let mut lp = 0.0;
            gen(sr, 0.45, move |t, _| {
                let cut = 0.02 + 0.25 * (-t * 7.0).exp();
                lp += cut * (nz.next() - lp);
                lp * 1.6 * (-t * 6.0).exp()
            })
        }
        x if x == Sfx::Whoosh as usize => {
            let (mut lp, mut lp2) = (0.0, 0.0);
            gen(sr, 0.3, move |t, _| {
                let k = t / 0.3;
                let cut = 0.03 + 0.25 * k;
                lp += cut * (nz.next() - lp);
                lp2 += cut * 0.5 * (lp - lp2);
                (lp - lp2) * 2.2 * (k * std::f32::consts::PI).sin()
            })
        }
        x if x == Sfx::Unlock as usize => {
            let notes = [784.0, 987.8, 1174.7, 1568.0, 1975.5];
            layer(sr, notes.iter().enumerate().map(|(i, &h)| (i as f32 * 0.07, tone(sr, h, h, 0.4, 7.0, sine, 0.45))).collect())
        }
        x if x == Sfx::Achievement as usize => layer(
            sr,
            vec![
                (0.0, tone(sr, 784.0, 784.0, 0.14, 6.0, sq25, 0.4)),
                (0.12, tone(sr, 1046.5, 1046.5, 0.14, 6.0, sq25, 0.4)),
                (0.24, tone(sr, 1568.0, 1568.0, 0.5, 3.0, sq50, 0.4)),
                (0.24, tone(sr, 1975.5, 1975.5, 0.5, 4.0, sine, 0.3)),
            ],
        ),
        x if x == Sfx::Eat as usize => {
            let mut chomp = |off: f32| {
                let mut lp = 0.0;
                let a = gen(sr, 0.07, |t, _| {
                    lp += 0.25 * (nz.next() - lp);
                    lp * 1.6 * (-t * 40.0).exp()
                });
                (off, a)
            };
            let a = chomp(0.0);
            let b = chomp(0.09);
            layer(sr, vec![a, b, (0.0, tone(sr, 300.0, 160.0, 0.08, 30.0, sine, 0.5))])
        }
        x if x == Sfx::Thud as usize => {
            let n = gen(sr, 0.05, |t, _| nz.next() * (-t * 90.0).exp() * 0.35);
            layer(sr, vec![(0.0, tone(sr, 150.0, 55.0, 0.16, 18.0, sine, 1.0)), (0.0, n)])
        }
        x if x == Sfx::Bounce as usize => {
            let mut ph = 0.0;
            gen(sr, 0.3, move |t, dt| {
                let hz = 180.0 + 320.0 * (1.0 - (-t * 12.0).exp()) + 30.0 * (t * 40.0).sin();
                ph += hz * dt;
                sine(ph) * (-t * 9.0).exp() * 0.8
            })
        }
        x if x == Sfx::Pump as usize => {
            let mut lp = 0.0;
            gen(sr, 0.22, move |t, _| {
                let k = t / 0.22;
                lp += (0.05 + 0.2 * k) * (nz.next() - lp);
                lp * 1.4 * (k * std::f32::consts::PI).sin()
            })
        }
        x if x == Sfx::Kick as usize => {
            let n = gen(sr, 0.02, |t, _| nz.next() * (-t * 200.0).exp() * 0.5);
            layer(sr, vec![(0.0, tone(sr, 320.0, 90.0, 0.12, 25.0, sine, 0.9)), (0.0, n)])
        }
        x if x == Sfx::Whistle as usize => {
            let mut ph = 0.0;
            gen(sr, 0.45, move |t, dt| {
                ph += (2100.0 + 90.0 * (t * 28.0 * TAU).sin()) * dt;
                sine(ph) * 0.45 * (1.0 - (t / 0.45).powi(3))
            })
        }
        x if x == Sfx::Splat as usize => {
            let mut lp = 0.0;
            let n = gen(sr, 0.16, |t, _| {
                lp += 0.18 * (nz.next() - lp);
                lp * 1.8 * (-t * 22.0).exp()
            });
            layer(sr, vec![(0.0, n), (0.0, tone(sr, 520.0, 150.0, 0.12, 20.0, sine, 0.5))])
        }
        x if x == Sfx::Crack as usize => gen(sr, 0.12, |t, _| {
            let s = nz.next();
            s * (-t * 45.0).exp() * if (t * 300.0) as i32 % 3 == 0 { 0.9 } else { 0.4 }
        }),
        x if x == Sfx::Swap as usize => tone(sr, 600.0, 900.0, 0.08, 20.0, sq25, 0.4),
        x if x == Sfx::Rank as usize => {
            let notes = [523.3, 659.3, 784.0, 1046.5, 1318.5, 1568.0];
            layer(sr, notes.iter().enumerate().map(|(i, &h)| (i as f32 * 0.08, tone(sr, h, h, 0.5, 4.0, sq25, 0.35))).collect())
        }
        x if x == Sfx::Clock as usize => layer(
            sr,
            vec![(0.0, tone(sr, 2093.0, 2093.0, 0.25, 9.0, sine, 0.5)), (0.08, tone(sr, 2637.0, 2637.0, 0.35, 8.0, sine, 0.45))],
        ),
        x if x == Sfx::Tick as usize => tone(sr, 1400.0, 1400.0, 0.03, 80.0, sq50, 0.35),
        x if x == Sfx::Note as usize => {
            // soft marimba: fundamental plus a quickly decaying 4th harmonic
            let hz = 523.25;
            layer(sr, vec![(0.0, tone(sr, hz, hz, 0.5, 7.0, sine, 0.6)), (0.0, tone(sr, hz * 4.0, hz * 4.0, 0.12, 30.0, sine, 0.18))])
        }
        _ => {
            // Burst: bright sparkle
            let notes = [1568.0, 2093.0, 2637.0];
            let mut parts: Vec<(f32, Vec<f32>)> = notes.iter().enumerate().map(|(i, &h)| (i as f32 * 0.03, tone(sr, h, h, 0.25, 14.0, sine, 0.35))).collect();
            parts.push((0.0, gen(sr, 0.06, |t, _| nz.next() * (-t * 80.0).exp() * 0.3)));
            layer(sr, parts)
        }
    }
}

// ------------------------------------------------------------------ music

/// One looping tune: 8 bars of eighth-note melody, chord roots and a drum feel.
struct Song {
    bpm: f32,
    melody: &'static [u8],
    chords: &'static [[u8; 3]],
    swing: f32,
}

// Original compositions written for Ribbit Revived.
const MENU_SONG: Song = Song {
    bpm: 104.0,
    melody: &[
        64, 67, 72, 67, 64, 62, 60, 62, //
        64, 0, 69, 67, 64, 0, 60, 0, //
        65, 69, 72, 69, 65, 64, 65, 67, //
        67, 0, 71, 74, 71, 67, 62, 0, //
        64, 67, 72, 74, 76, 74, 72, 67, //
        71, 0, 67, 64, 67, 0, 71, 0, //
        69, 72, 77, 76, 74, 72, 69, 65, //
        67, 71, 74, 71, 67, 0, 0, 0,
    ],
    chords: &[[48, 52, 55], [45, 48, 52], [41, 45, 48], [43, 47, 50], [48, 52, 55], [40, 43, 47], [41, 45, 48], [43, 47, 50]],
    swing: 0.0,
};

const LEVEL_SONG: Song = Song {
    bpm: 136.0,
    melody: &[
        67, 0, 71, 74, 79, 74, 71, 0, //
        76, 0, 74, 71, 67, 0, 64, 0, //
        72, 76, 79, 76, 72, 0, 67, 72, //
        74, 0, 78, 81, 78, 74, 0, 66, //
        79, 0, 79, 81, 83, 81, 79, 74, //
        76, 79, 76, 74, 71, 0, 67, 0, //
        72, 0, 76, 74, 72, 71, 69, 67, //
        66, 69, 74, 78, 81, 0, 74, 0,
    ],
    chords: &[[43, 47, 50], [40, 43, 47], [36, 40, 43], [38, 42, 45], [43, 47, 50], [40, 43, 47], [36, 40, 43], [38, 42, 45]],
    swing: 0.0,
};

struct Sequencer {
    sr: f32,
    track: Option<Track>,
    t: f32,
    step: usize,
    step_len: f32,
    lead_ph: f32,
    lead_hz: f32,
    lead_env: f32,
    bass_ph: f32,
    bass_hz: f32,
    bass_env: f32,
    arp_ph: f32,
    arp_hz: f32,
    arp_env: f32,
    kick_t: f32,
    snare_t: f32,
    hat_t: f32,
    noise: Noise,
    lp: f32,
}

impl Sequencer {
    fn new(sr: f32) -> Sequencer {
        Sequencer {
            sr,
            track: None,
            t: 0.0,
            step: 0,
            step_len: 1.0,
            lead_ph: 0.0,
            lead_hz: 0.0,
            lead_env: 0.0,
            bass_ph: 0.0,
            bass_hz: 0.0,
            bass_env: 0.0,
            arp_ph: 0.0,
            arp_hz: 0.0,
            arp_env: 0.0,
            kick_t: 9.0,
            snare_t: 9.0,
            hat_t: 9.0,
            noise: Noise(0xBEEF),
            lp: 0.0,
        }
    }

    fn song(&self) -> &'static Song {
        match self.track {
            Some(Track::Level) => &LEVEL_SONG,
            _ => &MENU_SONG,
        }
    }

    fn set_track(&mut self, t: Option<Track>) {
        self.track = t;
        self.step = 0;
        self.t = 0.0;
        self.lead_env = 0.0;
        self.bass_env = 0.0;
        self.arp_env = 0.0;
        let s = self.song();
        self.step_len = 60.0 / s.bpm / 4.0; // sixteenth notes
        self.trigger();
    }

    /// Fires the events for the current sixteenth step.
    fn trigger(&mut self) {
        let s = self.song();
        let steps_per_bar = 16;
        let total = s.chords.len() * steps_per_bar;
        let st = self.step % total;
        let bar = st / steps_per_bar;
        let in_bar = st % steps_per_bar;
        // melody on eighth notes
        if in_bar % 2 == 0 {
            let n = s.melody[bar * 8 + in_bar / 2];
            if n > 0 {
                self.lead_hz = midi_hz(n as f32);
                self.lead_env = 1.0;
            }
        }
        let chord = s.chords[bar];
        // bass on quarters, octave bounce
        if in_bar % 4 == 0 {
            let oct = if in_bar % 8 == 4 { 12.0 } else { 0.0 };
            self.bass_hz = midi_hz(chord[0] as f32 - 12.0 + oct);
            self.bass_env = 1.0;
        }
        // arpeggio on sixteenths
        let a = chord[in_bar % 3] as f32 + 12.0 + if in_bar >= 8 { 12.0 } else { 0.0 };
        self.arp_hz = midi_hz(a);
        self.arp_env = 1.0;
        // drums
        if in_bar % 8 == 0 {
            self.kick_t = 0.0;
        }
        if in_bar % 8 == 4 {
            self.snare_t = 0.0;
        }
        if in_bar % 2 == 0 {
            self.hat_t = 0.0;
        }
    }

    fn next(&mut self) -> (f32, f32) {
        let dt = 1.0 / self.sr;
        self.t += dt;
        if self.t >= self.step_len {
            self.t -= self.step_len;
            self.step += 1;
            self.trigger();
        }
        // lead: pulse with gentle vibrato
        self.lead_ph += self.lead_hz * dt * (1.0 + 0.004 * (self.step as f32 * 0.7 + self.t * 30.0).sin());
        self.lead_env *= 1.0 - dt * 5.5;
        let lead = (square(self.lead_ph, 0.3) * 0.5 + tri(self.lead_ph) * 0.5) * self.lead_env * 0.28;
        // bass: triangle
        self.bass_ph += self.bass_hz * dt;
        self.bass_env *= 1.0 - dt * 3.0;
        let bass = tri(self.bass_ph) * self.bass_env * 0.42;
        // arp: soft square
        self.arp_ph += self.arp_hz * dt;
        self.arp_env *= 1.0 - dt * 18.0;
        let arp = square(self.arp_ph, 0.5) * self.arp_env * 0.06;
        // drums
        let mut drums = 0.0;
        if self.kick_t < 0.2 {
            let k = self.kick_t;
            drums += (TAU * (55.0 + 120.0 * (-k * 30.0).exp()) * k).sin() * (-k * 18.0).exp() * 0.55;
            self.kick_t += dt;
        }
        if self.snare_t < 0.2 {
            let n = self.noise.next();
            self.lp += 0.4 * (n - self.lp);
            drums += (n - self.lp) * (-self.snare_t * 22.0).exp() * 0.22;
            self.snare_t += dt;
        }
        if self.hat_t < 0.05 {
            let n = self.noise.next();
            drums += n * (-self.hat_t * 120.0).exp() * 0.05;
            self.hat_t += dt;
        }
        let l = lead * 0.9 + bass + arp * 1.2 + drums;
        let r = lead * 1.1 + bass + arp * 0.8 + drums;
        (l, r)
    }
}
