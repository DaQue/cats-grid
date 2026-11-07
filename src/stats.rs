//! Persistent stats: clean (no hint, no extra life) vs assisted wins.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Stats {
    /// Wins with no hint and no extra life used.
    pub clean_wins: u32,
    /// Wins that used a hint or bought an extra life.
    pub assisted_wins: u32,
    pub games_started: u32,
    pub best_clean_ms: Option<u64>,
}

fn path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".local/share/cats-grid/stats.json")
}

impl Stats {
    pub fn load() -> Stats {
        match fs::read_to_string(path()) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Stats::default(),
        }
    }

    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = fs::write(p, s);
        }
    }

    pub fn record_win(&mut self, clean: bool, ms: u64) {
        if clean {
            self.clean_wins += 1;
            self.best_clean_ms = Some(match self.best_clean_ms {
                Some(b) if b <= ms => b,
                _ => ms,
            });
        } else {
            self.assisted_wins += 1;
        }
        self.save();
    }
}
