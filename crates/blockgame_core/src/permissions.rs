use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Checks if a granted permission pattern matches a requested permission node.
/// Supports exact match, wildcard `*`, hierarchical wildcard `prefix.*`, and negative `-perm`.
pub fn permission_matches(granted: &str, requested: &str) -> bool {
    let granted = granted.trim().to_ascii_lowercase();
    let requested = requested.trim().to_ascii_lowercase();

    if granted.is_empty() || requested.is_empty() {
        return false;
    }

    if granted == "*" {
        return true;
    }

    if granted == requested {
        return true;
    }

    if let Some(prefix) = granted.strip_suffix(".*") {
        let prefix_dot = format!("{}.", prefix);
        if requested.starts_with(&prefix_dot) {
            return true;
        }
    }

    false
}

/// A permission group containing assigned permission nodes, inherited groups, and chat prefix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionGroup {
    pub name: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub inherits: Vec<String>,
}

impl PermissionGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            prefix: String::new(),
            priority: 0,
            permissions: Vec::new(),
            inherits: Vec::new(),
        }
    }

    pub fn with_permissions(name: impl Into<String>, permissions: Vec<String>) -> Self {
        Self {
            name: name.into(),
            prefix: String::new(),
            priority: 0,
            permissions,
            inherits: Vec::new(),
        }
    }

    pub fn with_style(
        name: impl Into<String>,
        prefix: impl Into<String>,
        priority: i32,
        permissions: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            prefix: prefix.into(),
            priority,
            permissions,
            inherits: Vec::new(),
        }
    }
}

/// A player's permission record containing their assigned groups and direct permissions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerPermissions {
    pub username: String,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

impl PlayerPermissions {
    pub fn new(username: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            groups: vec!["default".to_string()],
            permissions: Vec::new(),
        }
    }
}

/// Serializable YAML schema for `permissions.yml`.
#[derive(Debug, Serialize, Deserialize)]
struct PermissionsYamlConfig {
    #[serde(default)]
    groups: BTreeMap<String, PermissionGroup>,
    #[serde(default)]
    players: BTreeMap<String, PlayerPermissions>,
}

/// Full hierarchical permissions manager with group inheritance and YAML persistence.
#[derive(Debug, Clone)]
pub struct PermissionsManager {
    pub groups: HashMap<String, PermissionGroup>,
    pub players: HashMap<String, PlayerPermissions>,
}

impl Default for PermissionsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionsManager {
    /// Creates a new permissions manager initialized with default groups: `admin`, `moderator`, and `default`.
    pub fn new() -> Self {
        let mut mgr = Self {
            groups: HashMap::new(),
            players: HashMap::new(),
        };

        // Admin group has root permissions
        mgr.groups.insert(
            "admin".to_string(),
            PermissionGroup::with_style("admin", "&c[Admin] ", 100, vec!["*".to_string()]),
        );

        // Moderator group has kick, tp, and tphere
        mgr.groups.insert(
            "moderator".to_string(),
            PermissionGroup::with_style(
                "moderator",
                "&9[Mod] ",
                50,
                vec![
                    "blockgame.command.kick".to_string(),
                    "blockgame.command.tp".to_string(),
                    "blockgame.command.tphere".to_string(),
                ],
            ),
        );

        // Default group for all regular players
        mgr.groups.insert(
            "default".to_string(),
            PermissionGroup::new("default"),
        );

        mgr
    }

    /// Evaluates whether a player possesses the specified permission node.
    /// Evaluation precedence:
    /// 1. Player explicit negative permissions (`-node`) -> DENY
    /// 2. Player explicit positive permissions (`node` or wildcard) -> ALLOW
    /// 3. Inherited group negative permissions -> DENY
    /// 4. Inherited group positive permissions -> ALLOW
    /// 5. Fallback -> DENY
    pub fn has_permission(&self, username: &str, required: &str) -> bool {
        let canonical_user = username.trim().to_ascii_lowercase();

        // 1 & 2: Check direct player permissions
        if let Some(player) = self.players.get(&canonical_user) {
            for perm in &player.permissions {
                if let Some(negated) = perm.strip_prefix('-') {
                    if permission_matches(negated, required) {
                        return false;
                    }
                } else if permission_matches(perm, required) {
                    return true;
                }
            }
        }

        // 3 & 4: Check assigned & inherited groups
        let player_groups = self.get_effective_groups(&canonical_user);
        for group_name in player_groups {
            if let Some(group) = self.groups.get(&group_name) {
                // Check negative group perms first
                for perm in &group.permissions {
                    if let Some(negated) = perm.strip_prefix('-') {
                        if permission_matches(negated, required) {
                            return false;
                        }
                    }
                }
                // Check positive group perms
                for perm in &group.permissions {
                    if !perm.starts_with('-') && permission_matches(perm, required) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Returns all groups a player belongs to, including recursively inherited groups.
    pub fn get_effective_groups(&self, canonical_user: &str) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut result = Vec::new();

        let initial_groups: Vec<String> = if let Some(player) = self.players.get(canonical_user) {
            if player.groups.is_empty() {
                vec!["default".to_string()]
            } else {
                player.groups.clone()
            }
        } else {
            vec!["default".to_string()]
        };

        for group in initial_groups {
            self.resolve_group_inheritance(&group.to_ascii_lowercase(), &mut visited, &mut result);
        }

        result
    }

    fn resolve_group_inheritance(
        &self,
        group_name: &str,
        visited: &mut HashSet<String>,
        result: &mut Vec<String>,
    ) {
        if visited.insert(group_name.to_string()) {
            result.push(group_name.to_string());
            if let Some(group) = self.groups.get(group_name) {
                for parent in &group.inherits {
                    self.resolve_group_inheritance(
                        &parent.to_ascii_lowercase(),
                        visited,
                        result,
                    );
                }
            }
        }
    }

    // --- Group Management API ---

    pub fn create_group(&mut self, group_name: &str) -> Result<(), String> {
        let name = group_name.trim();
        if name.is_empty() {
            return Err("Group name cannot be empty.".to_string());
        }
        let key = name.to_ascii_lowercase();
        if self.groups.contains_key(&key) {
            return Err(format!("Group '{}' already exists.", name));
        }
        self.groups.insert(key, PermissionGroup::new(name));
        Ok(())
    }

    pub fn delete_group(&mut self, group_name: &str) -> Result<(), String> {
        let key = group_name.trim().to_ascii_lowercase();
        if key == "default" {
            return Err("Cannot delete the built-in 'default' group.".to_string());
        }
        if self.groups.remove(&key).is_some() {
            // Remove from any players
            for player in self.players.values_mut() {
                player.groups.retain(|g| !g.eq_ignore_ascii_case(&key));
            }
            Ok(())
        } else {
            Err(format!("Group '{}' not found.", group_name))
        }
    }

    pub fn add_group_permission(&mut self, group_name: &str, permission: &str) -> Result<(), String> {
        let key = group_name.trim().to_ascii_lowercase();
        let perm = permission.trim().to_string();
        if perm.is_empty() {
            return Err("Permission cannot be empty.".to_string());
        }
        if let Some(group) = self.groups.get_mut(&key) {
            if !group.permissions.iter().any(|p| p.eq_ignore_ascii_case(&perm)) {
                group.permissions.push(perm);
            }
            Ok(())
        } else {
            Err(format!("Group '{}' does not exist.", group_name))
        }
    }

    pub fn remove_group_permission(
        &mut self,
        group_name: &str,
        permission: &str,
    ) -> Result<(), String> {
        let key = group_name.trim().to_ascii_lowercase();
        let perm = permission.trim();
        if let Some(group) = self.groups.get_mut(&key) {
            let before = group.permissions.len();
            group.permissions.retain(|p| !p.eq_ignore_ascii_case(perm));
            if group.permissions.len() < before {
                Ok(())
            } else {
                Err(format!(
                    "Group '{}' did not have permission '{}'.",
                    group_name, permission
                ))
            }
        } else {
            Err(format!("Group '{}' does not exist.", group_name))
        }
    }

    // --- Player Management API ---

    pub fn add_player_group(&mut self, username: &str, group_name: &str) -> Result<(), String> {
        let key = group_name.trim().to_ascii_lowercase();
        if !self.groups.contains_key(&key) {
            return Err(format!("Group '{}' does not exist.", group_name));
        }

        let canonical_user = username.trim().to_ascii_lowercase();
        let player = self
            .players
            .entry(canonical_user)
            .or_insert_with(|| PlayerPermissions::new(username.trim()));

        if !player.groups.iter().any(|g| g.eq_ignore_ascii_case(&key)) {
            player.groups.push(key);
        }
        Ok(())
    }

    pub fn remove_player_group(&mut self, username: &str, group_name: &str) -> Result<(), String> {
        let canonical_user = username.trim().to_ascii_lowercase();
        let key = group_name.trim().to_ascii_lowercase();

        if let Some(player) = self.players.get_mut(&canonical_user) {
            let before = player.groups.len();
            player.groups.retain(|g| !g.eq_ignore_ascii_case(&key));
            if player.groups.len() < before {
                Ok(())
            } else {
                Err(format!(
                    "Player '{}' was not in group '{}'.",
                    username, group_name
                ))
            }
        } else {
            Err(format!("Player '{}' not found in permissions.", username))
        }
    }

    pub fn add_player_permission(&mut self, username: &str, permission: &str) -> Result<(), String> {
        let perm = permission.trim().to_string();
        if perm.is_empty() {
            return Err("Permission cannot be empty.".to_string());
        }
        let canonical_user = username.trim().to_ascii_lowercase();
        let player = self
            .players
            .entry(canonical_user)
            .or_insert_with(|| PlayerPermissions::new(username.trim()));

        if !player.permissions.iter().any(|p| p.eq_ignore_ascii_case(&perm)) {
            player.permissions.push(perm);
        }
        Ok(())
    }

    pub fn remove_player_permission(
        &mut self,
        username: &str,
        permission: &str,
    ) -> Result<(), String> {
        let canonical_user = username.trim().to_ascii_lowercase();
        let perm = permission.trim();

        if let Some(player) = self.players.get_mut(&canonical_user) {
            let before = player.permissions.len();
            player.permissions.retain(|p| !p.eq_ignore_ascii_case(perm));
            if player.permissions.len() < before {
                Ok(())
            } else {
                Err(format!(
                    "Player '{}' did not have permission '{}'.",
                    username, permission
                ))
            }
        } else {
            Err(format!("Player '{}' not found in permissions.", username))
        }
    }

    pub fn get_player_groups(&self, username: &str) -> Vec<String> {
        let canonical_user = username.trim().to_ascii_lowercase();
        if let Some(player) = self.players.get(&canonical_user) {
            player.groups.clone()
        } else {
            vec!["default".to_string()]
        }
    }

    pub fn get_player_permissions(&self, username: &str) -> Vec<String> {
        let canonical_user = username.trim().to_ascii_lowercase();
        if let Some(player) = self.players.get(&canonical_user) {
            player.permissions.clone()
        } else {
            Vec::new()
        }
    }

    /// Sets the chat prefix for a permission group.
    pub fn set_group_prefix(&mut self, group_name: &str, prefix: &str) -> Result<(), String> {
        let key = group_name.trim().to_ascii_lowercase();
        if let Some(group) = self.groups.get_mut(&key) {
            group.prefix = prefix.to_string();
            Ok(())
        } else {
            Err(format!("Group '{}' not found.", group_name))
        }
    }

    /// Sets the priority for a permission group (higher priority wins for chat prefixes).
    pub fn set_group_priority(&mut self, group_name: &str, priority: i32) -> Result<(), String> {
        let key = group_name.trim().to_ascii_lowercase();
        if let Some(group) = self.groups.get_mut(&key) {
            group.priority = priority;
            Ok(())
        } else {
            Err(format!("Group '{}' not found.", group_name))
        }
    }

    /// Returns the highest-priority non-empty group prefix for `username`.
    pub fn get_player_prefix(&self, username: &str) -> String {
        let canonical_user = username.trim().to_ascii_lowercase();
        let groups = self.get_effective_groups(&canonical_user);
        let mut best: Option<(i32, &str)> = None;
        for group_name in &groups {
            if let Some(group) = self.groups.get(group_name) {
                if !group.prefix.is_empty() {
                    if best.map_or(true, |(prio, _)| group.priority > prio) {
                        best = Some((group.priority, group.prefix.as_str()));
                    }
                }
            }
        }
        best.map(|(_, p)| p.to_string()).unwrap_or_default()
    }

    // --- Persistence ---

    /// Saves permissions to `<dir>/permissions.yml`.
    pub fn save_to_dir(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        let path = dir.join("permissions.yml");

        let config = PermissionsYamlConfig {
            groups: self
                .groups
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            players: self
                .players
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        };

        let yaml = serde_yaml::to_string(&config)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("YAML serialize error: {e}")))?;

        let mut file = File::create(path)?;
        writeln!(file, "# BlockGame Permissions Configuration")?;
        writeln!(file, "# Groups define permissions, inheritance, priority, and & color-coded prefixes.")?;
        file.write_all(yaml.as_bytes())?;
        file.flush()
    }

    /// Loads permissions from `<dir>/permissions.yml`.
    /// If `permissions.yml` does not exist but a legacy `permissions.txt` does, migrates it automatically.
    pub fn load_from_dir(dir: &Path) -> io::Result<Self> {
        let yml_path = dir.join("permissions.yml");
        if !yml_path.exists() {
            let legacy_txt_path = dir.join("permissions.txt");
            if legacy_txt_path.exists() {
                let migrated = Self::load_legacy_txt(&legacy_txt_path)?;
                let _ = migrated.save_to_dir(dir);
                return Ok(migrated);
            }

            let default_mgr = Self::new();
            let _ = default_mgr.save_to_dir(dir);
            return Ok(default_mgr);
        }

        let content = fs::read_to_string(&yml_path)?;
        let config: PermissionsYamlConfig = serde_yaml::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid permissions.yml: {e}")))?;

        let mut mgr = Self {
            groups: config
                .groups
                .into_iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), v))
                .collect(),
            players: config
                .players
                .into_iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), v))
                .collect(),
        };

        if !mgr.groups.contains_key("default") {
            mgr.groups
                .insert("default".to_string(), PermissionGroup::new("default"));
        }

        Ok(mgr)
    }

    /// Parses legacy `<dir>/permissions.txt` INI-like format for one-time migration to `permissions.yml`.
    fn load_legacy_txt(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut mgr = Self {
            groups: HashMap::new(),
            players: HashMap::new(),
        };

        let mut current_section: Option<String> = None;
        let mut cur_group = PermissionGroup::new("");
        let mut cur_player = PlayerPermissions::new("");

        for line_res in reader.lines() {
            let line = line_res?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if let Some(sec) = current_section.take() {
                    if let Some(group_key) = sec.strip_prefix("group:") {
                        mgr.groups.insert(
                            group_key.to_string(),
                            std::mem::replace(&mut cur_group, PermissionGroup::new("")),
                        );
                    } else if let Some(player_key) = sec.strip_prefix("player:") {
                        mgr.players.insert(
                            player_key.to_string(),
                            std::mem::replace(&mut cur_player, PlayerPermissions::new("")),
                        );
                    }
                }

                let sec_name = &trimmed[1..trimmed.len() - 1];
                current_section = Some(sec_name.to_string());
                if let Some(gk) = sec_name.strip_prefix("group:") {
                    cur_group = PermissionGroup::new(gk);
                } else if let Some(pk) = sec_name.strip_prefix("player:") {
                    cur_player = PlayerPermissions::new(pk);
                }
                continue;
            }

            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim();
                let val = v.trim();

                if let Some(sec) = &current_section {
                    if sec.starts_with("group:") {
                        match key {
                            "name" => cur_group.name = val.to_string(),
                            "permissions" => {
                                cur_group.permissions = val
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                            }
                            "inherits" => {
                                cur_group.inherits = val
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                            }
                            _ => {}
                        }
                    } else if sec.starts_with("player:") {
                        match key {
                            "username" => cur_player.username = val.to_string(),
                            "groups" => {
                                cur_player.groups = val
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                            }
                            "permissions" => {
                                cur_player.permissions = val
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if let Some(sec) = current_section {
            if let Some(group_key) = sec.strip_prefix("group:") {
                mgr.groups.insert(group_key.to_string(), cur_group);
            } else if let Some(player_key) = sec.strip_prefix("player:") {
                mgr.players.insert(player_key.to_string(), cur_player);
            }
        }

        if !mgr.groups.contains_key("default") {
            mgr.groups
                .insert("default".to_string(), PermissionGroup::new("default"));
        }
        if let Some(admin) = mgr.groups.get_mut("admin") {
            if admin.prefix.is_empty() && admin.priority == 0 {
                admin.prefix = "&c[Admin] ".to_string();
                admin.priority = 100;
            }
        }
        if let Some(mod_group) = mgr.groups.get_mut("moderator") {
            if mod_group.prefix.is_empty() && mod_group.priority == 0 {
                mod_group.prefix = "&9[Mod] ".to_string();
                mod_group.priority = 50;
            }
        }

        Ok(mgr)
    }
}

/// A ban record detailing a banned player, the banner, reason, and timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct BanEntry {
    pub username: String,
    pub reason: String,
    pub banned_by: String,
    pub created_at: u64,
}

/// Persistent ban list manager (`bans.txt`).
#[derive(Debug, Clone, Default)]
pub struct BanManager {
    /// Keyed by lowercase canonical username
    pub bans: HashMap<String, BanEntry>,
}

impl BanManager {
    pub fn new() -> Self {
        Self {
            bans: HashMap::new(),
        }
    }

    pub fn ban(&mut self, username: &str, reason: &str, banned_by: &str) {
        let key = username.trim().to_ascii_lowercase();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.bans.insert(
            key,
            BanEntry {
                username: username.trim().to_string(),
                reason: reason.trim().to_string(),
                banned_by: banned_by.trim().to_string(),
                created_at: now,
            },
        );
    }

    pub fn unban(&mut self, username: &str) -> bool {
        let key = username.trim().to_ascii_lowercase();
        self.bans.remove(&key).is_some()
    }

    pub fn is_banned(&self, username: &str) -> Option<&BanEntry> {
        let key = username.trim().to_ascii_lowercase();
        self.bans.get(&key)
    }

    pub fn list_bans(&self) -> Vec<&BanEntry> {
        let mut list: Vec<&BanEntry> = self.bans.values().collect();
        list.sort_by_key(|b| &b.username);
        list
    }

    pub fn save_to_dir(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        let path = dir.join("bans.txt");
        let mut file = File::create(path)?;

        writeln!(file, "# BlockGame Server Ban List")?;
        writeln!(file, "# Format: username|banned_by|timestamp|reason")?;
        for entry in self.bans.values() {
            writeln!(
                file,
                "{}|{}|{}|{}",
                entry.username, entry.banned_by, entry.created_at, entry.reason
            )?;
        }
        file.flush()
    }

    pub fn load_from_dir(dir: &Path) -> io::Result<Self> {
        let path = dir.join("bans.txt");
        if !path.exists() {
            return Ok(Self::new());
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut mgr = Self::new();

        for line_res in reader.lines() {
            let line = line_res?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = trimmed.splitn(4, '|').collect();
            if parts.len() >= 4 {
                let username = parts[0].trim();
                let banned_by = parts[1].trim();
                let created_at: u64 = parts[2].trim().parse().unwrap_or(0);
                let reason = parts[3].trim();

                mgr.bans.insert(
                    username.to_ascii_lowercase(),
                    BanEntry {
                        username: username.to_string(),
                        banned_by: banned_by.to_string(),
                        created_at,
                        reason: reason.to_string(),
                    },
                );
            }
        }

        Ok(mgr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_matching() {
        assert!(permission_matches("*", "blockgame.command.kick"));
        assert!(permission_matches("blockgame.*", "blockgame.command.kick"));
        assert!(permission_matches("blockgame.command.*", "blockgame.command.kick"));
        assert!(permission_matches("blockgame.command.kick", "blockgame.command.kick"));
        assert!(!permission_matches("blockgame.command.ban", "blockgame.command.kick"));
        assert!(!permission_matches("other.*", "blockgame.command.kick"));
    }

    #[test]
    fn test_permissions_manager_groups_and_inheritance() {
        let mut pm = PermissionsManager::new();

        // 1. Admin player has all permissions
        pm.add_player_group("Alice", "admin").unwrap();
        assert!(pm.has_permission("Alice", "blockgame.command.kick"));
        assert!(pm.has_permission("Alice", "blockgame.command.ban"));
        assert!(pm.has_permission("Alice", "custom.special.perm"));

        // 2. Moderator player has kick and tp, but not ban
        pm.add_player_group("Bob", "moderator").unwrap();
        assert!(pm.has_permission("Bob", "blockgame.command.kick"));
        assert!(pm.has_permission("Bob", "blockgame.command.tp"));
        assert!(!pm.has_permission("Bob", "blockgame.command.ban"));

        // 3. Regular player in default group has no admin commands
        assert!(!pm.has_permission("Charlie", "blockgame.command.kick"));

        // 4. Giving Charlie direct permission
        pm.add_player_permission("Charlie", "blockgame.command.tp").unwrap();
        assert!(pm.has_permission("Charlie", "blockgame.command.tp"));
        assert!(!pm.has_permission("Charlie", "blockgame.command.kick"));

        // 5. Explicit negative permission overrides wildcard
        pm.add_player_permission("Alice", "-blockgame.command.ban").unwrap();
        assert!(!pm.has_permission("Alice", "blockgame.command.ban"));
        assert!(pm.has_permission("Alice", "blockgame.command.kick"));
    }

    #[test]
    fn test_permissions_manager_persistence_roundtrip() {
        let temp_dir = std::env::temp_dir().join("blockgame_perm_test");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut pm = PermissionsManager::new();
        pm.create_group("vip").unwrap();
        pm.set_group_prefix("vip", "&b[VIP] ").unwrap();
        pm.set_group_priority("vip", 25).unwrap();
        pm.add_group_permission("vip", "blockgame.command.tp").unwrap();
        pm.add_player_group("ExplorerBob", "vip").unwrap();
        pm.add_player_permission("ExplorerBob", "blockgame.command.tphere").unwrap();

        pm.save_to_dir(&temp_dir).unwrap();
        assert!(temp_dir.join("permissions.yml").exists());

        let loaded = PermissionsManager::load_from_dir(&temp_dir).unwrap();
        assert!(loaded.has_permission("ExplorerBob", "blockgame.command.tp"));
        assert!(loaded.has_permission("ExplorerBob", "blockgame.command.tphere"));
        assert!(!loaded.has_permission("ExplorerBob", "blockgame.command.ban"));
        assert_eq!(loaded.get_player_prefix("ExplorerBob"), "&b[VIP] ");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_prefix_priority_and_legacy_txt_migration() {
        let temp_dir = std::env::temp_dir().join("blockgame_perm_migrate_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Write a legacy permissions.txt
        let legacy = "\
[group:default]
name=default
permissions=
inherits=

[group:builder]
name=builder
permissions=blockgame.chat.color
inherits=

[player:alice]
username=Alice
groups=default,builder
permissions=
";
        fs::write(temp_dir.join("permissions.txt"), legacy).unwrap();

        let loaded = PermissionsManager::load_from_dir(&temp_dir).unwrap();
        assert!(temp_dir.join("permissions.yml").exists(), "Should auto-migrate to permissions.yml");
        assert!(loaded.has_permission("Alice", "blockgame.chat.color"));

        // Priority resolution: admin (100) beats moderator (50)
        let mut pm = PermissionsManager::new();
        pm.add_player_group("Alice", "moderator").unwrap();
        pm.add_player_group("Alice", "admin").unwrap();
        assert_eq!(pm.get_player_prefix("Alice"), "&c[Admin] ");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ban_manager_roundtrip() {
        let temp_dir = std::env::temp_dir().join("blockgame_ban_test");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut bm = BanManager::new();
        bm.ban("Griefer1", "Spamming TNT", "Admin");
        assert!(bm.is_banned("griefer1").is_some());
        assert!(bm.is_banned("Griefer1").is_some());
        assert!(bm.is_banned("FriendlyPlayer").is_none());

        bm.save_to_dir(&temp_dir).unwrap();

        let loaded = BanManager::load_from_dir(&temp_dir).unwrap();
        let ban = loaded.is_banned("Griefer1").expect("Must be banned in loaded db");
        assert_eq!(ban.reason, "Spamming TNT");
        assert_eq!(ban.banned_by, "Admin");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
