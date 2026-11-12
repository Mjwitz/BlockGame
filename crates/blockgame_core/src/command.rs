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
}

/// Declarative results produced by executing a command.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CommandOutput {
    /// Private feedback sent directly to the command sender.
    pub feedback: Vec<String>,
    /// Global system broadcasts sent to all connected players.
    pub broadcasts: Vec<String>,
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
) -> CommandOutput {
    let mut out = CommandOutput::default();
    let raw = command_line.trim();
    let clean = raw.strip_prefix('/').unwrap_or(raw).trim();

    if clean.is_empty() {
        return out;
    }

    let parts: Vec<&str> = clean.split_whitespace().collect();
    let cmd = parts[0].to_ascii_lowercase();
    let args = &parts[1..];

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
                        "list" => {
                            if let Some(group) = perms.groups.get(&group_name.to_ascii_lowercase()) {
                                out.feedback.push(format!(
                                    "Group '{}' permissions: {}",
                                    group.name,
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
                                "Unknown group action. Available: create, delete, add, remove, list",
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

        // --- 13. HELP COMMAND ---
        "help" | "?" => {
            out.feedback.push("Available Commands:".to_string());
            if check_perm("blockgame.command.kick") {
                out.feedback.push("  /kick <player> [reason] - Disconnect a player".to_string());
            }
            if check_perm("blockgame.command.ban") {
                out.feedback.push("  /ban <player> [reason] - Ban a player from the server".to_string());
                out.feedback.push("  /unban <player> - Remove ban from a player".to_string());
            }
            if check_perm("blockgame.command.tp") {
                out.feedback.push("  /tp <target> or /tp <player> <target> - Teleport players".to_string());
            }
            if check_perm("blockgame.command.tphere") {
                out.feedback.push("  /tphere <player> - Teleport player to your position".to_string());
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
            if check_perm("blockgame.command.stop") {
                out.feedback.push("  /stop - Shut down server".to_string());
            }
        }

        other => {
            return CommandOutput::error(format!(
                "Unknown command '/{}'. Type '/help' for a list of commands.",
                other
            ));
        }
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

    #[test]
    fn test_kick_and_ban_commands() {
        let registry = ItemRegistry::new();
        let mut perms = PermissionsManager::new();
        let mut bans = BanManager::new();

        let online = vec![
            OnlinePlayerInfo {
                player_id: 1,
                username: "Alice".to_string(),
                position: Vec3::new(10.0, 65.0, 10.0),
                yaw: 0.0,
                pitch: 0.0,
            },
            OnlinePlayerInfo {
                player_id: 2,
                username: "Bob".to_string(),
                position: Vec3::new(20.0, 65.0, 20.0),
                yaw: 1.0,
                pitch: 0.0,
            },
        ];

        // 1. Bob has default perms; attempting /kick fails
        let res = execute_command(
            &CommandSource::Player {
                player_id: 2,
                username: "Bob".to_string(),
            },
            "/kick Alice griefing",
            &online,
            &mut perms,
            &mut bans,
            &registry,
        );
        assert!(res.kick_targets.is_empty());
        assert!(res.feedback[0].contains("do not have permission"));

        // 2. Promote Bob to moderator
        perms.add_player_group("Bob", "moderator").unwrap();

        // 3. Bob kicks Alice
        let res = execute_command(
            &CommandSource::Player {
                player_id: 2,
                username: "Bob".to_string(),
            },
            "/kick Alice griefing",
            &online,
            &mut perms,
            &mut bans,
            &registry,
        );
        assert_eq!(res.kick_targets.len(), 1);
        assert_eq!(res.kick_targets[0].0, 1);
        assert_eq!(res.kick_targets[0].1, "griefing");

        // 4. Console bans Alice
        let res = execute_command(
            &CommandSource::Console,
            "ban Alice permanent ban",
            &online,
            &mut perms,
            &mut bans,
            &registry,
        );
        assert_eq!(res.kick_targets.len(), 1);
        assert!(bans.is_banned("Alice").is_some());
    }

    #[test]
    fn test_tp_and_give_commands() {
        let registry = ItemRegistry::new();
        let mut perms = PermissionsManager::new();
        let mut bans = BanManager::new();

        let online = vec![
            OnlinePlayerInfo {
                player_id: 1,
                username: "Alice".to_string(),
                position: Vec3::new(10.0, 65.0, 10.0),
                yaw: 0.0,
                pitch: 0.0,
            },
            OnlinePlayerInfo {
                player_id: 2,
                username: "Bob".to_string(),
                position: Vec3::new(20.0, 65.0, 20.0),
                yaw: 1.0,
                pitch: 0.0,
            },
        ];

        // Give Bob diamonds from console
        let res = execute_command(
            &CommandSource::Console,
            "give Bob diamond 16",
            &online,
            &mut perms,
            &mut bans,
            &registry,
        );
        assert_eq!(res.gives.len(), 1);
        assert_eq!(res.gives[0].0, 2);
        assert_eq!(res.gives[0].1.item_id, ItemId::DIAMOND);
        assert_eq!(res.gives[0].1.count, 16);

        // Teleport Bob to Alice from console
        let res = execute_command(
            &CommandSource::Console,
            "tp Bob Alice",
            &online,
            &mut perms,
            &mut bans,
            &registry,
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
        let online = vec![];

        // 1. Console creates group 'builder'
        let res = execute_command(
            &CommandSource::Console,
            "perm group builder create",
            &online,
            &mut perms,
            &mut bans,
            &registry,
        );
        assert!(res.save_permissions);
        assert!(perms.groups.contains_key("builder"));

        // 2. Console adds permission to 'builder'
        let res = execute_command(
            &CommandSource::Console,
            "perm group builder add blockgame.command.give",
            &online,
            &mut perms,
            &mut bans,
            &registry,
        );
        assert!(res.save_permissions);

        // 3. Assign Charlie to 'builder'
        let res = execute_command(
            &CommandSource::Console,
            "perm user Charlie group add builder",
            &online,
            &mut perms,
            &mut bans,
            &registry,
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
        let online = vec![OnlinePlayerInfo {
            player_id: 1,
            username: "Steve".to_string(),
            position: Vec3::new(0.0, 50.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
        }];

        // Query default rank
        let res = execute_command(
            &CommandSource::Console,
            "rank Steve",
            &online,
            &mut perms,
            &mut bans,
            &registry,
        );
        assert!(res.feedback[0].contains("default"));

        // Set rank to moderator
        let res = execute_command(
            &CommandSource::Console,
            "rank Steve moderator",
            &online,
            &mut perms,
            &mut bans,
            &registry,
        );
        assert!(res.save_permissions);
        assert_eq!(res.rank_updates, vec![("Steve".to_string(), "moderator".to_string())]);
        assert!(perms.has_permission("Steve", "blockgame.command.kick"));

        // Reset player
        let res = execute_command(
            &CommandSource::Console,
            "resetplayer Steve",
            &online,
            &mut perms,
            &mut bans,
            &registry,
        );
        assert_eq!(res.reset_players, vec!["Steve".to_string()]);
    }
}
