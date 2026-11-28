use glam::IVec3;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::block::BlockId;
use crate::item::{Enhancement, ItemId, ItemSlot};

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Configuration stored in `config/blocklog.yml` (ADR-0012 & ADR-0014).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockLogConfig {
    /// Days to retain logs (`0` = keep forever).
    #[serde(default)]
    pub retention_days: u64,
    /// Maximum rollback radius allowed for in-game admins.
    #[serde(default = "default_max_rollback_radius")]
    pub max_rollback_radius: i32,
}

fn default_max_rollback_radius() -> i32 {
    100
}

impl Default for BlockLogConfig {
    fn default() -> Self {
        Self {
            retention_days: 0,
            max_rollback_radius: default_max_rollback_radius(),
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
    pub container_kind: String,
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
    /// `None` = both, `Some("block")` = blocks only, `Some("container")` = containers only.
    pub action_filter: Option<String>,
    pub page: usize,
    pub page_size: usize,
}

/// Concrete set of block & container events to revert (or replay on `/log undo`).
#[derive(Debug, Clone, Default)]
pub struct RollbackPlan {
    pub block_events: Vec<BlockLogEvent>,
    pub container_events: Vec<ContainerLogEvent>,
}

enum WriterCmd {
    Block(BlockLogEvent),
    Container(ContainerLogEvent),
    Flush(Sender<()>),
    Shutdown(Sender<()>),
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
    keys.sort_by_key(|k| (k.item_id, k.durability));
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
        CREATE INDEX IF NOT EXISTS idx_block_xyz_ts ON block_events(x, y, z, ts DESC);
        CREATE INDEX IF NOT EXISTS idx_block_ts ON block_events(ts DESC);
        CREATE INDEX IF NOT EXISTS idx_block_actor_ts ON block_events(actor, ts DESC);
        CREATE INDEX IF NOT EXISTS idx_block_rb ON block_events(rollback_id);

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
            enhancement_yaml TEXT,
            rollback_id INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_cont_xyz_ts ON container_events(x, y, z, ts DESC);
        CREATE INDEX IF NOT EXISTS idx_cont_ts ON container_events(ts DESC);
        CREATE INDEX IF NOT EXISTS idx_cont_actor_ts ON container_events(actor, ts DESC);
        CREATE INDEX IF NOT EXISTS idx_cont_rb ON container_events(rollback_id);

        CREATE TABLE IF NOT EXISTS rollbacks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL,
            actor TEXT NOT NULL,
            undone INTEGER NOT NULL DEFAULT 0
        );
        "#,
    )
}

fn flush_batch(
    conn: &mut Connection,
    blocks: &mut Vec<BlockLogEvent>,
    containers: &mut Vec<ContainerLogEvent>,
) {
    if blocks.is_empty() && containers.is_empty() {
        return;
    }
    if let Ok(tx) = conn.transaction() {
        if !blocks.is_empty() {
            if let Ok(mut stmt) = tx.prepare_cached(
                "INSERT INTO block_events (ts, actor, x, y, z, old_block, new_block, container_snapshot)
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
        }
        if !containers.is_empty() {
            if let Ok(mut stmt) = tx.prepare_cached(
                "INSERT INTO container_events (ts, actor, x, y, z, container_kind, action, item_id, count, durability, enhancement_yaml)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            ) {
                for ev in containers.drain(..) {
                    let enh = ev
                        .item
                        .enhancement
                        .as_ref()
                        .and_then(|e| serde_yaml::to_string(e).ok());
                    let _ = stmt.execute(params![
                        ev.ts as i64,
                        ev.actor,
                        ev.pos.x,
                        ev.pos.y,
                        ev.pos.z,
                        ev.container_kind,
                        if ev.is_add { "add" } else { "remove" },
                        ev.item.item_id.0 as i64,
                        ev.item.count as i64,
                        ev.item.durability as i64,
                        enh,
                    ]);
                }
            }
        }
        let _ = tx.commit();
    } else {
        blocks.clear();
        containers.clear();
    }
}

fn writer_thread_main(db_path: PathBuf, rx: Receiver<WriterCmd>) {
    let Ok(mut conn) = Connection::open(&db_path) else {
        log::error!("Failed to open Block Log SQLite DB at '{}'", db_path.display());
        return;
    };
    let _ = init_schema(&conn);

    let mut block_batch: Vec<BlockLogEvent> = Vec::with_capacity(128);
    let mut cont_batch: Vec<ContainerLogEvent> = Vec::with_capacity(128);

    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(WriterCmd::Block(ev)) => {
                block_batch.push(ev);
                if block_batch.len() >= 128 {
                    flush_batch(&mut conn, &mut block_batch, &mut cont_batch);
                }
            }
            Ok(WriterCmd::Container(ev)) => {
                cont_batch.push(ev);
                if cont_batch.len() >= 128 {
                    flush_batch(&mut conn, &mut block_batch, &mut cont_batch);
                }
            }
            Ok(WriterCmd::Flush(ack)) => {
                flush_batch(&mut conn, &mut block_batch, &mut cont_batch);
                let _ = ack.send(());
            }
            Ok(WriterCmd::Shutdown(ack)) => {
                flush_batch(&mut conn, &mut block_batch, &mut cont_batch);
                let _ = ack.send(());
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                flush_batch(&mut conn, &mut block_batch, &mut cont_batch);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                flush_batch(&mut conn, &mut block_batch, &mut cont_batch);
                break;
            }
        }
    }
}

/// Per-world SQLite Block Log manager with non-blocking batched background writer (ADR-0014).
pub struct BlockLogManager {
    pub config: BlockLogConfig,
    db_path: PathBuf,
    tx: Option<Sender<WriterCmd>>,
    handle: Option<JoinHandle<()>>,
}

impl BlockLogManager {
    /// Opens `saves/<world_name>/blocklog.db` and loads `config/blocklog.yml`.
    pub fn open(config_dir: &Path, world_dir: &Path) -> Self {
        let cfg_path = config_dir.join("blocklog.yml");
        let config = if cfg_path.exists() {
            fs::read_to_string(&cfg_path)
                .ok()
                .and_then(|s| serde_yaml::from_str::<BlockLogConfig>(&s).ok())
                .unwrap_or_default()
        } else {
            let def = BlockLogConfig::default();
            let _ = crate::config_io::atomic_write_yaml(
                &cfg_path,
                "# BlockGame Block Log Configuration (retention_days: 0 = keep forever)",
                &def,
            );
            def
        };

        let _ = fs::create_dir_all(world_dir);
        let db_path = world_dir.join("blocklog.db");

        // Ensure schema exists synchronously on startup before spawning background writer
        if let Ok(conn) = Connection::open(&db_path) {
            let _ = init_schema(&conn);
        }

        let (tx, rx) = mpsc::channel();
        let db_clone = db_path.clone();
        let handle = thread::Builder::new()
            .name("blockgame-blocklog-writer".to_string())
            .spawn(move || writer_thread_main(db_clone, rx))
            .ok();

        let mgr = Self {
            config,
            db_path,
            tx: Some(tx),
            handle,
        };

        if mgr.config.retention_days > 0 {
            let cutoff = now_unix_secs().saturating_sub(mgr.config.retention_days * 86_400);
            let _ = mgr.purge_older_than(cutoff);
        }

        mgr
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

    /// Non-blocking enqueue of a container item add/remove diff event.
    pub fn log_container(
        &self,
        actor: &str,
        pos: IVec3,
        container_kind: &str,
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
                container_kind: container_kind.to_string(),
                is_add,
                item,
            }));
        }
    }

    /// Synchronously flushes any buffered events in the background writer thread.
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
        Connection::open(&self.db_path).map_err(|e| format!("Block Log DB error: {}", e))
    }

    /// Queries Block Log entries matching `query` with pagination.
    pub fn lookup(&self, query: &BlockLogQuery) -> Result<(Vec<LogEntryView>, usize), String> {
        let conn = self.open_reader()?;
        let mut all: Vec<LogEntryView> = Vec::new();
        let include_blocks = query
            .action_filter
            .as_deref()
            .map(|f| f.eq_ignore_ascii_case("block"))
            .unwrap_or(true);
        let include_containers = query
            .action_filter
            .as_deref()
            .map(|f| f.eq_ignore_ascii_case("container"))
            .unwrap_or(true);

        let min_ts = query.since_ts.unwrap_or(0) as i64;
        let actor_lower = query.actor.as_ref().map(|a| a.to_ascii_lowercase());
        let (min_x, max_x, min_y, max_y, min_z, max_z) = match (query.center, query.radius) {
            (Some(c), Some(r)) => (c.x - r, c.x + r, c.y - r, c.y + r, c.z - r, c.z + r),
            _ => (i32::MIN, i32::MAX, i32::MIN, i32::MAX, i32::MIN, i32::MAX),
        };

        if include_blocks {
            let mut stmt = conn
                .prepare(
                    "SELECT ts, actor, x, y, z, old_block, new_block, rollback_id
                     FROM block_events
                     WHERE ts >= ?1 AND x BETWEEN ?2 AND ?3 AND y BETWEEN ?4 AND ?5 AND z BETWEEN ?6 AND ?7
                     ORDER BY ts DESC, id DESC LIMIT 500",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(
                    params![min_ts, min_x, max_x, min_y, max_y, min_z, max_z],
                    |row| {
                        let ts: i64 = row.get(0)?;
                        let actor: String = row.get(1)?;
                        let x: i32 = row.get(2)?;
                        let y: i32 = row.get(3)?;
                        let z: i32 = row.get(4)?;
                        let old_b: i64 = row.get(5)?;
                        let new_b: i64 = row.get(6)?;
                        let rb: Option<i64> = row.get(7)?;
                        Ok((ts as u64, actor, IVec3::new(x, y, z), BlockId(old_b as u8), BlockId(new_b as u8), rb.is_some()))
                    },
                )
                .map_err(|e| e.to_string())?;
            for r in rows.flatten() {
                if let Some(ref want) = actor_lower {
                    if !r.1.to_ascii_lowercase().eq(want) {
                        continue;
                    }
                }
                let verb = if r.4.is_air() {
                    format!("broke block #{}", r.3.0)
                } else if r.3.is_air() {
                    format!("placed block #{}", r.4.0)
                } else {
                    format!("changed block #{} -> #{}", r.3.0, r.4.0)
                };
                all.push(LogEntryView {
                    ts: r.0,
                    actor: r.1,
                    pos: r.2,
                    summary: verb,
                    rolled_back: r.5,
                });
            }
        }

        if include_containers {
            let mut stmt = conn
                .prepare(
                    "SELECT ts, actor, x, y, z, container_kind, action, item_id, count, rollback_id
                     FROM container_events
                     WHERE ts >= ?1 AND x BETWEEN ?2 AND ?3 AND y BETWEEN ?4 AND ?5 AND z BETWEEN ?6 AND ?7
                     ORDER BY ts DESC, id DESC LIMIT 500",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(
                    params![min_ts, min_x, max_x, min_y, max_y, min_z, max_z],
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
                        Ok((ts as u64, actor, IVec3::new(x, y, z), kind, action, item_id, count, rb.is_some()))
                    },
                )
                .map_err(|e| e.to_string())?;
            for r in rows.flatten() {
                if let Some(ref want) = actor_lower {
                    if !r.1.to_ascii_lowercase().eq(want) {
                        continue;
                    }
                }
                let verb = if r.4 == "add" {
                    format!("added {}x item #{} to {}", r.6, r.5, r.3)
                } else {
                    format!("took {}x item #{} from {}", r.6, r.5, r.3)
                };
                all.push(LogEntryView {
                    ts: r.0,
                    actor: r.1,
                    pos: r.2,
                    summary: verb,
                    rolled_back: r.7,
                });
            }
        }

        all.sort_by(|a, b| b.ts.cmp(&a.ts));
        let total = all.len();
        let page_size = query.page_size.max(1);
        let page = query.page.max(1);
        let skip = (page - 1) * page_size;
        let slice = all.into_iter().skip(skip).take(page_size).collect();
        Ok((slice, total))
    }

    /// Returns recent Block Log history at an exact block/container coordinate for Inspect Mode.
    pub fn inspect_coord(&self, pos: IVec3, limit: usize) -> Result<Vec<LogEntryView>, String> {
        let q = BlockLogQuery {
            center: Some(pos),
            radius: Some(0),
            page: 1,
            page_size: limit,
            ..Default::default()
        };
        Ok(self.lookup(&q)?.0)
    }

    /// Queries all non-rolled-back events in reverse chronological order for `/log rollback`.
    pub fn query_rollback(
        &self,
        center: IVec3,
        radius: i32,
        since_ts: u64,
        player: Option<&str>,
    ) -> Result<RollbackPlan, String> {
        let conn = self.open_reader()?;
        let actor_lower = player.map(|p| p.to_ascii_lowercase());
        let (min_x, max_x) = (center.x - radius, center.x + radius);
        let (min_y, max_y) = (center.y - radius, center.y + radius);
        let (min_z, max_z) = (center.z - radius, center.z + radius);

        let mut plan = RollbackPlan::default();

        {
            let mut stmt = conn
                .prepare(
                    "SELECT id, ts, actor, x, y, z, old_block, new_block, container_snapshot
                     FROM block_events
                     WHERE rollback_id IS NULL AND ts >= ?1
                       AND x BETWEEN ?2 AND ?3 AND y BETWEEN ?4 AND ?5 AND z BETWEEN ?6 AND ?7
                     ORDER BY id DESC",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(
                    params![since_ts as i64, min_x, max_x, min_y, max_y, min_z, max_z],
                    |row| {
                        let id: i64 = row.get(0)?;
                        let ts: i64 = row.get(1)?;
                        let actor: String = row.get(2)?;
                        let x: i32 = row.get(3)?;
                        let y: i32 = row.get(4)?;
                        let z: i32 = row.get(5)?;
                        let old_b: i64 = row.get(6)?;
                        let new_b: i64 = row.get(7)?;
                        let snap_str: Option<String> = row.get(8)?;
                        let container_snapshot = snap_str
                            .and_then(|s| serde_yaml::from_str::<Vec<(usize, ItemSlot)>>(&s).ok());
                        Ok(BlockLogEvent {
                            id,
                            ts: ts as u64,
                            actor,
                            pos: IVec3::new(x, y, z),
                            old_block: BlockId(old_b as u8),
                            new_block: BlockId(new_b as u8),
                            container_snapshot,
                        })
                    },
                )
                .map_err(|e| e.to_string())?;
            for ev in rows.flatten() {
                if let Some(ref want) = actor_lower {
                    if !ev.actor.to_ascii_lowercase().eq(want) {
                        continue;
                    }
                }
                plan.block_events.push(ev);
            }
        }

        {
            let mut stmt = conn
                .prepare(
                    "SELECT id, ts, actor, x, y, z, container_kind, action, item_id, count, durability, enhancement_yaml
                     FROM container_events
                     WHERE rollback_id IS NULL AND ts >= ?1
                       AND x BETWEEN ?2 AND ?3 AND y BETWEEN ?4 AND ?5 AND z BETWEEN ?6 AND ?7
                     ORDER BY id DESC",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(
                    params![since_ts as i64, min_x, max_x, min_y, max_y, min_z, max_z],
                    |row| {
                        let id: i64 = row.get(0)?;
                        let ts: i64 = row.get(1)?;
                        let actor: String = row.get(2)?;
                        let x: i32 = row.get(3)?;
                        let y: i32 = row.get(4)?;
                        let z: i32 = row.get(5)?;
                        let container_kind: String = row.get(6)?;
                        let action: String = row.get(7)?;
                        let item_id: i64 = row.get(8)?;
                        let count: i64 = row.get(9)?;
                        let durability: i64 = row.get(10)?;
                        let enh_str: Option<String> = row.get(11)?;
                        let enhancement = enh_str
                            .and_then(|s| serde_yaml::from_str::<Enhancement>(&s).ok());
                        Ok(ContainerLogEvent {
                            id,
                            ts: ts as u64,
                            actor,
                            pos: IVec3::new(x, y, z),
                            container_kind,
                            is_add: action == "add",
                            item: ItemSlot {
                                item_id: ItemId(item_id as u16),
                                count: count as u16,
                                durability: durability as u16,
                                enhancement,
                            },
                        })
                    },
                )
                .map_err(|e| e.to_string())?;
            for ev in rows.flatten() {
                if let Some(ref want) = actor_lower {
                    if !ev.actor.to_ascii_lowercase().eq(want) {
                        continue;
                    }
                }
                plan.container_events.push(ev);
            }
        }

        Ok(plan)
    }

    /// Marks the given block and container events as rolled back under a new `rollback_id`.
    pub fn mark_rolled_back(
        &self,
        block_ids: &[i64],
        container_ids: &[i64],
        actor: &str,
    ) -> Result<i64, String> {
        let mut conn = self.open_reader()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO rollbacks (ts, actor, undone) VALUES (?1, ?2, 0)",
            params![now_unix_secs() as i64, actor],
        )
        .map_err(|e| e.to_string())?;
        let rb_id = tx.last_insert_rowid();

        {
            let mut stmt = tx
                .prepare("UPDATE block_events SET rollback_id = ?1 WHERE id = ?2")
                .map_err(|e| e.to_string())?;
            for &id in block_ids {
                let _ = stmt.execute(params![rb_id, id]);
            }
        }
        {
            let mut stmt = tx
                .prepare("UPDATE container_events SET rollback_id = ?1 WHERE id = ?2")
                .map_err(|e| e.to_string())?;
            for &id in container_ids {
                let _ = stmt.execute(params![rb_id, id]);
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(rb_id)
    }

    /// Finds `actor`'s most recent non-undone rollback, returns its events in forward chronological
    /// order so the caller can re-apply them, and clears `rollback_id` on those rows.
    pub fn undo_last_rollback(&self, actor: &str) -> Result<Option<RollbackPlan>, String> {
        let mut conn = self.open_reader()?;
        let rb_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM rollbacks WHERE LOWER(actor) = LOWER(?1) AND undone = 0 ORDER BY id DESC LIMIT 1",
                params![actor],
                |row| row.get(0),
            )
            .ok();
        let Some(rb_id) = rb_id else {
            return Ok(None);
        };

        let mut plan = RollbackPlan::default();
        {
            let mut stmt = conn
                .prepare(
                    "SELECT id, ts, actor, x, y, z, old_block, new_block, container_snapshot
                     FROM block_events WHERE rollback_id = ?1 ORDER BY id ASC",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![rb_id], |row| {
                    let id: i64 = row.get(0)?;
                    let ts: i64 = row.get(1)?;
                    let actor: String = row.get(2)?;
                    let x: i32 = row.get(3)?;
                    let y: i32 = row.get(4)?;
                    let z: i32 = row.get(5)?;
                    let old_b: i64 = row.get(6)?;
                    let new_b: i64 = row.get(7)?;
                    let snap_str: Option<String> = row.get(8)?;
                    let container_snapshot = snap_str
                        .and_then(|s| serde_yaml::from_str::<Vec<(usize, ItemSlot)>>(&s).ok());
                    Ok(BlockLogEvent {
                        id,
                        ts: ts as u64,
                        actor,
                        pos: IVec3::new(x, y, z),
                        old_block: BlockId(old_b as u8),
                        new_block: BlockId(new_b as u8),
                        container_snapshot,
                    })
                })
                .map_err(|e| e.to_string())?;
            for ev in rows.flatten() {
                plan.block_events.push(ev);
            }
        }
        {
            let mut stmt = conn
                .prepare(
                    "SELECT id, ts, actor, x, y, z, container_kind, action, item_id, count, durability, enhancement_yaml
                     FROM container_events WHERE rollback_id = ?1 ORDER BY id ASC",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![rb_id], |row| {
                    let id: i64 = row.get(0)?;
                    let ts: i64 = row.get(1)?;
                    let actor: String = row.get(2)?;
                    let x: i32 = row.get(3)?;
                    let y: i32 = row.get(4)?;
                    let z: i32 = row.get(5)?;
                    let container_kind: String = row.get(6)?;
                    let action: String = row.get(7)?;
                    let item_id: i64 = row.get(8)?;
                    let count: i64 = row.get(9)?;
                    let durability: i64 = row.get(10)?;
                    let enh_str: Option<String> = row.get(11)?;
                    let enhancement =
                        enh_str.and_then(|s| serde_yaml::from_str::<Enhancement>(&s).ok());
                    Ok(ContainerLogEvent {
                        id,
                        ts: ts as u64,
                        actor,
                        pos: IVec3::new(x, y, z),
                        container_kind,
                        is_add: action == "add",
                        item: ItemSlot {
                            item_id: ItemId(item_id as u16),
                            count: count as u16,
                            durability: durability as u16,
                            enhancement,
                        },
                    })
                })
                .map_err(|e| e.to_string())?;
            for ev in rows.flatten() {
                plan.container_events.push(ev);
            }
        }

        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let _ = tx.execute("UPDATE block_events SET rollback_id = NULL WHERE rollback_id = ?1", params![rb_id]);
        let _ = tx.execute("UPDATE container_events SET rollback_id = NULL WHERE rollback_id = ?1", params![rb_id]);
        let _ = tx.execute("UPDATE rollbacks SET undone = 1 WHERE id = ?1", params![rb_id]);
        tx.commit().map_err(|e| e.to_string())?;
        Ok(Some(plan))
    }

    /// Deletes block and container log entries older than `cutoff_ts`.
    pub fn purge_older_than(&self, cutoff_ts: u64) -> Result<usize, String> {
        let conn = self.open_reader()?;
        let b = conn
            .execute("DELETE FROM block_events WHERE ts < ?1", params![cutoff_ts as i64])
            .map_err(|e| e.to_string())?;
        let c = conn
            .execute("DELETE FROM container_events WHERE ts < ?1", params![cutoff_ts as i64])
            .map_err(|e| e.to_string())?;
        Ok(b + c)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_slot_diff_ignores_shuffles_and_preserves_enhancements() {
        let mut sword = ItemSlot::new(ItemId::DIAMOND_SWORD, 1);
        sword.durability = 120;
        sword.enhancement = Some(Enhancement::Keen);

        let old = vec![ItemSlot::new(ItemId::PLANKS, 16), sword, ItemSlot::EMPTY];
        // Shuffle planks to slot 2, remove sword, add 4 apples
        let new = vec![
            ItemSlot::new(ItemId::APPLE, 4),
            ItemSlot::EMPTY,
            ItemSlot::new(ItemId::PLANKS, 16),
        ];

        let diffs = diff_container_slots(&old, &new);
        assert_eq!(diffs.len(), 2);
        assert!(diffs.iter().any(|(add, s)| !*add && s.item_id == ItemId::DIAMOND_SWORD && s.durability == 120 && s.enhancement == Some(Enhancement::Keen)));
        assert!(diffs.iter().any(|(add, s)| *add && s.item_id == ItemId::APPLE && s.count == 4));
    }

    #[test]
    fn sqlite_async_write_lookup_rollback_undo_and_purge() {
        let root = std::env::temp_dir().join(format!("blockgame_blocklog_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let cfg_dir = root.join("config");
        let world_dir = root.join("world");

        let mut mgr = BlockLogManager::open(&cfg_dir, &world_dir);
        assert!(cfg_dir.join("blocklog.yml").exists());
        assert!(world_dir.join("blocklog.db").exists());

        let pos = IVec3::new(10, 64, 10);
        mgr.log_block("Griefer", pos, BlockId::STONE, BlockId::AIR, None);
        mgr.log_container("Griefer", IVec3::new(11, 64, 10), "chest", false, ItemSlot::new(ItemId::DIAMOND, 3));
        mgr.flush();

        let (entries, total) = mgr
            .lookup(&BlockLogQuery {
                actor: Some("Griefer".to_string()),
                center: Some(pos),
                radius: Some(5),
                page: 1,
                page_size: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(total, 2);
        assert_eq!(entries.len(), 2);

        let plan = mgr.query_rollback(pos, 5, 0, Some("Griefer")).unwrap();
        assert_eq!(plan.block_events.len(), 1);
        assert_eq!(plan.container_events.len(), 1);

        let b_ids: Vec<i64> = plan.block_events.iter().map(|e| e.id).collect();
        let c_ids: Vec<i64> = plan.container_events.iter().map(|e| e.id).collect();
        mgr.mark_rolled_back(&b_ids, &c_ids, "Admin").unwrap();

        // Already rolled back -> second rollback query returns empty
        let empty_plan = mgr.query_rollback(pos, 5, 0, Some("Griefer")).unwrap();
        assert!(empty_plan.block_events.is_empty());

        // Undo restores them
        let undone = mgr.undo_last_rollback("Admin").unwrap().expect("Should undo rollback");
        assert_eq!(undone.block_events.len(), 1);
        assert_eq!(undone.container_events.len(), 1);

        // Purge everything (future cutoff)
        let purged = mgr.purge_older_than(now_unix_secs() + 100).unwrap();
        assert_eq!(purged, 2);

        mgr.shutdown();
        let _ = fs::remove_dir_all(&root);
    }
}
