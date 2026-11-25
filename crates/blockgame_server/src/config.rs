use std::fs;
use std::io;
use std::path::Path;

use blockgame_core::directory::DEFAULT_DIRECTORY_URL;
use blockgame_core::net::DEFAULT_SERVER_PORT;
use blockgame_core::save::parse_or_hash_seed;

pub const DEFAULT_CONFIG_FILE: &str = "server.properties";

/// Configuration settings for the dedicated BlockGame multiplayer server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// Port to listen on.
    pub port: u16,
    /// Network interface to bind to (e.g. 0.0.0.0 or 127.0.0.1).
    pub bind_host: String,
    /// Name of the world and save folder inside `saves/`.
    pub world_name: String,
    /// Generation seed for the world.
    pub seed: u32,
    /// Whether to announce this server to the central public server directory.
    pub discoverable: bool,
    /// Display name shown in discoverable server lists (defaults to world name).
    pub server_name: String,
    /// Message of the day shown in the server list (supports & color codes).
    pub motd: String,
    /// Central directory server URL.
    pub directory_url: String,
    /// Overridden public IP or domain name for discovery (or 'auto').
    pub public_host: String,
    /// Maximum reported player slots.
    pub max_players: usize,
    /// Whether to broadcast player achievement unlocks in multiplayer chat.
    pub broadcast_achievements: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_SERVER_PORT,
            bind_host: "0.0.0.0".to_string(),
            world_name: "Multiplayer World".to_string(),
            seed: 1337,
            discoverable: false,
            server_name: "Multiplayer World".to_string(),
            motd: "&aA BlockGame Dedicated Server".to_string(),
            directory_url: DEFAULT_DIRECTORY_URL.to_string(),
            public_host: "auto".to_string(),
            max_players: 20,
            broadcast_achievements: true,
        }
    }
}

impl ServerConfig {
    /// Loads configuration from `path`. If the file does not exist, a commented
    /// default `server.properties` is generated and saved to `path`.
    pub fn load_or_create(path: &Path) -> Result<Self, io::Error> {
        if !path.exists() {
            let default_cfg = Self::default();
            default_cfg.save(path)?;
            log::info!("Created default server configuration at '{}'", path.display());
            return Ok(default_cfg);
        }

        let content = fs::read_to_string(path)?;
        let cfg = Self::parse(&content);
        log::info!("Loaded server configuration from '{}'", path.display());
        Ok(cfg)
    }

    /// Parses configuration from key-value text lines.
    pub fn parse(content: &str) -> Self {
        let mut cfg = Self::default();
        let mut server_name_set = false;

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty()
                || line.starts_with('#')
                || line.starts_with(';')
                || line.starts_with("//")
            {
                continue;
            }

            let Some((raw_key, raw_val)) = line.split_once('=') else {
                continue;
            };

            let key = raw_key.trim().to_ascii_lowercase().replace('-', "_");
            let val = raw_val.trim();

            match key.as_str() {
                "server_port" | "port" => {
                    if let Ok(p) = val.parse::<u16>() {
                        cfg.port = p;
                    }
                }
                "bind_address" | "bind_host" | "bind" | "server_ip" => {
                    if !val.is_empty() {
                        cfg.bind_host = val.to_string();
                    }
                }
                "world_name" | "world" | "level_name" => {
                    if !val.is_empty() {
                        cfg.world_name = val.to_string();
                        if !server_name_set {
                            cfg.server_name = val.to_string();
                        }
                    }
                }
                "seed" | "level_seed" => {
                    if !val.is_empty() {
                        cfg.seed = parse_or_hash_seed(val);
                    }
                }
                "discoverable" | "enable_discovery" | "public" => {
                    cfg.discoverable = parse_bool(val).unwrap_or(cfg.discoverable);
                }
                "server_name" | "name" => {
                    if !val.is_empty() {
                        cfg.server_name = val.to_string();
                        server_name_set = true;
                    }
                }
                "motd" | "server_motd" => {
                    cfg.motd = val.to_string();
                }
                "directory_url" | "central_directory_url" => {
                    if !val.is_empty() {
                        cfg.directory_url = val.to_string();
                    }
                }
                "public_host" | "public_ip" => {
                    if !val.is_empty() {
                        cfg.public_host = val.to_string();
                    }
                }
                "max_players" | "max_player_count" => {
                    if let Ok(mp) = val.parse::<usize>() {
                        cfg.max_players = mp.max(1);
                    }
                }
                "broadcast_achievements" => {
                    cfg.broadcast_achievements =
                        parse_bool(val).unwrap_or(cfg.broadcast_achievements);
                }
                _ => {}
            }
        }

        cfg
    }

    /// Serializes configuration into commented `server.properties` format.
    pub fn serialize(&self) -> String {
        format!(
            "# BlockGame Dedicated Server Configuration\n\
             # Lines starting with '#' are comments.\n\
             # Changes take effect when the server restarts.\n\
             \n\
             # --- Network Settings ---\n\
             # The port and bind address the server listens on.\n\
             server-port={}\n\
             bind-address={}\n\
             \n\
             # --- World Settings ---\n\
             # The world save name (stored under saves/<world-name>) and generation seed.\n\
             world-name={}\n\
             seed={}\n\
             \n\
             # --- Discovery & Public Server Directory ---\n\
             # Set 'discoverable=true' to show this server in the in-game discoverable server browser.\n\
             discoverable={}\n\
             server-name={}\n\
             motd={}\n\
             directory-url={}\n\
             public-host={}\n\
             max-players={}\n\
             \n\
             # --- Gameplay Settings ---\n\
             # Broadcast player Achievement unlocks in multiplayer chat (default: true).\n\
             broadcast-achievements={}\n",
            self.port,
            self.bind_host,
            self.world_name,
            self.seed,
            self.discoverable,
            self.server_name,
            self.motd,
            self.directory_url,
            self.public_host,
            self.max_players,
            self.broadcast_achievements
        )
    }

    /// Saves the configuration to a file on disk.
    pub fn save(&self, path: &Path) -> Result<(), io::Error> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(path, self.serialize())
    }
}

fn parse_bool(val: &str) -> Option<bool> {
    match val.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_defaults_and_comments() {
        let text = r#"
            # Comment line
            ; Another comment
            // Third comment style

            server-port = 25570
            bind-address = 127.0.0.1
            world-name = SurvivalWorld
            seed = 42
            discoverable = true
            motd = &bWelcome!
            max-players = 50
            broadcast-achievements = false
        "#;
        let cfg = ServerConfig::parse(text);
        assert_eq!(cfg.port, 25570);
        assert_eq!(cfg.bind_host, "127.0.0.1");
        assert_eq!(cfg.world_name, "SurvivalWorld");
        assert_eq!(cfg.server_name, "SurvivalWorld");
        assert_eq!(cfg.seed, 42);
        assert!(cfg.discoverable);
        assert_eq!(cfg.motd, "&bWelcome!");
        assert_eq!(cfg.max_players, 50);
        assert!(!cfg.broadcast_achievements);
    }

    #[test]
    fn test_parse_aliases_and_case() {
        let text = r#"
            port=25575
            server_ip=192.168.1.100
            level_name=Adventure
            name=Epic Adventure Server
            enable_discovery=yes
            max_player_count=12
        "#;
        let cfg = ServerConfig::parse(text);
        assert_eq!(cfg.port, 25575);
        assert_eq!(cfg.bind_host, "192.168.1.100");
        assert_eq!(cfg.world_name, "Adventure");
        assert_eq!(cfg.server_name, "Epic Adventure Server");
        assert!(cfg.discoverable);
        assert_eq!(cfg.max_players, 12);
        assert!(cfg.broadcast_achievements);
    }

    #[test]
    fn test_round_trip_serialization() {
        let mut cfg = ServerConfig::default();
        cfg.port = 25580;
        cfg.world_name = "CustomWorld".to_string();
        cfg.discoverable = true;
        cfg.motd = "Test MOTD".to_string();
        cfg.broadcast_achievements = false;

        let serialized = cfg.serialize();
        let parsed = ServerConfig::parse(&serialized);
        assert_eq!(cfg, parsed);
    }
}
