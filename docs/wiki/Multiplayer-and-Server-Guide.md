# Multiplayer and Server Guide

BlockGame includes a headless dedicated multiplayer server (`blockgame_server`) that runs on both **Linux** and **Windows**.

## Starting the Dedicated Server

### Linux
```bash
./blockgame_server
```

### Windows
```powershell
.\blockgame_server.exe
```

*(Or from source with Cargo: `cargo run --release -p blockgame_server`)*

When the server starts without arguments, it automatically creates and loads **`server.properties`**. You can customize all settings inside this file once, so you don't need to specify command-line flags on every launch.

## Configuration File (`server.properties`)

The server reads `server.properties` from the current working directory. If it doesn't exist yet, a default template is generated on first launch:

```properties
# BlockGame Dedicated Server Configuration
server-port=25565
bind-address=0.0.0.0
world-name=Multiplayer World
seed=1337

# Public Server Browser / Discovery
discoverable=false
server-name=Multiplayer World
motd=&aA BlockGame Dedicated Server
directory-url=http://15.204.81.97:25569
public-host=auto
max-players=20
```

To use a custom config file path (e.g. for multiple server instances):
```bash
./blockgame_server --config /path/to/my_server.properties
```

## CLI Options (Overrides)

Any flags provided on the command line override settings defined in `server.properties`:

| Flag | Description | Default |
| :--- | :--- | :--- |
| `-c, --config <FILE>` | Path to config file | `server.properties` |
| `-p, --port <PORT>` | TCP port to listen on | `25565` |
| `-w, --world <NAME>` | World name and save folder under `saves/` | `Multiplayer World` |
| `-s, --seed <SEED>` | World generation seed (number or text string) | `1337` |
| `-b, --bind <HOST>` | Bind address host | `0.0.0.0` |
| `--discoverable` | Register this server on the central discoverable server directory | `false` |
| `--no-discoverable` | Disable discovery registration (overrides config) | — |
| `--server-name <NAME>` | Public server name displayed in browser | `world-name` |
| `--motd <TEXT>` | Message of the day (supports & color codes) | `&aA BlockGame Dedicated Server` |
| `--directory-url <URL>` | Central directory URL to announce to | `http://15.204.81.97:25569` |
| `--public-host <HOST>` | Override public IP/hostname (`auto` detects peer IP) | `auto` |
| `--max-players <N>` | Maximum reported player slots | `20` |
| `-h, --help` | Print CLI help and console command summary | — |


## Discoverable Servers & Central Directory

To make your dedicated server appear in the in-game **Multiplayer -> Discoverable Servers** tab, run with `--discoverable`:

```bash
./blockgame_server --port 25565 --discoverable --directory-url "http://<directory-host>:25569"
```

The game server sends heartbeats every 15 seconds to keep its entry active.

### Running the Central Directory Server (`blockgame_directory`)

The central directory is a lightweight HTTP service (`blockgame_directory`) that tracks active servers and serves the server list to players.

You can manage it without `systemd` using the provided manager script:

```bash
# Build the release binary
./scripts/manage_directory.sh build

# Option A: Start in background with auto-restart watchdog
./scripts/manage_directory.sh start
./scripts/manage_directory.sh status
./scripts/manage_directory.sh logs
./scripts/manage_directory.sh stop

# Option B: Run in a detachable tmux session
./scripts/manage_directory.sh tmux
# (Detach with Ctrl+B then D, reattach with tmux attach -t blockgame-directory)

# Option C: Docker / Docker Compose
docker compose -f docker-compose.directory.yml up -d
```

## Multiplayer World Behavior & Server Features

### Real-Time World & Pause Menu
In multiplayer mode, opening the in-game Pause Menu (`Esc`) does **not** pause the world or server. The game continues running around the player: you can still take damage from monsters, drown, fall, or be interacted with by other players while in the menu.

### Server Console Chat & Command Logging
The dedicated server console continuously streams all in-game activity:
- Chat messages: `[CHAT] <PlayerName>: message text`
- Commands issued: `[COMMAND] <PlayerName> issued: /command arguments`
This makes server administration, moderation, and auditing easy directly from your server terminal or tmux pane.

### IP Bans & Alt Account Protection (`banned-ips.txt`)
To prevent griefers and banned players from simply reconnecting with an alternate username, admins can ban by IP:
```text
ban-ip <player|ip> [reason]
unban-ip <ip>
banlist ips
```
When an IP ban is issued, any online players connected from that IP address are immediately kicked, and their IP address is persisted to `banned-ips.txt`. Any future connection attempts from banned IPs are rejected during the TCP handshake before reaching the world.

### Ungraceful Disconnect & Timeout Detection
The server monitors client connection heartbeats and handles abrupt network drops (such as client crashes or internet outages). If a client fails to send a packet within **15 seconds** or triggers a TCP write error, the server automatically cleans up the player entity, releases the slot, and broadcasts a disconnect notice so "ghost players" are never left lingering on the server.

## Connecting Clients
1. Launch `blockgame_client` (or `blockgame_client.exe`).
2. Select **Multiplayer** from the title screen.
3. You can either:
   - Enter a server address manually (e.g. `<server-ip>:25565`).
   - Switch to the **Discoverable Servers** tab and click **Refresh** to join any public server announced to the central directory.
4. Make sure TCP port `25565` (for game traffic) and TCP port `25569` (for central directory, if hosting) are allowed in your firewall.

## Stopping the Server Safely
Type `stop` in the server console (or press `Ctrl+C`) to unregister from the directory, save all chunks, player inventories, permissions, homes, and bans cleanly before exiting.


