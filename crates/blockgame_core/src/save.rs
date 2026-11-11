use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use glam::{IVec2, IVec3, Vec3};

use crate::{
    animal::{AnimalEntity, AnimalKind},
    block::BlockId,
    chunk::{Chunk, ChunkPos, CHUNK_VOLUME},
    entity::{FallingBlock, ItemDrop},
    item::{Inventory, ItemId, ItemSlot},
    player::PlayerStats,
    world::{FurnaceState, World},
};

const SAVE_MAGIC: &[u8; 4] = b"BGSV";
const SAVE_VERSION: u32 = 2;

/// Summary metadata for a saved world displayed in the Singleplayer World Selection list.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldMetadata {
    pub name: String,
    pub folder_name: String,
    pub seed: u32,
    pub last_played: u64,
    pub total_time: f32,
    pub day_night_time: f32,
}

impl WorldMetadata {
    pub fn new(name: impl Into<String>, seed: u32) -> Self {
        let name_str = name.into();
        let folder_name = sanitize_folder_name(&name_str);
        let now = current_unix_timestamp();
        Self {
            name: name_str,
            folder_name,
            seed,
            last_played: now,
            total_time: 0.0,
            day_night_time: 0.25, // Morning start
        }
    }
}

/// Complete player state persisted across sessions.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedPlayerState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub stats: PlayerStats,
    pub inventory: Inventory,
    pub active_hotbar_slot: usize,
}

impl Default for SavedPlayerState {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 50.0, 0.0),
            velocity: Vec3::ZERO,
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: 0.0,
            stats: PlayerStats::new(),
            inventory: Inventory::starter_inventory(),
            active_hotbar_slot: 0,
        }
    }
}

/// Full snapshot of a world's modified state, entities, containers, and player.
#[derive(Debug, Clone)]
pub struct WorldSaveData {
    pub metadata: WorldMetadata,
    pub player: SavedPlayerState,
    pub modified_chunks: HashMap<ChunkPos, Vec<(u16, BlockId)>>,
    pub chest_containers: HashMap<IVec3, Vec<ItemSlot>>,
    pub furnaces: HashMap<IVec3, FurnaceState>,
    pub item_drops: Vec<ItemDrop>,
    pub falling_blocks: Vec<FallingBlock>,
    pub animals: Vec<AnimalEntity>,
    pub next_animal_id: u32,
    pub planted_crops: Vec<(IVec3, f32)>,
    pub active_farmland: Vec<(IVec3, f32)>,
    pub growing_cacti: Vec<(IVec3, f32)>,
    pub decaying_leaves: Vec<(IVec3, f32)>,
    pub animal_spawned_columns: HashSet<IVec2>,
}

impl WorldSaveData {
    pub fn new(metadata: WorldMetadata, player: SavedPlayerState) -> Self {
        Self {
            metadata,
            player,
            modified_chunks: HashMap::new(),
            chest_containers: HashMap::new(),
            furnaces: HashMap::new(),
            item_drops: Vec::new(),
            falling_blocks: Vec::new(),
            animals: Vec::new(),
            next_animal_id: 1,
            planted_crops: Vec::new(),
            active_farmland: Vec::new(),
            growing_cacti: Vec::new(),
            decaying_leaves: Vec::new(),
            animal_spawned_columns: HashSet::new(),
        }
    }

    /// Captures all dirty chunks, chests, and furnaces from `World` into this `WorldSaveData`.
    pub fn capture_from_world(&mut self, world: &World) {
        for &cpos in &world.dirty_chunks {
            if let Some(chunk) = world.get_chunk(cpos) {
                let rle = encode_chunk_rle(chunk);
                self.modified_chunks.insert(cpos, rle);
            }
        }
        self.chest_containers = world.chest_containers.clone();
        self.furnaces = world.furnaces.clone();
    }

    /// Applies saved modified chunks, chests, and furnaces into `World`.
    pub fn apply_to_world(&self, world: &mut World) {
        for (&cpos, rle) in &self.modified_chunks {
            let chunk = decode_chunk_rle(cpos, rle);
            world.insert_dirty_chunk(chunk);
        }
        world.chest_containers = self.chest_containers.clone();
        world.furnaces = self.furnaces.clone();
    }
}

/// Returns current Unix timestamp in seconds.
pub fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Converts a display world name into a safe directory slug.
pub fn sanitize_folder_name(name: &str) -> String {
    let mut slug: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c.is_whitespace() {
                '_'
            } else {
                '-'
            }
        })
        .collect();
    if slug.is_empty() {
        slug = "World".to_string();
    }
    slug
}

/// Parses a user-entered seed string into a `u32` seed.
/// If empty, generates a pseudo-random seed from system time.
/// If a valid integer, uses it directly; otherwise hashes the text string.
pub fn parse_or_hash_seed(seed_input: &str) -> u32 {
    let trimmed = seed_input.trim();
    if trimmed.is_empty() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1337);
        let hashed = nanos.wrapping_mul(0x517cc1b727220a95) ^ (nanos >> 32);
        let s = (hashed as u32).max(1);
        return s;
    }
    if let Ok(val) = trimmed.parse::<u32>() {
        return val;
    }
    if let Ok(val) = trimmed.parse::<i32>() {
        return val as u32;
    }
    // FNV-1a 32-bit hash for arbitrary seed strings
    let mut hash: u32 = 2166136261;
    for byte in trimmed.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

/// Compresses a 4096-block `Chunk` into run-length encoded `(count, BlockId)` pairs.
pub fn encode_chunk_rle(chunk: &Chunk) -> Vec<(u16, BlockId)> {
    let raw = chunk.raw_blocks();
    let mut runs = Vec::new();
    let mut current_id = raw[0];
    let mut count: u16 = 1;

    for &block in &raw[1..] {
        if block == current_id && count < u16::MAX {
            count += 1;
        } else {
            runs.push((count, current_id));
            current_id = block;
            count = 1;
        }
    }
    runs.push((count, current_id));
    runs
}

/// Decompresses run-length encoded `(count, BlockId)` pairs back into a `Chunk`.
pub fn decode_chunk_rle(pos: ChunkPos, runs: &[(u16, BlockId)]) -> Chunk {
    let mut chunk = Chunk::new(pos);
    let raw = chunk.raw_blocks_mut();
    let mut idx = 0;
    for &(count, block_id) in runs {
        let end = (idx + count as usize).min(CHUNK_VOLUME);
        raw[idx..end].fill(block_id);
        idx = end;
        if idx >= CHUNK_VOLUME {
            break;
        }
    }
    chunk
}

/// Lists all saved worlds inside `saves_root`, sorted by `last_played` descending (newest first).
pub fn list_saved_worlds(saves_root: &Path) -> Vec<WorldMetadata> {
    let mut worlds = Vec::new();
    let Ok(entries) = fs::read_dir(saves_root) else {
        return worlds;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let meta_file = path.join("level.dat");
            if meta_file.exists() {
                if let Ok(meta) = load_world_metadata(&path) {
                    worlds.push(meta);
                }
            }
        }
    }

    worlds.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    worlds
}

/// Generates a unique folder name inside `saves_root` for a newly created world.
pub fn unique_world_folder_name(saves_root: &Path, desired_name: &str) -> String {
    let base = sanitize_folder_name(desired_name);
    if !saves_root.join(&base).exists() {
        return base;
    }
    for i in 2..1000 {
        let candidate = format!("{}_{}", base, i);
        if !saves_root.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{}_{}", base, current_unix_timestamp())
}

/// Deletes a saved world directory inside `saves_root`.
pub fn delete_saved_world(saves_root: &Path, folder_name: &str) -> io::Result<()> {
    let target: PathBuf = saves_root.join(folder_name);
    if target.exists() && target.is_dir() {
        fs::remove_dir_all(target)?;
    }
    Ok(())
}

/// Loads only the lightweight `WorldMetadata` (`level.dat`) from a world directory.
pub fn load_world_metadata(world_dir: &Path) -> io::Result<WorldMetadata> {
    let file = File::open(world_dir.join("level.dat"))?;
    let mut r = BufReader::new(file);

    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != SAVE_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid save file magic header",
        ));
    }

    let version = read_u32(&mut r)?;
    if version != 1 && version != SAVE_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported save version: {}", version),
        ));
    }

    let name = read_string(&mut r)?;
    let folder_name = read_string(&mut r)?;
    let seed = read_u32(&mut r)?;
    let last_played = read_u64(&mut r)?;
    let total_time = read_f32(&mut r)?;
    let day_night_time = read_f32(&mut r)?;

    Ok(WorldMetadata {
        name,
        folder_name,
        seed,
        last_played,
        total_time,
        day_night_time,
    })
}

/// Saves the complete `WorldSaveData` to `saves_root/<folder_name>/`.
pub fn save_world_to_dir(saves_root: &Path, data: &mut WorldSaveData) -> io::Result<PathBuf> {
    data.metadata.last_played = current_unix_timestamp();
    let world_dir = saves_root.join(&data.metadata.folder_name);
    fs::create_dir_all(&world_dir)?;

    // 1. Write level.dat (metadata + player state)
    {
        let file = File::create(world_dir.join("level.dat"))?;
        let mut w = BufWriter::new(file);

        w.write_all(SAVE_MAGIC)?;
        write_u32(&mut w, SAVE_VERSION)?;

        // Metadata
        write_string(&mut w, &data.metadata.name)?;
        write_string(&mut w, &data.metadata.folder_name)?;
        write_u32(&mut w, data.metadata.seed)?;
        write_u64(&mut w, data.metadata.last_played)?;
        write_f32(&mut w, data.metadata.total_time)?;
        write_f32(&mut w, data.metadata.day_night_time)?;

        // Player state
        write_vec3(&mut w, data.player.position)?;
        write_vec3(&mut w, data.player.velocity)?;
        write_f32(&mut w, data.player.yaw)?;
        write_f32(&mut w, data.player.pitch)?;
        write_u32(&mut w, data.player.active_hotbar_slot as u32)?;

        // Player stats
        write_f32(&mut w, data.player.stats.health)?;
        write_f32(&mut w, data.player.stats.max_health)?;
        write_f32(&mut w, data.player.stats.hunger)?;
        write_f32(&mut w, data.player.stats.max_hunger)?;
        write_f32(&mut w, data.player.stats.saturation)?;
        write_f32(&mut w, data.player.stats.exhaustion)?;
        write_f32(&mut w, data.player.stats.breath)?;
        write_f32(&mut w, data.player.stats.max_breath)?;
        write_bool(&mut w, data.player.stats.is_dead)?;

        // Player inventory (36 slots)
        write_u32(&mut w, data.player.inventory.slots.len() as u32)?;
        for slot in &data.player.inventory.slots {
            write_item_slot(&mut w, *slot)?;
        }

        w.flush()?;
    }

    // 2. Write world.bin (modified chunks, containers, entities, simulation timers)
    {
        let file = File::create(world_dir.join("world.bin"))?;
        let mut w = BufWriter::new(file);

        w.write_all(SAVE_MAGIC)?;
        write_u32(&mut w, SAVE_VERSION)?;

        // Modified chunks
        write_u32(&mut w, data.modified_chunks.len() as u32)?;
        for (&cpos, runs) in &data.modified_chunks {
            write_ivec3(&mut w, cpos.0)?;
            write_u32(&mut w, runs.len() as u32)?;
            for &(count, block_id) in runs {
                write_u16(&mut w, count)?;
                write_u8(&mut w, block_id.0)?;
            }
        }

        // Chest containers
        write_u32(&mut w, data.chest_containers.len() as u32)?;
        for (&pos, slots) in &data.chest_containers {
            write_ivec3(&mut w, pos)?;
            write_u32(&mut w, slots.len() as u32)?;
            for slot in slots {
                write_item_slot(&mut w, *slot)?;
            }
        }

        // Furnaces
        write_u32(&mut w, data.furnaces.len() as u32)?;
        for (&pos, furnace) in &data.furnaces {
            write_ivec3(&mut w, pos)?;
            write_item_slot(&mut w, furnace.input)?;
            write_item_slot(&mut w, furnace.fuel)?;
            write_item_slot(&mut w, furnace.output)?;
            write_f32(&mut w, furnace.burn_time_remaining)?;
            write_f32(&mut w, furnace.max_burn_time)?;
            write_f32(&mut w, furnace.cook_progress)?;
        }

        // Item drops
        write_u32(&mut w, data.item_drops.len() as u32)?;
        for drop in &data.item_drops {
            write_vec3(&mut w, drop.position)?;
            write_vec3(&mut w, drop.velocity)?;
            write_item_slot(&mut w, drop.item)?;
            write_f32(&mut w, drop.age)?;
            write_f32(&mut w, drop.despawn_time)?;
            write_f32(&mut w, drop.pickup_delay)?;
        }

        // Falling blocks
        write_u32(&mut w, data.falling_blocks.len() as u32)?;
        for fb in &data.falling_blocks {
            write_u8(&mut w, fb.block_id.0)?;
            write_vec3(&mut w, fb.position)?;
            write_f32(&mut w, fb.velocity_y)?;
            write_f32(&mut w, fb.time_falling)?;
        }

        // Animals
        write_u32(&mut w, data.next_animal_id)?;
        write_u32(&mut w, data.animals.len() as u32)?;
        for animal in &data.animals {
            let kind_u8 = match animal.kind {
                AnimalKind::Cow => 0,
                AnimalKind::Pig => 1,
                AnimalKind::Chicken => 2,
            };
            write_u32(&mut w, animal.id)?;
            write_u8(&mut w, kind_u8)?;
            write_vec3(&mut w, animal.position)?;
            write_vec3(&mut w, animal.velocity)?;
            write_f32(&mut w, animal.yaw)?;
            write_f32(&mut w, animal.health)?;
            write_f32(&mut w, animal.breath)?;
        }

        // Planted crops, active farmland, growing cacti, decaying leaves
        write_pos_timer_list(&mut w, &data.planted_crops)?;
        write_pos_timer_list(&mut w, &data.active_farmland)?;
        write_pos_timer_list(&mut w, &data.growing_cacti)?;
        write_pos_timer_list(&mut w, &data.decaying_leaves)?;

        // Animal spawned columns
        write_u32(&mut w, data.animal_spawned_columns.len() as u32)?;
        for col in &data.animal_spawned_columns {
            write_i32(&mut w, col.x)?;
            write_i32(&mut w, col.y)?;
        }

        w.flush()?;
    }

    Ok(world_dir)
}

/// Loads the complete `WorldSaveData` from `saves_root/<folder_name>/`.
pub fn load_world_from_dir(saves_root: &Path, folder_name: &str) -> io::Result<WorldSaveData> {
    let world_dir = saves_root.join(folder_name);

    // 1. Read level.dat
    let (metadata, player) = {
        let file = File::open(world_dir.join("level.dat"))?;
        let mut r = BufReader::new(file);

        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if &magic != SAVE_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid level.dat magic header",
            ));
        }
        let version = read_u32(&mut r)?;
        if version != 1 && version != SAVE_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported save version: {}", version),
            ));
        }

        let name = read_string(&mut r)?;
        let saved_folder = read_string(&mut r)?;
        let seed = read_u32(&mut r)?;
        let last_played = read_u64(&mut r)?;
        let total_time = read_f32(&mut r)?;
        let day_night_time = read_f32(&mut r)?;

        let position = read_vec3(&mut r)?;
        let velocity = read_vec3(&mut r)?;
        let yaw = read_f32(&mut r)?;
        let pitch = read_f32(&mut r)?;
        let active_hotbar_slot = (read_u32(&mut r)? as usize).min(8);

        let mut stats = PlayerStats::new();
        stats.health = read_f32(&mut r)?;
        stats.max_health = read_f32(&mut r)?;
        stats.hunger = read_f32(&mut r)?;
        stats.max_hunger = read_f32(&mut r)?;
        stats.saturation = read_f32(&mut r)?;
        stats.exhaustion = read_f32(&mut r)?;
        stats.breath = read_f32(&mut r)?;
        stats.max_breath = read_f32(&mut r)?;
        stats.is_dead = read_bool(&mut r)?;

        let slot_count = read_u32(&mut r)? as usize;
        let mut inventory = Inventory::default();
        for i in 0..slot_count {
            let slot = read_item_slot(&mut r, version)?;
            if i < Inventory::TOTAL_COUNT {
                inventory.slots[i] = slot;
            }
        }

        (
            WorldMetadata {
                name,
                folder_name: if saved_folder.is_empty() {
                    folder_name.to_string()
                } else {
                    saved_folder
                },
                seed,
                last_played,
                total_time,
                day_night_time,
            },
            SavedPlayerState {
                position,
                velocity,
                yaw,
                pitch,
                stats,
                inventory,
                active_hotbar_slot,
            },
        )
    };

    let mut data = WorldSaveData::new(metadata, player);

    // 2. Read world.bin if present
    let world_bin_path = world_dir.join("world.bin");
    if world_bin_path.exists() {
        let file = File::open(world_bin_path)?;
        let mut r = BufReader::new(file);

        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if &magic != SAVE_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid world.bin magic header",
            ));
        }
        let version = read_u32(&mut r)?;
        if version != 1 && version != SAVE_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported world.bin version: {}", version),
            ));
        }

        // Modified chunks
        let chunk_count = read_u32(&mut r)? as usize;
        for _ in 0..chunk_count {
            let cpos = ChunkPos(read_ivec3(&mut r)?);
            let run_count = read_u32(&mut r)? as usize;
            let mut runs = Vec::with_capacity(run_count);
            for _ in 0..run_count {
                let count = read_u16(&mut r)?;
                let block_id = BlockId(read_u8(&mut r)?);
                runs.push((count, block_id));
            }
            data.modified_chunks.insert(cpos, runs);
        }

        // Chest containers
        let chest_count = read_u32(&mut r)? as usize;
        for _ in 0..chest_count {
            let pos = read_ivec3(&mut r)?;
            let len = read_u32(&mut r)? as usize;
            let mut slots = Vec::with_capacity(len);
            for _ in 0..len {
                slots.push(read_item_slot(&mut r, version)?);
            }
            data.chest_containers.insert(pos, slots);
        }

        // Furnaces
        let furnace_count = read_u32(&mut r)? as usize;
        for _ in 0..furnace_count {
            let pos = read_ivec3(&mut r)?;
            let input = read_item_slot(&mut r, version)?;
            let fuel = read_item_slot(&mut r, version)?;
            let output = read_item_slot(&mut r, version)?;
            let burn_time_remaining = read_f32(&mut r)?;
            let max_burn_time = read_f32(&mut r)?;
            let cook_progress = read_f32(&mut r)?;
            data.furnaces.insert(
                pos,
                FurnaceState {
                    input,
                    fuel,
                    output,
                    burn_time_remaining,
                    max_burn_time,
                    cook_progress,
                },
            );
        }

        // Item drops
        let drop_count = read_u32(&mut r)? as usize;
        for _ in 0..drop_count {
            let position = read_vec3(&mut r)?;
            let velocity = read_vec3(&mut r)?;
            let item = read_item_slot(&mut r, version)?;
            let age = read_f32(&mut r)?;
            let despawn_time = read_f32(&mut r)?;
            let pickup_delay = read_f32(&mut r)?;
            let mut drop = ItemDrop::new(position, item);
            drop.velocity = velocity;
            drop.age = age;
            drop.despawn_time = despawn_time;
            drop.pickup_delay = pickup_delay;
            data.item_drops.push(drop);
        }

        // Falling blocks
        let fb_count = read_u32(&mut r)? as usize;
        for _ in 0..fb_count {
            let block_id = BlockId(read_u8(&mut r)?);
            let position = read_vec3(&mut r)?;
            let velocity_y = read_f32(&mut r)?;
            let time_falling = read_f32(&mut r)?;
            let mut fb = FallingBlock::with_velocity(block_id, position, velocity_y);
            fb.time_falling = time_falling;
            data.falling_blocks.push(fb);
        }

        // Animals
        data.next_animal_id = read_u32(&mut r)?;
        let animal_count = read_u32(&mut r)? as usize;
        for _ in 0..animal_count {
            let id = read_u32(&mut r)?;
            let kind_u8 = read_u8(&mut r)?;
            let kind = match kind_u8 {
                0 => AnimalKind::Cow,
                1 => AnimalKind::Pig,
                _ => AnimalKind::Chicken,
            };
            let position = read_vec3(&mut r)?;
            let velocity = read_vec3(&mut r)?;
            let yaw = read_f32(&mut r)?;
            let health = read_f32(&mut r)?;
            let breath = read_f32(&mut r)?;
            let mut animal = AnimalEntity::new(id, kind, position);
            animal.velocity = velocity;
            animal.yaw = yaw;
            animal.health = health;
            animal.breath = breath;
            data.animals.push(animal);
        }

        // Planted crops, active farmland, growing cacti, decaying leaves
        data.planted_crops = read_pos_timer_list(&mut r)?;
        data.active_farmland = read_pos_timer_list(&mut r)?;
        data.growing_cacti = read_pos_timer_list(&mut r)?;
        data.decaying_leaves = read_pos_timer_list(&mut r)?;

        // Animal spawned columns
        let col_count = read_u32(&mut r)? as usize;
        for _ in 0..col_count {
            let x = read_i32(&mut r)?;
            let y = read_i32(&mut r)?;
            data.animal_spawned_columns.insert(IVec2::new(x, y));
        }
    }

    Ok(data)
}

// --- Binary Serialization Helpers ---

fn write_u8<W: Write>(w: &mut W, val: u8) -> io::Result<()> {
    w.write_all(&[val])
}

fn read_u8<R: Read>(r: &mut R) -> io::Result<u8> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn write_bool<W: Write>(w: &mut W, val: bool) -> io::Result<()> {
    write_u8(w, if val { 1 } else { 0 })
}

fn read_bool<R: Read>(r: &mut R) -> io::Result<bool> {
    Ok(read_u8(r)? != 0)
}

fn write_u16<W: Write>(w: &mut W, val: u16) -> io::Result<()> {
    w.write_all(&val.to_le_bytes())
}

fn read_u16<R: Read>(r: &mut R) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn write_u32<W: Write>(w: &mut W, val: u32) -> io::Result<()> {
    w.write_all(&val.to_le_bytes())
}

fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn write_i32<W: Write>(w: &mut W, val: i32) -> io::Result<()> {
    w.write_all(&val.to_le_bytes())
}

fn read_i32<R: Read>(r: &mut R) -> io::Result<i32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(i32::from_le_bytes(buf))
}

fn write_u64<W: Write>(w: &mut W, val: u64) -> io::Result<()> {
    w.write_all(&val.to_le_bytes())
}

fn read_u64<R: Read>(r: &mut R) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn write_f32<W: Write>(w: &mut W, val: f32) -> io::Result<()> {
    w.write_all(&val.to_le_bytes())
}

fn read_f32<R: Read>(r: &mut R) -> io::Result<f32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(f32::from_le_bytes(buf))
}

fn write_vec3<W: Write>(w: &mut W, v: Vec3) -> io::Result<()> {
    write_f32(w, v.x)?;
    write_f32(w, v.y)?;
    write_f32(w, v.z)
}

fn read_vec3<R: Read>(r: &mut R) -> io::Result<Vec3> {
    Ok(Vec3::new(read_f32(r)?, read_f32(r)?, read_f32(r)?))
}

fn write_ivec3<W: Write>(w: &mut W, v: IVec3) -> io::Result<()> {
    write_i32(w, v.x)?;
    write_i32(w, v.y)?;
    write_i32(w, v.z)
}

fn read_ivec3<R: Read>(r: &mut R) -> io::Result<IVec3> {
    Ok(IVec3::new(read_i32(r)?, read_i32(r)?, read_i32(r)?))
}

fn write_string<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    write_u32(w, bytes.len() as u32)?;
    w.write_all(bytes)
}

fn read_string<R: Read>(r: &mut R) -> io::Result<String> {
    let len = read_u32(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_item_slot<W: Write>(w: &mut W, slot: ItemSlot) -> io::Result<()> {
    write_u16(w, slot.item_id.0)?;
    write_u16(w, slot.count)?;
    write_u16(w, slot.durability)
}

fn read_item_slot<R: Read>(r: &mut R, version: u32) -> io::Result<ItemSlot> {
    let id = ItemId(read_u16(r)?);
    let count = read_u16(r)?;
    let durability = if version >= 2 {
        read_u16(r)?
    } else {
        0
    };
    if id == ItemId::EMPTY || count == 0 {
        Ok(ItemSlot::EMPTY)
    } else {
        Ok(ItemSlot::with_durability(id, count, durability))
    }
}

fn write_pos_timer_list<W: Write>(w: &mut W, list: &[(IVec3, f32)]) -> io::Result<()> {
    write_u32(w, list.len() as u32)?;
    for &(pos, timer) in list {
        write_ivec3(w, pos)?;
        write_f32(w, timer)?;
    }
    Ok(())
}

fn read_pos_timer_list<R: Read>(r: &mut R) -> io::Result<Vec<(IVec3, f32)>> {
    let count = read_u32(r)? as usize;
    let mut list = Vec::with_capacity(count);
    for _ in 0..count {
        let pos = read_ivec3(r)?;
        let timer = read_f32(r)?;
        list.push((pos, timer));
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_rle_compression_roundtrip() {
        let pos = ChunkPos::new(2, 1, -3);
        let mut chunk = Chunk::new(pos);
        chunk.fill(BlockId::STONE);
        chunk.set(0, 0, 0, BlockId::BEDROCK);
        chunk.set(5, 10, 5, BlockId::DIAMOND_ORE);
        chunk.set(15, 15, 15, BlockId::TORCH);

        let rle = encode_chunk_rle(&chunk);
        // Should compress 4096 blocks into only a handful of runs
        assert!(rle.len() < 10, "RLE should be compact, got {} runs", rle.len());

        let decoded = decode_chunk_rle(pos, &rle);
        assert_eq!(decoded.pos, pos);
        assert_eq!(decoded.raw_blocks(), chunk.raw_blocks());
    }

    #[test]
    fn test_world_save_and_load_roundtrip() {
        let temp_root = std::env::temp_dir().join(format!(
            "blockgame_save_test_{}",
            current_unix_timestamp()
        ));
        let _ = fs::remove_dir_all(&temp_root);
        fs::create_dir_all(&temp_root).unwrap();

        // 1. Setup a World with modified blocks, a chest, and an active furnace
        let mut world = World::new();
        let cpos = ChunkPos::new(0, 0, 0);
        let mut chunk = Chunk::new(cpos);
        chunk.fill(BlockId::STONE);
        world.insert_chunk(chunk);

        // Modify blocks via set_block (which marks chunk dirty)
        world.set_block(IVec3::new(3, 4, 5), BlockId::CRAFTING_TABLE);
        world.set_block(IVec3::new(3, 5, 5), BlockId::TORCH);
        assert!(world.dirty_chunks.contains(&cpos));

        // Add items to a chest
        let chest_pos = IVec3::new(4, 4, 5);
        let chest = world.get_or_create_chest(chest_pos);
        chest[0] = ItemSlot::new(ItemId::DIAMOND, 12);
        chest[5] = ItemSlot::new(ItemId::BREAD, 7);

        // Configure a burning furnace
        let furnace_pos = IVec3::new(5, 4, 5);
        let furnace = world.get_or_create_furnace(furnace_pos);
        furnace.input = ItemSlot::new(ItemId::RAW_IRON, 4);
        furnace.fuel = ItemSlot::new(ItemId::COAL, 2);
        furnace.output = ItemSlot::new(ItemId::IRON_INGOT, 1);
        furnace.burn_time_remaining = 42.5;
        furnace.max_burn_time = 80.0;
        furnace.cook_progress = 3.25;

        // 2. Setup player and world save data
        let mut meta = WorldMetadata::new("Test Survival World", 987654);
        meta.total_time = 123.4;
        meta.day_night_time = 0.65;

        let mut player = SavedPlayerState::default();
        player.position = Vec3::new(12.5, 45.0, -8.25);
        player.yaw = 1.25;
        player.pitch = -0.35;
        player.active_hotbar_slot = 4;
        player.stats.health = 14.5;
        player.stats.hunger = 16.0;
        player.inventory.slots[0] = ItemSlot::new(ItemId::DIAMOND_PICKAXE, 1);
        player.inventory.slots[4] = ItemSlot::new(ItemId::WATER_BUCKET, 1);

        let mut save_data = WorldSaveData::new(meta, player);
        save_data.capture_from_world(&world);
        save_data.planted_crops.push((IVec3::new(10, 4, 10), 4.5));
        save_data.active_farmland.push((IVec3::new(10, 3, 10), 1.5));
        save_data.animals.push(AnimalEntity::new(1, AnimalKind::Cow, Vec3::new(5.0, 10.0, 5.0)));
        save_data.next_animal_id = 2;

        // 3. Save to disk
        let saved_dir = save_world_to_dir(&temp_root, &mut save_data).expect("Failed to save world");
        assert!(saved_dir.join("level.dat").exists());
        assert!(saved_dir.join("world.bin").exists());

        // Verify list_saved_worlds discovers it
        let listed = list_saved_worlds(&temp_root);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Test Survival World");
        assert_eq!(listed[0].seed, 987654);

        // 4. Load back from disk into a fresh World
        let loaded = load_world_from_dir(&temp_root, &save_data.metadata.folder_name)
            .expect("Failed to load world");

        assert_eq!(loaded.metadata.name, "Test Survival World");
        assert_eq!(loaded.metadata.seed, 987654);
        assert!((loaded.metadata.day_night_time - 0.65).abs() < 1e-5);
        assert_eq!(loaded.player.position, Vec3::new(12.5, 45.0, -8.25));
        assert_eq!(loaded.player.active_hotbar_slot, 4);
        assert_eq!(loaded.player.stats.health, 14.5);
        assert_eq!(loaded.player.inventory.slots[0], ItemSlot::new(ItemId::DIAMOND_PICKAXE, 1));
        assert_eq!(loaded.player.inventory.slots[4], ItemSlot::new(ItemId::WATER_BUCKET, 1));

        let mut restored_world = World::new();
        loaded.apply_to_world(&mut restored_world);

        assert_eq!(restored_world.get_block(IVec3::new(3, 4, 5)), BlockId::CRAFTING_TABLE);
        assert_eq!(restored_world.get_block(IVec3::new(3, 5, 5)), BlockId::TORCH);
        assert!(restored_world.light_sources.contains(&IVec3::new(3, 5, 5)));

        let restored_chest = restored_world.get_chest(chest_pos).expect("Chest should exist");
        assert_eq!(restored_chest[0], ItemSlot::new(ItemId::DIAMOND, 12));
        assert_eq!(restored_chest[5], ItemSlot::new(ItemId::BREAD, 7));

        let restored_furnace = restored_world.get_furnace(furnace_pos).expect("Furnace should exist");
        assert_eq!(restored_furnace.input, ItemSlot::new(ItemId::RAW_IRON, 4));
        assert_eq!(restored_furnace.fuel, ItemSlot::new(ItemId::COAL, 2));
        assert_eq!(restored_furnace.output, ItemSlot::new(ItemId::IRON_INGOT, 1));
        assert!((restored_furnace.burn_time_remaining - 42.5).abs() < 1e-4);

        assert_eq!(loaded.planted_crops.len(), 1);
        assert_eq!(loaded.active_farmland.len(), 1);
        assert_eq!(loaded.animals.len(), 1);
        assert_eq!(loaded.animals[0].kind, AnimalKind::Cow);

        // 5. Delete saved world
        delete_saved_world(&temp_root, &loaded.metadata.folder_name).unwrap();
        assert!(list_saved_worlds(&temp_root).is_empty());

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_save_version_1_backward_compatibility() {
        let temp_root = std::env::temp_dir().join(format!(
            "blockgame_v1_compat_test_{}",
            current_unix_timestamp()
        ));
        let world_dir = temp_root.join("test_v1_world");
        let _ = fs::remove_dir_all(&temp_root);
        fs::create_dir_all(&world_dir).unwrap();

        // Write a valid Version 1 level.dat (4 bytes per item slot)
        {
            let file = File::create(world_dir.join("level.dat")).unwrap();
            let mut w = BufWriter::new(file);
            w.write_all(SAVE_MAGIC).unwrap();
            write_u32(&mut w, 1).unwrap(); // Version 1

            write_string(&mut w, "Test V1 World").unwrap();
            write_string(&mut w, "test_v1_world").unwrap();
            write_u32(&mut w, 55555).unwrap();
            write_u64(&mut w, 12345678).unwrap();
            write_f32(&mut w, 100.0).unwrap();
            write_f32(&mut w, 0.4).unwrap();

            write_vec3(&mut w, Vec3::new(1.0, 64.0, 1.0)).unwrap();
            write_vec3(&mut w, Vec3::ZERO).unwrap();
            write_f32(&mut w, 0.0).unwrap();
            write_f32(&mut w, 0.0).unwrap();
            write_u32(&mut w, 0).unwrap();

            // Stats
            write_f32(&mut w, 20.0).unwrap();
            write_f32(&mut w, 20.0).unwrap();
            write_f32(&mut w, 20.0).unwrap();
            write_f32(&mut w, 20.0).unwrap();
            write_f32(&mut w, 5.0).unwrap();
            write_f32(&mut w, 0.0).unwrap();
            write_f32(&mut w, 10.0).unwrap();
            write_f32(&mut w, 10.0).unwrap();
            write_bool(&mut w, false).unwrap();

            // 36 Inventory slots with v1 format (only 4 bytes: id + count, NO durability)
            write_u32(&mut w, 36).unwrap();
            // Slot 0: Diamond Pickaxe
            write_u16(&mut w, ItemId::DIAMOND_PICKAXE.0).unwrap();
            write_u16(&mut w, 1).unwrap();
            // Remaining 35 empty slots
            for _ in 1..36 {
                write_u16(&mut w, 0).unwrap();
                write_u16(&mut w, 0).unwrap();
            }
            w.flush().unwrap();
        }

        // Write a valid Version 1 world.bin (4 bytes per item slot for chest, furnace, drop)
        {
            let file = File::create(world_dir.join("world.bin")).unwrap();
            let mut w = BufWriter::new(file);
            w.write_all(SAVE_MAGIC).unwrap();
            write_u32(&mut w, 1).unwrap(); // Version 1

            // 0 modified chunks
            write_u32(&mut w, 0).unwrap();

            // 1 chest container with 27 v1 slots
            write_u32(&mut w, 1).unwrap();
            write_ivec3(&mut w, IVec3::new(0, 60, 0)).unwrap();
            write_u32(&mut w, 27).unwrap();
            // slot 0: Iron Pickaxe
            write_u16(&mut w, ItemId::IRON_PICKAXE.0).unwrap();
            write_u16(&mut w, 1).unwrap();
            for _ in 1..27 {
                write_u16(&mut w, 0).unwrap();
                write_u16(&mut w, 0).unwrap();
            }

            // 1 furnace
            write_u32(&mut w, 1).unwrap();
            write_ivec3(&mut w, IVec3::new(1, 60, 0)).unwrap();
            // input: raw iron (id, count)
            write_u16(&mut w, ItemId::RAW_IRON.0).unwrap();
            write_u16(&mut w, 5).unwrap();
            // fuel: coal (id, count)
            write_u16(&mut w, ItemId::COAL.0).unwrap();
            write_u16(&mut w, 3).unwrap();
            // output: empty
            write_u16(&mut w, 0).unwrap();
            write_u16(&mut w, 0).unwrap();
            write_f32(&mut w, 10.0).unwrap();
            write_f32(&mut w, 80.0).unwrap();
            write_f32(&mut w, 2.0).unwrap();

            // 1 item drop
            write_u32(&mut w, 1).unwrap();
            write_vec3(&mut w, Vec3::new(0.0, 60.5, 0.0)).unwrap();
            write_vec3(&mut w, Vec3::ZERO).unwrap();
            // item: stick (id, count)
            write_u16(&mut w, ItemId::STICK.0).unwrap();
            write_u16(&mut w, 2).unwrap();
            write_f32(&mut w, 5.0).unwrap();
            write_f32(&mut w, 300.0).unwrap();
            write_f32(&mut w, 0.0).unwrap();

            // 0 falling blocks
            write_u32(&mut w, 0).unwrap();
            // Animals: next_id = 1, count = 0
            write_u32(&mut w, 1).unwrap();
            write_u32(&mut w, 0).unwrap();

            // Simulation lists
            write_pos_timer_list(&mut w, &[]).unwrap();
            write_pos_timer_list(&mut w, &[]).unwrap();
            write_pos_timer_list(&mut w, &[]).unwrap();
            write_pos_timer_list(&mut w, &[]).unwrap();

            // Animal spawned columns
            write_u32(&mut w, 0).unwrap();

            w.flush().unwrap();
        }

        // Test metadata load
        let meta = load_world_metadata(&world_dir).expect("load_world_metadata should succeed for v1");
        assert_eq!(meta.name, "Test V1 World");
        assert_eq!(meta.seed, 55555);

        // Test full load from dir without UnexpectedEof
        let loaded = load_world_from_dir(&temp_root, "test_v1_world")
            .expect("load_world_from_dir must succeed for v1 save files without UnexpectedEof");

        assert_eq!(loaded.player.inventory.slots[0].item_id, ItemId::DIAMOND_PICKAXE);
        assert_eq!(loaded.player.inventory.slots[0].count, 1);
        assert_eq!(loaded.player.inventory.slots[0].durability, 0);

        let chest_slots = loaded.chest_containers.get(&IVec3::new(0, 60, 0)).unwrap();
        assert_eq!(chest_slots[0].item_id, ItemId::IRON_PICKAXE);
        assert_eq!(chest_slots[0].durability, 0);

        let furnace = loaded.furnaces.get(&IVec3::new(1, 60, 0)).unwrap();
        assert_eq!(furnace.input.item_id, ItemId::RAW_IRON);
        assert_eq!(furnace.input.durability, 0);

        assert_eq!(loaded.item_drops.len(), 1);
        assert_eq!(loaded.item_drops[0].item.item_id, ItemId::STICK);
        assert_eq!(loaded.item_drops[0].item.durability, 0);

        let _ = fs::remove_dir_all(&temp_root);
    }
}

