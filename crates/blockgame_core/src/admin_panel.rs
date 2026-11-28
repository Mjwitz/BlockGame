use serde::{Deserialize, Serialize};

use crate::automessage::AutoMessageConfig;
use crate::moderation::{ChatSettingsConfig, MuteEntry, WarningEntry, WordFilterConfig};
use crate::permissions::{
    BanEntry, IpBanEntry, PermissionGroup, PermissionsManager, PlayerPermissions,
};

/// Modular tabs inside the unified in-game Admin Panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AdminTab {
    Players,
    Bans,
    Chat,
    Permissions,
    AutoMessages,
    Server,
}

impl AdminTab {
    pub const ALL: &'static [AdminTab] = &[
        AdminTab::Players,
        AdminTab::Bans,
        AdminTab::Chat,
        AdminTab::Permissions,
        AdminTab::AutoMessages,
        AdminTab::Server,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Players => "Players",
            Self::Bans => "Bans",
            Self::Chat => "Chat",
            Self::Permissions => "Permissions",
            Self::AutoMessages => "Auto-Msg",
            Self::Server => "Server",
        }
    }
}

/// Built-in catalog of all known `blockgame.*` permission nodes surfaced in the Permissions tab.
pub const KNOWN_PERMISSION_NODES: &[&str] = &[
    "blockgame.admin.players",
    "blockgame.admin.bans",
    "blockgame.admin.chat",
    "blockgame.admin.permissions",
    "blockgame.admin.automessages",
    "blockgame.admin.server",
    "blockgame.command.say",
    "blockgame.command.kick",
    "blockgame.command.ban",
    "blockgame.command.unban",
    "blockgame.command.banip",
    "blockgame.command.unbanip",
    "blockgame.command.banlist",
    "blockgame.command.tp",
    "blockgame.command.tphere",
    "blockgame.command.give",
    "blockgame.command.gamemode",
    "blockgame.command.skills",
    "blockgame.command.skills.admin",
    "blockgame.command.rank",
    "blockgame.command.resetplayer",
    "blockgame.command.setspawn",
    "blockgame.command.stop",
    "blockgame.command.perm",
    "blockgame.command.spawn",
    "blockgame.command.home",
    "blockgame.command.sethome",
    "blockgame.command.delhome",
    "blockgame.command.homes",
    "blockgame.command.tpa",
    "blockgame.command.tpaccept",
    "blockgame.command.tpdeny",
    "blockgame.command.automessage",
    "blockgame.command.mute",
    "blockgame.command.unmute",
    "blockgame.command.warn",
    "blockgame.command.warnings",
    "blockgame.command.clearwarnings",
    "blockgame.command.wordfilter",
    "blockgame.command.chatlock",
    "blockgame.command.slowmode",
    "blockgame.command.clearchat",
    "blockgame.chat.color",
    "blockgame.chat.lock.bypass",
    "blockgame.chat.slowmode.bypass",
    "blockgame.chat.filter.bypass",
    "blockgame.claim.admin",
    "blockgame.claim.bypass",
    "blockgame.log.lookup",
    "blockgame.log.inspect",
    "blockgame.log.rollback",
    "blockgame.log.purge",
];

/// Returns which Admin Panel tabs a player is permitted to see and use.
pub fn visible_tabs_for_player(perms: &PermissionsManager, username: &str) -> Vec<AdminTab> {
    let has_any = |nodes: &[&str]| nodes.iter().any(|n| perms.has_permission(username, n));
    let mut tabs = Vec::new();
    if has_any(&[
        "blockgame.admin.players",
        "blockgame.command.kick",
        "blockgame.command.ban",
        "blockgame.command.mute",
        "blockgame.command.warn",
        "blockgame.command.tp",
        "blockgame.command.tphere",
        "blockgame.command.gamemode",
        "blockgame.command.rank",
        "blockgame.command.resetplayer",
    ]) {
        tabs.push(AdminTab::Players);
    }
    if has_any(&[
        "blockgame.admin.bans",
        "blockgame.command.banlist",
        "blockgame.command.ban",
        "blockgame.command.unban",
        "blockgame.command.banip",
        "blockgame.command.unbanip",
    ]) {
        tabs.push(AdminTab::Bans);
    }
    if has_any(&[
        "blockgame.admin.chat",
        "blockgame.command.mute",
        "blockgame.command.unmute",
        "blockgame.command.warn",
        "blockgame.command.warnings",
        "blockgame.command.clearwarnings",
        "blockgame.command.wordfilter",
        "blockgame.command.chatlock",
        "blockgame.command.slowmode",
        "blockgame.command.clearchat",
    ]) {
        tabs.push(AdminTab::Chat);
    }
    if has_any(&["blockgame.admin.permissions", "blockgame.command.perm"]) {
        tabs.push(AdminTab::Permissions);
    }
    if has_any(&["blockgame.admin.automessages", "blockgame.command.automessage"]) {
        tabs.push(AdminTab::AutoMessages);
    }
    if has_any(&[
        "blockgame.admin.server",
        "blockgame.command.say",
        "blockgame.command.setspawn",
        "blockgame.command.stop",
    ]) {
        tabs.push(AdminTab::Server);
    }
    tabs
}

/// Summary of an online player shown in the Admin Panel `Players` tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminOnlinePlayer {
    pub username: String,
    pub gamemode: String,
    pub groups: Vec<String>,
    pub pos: [i32; 3],
    pub is_muted: bool,
    pub warning_count: usize,
}

/// Live server metrics and toggles shown in the Admin Panel `Server` tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminServerStats {
    pub world_name: String,
    pub online_count: usize,
    pub uptime_secs: u64,
    #[serde(default)]
    pub tps: f32,
    pub broadcast_achievements: bool,
}

/// Permission-scoped snapshot sent from the server to an authorized staff client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AdminPanelSnapshot {
    pub allowed_tabs: Vec<AdminTab>,
    #[serde(default)]
    pub requested_tab: Option<AdminTab>,
    #[serde(default)]
    pub status_message: String,
    #[serde(default)]
    pub online_players: Vec<AdminOnlinePlayer>,
    #[serde(default)]
    pub banned_players: Vec<BanEntry>,
    #[serde(default)]
    pub banned_ips: Vec<IpBanEntry>,
    #[serde(default)]
    pub active_mutes: Vec<MuteEntry>,
    #[serde(default)]
    pub recent_warnings: Vec<WarningEntry>,
    #[serde(default)]
    pub word_filter: WordFilterConfig,
    #[serde(default)]
    pub chat_settings: ChatSettingsConfig,
    #[serde(default)]
    pub automessages: Option<AutoMessageConfig>,
    #[serde(default)]
    pub server_stats: Option<AdminServerStats>,
    #[serde(default)]
    pub permissions_groups: Vec<PermissionGroup>,
    #[serde(default)]
    pub permissions_players: Vec<PlayerPermissions>,
    #[serde(default)]
    pub known_permission_nodes: Vec<String>,
}

/// Actions sent from the Admin Panel client to the dedicated server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminPanelAction {
    RequestSnapshot {
        preferred_tab: Option<AdminTab>,
    },
    /// Executes an equivalent slash command through the server's standard command & permission pipeline.
    RunCommand(String),
    SetBroadcastAchievements(bool),
    SaveWorld,
    // --- Permissions tab mutations (used by #41) ---
    CreateGroup {
        name: String,
    },
    DeleteGroup {
        name: String,
    },
    UpdateGroupMeta {
        name: String,
        prefix: String,
        priority: i32,
        inherits: Vec<String>,
    },
    SetGroupNode {
        group: String,
        node: String,
        /// `Some(true)` = Allow, `Some(false)` = Deny (`-node`), `None` = Neutral (remove)
        state: Option<bool>,
    },
    SetPlayerGroup {
        username: String,
        group: String,
        assigned: bool,
    },
    SetPlayerNode {
        username: String,
        node: String,
        state: Option<bool>,
    },
}

/// Validates that an in-game permission edit does not delete `default` or strip the acting
/// player's own ability to manage permissions (server console is unrestricted).
pub fn validate_no_self_lockout(
    candidate: &PermissionsManager,
    acting_username: &str,
) -> Result<(), String> {
    if !candidate.groups.contains_key("default") {
        return Err("Cannot delete the built-in 'default' group from in-game.".to_string());
    }
    let can_still_manage = candidate.has_permission(acting_username, "blockgame.command.perm")
        || candidate.has_permission(acting_username, "blockgame.admin.permissions");
    if !can_still_manage {
        return Err(
            "Blocked by self-lockout guard: this edit would remove your own permissions management access."
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_tabs_are_permission_scoped() {
        let mut perms = PermissionsManager::new();
        perms.add_player_group("Regular", "default").unwrap();
        assert!(visible_tabs_for_player(&perms, "Regular").is_empty());

        perms.add_player_group("ModUser", "moderator").unwrap();
        let mod_tabs = visible_tabs_for_player(&perms, "ModUser");
        assert!(mod_tabs.contains(&AdminTab::Players));
        assert!(!mod_tabs.contains(&AdminTab::Permissions));
        assert!(!mod_tabs.contains(&AdminTab::Server));

        perms.add_player_group("AdminUser", "admin").unwrap();
        assert_eq!(visible_tabs_for_player(&perms, "AdminUser").len(), AdminTab::ALL.len());
    }

    #[test]
    fn self_lockout_guard_blocks_default_delete_and_self_demote() {
        let mut perms = PermissionsManager::new();
        perms.add_player_group("Owner", "admin").unwrap();
        assert!(validate_no_self_lockout(&perms, "Owner").is_ok());

        let mut removed_default = perms.clone();
        removed_default.groups.remove("default");
        assert!(validate_no_self_lockout(&removed_default, "Owner").is_err());

        let mut demoted = perms.clone();
        let _ = demoted.remove_player_group("Owner", "admin");
        assert!(validate_no_self_lockout(&demoted, "Owner").is_err());
    }
}
