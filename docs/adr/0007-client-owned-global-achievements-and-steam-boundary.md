# Client-Owned Global Achievements with Server-Emitted Events and Pluggable Steam Backend

Achievement unlocks and lifetime Career Stats are owned by the client in a versioned local profile (`achievements.yaml`), evaluated by a shared engine in `blockgame_core`, and decoupled from platform SDKs behind an `AchievementBackend` trait using stable `snake_case` achievement and stat IDs. In singleplayer, `blockgame_client` feeds gameplay events directly into the engine; in multiplayer, `MultiplayerServer` remains authoritative for world simulation ([ADR 0004](./0004-shared-core-server-authoritative-world-simulation.md)) and emits lightweight `AchievementEvent` packets along with per-session Achievement Eligibility so the client updates its global profile consistently across worlds and servers.

## Considered Options

- **Per-world / server-owned achievements in `players/<name>.dat`**: Mirrors Minecraft advancements, but conflicts with 1:1 Steam achievement semantics, resets player progress on every new world or server, and complicates bidirectional Steam reconciliation.
- **Purely client-inferred multiplayer achievements**: Avoids new network packets, but misses server-authoritative events (combat/boss kills, breeding, Shopkeeper tiers, Enhancements, admin cheat tainting) and is brittle against packet changes.

## Consequences

- `MultiplayerServer` does not persist achievement progress; it tracks world/session Achievement Eligibility, emits gameplay event packets to clients, and optionally broadcasts client-reported unlocks in chat when `broadcast-achievements=true` in `server.properties`.
- `WorldMetadata` (`level.dat`) gains a persistent `achievements_enabled` flag (permanently cleared if Creative mode or admin/cheat commands are used in singleplayer); in multiplayer Survival worlds, per-player admin cheat commands taint only the targeted player's session.
- Future Steamworks (`steamworks-rs`) integration only needs to implement `AchievementBackend` and reconcile `achievements.yaml` against Steam on startup, without refactoring gameplay or networking code.
