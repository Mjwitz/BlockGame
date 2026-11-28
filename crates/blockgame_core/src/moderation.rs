use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

use crate::config_io::now_unix_secs;

/// Parses a compact duration string such as `30s`, `10m`, `2h`, `7d`, or `1h30m` into seconds.
pub fn parse_duration_secs(input: &str) -> Option<u64> {
    let s = input.trim().to_ascii_lowercase();
    if s.is_empty() {
        return None;
    }
    let mut total: u64 = 0;
    let mut num_buf = String::new();
    let mut matched_any = false;

    for ch in s.chars() {
        if ch.is_ascii_digit() {
            num_buf.push(ch);
        } else {
            let val: u64 = num_buf.parse().ok()?;
            num_buf.clear();
            let mult: u64 = match ch {
                's' => 1,
                'm' => 60,
                'h' => 3600,
                'd' => 86_400,
                'w' => 604_800,
                _ => return None,
            };
            total = total.checked_add(val.checked_mul(mult)?)?;
            matched_any = true;
        }
    }

    if !num_buf.is_empty() || !matched_any || total == 0 {
        return None;
    }
    Some(total)
}

/// Formats a duration in seconds into a human-readable string (e.g., `1h 30m`, `45s`).
pub fn format_duration_secs(secs: u64) -> String {
    if secs == 0 {
        return "0s".to_string();
    }
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3600;
    let mins = (secs % 3600) / 60;
    let rem = secs % 60;
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{}d", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if mins > 0 {
        parts.push(format!("{}m", mins));
    }
    if rem > 0 || parts.is_empty() {
        parts.push(format!("{}s", rem));
    }
    parts.join(" ")
}

/// Persistent mute record stored in `config/mutes.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MuteEntry {
    pub username: String,
    pub muted_by: String,
    pub reason: String,
    pub created_at: u64,
    /// `None` = permanent mute; `Some(ts)` = Unix timestamp when mute expires.
    #[serde(default)]
    pub expires_at: Option<u64>,
}

impl MuteEntry {
    pub fn remaining_secs(&self, now: u64) -> Option<u64> {
        self.expires_at.map(|exp| exp.saturating_sub(now))
    }

    pub fn is_expired(&self, now: u64) -> bool {
        self.expires_at.is_some_and(|exp| now >= exp)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MutesYaml {
    #[serde(default)]
    mutes: BTreeMap<String, MuteEntry>,
}

/// Persistent Warning record stored in `config/warnings.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarningEntry {
    pub id: u64,
    pub username: String,
    pub warned_by: String,
    pub reason: String,
    pub created_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct WarningsYaml {
    #[serde(default)]
    next_id: u64,
    #[serde(default)]
    warnings: BTreeMap<String, Vec<WarningEntry>>,
}

/// Action taken when the Word Filter matches a chat message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WordFilterMode {
    #[default]
    Censor,
    Block,
}

impl WordFilterMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Censor => "censor",
            Self::Block => "block",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "censor" | "mask" | "replace" => Some(Self::Censor),
            "block" | "reject" | "deny" => Some(Self::Block),
            _ => None,
        }
    }
}

/// Persistent Word Filter configuration stored in `config/word_filter.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WordFilterConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: WordFilterMode,
    #[serde(default)]
    pub words: Vec<String>,
}

/// Persistent Global Chat Lock and Slowmode configuration stored in `config/chat_settings.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ChatSettingsConfig {
    #[serde(default)]
    pub chat_locked: bool,
    #[serde(default)]
    pub slowmode_secs: u64,
}

/// Result of evaluating an incoming chat message through the moderation pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatModerationDecision {
    Allow(String),
    Reject(String),
}

/// Manages Mutes, Warnings, Word Filter, Global Chat Lock, and Slowmode,
/// persisting each concern to its own file under `config/` (ADR-0012).
#[derive(Debug, Clone)]
pub struct ModerationManager {
    pub mutes: BTreeMap<String, MuteEntry>,
    pub warnings: BTreeMap<String, Vec<WarningEntry>>,
    next_warning_id: u64,
    pub word_filter: WordFilterConfig,
    pub chat_settings: ChatSettingsConfig,
    last_chat_time: HashMap<String, Instant>,
}

impl Default for ModerationManager {
    fn default() -> Self {
        Self {
            mutes: BTreeMap::new(),
            warnings: BTreeMap::new(),
            next_warning_id: 1,
            word_filter: WordFilterConfig::default(),
            chat_settings: ChatSettingsConfig::default(),
            last_chat_time: HashMap::new(),
        }
    }
}

impl ModerationManager {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Mutes ---

    pub fn mute(
        &mut self,
        username: &str,
        muted_by: &str,
        duration_secs: Option<u64>,
        reason: &str,
    ) -> MuteEntry {
        let now = now_unix_secs();
        let entry = MuteEntry {
            username: username.trim().to_string(),
            muted_by: muted_by.trim().to_string(),
            reason: if reason.trim().is_empty() {
                "Muted by staff".to_string()
            } else {
                reason.trim().to_string()
            },
            created_at: now,
            expires_at: duration_secs.map(|d| now.saturating_add(d)),
        };
        self.mutes
            .insert(username.trim().to_ascii_lowercase(), entry.clone());
        entry
    }

    pub fn unmute(&mut self, username: &str) -> bool {
        self.mutes
            .remove(&username.trim().to_ascii_lowercase())
            .is_some()
    }

    /// Returns the active mute for `username`, pruning it if expired.
    /// Second tuple element is `true` if an expired mute was pruned (caller should save `mutes.yml`).
    pub fn check_mute(&mut self, username: &str) -> (Option<MuteEntry>, bool) {
        let key = username.trim().to_ascii_lowercase();
        let now = now_unix_secs();
        if let Some(entry) = self.mutes.get(&key) {
            if entry.is_expired(now) {
                self.mutes.remove(&key);
                return (None, true);
            }
            return (Some(entry.clone()), false);
        }
        (None, false)
    }

    pub fn active_mutes(&mut self) -> (Vec<MuteEntry>, bool) {
        let now = now_unix_secs();
        let before = self.mutes.len();
        self.mutes.retain(|_, m| !m.is_expired(now));
        let pruned = self.mutes.len() != before;
        (self.mutes.values().cloned().collect(), pruned)
    }

    // --- Warnings ---

    pub fn warn(&mut self, username: &str, warned_by: &str, reason: &str) -> WarningEntry {
        let id = self.next_warning_id.max(1);
        self.next_warning_id = id + 1;
        let entry = WarningEntry {
            id,
            username: username.trim().to_string(),
            warned_by: warned_by.trim().to_string(),
            reason: reason.trim().to_string(),
            created_at: now_unix_secs(),
        };
        self.warnings
            .entry(username.trim().to_ascii_lowercase())
            .or_default()
            .push(entry.clone());
        entry
    }

    pub fn get_warnings(&self, username: &str) -> &[WarningEntry] {
        self.warnings
            .get(&username.trim().to_ascii_lowercase())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn clear_warnings(&mut self, username: &str) -> usize {
        self.warnings
            .remove(&username.trim().to_ascii_lowercase())
            .map(|v| v.len())
            .unwrap_or(0)
    }

    // --- Word Filter ---

    pub fn add_filter_word(&mut self, word: &str) -> bool {
        let clean = word.trim().to_ascii_lowercase();
        if clean.is_empty() || self.word_filter.words.iter().any(|w| w.eq_ignore_ascii_case(&clean)) {
            return false;
        }
        self.word_filter.words.push(clean);
        true
    }

    pub fn remove_filter_word(&mut self, word: &str) -> bool {
        let clean = word.trim().to_ascii_lowercase();
        let before = self.word_filter.words.len();
        self.word_filter
            .words
            .retain(|w| !w.eq_ignore_ascii_case(&clean));
        self.word_filter.words.len() != before
    }

    /// Applies whole-word, case-insensitive Word Filter matching.
    /// Returns `Ok(output_message)` (possibly censored with `***`) or `Err(matched_word)` if blocked.
    pub fn apply_word_filter(&self, message: &str) -> Result<String, String> {
        if !self.word_filter.enabled || self.word_filter.words.is_empty() {
            return Ok(message.to_string());
        }

        let mut out = String::with_capacity(message.len());
        let mut cursor = 0;

        while cursor < message.len() {
            let ch = message[cursor..].chars().next().unwrap();
            if ch.is_alphanumeric() {
                let start = cursor;
                cursor += ch.len_utf8();
                while cursor < message.len() {
                    let next = message[cursor..].chars().next().unwrap();
                    if next.is_alphanumeric() {
                        cursor += next.len_utf8();
                    } else {
                        break;
                    }
                }
                let token = &message[start..cursor];
                let matched = self
                    .word_filter
                    .words
                    .iter()
                    .find(|w| w.eq_ignore_ascii_case(token));
                if let Some(word) = matched {
                    match self.word_filter.mode {
                        WordFilterMode::Block => return Err(word.clone()),
                        WordFilterMode::Censor => out.push_str("***"),
                    }
                } else {
                    out.push_str(token);
                }
            } else {
                out.push(ch);
                cursor += ch.len_utf8();
            }
        }

        Ok(out)
    }

    // --- Chat Pipeline ---

    /// Evaluates an incoming chat message through Global Chat Lock, Mutes, Slowmode, and Word Filter.
    /// Returns `(decision, mutes_pruned)`.
    pub fn evaluate_chat_message(
        &mut self,
        username: &str,
        message: &str,
        bypass_lock: bool,
        bypass_slowmode: bool,
        bypass_filter: bool,
    ) -> (ChatModerationDecision, bool) {
        // 1. Global Chat Lock
        if self.chat_settings.chat_locked && !bypass_lock {
            return (
                ChatModerationDecision::Reject(
                    "Chat is currently locked by server staff.".to_string(),
                ),
                false,
            );
        }

        // 2. Active Mute
        let (active_mute, pruned) = self.check_mute(username);
        if let Some(m) = active_mute {
            let now = now_unix_secs();
            let detail = match m.remaining_secs(now) {
                Some(rem) => format!(
                    "You are muted for {} (Reason: {}).",
                    format_duration_secs(rem),
                    m.reason
                ),
                None => format!("You are permanently muted (Reason: {}).", m.reason),
            };
            return (ChatModerationDecision::Reject(detail), pruned);
        }

        // 3. Slowmode
        if self.chat_settings.slowmode_secs > 0 && !bypass_slowmode {
            let key = username.trim().to_ascii_lowercase();
            let now_inst = Instant::now();
            if let Some(prev) = self.last_chat_time.get(&key) {
                let elapsed = now_inst.duration_since(*prev).as_secs_f32();
                let limit = self.chat_settings.slowmode_secs as f32;
                if elapsed < limit {
                    let wait = (limit - elapsed).ceil() as u64;
                    return (
                        ChatModerationDecision::Reject(format!(
                            "Slowmode is active. Please wait {}s before chatting again.",
                            wait.max(1)
                        )),
                        pruned,
                    );
                }
            }
            self.last_chat_time.insert(key, now_inst);
        }

        // 4. Word Filter
        if !bypass_filter {
            match self.apply_word_filter(message) {
                Ok(filtered) => return (ChatModerationDecision::Allow(filtered), pruned),
                Err(_) => {
                    return (
                        ChatModerationDecision::Reject(
                            "Your message was blocked by the server Word Filter.".to_string(),
                        ),
                        pruned,
                    );
                }
            }
        }

        (ChatModerationDecision::Allow(message.to_string()), pruned)
    }

    // --- Persistence (4 separate files in config/) ---

    pub fn save_mutes(&self, config_dir: &Path) -> io::Result<()> {
        let data = MutesYaml {
            mutes: self.mutes.clone(),
        };
        crate::config_io::atomic_write_yaml(
            &config_dir.join("mutes.yml"),
            "# BlockGame Active Mutes",
            &data,
        )
    }

    pub fn save_warnings(&self, config_dir: &Path) -> io::Result<()> {
        let data = WarningsYaml {
            next_id: self.next_warning_id,
            warnings: self.warnings.clone(),
        };
        crate::config_io::atomic_write_yaml(
            &config_dir.join("warnings.yml"),
            "# BlockGame Player Warning History",
            &data,
        )
    }

    pub fn save_word_filter(&self, config_dir: &Path) -> io::Result<()> {
        crate::config_io::atomic_write_yaml(
            &config_dir.join("word_filter.yml"),
            "# BlockGame Chat Word Filter (mode: censor | block)",
            &self.word_filter,
        )
    }

    pub fn save_chat_settings(&self, config_dir: &Path) -> io::Result<()> {
        crate::config_io::atomic_write_yaml(
            &config_dir.join("chat_settings.yml"),
            "# BlockGame Chat Moderation Settings (Global Chat Lock & Slowmode)",
            &self.chat_settings,
        )
    }

    pub fn load_from_dir(config_dir: &Path) -> Self {
        let mut mgr = Self::new();

        let mutes_path = config_dir.join("mutes.yml");
        if mutes_path.exists() {
            if let Ok(text) = fs::read_to_string(&mutes_path) {
                if let Ok(parsed) = serde_yaml::from_str::<MutesYaml>(&text) {
                    mgr.mutes = parsed.mutes;
                }
            }
        } else {
            let _ = mgr.save_mutes(config_dir);
        }

        let warnings_path = config_dir.join("warnings.yml");
        if warnings_path.exists() {
            if let Ok(text) = fs::read_to_string(&warnings_path) {
                if let Ok(parsed) = serde_yaml::from_str::<WarningsYaml>(&text) {
                    mgr.warnings = parsed.warnings;
                    mgr.next_warning_id = parsed.next_id.max(1);
                }
            }
        } else {
            let _ = mgr.save_warnings(config_dir);
        }

        let filter_path = config_dir.join("word_filter.yml");
        if filter_path.exists() {
            if let Ok(text) = fs::read_to_string(&filter_path) {
                if let Ok(parsed) = serde_yaml::from_str::<WordFilterConfig>(&text) {
                    mgr.word_filter = parsed;
                }
            }
        } else {
            let _ = mgr.save_word_filter(config_dir);
        }

        let chat_path = config_dir.join("chat_settings.yml");
        if chat_path.exists() {
            if let Ok(text) = fs::read_to_string(&chat_path) {
                if let Ok(parsed) = serde_yaml::from_str::<ChatSettingsConfig>(&text) {
                    mgr.chat_settings = parsed;
                }
            }
        } else {
            let _ = mgr.save_chat_settings(config_dir);
        }

        mgr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_parsing_and_formatting() {
        assert_eq!(parse_duration_secs("30s"), Some(30));
        assert_eq!(parse_duration_secs("10m"), Some(600));
        assert_eq!(parse_duration_secs("1h30m"), Some(5400));
        assert_eq!(parse_duration_secs("2d"), Some(172_800));
        assert_eq!(parse_duration_secs("bad"), None);
        assert_eq!(parse_duration_secs("0m"), None);
        assert_eq!(format_duration_secs(5400), "1h 30m");
    }

    #[test]
    fn word_filter_whole_word_censor_and_block() {
        let mut mgr = ModerationManager::new();
        mgr.word_filter.enabled = true;
        mgr.word_filter.mode = WordFilterMode::Censor;
        mgr.add_filter_word("bad");

        // Whole-word match censors "BAD" but does not touch "badge"
        assert_eq!(
            mgr.apply_word_filter("That badge is BAD!").unwrap(),
            "That badge is ***!"
        );

        mgr.word_filter.mode = WordFilterMode::Block;
        assert!(mgr.apply_word_filter("This is bad.").is_err());
        assert_eq!(mgr.apply_word_filter("Nice badge.").unwrap(), "Nice badge.");
    }

    #[test]
    fn moderation_pipeline_and_four_file_persistence() {
        let root = std::env::temp_dir().join(format!("blockgame_mod_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let mut mgr = ModerationManager::load_from_dir(&root);
        assert!(root.join("mutes.yml").exists());
        assert!(root.join("warnings.yml").exists());
        assert!(root.join("word_filter.yml").exists());
        assert!(root.join("chat_settings.yml").exists());

        // Global chat lock
        mgr.chat_settings.chat_locked = true;
        let (dec, _) = mgr.evaluate_chat_message("Bob", "hi", false, false, false);
        assert!(matches!(dec, ChatModerationDecision::Reject(_)));
        let (dec_staff, _) = mgr.evaluate_chat_message("Admin", "hi", true, false, false);
        assert_eq!(dec_staff, ChatModerationDecision::Allow("hi".to_string()));
        mgr.chat_settings.chat_locked = false;

        // Mute & Warning
        mgr.mute("Bob", "Admin", Some(600), "Spamming");
        let (dec_muted, _) = mgr.evaluate_chat_message("Bob", "hi", false, false, false);
        assert!(matches!(dec_muted, ChatModerationDecision::Reject(s) if s.contains("muted")));

        mgr.warn("Bob", "Admin", "First warning");
        assert_eq!(mgr.get_warnings("Bob").len(), 1);

        mgr.word_filter.enabled = true;
        mgr.add_filter_word("grief");
        mgr.chat_settings.slowmode_secs = 5;

        mgr.save_mutes(&root).unwrap();
        mgr.save_warnings(&root).unwrap();
        mgr.save_word_filter(&root).unwrap();
        mgr.save_chat_settings(&root).unwrap();

        let reloaded = ModerationManager::load_from_dir(&root);
        assert!(reloaded.mutes.contains_key("bob"));
        assert_eq!(reloaded.get_warnings("Bob").len(), 1);
        assert!(reloaded.word_filter.enabled);
        assert_eq!(reloaded.word_filter.words, vec!["grief".to_string()]);
        assert_eq!(reloaded.chat_settings.slowmode_secs, 5);

        let _ = fs::remove_dir_all(&root);
    }
}
