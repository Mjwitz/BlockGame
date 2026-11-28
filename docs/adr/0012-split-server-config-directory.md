# Split Server Configuration into Per-Concern Files under `config/`

Dedicated server configuration and moderation state (permissions, bans, mutes, warnings, word filter, chat settings, and Auto-Messages) are stored as separate per-concern files inside a server-root `config/` directory—`config/permissions.yml`, `config/bans.txt`, `config/banned-ips.txt`, `config/mutes.yml`, `config/warnings.yml`, `config/word_filter.yml`, `config/chat_settings.yml`, and `config/automessages.yml`—while `server.properties` remains in the server root as the primary startup entrypoint. Each file is saved via atomic write-then-rename, and existing root-level `permissions.yml` (or legacy `permissions.txt`), `bans.txt`, and `banned-ips.txt` are automatically migrated into `config/` on startup.

## Considered Options

- **Combine moderation state into a single `moderation.yml` alongside root-level `permissions.yml` and ban files**: Reduces file count, but mixes unrelated write paths (frequent mute/warning updates alongside static word-filter and chat-lock settings) into one monolithic file where a bad edit or partial write risks corrupting all moderation state.
- **Keep all new YAML files in the server root**: Avoids creating a subdirectory or migration step, but clutters the server root as multiplayer subsystems grow (including future Claiming and Block Logging configs).

## Consequences

- Each server subsystem owns and persists only its own file in `config/`, isolating corruption blast radius and making manual editing and backups straightforward.
- Future multiplayer server systems (such as Chunk Claiming configuration) follow the same `config/<subsystem>.yml` convention.
- Startup runs a one-time, non-destructive migration when legacy root-level `permissions.yml`/`permissions.txt`, `bans.txt`, or `banned-ips.txt` exist and their `config/` counterparts do not yet exist.
