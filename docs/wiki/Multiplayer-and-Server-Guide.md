# Multiplayer and Server Guide

BlockGame includes a headless dedicated multiplayer server (`blockgame_server`) that runs on both **Linux** and **Windows**.

## Starting the Dedicated Server

### Linux
```bash
./blockgame_server --port 25565 --world "MyWorld" --seed 1337
```

### Windows
```powershell
.\blockgame_server.exe --port 25565 --world "MyWorld" --seed 1337
```

*(Or from source with Cargo: `cargo run --release -p blockgame_server -- --port 25565 --world "MyWorld" --seed 1337`)*

## CLI Options

| Flag | Description | Default |
| :--- | :--- | :--- |
| `-p, --port <PORT>` | TCP port to listen on | `25565` |
| `-w, --world <NAME>` | World name and save folder under `saves/` | `Multiplayer World` |
| `-s, --seed <SEED>` | World generation seed (number or text string) | `1337` |
| `-b, --bind <HOST>` | Bind address host | `0.0.0.0` |
| `--discoverable` | Register this server on the central discoverable server directory | `false` |
| `--directory-url <URL>` | Central directory URL to announce to | `http://15.204.81.97:25569` |
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
```

## Connecting Clients
1. Launch `blockgame_client` (or `blockgame_client.exe`).
2. Select **Multiplayer** from the title screen.
3. You can either:
   - Enter a server address manually (e.g. `<server-ip>:25565`).
   - Switch to the **Discoverable Servers** tab and click **Refresh** to join any public server.
4. Make sure TCP port `25565` (for game traffic) and TCP port `25569` (for central directory, if hosting) are allowed in your firewall.

## Stopping the Server Safely
Type `stop` in the server console (or press `Ctrl+C`) to unregister from the directory, save all chunks, player inventories, permissions, and bans cleanly before exiting.

