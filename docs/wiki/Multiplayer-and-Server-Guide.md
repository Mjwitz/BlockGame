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
| `-h, --help` | Print CLI help and console command summary | — |

## Connecting Clients
1. Launch `blockgame_client` (or `blockgame_client.exe`).
2. Select **Multiplayer** from the title screen.
3. Enter the server address (e.g., `127.0.0.1:25565` for local testing or `<server-ip>:25565` over LAN/Internet), your username, and password.
4. Make sure TCP port `25565` (or your chosen `--port`) is allowed through your firewall/router if hosting for remote players.

## Stopping the Server Safely
Type `stop` in the server console (or press `Ctrl+C`) to save all chunks, player inventories, permissions, and bans cleanly before exiting.
