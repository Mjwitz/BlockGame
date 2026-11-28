//! Per-world asynchronous batched SQLite Block Log (ADR-0014).
//!
//! Stores player block edits (with broken-container item snapshots) and
//! Chest/Furnace/Cooking Pot item diffs in `saves/<world>/blocklog.db` using
//! WAL mode and a dedicated background writer thread so the 20 TPS server tick
//! loop never performs synchronous SQLite writes.
//!
//! When compiled without the `sqlite-blocklog` feature (e.g. `blockgame_client`
//! or WASM targets), a zero-dependency no-op stub is compiled instead so
//! `blockgame_core` remains platform-agnostic.

use std::collections::HashMap;
use std::path::Path;

use glam::IVec3;
use serde::{Deserialize, Serialize};

use crate::block::BlockId;
#[cfg(feature = "sqlite-blocklog")]
use crate::config_io::now_unix_secs;
use crate::item::{Enhancement, ItemId, ItemSlot};

fn default_retention_days() -> u32 {
    0
}

fn default_max_rollback_radius() -> i32 {
    50
}

/// Global Block Log settings persisted in `config/blocklog.yml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockLogConfig {
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_max_rollback_radius")]
    pub max_rollback_radius: i32,
}

impl Default for BlockLogConfig {
    fn default() -> Self {
        Self {
            retention_days: default_retention_days(),
            max_rollback_radius: default_max_rollback_radius(),
        }
    }
}

/// Container type tracked by the Block Log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerKind {
    Chest,
    Furnace,
    CookingPot,
}

impl ContainerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chest => "chest",
            Self::Furnace => "furnace",
            Self::CookingPot => "cooking_pot",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "furnace" => Self::Furnace,
            "cooking_pot" => Self::CookingPot,
            _ => Self::Chest,
        }
    }
}

/// Action filter for `/log lookup`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockLogActionFilter {
    #[default]
    All,
    Block,
    Container,
}

impl BlockLogActionFilter {
    pub fn parse_opt(raw: Option<&str>) -> Self {
        match raw.map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("block") | Some("blocks") => Self::Block,
            Some("container") | Some("containers") | Some("item") | Some("items") => Self::Container,
            _ => Self::All,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlockLogEvent {
    pub id: i64,
    pub ts: u64,
    pub actor: String,
    pub pos: IVec3,
    pub old_block: BlockId,
    pub new_block: BlockId,
    /// Optional non-empty slot snapshot captured when a container block is broken.
    pub container_snapshot: Option<Vec<(usize, ItemSlot)>>,
}

#[derive(Debug, Clone)]
pub struct ContainerLogEvent {
    pub id: i64,
    pub ts: u64,
    pub actor: String,
    pub pos: IVec3,
    pub container_kind: ContainerKind,
    pub is_add: bool,
    pub item: ItemSlot,
}

/// Unified lookup/inspect entry returned by `/log lookup` and `/log inspect`.
#[derive(Debug, Clone)]
pub struct LogEntryView {
    pub ts: u64,
    pub actor: String,
    pub pos: IVec3,
    pub summary: String,
    pub rolled_back: bool,
}

/// Filter parameters for `/log lookup`.
#[derive(Debug, Clone, Default)]
pub struct BlockLogQuery {
    pub actor: Option<String>,
    pub since_ts: Option<u64>,
    pub center: Option<IVec3>,
    pub radius: Option<i32>,
    pub action_filter: BlockLogActionFilter,
    pub page: usize,
    pub page_size: usize,
}

/// Concrete set of block & container events to revert (or replay on `/log undo`).
#[derive(Debug, Clone, Default)]
pub struct RollbackPlan {
    pub block_events: Vec<BlockLogEvent>,
    pub container_events: Vec<ContainerLogEvent>,
}

/// Computes per-item added/removed diffs between two container slot snapshots,
/// preserving item ID, durability, and Enhancement while ignoring in-chest slot shuffles.
pub fn diff_container_slots(old_slots: &[ItemSlot], new_slots: &[ItemSlot]) -> Vec<(bool, ItemSlot)> {
    #[derive(Hash, PartialEq, Eq, Clone)]
    struct ItemKey {
        item_id: u16,
        durability: u16,
        enhancement: Option<Enhancement>,
    }

    let tally = |slots: &[ItemSlot]| -> HashMap<ItemKey, i64> {
        let mut map = HashMap::new();
        for s in slots {
            if !s.is_empty() && s.count > 0 {
                let key = ItemKey {
                    item_id: s.item_id.0,
                    durability: s.durability,
                    enhancement: s.enhancement,
                };
                *map.entry(key).or_insert(0) += s.count as i64;
            }
        }
        map
    };

    let old_map = tally(old_slots);
    let new_map = tally(new_slots);
    let mut keys: Vec<ItemKey> = old_map.keys().chain(new_map.keys()).cloned().collect();
    keys.sort_by_key(|k| {
        (
            k.item_id,
            k.durability,
            k.enhancement.map(|e| e.to_u8()).unwrap_or(0),
        )
    });
    keys.dedup();

    let mut diffs = Vec::new();
    for k in keys {
        let before = *old_map.get(&k).unwrap_or(&0);
        let after = *new_map.get(&k).unwrap_or(&0);
        let delta = after - before;
        if delta != 0 {
            let is_add = delta > 0;
            let count = delta.unsigned_abs().min(u16::MAX as u64) as u16;
            diffs.push((
                is_add,
                ItemSlot {
                    item_id: ItemId(k.item_id),
                    count,
                    durability: k.durability,
                    enhancement: k.enhancement,
                },
            ));
        }
    }
    diffs
}

fn load_or_create_config(config_dir: &Path) -> BlockLogConfig {
    let cfg_path = config_dir.join("blocklog.yml");
    if cfg_path.exists() {
        std::fs::read_to_string(&cfg_path)
            .ok()
            .and_then(|s| serde_yaml::from_str::<BlockLogConfig>(&s).ok())
            .unwrap_or_default()
    } else {
        let def = BlockLogConfig::default();
        let _ = crate::config_io::atomic_write_yaml(
            &cfg_path,
            "# BlockGame Block Log Configuration\n\
             # retention_days: auto-purge entries older than N days on server startup (0 disables).\n\
             # max_rollback_radius: maximum radius allowed for in-game /log rollback.",
            &def,
        );
        def
    }
}

#[cfg(feature = "sqlite-blocklog")]
mod sqlite_impl {
    use super::*;
    use rusqlite::{params, Connection, Row};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    enum WriterCmd {
        Block(BlockLogEvent),
        Container(ContainerLogEvent),
        MarkRolledBack {
            block_ids: Vec<i64>,
            container_ids: Vec<i64>,
            actor: String,
            reply: Sender<Result<i64, String>>,
        },
        UndoLastRollback {
            actor: String,
            reply: Sender<Result<Option<RollbackPlan>, String>>,
        },
        PurgeOlderThan {
            cutoff_ts: u64,
            reply: Sender<Result<usize, String>>,
        },
        Flush(Sender<()>),
        Shutdown(Sender<()>),
    }

    fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS block_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                actor TEXT NOT NULL,
                x INTEGER NOT NULL,
                y INTEGER NOT NULL,
                z INTEGER NOT NULL,
                old_block INTEGER NOT NULL,
                new_block INTEGER NOT NULL,
                container_snapshot TEXT,
                rollback_id INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_block_events_xyz_ts ON block_events (x, y, z, ts DESC);
            CREATE INDEX IF NOT EXISTS idx_block_events_ts ON block_events (ts DESC);
            CREATE INDEX IF NOT EXISTS idx_block_events_actor ON block_events (actor COLLATE NOCASE, ts DESC);
            CREATE INDEX IF NOT EXISTS idx_block_events_rb ON block_events (rollback_id);

            CREATE TABLE IF NOT EXISTS container_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                actor TEXT NOT NULL,
                x INTEGER NOT NULL,
                y INTEGER NOT NULL,
                z INTEGER NOT NULL,
                container_kind TEXT NOT NULL,
                action TEXT NOT NULL,
                item_id INTEGER NOT NULL,
                count INTEGER NOT NULL,
                durability INTEGER NOT NULL,
                enhancement TEXT,
                rollback_id INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_container_events_xyz_ts ON container_events (x, y, z, ts DESC);
            CREATE INDEX IF NOT EXISTS idx_container_events_ts ON container_events (ts DESC);
            CREATE INDEX IF NOT EXISTS idx_container_events_actor ON container_events (actor COLLATE NOCASE, ts DESC);
            CREATE INDEX IF NOT EXISTS idx_container_events_rb ON container_events (rollback_id);

            CREATE TABLE IF NOT EXISTS rollbacks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                actor TEXT NOT NULL,
                undone INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )
    }

    fn row_to_block_event(row: &Row<'_>) -> rusqlite::Result<BlockLogEvent> {
        let id: i64 = row.get(0)?;
        let ts: i64 = row.get(1)?;
        let actor: String = row.get(2)?;
        let x: i32 = row.get(3)?;
        let y: i32 = row.get(4)?;
        let z: i32 = row.get(5)?;
        let old_b: i64 = row.get(6)?;
        let new_b: i64 = row.get(7)?;
        let snap: Option<String> = row.get(8)?;
        let container_snapshot =
            snap.and_then(|s| serde_yaml::from_str::<Vec<(usize, ItemSlot)>>(&s).ok());
        Ok(BlockLogEvent {
            id,
            ts: ts as u64,
            actor,
            pos: IVec3::new(x, y, z),
            old_block: BlockId(old_b as u8),
            new_block: BlockId(new_b as u8),
            container_snapshot,
        })
    }

    fn row_to_container_event(row: &Row<'_>) -> rusqlite::Result<ContainerLogEvent> {
        let id: i64 = row.get(0)?;
        let ts: i64 = row.get(1)?;
        let actor: String = row.get(2)?;
        let x: i32 = row.get(3)?;
        let y: i32 = row.get(4)?;
        let z: i32 = row.get(5)?;
        let kind_str: String = row.get(6)?;
        let action: String = row.get(7)?;
        let item_id: i64 = row.get(8)?;
        let count: i64 = row.get(9)?;
        let durability: i64 = row.get(10)?;
        let enh_str: Option<String> = row.get(11)?;
        let enhancement = enh_str.and_then(|s| serde_yaml::from_str::<Enhancement>(&s).ok());
        Ok(ContainerLogEvent {
            id,
            ts: ts as u64,
            actor,
            pos: IVec3::new(x, y, z),
            container_kind: ContainerKind::from_db_str(&kind_str),
            is_add: action == "add",
            item: ItemSlot {
                item_id: ItemId(item_id as u16),
                count: count as u16,
                durability: durability as u16,
                enhancement,
            },
        })
    }

    fn flush_batch(
        conn: &mut Connection,
        blocks: &mut Vec<BlockLogEvent>,
        containers: &mut Vec<ContainerLogEvent>,
    ) {
        if blocks.is_empty() && containers.is_empty() {
            return;
        }
        let Ok(tx) = conn.transaction() else {
            blocks.clear();
            containers.clear();
            return;
        };
        {
            if let Ok(mut stmt) = tx.prepare_cached(
                "INSERT INTO block_events (ts, actor, x, y, z, old_block, new_block, container_snapshot) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            ) {
                for ev in blocks.drain(..) {
                    let snap = ev
                        .container_snapshot
                        .as_ref()
                        .and_then(|s| serde_yaml::to_string(s).ok());
                    let _ = stmt.execute(params![
                        ev.ts as i64,
                        ev.actor,
                        ev.pos.x,
                        ev.pos.y,
                        ev.pos.z,
                        ev.old_block.0 as i64,
                        ev.new_block.0 as i64,
                        snap,
                    ]);
                }
            }
            if let Ok(mut stmt) = tx.prepare_cached(
                "INSERT INTO container_events (ts, actor, x, y, z, container_kind, action, item_id, count, durability, enhancement) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            ) {
                for ev in containers.drain(..) {
                    let enh = ev
                        .item
                        .enhancement
                        .as_ref()
                        .and_then(|e| serde_yaml::to_string(e).ok());
                    let action = if ev.is_add { "add" } else { "remove" };
                    let _ = stmt.execute(params![
                        ev.ts as i64,
                        ev.actor,
                        ev.pos.x,
                        ev.pos.y,
                        ev.pos.z,
                        ev.container_kind.as_str(),
                        action,
                        ev.item.item_id.0 as i64,
                        ev.item.count as i64,
                        ev.item.durability as i64,
                        enh,
                    ]);
                }
            }
        }
        let _ = tx.commit();
    }

    fn exec_mark_rolled_back(
        conn: &mut Connection,
        block_ids: &[i64],
        container_ids: &[i64],
        actor: &str,
    ) -> Result<i64, String> {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO rollbacks (ts, actor, undone) VALUES (?1, ?2, 0)",
            params![now_unix_secs() as i64, actor],
        )
        .map_err(|e| e.to_string())?;
        let rb_id = tx.last_insert_rowid();
        {
            let mut b_stmt = tx
                .prepare("UPDATE block_events SET rollback_id = ?1 WHERE id = ?2")
                .map_err(|e| e.to_string())?;
            for &id in block_ids {
                let _ = b_stmt.execute(params![rb_id, id]);
            }
            let mut c_stmt = tx
                .prepare("UPDATE container_events SET rollback_id = ?1 WHERE id = ?2")
                .map_err(|e| e.to_string())?;
            for &id in container_ids {
                let _ = c_stmt.execute(params![rb_id, id]);
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(rb_id)
    }

    fn exec_undo_last_rollback(
        conn: &mut Connection,
        actor: &str,
    ) -> Result<Option<RollbackPlan>, String> {
        let rb_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM rollbacks WHERE LOWER(actor) = LOWER(?1) AND undone = 0 ORDER BY id DESC LIMIT 1",
                params![actor],
                |r| r.get(0),
            )
            .ok();
        let Some(rb_id) = rb_id else {
            return Ok(None);
        };

        let mut plan = RollbackPlan::default();
        {
            let mut stmt = conn
                .prepare(
                    "SELECT id, ts, actor, x, y, z, old_block, new_block, container_snapshot \
                     FROM block_events WHERE rollback_id = ?1 ORDER BY ts ASC, id ASC",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![rb_id], row_to_block_event)
                .map_err(|e| e.to_string())?;
            for ev in rows.flatten() {
                plan.block_events.push(ev);
            }
        }
        {
            let mut stmt = conn
                .prepare(
                    "SELECT id, ts, actor, x, y, z, container_kind, action, item_id, count, durability, enhancement \
                     FROM container_events WHERE rollback_id = ?1 ORDER BY ts ASC, id ASC",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![rb_id], row_to_container_event)
                .map_err(|e| e.to_string())?;
            for ev in rows.flatten() {
                plan.container_events.push(ev);
            }
        }

        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let _ = tx.execute(
            "UPDATE block_events SET rollback_id = NULL WHERE rollback_id = ?1",
            params![rb_id],
        );
        let _ = tx.execute(
            "UPDATE container_events SET rollback_id = NULL WHERE rollback_id = ?1",
            params![rb_id],
        );
        let _ = tx.execute(
            "UPDATE rollbacks SET undone = 1 WHERE id = ?1",
            params![rb_id],
        );
        tx.commit().map_err(|e| e.to_string())?;
        Ok(Some(plan))
    }

    fn exec_purge_older_than(conn: &mut Connection, cutoff_ts: u64) -> Result<usize, String> {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let b = tx
            .execute("DELETE FROM block_events WHERE ts < ?1", params![cutoff_ts as i64])
            .map_err(|e| e.to_string())?;
        let c = tx
            .execute("DELETE FROM container_events WHERE ts < ?1", params![cutoff_ts as i64])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(b + c)
    }

    fn run_writer_thread(db_path: PathBuf, rx: Receiver<WriterCmd>, retention_days: u32) {
        let Ok(mut conn) = Connection::open(&db_path) else {
            return;
        };
        if init_schema(&conn).is_err() {
            return;
        }
        if retention_days > 0 {
            let cutoff = now_unix_secs().saturating_sub(retention_days as u64 * 86_400);
            let _ = exec_purge_older_than(&mut conn, cutoff);
        }

        let mut blocks: Vec<BlockLogEvent> = Vec::with_capacity(64);
        let mut containers: Vec<ContainerLogEvent> = Vec::with_capacity(64);
        let mut last_flush = Instant::now();
        let flush_interval = Duration::from_millis(250);

        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(WriterCmd::Block(ev)) => {
                    blocks.push(ev);
                    if blocks.len() + containers.len() >= 64 {
                        flush_batch(&mut conn, &mut blocks, &mut containers);
                        last_flush = Instant::now();
                    }
                }
                Ok(WriterCmd::Container(ev)) => {
                    containers.push(ev);
                    if blocks.len() + containers.len() >= 64 {
                        flush_batch(&mut conn, &mut blocks, &mut containers);
                        last_flush = Instant::now();
                    }
                }
                Ok(WriterCmd::MarkRolledBack {
                    block_ids,
                    container_ids,
                    actor,
                    reply,
                }) => {
                    flush_batch(&mut conn, &mut blocks, &mut containers);
                    last_flush = Instant::now();
                    let _ = reply.send(exec_mark_rolled_back(
                        &mut conn,
                        &block_ids,
                        &container_ids,
                        &actor,
                    ));
                }
                Ok(WriterCmd::UndoLastRollback { actor, reply }) => {
                    flush_batch(&mut conn, &mut blocks, &mut containers);
                    last_flush = Instant::now();
                    let _ = reply.send(exec_undo_last_rollback(&mut conn, &actor));
                }
                Ok(WriterCmd::PurgeOlderThan { cutoff_ts, reply }) => {
                    flush_batch(&mut conn, &mut blocks, &mut containers);
                    last_flush = Instant::now();
                    let _ = reply.send(exec_purge_older_than(&mut conn, cutoff_ts));
                }
                Ok(WriterCmd::Flush(ack)) => {
                    flush_batch(&mut conn, &mut blocks, &mut containers);
                    last_flush = Instant::now();
                    let _ = ack.send(());
                }
                Ok(WriterCmd::Shutdown(ack)) => {
                    flush_batch(&mut conn, &mut blocks, &mut containers);
                    let _ = ack.send(());
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if last_flush.elapsed() >= flush_interval {
                        flush_batch(&mut conn, &mut blocks, &mut containers);
                        last_flush = Instant::now();
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    flush_batch(&mut conn, &mut blocks, &mut containers);
                    break;
                }
            }
        }
    }

    /// Server-side Block Log manager backed by per-world SQLite (`saves/<world>/blocklog.db`).
    pub struct BlockLogManager {
        pub config: BlockLogConfig,
        db_path: PathBuf,
        tx: Option<Sender<WriterCmd>>,
        handle: Option<JoinHandle<()>>,
    }

    impl BlockLogManager {
        /// Opens or creates `saves/<world>/blocklog.db`, loads `config/blocklog.yml`,
        /// and spawns the batched background writer thread.
        pub fn open(config_dir: &Path, world_dir: &Path) -> Self {
            let _ = fs::create_dir_all(config_dir);
            let _ = fs::create_dir_all(world_dir);
            let config = load_or_create_config(config_dir);

            let db_path = world_dir.join("blocklog.db");
            if let Ok(conn) = Connection::open(&db_path) {
                let _ = init_schema(&conn);
            }

            let (tx, rx) = mpsc::channel();
            let thread_path = db_path.clone();
            let retention_days = config.retention_days;
            let handle = thread::Builder::new()
                .name("blockgame-blocklog-writer".to_string())
                .spawn(move || run_writer_thread(thread_path, rx, retention_days))
                .ok();

            Self {
                config,
                db_path,
                tx: Some(tx),
                handle,
            }
        }

        /// Non-blocking enqueue of a player block placement or break event.
        pub fn log_block(
            &self,
            actor: &str,
            pos: IVec3,
            old_block: BlockId,
            new_block: BlockId,
            container_snapshot: Option<Vec<(usize, ItemSlot)>>,
        ) {
            if old_block == new_block {
                return;
            }
            if let Some(tx) = &self.tx {
                let _ = tx.send(WriterCmd::Block(BlockLogEvent {
                    id: 0,
                    ts: now_unix_secs(),
                    actor: actor.to_string(),
                    pos,
                    old_block,
                    new_block,
                    container_snapshot,
                }));
            }
        }

        /// Non-blocking enqueue of a container item add/remove diff.
        pub fn log_container(
            &self,
            actor: &str,
            pos: IVec3,
            container_kind: ContainerKind,
            is_add: bool,
            item: ItemSlot,
        ) {
            if item.is_empty() || item.count == 0 {
                return;
            }
            if let Some(tx) = &self.tx {
                let _ = tx.send(WriterCmd::Container(ContainerLogEvent {
                    id: 0,
                    ts: now_unix_secs(),
                    actor: actor.to_string(),
                    pos,
                    container_kind,
                    is_add,
                    item,
                }));
            }
        }

        /// Waits for all queued writes to be committed to SQLite (used before read queries & shutdown).
        pub fn flush(&self) {
            if let Some(tx) = &self.tx {
                let (ack_tx, ack_rx) = mpsc::channel();
                if tx.send(WriterCmd::Flush(ack_tx)).is_ok() {
                    let _ = ack_rx.recv_timeout(Duration::from_secs(5));
                }
            }
        }

        fn open_reader(&self) -> Result<Connection, String> {
            self.flush();
            let conn = Connection::open(&self.db_path).map_err(|e| e.to_string())?;
            let _ = conn.busy_timeout(Duration::from_millis(2000));
            Ok(conn)
        }

        /// Queries combined block + container history for `/log lookup`.
        pub fn lookup(&self, q: &BlockLogQuery) -> Result<(Vec<LogEntryView>, usize), String> {
            let conn = self.open_reader()?;
            let mut entries: Vec<LogEntryView> = Vec::new();
            let since = q.since_ts.unwrap_or(0) as i64;
            let (min_x, max_x, min_y, max_y, min_z, max_z) = match (q.center, q.radius) {
                (Some(c), Some(r)) => (c.x - r, c.x + r, c.y - r, c.y + r, c.z - r, c.z + r),
                _ => (i32::MIN, i32::MAX, i32::MIN, i32::MAX, i32::MIN, i32::MAX),
            };
            let actor_param: Option<String> = q.actor.clone();

            let include_blocks = matches!(
                q.action_filter,
                BlockLogActionFilter::All | BlockLogActionFilter::Block
            );
            let include_containers = matches!(
                q.action_filter,
                BlockLogActionFilter::All | BlockLogActionFilter::Container
            );

            if include_blocks {
                let mut stmt = conn
                    .prepare(
                        "SELECT ts, actor, x, y, z, old_block, new_block, rollback_id \
                         FROM block_events \
                         WHERE ts >= ?1 AND x BETWEEN ?2 AND ?3 AND y BETWEEN ?4 AND ?5 AND z BETWEEN ?6 AND ?7 \
                           AND (?8 IS NULL OR LOWER(actor) = LOWER(?8)) \
                         ORDER BY ts DESC, id DESC LIMIT 500",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = stmt
                    .query_map(
                        params![since, min_x, max_x, min_y, max_y, min_z, max_z, actor_param],
                        |row| {
                            let ts: i64 = row.get(0)?;
                            let actor: String = row.get(1)?;
                            let x: i32 = row.get(2)?;
                            let y: i32 = row.get(3)?;
                            let z: i32 = row.get(4)?;
                            let old_b: i64 = row.get(5)?;
                            let new_b: i64 = row.get(6)?;
                            let rb: Option<i64> = row.get(7)?;
                            Ok((ts as u64, actor, IVec3::new(x, y, z), old_b as u8, new_b as u8, rb.is_some()))
                        },
                    )
                    .map_err(|e| e.to_string())?;
                for r in rows.flatten() {
                    let (ts, actor, pos, old_b, new_b, rolled_back) = r;
                    let summary = if new_b == BlockId::AIR.0 {
                        format!("broke block #{}", old_b)
                    } else if old_b == BlockId::AIR.0 {
                        format!("placed block #{}", new_b)
                    } else {
                        format!("changed block #{} -> #{}", old_b, new_b)
                    };
                    entries.push(LogEntryView {
                        ts,
                        actor,
                        pos,
                        summary,
                        rolled_back,
                    });
                }
            }

            if include_containers {
                let mut stmt = conn
                    .prepare(
                        "SELECT ts, actor, x, y, z, container_kind, action, item_id, count, rollback_id \
                         FROM container_events \
                         WHERE ts >= ?1 AND x BETWEEN ?2 AND ?3 AND y BETWEEN ?4 AND ?5 AND z BETWEEN ?6 AND ?7 \
                           AND (?8 IS NULL OR LOWER(actor) = LOWER(?8)) \
                         ORDER BY ts DESC, id DESC LIMIT 500",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = stmt
                    .query_map(
                        params![since, min_x, max_x, min_y, max_y, min_z, max_z, actor_param],
                        |row| {
                            let ts: i64 = row.get(0)?;
                            let actor: String = row.get(1)?;
                            let x: i32 = row.get(2)?;
                            let y: i32 = row.get(3)?;
                            let z: i32 = row.get(4)?;
                            let kind: String = row.get(5)?;
                            let action: String = row.get(6)?;
                            let item_id: i64 = row.get(7)?;
                            let count: i64 = row.get(8)?;
                            let rb: Option<i64> = row.get(9)?;
                            Ok((ts as u64, actor, IVec3::new(x, y, z), kind, action, item_id as u16, count, rb.is_some()))
                        },
                    )
                    .map_err(|e| e.to_string())?;
                for r in rows.flatten() {
                    let (ts, actor, pos, kind, action, item_id, count, rolled_back) = r;
                    let verb = if action == "add" { "added" } else { "removed" };
                    let prep = if action == "add" { "to" } else { "from" };
                    entries.push(LogEntryView {
                        ts,
                        actor,
                        pos,
                        summary: format!("{} {}x item #{} {} {}", verb, count, item_id, prep, kind),
                        rolled_back,
                    });
                }
            }

            entries.sort_by(|a, b| b.ts.cmp(&a.ts));
            let total = entries.len();
            let page_size = q.page_size.max(1);
            let page = q.page.max(1);
            let start = (page - 1) * page_size;
            let slice = if start < total {
                entries[start..(start + page_size).min(total)].to_vec()
            } else {
                Vec::new()
            };
            Ok((slice, total))
        }

        /// Returns recent block & container history at an exact block coordinate (for `/log inspect`).
        pub fn inspect_coord(&self, pos: IVec3, limit: usize) -> Result<Vec<LogEntryView>, String> {
            let (entries, _) = self.lookup(&BlockLogQuery {
                center: Some(pos),
                radius: Some(0),
                page: 1,
                page_size: limit.max(1),
                ..Default::default()
            })?;
            Ok(entries)
        }

        /// Queries un-rolled-back block & container events within a rollback area & time window.
        pub fn query_rollback(
            &self,
            center: IVec3,
            radius: i32,
            since_ts: u64,
            actor: Option<&str>,
        ) -> Result<RollbackPlan, String> {
            let conn = self.open_reader()?;
            let mut plan = RollbackPlan::default();
            let actor_param: Option<String> = actor.map(|a| a.to_string());

            {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, ts, actor, x, y, z, old_block, new_block, container_snapshot \
                         FROM block_events \
                         WHERE rollback_id IS NULL AND ts >= ?1 \
                           AND x BETWEEN ?2 AND ?3 AND y BETWEEN ?4 AND ?5 AND z BETWEEN ?6 AND ?7 \
                           AND (?8 IS NULL OR LOWER(actor) = LOWER(?8)) \
                         ORDER BY ts DESC, id DESC",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = stmt
                    .query_map(
                        params![
                            since_ts as i64,
                            center.x - radius,
                            center.x + radius,
                            center.y - radius,
                            center.y + radius,
                            center.z - radius,
                            center.z + radius,
                            actor_param,
                        ],
                        row_to_block_event,
                    )
                    .map_err(|e| e.to_string())?;
                for ev in rows.flatten() {
                    plan.block_events.push(ev);
                }
            }

            {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, ts, actor, x, y, z, container_kind, action, item_id, count, durability, enhancement \
                         FROM container_events \
                         WHERE rollback_id IS NULL AND ts >= ?1 \
                           AND x BETWEEN ?2 AND ?3 AND y BETWEEN ?4 AND ?5 AND z BETWEEN ?6 AND ?7 \
                           AND (?8 IS NULL OR LOWER(actor) = LOWER(?8)) \
                         ORDER BY ts DESC, id DESC",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = stmt
                    .query_map(
                        params![
                            since_ts as i64,
                            center.x - radius,
                            center.x + radius,
                            center.y - radius,
                            center.y + radius,
                            center.z - radius,
                            center.z + radius,
                            actor_param,
                        ],
                        row_to_container_event,
                    )
                    .map_err(|e| e.to_string())?;
                for ev in rows.flatten() {
                    plan.container_events.push(ev);
                }
            }

            Ok(plan)
        }

        /// Records a rollback record and marks affected events via the background writer thread.
        pub fn mark_rolled_back(
            &self,
            block_ids: &[i64],
            container_ids: &[i64],
            actor: &str,
        ) -> Result<i64, String> {
            let Some(tx) = &self.tx else {
                return Err("Block Log writer thread is not running.".to_string());
            };
            let (reply_tx, reply_rx) = mpsc::channel();
            tx.send(WriterCmd::MarkRolledBack {
                block_ids: block_ids.to_vec(),
                container_ids: container_ids.to_vec(),
                actor: actor.to_string(),
                reply: reply_tx,
            })
            .map_err(|e| e.to_string())?;
            reply_rx
                .recv_timeout(Duration::from_secs(5))
                .map_err(|e| e.to_string())?
        }

        /// Undoes the most recent un-undone rollback performed by `actor` via the background writer thread.
        pub fn undo_last_rollback(&self, actor: &str) -> Result<Option<RollbackPlan>, String> {
            let Some(tx) = &self.tx else {
                return Err("Block Log writer thread is not running.".to_string());
            };
            let (reply_tx, reply_rx) = mpsc::channel();
            tx.send(WriterCmd::UndoLastRollback {
                actor: actor.to_string(),
                reply: reply_tx,
            })
            .map_err(|e| e.to_string())?;
            reply_rx
                .recv_timeout(Duration::from_secs(5))
                .map_err(|e| e.to_string())?
        }

        /// Deletes block and container log entries older than `cutoff_ts` via the background writer thread.
        pub fn purge_older_than(&self, cutoff_ts: u64) -> Result<usize, String> {
            let Some(tx) = &self.tx else {
                return Err("Block Log writer thread is not running.".to_string());
            };
            let (reply_tx, reply_rx) = mpsc::channel();
            tx.send(WriterCmd::PurgeOlderThan {
                cutoff_ts,
                reply: reply_tx,
            })
            .map_err(|e| e.to_string())?;
            reply_rx
                .recv_timeout(Duration::from_secs(5))
                .map_err(|e| e.to_string())?
        }

        pub fn shutdown(&mut self) {
            if let Some(tx) = self.tx.take() {
                let (ack_tx, ack_rx) = mpsc::channel();
                if tx.send(WriterCmd::Shutdown(ack_tx)).is_ok() {
                    let _ = ack_rx.recv_timeout(Duration::from_secs(5));
                }
            }
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    impl Drop for BlockLogManager {
        fn drop(&mut self) {
            self.shutdown();
        }
    }
}

#[cfg(not(feature = "sqlite-blocklog"))]
mod stub_impl {
    use super::*;

    /// No-op Block Log stub used when `sqlite-blocklog` is disabled (client / WASM builds).
    pub struct BlockLogManager {
        pub config: BlockLogConfig,
    }

    impl BlockLogManager {
        pub fn open(config_dir: &Path, _world_dir: &Path) -> Self {
            Self {
                config: load_or_create_config(config_dir),
            }
        }

        pub fn log_block(
            &self,
            _actor: &str,
            _pos: IVec3,
            _old_block: BlockId,
            _new_block: BlockId,
            _container_snapshot: Option<Vec<(usize, ItemSlot)>>,
        ) {
        }

        pub fn log_container(
            &self,
            _actor: &str,
            _pos: IVec3,
            _container_kind: ContainerKind,
            _is_add: bool,
            _item: ItemSlot,
        ) {
        }

        pub fn flush(&self) {}

        pub fn lookup(&self, _q: &BlockLogQuery) -> Result<(Vec<LogEntryView>, usize), String> {
            Err("Block Log requires the `sqlite-blocklog` feature (enabled in blockgame_server).".to_string())
        }

        pub fn inspect_coord(&self, _pos: IVec3, _limit: usize) -> Result<Vec<LogEntryView>, String> {
            Err("Block Log requires the `sqlite-blocklog` feature (enabled in blockgame_server).".to_string())
        }

        pub fn query_rollback(
            &self,
            _center: IVec3,
            _radius: i32,
            _since_ts: u64,
            _actor: Option<&str>,
        ) -> Result<RollbackPlan, String> {
            Err("Block Log requires the `sqlite-blocklog` feature (enabled in blockgame_server).".to_string())
        }

        pub fn mark_rolled_back(
            &self,
            _block_ids: &[i64],
            _container_ids: &[i64],
            _actor: &str,
        ) -> Result<i64, String> {
            Err("Block Log requires the `sqlite-blocklog` feature (enabled in blockgame_server).".to_string())
        }

        pub fn undo_last_rollback(&self, _actor: &str) -> Result<Option<RollbackPlan>, String> {
            Err("Block Log requires the `sqlite-blocklog` feature (enabled in blockgame_server).".to_string())
        }

        pub fn purge_older_than(&self, _cutoff_ts: u64) -> Result<usize, String> {
            Err("Block Log requires the `sqlite-blocklog` feature (enabled in blockgame_server).".to_string())
        }

        pub fn shutdown(&mut self) {}
    }
}

#[cfg(feature = "sqlite-blocklog")]
pub use sqlite_impl::BlockLogManager;

#[cfg(not(feature = "sqlite-blocklog"))]
pub use stub_impl::BlockLogManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_slot_diff_ignores_shuffles_and_preserves_enhancements() {
        let mut sword_keen = ItemSlot::new(ItemId::DIAMOND_SWORD, 1);
        sword_keen.durability = 120;
        sword_keen.enhancement = Some(Enhancement::Keen);

        let mut sword_reinf = ItemSlot::new(ItemId::DIAMOND_SWORD, 1);
        sword_reinf.durability = 120;
        sword_reinf.enhancement = Some(Enhancement::Reinforced);

        let old = vec![
            ItemSlot::new(ItemId::PLANKS, 16),
            sword_keen,
            sword_reinf,
            ItemSlot::EMPTY,
        ];
        // Shuffle planks to slot 2, remove Keen sword, keep Reinforced sword, add 4 apples
        let new = vec![
            ItemSlot::new(ItemId::APPLE, 4),
            sword_reinf,
            ItemSlot::new(ItemId::PLANKS, 16),
            ItemSlot::EMPTY,
        ];

        let diffs = diff_container_slots(&old, &new);
        assert_eq!(diffs.len(), 2);
        assert!(diffs.iter().any(|(add, s)| {
            !*add
                && s.item_id == ItemId::DIAMOND_SWORD
                && s.durability == 120
                && s.enhancement == Some(Enhancement::Keen)
        }));
        assert!(diffs
            .iter()
            .any(|(add, s)| *add && s.item_id == ItemId::APPLE && s.count == 4));
    }

    #[cfg(feature = "sqlite-blocklog")]
    #[test]
    fn sqlite_async_write_lookup_rollback_undo_and_purge() {
        let root = std::env::temp_dir().join(format!(
            "blockgame_blocklog_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let cfg_dir = root.join("config");
        let world_dir = root.join("world");

        let mut mgr = BlockLogManager::open(&cfg_dir, &world_dir);
        assert!(cfg_dir.join("blocklog.yml").exists());
        assert!(world_dir.join("blocklog.db").exists());

        let pos = IVec3::new(10, 64, -5);
        mgr.log_block("Alice", pos, BlockId::AIR, BlockId::CHEST, None);
        mgr.log_container(
            "Alice",
            pos,
            ContainerKind::Chest,
            true,
            ItemSlot::new(ItemId::DIAMOND, 3),
        );
        mgr.log_block(
            "Bob",
            pos,
            BlockId::CHEST,
            BlockId::AIR,
            Some(vec![(0, ItemSlot::new(ItemId::DIAMOND, 3))]),
        );

        let (entries, total) = mgr
            .lookup(&BlockLogQuery {
                center: Some(pos),
                radius: Some(5),
                page: 1,
                page_size: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(total, 3);
        assert_eq!(entries.len(), 3);

        // Actor filter pushed to SQL WHERE
        let (bob_entries, bob_total) = mgr
            .lookup(&BlockLogQuery {
                actor: Some("bob".to_string()),
                page: 1,
                page_size: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(bob_total, 1);
        assert_eq!(bob_entries[0].actor, "Bob");

        // Rollback only Bob's grief
        let plan = mgr.query_rollback(pos, 5, 0, Some("Bob")).unwrap();
        assert_eq!(plan.block_events.len(), 1);
        assert_eq!(plan.block_events[0].old_block, BlockId::CHEST);
        assert!(plan.block_events[0].container_snapshot.is_some());

        let b_ids: Vec<i64> = plan.block_events.iter().map(|e| e.id).collect();
        let rb_id = mgr.mark_rolled_back(&b_ids, &[], "Admin").unwrap();
        assert!(rb_id > 0);

        // Undo restores the rollback
        let undone = mgr.undo_last_rollback("Admin").unwrap().unwrap();
        assert_eq!(undone.block_events.len(), 1);

        // Purge everything in the future clears all rows
        let purged = mgr.purge_older_than(now_unix_secs() + 1000).unwrap();
        assert_eq!(purged, 3);

        mgr.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }
}
