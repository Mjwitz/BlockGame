use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

/// Playback order for scheduled Auto-Messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AutoMessageOrder {
    #[default]
    Sequential,
    Random,
}

impl AutoMessageOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::Random => "random",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sequential" | "seq" | "order" => Some(Self::Sequential),
            "random" | "rand" | "shuffle" => Some(Self::Random),
            _ => None,
        }
    }
}

/// Serializable YAML configuration for `config/automessages.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoMessageConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_interval_secs")]
    pub interval_secs: u64,
    #[serde(default)]
    pub order: AutoMessageOrder,
    #[serde(default = "default_prefix")]
    pub prefix: String,
    #[serde(default)]
    pub messages: Vec<String>,
}

fn default_interval_secs() -> u64 {
    300
}

fn default_prefix() -> String {
    "&6[Auto]&r ".to_string()
}

impl Default for AutoMessageConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_secs: default_interval_secs(),
            order: AutoMessageOrder::Sequential,
            prefix: default_prefix(),
            messages: vec![
                "&aWelcome to the server! Type &e/help &afor available commands.".to_string(),
            ],
        }
    }
}

/// Server-side scheduler and persistence manager for timed Auto-Messages.
#[derive(Debug, Clone)]
pub struct AutoMessageManager {
    pub config: AutoMessageConfig,
    next_index: usize,
    elapsed_secs: f32,
    rng_state: u64,
}

impl Default for AutoMessageManager {
    fn default() -> Self {
        Self::new(AutoMessageConfig::default())
    }
}

impl AutoMessageManager {
    pub const MIN_INTERVAL_SECS: u64 = 5;

    pub fn new(mut config: AutoMessageConfig) -> Self {
        if config.interval_secs < Self::MIN_INTERVAL_SECS {
            config.interval_secs = Self::MIN_INTERVAL_SECS;
        }
        Self {
            config,
            next_index: 0,
            elapsed_secs: 0.0,
            rng_state: 0x9E37_79B9_7F4A_7C15,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
        if !enabled {
            self.elapsed_secs = 0.0;
        }
    }

    pub fn set_interval_secs(&mut self, secs: u64) -> Result<(), String> {
        if secs < Self::MIN_INTERVAL_SECS {
            return Err(format!(
                "Interval must be at least {} seconds.",
                Self::MIN_INTERVAL_SECS
            ));
        }
        self.config.interval_secs = secs;
        self.elapsed_secs = self.elapsed_secs.min(secs as f32);
        Ok(())
    }

    pub fn set_order(&mut self, order: AutoMessageOrder) {
        self.config.order = order;
    }

    pub fn add_message(&mut self, msg: impl Into<String>) -> usize {
        self.config.messages.push(msg.into());
        self.config.messages.len()
    }

    pub fn edit_message(
        &mut self,
        one_based_index: usize,
        msg: impl Into<String>,
    ) -> Result<(), String> {
        if one_based_index == 0 || one_based_index > self.config.messages.len() {
            return Err(format!(
                "Invalid message index {}. Valid range: 1..={}.",
                one_based_index,
                self.config.messages.len()
            ));
        }
        let text = msg.into();
        if text.trim().is_empty() {
            return Err("Auto-Message text cannot be empty.".to_string());
        }
        self.config.messages[one_based_index - 1] = text;
        Ok(())
    }

    pub fn remove_message(&mut self, one_based_index: usize) -> Result<String, String> {
        if one_based_index == 0 || one_based_index > self.config.messages.len() {
            return Err(format!(
                "Invalid message index {}. Valid range: 1..={}.",
                one_based_index,
                self.config.messages.len()
            ));
        }
        let removed = self.config.messages.remove(one_based_index - 1);
        if self.config.messages.is_empty() {
            self.next_index = 0;
        } else {
            self.next_index %= self.config.messages.len();
        }
        Ok(removed)
    }

    /// Advances the timer by `dt_secs` and returns a formatted broadcast when due.
    pub fn tick(&mut self, dt_secs: f32, has_online_players: bool) -> Option<String> {
        if !self.config.enabled || self.config.messages.is_empty() || !has_online_players {
            return None;
        }
        self.elapsed_secs += dt_secs.max(0.0);
        let interval = self.config.interval_secs.max(Self::MIN_INTERVAL_SECS) as f32;
        if self.elapsed_secs < interval {
            return None;
        }
        self.elapsed_secs -= interval;

        let idx = match self.config.order {
            AutoMessageOrder::Sequential => {
                let i = self.next_index % self.config.messages.len();
                self.next_index = (i + 1) % self.config.messages.len();
                i
            }
            AutoMessageOrder::Random => {
                let mut x = self.rng_state;
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                self.rng_state = x;
                (x as usize) % self.config.messages.len()
            }
        };

        let raw = &self.config.messages[idx];
        Some(if self.config.prefix.is_empty() {
            raw.clone()
        } else {
            format!("{}{}", self.config.prefix, raw)
        })
    }

    pub fn save_to_dir(&self, config_dir: &Path) -> io::Result<()> {
        let path = config_dir.join("automessages.yml");
        let header = "# BlockGame Auto-Messages Configuration\n# Supports & color codes and clickable http(s):// URLs.";
        crate::config_io::atomic_write_yaml(&path, header, &self.config)
    }

    pub fn load_from_dir(config_dir: &Path) -> io::Result<Self> {
        let path = config_dir.join("automessages.yml");
        if !path.exists() {
            let mgr = Self::default();
            let _ = mgr.save_to_dir(config_dir);
            return Ok(mgr);
        }
        let content = fs::read_to_string(&path)?;
        let cfg: AutoMessageConfig = serde_yaml::from_str(&content).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid automessages.yml: {e}"),
            )
        })?;
        Ok(Self::new(cfg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_order_wraps_and_respects_enabled_and_online_flags() {
        let mut mgr = AutoMessageManager::new(AutoMessageConfig {
            enabled: false,
            interval_secs: 5,
            order: AutoMessageOrder::Sequential,
            prefix: "&6[Info] &r".to_string(),
            messages: vec!["First".to_string(), "Second".to_string()],
        });

        assert_eq!(mgr.tick(10.0, true), None, "Disabled must not broadcast");
        mgr.set_enabled(true);
        assert_eq!(mgr.tick(10.0, false), None, "No online players must not broadcast");
        assert_eq!(mgr.tick(4.0, true), None);
        assert_eq!(mgr.tick(1.5, true), Some("&6[Info] &rFirst".to_string()));
        assert_eq!(mgr.tick(5.0, true), Some("&6[Info] &rSecond".to_string()));
        assert_eq!(mgr.tick(5.0, true), Some("&6[Info] &rFirst".to_string()));
    }

    #[test]
    fn random_order_and_yaml_persistence_roundtrip() {
        let root = std::env::temp_dir().join(format!("blockgame_automessage_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let mut mgr = AutoMessageManager::new(AutoMessageConfig {
            enabled: true,
            interval_secs: 10,
            order: AutoMessageOrder::Random,
            prefix: "".to_string(),
            messages: vec!["Alpha".to_string(), "Beta".to_string(), "Gamma".to_string()],
        });
        let msg = mgr.tick(10.0, true).expect("Should emit random message");
        assert!(["Alpha", "Beta", "Gamma"].contains(&msg.as_str()));

        mgr.save_to_dir(&root).unwrap();
        let loaded = AutoMessageManager::load_from_dir(&root).unwrap();
        assert_eq!(loaded.config, mgr.config);

        let _ = fs::remove_dir_all(&root);
    }
}
