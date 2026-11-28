use std::collections::HashMap;
use std::time::Instant;

use glam::Vec3;

use crate::{
    item::{ItemId, ItemRegistry, ItemSlot},
    permissions::{BanManager, PermissionsManager},
};

/// The execution origin of a server command.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandSource {
    Console,
    Player {
        player_id: u32,
        username: String,
    },
}

impl CommandSource {
    pub fn username(&self) -> &str {
        match self {
            Self::Console => "Console",
            Self::Player { username, .. } => username.as_str(),
        }
    }

    pub fn player_id(&self) -> Option<u32> {
        match self {
            Self::Console => None,
            Self::Player { player_id, .. } => Some(*player_id),
        }
    }

    pub fn is_console(&self) -> bool {
        matches!(self, Self::Console)
    }
}

/// Snapshot of an online player used by the command processor.
#[derive(Debug, Clone, PartialEq)]
pub struct OnlinePlayerInfo {
    pub player_id: u32,
    pub username: String,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub homes: HashMap<String, (Vec3, f32, f32)>,
    pub ip: String,
}

impl OnlinePlayerInfo {
    pub fn new(player_id: u32, username: impl Into<String>, position: Vec3, yaw: f32, pitch: f32) -> Self {
        Self {
            player_id,
            username: username.into(),
            position,
            yaw,
            pitch,
            homes: HashMap::new(),
            ip: "127.0.0.1".to_string(),
        }
    }

    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip = ip.into();
        self
    }
}

/// Pending TPA request recorded on the server.
#[derive(Debug, Clone)]
pub struct PendingTpaRequest {
    pub requester_id: u32,
    pub requester_name: String,
    pub created_at: Instant,
}

/// Declarative results produced by executing a command.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CommandOutput {
    /// Private feedback sent directly to the command sender.
    pub feedback: Vec<String>,
    /// Global system broadcasts sent to all connected players.
    pub broadcasts: Vec<String>,
    /// Direct messages sent to specific players: `(player_id, message)`.
    pub direct_messages: Vec<(u32, String)>,
    /// Players to immediately kick with `(player_id, reason)`.
    pub kick_targets: Vec<(u32, String)>,
    /// Teleport directives: `(player_id, target_position, yaw, pitch)`.
    pub teleports: Vec<(u32, Vec3, f32, f32)>,
    /// Items to grant to a player: `(player_id, item_slot)`.
    pub gives: Vec<(u32, ItemSlot)>,
    /// Flag indicating whether permissions configuration changed and should be saved.
    pub save_permissions: bool,
    /// Flag indicating whether ban records changed and should be saved.
    pub save_bans: bool,
    /// Flag indicating whether the server should cleanly shut down.
    pub stop_server: bool,
    /// Rank updates to apply to player data: `(username, new_rank)`.
    pub rank_updates: Vec<(String, String)>,
    /// Player accounts/data to reset: `username`.
    pub reset_players: Vec<String>,
    /// New world spawn point: `position`.
    pub new_spawn: Option<Vec3>,
    /// Set home: `(player_id, home_name, position, yaw, pitch)`.
    pub set_home: Option<(u32, String, Vec3, f32, f32)>,
    /// Delete home: `(player_id, home_name)`.
    pub del_home: Option<(u32, String)>,
    /// GameMode updates: `(player_id, new_mode)`.
    pub gamemode_updates: Vec<(u32, crate::player::GameMode)>,
    /// Skill inspection or modification actions: handled by the server loop.
    pub skill_actions: Vec<SkillCommandAction>,
    /// Auto-Message configuration or listing actions: handled by the server loop.
    pub automessage_actions: Vec<AutoMessageCommandAction>,
    /// Chat moderation actions: handled by the server loop.
    pub moderation_actions: Vec<ModerationCommandAction>,
    /// Requests opening the in-game Admin Panel for the command sender (with optional target tab).
    pub open_admin_panel: Option<Option<crate::admin_panel::AdminTab>>,
    /// Land claiming actions: handled by the server loop.
    pub claim_actions: Vec<ClaimCommandAction>,
    /// Block Log actions (`/log lookup|inspect|rollback|undo|purge`): handled by the server loop.
    pub log_actions: Vec<LogCommandAction>,
    /// Player IDs whose Achievement Eligibility is forfeited by this admin/cheat command.
    pub cheat_taint_players: Vec<u32>,
}

/// Block Log actions produced by `/log` and `/rollback`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogCommandAction {
    Lookup {
        player: Option<String>,
        time_secs: Option<u64>,
        radius: Option<i32>,
        action: Option<String>,
        page: usize,
    },
    InspectToggle,
    Rollback {
        radius: i32,
        time_secs: u64,
        player: Option<String>,
        preview: bool,
    },
    Undo,
    Purge {
        older_than_secs: u64,
    },
}

/// Land claiming actions produced by `/claim`, `/unclaim`, and `/unclaimall`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimCommandAction {
    ClaimCurrent,
    UnclaimCurrent,
    UnclaimAll,
    Info,
    List,
    Map,
    Trust(String),
    Untrust(String),
    TrustList,
    AdminClaim(Option<String>),
    AdminBypassToggle,
    AdminUnclaim,
    ToggleBorders,
}

/// Chat moderation actions produced by `/mute`, `/unmute`, `/warn`, `/warnings`,
/// `/clearwarnings`, `/wordfilter`, `/chatlock`, `/slowmode`, and `/clearchat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationCommandAction {
    Mute {
        target: String,
        duration_secs: Option<u64>,
        reason: String,
    },
    Unmute {
        target: String,
    },
    Warn {
        target: String,
        reason: String,
    },
    Warnings {
        target: String,
    },
    ClearWarnings {
        target: String,
    },
    WordFilterStatus,
    WordFilterSetEnabled(bool),
    WordFilterSetMode(crate::moderation::WordFilterMode),
    WordFilterList,
    WordFilterAdd(String),
    WordFilterRemove(String),
    ChatLockToggle(Option<bool>),
    SlowmodeSet(u64),
    ClearChat,
}

/// Auto-Message management actions produced by `/automessage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoMessageCommandAction {
    Status,
    SetEnabled(bool),
    SetInterval(u64),
    SetOrder(crate::automessage::AutoMessageOrder),
    List,
    Add(String),
    Remove(usize),
}

/// Skill inspection or administrative adjustment produced by `/skills`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillCommandAction {
    View {
        requester_pid: u32,
        skill: Option<crate::skills::SkillKind>,
    },
    Set {
        target_pid: u32,
        skill: crate::skills::SkillKind,
        level: u16,
    },
    AddXp {
        target_pid: u32,
        skill: crate::skills::SkillKind,
        amount: u64,
    },
    Reset {
        target_pid: u32,
    },
}

impl CommandOutput {
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            feedback: vec![format!("[Error] {}", msg.into())],
            ..Default::default()
        }
    }

    pub fn success(msg: impl Into<String>) -> Self {
        Self {
            feedback: vec![msg.into()],
            ..Default::default()
        }
    }
}

/// Resolves an item from either an integer ID or a friendly name (case-insensitive).
pub fn resolve_item(input: &str, registry: &ItemRegistry) -> Option<(ItemId, String)> {
    let input = input.trim();
    if let Ok(id_num) = input.parse::<u16>() {
        let item_id = ItemId(id_num);
        if let Some(def) = registry.get(item_id) {
            return Some((item_id, def.display_name.to_string()));
        }
    }

    let search = input.to_ascii_lowercase().replace(' ', "_");
    for def in registry.all_items() {
        let canonical_name = def.name.to_ascii_lowercase();
        let display = def.display_name.to_ascii_lowercase().replace(' ', "_");
        let stripped = canonical_name.strip_prefix("blockgame:").unwrap_or(&canonical_name);

        if stripped == search || display == search || canonical_name == search {
            return Some((def.id, def.display_name.to_string()));
        }
    }

    None
}

/// Executes a server command with full permission checks, argument parsing, and state updates.
pub fn execute_command(
    source: &CommandSource,
    command_line: &str,
    online_players: &[OnlinePlayerInfo],
    perms: &mut PermissionsManager,
    bans: &mut BanManager,
    item_registry: &ItemRegistry,
    default_spawn: Vec3,
    tpa_requests: &mut HashMap<u32, PendingTpaRequest>,
) -> CommandOutput {
    let mut out = CommandOutput::default();
    let raw = command_line.trim();
    let clean = raw.strip_prefix('/').unwrap_or(raw).trim();

    if clean.is_empty() {
        return out;
    }

    let parts: Vec<&str> = clean.split_whitespace().collect();
    let initial_cmd = parts[0].to_ascii_lowercase();
    let initial_args = &parts[1..];

    // Handle `/set spawn` alias to `/setspawn`
    let (cmd, args) = if initial_cmd == "set" && initial_args.first().map(|a| a.eq_ignore_ascii_case("spawn")).unwrap_or(false) {
        ("setspawn".to_string(), &initial_args[1..])
    } else {
        (initial_cmd, initial_args)
    };

    // Helper to check permissions
    let check_perm = |node: &str| -> bool {
        match source {
            CommandSource::Console => true,
            CommandSource::Player { username, .. } => perms.has_permission(username, node),
        }
    };

    match cmd.as_str() {
        // --- 1. KICK COMMAND ---
        "kick" => {
            if !check_perm("blockgame.command.kick") {
                return CommandOutput::error("You do not have permission: blockgame.command.kick");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /kick <player> [reason]");
            }
            let target_name = args[0];
            let reason = if args.len() > 1 {
                args[1..].join(" ")
            } else {
                "Kicked by an operator.".to_string()
            };

            if let Some(target) = online_players
                .iter()
                .find(|p| p.username.eq_ignore_ascii_case(target_name))
            {
                out.kick_targets.push((target.player_id, reason.clone()));
                out.broadcasts.push(format!(
                    "{} was kicked by {} ({})",
                    target.username,
                    source.username(),
                    reason
                ));
                out.feedback
                    .push(format!("Kicked '{}' for: {}", target.username, reason));
            } else {
                return CommandOutput::error(format!("Player '{}' is not online.", target_name));
            }
        }

        // --- 2. BAN COMMAND ---
        "ban" => {
            if !check_perm("blockgame.command.ban") {
                return CommandOutput::error("You do not have permission: blockgame.command.ban");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /ban <player> [reason]");
            }
            let target_name = args[0];
            let reason = if args.len() > 1 {
                args[1..].join(" ")
            } else {
                "Banned by an operator.".to_string()
            };

            bans.ban(target_name, &reason, source.username());
            out.save_bans = true;

            // If online, kick them immediately
            if let Some(target) = online_players
                .iter()
                .find(|p| p.username.eq_ignore_ascii_case(target_name))
            {
                out.kick_targets
                    .push((target.player_id, format!("Banned: {}", reason)));
            }

            out.broadcasts.push(format!(
                "{} was banned by {} ({})",
                target_name,
                source.username(),
                reason
            ));
            out.feedback
                .push(format!("Banned '{}' for: {}", target_name, reason));
        }

        // --- 3. UNBAN COMMAND ---
        "unban" | "pardon" => {
            if !check_perm("blockgame.command.ban") {
                return CommandOutput::error("You do not have permission: blockgame.command.ban");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /unban <player>");
            }
            let target_name = args[0];
            if bans.unban(target_name) {
                out.save_bans = true;
                out.feedback.push(format!("Unbanned player '{}'.", target_name));
            } else {
                return CommandOutput::error(format!("Player '{}' is not banned.", target_name));
            }
        }

        // --- BAN-IP COMMAND ---
        "ban-ip" | "banip" => {
            if !check_perm("blockgame.command.ban") && !check_perm("blockgame.command.banip") {
                return CommandOutput::error("You do not have permission: blockgame.command.ban");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /ban-ip <player|ip> [reason]");
            }
            let target_arg = args[0];
            let reason = if args.len() > 1 {
                args[1..].join(" ")
            } else {
                "Banned by an operator.".to_string()
            };

            // Find if target is an online player
            let target_ip = if let Some(target_player) = online_players
                .iter()
                .find(|p| p.username.eq_ignore_ascii_case(target_arg))
            {
                target_player.ip.clone()
            } else if target_arg.parse::<std::net::IpAddr>().is_ok() {
                target_arg.to_string()
            } else {
                return CommandOutput::error(format!(
                    "Player '{}' is not online and is not a valid IP address.",
                    target_arg
                ));
            };

            bans.ban_ip(&target_ip, &reason, source.username());
            out.save_bans = true;

            // Kick any online players with this IP
            for p in online_players {
                if p.ip == target_ip {
                    out.kick_targets
                        .push((p.player_id, format!("Banned IP: {}", reason)));
                }
            }

            out.broadcasts.push(format!(
                "IP {} was banned by {} ({})",
                target_ip,
                source.username(),
                reason
            ));
            out.feedback
                .push(format!("Banned IP '{}' for: {}", target_ip, reason));
        }

        // --- UNBAN-IP COMMAND ---
        "unban-ip" | "unbanip" | "pardon-ip" | "pardonip" => {
            if !check_perm("blockgame.command.ban") && !check_perm("blockgame.command.banip") {
                return CommandOutput::error("You do not have permission: blockgame.command.ban");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /unban-ip <ip>");
            }
            let target_ip = args[0];
            if bans.unban_ip(target_ip) {
                out.save_bans = true;
                out.feedback.push(format!("Unbanned IP '{}'.", target_ip));
            } else {
                return CommandOutput::error(format!("IP '{}' is not banned.", target_ip));
            }
        }

        // --- BANLIST COMMAND ---
        "banlist" => {
            if !check_perm("blockgame.command.ban") && !check_perm("blockgame.command.banip") {
                return CommandOutput::error("You do not have permission: blockgame.command.ban");
            }
            let show_ips = args.first().map_or(false, |a| a.eq_ignore_ascii_case("ips") || a.eq_ignore_ascii_case("ip"));
            if show_ips {
                let ip_bans = bans.list_ip_bans();
                if ip_bans.is_empty() {
                    out.feedback.push("There are no banned IP addresses.".to_string());
                } else {
                    out.feedback.push(format!("There are {} banned IP(s):", ip_bans.len()));
                    for b in ip_bans {
                        out.feedback.push(format!(" - {} (by {}, reason: {})", b.ip, b.banned_by, b.reason));
                    }
                }
            } else {
                let player_bans = bans.list_bans();
                if player_bans.is_empty() {
                    out.feedback.push("There are no banned players.".to_string());
                } else {
                    out.feedback.push(format!("There are {} banned player(s):", player_bans.len()));
                    for b in player_bans {
                        out.feedback.push(format!(" - {} (by {}, reason: {})", b.username, b.banned_by, b.reason));
                    }
                }
            }
        }

        // --- 4. TELEPORT (TP) COMMAND ---
        "tp" => {
            if !check_perm("blockgame.command.tp") {
                return CommandOutput::error("You do not have permission: blockgame.command.tp");
            }

            match args.len() {
                // /tp <target_player> (teleports caller to target)
                1 => {
                    let caller_id = match source {
                        CommandSource::Player { player_id, .. } => *player_id,
                        CommandSource::Console => {
                            return CommandOutput::error(
                                "Console must specify: /tp <player> <target> or /tp <player> <x> <y> <z>",
                            );
                        }
                    };
                    let target_name = args[0];
                    if let Some(target) = online_players
                        .iter()
                        .find(|p| p.username.eq_ignore_ascii_case(target_name))
                    {
                        out.teleports.push((caller_id, target.position, target.yaw, target.pitch));
                        out.feedback.push(format!("Teleported to {}.", target.username));
                    } else {
                        return CommandOutput::error(format!("Player '{}' is not online.", target_name));
                    }
                }
                // /tp <player> <target>
                2 => {
                    let victim_name = args[0];
                    let target_name = args[1];

                    let victim = online_players
                        .iter()
                        .find(|p| p.username.eq_ignore_ascii_case(victim_name));
                    let target = online_players
                        .iter()
                        .find(|p| p.username.eq_ignore_ascii_case(target_name));

                    match (victim, target) {
                        (Some(v), Some(t)) => {
                            out.teleports.push((v.player_id, t.position, t.yaw, t.pitch));
                            out.feedback.push(format!("Teleported {} to {}.", v.username, t.username));
                        }
                        (None, _) => {
                            return CommandOutput::error(format!("Player '{}' is not online.", victim_name));
                        }
                        (_, None) => {
                            return CommandOutput::error(format!("Target '{}' is not online.", target_name));
                        }
                    }
                }
                // /tp <x> <y> <z> (caller to coords)
                3 => {
                    let caller_id = match source {
                        CommandSource::Player { player_id, .. } => *player_id,
                        CommandSource::Console => {
                            return CommandOutput::error(
                                "Console must specify: /tp <player> <x> <y> <z>",
                            );
                        }
                    };
                    let x: f32 = match args[0].parse() {
                        Ok(v) => v,
                        Err(_) => return CommandOutput::error("Invalid X coordinate."),
                    };
                    let y: f32 = match args[1].parse() {
                        Ok(v) => v,
                        Err(_) => return CommandOutput::error("Invalid Y coordinate."),
                    };
                    let z: f32 = match args[2].parse() {
                        Ok(v) => v,
                        Err(_) => return CommandOutput::error("Invalid Z coordinate."),
                    };
                    let target_pos = Vec3::new(x, y, z);
                    let (yaw, pitch) = online_players
                        .iter()
                        .find(|p| p.player_id == caller_id)
                        .map(|p| (p.yaw, p.pitch))
                        .unwrap_or((0.0, 0.0));

                    out.teleports.push((caller_id, target_pos, yaw, pitch));
                    out.feedback.push(format!("Teleported to ({:.1}, {:.1}, {:.1}).", x, y, z));
                }
                // /tp <player> <x> <y> <z>
                4 => {
                    let victim_name = args[0];
                    let x: f32 = match args[1].parse() {
                        Ok(v) => v,
                        Err(_) => return CommandOutput::error("Invalid X coordinate."),
                    };
                    let y: f32 = match args[2].parse() {
                        Ok(v) => v,
                        Err(_) => return CommandOutput::error("Invalid Y coordinate."),
                    };
                    let z: f32 = match args[3].parse() {
                        Ok(v) => v,
                        Err(_) => return CommandOutput::error("Invalid Z coordinate."),
                    };

                    if let Some(victim) = online_players
                        .iter()
                        .find(|p| p.username.eq_ignore_ascii_case(victim_name))
                    {
                        out.teleports.push((victim.player_id, Vec3::new(x, y, z), victim.yaw, victim.pitch));
                        out.feedback.push(format!(
                            "Teleported {} to ({:.1}, {:.1}, {:.1}).",
                            victim.username, x, y, z
                        ));
                    } else {
                        return CommandOutput::error(format!("Player '{}' is not online.", victim_name));
                    }
                }
                _ => {
                    return CommandOutput::error(
                        "Usage: /tp <target> | /tp <player> <target> | /tp <x> <y> <z> | /tp <player> <x> <y> <z>",
                    );
                }
            }
        }

        // --- 5. TELEPORT HERE (TPHERE) COMMAND ---
        "tphere" => {
            if !check_perm("blockgame.command.tphere") {
                return CommandOutput::error("You do not have permission: blockgame.command.tphere");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /tphere <player>");
            }
            let (caller_pos, caller_yaw, caller_pitch) = match source {
                CommandSource::Player { player_id, .. } => {
                    if let Some(p) = online_players.iter().find(|pl| pl.player_id == *player_id) {
                        (p.position, p.yaw, p.pitch)
                    } else {
                        return CommandOutput::error("Could not determine your position.");
                    }
                }
                CommandSource::Console => {
                    return CommandOutput::error(
                        "Console cannot use /tphere without coordinates; use /tp <player> <x> <y> <z>",
                    );
                }
            };

            let victim_name = args[0];
            if let Some(victim) = online_players
                .iter()
                .find(|p| p.username.eq_ignore_ascii_case(victim_name))
            {
                out.teleports.push((victim.player_id, caller_pos, caller_yaw, caller_pitch));
                out.feedback.push(format!("Teleported {} to you.", victim.username));
            } else {
                return CommandOutput::error(format!("Player '{}' is not online.", victim_name));
            }
        }

        // --- 6. GIVE COMMAND ---
        "give" => {
            if !check_perm("blockgame.command.give") {
                return CommandOutput::error("You do not have permission: blockgame.command.give");
            }
            if args.len() < 2 {
                return CommandOutput::error("Usage: /give <player> <item_id_or_name> [amount]");
            }
            let target_name = args[0];
            let item_str = args[1];
            let amount: u16 = if args.len() > 2 {
                args[2].parse().unwrap_or(1).clamp(1, 64)
            } else {
                1
            };

            let target = match online_players
                .iter()
                .find(|p| p.username.eq_ignore_ascii_case(target_name))
            {
                Some(t) => t,
                None => return CommandOutput::error(format!("Player '{}' is not online.", target_name)),
            };

            let (item_id, display_name) = match resolve_item(item_str, item_registry) {
                Some(res) => res,
                None => return CommandOutput::error(format!("Unknown item '{}'.", item_str)),
            };

            let slot = ItemSlot::new(item_id, amount);
            out.gives.push((target.player_id, slot));
            out.feedback.push(format!(
                "Gave {}x {} to {}.",
                amount, display_name, target.username
            ));
        }

        // --- 7. PERMISSIONS (PERM) COMMAND ---
        "perm" | "permission" | "permissions" => {
            if !check_perm("blockgame.command.perm") {
                return CommandOutput::error("You do not have permission: blockgame.command.perm");
            }

            if args.is_empty() {
                return CommandOutput::error(
                    "Usage: /perm <user|group|list> ... (Type /perm help for all subcommands)",
                );
            }

            match args[0].to_ascii_lowercase().as_str() {
                "list" => {
                    let mut group_list: Vec<_> = perms.groups.keys().cloned().collect();
                    group_list.sort();
                    out.feedback.push(format!("Registered groups: {}", group_list.join(", ")));
                }
                "group" => {
                    if args.len() < 3 {
                        return CommandOutput::error(
                            "Usage: /perm group <name> <create|delete|add|remove|list> [permission]",
                        );
                    }
                    let group_name = args[1];
                    let action = args[2].to_ascii_lowercase();

                    match action.as_str() {
                        "create" => match perms.create_group(group_name) {
                            Ok(()) => {
                                out.save_permissions = true;
                                out.feedback.push(format!("Created group '{}'.", group_name));
                            }
                            Err(e) => return CommandOutput::error(e),
                        },
                        "delete" => match perms.delete_group(group_name) {
                            Ok(()) => {
                                out.save_permissions = true;
                                out.feedback.push(format!("Deleted group '{}'.", group_name));
                            }
                            Err(e) => return CommandOutput::error(e),
                        },
                        "add" => {
                            if args.len() < 4 {
                                return CommandOutput::error(
                                    "Usage: /perm group <name> add <permission>",
                                );
                            }
                            let perm_node = args[3];
                            match perms.add_group_permission(group_name, perm_node) {
                                Ok(()) => {
                                    out.save_permissions = true;
                                    out.feedback.push(format!(
                                        "Added permission '{}' to group '{}'.",
                                        perm_node, group_name
                                    ));
                                }
                                Err(e) => return CommandOutput::error(e),
                            }
                        }
                        "remove" => {
                            if args.len() < 4 {
                                return CommandOutput::error(
                                    "Usage: /perm group <name> remove <permission>",
                                );
                            }
                            let perm_node = args[3];
                            match perms.remove_group_permission(group_name, perm_node) {
                                Ok(()) => {
                                    out.save_permissions = true;
                                    out.feedback.push(format!(
                                        "Removed permission '{}' from group '{}'.",
                                        perm_node, group_name
                                    ));
                                }
                                Err(e) => return CommandOutput::error(e),
                            }
                        }
                        "prefix" => {
                            if args.len() < 4 {
                                return CommandOutput::error(
                                    "Usage: /perm group <name> prefix <prefix|clear>",
                                );
                            }
                            let raw = args[3..].join(" ");
                            let prefix = if raw.eq_ignore_ascii_case("clear")
                                || raw.eq_ignore_ascii_case("none")
                            {
                                String::new()
                            } else if raw.ends_with(' ') {
                                raw
                            } else {
                                format!("{} ", raw)
                            };
                            match perms.set_group_prefix(group_name, &prefix) {
                                Ok(()) => {
                                    out.save_permissions = true;
                                    if prefix.is_empty() {
                                        out.feedback.push(format!(
                                            "Cleared prefix for group '{}'.",
                                            group_name
                                        ));
                                    } else {
                                        out.feedback.push(format!(
                                            "Set prefix for group '{}' to '{}'.",
                                            group_name, prefix
                                        ));
                                    }
                                }
                                Err(e) => return CommandOutput::error(e),
                            }
                        }
                        "priority" => {
                            if args.len() < 4 {
                                return CommandOutput::error(
                                    "Usage: /perm group <name> priority <number>",
                                );
                            }
                            let Ok(prio) = args[3].parse::<i32>() else {
                                return CommandOutput::error(
                                    "Priority must be an integer (e.g. 0, 50, 100).",
                                );
                            };
                            match perms.set_group_priority(group_name, prio) {
                                Ok(()) => {
                                    out.save_permissions = true;
                                    out.feedback.push(format!(
                                        "Set priority for group '{}' to {}.",
                                        group_name, prio
                                    ));
                                }
                                Err(e) => return CommandOutput::error(e),
                            }
                        }
                        "list" => {
                            if let Some(group) = perms.groups.get(&group_name.to_ascii_lowercase()) {
                                out.feedback.push(format!(
                                    "Group '{}' (priority: {}, prefix: '{}') permissions: {}",
                                    group.name,
                                    group.priority,
                                    group.prefix,
                                    if group.permissions.is_empty() {
                                        "(none)".to_string()
                                    } else {
                                        group.permissions.join(", ")
                                    }
                                ));
                            } else {
                                return CommandOutput::error(format!("Group '{}' not found.", group_name));
                            }
                        }
                        _ => {
                            return CommandOutput::error(
                                "Unknown group action. Available: create, delete, add, remove, prefix, priority, list",
                            );
                        }
                    }
                }
                "user" | "player" => {
                    if args.len() < 3 {
                        return CommandOutput::error(
                            "Usage: /perm user <player> <group|add|remove|list> ...",
                        );
                    }
                    let username = args[1];
                    let action = args[2].to_ascii_lowercase();

                    match action.as_str() {
                        "group" => {
                            if args.len() < 4 {
                                return CommandOutput::error(
                                    "Usage: /perm user <player> group <add|remove|list> [group_name]",
                                );
                            }
                            let subaction = args[3].to_ascii_lowercase();
                            match subaction.as_str() {
                                "add" => {
                                    if args.len() < 5 {
                                        return CommandOutput::error(
                                            "Usage: /perm user <player> group add <group_name>",
                                        );
                                    }
                                    let group_name = args[4];
                                    match perms.add_player_group(username, group_name) {
                                        Ok(()) => {
                                            out.save_permissions = true;
                                            out.feedback.push(format!(
                                                "Added player '{}' to group '{}'.",
                                                username, group_name
                                            ));
                                        }
                                        Err(e) => return CommandOutput::error(e),
                                    }
                                }
                                "remove" => {
                                    if args.len() < 5 {
                                        return CommandOutput::error(
                                            "Usage: /perm user <player> group remove <group_name>",
                                        );
                                    }
                                    let group_name = args[4];
                                    match perms.remove_player_group(username, group_name) {
                                        Ok(()) => {
                                            out.save_permissions = true;
                                            out.feedback.push(format!(
                                                "Removed player '{}' from group '{}'.",
                                                username, group_name
                                            ));
                                        }
                                        Err(e) => return CommandOutput::error(e),
                                    }
                                }
                                "list" => {
                                    let groups = perms.get_player_groups(username);
                                    out.feedback.push(format!(
                                        "Player '{}' belongs to groups: {}",
                                        username,
                                        groups.join(", ")
                                    ));
                                }
                                _ => {
                                    return CommandOutput::error("Usage: /perm user <player> group <add|remove|list> [group]");
                                }
                            }
                        }
                        "add" => {
                            if args.len() < 4 {
                                return CommandOutput::error(
                                    "Usage: /perm user <player> add <permission>",
                                );
                            }
                            let perm_node = args[3];
                            match perms.add_player_permission(username, perm_node) {
                                Ok(()) => {
                                    out.save_permissions = true;
                                    out.feedback.push(format!(
                                        "Added direct permission '{}' to player '{}'.",
                                        perm_node, username
                                    ));
                                }
                                Err(e) => return CommandOutput::error(e),
                            }
                        }
                        "remove" => {
                            if args.len() < 4 {
                                return CommandOutput::error(
                                    "Usage: /perm user <player> remove <permission>",
                                );
                            }
                            let perm_node = args[3];
                            match perms.remove_player_permission(username, perm_node) {
                                Ok(()) => {
                                    out.save_permissions = true;
                                    out.feedback.push(format!(
                                        "Removed permission '{}' from player '{}'.",
                                        perm_node, username
                                    ));
                                }
                                Err(e) => return CommandOutput::error(e),
                            }
                        }
                        "list" => {
                            let groups = perms.get_player_groups(username);
                            let direct = perms.get_player_permissions(username);
                            out.feedback.push(format!(
                                "Player '{}': Groups: [{}], Direct Permissions: [{}]",
                                username,
                                groups.join(", "),
                                if direct.is_empty() {
                                    "none".to_string()
                                } else {
                                    direct.join(", ")
                                }
                            ));
                        }
                        _ => {
                            return CommandOutput::error(
                                "Unknown user action. Available: group, add, remove, list",
                            );
                        }
                    }
                }
                "help" => {
                    out.feedback.push("Permissions commands:".to_string());
                    out.feedback.push("  /perm list - List all groups".to_string());
                    out.feedback.push("  /perm group <group> create/delete - Manage group".to_string());
                    out.feedback.push("  /perm group <group> add/remove/list <perm> - Manage group perms".to_string());
                    out.feedback.push("  /perm group <group> prefix <text|clear> - Set group chat prefix".to_string());
                    out.feedback.push("  /perm group <group> priority <num> - Set group prefix priority".to_string());
                    out.feedback.push("  /perm user <player> group add/remove/list <group> - Player groups".to_string());
                    out.feedback.push("  /perm user <player> add/remove/list <perm> - Direct player perms".to_string());
                }
                _ => {
                    return CommandOutput::error(
                        "Unknown subcommand. Use: /perm <user|group|list|help>",
                    );
                }
            }
        }

        // --- 8. LIST COMMAND ---
        "list" => {
            let names: Vec<&str> = online_players.iter().map(|p| p.username.as_str()).collect();
            out.feedback.push(format!(
                "Connected players ({}): {}",
                online_players.len(),
                if names.is_empty() {
                    "None".to_string()
                } else {
                    names.join(", ")
                }
            ));
        }

        // --- 9. SAY COMMAND ---
        "say" => {
            if !check_perm("blockgame.command.say") {
                return CommandOutput::error("You do not have permission: blockgame.command.say");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /say <message>");
            }
            let msg = args.join(" ");
            out.broadcasts.push(format!("[{}] {}", source.username(), msg));
        }

        // --- 10. STOP COMMAND ---
        "stop" => {
            if !check_perm("blockgame.command.stop") {
                return CommandOutput::error("You do not have permission: blockgame.command.stop");
            }
            out.stop_server = true;
            out.broadcasts.push("[Server] Server is shutting down...".to_string());
            out.feedback.push("Shutting down server...".to_string());
        }

        // --- 11. RANK COMMAND ---
        "rank" => {
            if !check_perm("blockgame.command.rank") {
                return CommandOutput::error("You do not have permission: blockgame.command.rank");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /rank <player> [new_rank]");
            }
            let target_name = args[0];
            if args.len() == 1 {
                let groups = perms.get_player_groups(target_name);
                let display_rank = groups.first().map(|s| s.as_str()).unwrap_or("default");
                out.feedback.push(format!(
                    "Player '{}' rank: '{}' (Groups: {})",
                    target_name,
                    display_rank,
                    groups.join(", ")
                ));
            } else {
                let new_rank = args[1].to_ascii_lowercase();
                if !perms.groups.contains_key(&new_rank) {
                    let _ = perms.create_group(&new_rank);
                }
                let _ = perms.add_player_group(target_name, &new_rank);
                out.save_permissions = true;
                out.rank_updates.push((target_name.to_string(), new_rank.clone()));
                out.feedback.push(format!(
                    "Set rank of player '{}' to '{}'.",
                    target_name, new_rank
                ));
                out.broadcasts.push(format!(
                    "[Server] Player '{}' is now rank '{}'.",
                    target_name, new_rank
                ));
            }
        }

        // --- 12. RESETPLAYER COMMAND ---
        "resetplayer" => {
            if !check_perm("blockgame.command.resetplayer") {
                return CommandOutput::error("You do not have permission: blockgame.command.resetplayer");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /resetplayer <player>");
            }
            let target_name = args[0];
            out.reset_players.push(target_name.to_string());
            out.feedback.push(format!(
                "Reset player data for '{}'. Position, inventory, and stats restored to defaults.",
                target_name
            ));
            out.broadcasts.push(format!(
                "[Server] Player data for '{}' was reset by an administrator.",
                target_name
            ));
        }

        // --- 13. SPAWN COMMANDS ---
        "gamemode" | "gm" => {
            if !check_perm("blockgame.command.gamemode") {
                return CommandOutput::error("You don't have permission to use /gamemode.");
            }
            if args.is_empty() {
                return CommandOutput::error(
                    "Usage: /gamemode <survival|creative|0|1> [player]",
                );
            }
            let Some(mode) = crate::player::GameMode::parse(args[0]) else {
                return CommandOutput::error(format!(
                    "Unknown game mode '{}'. Use survival (0) or creative (1).",
                    args[0]
                ));
            };
            let target = if args.len() >= 2 {
                let target_name = args[1];
                match online_players
                    .iter()
                    .find(|p| p.username.eq_ignore_ascii_case(target_name))
                {
                    Some(p) => p,
                    None => {
                        return CommandOutput::error(format!(
                            "Player '{}' is not online.",
                            target_name
                        ))
                    }
                }
            } else if let Some(pid) = source.player_id() {
                match online_players.iter().find(|p| p.player_id == pid) {
                    Some(p) => p,
                    None => return CommandOutput::error("Sender not found online."),
                }
            } else {
                return CommandOutput::error(
                    "Console must specify a player: /gamemode <survival|creative> <player>",
                );
            };

            let mut out = CommandOutput::default();
            out.gamemode_updates.push((target.player_id, mode));
            out.cheat_taint_players.push(target.player_id);
            let msg = format!(
                "Set {}'s game mode to {}.",
                target.username,
                mode.display_name()
            );
            out.feedback.push(msg);
            if source.player_id() != Some(target.player_id) {
                out.direct_messages.push((
                    target.player_id,
                    format!("Your game mode was changed to {}.", mode.display_name()),
                ));
            }
            return out;
        }
        "setspawn" => {
            if !check_perm("blockgame.command.setspawn") {
                return CommandOutput::error("You do not have permission: blockgame.command.setspawn");
            }
            let spawn_pos = if args.len() >= 3 {
                let x = args[0].parse::<f32>();
                let y = args[1].parse::<f32>();
                let z = args[2].parse::<f32>();
                match (x, y, z) {
                    (Ok(x), Ok(y), Ok(z)) => Vec3::new(x, y, z),
                    _ => return CommandOutput::error("Usage: /setspawn [x y z]"),
                }
            } else {
                match source {
                    CommandSource::Player { player_id, .. } => {
                        if let Some(p) = online_players.iter().find(|p| p.player_id == *player_id) {
                            p.position
                        } else {
                            default_spawn
                        }
                    }
                    CommandSource::Console => {
                        return CommandOutput::error("Console must provide coordinates: /setspawn <x> <y> <z>");
                    }
                }
            };
            out.new_spawn = Some(spawn_pos);
            out.feedback.push(format!("[Server] World spawn set to ({:.1}, {:.1}, {:.1}).", spawn_pos.x, spawn_pos.y, spawn_pos.z));
            out.broadcasts.push(format!(
                "[Server] World spawn set to ({:.1}, {:.1}, {:.1}) by {}. [Spawn]",
                spawn_pos.x, spawn_pos.y, spawn_pos.z, source.username()
            ));
        }

        "spawn" => {
            if !check_perm("blockgame.command.spawn") {
                return CommandOutput::error("You do not have permission: blockgame.command.spawn");
            }
            let player_id = match source {
                CommandSource::Player { player_id, .. } => *player_id,
                CommandSource::Console => {
                    return CommandOutput::error("Only players can execute /spawn");
                }
            };
            out.teleports.push((player_id, default_spawn, 0.0, 0.0));
            out.feedback.push(format!(
                "[Teleport] Teleported to world spawn ({:.1}, {:.1}, {:.1}).",
                default_spawn.x, default_spawn.y, default_spawn.z
            ));
        }

        // --- 14. HOME COMMANDS ---
        "sethome" => {
            if !check_perm("blockgame.command.home") {
                return CommandOutput::error("You do not have permission: blockgame.command.home");
            }
            let player_id = match source {
                CommandSource::Player { player_id, .. } => *player_id,
                CommandSource::Console => {
                    return CommandOutput::error("Only players can execute /sethome");
                }
            };
            let home_name = args.first().copied().unwrap_or("home").to_ascii_lowercase();
            if home_name.len() > 16 || !home_name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                return CommandOutput::error("Home name must be 1-16 alphanumeric characters.");
            }
            if let Some(p) = online_players.iter().find(|p| p.player_id == player_id) {
                if p.homes.len() >= 5 && !p.homes.iter().any(|(k, _)| k.to_ascii_lowercase() == home_name) {
                    return CommandOutput::error("You have reached the maximum of 5 homes. Use /delhome <name> to delete one.");
                }
                out.set_home = Some((player_id, home_name.clone(), p.position, p.yaw, p.pitch));
                out.feedback.push(format!(
                    "[Home] Home '{}' set at ({:.1}, {:.1}, {:.1}).",
                    home_name, p.position.x, p.position.y, p.position.z
                ));
            } else {
                return CommandOutput::error("Player not found online.");
            }
        }

        "home" => {
            if !check_perm("blockgame.command.home") {
                return CommandOutput::error("You do not have permission: blockgame.command.home");
            }
            let player_id = match source {
                CommandSource::Player { player_id, .. } => *player_id,
                CommandSource::Console => {
                    return CommandOutput::error("Only players can execute /home");
                }
            };
            let home_name = args.first().copied().unwrap_or("home").to_ascii_lowercase();
            if let Some(p) = online_players.iter().find(|p| p.player_id == player_id) {
                if let Some((_, (pos, yaw, pitch))) = p.homes.iter().find(|(k, _)| k.to_ascii_lowercase() == home_name) {
                    out.teleports.push((player_id, *pos, *yaw, *pitch));
                    out.feedback.push(format!("[Home] Teleported to home '{}'.", home_name));
                } else {
                    out.feedback.push(format!(
                        "[Error] Home '{}' not found. Use /homes to view your saved homes.",
                        home_name
                    ));
                }
            } else {
                return CommandOutput::error("Player not found online.");
            }
        }

        "delhome" => {
            if !check_perm("blockgame.command.home") {
                return CommandOutput::error("You do not have permission: blockgame.command.home");
            }
            let player_id = match source {
                CommandSource::Player { player_id, .. } => *player_id,
                CommandSource::Console => {
                    return CommandOutput::error("Only players can execute /delhome");
                }
            };
            if args.is_empty() {
                return CommandOutput::error("Usage: /delhome <name>");
            }
            let home_name = args[0].to_ascii_lowercase();
            if let Some(p) = online_players.iter().find(|p| p.player_id == player_id) {
                if p.homes.iter().any(|(k, _)| k.to_ascii_lowercase() == home_name) {
                    out.del_home = Some((player_id, home_name.clone()));
                    out.feedback.push(format!("[Home] Home '{}' deleted.", home_name));
                } else {
                    out.feedback.push(format!("[Error] Home '{}' not found.", home_name));
                }
            } else {
                return CommandOutput::error("Player not found online.");
            }
        }

        "homes" => {
            if !check_perm("blockgame.command.home") {
                return CommandOutput::error("You do not have permission: blockgame.command.home");
            }
            let player_id = match source {
                CommandSource::Player { player_id, .. } => *player_id,
                CommandSource::Console => {
                    return CommandOutput::error("Only players can execute /homes");
                }
            };
            if let Some(p) = online_players.iter().find(|p| p.player_id == player_id) {
                if p.homes.is_empty() {
                    out.feedback.push("[Homes] You have no saved homes. Use /sethome [name] to save a home.".to_string());
                } else {
                    let mut names: Vec<String> = p.homes.keys().cloned().collect();
                    names.sort();
                    let formatted = names.into_iter().map(|n| format!("[/home {}]", n)).collect::<Vec<_>>().join(" ");
                    out.feedback.push(format!("[Homes] ({}/5): {}", p.homes.len(), formatted));
                }
            } else {
                return CommandOutput::error("Player not found online.");
            }
        }

        // --- 15. TPA COMMANDS ---
        "tpa" => {
            if !check_perm("blockgame.command.tpa") {
                return CommandOutput::error("You do not have permission: blockgame.command.tpa");
            }
            let player_id = match source {
                CommandSource::Player { player_id, .. } => *player_id,
                CommandSource::Console => {
                    return CommandOutput::error("Only players can execute /tpa");
                }
            };
            if args.is_empty() {
                return CommandOutput::error("Usage: /tpa <player> (or /tpa accept | /tpa deny)");
            }
            if args[0].eq_ignore_ascii_case("accept") {
                return execute_command(source, "tpaccept", online_players, perms, bans, item_registry, default_spawn, tpa_requests);
            }
            if args[0].eq_ignore_ascii_case("deny") {
                return execute_command(source, "tpdeny", online_players, perms, bans, item_registry, default_spawn, tpa_requests);
            }
            let target_name = args[0];
            let source_username = source.username().to_string();
            if let Some(target) = online_players.iter().find(|p| p.username.eq_ignore_ascii_case(target_name)) {
                if target.player_id == player_id {
                    return CommandOutput::error("You cannot send a teleport request to yourself.");
                }
                tpa_requests.insert(target.player_id, PendingTpaRequest {
                    requester_id: player_id,
                    requester_name: source_username.clone(),
                    created_at: Instant::now(),
                });
                out.feedback.push(format!("[TPA] Teleport request sent to {}. It expires in 60 seconds.", target.username));
                out.direct_messages.push((
                    target.player_id,
                    format!("[TPA] <{}> has requested to teleport to you. [Accept] [Deny] or /tpaccept / /tpdeny", source_username),
                ));
            } else {
                return CommandOutput::error(format!("Player '{}' is not online.", target_name));
            }
        }

        "tpaccept" => {
            if !check_perm("blockgame.command.tpa") {
                return CommandOutput::error("You do not have permission: blockgame.command.tpa");
            }
            let player_id = match source {
                CommandSource::Player { player_id, .. } => *player_id,
                CommandSource::Console => {
                    return CommandOutput::error("Only players can execute /tpaccept");
                }
            };
            if let Some(req) = tpa_requests.remove(&player_id) {
                if req.created_at.elapsed() > std::time::Duration::from_secs(60) {
                    out.feedback.push("[TPA] The teleport request has expired.".to_string());
                } else if let Some(requester) = online_players.iter().find(|p| p.player_id == req.requester_id) {
                    if let Some(target) = online_players.iter().find(|p| p.player_id == player_id) {
                        out.teleports.push((requester.player_id, target.position, target.yaw, target.pitch));
                        out.feedback.push(format!("[TPA] Accepted teleport request from {}.", req.requester_name));
                        out.direct_messages.push((requester.player_id, format!("[TPA] Teleporting to {}...", target.username)));
                    }
                } else {
                    out.feedback.push(format!("[TPA] Player '{}' is no longer online.", req.requester_name));
                }
            } else {
                out.feedback.push("[TPA] You have no pending teleport requests.".to_string());
            }
        }

        "tpdeny" => {
            if !check_perm("blockgame.command.tpa") {
                return CommandOutput::error("You do not have permission: blockgame.command.tpa");
            }
            let _player_id = match source {
                CommandSource::Player { player_id, .. } => *player_id,
                CommandSource::Console => {
                    return CommandOutput::error("Only players can execute /tpdeny");
                }
            };
            if let Some(req) = tpa_requests.remove(&_player_id) {
                out.feedback.push(format!("[TPA] Denied teleport request from {}.", req.requester_name));
                out.direct_messages.push((req.requester_id, format!("[TPA] {} denied your teleport request.", source.username())));
            } else {
                out.feedback.push("[TPA] You have no pending teleport requests.".to_string());
            }
        }

        // --- 15b. SKILLS COMMAND ---
        "skills" | "skill" => {
            if args.is_empty() {
                if !check_perm("blockgame.command.skills") {
                    return CommandOutput::error("You do not have permission: blockgame.command.skills");
                }
                let Some(pid) = source.player_id() else {
                    return CommandOutput::error(
                        "Console usage: /skills <set|addxp|reset> <player> ...",
                    );
                };
                out.skill_actions.push(SkillCommandAction::View {
                    requester_pid: pid,
                    skill: None,
                });
            } else if let Some(skill) = crate::skills::SkillKind::parse(args[0]) {
                if !check_perm("blockgame.command.skills") {
                    return CommandOutput::error("You do not have permission: blockgame.command.skills");
                }
                let Some(pid) = source.player_id() else {
                    return CommandOutput::error(
                        "Console usage: /skills <set|addxp|reset> <player> ...",
                    );
                };
                out.skill_actions.push(SkillCommandAction::View {
                    requester_pid: pid,
                    skill: Some(skill),
                });
            } else {
                match args[0].to_ascii_lowercase().as_str() {
                    "set" => {
                        if !check_perm("blockgame.command.skills.admin") {
                            return CommandOutput::error(
                                "You do not have permission: blockgame.command.skills.admin",
                            );
                        }
                        if args.len() < 4 {
                            return CommandOutput::error(
                                "Usage: /skills set <player> <skill> <level>",
                            );
                        }
                        let target_name = args[1];
                        let Some(target) = online_players
                            .iter()
                            .find(|p| p.username.eq_ignore_ascii_case(target_name))
                        else {
                            return CommandOutput::error(format!(
                                "Player '{}' is not online.",
                                target_name
                            ));
                        };
                        let Some(skill) = crate::skills::SkillKind::parse(args[2]) else {
                            return CommandOutput::error(format!(
                                "Unknown skill '{}'. Valid: combat, woodcutting, mining, fishing, digging, farming, agility",
                                args[2]
                            ));
                        };
                        let Ok(level) = args[3].parse::<u16>() else {
                            return CommandOutput::error(format!(
                                "Invalid level '{}'. Must be 1..={}.",
                                args[3],
                                crate::skills::MAX_SKILL_LEVEL
                            ));
                        };
                        let clamped = level.clamp(1, crate::skills::MAX_SKILL_LEVEL);
                        out.skill_actions.push(SkillCommandAction::Set {
                            target_pid: target.player_id,
                            skill,
                            level: clamped,
                        });
                        out.feedback.push(format!(
                            "Set {}'s {} level to {}.",
                            target.username,
                            skill.name(),
                            clamped
                        ));
                    }
                    "addxp" | "xp" => {
                        if !check_perm("blockgame.command.skills.admin") {
                            return CommandOutput::error(
                                "You do not have permission: blockgame.command.skills.admin",
                            );
                        }
                        if args.len() < 4 {
                            return CommandOutput::error(
                                "Usage: /skills addxp <player> <skill> <amount>",
                            );
                        }
                        let target_name = args[1];
                        let Some(target) = online_players
                            .iter()
                            .find(|p| p.username.eq_ignore_ascii_case(target_name))
                        else {
                            return CommandOutput::error(format!(
                                "Player '{}' is not online.",
                                target_name
                            ));
                        };
                        let Some(skill) = crate::skills::SkillKind::parse(args[2]) else {
                            return CommandOutput::error(format!(
                                "Unknown skill '{}'. Valid: combat, woodcutting, mining, fishing, digging, farming, agility",
                                args[2]
                            ));
                        };
                        let Ok(amount) = args[3].parse::<u64>() else {
                            return CommandOutput::error("XP amount must be a positive integer.");
                        };
                        if amount == 0 {
                            return CommandOutput::error("XP amount must be greater than 0.");
                        }
                        out.skill_actions.push(SkillCommandAction::AddXp {
                            target_pid: target.player_id,
                            skill,
                            amount,
                        });
                        out.feedback.push(format!(
                            "Granted {} {} XP to {}.",
                            amount,
                            skill.name(),
                            target.username
                        ));
                    }
                    "reset" => {
                        if !check_perm("blockgame.command.skills.admin") {
                            return CommandOutput::error(
                                "You do not have permission: blockgame.command.skills.admin",
                            );
                        }
                        if args.len() < 2 {
                            return CommandOutput::error("Usage: /skills reset <player>");
                        }
                        let target_name = args[1];
                        let Some(target) = online_players
                            .iter()
                            .find(|p| p.username.eq_ignore_ascii_case(target_name))
                        else {
                            return CommandOutput::error(format!(
                                "Player '{}' is not online.",
                                target_name
                            ));
                        };
                        out.skill_actions.push(SkillCommandAction::Reset {
                            target_pid: target.player_id,
                        });
                        out.feedback.push(format!(
                            "Reset all skills for {} to level 1.",
                            target.username
                        ));
                    }
                    _ => {
                        return CommandOutput::error(
                            "Usage: /skills [skill] or /skills <set|addxp|reset> <player> ...",
                        );
                    }
                }
            }
        }

        // --- 16. HELP COMMAND ---
        "help" | "?" => {
            out.feedback.push("Available Commands:".to_string());
            if check_perm("blockgame.command.kick") {
                out.feedback.push("  /kick <player> [reason] - Disconnect a player".to_string());
            }
            if check_perm("blockgame.command.ban") {
                out.feedback.push("  /ban <player> [reason] - Ban a player from the server".to_string());
                out.feedback.push("  /unban <player> - Remove ban from a player".to_string());
                out.feedback.push("  /ban-ip <player|ip> [reason] - Ban an IP address".to_string());
                out.feedback.push("  /unban-ip <ip> - Remove an IP ban".to_string());
                out.feedback.push("  /banlist [ips|players] - View banned players or IPs".to_string());
            }
            if check_perm("blockgame.command.tp") {
                out.feedback.push("  /tp <target> or /tp <player> <target> - Teleport players".to_string());
            }
            if check_perm("blockgame.command.tphere") {
                out.feedback.push("  /tphere <player> - Teleport player to your position".to_string());
            }
            if check_perm("blockgame.command.spawn") {
                out.feedback.push("  /spawn - Teleport to world spawn".to_string());
            }
            if check_perm("blockgame.command.setspawn") {
                out.feedback.push("  /setspawn [x y z] - Set world spawn point".to_string());
            }
            if check_perm("blockgame.command.home") {
                out.feedback.push("  /home [name] - Teleport to saved home".to_string());
                out.feedback.push("  /sethome [name] - Save home location".to_string());
                out.feedback.push("  /delhome <name> - Delete a saved home".to_string());
                out.feedback.push("  /homes - List your saved homes".to_string());
            }
            if check_perm("blockgame.command.tpa") {
                out.feedback.push("  /tpa <player> - Request teleport to player".to_string());
                out.feedback.push("  /tpaccept - Accept incoming teleport request".to_string());
                out.feedback.push("  /tpdeny - Deny incoming teleport request".to_string());
            }
            if check_perm("blockgame.command.skills") {
                out.feedback.push("  /skills [skill] - View your MMO skill levels & milestones".to_string());
            }
            if check_perm("blockgame.command.skills.admin") {
                out.feedback.push("  /skills <set|addxp|reset> <player> ... - Manage player skills".to_string());
            }
            if check_perm("blockgame.command.give") {
                out.feedback.push("  /give <player> <item> [amount] - Give items to player".to_string());
            }
            if check_perm("blockgame.command.perm") {
                out.feedback.push("  /perm <user|group|list> - Manage permissions and groups".to_string());
            }
            if check_perm("blockgame.command.rank") {
                out.feedback.push("  /rank <player> [rank] - View or set player rank".to_string());
            }
            if check_perm("blockgame.command.resetplayer") {
                out.feedback.push("  /resetplayer <player> - Reset player data to defaults".to_string());
            }
            out.feedback.push("  /list - List all online players".to_string());
            if check_perm("blockgame.command.say") {
                out.feedback.push("  /say <message> - Broadcast announcement".to_string());
            }
            if check_perm("blockgame.command.automessage") {
                out.feedback.push("  /automessage <on|off|interval|order|list|add|remove> - Manage Auto-Messages".to_string());
            }
            if check_perm("blockgame.command.stop") {
                out.feedback.push("  /stop - Shut down server".to_string());
            }
        }

        "automessage" | "automessages" | "am" => {
            if !check_perm("blockgame.command.automessage") {
                return CommandOutput::error(
                    "You do not have permission: blockgame.command.automessage",
                );
            }
            let sub = args.first().map(|s| s.to_ascii_lowercase());
            match sub.as_deref() {
                None | Some("status") => {
                    out.automessage_actions.push(AutoMessageCommandAction::Status);
                }
                Some("on") | Some("enable") => {
                    out.automessage_actions
                        .push(AutoMessageCommandAction::SetEnabled(true));
                }
                Some("off") | Some("disable") => {
                    out.automessage_actions
                        .push(AutoMessageCommandAction::SetEnabled(false));
                }
                Some("interval") => {
                    let Some(raw) = args.get(1) else {
                        return CommandOutput::error("Usage: /automessage interval <seconds>");
                    };
                    let Ok(secs) = raw.parse::<u64>() else {
                        return CommandOutput::error(format!("Invalid interval '{}'.", raw));
                    };
                    out.automessage_actions
                        .push(AutoMessageCommandAction::SetInterval(secs));
                }
                Some("order") => {
                    let Some(raw) = args.get(1) else {
                        return CommandOutput::error(
                            "Usage: /automessage order <sequential|random>",
                        );
                    };
                    let Some(order) = crate::automessage::AutoMessageOrder::parse(raw) else {
                        return CommandOutput::error(
                            "Order must be 'sequential' or 'random'.",
                        );
                    };
                    out.automessage_actions
                        .push(AutoMessageCommandAction::SetOrder(order));
                }
                Some("list") => {
                    out.automessage_actions.push(AutoMessageCommandAction::List);
                }
                Some("add") => {
                    if args.len() < 2 {
                        return CommandOutput::error("Usage: /automessage add <message>");
                    }
                    let msg = args[1..].join(" ");
                    out.automessage_actions
                        .push(AutoMessageCommandAction::Add(msg));
                }
                Some("remove") | Some("del") | Some("delete") => {
                    let Some(raw) = args.get(1) else {
                        return CommandOutput::error("Usage: /automessage remove <index>");
                    };
                    let Ok(idx) = raw.parse::<usize>() else {
                        return CommandOutput::error(format!("Invalid index '{}'.", raw));
                    };
                    out.automessage_actions
                        .push(AutoMessageCommandAction::Remove(idx));
                }
                _ => {
                    return CommandOutput::error(
                        "Usage: /automessage <status|on|off|interval <sec>|order <sequential|random>|list|add <msg>|remove <idx>>",
                    );
                }
            }
        }

        "mute" => {
            if !check_perm("blockgame.command.mute") {
                return CommandOutput::error("You do not have permission: blockgame.command.mute");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /mute <player> [duration] [reason]");
            }
            let target = args[0].to_string();
            let (duration_secs, reason) = if args.len() >= 2 {
                if let Some(secs) = crate::moderation::parse_duration_secs(args[1]) {
                    let r = if args.len() > 2 {
                        args[2..].join(" ")
                    } else {
                        "Muted by staff".to_string()
                    };
                    (Some(secs), r)
                } else {
                    (None, args[1..].join(" "))
                }
            } else {
                (None, "Muted by staff".to_string())
            };
            out.moderation_actions.push(ModerationCommandAction::Mute {
                target,
                duration_secs,
                reason,
            });
        }

        "unmute" => {
            if !check_perm("blockgame.command.unmute") {
                return CommandOutput::error("You do not have permission: blockgame.command.unmute");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /unmute <player>");
            }
            out.moderation_actions.push(ModerationCommandAction::Unmute {
                target: args[0].to_string(),
            });
        }

        "warn" => {
            if !check_perm("blockgame.command.warn") {
                return CommandOutput::error("You do not have permission: blockgame.command.warn");
            }
            if args.len() < 2 {
                return CommandOutput::error("Usage: /warn <player> <reason>");
            }
            out.moderation_actions.push(ModerationCommandAction::Warn {
                target: args[0].to_string(),
                reason: args[1..].join(" "),
            });
        }

        "warnings" => {
            if !check_perm("blockgame.command.warnings") {
                return CommandOutput::error("You do not have permission: blockgame.command.warnings");
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /warnings <player>");
            }
            out.moderation_actions.push(ModerationCommandAction::Warnings {
                target: args[0].to_string(),
            });
        }

        "clearwarnings" => {
            if !check_perm("blockgame.command.clearwarnings") {
                return CommandOutput::error(
                    "You do not have permission: blockgame.command.clearwarnings",
                );
            }
            if args.is_empty() {
                return CommandOutput::error("Usage: /clearwarnings <player>");
            }
            out.moderation_actions
                .push(ModerationCommandAction::ClearWarnings {
                    target: args[0].to_string(),
                });
        }

        "wordfilter" | "wf" => {
            if !check_perm("blockgame.command.wordfilter") {
                return CommandOutput::error(
                    "You do not have permission: blockgame.command.wordfilter",
                );
            }
            let sub = args.first().map(|s| s.to_ascii_lowercase());
            match sub.as_deref() {
                None | Some("status") => out
                    .moderation_actions
                    .push(ModerationCommandAction::WordFilterStatus),
                Some("on") | Some("enable") => out
                    .moderation_actions
                    .push(ModerationCommandAction::WordFilterSetEnabled(true)),
                Some("off") | Some("disable") => out
                    .moderation_actions
                    .push(ModerationCommandAction::WordFilterSetEnabled(false)),
                Some("mode") => {
                    let Some(raw) = args.get(1) else {
                        return CommandOutput::error("Usage: /wordfilter mode <censor|block>");
                    };
                    let Some(mode) = crate::moderation::WordFilterMode::parse(raw) else {
                        return CommandOutput::error("Mode must be 'censor' or 'block'.");
                    };
                    out.moderation_actions
                        .push(ModerationCommandAction::WordFilterSetMode(mode));
                }
                Some("list") => out
                    .moderation_actions
                    .push(ModerationCommandAction::WordFilterList),
                Some("add") => {
                    let Some(word) = args.get(1) else {
                        return CommandOutput::error("Usage: /wordfilter add <word>");
                    };
                    out.moderation_actions
                        .push(ModerationCommandAction::WordFilterAdd((*word).to_string()));
                }
                Some("remove") | Some("del") => {
                    let Some(word) = args.get(1) else {
                        return CommandOutput::error("Usage: /wordfilter remove <word>");
                    };
                    out.moderation_actions
                        .push(ModerationCommandAction::WordFilterRemove(
                            (*word).to_string(),
                        ));
                }
                _ => {
                    return CommandOutput::error(
                        "Usage: /wordfilter <status|on|off|mode <censor|block>|list|add <word>|remove <word>>",
                    );
                }
            }
        }

        "chatlock" => {
            if !check_perm("blockgame.command.chatlock") {
                return CommandOutput::error(
                    "You do not have permission: blockgame.command.chatlock",
                );
            }
            let state = match args.first().map(|s| s.to_ascii_lowercase()).as_deref() {
                None => None,
                Some("on") | Some("lock") | Some("true") => Some(true),
                Some("off") | Some("unlock") | Some("false") => Some(false),
                _ => return CommandOutput::error("Usage: /chatlock [on|off]"),
            };
            out.moderation_actions
                .push(ModerationCommandAction::ChatLockToggle(state));
        }

        "slowmode" => {
            if !check_perm("blockgame.command.slowmode") {
                return CommandOutput::error(
                    "You do not have permission: blockgame.command.slowmode",
                );
            }
            let Some(raw) = args.first() else {
                return CommandOutput::error("Usage: /slowmode <seconds>");
            };
            let secs = if raw.eq_ignore_ascii_case("off") {
                0
            } else {
                match raw.parse::<u64>() {
                    Ok(v) => v,
                    Err(_) => return CommandOutput::error(format!("Invalid seconds '{}'.", raw)),
                }
            };
            out.moderation_actions
                .push(ModerationCommandAction::SlowmodeSet(secs));
        }

        "clearchat" | "cc" => {
            if !check_perm("blockgame.command.clearchat") {
                return CommandOutput::error(
                    "You do not have permission: blockgame.command.clearchat",
                );
            }
            out.moderation_actions
                .push(ModerationCommandAction::ClearChat);
        }

        "admin" => {
            if source.is_console() {
                return CommandOutput::error("The Admin Panel can only be opened by an in-game player.");
            }
            let tabs = crate::admin_panel::visible_tabs_for_player(perms, source.username());
            if tabs.is_empty() {
                return CommandOutput::error("You do not have permission to open the Admin Panel.");
            }
            out.open_admin_panel = Some(None);
        }

        "perms" | "permui" => {
            if source.is_console() {
                return CommandOutput::error("The Permissions UI can only be opened by an in-game player.");
            }
            let tabs = crate::admin_panel::visible_tabs_for_player(perms, source.username());
            if !tabs.contains(&crate::admin_panel::AdminTab::Permissions) {
                return CommandOutput::error(
                    "You do not have permission to manage Groups & Permissions.",
                );
            }
            out.open_admin_panel = Some(Some(crate::admin_panel::AdminTab::Permissions));
        }

        "claim" => {
            if source.is_console() {
                return CommandOutput::error("/claim can only be run by an in-game player.");
            }
            let sub = args.first().map(|s| s.to_ascii_lowercase());
            match sub.as_deref() {
                None => out.claim_actions.push(ClaimCommandAction::ClaimCurrent),
                Some("info") => out.claim_actions.push(ClaimCommandAction::Info),
                Some("list") => out.claim_actions.push(ClaimCommandAction::List),
                Some("map") => out.claim_actions.push(ClaimCommandAction::Map),
                Some("borders") | Some("border") => {
                    out.claim_actions.push(ClaimCommandAction::ToggleBorders);
                }
                Some("trust") => {
                    let Some(friend) = args.get(1) else {
                        return CommandOutput::error("Usage: /claim trust <player>");
                    };
                    out.claim_actions
                        .push(ClaimCommandAction::Trust((*friend).to_string()));
                }
                Some("untrust") => {
                    let Some(friend) = args.get(1) else {
                        return CommandOutput::error("Usage: /claim untrust <player>");
                    };
                    out.claim_actions
                        .push(ClaimCommandAction::Untrust((*friend).to_string()));
                }
                Some("trustlist") => out.claim_actions.push(ClaimCommandAction::TrustList),
                Some("admin") => {
                    if !check_perm("blockgame.claim.admin") {
                        return CommandOutput::error(
                            "You do not have permission: blockgame.claim.admin",
                        );
                    }
                    let label = if args.len() > 1 {
                        Some(args[1..].join(" "))
                    } else {
                        None
                    };
                    out.claim_actions.push(ClaimCommandAction::AdminClaim(label));
                }
                Some("adminbypass") | Some("bypass") => {
                    if !check_perm("blockgame.claim.bypass") {
                        return CommandOutput::error(
                            "You do not have permission: blockgame.claim.bypass",
                        );
                    }
                    out.claim_actions.push(ClaimCommandAction::AdminBypassToggle);
                }
                Some("adminunclaim") => {
                    if !check_perm("blockgame.claim.admin") && !check_perm("blockgame.claim.bypass") {
                        return CommandOutput::error(
                            "You do not have permission: blockgame.claim.admin",
                        );
                    }
                    out.claim_actions.push(ClaimCommandAction::AdminUnclaim);
                }
                _ => {
                    return CommandOutput::error(
                        "Usage: /claim [info|list|map|borders|trust <p>|untrust <p>|trustlist|admin [label]|adminbypass|adminunclaim]",
                    );
                }
            }
        }

        "unclaim" => {
            if source.is_console() {
                return CommandOutput::error("/unclaim can only be run by an in-game player.");
            }
            out.claim_actions.push(ClaimCommandAction::UnclaimCurrent);
        }

        "unclaimall" => {
            if source.is_console() {
                return CommandOutput::error("/unclaimall can only be run by an in-game player.");
            }
            out.claim_actions.push(ClaimCommandAction::UnclaimAll);
        }

        "log" | "blocklog" | "rollback" => {
            let (sub, rest): (&str, &[&str]) = if cmd == "rollback" {
                ("rollback", args)
            } else {
                match args.first() {
                    Some(&s) => (s, &args[1..]),
                    None => {
                        return CommandOutput::error(
                            "Usage: /log <lookup|inspect|rollback|undo|purge> ...",
                        );
                    }
                }
            };
            match sub.to_ascii_lowercase().as_str() {
                "lookup" | "l" => {
                    if !check_perm("blockgame.log.lookup") {
                        return CommandOutput::error("You do not have permission: blockgame.log.lookup");
                    }
                    let mut player = None;
                    let mut time_secs = None;
                    let mut radius = None;
                    let mut action = None;
                    let mut page = 1usize;
                    for tok in rest {
                        if let Some((k, v)) = tok.split_once(':') {
                            match k.to_ascii_lowercase().as_str() {
                                "p" | "player" | "u" | "user" => player = Some(v.to_string()),
                                "t" | "time" => {
                                    let Some(secs) = crate::moderation::parse_duration_secs(v) else {
                                        return CommandOutput::error(format!("Invalid time '{}'.", v));
                                    };
                                    time_secs = Some(secs);
                                }
                                "r" | "radius" => {
                                    let Ok(r) = v.parse::<i32>() else {
                                        return CommandOutput::error(format!("Invalid radius '{}'.", v));
                                    };
                                    radius = Some(r.max(0));
                                }
                                "a" | "action" => action = Some(v.to_ascii_lowercase()),
                                "page" => page = v.parse::<usize>().unwrap_or(1).max(1),
                                _ => {}
                            }
                        }
                    }
                    out.log_actions.push(LogCommandAction::Lookup {
                        player,
                        time_secs,
                        radius,
                        action,
                        page,
                    });
                }
                "inspect" | "i" => {
                    if source.is_console() {
                        return CommandOutput::error("/log inspect can only be used in-game.");
                    }
                    if !check_perm("blockgame.log.inspect") {
                        return CommandOutput::error("You do not have permission: blockgame.log.inspect");
                    }
                    out.log_actions.push(LogCommandAction::InspectToggle);
                }
                "rollback" | "rb" => {
                    if !check_perm("blockgame.log.rollback") {
                        return CommandOutput::error("You do not have permission: blockgame.log.rollback");
                    }
                    let mut radius: Option<i32> = None;
                    let mut time_secs: Option<u64> = None;
                    let mut player: Option<String> = None;
                    let mut preview = false;
                    for tok in rest {
                        if *tok == "--preview" || tok.eq_ignore_ascii_case("preview") {
                            preview = true;
                        } else if let Some((k, v)) = tok.split_once(':') {
                            match k.to_ascii_lowercase().as_str() {
                                "r" | "radius" => {
                                    let Ok(r) = v.parse::<i32>() else {
                                        return CommandOutput::error(format!("Invalid radius '{}'.", v));
                                    };
                                    radius = Some(r.max(0));
                                }
                                "t" | "time" => {
                                    let Some(secs) = crate::moderation::parse_duration_secs(v) else {
                                        return CommandOutput::error(format!("Invalid time '{}'.", v));
                                    };
                                    time_secs = Some(secs);
                                }
                                "p" | "player" | "u" | "user" => player = Some(v.to_string()),
                                _ => {}
                            }
                        }
                    }
                    let (Some(r), Some(t)) = (radius, time_secs) else {
                        return CommandOutput::error(
                            "Usage: /log rollback radius:<r> time:<t> [player:<name>] [--preview]",
                        );
                    };
                    out.log_actions.push(LogCommandAction::Rollback {
                        radius: r,
                        time_secs: t,
                        player,
                        preview,
                    });
                }
                "undo" => {
                    if !check_perm("blockgame.log.rollback") {
                        return CommandOutput::error("You do not have permission: blockgame.log.rollback");
                    }
                    out.log_actions.push(LogCommandAction::Undo);
                }
                "purge" => {
                    if !check_perm("blockgame.log.purge") {
                        return CommandOutput::error("You do not have permission: blockgame.log.purge");
                    }
                    let Some(raw) = rest.first() else {
                        return CommandOutput::error("Usage: /log purge <duration>");
                    };
                    let Some(secs) = crate::moderation::parse_duration_secs(raw) else {
                        return CommandOutput::error(format!("Invalid duration '{}'.", raw));
                    };
                    out.log_actions.push(LogCommandAction::Purge {
                        older_than_secs: secs,
                    });
                }
                _ => {
                    return CommandOutput::error(
                        "Usage: /log <lookup|inspect|rollback|undo|purge> ...",
                    );
                }
            }
        }

        other => {
            return CommandOutput::error(format!(
                "Unknown command '/{}'. Type '/help' for a list of commands.",
                other
            ));
        }
    }

    // Mark players whose Achievement Eligibility is forfeited by admin/cheat commands.
    // Standard utility commands (/spawn, /sethome, /home, /tpa, /skills view) preserve eligibility.
    match cmd.as_str() {
        "tp" | "tphere" => {
            for &(pid, _, _, _) in &out.teleports {
                if !out.cheat_taint_players.contains(&pid) {
                    out.cheat_taint_players.push(pid);
                }
            }
        }
        "give" => {
            for &(pid, _) in &out.gives {
                if !out.cheat_taint_players.contains(&pid) {
                    out.cheat_taint_players.push(pid);
                }
            }
        }
        "gamemode" | "gm" => {
            for &(pid, _) in &out.gamemode_updates {
                if !out.cheat_taint_players.contains(&pid) {
                    out.cheat_taint_players.push(pid);
                }
            }
        }
        "skills" | "skill" => {
            for action in &out.skill_actions {
                let target_pid = match action {
                    SkillCommandAction::Set { target_pid, .. }
                    | SkillCommandAction::AddXp { target_pid, .. }
                    | SkillCommandAction::Reset { target_pid } => Some(*target_pid),
                    SkillCommandAction::View { .. } => None,
                };
                if let Some(pid) = target_pid {
                    if !out.cheat_taint_players.contains(&pid) {
                        out.cheat_taint_players.push(pid);
                    }
                }
            }
        }
        "resetplayer" => {
            for uname in &out.reset_players {
                if let Some(p) = online_players
                    .iter()
                    .find(|p| p.username.eq_ignore_ascii_case(uname))
                {
                    if !out.cheat_taint_players.contains(&p.player_id) {
                        out.cheat_taint_players.push(p.player_id);
                    }
                }
            }
        }
        _ => {}
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_item_by_id_and_name() {
        let registry = ItemRegistry::new();
        let (id, name) = resolve_item("28", &registry).expect("Diamond by ID");
        assert_eq!(id, ItemId::DIAMOND);
        assert_eq!(name, "Diamond");

        let (id2, name2) = resolve_item("diamond", &registry).expect("Diamond by name");
        assert_eq!(id2, ItemId::DIAMOND);
        assert_eq!(name2, "Diamond");

        let (id3, name3) = resolve_item("diamond_pickaxe", &registry).expect("Diamond pickaxe");
        assert_eq!(id3, ItemId::DIAMOND_PICKAXE);
        assert_eq!(name3, "Diamond Pickaxe");
    }

    fn exec(
        source: &CommandSource,
        cmd: &str,
        online: &[OnlinePlayerInfo],
        perms: &mut PermissionsManager,
        bans: &mut BanManager,
        registry: &ItemRegistry,
        spawn: Vec3,
        tpa: &mut HashMap<u32, PendingTpaRequest>,
    ) -> CommandOutput {
        execute_command(source, cmd, online, perms, bans, registry, spawn, tpa)
    }

    #[test]
    fn test_kick_and_ban_commands() {
        let registry = ItemRegistry::new();
        let mut perms = PermissionsManager::new();
        let mut bans = BanManager::new();
        let mut tpa = HashMap::new();
        let spawn = Vec3::new(0.0, 64.0, 0.0);

        let online = vec![
            OnlinePlayerInfo::new(1, "Alice", Vec3::new(10.0, 65.0, 10.0), 0.0, 0.0),
            OnlinePlayerInfo::new(2, "Bob", Vec3::new(20.0, 65.0, 20.0), 1.0, 0.0),
        ];

        // 1. Bob has default perms; attempting /kick fails
        let res = exec(
            &CommandSource::Player {
                player_id: 2,
                username: "Bob".to_string(),
            },
            "/kick Alice griefing",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.kick_targets.is_empty());
        assert!(res.feedback[0].contains("do not have permission"));

        // 2. Promote Bob to moderator
        perms.add_player_group("Bob", "moderator").unwrap();

        // 3. Bob kicks Alice
        let res = exec(
            &CommandSource::Player {
                player_id: 2,
                username: "Bob".to_string(),
            },
            "/kick Alice griefing",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.kick_targets.len(), 1);
        assert_eq!(res.kick_targets[0].0, 1);
        assert_eq!(res.kick_targets[0].1, "griefing");

        // 4. Console bans Alice
        let res = exec(
            &CommandSource::Console,
            "ban Alice permanent ban",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.kick_targets.len(), 1);
        assert!(bans.is_banned("Alice").is_some());
    }

    #[test]
    fn test_tp_and_give_commands() {
        let registry = ItemRegistry::new();
        let mut perms = PermissionsManager::new();
        let mut bans = BanManager::new();
        let mut tpa = HashMap::new();
        let spawn = Vec3::new(0.0, 64.0, 0.0);

        let online = vec![
            OnlinePlayerInfo::new(1, "Alice", Vec3::new(10.0, 65.0, 10.0), 0.0, 0.0),
            OnlinePlayerInfo::new(2, "Bob", Vec3::new(20.0, 65.0, 20.0), 1.0, 0.0),
        ];

        // Give Bob diamonds from console
        let res = exec(
            &CommandSource::Console,
            "give Bob diamond 16",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.gives.len(), 1);
        assert_eq!(res.gives[0].0, 2);
        assert_eq!(res.gives[0].1.item_id, ItemId::DIAMOND);
        assert_eq!(res.gives[0].1.count, 16);

        // Teleport Bob to Alice from console
        let res = exec(
            &CommandSource::Console,
            "tp Bob Alice",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.teleports.len(), 1);
        assert_eq!(res.teleports[0].0, 2); // Bob teleported
        assert_eq!(res.teleports[0].1, Vec3::new(10.0, 65.0, 10.0)); // to Alice's position
    }

    #[test]
    fn test_permissions_command_management() {
        let registry = ItemRegistry::new();
        let mut perms = PermissionsManager::new();
        let mut bans = BanManager::new();
        let mut tpa = HashMap::new();
        let spawn = Vec3::new(0.0, 64.0, 0.0);
        let online = vec![];

        // 1. Console creates group 'builder'
        let res = exec(
            &CommandSource::Console,
            "perm group builder create",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.save_permissions);
        assert!(perms.groups.contains_key("builder"));

        // 2. Console adds permission to 'builder'
        let res = exec(
            &CommandSource::Console,
            "perm group builder add blockgame.command.give",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.save_permissions);

        // 3. Assign Charlie to 'builder'
        let res = exec(
            &CommandSource::Console,
            "perm user Charlie group add builder",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.save_permissions);
        assert!(perms.has_permission("Charlie", "blockgame.command.give"));
        assert!(!perms.has_permission("Charlie", "blockgame.command.kick"));
    }

    #[test]
    fn test_rank_and_resetplayer_commands() {
        let registry = ItemRegistry::new();
        let mut perms = PermissionsManager::new();
        let mut bans = BanManager::new();
        let mut tpa = HashMap::new();
        let spawn = Vec3::new(0.0, 64.0, 0.0);
        let online = vec![OnlinePlayerInfo::new(1, "Steve", Vec3::new(0.0, 50.0, 0.0), 0.0, 0.0)];

        // Query default rank
        let res = exec(
            &CommandSource::Console,
            "rank Steve",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.feedback[0].contains("default"));

        // Set rank to moderator
        let res = exec(
            &CommandSource::Console,
            "rank Steve moderator",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.save_permissions);
        assert_eq!(res.rank_updates, vec![("Steve".to_string(), "moderator".to_string())]);
        assert!(perms.has_permission("Steve", "blockgame.command.kick"));

        // Reset player
        let res = exec(
            &CommandSource::Console,
            "resetplayer Steve",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.reset_players, vec!["Steve".to_string()]);
    }

    #[test]
    fn test_spawn_and_setspawn_commands() {
        let registry = ItemRegistry::new();
        let mut perms = PermissionsManager::new();
        let mut bans = BanManager::new();
        let mut tpa = HashMap::new();
        let spawn = Vec3::new(100.0, 70.0, 200.0);
        let online = vec![OnlinePlayerInfo::new(1, "Alice", Vec3::new(15.0, 64.0, 25.0), 0.0, 0.0)];

        // 1. Regular player executes /spawn
        let res = exec(
            &CommandSource::Player {
                player_id: 1,
                username: "Alice".to_string(),
            },
            "/spawn",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.teleports.len(), 1);
        assert_eq!(res.teleports[0].0, 1);
        assert_eq!(res.teleports[0].1, spawn);

        // 2. Regular player attempting /setspawn fails (needs perm)
        let res = exec(
            &CommandSource::Player {
                player_id: 1,
                username: "Alice".to_string(),
            },
            "/setspawn",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.new_spawn.is_none());
        assert!(res.feedback[0].contains("do not have permission"));

        // 3. Admin / console sets spawn
        let res = exec(
            &CommandSource::Console,
            "/setspawn 50 65 50",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.new_spawn, Some(Vec3::new(50.0, 65.0, 50.0)));
        assert!(res.broadcasts[0].contains("[Spawn]"));

        // 4. Test alias `/set spawn`
        let res2 = exec(
            &CommandSource::Console,
            "/set spawn 12 34 56",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res2.new_spawn, Some(Vec3::new(12.0, 34.0, 56.0)));
    }

    #[test]
    fn test_homes_commands() {
        let registry = ItemRegistry::new();
        let mut perms = PermissionsManager::new();
        let mut bans = BanManager::new();
        let mut tpa = HashMap::new();
        let spawn = Vec3::new(0.0, 64.0, 0.0);

        let mut online = vec![OnlinePlayerInfo::new(1, "Alice", Vec3::new(10.0, 65.0, 10.0), 0.0, 0.0)];

        // Alice sets home 'base'
        let res = exec(
            &CommandSource::Player {
                player_id: 1,
                username: "Alice".to_string(),
            },
            "/sethome base",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.set_home, Some((1, "base".to_string(), Vec3::new(10.0, 65.0, 10.0), 0.0, 0.0)));

        // Record the home into Alice's online player state
        online[0].homes.insert("base".to_string(), (Vec3::new(10.0, 65.0, 10.0), 0.0, 0.0));

        // Alice moves away and types /home base
        online[0].position = Vec3::new(500.0, 64.0, 500.0);
        let res = exec(
            &CommandSource::Player {
                player_id: 1,
                username: "Alice".to_string(),
            },
            "/home base",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.teleports.len(), 1);
        assert_eq!(res.teleports[0].1, Vec3::new(10.0, 65.0, 10.0));

        // Alice lists homes
        let res = exec(
            &CommandSource::Player {
                player_id: 1,
                username: "Alice".to_string(),
            },
            "/homes",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.feedback[0].contains("[/home base]"));

        // Alice deletes home 'base'
        let res = exec(
            &CommandSource::Player {
                player_id: 1,
                username: "Alice".to_string(),
            },
            "/delhome base",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.del_home, Some((1, "base".to_string())));
    }

    #[test]
    fn test_tpa_system() {
        let registry = ItemRegistry::new();
        let mut perms = PermissionsManager::new();
        let mut bans = BanManager::new();
        let mut tpa = HashMap::new();
        let spawn = Vec3::new(0.0, 64.0, 0.0);

        let online = vec![
            OnlinePlayerInfo::new(1, "Alice", Vec3::new(10.0, 65.0, 10.0), 0.0, 0.0),
            OnlinePlayerInfo::new(2, "Bob", Vec3::new(50.0, 65.0, 50.0), 0.0, 0.0),
        ];

        // 1. Alice requests teleport to Bob
        let res = exec(
            &CommandSource::Player {
                player_id: 1,
                username: "Alice".to_string(),
            },
            "/tpa Bob",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.feedback[0].contains("Teleport request sent to Bob"));
        assert_eq!(res.direct_messages.len(), 1);
        assert_eq!(res.direct_messages[0].0, 2); // sent to Bob
        assert!(res.direct_messages[0].1.contains("[Accept] [Deny]"));
        assert!(tpa.contains_key(&2));

        // 2. Bob accepts the request
        let res = exec(
            &CommandSource::Player {
                player_id: 2,
                username: "Bob".to_string(),
            },
            "/tpaccept",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.teleports.len(), 1);
        assert_eq!(res.teleports[0].0, 1); // Alice teleported
        assert_eq!(res.teleports[0].1, Vec3::new(50.0, 65.0, 50.0)); // to Bob's position
        assert!(!tpa.contains_key(&2)); // request consumed
    }

    #[test]
    fn test_ban_ip_and_unban_ip_commands() {
        let registry = ItemRegistry::new();
        let mut perms = PermissionsManager::new();
        let mut bans = BanManager::new();
        let spawn = Vec3::new(0.5, 76.0, 0.5);
        let mut tpa = HashMap::new();

        let online = vec![
            OnlinePlayerInfo::new(1, "Alice", Vec3::ZERO, 0.0, 0.0).with_ip("10.0.0.1"),
            OnlinePlayerInfo::new(2, "Bob", Vec3::ZERO, 0.0, 0.0).with_ip("10.0.0.2"),
            OnlinePlayerInfo::new(3, "Charlie", Vec3::ZERO, 0.0, 0.0).with_ip("10.0.0.1"), // Same IP as Alice
        ];

        // 1. Regular player cannot run /ban-ip
        let res = exec(
            &CommandSource::Player {
                player_id: 2,
                username: "Bob".to_string(),
            },
            "/ban-ip Alice Griefing",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.feedback[0].contains("You do not have permission"));
        assert!(bans.is_ip_banned("10.0.0.1").is_none());

        // 2. Console runs /ban-ip by player name: resolves Alice's IP (10.0.0.1) and kicks both Alice & Charlie
        let res = exec(
            &CommandSource::Console,
            "/ban-ip Alice Griefing",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(bans.is_ip_banned("10.0.0.1").is_some());
        assert!(res.save_bans);
        assert_eq!(res.kick_targets.len(), 2);
        assert!(res.kick_targets.iter().any(|(id, r)| *id == 1 && r.contains("Griefing")));
        assert!(res.kick_targets.iter().any(|(id, r)| *id == 3 && r.contains("Griefing")));

        // 3. /banlist ips shows the banned IP
        let res = exec(
            &CommandSource::Console,
            "/banlist ips",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.feedback.iter().any(|f| f.contains("10.0.0.1")));

        // 4. Console runs /unban-ip
        let res = exec(
            &CommandSource::Console,
            "/unban-ip 10.0.0.1",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(bans.is_ip_banned("10.0.0.1").is_none());
        assert!(res.feedback[0].contains("Unbanned IP '10.0.0.1'"));

        // 5. Console runs /ban-ip directly with raw IP address
        let _res = exec(
            &CommandSource::Console,
            "/ban-ip 192.168.1.55 Proxy botting",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(bans.is_ip_banned("192.168.1.55").is_some());
        assert_eq!(bans.is_ip_banned("192.168.1.55").unwrap().reason, "Proxy botting");
    }

    #[test]
    fn test_gamemode_command() {
        let mut perms = PermissionsManager::new();
        let _ = perms.add_player_group("Alice", "admin");
        let mut bans = BanManager::new();
        let mut tpa = HashMap::new();
        let registry = ItemRegistry::new();
        let spawn = Vec3::new(0.0, 65.0, 0.0);
        let online = vec![
            OnlinePlayerInfo::new(1, "Alice", Vec3::ZERO, 0.0, 0.0).with_ip("127.0.0.1"),
            OnlinePlayerInfo::new(2, "Bob", Vec3::ZERO, 0.0, 0.0).with_ip("127.0.0.2"),
        ];

        // 1. Admin switches self to Creative via /gamemode c
        let res = exec(
            &CommandSource::Player {
                player_id: 1,
                username: "Alice".to_string(),
            },
            "/gamemode creative",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.gamemode_updates, vec![(1, crate::GameMode::Creative)]);

        // 2. Console switches Bob to Survival via /gm 0 Bob
        let res = exec(
            &CommandSource::Console,
            "/gm 0 Bob",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(res.gamemode_updates, vec![(2, crate::GameMode::Survival)]);
        assert!(!res.direct_messages.is_empty());

        // 3. Invalid mode returns error
        let res = exec(
            &CommandSource::Console,
            "/gamemode spectator",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.gamemode_updates.is_empty());
        assert!(res.feedback[0].contains("Unknown game mode"));
    }

    #[test]
    fn test_skills_command() {
        let mut perms = PermissionsManager::new();
        let _ = perms.add_player_group("Alice", "admin");
        let mut bans = BanManager::new();
        let mut tpa = HashMap::new();
        let registry = ItemRegistry::new();
        let spawn = Vec3::new(0.0, 65.0, 0.0);
        let online = vec![
            OnlinePlayerInfo::new(1, "Alice", Vec3::ZERO, 0.0, 0.0).with_ip("127.0.0.1"),
            OnlinePlayerInfo::new(2, "Bob", Vec3::ZERO, 0.0, 0.0).with_ip("127.0.0.2"),
        ];

        // 1. Default player Bob can view all skills (/skills) and a specific skill (/skills mining)
        let res = exec(
            &CommandSource::Player {
                player_id: 2,
                username: "Bob".to_string(),
            },
            "/skills",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(
            res.skill_actions,
            vec![SkillCommandAction::View {
                requester_pid: 2,
                skill: None,
            }]
        );

        let res = exec(
            &CommandSource::Player {
                player_id: 2,
                username: "Bob".to_string(),
            },
            "/skill mining",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(
            res.skill_actions,
            vec![SkillCommandAction::View {
                requester_pid: 2,
                skill: Some(crate::skills::SkillKind::Mining),
            }]
        );

        // 2. Non-admin Bob cannot set skills
        let res = exec(
            &CommandSource::Player {
                player_id: 2,
                username: "Bob".to_string(),
            },
            "/skills set Alice mining 250",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert!(res.skill_actions.is_empty());
        assert!(res.feedback[0].contains("do not have permission"));

        // 3. Admin Alice can set, addxp, and reset skills (with level clamp to 1000)
        let res = exec(
            &CommandSource::Player {
                player_id: 1,
                username: "Alice".to_string(),
            },
            "/skills set Bob mining 2500",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(
            res.skill_actions,
            vec![SkillCommandAction::Set {
                target_pid: 2,
                skill: crate::skills::SkillKind::Mining,
                level: crate::skills::MAX_SKILL_LEVEL,
            }]
        );

        let res = exec(
            &CommandSource::Console,
            "/skills addxp Bob combat 500",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(
            res.skill_actions,
            vec![SkillCommandAction::AddXp {
                target_pid: 2,
                skill: crate::skills::SkillKind::Combat,
                amount: 500,
            }]
        );

        let res = exec(
            &CommandSource::Console,
            "/skills reset Bob",
            &online,
            &mut perms,
            &mut bans,
            &registry,
            spawn,
            &mut tpa,
        );
        assert_eq!(
            res.skill_actions,
            vec![SkillCommandAction::Reset { target_pid: 2 }]
        );
    }

    #[test]
    fn test_achievement_cheat_taint_command_boundaries() {
        let mut perms = PermissionsManager::new();
        let _ = perms.add_player_group("Admin", "admin");
        let mut bans = BanManager::new();
        let registry = ItemRegistry::new();
        let spawn = Vec3::new(0.0, 20.0, 0.0);
        let mut tpa = HashMap::new();
        let online = vec![
            OnlinePlayerInfo::new(1, "Admin", Vec3::new(10.0, 20.0, 10.0), 0.0, 0.0),
            OnlinePlayerInfo::new(2, "Bob", Vec3::new(50.0, 20.0, 50.0), 0.0, 0.0),
        ];
        let admin_src = CommandSource::Player {
            player_id: 1,
            username: "Admin".to_string(),
        };

        // Utility commands preserve Achievement Eligibility
        for cmd in ["/spawn", "/sethome base", "/tpa Bob", "/skills"] {
            let res = execute_command(
                &admin_src,
                cmd,
                &online,
                &mut perms,
                &mut bans,
                &registry,
                spawn,
                &mut tpa,
            );
            assert!(
                res.cheat_taint_players.is_empty(),
                "Utility command '{cmd}' must not taint eligibility"
            );
        }

        // Admin/cheat commands taint only the targeted player
        for (cmd, expected_pid) in [
            ("/give Bob diamond 1", 2),
            ("/gamemode creative Bob", 2),
            ("/tp Bob 0 20 0", 2),
            ("/tphere Bob", 2),
            ("/skills set Bob mining 50", 2),
            ("/skills addxp Bob mining 100", 2),
            ("/skills reset Bob", 2),
        ] {
            let res = execute_command(
                &admin_src,
                cmd,
                &online,
                &mut perms,
                &mut bans,
                &registry,
                spawn,
                &mut tpa,
            );
            assert_eq!(
                res.cheat_taint_players,
                vec![expected_pid],
                "Cheat command '{cmd}' must taint target player {expected_pid}"
            );
        }
    }
}
