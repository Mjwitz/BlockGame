# Commands and Permissions

Commands can be run from the **dedicated server console** (without a leading `/`) or in **in-game chat** (press `T` and prefix with `/`, e.g., `/give Steve diamond 16`).

## Server & Admin Commands

| Command | Description | Permission Node |
| :--- | :--- | :--- |
| `help` / `?` | Display available commands | *(all players)* |
| `list` | List online players | *(all players)* |
| `say <message>` | Broadcast a server message to all players | `blockgame.command.say` |
| `kick <player> [reason]` | Kick an online player from the server | `blockgame.command.kick` |
| `ban <player> [reason]` | Ban a player username from joining | `blockgame.command.ban` |
| `unban <player>` / `pardon <player>` | Remove a player's ban | `blockgame.command.unban` |
| `ban-ip <player\|ip> [reason]` | Ban an IP address (or the IP of an online player) to block alts | `blockgame.command.banip` |
| `unban-ip <ip>` / `pardon-ip <ip>` | Remove an IP address from the ban list | `blockgame.command.unbanip` |
| `banlist [ips\|players]` | View all banned player accounts or banned IP addresses | `blockgame.command.banlist` |
| `tp <player> <target>` | Teleport `<player>` to `<target>` | `blockgame.command.tp` |
| `tp <player> <x> <y> <z>` | Teleport `<player>` to specific coordinates | `blockgame.command.tp` |
| `tphere <player>` | Teleport `<player>` to your current position | `blockgame.command.tphere` |
| `give <player> <item> [amount]` | Give an item by ID or name (e.g. `diamond`, `iron_pickaxe`) | `blockgame.command.give` |
| `rank <player> [rank]` | View or set a player's rank (e.g. `member`, `moderator`, `admin`) | `blockgame.command.rank` |
| `resetplayer <player>` | Reset a player's saved world data & inventory | `blockgame.command.resetplayer` |
| `setspawn` | Set world spawn coordinates to your current position | `blockgame.command.setspawn` |
| `stop` | Save the world and cleanly shut down the server | *(console only)* |

## Waypoints & Player Teleportation Commands

Players can set personal homes, teleport to spawn, and request teleportation to other players:

| Command | Description | Permission Node |
| :--- | :--- | :--- |
| `spawn` | Teleport to the world spawn point | `blockgame.command.spawn` |
| `sethome [name]` | Save current location as a named home (defaults to `home`) | `blockgame.command.sethome` |
| `home [name]` | Teleport to your saved home | `blockgame.command.home` |
| `delhome <name>` | Delete a previously saved home | `blockgame.command.delhome` |
| `homes` | List all your saved homes with clickable `[Teleport]` buttons | `blockgame.command.homes` |
| `tpa <player>` | Send a teleport request to `<player>` with clickable buttons | `blockgame.command.tpa` |
| `tpaccept` | Accept a pending teleport request | `blockgame.command.tpaccept` |
| `tpdeny` | Deny a pending teleport request | `blockgame.command.tpdeny` |

### Interactive Clickable Chat Buttons
When running `/tpa <player>`, the target player sees clickable `[Accept]` and `[Deny]` action buttons in chat. Similarly, `/homes` displays a `[Teleport]` button next to each home name. Opening chat (`T`) releases the mouse cursor so you can click these buttons directly with your mouse.

## Permissions & Groups Configuration (`permissions.yml`)

The server uses a YAML-based permission manager (`permissions.yml` in the server root), supporting group inheritance, fine-grained permission nodes, and chat prefixes with color codes:

```yaml
groups:
  default:
    prefix: "&7[Member]&r "
    permissions:
      - blockgame.command.spawn
      - blockgame.command.home
      - blockgame.command.sethome
      - blockgame.command.delhome
      - blockgame.command.homes
      - blockgame.command.tpa
      - blockgame.command.tpaccept
      - blockgame.command.tpdeny
  moderator:
    inheritance:
      - default
    prefix: "&a[Mod]&r "
    permissions:
      - blockgame.command.kick
      - blockgame.command.ban
      - blockgame.command.unban
      - blockgame.command.banip
      - blockgame.command.unbanip
      - blockgame.command.banlist
      - blockgame.command.tp
      - blockgame.command.tphere
  admin:
    inheritance:
      - moderator
    prefix: "&c[Admin]&r "
    permissions:
      - "*"
users:
  Steve:
    groups:
      - admin
```

*(Legacy `permissions.txt` files are automatically migrated to `permissions.yml` on launch).*

### In-Game / Console Permission Commands (`perm`)

Manage permissions and groups on the fly without restarting:

- **Groups**:
  - `perm group list` — List all permission groups
  - `perm group <group> create` — Create a new group
  - `perm group <group> delete` — Delete a group
  - `perm group <group> add <node>` — Grant a permission node to a group
  - `perm group <group> remove <node>` — Revoke a permission node from a group
  - `perm group <group> list` — List permissions assigned to a group
- **Users**:
  - `perm user <player> group add <group>` — Add a player to a group
  - `perm user <player> group remove <group>` — Remove a player from a group
  - `perm user <player> add <node>` — Grant a permission node directly to a player
  - `perm user <player> remove <node>` — Revoke a permission node from a player
  - `perm user <player> list` — Inspect a player's groups and permissions

Permission wildcards are supported (e.g., `blockgame.command.*` or `*` for full admin privileges).
