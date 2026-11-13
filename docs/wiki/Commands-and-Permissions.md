# Commands and Permissions

Commands can be run from the **dedicated server console** (without a leading `/`) or in **in-game chat** (press `T` and prefix with `/`, e.g., `/give Steve diamond 16`).

## Server & Admin Commands

| Command | Description |
| :--- | :--- |
| `help` / `?` | Display available commands |
| `list` | List online players |
| `say <message>` | Broadcast a server message to all players |
| `kick <player> [reason]` | Kick an online player |
| `ban <player> [reason]` | Ban a player from joining the server |
| `unban <player>` / `pardon <player>` | Remove a player's ban |
| `tp <player> <target>` | Teleport `<player>` to `<target>` |
| `tp <player> <x> <y> <z>` | Teleport `<player>` to coordinates |
| `tphere <player>` | Teleport `<player>` to your position (in-game) |
| `give <player> <item> [amount]` | Give an item by ID or name (e.g., `diamond`, `iron_pickaxe`) |
| `rank <player> [rank]` | View or set a player's rank (e.g., `moderator`, `admin`) |
| `resetplayer <player>` | Reset a player's saved account/data |
| `stop` | Save the world and cleanly shut down the server |

## Permissions & Groups (`perm`)

Manage fine-grained permission nodes and groups using `perm`:

- **Groups**:
  - `perm group list` — List all permission groups
  - `perm group <group> create` — Create a new group
  - `perm group <group> delete` — Delete a group
  - `perm group <group> add <node>` — Grant a permission node to a group
  - `perm group <group> remove <node>` — Revoke a permission node from a group
  - `perm group <group> list` — List permissions on a group
- **Users**:
  - `perm user <player> group add <group>` — Add a player to a group
  - `perm user <player> group remove <group>` — Remove a player from a group
  - `perm user <player> add <node>` — Grant a permission node directly to a player
  - `perm user <player> remove <node>` — Revoke a permission node from a player
  - `perm user <player> list` — Inspect a player's groups and permissions

Permission nodes follow the `blockgame.command.<command>` convention (for example, `blockgame.command.give`, `blockgame.command.tp`, `blockgame.command.ban`).
