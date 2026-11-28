# Async Batched Per-World SQLite Block Log

Player block breaks, block placements (with container snapshots on break), and container item add/remove diffs are stored in a per-world SQLite database at `saves/<world_name>/blocklog.db` using bundled `rusqlite` in WAL mode. The 20 TPS server tick thread never performs synchronous SQLite writes; instead, it pushes log events onto an in-memory channel consumed by a dedicated background writer thread that flushes batches periodically and on graceful shutdown.

## Considered Options

- **Synchronous SQLite writes on the server tick thread**: Simpler to wire up, but fsync latency spikes during heavy building or chest transfers would stall the 20 TPS simulation loop for all connected players.
- **Append-only YAML/JSON log files**: Avoids adding a SQLite dependency, but cannot efficiently index spatial/temporal/player queries for `/log lookup` and `/log rollback` or track reversible `rolled_back` flags.

## Consequences

- `blockgame_server` adds a `rusqlite` (`bundled`) dependency so servers require no external system SQLite library.
- A hard process crash can lose at most one unflushed in-memory batch (~250ms of events), while graceful `/stop` flushes and closes the writer cleanly.
- Natural world simulation ticks (crop/grass growth, leaf decay, natural fluid spread) are excluded to keep database growth bounded; retention and manual `/log purge` settings live in `config/blocklog.yml`.
