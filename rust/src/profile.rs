//! Persistent player profile stored as a small key=value text file in the app's private dir.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Default, Debug)]
pub struct LevelRecord {
    pub best: i64,
    pub stars: u8,
    pub plays: u32,
    /// Lifetime units scored (frogs popped, metres jumped, ...), drives "total" achievements.
    pub total: i64,
    /// Best result for this level's challenge stat (e.g. pops in first 5 s).
    pub feat: i64,
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub path: PathBuf,
    pub music: bool,
    pub sfx: bool,
    pub haptics: bool,
    pub clocks: i32,
    pub xp: i64,
    pub levels: HashMap<String, LevelRecord>,
    pub achievements: HashMap<String, bool>,
    pub tips_seen: HashMap<String, bool>,
    pub last_day: i64,
    pub streak: i32,
    /// Colour-blind helper: draw a shape on every coloured frog and ball.
    pub color_marks: bool,
    /// Day number (UTC days since 1970) of the last completed daily challenge.
    pub daily_done: i64,
    pub daily_total: i32,
    /// Look for new versions on GitHub (Options > Updates).
    pub update_check: bool,
    pub dirty: bool,
}

impl Profile {
    pub fn new(dir: &str) -> Profile {
        Profile {
            path: PathBuf::from(dir).join("profile.txt"),
            music: true,
            sfx: true,
            haptics: true,
            clocks: 3,
            xp: 0,
            levels: HashMap::new(),
            achievements: HashMap::new(),
            tips_seen: HashMap::new(),
            last_day: 0,
            streak: 0,
            color_marks: false,
            daily_done: 0,
            daily_total: 0,
            update_check: true,
            dirty: false,
        }
    }

    pub fn load(dir: &str) -> Profile {
        let mut p = Profile::new(dir);
        let Ok(text) = std::fs::read_to_string(&p.path) else { return p };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let b = v == "1";
            let n = v.parse::<i64>().unwrap_or(0);
            match k {
                "music" => p.music = b,
                "sfx" => p.sfx = b,
                "haptics" => p.haptics = b,
                "clocks" => p.clocks = n as i32,
                "xp" => p.xp = n,
                "last_day" => p.last_day = n,
                "streak" => p.streak = n as i32,
                "color_marks" => p.color_marks = b,
                "daily_done" => p.daily_done = n,
                "daily_total" => p.daily_total = n as i32,
                "update_check" => p.update_check = b,
                _ => {
                    if let Some(rest) = k.strip_prefix("lvl.") {
                        if let Some((id, field)) = rest.rsplit_once('.') {
                            let r = p.levels.entry(id.to_string()).or_default();
                            match field {
                                "best" => r.best = n,
                                "stars" => r.stars = n.clamp(0, 3) as u8,
                                "plays" => r.plays = n as u32,
                                "total" => r.total = n,
                                "feat" => r.feat = n,
                                _ => {}
                            }
                        }
                    } else if let Some(id) = k.strip_prefix("ach.") {
                        p.achievements.insert(id.to_string(), b);
                    } else if let Some(id) = k.strip_prefix("tip.") {
                        p.tips_seen.insert(id.to_string(), b);
                    }
                }
            }
        }
        p
    }

    pub fn save(&mut self) {
        let mut s = String::with_capacity(4096);
        let bit = |b: bool| if b { "1" } else { "0" };
        s += &format!("version=1\nmusic={}\nsfx={}\nhaptics={}\n", bit(self.music), bit(self.sfx), bit(self.haptics));
        s += &format!("clocks={}\nxp={}\nlast_day={}\nstreak={}\n", self.clocks, self.xp, self.last_day, self.streak);
        s += &format!("color_marks={}\ndaily_done={}\ndaily_total={}\n", bit(self.color_marks), self.daily_done, self.daily_total);
        s += &format!("update_check={}\n", bit(self.update_check));
        let mut ids: Vec<_> = self.levels.keys().cloned().collect();
        ids.sort();
        for id in ids {
            let r = &self.levels[&id];
            s += &format!(
                "lvl.{id}.best={}\nlvl.{id}.stars={}\nlvl.{id}.plays={}\nlvl.{id}.total={}\nlvl.{id}.feat={}\n",
                r.best, r.stars, r.plays, r.total, r.feat
            );
        }
        for (k, v) in &self.achievements {
            if *v {
                s += &format!("ach.{k}=1\n");
            }
        }
        for (k, v) in &self.tips_seen {
            if *v {
                s += &format!("tip.{k}=1\n");
            }
        }
        // write-then-rename so a crash never leaves a truncated profile
        let tmp = self.path.with_extension("tmp");
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
        self.dirty = false;
    }

    pub fn level(&self, id: &str) -> LevelRecord {
        self.levels.get(id).cloned().unwrap_or_default()
    }
    pub fn level_mut(&mut self, id: &str) -> &mut LevelRecord {
        self.dirty = true;
        self.levels.entry(id.to_string()).or_default()
    }
    pub fn has_ach(&self, id: &str) -> bool {
        self.achievements.get(id).copied().unwrap_or(false)
    }
    pub fn total_stars(&self) -> i32 {
        self.levels.values().map(|r| r.stars as i32).sum()
    }
    pub fn reset(&mut self) {
        let path = self.path.clone();
        let (music, sfx, haptics, marks, upd) = (self.music, self.sfx, self.haptics, self.color_marks, self.update_check);
        *self = Profile { path, music, sfx, haptics, color_marks: marks, update_check: upd, ..Profile::new("") };
        self.dirty = true;
    }
}
