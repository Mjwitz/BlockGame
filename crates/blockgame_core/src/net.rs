use std::collections::HashMap;
use std::io::{self, Cursor, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use glam::{IVec3, Vec3};

use crate::{
    auth::{derive_client_password_token, validate_username, AuthDatabase},
    block::BlockId,
    chunk::ChunkPos,
    command::{execute_command, CommandOutput, CommandSource, OnlinePlayerInfo},
    entity::{next_local_drop_id, ItemDrop},
    item::{Inventory, ItemId, ItemRegistry, ItemSlot},
    permissions::{BanManager, PermissionsManager},
    player::PlayerStats,
    save::{
        encode_chunk_rle, load_world_from_dir, save_world_to_dir, SavedPlayerState, WorldMetadata,
        WorldSaveData,
    },
    world::{world_to_chunk_coords, FurnaceState, World},
};

pub const NET_MAGIC: &[u8; 4] = b"BGMP";
pub const DEFAULT_SERVER_PORT: u16 = 25565;
const MAX_PACKET_SIZE: usize = 16 * 1024 * 1024; // 16 MB max frame

/// Returns the human-readable name for a player skin preset (`0..=3`).
pub fn player_skin_name(skin_id: u8) -> &'static str {
    match skin_id % 4 {
        0 => "Explorer Cyan",
        1 => "Crimson Knight",
        2 => "Forest Ranger",
        _ => "Gold Miner",
    }
}

/// Complete state of a player character for 3D model rendering, animations, and nametags.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerModelState {
    pub player_id: u32,
    pub username: String,
    pub position: Vec3, // Feet position (AABB min.y)
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub held_item: ItemId,
    pub walk_anim: f32,
    pub swing_anim: f32,
    pub hurt_timer: f32,
    pub is_sneaking: bool,
    pub skin_id: u8,
    pub health: f32,
    pub max_health: f32,
}

impl PlayerModelState {
    pub fn new(player_id: u32, username: impl Into<String>, position: Vec3, skin_id: u8) -> Self {
        Self {
            player_id,
            username: username.into(),
            position,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            held_item: ItemId::EMPTY,
            walk_anim: 0.0,
            swing_anim: 0.0,
            hurt_timer: 0.0,
            is_sneaking: false,
            skin_id: skin_id % 4,
            health: 20.0,
            max_health: 20.0,
        }
    }
}

/// Packets sent from a `MultiplayerClient` to the `MultiplayerServer`.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientPacket {
    AuthRequest {
        username: String,
        password_token: [u8; 32],
        skin_id: u8,
    },
    PlayerStateUpdate {
        position: Vec3,
        velocity: Vec3,
        yaw: f32,
        pitch: f32,
        held_item: ItemId,
        walk_anim: f32,
        swing_anim: f32,
        is_sneaking: bool,
        health: f32,
    },
    BlockChange {
        pos: IVec3,
        block_id: BlockId,
    },
    ContainerUpdate {
        pos: IVec3,
        is_furnace: bool,
        slots: Vec<ItemSlot>,
        burn_time: f32,
        max_burn_time: f32,
        cook_progress: f32,
    },
    SpawnItemDrop {
        drop_id: u64,
        position: Vec3,
        velocity: Vec3,
        item: ItemSlot,
    },
    PlayerInventorySync {
        inventory: Inventory,
        active_hotbar_slot: usize,
        stats: PlayerStats,
    },
    AttackPlayer {
        target_player_id: u32,
        damage: f32,
    },
    Chat {
        message: String,
    },
    Ping {
        timestamp_ms: u64,
    },
    Disconnect,
    PickupItemDrop {
        drop_id: u64,
    },
    SetSleeping {
        sleeping: bool,
    },
}

/// Returns `true` when `ticks` (0..24000) is nighttime (sun below the horizon).
pub fn is_night_ticks(ticks: f32) -> bool {
    let angle = (ticks.rem_euclid(24000.0) / 24000.0) * std::f32::consts::TAU;
    angle.sin() <= 0.05
}

/// Packets sent from the `MultiplayerServer` to connected `MultiplayerClient`s.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerPacket {
    AuthResponse {
        success: bool,
        player_id: u32,
        session_token: [u8; 16],
        message: String,
        seed: u32,
        day_night_time: f32,
        player_state: SavedPlayerState,
        skin_id: u8,
    },
    WorldDeltaSync {
        modified_chunks: Vec<(ChunkPos, Vec<(u16, BlockId)>)>,
        chests: Vec<(IVec3, Vec<ItemSlot>)>,
        furnaces: Vec<(IVec3, FurnaceState)>,
        item_drops: Vec<(u64, Vec3, Vec3, ItemSlot)>,
    },
    PlayerJoined {
        player: PlayerModelState,
    },
    PlayerLeft {
        player_id: u32,
        username: String,
    },
    WorldSnapshot {
        day_night_time: f32,
        players: Vec<PlayerModelState>,
    },
    BlockUpdate {
        pos: IVec3,
        block_id: BlockId,
    },
    ContainerSync {
        pos: IVec3,
        is_furnace: bool,
        slots: Vec<ItemSlot>,
        burn_time: f32,
        max_burn_time: f32,
        cook_progress: f32,
    },
    ItemDropSpawned {
        drop_id: u64,
        position: Vec3,
        velocity: Vec3,
        item: ItemSlot,
    },
    PlayerDamaged {
        target_player_id: u32,
        attacker_id: u32,
        damage: f32,
        new_health: f32,
    },
    ChatBroadcast {
        sender: String,
        message: String,
        is_system: bool,
    },
    Pong {
        timestamp_ms: u64,
    },
    Kick {
        reason: String,
    },
    ItemDropCollected {
        drop_id: u64,
        collector_player_id: u32,
        item: ItemSlot,
    },
    TeleportPlayer {
        position: Vec3,
        yaw: f32,
        pitch: f32,
    },
    ReceiveItem {
        item: ItemSlot,
    },
}


impl ClientPacket {
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let mut payload = Vec::with_capacity(128);
        match self {
            Self::AuthRequest {
                username,
                password_token,
                skin_id,
            } => {
                write_u8(&mut payload, 1)?;
                write_string(&mut payload, username)?;
                payload.write_all(password_token)?;
                write_u8(&mut payload, *skin_id)?;
            }
            Self::PlayerStateUpdate {
                position,
                velocity,
                yaw,
                pitch,
                held_item,
                walk_anim,
                swing_anim,
                is_sneaking,
                health,
            } => {
                write_u8(&mut payload, 2)?;
                write_vec3(&mut payload, *position)?;
                write_vec3(&mut payload, *velocity)?;
                write_f32(&mut payload, *yaw)?;
                write_f32(&mut payload, *pitch)?;
                write_u16(&mut payload, held_item.0)?;
                write_f32(&mut payload, *walk_anim)?;
                write_f32(&mut payload, *swing_anim)?;
                write_bool(&mut payload, *is_sneaking)?;
                write_f32(&mut payload, *health)?;
            }
            Self::BlockChange { pos, block_id } => {
                write_u8(&mut payload, 3)?;
                write_ivec3(&mut payload, *pos)?;
                write_u8(&mut payload, block_id.0)?;
            }
            Self::ContainerUpdate {
                pos,
                is_furnace,
                slots,
                burn_time,
                max_burn_time,
                cook_progress,
            } => {
                write_u8(&mut payload, 4)?;
                write_ivec3(&mut payload, *pos)?;
                write_bool(&mut payload, *is_furnace)?;
                write_u32(&mut payload, slots.len() as u32)?;
                for s in slots {
                    write_slot(&mut payload, *s)?;
                }
                write_f32(&mut payload, *burn_time)?;
                write_f32(&mut payload, *max_burn_time)?;
                write_f32(&mut payload, *cook_progress)?;
            }
            Self::SpawnItemDrop {
                drop_id,
                position,
                velocity,
                item,
            } => {
                write_u8(&mut payload, 5)?;
                write_u64(&mut payload, *drop_id)?;
                write_vec3(&mut payload, *position)?;
                write_vec3(&mut payload, *velocity)?;
                write_slot(&mut payload, *item)?;
            }
            Self::PlayerInventorySync {
                inventory,
                active_hotbar_slot,
                stats,
            } => {
                write_u8(&mut payload, 6)?;
                write_u32(&mut payload, *active_hotbar_slot as u32)?;
                write_f32(&mut payload, stats.health)?;
                write_f32(&mut payload, stats.hunger)?;
                write_f32(&mut payload, stats.saturation)?;
                write_f32(&mut payload, stats.breath)?;
                write_bool(&mut payload, stats.is_dead)?;
                write_u32(&mut payload, inventory.slots.len() as u32)?;
                for s in &inventory.slots {
                    write_slot(&mut payload, *s)?;
                }
            }
            Self::AttackPlayer {
                target_player_id,
                damage,
            } => {
                write_u8(&mut payload, 7)?;
                write_u32(&mut payload, *target_player_id)?;
                write_f32(&mut payload, *damage)?;
            }
            Self::Chat { message } => {
                write_u8(&mut payload, 8)?;
                write_string(&mut payload, message)?;
            }
            Self::Ping { timestamp_ms } => {
                write_u8(&mut payload, 9)?;
                write_u64(&mut payload, *timestamp_ms)?;
            }
            Self::Disconnect => {
                write_u8(&mut payload, 10)?;
            }
            Self::PickupItemDrop { drop_id } => {
                write_u8(&mut payload, 11)?;
                write_u64(&mut payload, *drop_id)?;
            }
            Self::SetSleeping { sleeping } => {
                write_u8(&mut payload, 12)?;
                write_bool(&mut payload, *sleeping)?;
            }
        }
        wrap_frame(&payload)
    }

    pub fn decode(payload: &[u8]) -> io::Result<Self> {
        let mut r = Cursor::new(payload);
        let tag = read_u8(&mut r)?;
        match tag {
            1 => {
                let username = read_string(&mut r)?;
                let mut password_token = [0u8; 32];
                r.read_exact(&mut password_token)?;
                let skin_id = read_u8(&mut r)?;
                Ok(Self::AuthRequest {
                    username,
                    password_token,
                    skin_id,
                })
            }
            2 => {
                let position = read_vec3(&mut r)?;
                let velocity = read_vec3(&mut r)?;
                let yaw = read_f32(&mut r)?;
                let pitch = read_f32(&mut r)?;
                let held_item = ItemId(read_u16(&mut r)?);
                let walk_anim = read_f32(&mut r)?;
                let swing_anim = read_f32(&mut r)?;
                let is_sneaking = read_bool(&mut r)?;
                let health = read_f32(&mut r)?;
                Ok(Self::PlayerStateUpdate {
                    position,
                    velocity,
                    yaw,
                    pitch,
                    held_item,
                    walk_anim,
                    swing_anim,
                    is_sneaking,
                    health,
                })
            }
            3 => {
                let pos = read_ivec3(&mut r)?;
                let block_id = BlockId(read_u8(&mut r)?);
                Ok(Self::BlockChange { pos, block_id })
            }
            4 => {
                let pos = read_ivec3(&mut r)?;
                let is_furnace = read_bool(&mut r)?;
                let count = read_u32(&mut r)? as usize;
                let mut slots = Vec::with_capacity(count.min(64));
                for _ in 0..count {
                    slots.push(read_slot(&mut r)?);
                }
                let burn_time = read_f32(&mut r)?;
                let max_burn_time = read_f32(&mut r)?;
                let cook_progress = read_f32(&mut r)?;
                Ok(Self::ContainerUpdate {
                    pos,
                    is_furnace,
                    slots,
                    burn_time,
                    max_burn_time,
                    cook_progress,
                })
            }
            5 => {
                let drop_id = read_u64(&mut r)?;
                let position = read_vec3(&mut r)?;
                let velocity = read_vec3(&mut r)?;
                let item = read_slot(&mut r)?;
                Ok(Self::SpawnItemDrop {
                    drop_id,
                    position,
                    velocity,
                    item,
                })
            }
            6 => {
                let active_hotbar_slot = (read_u32(&mut r)? as usize).min(8);
                let mut stats = PlayerStats::new();
                stats.health = read_f32(&mut r)?;
                stats.hunger = read_f32(&mut r)?;
                stats.saturation = read_f32(&mut r)?;
                stats.breath = read_f32(&mut r)?;
                stats.is_dead = read_bool(&mut r)?;
                let count = read_u32(&mut r)? as usize;
                let mut inventory = Inventory::default();
                for i in 0..count {
                    let s = read_slot(&mut r)?;
                    if i < Inventory::TOTAL_COUNT {
                        inventory.slots[i] = s;
                    }
                }
                Ok(Self::PlayerInventorySync {
                    inventory,
                    active_hotbar_slot,
                    stats,
                })
            }
            7 => {
                let target_player_id = read_u32(&mut r)?;
                let damage = read_f32(&mut r)?;
                Ok(Self::AttackPlayer {
                    target_player_id,
                    damage,
                })
            }
            8 => {
                let message = read_string(&mut r)?;
                Ok(Self::Chat { message })
            }
            9 => {
                let timestamp_ms = read_u64(&mut r)?;
                Ok(Self::Ping { timestamp_ms })
            }
            10 => Ok(Self::Disconnect),
            11 => {
                let drop_id = read_u64(&mut r)?;
                Ok(Self::PickupItemDrop { drop_id })
            }
            12 => {
                let sleeping = read_bool(&mut r)?;
                Ok(Self::SetSleeping { sleeping })
            }
            other => Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("Unknown ClientPacket tag {}", other),
            )),
        }
    }
}

impl ServerPacket {
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let mut payload = Vec::with_capacity(256);
        match self {
            Self::AuthResponse {
                success,
                player_id,
                session_token,
                message,
                seed,
                day_night_time,
                player_state,
                skin_id,
            } => {
                write_u8(&mut payload, 1)?;
                write_bool(&mut payload, *success)?;
                write_u32(&mut payload, *player_id)?;
                payload.write_all(session_token)?;
                write_string(&mut payload, message)?;
                write_u32(&mut payload, *seed)?;
                write_f32(&mut payload, *day_night_time)?;
                write_saved_player_state(&mut payload, player_state)?;
                write_u8(&mut payload, *skin_id)?;
            }
            Self::WorldDeltaSync {
                modified_chunks,
                chests,
                furnaces,
                item_drops,
            } => {
                write_u8(&mut payload, 2)?;
                write_u32(&mut payload, modified_chunks.len() as u32)?;
                for (cpos, runs) in modified_chunks {
                    write_ivec3(&mut payload, cpos.0)?;
                    write_u32(&mut payload, runs.len() as u32)?;
                    for &(cnt, bid) in runs {
                        write_u16(&mut payload, cnt)?;
                        write_u8(&mut payload, bid.0)?;
                    }
                }
                write_u32(&mut payload, chests.len() as u32)?;
                for (pos, slots) in chests {
                    write_ivec3(&mut payload, *pos)?;
                    write_u32(&mut payload, slots.len() as u32)?;
                    for s in slots {
                        write_slot(&mut payload, *s)?;
                    }
                }
                write_u32(&mut payload, furnaces.len() as u32)?;
                for (pos, f) in furnaces {
                    write_ivec3(&mut payload, *pos)?;
                    write_slot(&mut payload, f.input)?;
                    write_slot(&mut payload, f.fuel)?;
                    write_slot(&mut payload, f.output)?;
                    write_f32(&mut payload, f.burn_time_remaining)?;
                    write_f32(&mut payload, f.max_burn_time)?;
                    write_f32(&mut payload, f.cook_progress)?;
                }
                write_u32(&mut payload, item_drops.len() as u32)?;
                for (drop_id, pos, vel, item) in item_drops {
                    write_u64(&mut payload, *drop_id)?;
                    write_vec3(&mut payload, *pos)?;
                    write_vec3(&mut payload, *vel)?;
                    write_slot(&mut payload, *item)?;
                }
            }
            Self::PlayerJoined { player } => {
                write_u8(&mut payload, 3)?;
                write_player_model(&mut payload, player)?;
            }
            Self::PlayerLeft {
                player_id,
                username,
            } => {
                write_u8(&mut payload, 4)?;
                write_u32(&mut payload, *player_id)?;
                write_string(&mut payload, username)?;
            }
            Self::WorldSnapshot {
                day_night_time,
                players,
            } => {
                write_u8(&mut payload, 5)?;
                write_f32(&mut payload, *day_night_time)?;
                write_u32(&mut payload, players.len() as u32)?;
                for p in players {
                    write_player_model(&mut payload, p)?;
                }
            }
            Self::BlockUpdate { pos, block_id } => {
                write_u8(&mut payload, 6)?;
                write_ivec3(&mut payload, *pos)?;
                write_u8(&mut payload, block_id.0)?;
            }
            Self::ContainerSync {
                pos,
                is_furnace,
                slots,
                burn_time,
                max_burn_time,
                cook_progress,
            } => {
                write_u8(&mut payload, 7)?;
                write_ivec3(&mut payload, *pos)?;
                write_bool(&mut payload, *is_furnace)?;
                write_u32(&mut payload, slots.len() as u32)?;
                for s in slots {
                    write_slot(&mut payload, *s)?;
                }
                write_f32(&mut payload, *burn_time)?;
                write_f32(&mut payload, *max_burn_time)?;
                write_f32(&mut payload, *cook_progress)?;
            }
            Self::ItemDropSpawned {
                drop_id,
                position,
                velocity,
                item,
            } => {
                write_u8(&mut payload, 8)?;
                write_u64(&mut payload, *drop_id)?;
                write_vec3(&mut payload, *position)?;
                write_vec3(&mut payload, *velocity)?;
                write_slot(&mut payload, *item)?;
            }
            Self::PlayerDamaged {
                target_player_id,
                attacker_id,
                damage,
                new_health,
            } => {
                write_u8(&mut payload, 9)?;
                write_u32(&mut payload, *target_player_id)?;
                write_u32(&mut payload, *attacker_id)?;
                write_f32(&mut payload, *damage)?;
                write_f32(&mut payload, *new_health)?;
            }
            Self::ChatBroadcast {
                sender,
                message,
                is_system,
            } => {
                write_u8(&mut payload, 10)?;
                write_string(&mut payload, sender)?;
                write_string(&mut payload, message)?;
                write_bool(&mut payload, *is_system)?;
            }
            Self::Pong { timestamp_ms } => {
                write_u8(&mut payload, 11)?;
                write_u64(&mut payload, *timestamp_ms)?;
            }
            Self::Kick { reason } => {
                write_u8(&mut payload, 12)?;
                write_string(&mut payload, reason)?;
            }
            Self::ItemDropCollected {
                drop_id,
                collector_player_id,
                item,
            } => {
                write_u8(&mut payload, 13)?;
                write_u64(&mut payload, *drop_id)?;
                write_u32(&mut payload, *collector_player_id)?;
                write_slot(&mut payload, *item)?;
            }
            Self::TeleportPlayer {
                position,
                yaw,
                pitch,
            } => {
                write_u8(&mut payload, 14)?;
                write_vec3(&mut payload, *position)?;
                write_f32(&mut payload, *yaw)?;
                write_f32(&mut payload, *pitch)?;
            }
            Self::ReceiveItem { item } => {
                write_u8(&mut payload, 15)?;
                write_slot(&mut payload, *item)?;
            }
        }

        wrap_frame(&payload)
    }

    pub fn decode(payload: &[u8]) -> io::Result<Self> {
        let mut r = Cursor::new(payload);
        let tag = read_u8(&mut r)?;
        match tag {
            1 => {
                let success = read_bool(&mut r)?;
                let player_id = read_u32(&mut r)?;
                let mut session_token = [0u8; 16];
                r.read_exact(&mut session_token)?;
                let message = read_string(&mut r)?;
                let seed = read_u32(&mut r)?;
                let day_night_time = read_f32(&mut r)?;
                let player_state = read_saved_player_state(&mut r)?;
                let skin_id = read_u8(&mut r)?;
                Ok(Self::AuthResponse {
                    success,
                    player_id,
                    session_token,
                    message,
                    seed,
                    day_night_time,
                    player_state,
                    skin_id,
                })
            }
            2 => {
                let chunk_count = read_u32(&mut r)? as usize;
                let mut modified_chunks = Vec::with_capacity(chunk_count);
                for _ in 0..chunk_count {
                    let cpos = ChunkPos(read_ivec3(&mut r)?);
                    let run_count = read_u32(&mut r)? as usize;
                    let mut runs = Vec::with_capacity(run_count);
                    for _ in 0..run_count {
                        let cnt = read_u16(&mut r)?;
                        let bid = BlockId(read_u8(&mut r)?);
                        runs.push((cnt, bid));
                    }
                    modified_chunks.push((cpos, runs));
                }

                let chest_count = read_u32(&mut r)? as usize;
                let mut chests = Vec::with_capacity(chest_count);
                for _ in 0..chest_count {
                    let pos = read_ivec3(&mut r)?;
                    let len = read_u32(&mut r)? as usize;
                    let mut slots = Vec::with_capacity(len);
                    for _ in 0..len {
                        slots.push(read_slot(&mut r)?);
                    }
                    chests.push((pos, slots));
                }

                let furnace_count = read_u32(&mut r)? as usize;
                let mut furnaces = Vec::with_capacity(furnace_count);
                for _ in 0..furnace_count {
                    let pos = read_ivec3(&mut r)?;
                    let input = read_slot(&mut r)?;
                    let fuel = read_slot(&mut r)?;
                    let output = read_slot(&mut r)?;
                    let burn_time_remaining = read_f32(&mut r)?;
                    let max_burn_time = read_f32(&mut r)?;
                    let cook_progress = read_f32(&mut r)?;
                    furnaces.push((
                        pos,
                        FurnaceState {
                            input,
                            fuel,
                            output,
                            burn_time_remaining,
                            max_burn_time,
                            cook_progress,
                        },
                    ));
                }

                let drop_count = read_u32(&mut r)? as usize;
                let mut item_drops = Vec::with_capacity(drop_count);
                for _ in 0..drop_count {
                    let drop_id = read_u64(&mut r)?;
                    let pos = read_vec3(&mut r)?;
                    let vel = read_vec3(&mut r)?;
                    let item = read_slot(&mut r)?;
                    item_drops.push((drop_id, pos, vel, item));
                }

                Ok(Self::WorldDeltaSync {
                    modified_chunks,
                    chests,
                    furnaces,
                    item_drops,
                })
            }
            3 => {
                let player = read_player_model(&mut r)?;
                Ok(Self::PlayerJoined { player })
            }
            4 => {
                let player_id = read_u32(&mut r)?;
                let username = read_string(&mut r)?;
                Ok(Self::PlayerLeft {
                    player_id,
                    username,
                })
            }
            5 => {
                let day_night_time = read_f32(&mut r)?;
                let count = read_u32(&mut r)? as usize;
                let mut players = Vec::with_capacity(count);
                for _ in 0..count {
                    players.push(read_player_model(&mut r)?);
                }
                Ok(Self::WorldSnapshot {
                    day_night_time,
                    players,
                })
            }
            6 => {
                let pos = read_ivec3(&mut r)?;
                let block_id = BlockId(read_u8(&mut r)?);
                Ok(Self::BlockUpdate { pos, block_id })
            }
            7 => {
                let pos = read_ivec3(&mut r)?;
                let is_furnace = read_bool(&mut r)?;
                let count = read_u32(&mut r)? as usize;
                let mut slots = Vec::with_capacity(count);
                for _ in 0..count {
                    slots.push(read_slot(&mut r)?);
                }
                let burn_time = read_f32(&mut r)?;
                let max_burn_time = read_f32(&mut r)?;
                let cook_progress = read_f32(&mut r)?;
                Ok(Self::ContainerSync {
                    pos,
                    is_furnace,
                    slots,
                    burn_time,
                    max_burn_time,
                    cook_progress,
                })
            }
            8 => {
                let drop_id = read_u64(&mut r)?;
                let position = read_vec3(&mut r)?;
                let velocity = read_vec3(&mut r)?;
                let item = read_slot(&mut r)?;
                Ok(Self::ItemDropSpawned {
                    drop_id,
                    position,
                    velocity,
                    item,
                })
            }
            9 => {
                let target_player_id = read_u32(&mut r)?;
                let attacker_id = read_u32(&mut r)?;
                let damage = read_f32(&mut r)?;
                let new_health = read_f32(&mut r)?;
                Ok(Self::PlayerDamaged {
                    target_player_id,
                    attacker_id,
                    damage,
                    new_health,
                })
            }
            10 => {
                let sender = read_string(&mut r)?;
                let message = read_string(&mut r)?;
                let is_system = read_bool(&mut r)?;
                Ok(Self::ChatBroadcast {
                    sender,
                    message,
                    is_system,
                })
            }
            11 => {
                let timestamp_ms = read_u64(&mut r)?;
                Ok(Self::Pong { timestamp_ms })
            }
            12 => {
                let reason = read_string(&mut r)?;
                Ok(Self::Kick { reason })
            }
            13 => {
                let drop_id = read_u64(&mut r)?;
                let collector_player_id = read_u32(&mut r)?;
                let item = read_slot(&mut r)?;
                Ok(Self::ItemDropCollected {
                    drop_id,
                    collector_player_id,
                    item,
                })
            }
            14 => {
                let position = read_vec3(&mut r)?;
                let yaw = read_f32(&mut r)?;
                let pitch = read_f32(&mut r)?;
                Ok(Self::TeleportPlayer {
                    position,
                    yaw,
                    pitch,
                })
            }
            15 => {
                let item = read_slot(&mut r)?;
                Ok(Self::ReceiveItem { item })
            }
            other => Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("Unknown ServerPacket tag {}", other),
            )),

        }
    }
}

/// Non-blocking stream frame accumulator for `[b"BGMP"][u32 len][payload...]` packets.
#[derive(Default)]
pub struct PacketFramer {
    buf: Vec<u8>,
}

impl PacketFramer {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(4096),
        }
    }

    /// Reads all available bytes from a non-blocking `TcpStream` into the internal buffer.
    /// Returns `Ok(false)` if the remote peer cleanly closed the connection.
    pub fn read_available(&mut self, stream: &mut TcpStream) -> io::Result<bool> {
        let mut temp = [0u8; 8192];
        loop {
            match stream.read(&mut temp) {
                Ok(0) => return Ok(false),
                Ok(n) => {
                    self.buf.extend_from_slice(&temp[..n]);
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    return Ok(true);
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
    }

    /// Extracts the next complete frame payload if available.
    pub fn next_frame(&mut self) -> io::Result<Option<Vec<u8>>> {
        if self.buf.len() < 8 {
            return Ok(None);
        }
        if &self.buf[0..4] != NET_MAGIC {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "Invalid BGMP packet magic header",
            ));
        }
        let len = u32::from_le_bytes([self.buf[4], self.buf[5], self.buf[6], self.buf[7]]) as usize;
        if len > MAX_PACKET_SIZE {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("Packet frame exceeds maximum size ({} bytes)", len),
            ));
        }
        if self.buf.len() < 8 + len {
            return Ok(None);
        }
        let payload = self.buf[8..8 + len].to_vec();
        self.buf.drain(0..8 + len);
        Ok(Some(payload))
    }
}

// --- Authoritative Multiplayer Server ---

struct ConnectedClient {
    stream: TcpStream,
    framer: PacketFramer,
    authenticated: bool,
    player_id: u32,
    username: String,
    session_token: [u8; 16],
    model: PlayerModelState,
    saved_state: SavedPlayerState,
    is_sleeping: bool,
}

/// Authoritative Multiplayer Server managing authentication, persistent user accounts,
/// world state synchronization, block updates, containers, and chat.
pub struct MultiplayerServer {
    bind_addr: SocketAddr,
    stop_flag: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
    online_count: Arc<Mutex<usize>>,
    command_tx: Sender<String>,
}

impl MultiplayerServer {
    /// Starts an authoritative multiplayer server on `bind_addr` in a background thread,
    /// persisting world and user data under `saves_root/<folder_name>/`.
    pub fn start<F>(
        bind_addr: impl ToSocketAddrs,
        saves_root: PathBuf,
        folder_name: String,
        world_name: String,
        default_seed: u32,
        initial_world_data: Option<WorldSaveData>,
        chunk_generator: F,
    ) -> io::Result<Self>
    where
        F: Fn(ChunkPos) -> crate::chunk::Chunk + Send + Sync + 'static,
    {
        let listener = TcpListener::bind(bind_addr)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;

        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_flag.clone();
        let online_count = Arc::new(Mutex::new(0usize));
        let online_clone = online_count.clone();
        let (command_tx, command_rx) = mpsc::channel();
        let gen_arc: Arc<dyn Fn(ChunkPos) -> crate::chunk::Chunk + Send + Sync> =
            Arc::new(chunk_generator);

        let handle = thread::Builder::new()
            .name("blockgame-mp-server".to_string())
            .spawn(move || {
                run_server_loop(
                    listener,
                    saves_root,
                    folder_name,
                    world_name,
                    default_seed,
                    initial_world_data,
                    gen_arc,
                    stop_clone,
                    online_clone,
                    command_rx,
                );
            })?;

        log::info!("MultiplayerServer listening on {}", local_addr);

        Ok(Self {
            bind_addr: local_addr,
            stop_flag,
            join_handle: Some(handle),
            online_count,
            command_tx,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub fn online_player_count(&self) -> usize {
        *self.online_count.lock().unwrap()
    }

    /// Dispatches a command from the server console for immediate execution on the server thread.
    pub fn execute_command(&self, cmd: &str) {
        let _ = self.command_tx.send(cmd.to_string());
    }

    pub fn shutdown(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for MultiplayerServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn apply_command_output(
    output: CommandOutput,
    source_pid: Option<u32>,
    direct_sends: &mut Vec<(u32, ServerPacket)>,
    outgoing_broadcasts: &mut Vec<(Option<u32>, ServerPacket)>,
    clients: &mut [ConnectedClient],
    disconnected_indices: &mut Vec<usize>,
    perms_manager: &mut PermissionsManager,
    ban_manager: &mut BanManager,
    auth_db: &mut AuthDatabase,
    default_spawn: Vec3,
    world_dir: &std::path::Path,
    stop_flag: &Arc<AtomicBool>,
) {
    if output.save_permissions {
        let _ = perms_manager.save_to_dir(world_dir);
    }
    if output.save_bans {
        let _ = ban_manager.save_to_dir(world_dir);
    }

    for fb in output.feedback {
        if let Some(pid) = source_pid {
            direct_sends.push((
                pid,
                ServerPacket::ChatBroadcast {
                    sender: "Server".to_string(),
                    message: fb,
                    is_system: true,
                },
            ));
        } else {
            println!("{}", fb);
            log::info!("[Console] {}", fb);
        }
    }

    for msg in output.broadcasts {
        outgoing_broadcasts.push((
            None,
            ServerPacket::ChatBroadcast {
                sender: "Server".to_string(),
                message: msg,
                is_system: true,
            },
        ));
    }

    for (target_pid, reason) in output.kick_targets {
        direct_sends.push((target_pid, ServerPacket::Kick { reason }));
        if let Some(pos) = clients.iter().position(|c| c.player_id == target_pid) {
            disconnected_indices.push(pos);
        }
    }

    for (target_pid, pos, yaw, pitch) in output.teleports {
        if let Some(c) = clients.iter_mut().find(|c| c.player_id == target_pid) {
            c.model.position = pos;
            c.saved_state.position = pos;
            c.model.yaw = yaw;
            c.saved_state.yaw = yaw;
            c.model.pitch = pitch;
            c.saved_state.pitch = pitch;
        }
        direct_sends.push((
            target_pid,
            ServerPacket::TeleportPlayer {
                position: pos,
                yaw,
                pitch,
            },
        ));
    }

    for (target_pid, slot) in output.gives {
        direct_sends.push((target_pid, ServerPacket::ReceiveItem { item: slot }));
    }

    for (target_name, new_rank) in output.rank_updates {
        // Update permissions manager
        let _ = perms_manager.add_player_group(&target_name, &new_rank);
        let _ = perms_manager.save_to_dir(world_dir);

        // Update online client if connected
        if let Some(c) = clients.iter_mut().find(|c| c.username.eq_ignore_ascii_case(&target_name)) {
            c.saved_state.rank = new_rank.clone();
            let _ = crate::save::save_player_data(world_dir, &c.username, &c.saved_state);
        }

        // Update auth_db
        let key = target_name.trim().to_ascii_lowercase();
        if let Some(acc) = auth_db.accounts.get_mut(&key) {
            acc.player_state.rank = new_rank;
            let _ = crate::save::save_player_data(world_dir, &acc.username, &acc.player_state);
        }
        let _ = auth_db.save_to_dir(world_dir);
    }

    for target_name in output.reset_players {
        // Delete dedicated player file
        let _ = crate::save::delete_player_data(world_dir, &target_name);

        let mut fresh = SavedPlayerState::default();
        fresh.position = default_spawn;

        // Reset auth_db account
        auth_db.update_player_state(&target_name, 0, fresh.clone());
        let _ = auth_db.save_to_dir(world_dir);

        // Reset online client if connected
        if let Some(c) = clients.iter_mut().find(|c| c.username.eq_ignore_ascii_case(&target_name)) {
            c.model.position = default_spawn;
            c.model.velocity = Vec3::ZERO;
            c.model.health = 20.0;
            c.saved_state = fresh;
            direct_sends.push((
                c.player_id,
                ServerPacket::TeleportPlayer {
                    position: default_spawn,
                    yaw: 0.0,
                    pitch: 0.0,
                },
            ));
        }
    }

    if output.stop_server {
        stop_flag.store(true, Ordering::SeqCst);
    }
}

fn run_server_loop(
    listener: TcpListener,
    saves_root: PathBuf,
    folder_name: String,
    world_name: String,
    default_seed: u32,
    initial_world_data: Option<WorldSaveData>,
    chunk_generator: Arc<dyn Fn(ChunkPos) -> crate::chunk::Chunk + Send + Sync>,
    stop_flag: Arc<AtomicBool>,
    online_count: Arc<Mutex<usize>>,
    command_rx: Receiver<String>,
) {
    let world_dir = saves_root.join(&folder_name);
    let mut save_data = if let Some(data) = initial_world_data {
        data
    } else if let Ok(loaded) = load_world_from_dir(&saves_root, &folder_name) {
        loaded
    } else {
        let mut meta = WorldMetadata::new(world_name, default_seed);
        meta.folder_name = folder_name.clone();
        WorldSaveData::new(meta, SavedPlayerState::default())
    };

    let mut auth_db = AuthDatabase::load_from_dir(&world_dir).unwrap_or_default();
    let mut perms_manager =
        PermissionsManager::load_from_dir(&world_dir).unwrap_or_else(|_| PermissionsManager::new());
    let mut ban_manager = BanManager::load_from_dir(&world_dir).unwrap_or_default();

    let item_registry = ItemRegistry::new();
    let mut world = World::new();
    save_data.apply_to_world(&mut world);

    let default_spawn = if save_data.player.position.length_squared() > 0.1 {
        save_data.player.position
    } else {
        Vec3::new(0.5, 76.0, 0.5)
    };


    let mut clients: Vec<ConnectedClient> = Vec::new();
    let mut next_player_id: u32 = 1;
    let mut last_tick = Instant::now();
    if save_data.metadata.day_night_time <= 1.0 {
        save_data.metadata.day_night_time =
            (save_data.metadata.day_night_time * 24000.0).rem_euclid(24000.0);
    }

    let mut last_autosave = Instant::now();
    let tick_interval = Duration::from_millis(50); // 20 TPS

    while !stop_flag.load(Ordering::Relaxed) {
        // 1. Accept incoming TCP connections
        while let Ok((stream, addr)) = listener.accept() {
            let _ = stream.set_nonblocking(true);
            let _ = stream.set_nodelay(true);
            let pid = next_player_id;
            next_player_id += 1;
            log::info!("Incoming connection from {} (assigned id {})", addr, pid);
            clients.push(ConnectedClient {
                stream,
                framer: PacketFramer::new(),
                authenticated: false,
                player_id: pid,
                username: String::new(),
                session_token: [0u8; 16],
                model: PlayerModelState::new(pid, "", default_spawn, 0),
                saved_state: SavedPlayerState::default(),
                is_sleeping: false,
            });
        }

        // 2. Read incoming console commands
        let mut outgoing_broadcasts: Vec<(Option<u32>, ServerPacket)> = Vec::new();
        let mut direct_sends: Vec<(u32, ServerPacket)> = Vec::new();
        let mut disconnected_indices: Vec<usize> = Vec::new();

        while let Ok(cmd_line) = command_rx.try_recv() {
            let online_infos: Vec<OnlinePlayerInfo> = clients
                .iter()
                .filter(|c| c.authenticated)
                .map(|c| OnlinePlayerInfo {
                    player_id: c.player_id,
                    username: c.username.clone(),
                    position: c.model.position,
                    yaw: c.model.yaw,
                    pitch: c.model.pitch,
                })
                .collect();
            let out = execute_command(
                &CommandSource::Console,
                &cmd_line,
                &online_infos,
                &mut perms_manager,
                &mut ban_manager,
                &item_registry,
            );
            apply_command_output(
                out,
                None,
                &mut direct_sends,
                &mut outgoing_broadcasts,
                &mut clients,
                &mut disconnected_indices,
                &mut perms_manager,
                &mut ban_manager,
                &mut auth_db,
                default_spawn,
                &world_dir,
                &stop_flag,
            );
        }

        // 3. Read & process incoming packets from all connected clients
        // Snapshot currently authenticated usernames (lowercase) to prevent duplicate simultaneous logins
        let active_usernames: Vec<(u32, String)> = clients
            .iter()
            .filter(|c| c.authenticated)
            .map(|c| (c.player_id, c.username.to_ascii_lowercase()))
            .collect();

        for idx in 0..clients.len() {
            let alive = {
                let c = &mut clients[idx];
                c.framer.read_available(&mut c.stream).unwrap_or(false)
            };
            if !alive {
                disconnected_indices.push(idx);
                continue;
            }

            loop {
                let frame = match clients[idx].framer.next_frame() {
                    Ok(Some(f)) => f,
                    Ok(None) => break,
                    Err(_) => {
                        disconnected_indices.push(idx);
                        break;
                    }
                };
                let packet = match ClientPacket::decode(&frame) {
                    Ok(p) => p,
                    Err(_) => {
                        disconnected_indices.push(idx);
                        break;
                    }
                };

                let pid = clients[idx].player_id;
                match packet {
                    ClientPacket::AuthRequest {
                        username,
                        password_token,
                        skin_id,
                    } => {
                        let canonical = username.trim().to_ascii_lowercase();
                        let already_online = active_usernames
                            .iter()
                            .any(|(other_id, u)| *other_id != pid && *u == canonical);

                        if already_online {
                            direct_sends.push((
                                pid,
                                ServerPacket::AuthResponse {
                                    success: false,
                                    player_id: 0,
                                    session_token: [0u8; 16],
                                    message: format!(
                                        "User '{}' is already logged in on this server.",
                                        username.trim()
                                    ),
                                    seed: 0,
                                    day_night_time: 0.25,
                                    player_state: SavedPlayerState::default(),
                                    skin_id: 0,
                                },
                            ));
                            continue;
                        }

                        if let Some(ban) = ban_manager.is_banned(&username) {
                            let reason = ban.reason.clone();
                            direct_sends.push((
                                pid,
                                ServerPacket::AuthResponse {
                                    success: false,
                                    player_id: 0,
                                    session_token: [0u8; 16],
                                    message: format!("Banned: {}", reason),
                                    seed: 0,
                                    day_night_time: 0.25,
                                    player_state: SavedPlayerState::default(),
                                    skin_id: 0,
                                },
                            ));
                            continue;
                        }


                        match auth_db.authenticate_or_register(
                            &username,
                            &password_token,
                            skin_id,
                            default_spawn,
                        ) {
                            Ok((account, token, is_new)) => {
                                let mut player_state = account.player_state.clone();
                                if perms_manager.players.is_empty() {
                                    let _ = perms_manager.add_player_group(&account.username, "admin");
                                    let _ = perms_manager.save_to_dir(&world_dir);
                                    player_state.rank = "admin".to_string();
                                    auth_db.update_player_state(&account.username, account.skin_id, player_state.clone());
                                } else {
                                    if player_state.rank.is_empty() {
                                        player_state.rank = "default".to_string();
                                    }
                                    let groups = perms_manager.get_player_groups(&account.username);
                                    if groups.is_empty() || (groups.len() == 1 && groups[0] == "default" && player_state.rank != "default") {
                                        let _ = perms_manager.add_player_group(&account.username, &player_state.rank);
                                        let _ = perms_manager.save_to_dir(&world_dir);
                                    }
                                }
                                clients[idx].authenticated = true;
                                clients[idx].username = account.username.clone();
                                clients[idx].session_token = token;
                                clients[idx].saved_state = player_state.clone();
                                clients[idx].model = PlayerModelState {

                                    player_id: pid,
                                    username: account.username.clone(),
                                    position: account.player_state.position,
                                    velocity: account.player_state.velocity,
                                    yaw: account.player_state.yaw,
                                    pitch: account.player_state.pitch,
                                    held_item: account
                                        .player_state
                                        .inventory
                                        .get(account.player_state.active_hotbar_slot)
                                        .item_id,
                                    walk_anim: 0.0,
                                    swing_anim: 0.0,
                                    hurt_timer: 0.0,
                                    is_sneaking: false,
                                    skin_id: account.skin_id,
                                    health: account.player_state.stats.health,
                                    max_health: account.player_state.stats.max_health,
                                };
                                let _ = auth_db.save_to_dir(&world_dir);

                                let welcome_msg = if is_new {
                                    format!(
                                        "Registered & logged in as '{}'!",
                                        account.username
                                    )
                                } else {
                                    format!("Welcome back, '{}'!", account.username)
                                };

                                direct_sends.push((
                                    pid,
                                    ServerPacket::AuthResponse {
                                        success: true,
                                        player_id: pid,
                                        session_token: token,
                                        message: welcome_msg,
                                        seed: save_data.metadata.seed,
                                        day_night_time: save_data.metadata.day_night_time,
                                        player_state: account.player_state.clone(),
                                        skin_id: account.skin_id,
                                    },
                                ));

                                // Send authoritative world deltas (modified chunks, chests, furnaces, drops)
                                save_data.capture_from_world(&world);
                                let mod_chunks: Vec<_> = save_data
                                    .modified_chunks
                                    .iter()
                                    .map(|(&cp, r)| (cp, r.clone()))
                                    .collect();
                                let chests: Vec<_> = save_data
                                    .chest_containers
                                    .iter()
                                    .map(|(&p, s)| (p, s.clone()))
                                    .collect();
                                let furnaces: Vec<_> = save_data
                                    .furnaces
                                    .iter()
                                    .map(|(&p, f)| (p, f.clone()))
                                    .collect();
                                let drops: Vec<_> = save_data
                                    .item_drops
                                    .iter()
                                    .map(|d| (d.drop_id, d.position, d.velocity, d.item))
                                    .collect();

                                direct_sends.push((
                                    pid,
                                    ServerPacket::WorldDeltaSync {
                                        modified_chunks: mod_chunks,
                                        chests,
                                        furnaces,
                                        item_drops: drops,
                                    },
                                ));

                                // Broadcast join event & system chat message
                                outgoing_broadcasts.push((
                                    Some(pid),
                                    ServerPacket::PlayerJoined {
                                        player: clients[idx].model.clone(),
                                    },
                                ));
                                outgoing_broadcasts.push((
                                    None,
                                    ServerPacket::ChatBroadcast {
                                        sender: "Server".to_string(),
                                        message: format!(
                                            "{} joined the game",
                                            clients[idx].username
                                        ),
                                        is_system: true,
                                    },
                                ));
                            }
                            Err(err_msg) => {
                                direct_sends.push((
                                    pid,
                                    ServerPacket::AuthResponse {
                                        success: false,
                                        player_id: 0,
                                        session_token: [0u8; 16],
                                        message: err_msg,
                                        seed: 0,
                                        day_night_time: 0.25,
                                        player_state: SavedPlayerState::default(),
                                        skin_id: 0,
                                    },
                                ));
                            }
                        }
                    }
                    ClientPacket::PlayerStateUpdate {
                        position,
                        velocity,
                        yaw,
                        pitch,
                        held_item,
                        walk_anim,
                        swing_anim,
                        is_sneaking,
                        health,
                    } if clients[idx].authenticated => {
                        let m = &mut clients[idx].model;
                        m.position = position;
                        m.velocity = velocity;
                        m.yaw = yaw;
                        m.pitch = pitch;
                        m.held_item = held_item;
                        m.walk_anim = walk_anim;
                        m.swing_anim = swing_anim;
                        m.is_sneaking = is_sneaking;
                        m.health = health;

                        clients[idx].saved_state.position = position;
                        clients[idx].saved_state.velocity = velocity;
                        clients[idx].saved_state.yaw = yaw;
                        clients[idx].saved_state.pitch = pitch;
                        clients[idx].saved_state.stats.health = health;
                    }
                    ClientPacket::BlockChange { pos, block_id } if clients[idx].authenticated => {
                        // Ensure the base chunk exists in the server's world container so set_block preserves terrain
                        let (cpos, _, _, _) = world_to_chunk_coords(pos);
                        if world.get_chunk(cpos).is_none() {
                            world.insert_chunk(chunk_generator(cpos));
                        }
                        world.set_block(pos, block_id);
                        if let Some(chunk) = world.get_chunk(cpos) {
                            save_data
                                .modified_chunks
                                .insert(cpos, encode_chunk_rle(chunk));
                        }
                        outgoing_broadcasts
                            .push((Some(pid), ServerPacket::BlockUpdate { pos, block_id }));
                    }
                    ClientPacket::ContainerUpdate {
                        pos,
                        is_furnace,
                        slots,
                        burn_time,
                        max_burn_time,
                        cook_progress,
                    } if clients[idx].authenticated => {
                        if is_furnace {
                            let f = world.get_or_create_furnace(pos);
                            if slots.len() >= 3 {
                                f.input = slots[0];
                                f.fuel = slots[1];
                                f.output = slots[2];
                            }
                            f.burn_time_remaining = burn_time;
                            f.max_burn_time = max_burn_time;
                            f.cook_progress = cook_progress;
                        } else {
                            let chest = world.get_or_create_chest(pos);
                            for (i, &s) in slots.iter().enumerate() {
                                if i < chest.len() {
                                    chest[i] = s;
                                }
                            }
                        }
                        outgoing_broadcasts.push((
                            Some(pid),
                            ServerPacket::ContainerSync {
                                pos,
                                is_furnace,
                                slots,
                                burn_time,
                                max_burn_time,
                                cook_progress,
                            },
                        ));
                    }
                    ClientPacket::SpawnItemDrop {
                        drop_id,
                        position,
                        velocity,
                        item,
                    } if clients[idx].authenticated => {
                        let effective_drop_id = if drop_id == 0
                            || save_data.item_drops.iter().any(|d| d.drop_id == drop_id)
                        {
                            next_local_drop_id()
                        } else {
                            drop_id
                        };
                        let drop = ItemDrop::with_id_and_velocity(
                            effective_drop_id,
                            position,
                            velocity,
                            item,
                        );
                        save_data.item_drops.push(drop);
                        outgoing_broadcasts.push((
                            Some(pid),
                            ServerPacket::ItemDropSpawned {
                                drop_id: effective_drop_id,
                                position,
                                velocity,
                                item,
                            },
                        ));
                    }
                    ClientPacket::PickupItemDrop { drop_id } if clients[idx].authenticated => {
                        if let Some(pos) =
                            save_data.item_drops.iter().position(|d| d.drop_id == drop_id)
                        {
                            let collected = save_data.item_drops.remove(pos);
                            outgoing_broadcasts.push((
                                None,
                                ServerPacket::ItemDropCollected {
                                    drop_id,
                                    collector_player_id: pid,
                                    item: collected.item,
                                },
                            ));
                        } else {
                            direct_sends.push((
                                pid,
                                ServerPacket::ItemDropCollected {
                                    drop_id,
                                    collector_player_id: 0,
                                    item: ItemSlot::EMPTY,
                                },
                            ));
                        }
                    }
                    ClientPacket::PlayerInventorySync {
                        inventory,
                        active_hotbar_slot,
                        stats,
                    } if clients[idx].authenticated => {
                        clients[idx].saved_state.inventory = inventory;
                        clients[idx].saved_state.active_hotbar_slot = active_hotbar_slot;
                        clients[idx].saved_state.stats = stats;
                        auth_db.update_player_state(
                            &clients[idx].username,
                            clients[idx].model.skin_id,
                            clients[idx].saved_state.clone(),
                        );
                    }
                    ClientPacket::AttackPlayer {
                        target_player_id,
                        damage,
                    } if clients[idx].authenticated => {
                        if let Some(target) = clients
                            .iter_mut()
                            .find(|c| c.authenticated && c.player_id == target_player_id)
                        {
                            if target.model.hurt_timer <= 0.0 && target.model.health > 0.0 {
                                target.model.health = (target.model.health - damage).max(0.0);
                                target.model.hurt_timer = 0.45;
                                target.saved_state.stats.health = target.model.health;
                                let new_hp = target.model.health;
                                outgoing_broadcasts.push((
                                    None,
                                    ServerPacket::PlayerDamaged {
                                        target_player_id,
                                        attacker_id: pid,
                                        damage,
                                        new_health: new_hp,
                                    },
                                ));
                            }
                        }
                    }
                    ClientPacket::Chat { message } if clients[idx].authenticated => {
                        let trimmed = message.trim();
                        if !trimmed.is_empty() {
                            if trimmed.starts_with('/') {
                                let online_infos: Vec<OnlinePlayerInfo> = clients
                                    .iter()
                                    .filter(|c| c.authenticated)
                                    .map(|c| OnlinePlayerInfo {
                                        player_id: c.player_id,
                                        username: c.username.clone(),
                                        position: c.model.position,
                                        yaw: c.model.yaw,
                                        pitch: c.model.pitch,
                                    })
                                    .collect();
                                let out = execute_command(
                                    &CommandSource::Player {
                                        player_id: pid,
                                        username: clients[idx].username.clone(),
                                    },
                                    trimmed,
                                    &online_infos,
                                    &mut perms_manager,
                                    &mut ban_manager,
                                    &item_registry,
                                );
                                apply_command_output(
                                    out,
                                    Some(pid),
                                    &mut direct_sends,
                                    &mut outgoing_broadcasts,
                                    &mut clients,
                                    &mut disconnected_indices,
                                    &mut perms_manager,
                                    &mut ban_manager,
                                    &mut auth_db,
                                    default_spawn,
                                    &world_dir,
                                    &stop_flag,
                                );
                            } else {
                                outgoing_broadcasts.push((
                                    None,
                                    ServerPacket::ChatBroadcast {
                                        sender: clients[idx].username.clone(),
                                        message: trimmed.chars().take(120).collect(),
                                        is_system: false,
                                    },
                                ));
                            }
                        }
                    }
                    ClientPacket::SetSleeping { sleeping } if clients[idx].authenticated => {
                        if sleeping && !is_night_ticks(save_data.metadata.day_night_time) {
                            clients[idx].is_sleeping = false;
                            direct_sends.push((
                                pid,
                                ServerPacket::ChatBroadcast {
                                    sender: "Server".to_string(),
                                    message: "You can only sleep at night.".to_string(),
                                    is_system: true,
                                },
                            ));
                        } else {
                            let prev = clients[idx].is_sleeping;
                            clients[idx].is_sleeping = sleeping;
                            if sleeping && !prev {
                                let total = clients.iter().filter(|c| c.authenticated).count();
                                let sleeping_count = clients
                                    .iter()
                                    .filter(|c| c.authenticated && c.is_sleeping)
                                    .count();
                                if total > 1 && sleeping_count * 2 < total {
                                    outgoing_broadcasts.push((
                                        None,
                                        ServerPacket::ChatBroadcast {
                                            sender: "Server".to_string(),
                                            message: format!(
                                                "{} is sleeping ({}/{} players, need at least 50% to skip night).",
                                                clients[idx].username, sleeping_count, total
                                            ),
                                            is_system: true,
                                        },
                                    ));
                                }
                            }
                        }
                    }
                    ClientPacket::Ping { timestamp_ms } => {
                        direct_sends.push((pid, ServerPacket::Pong { timestamp_ms }));
                    }
                    ClientPacket::Disconnect => {
                        disconnected_indices.push(idx);
                        break;
                    }
                    _ => {}
                }
            }
        }

        // Check if enough players (>= 50%) are sleeping to advance night to daytime
        if is_night_ticks(save_data.metadata.day_night_time) {
            let total = clients.iter().filter(|c| c.authenticated).count();
            let sleeping_count = clients
                .iter()
                .filter(|c| c.authenticated && c.is_sleeping)
                .count();
            if total > 0 && sleeping_count > 0 && sleeping_count * 2 >= total {
                save_data.metadata.day_night_time = 1000.0;
                for c in clients.iter_mut() {
                    c.is_sleeping = false;
                }
                outgoing_broadcasts.push((
                    None,
                    ServerPacket::ChatBroadcast {
                        sender: "Server".to_string(),
                        message: "Good morning! Night skipped as players slept.".to_string(),
                        is_system: true,
                    },
                ));
            }
        }

        // Send direct packets first so kicked clients receive their Kick packet
        for (target_pid, pkt) in direct_sends {
            if let Ok(bytes) = pkt.encode() {
                if let Some(c) = clients.iter_mut().find(|c| c.player_id == target_pid) {
                    let _ = c.stream.write_all(&bytes);
                    let _ = c.stream.flush();
                }
            }
        }

        // Remove disconnected clients (in reverse order to preserve indices)
        disconnected_indices.sort_unstable();
        disconnected_indices.dedup();
        for &idx in disconnected_indices.iter().rev() {
            let removed = clients.remove(idx);
            if removed.authenticated {
                log::info!("Player '{}' (id {}) disconnected", removed.username, removed.player_id);
                auth_db.update_player_state(
                    &removed.username,
                    removed.model.skin_id,
                    removed.saved_state,
                );
                let _ = auth_db.save_to_dir(&world_dir);
                outgoing_broadcasts.push((
                    None,
                    ServerPacket::PlayerLeft {
                        player_id: removed.player_id,
                        username: removed.username.clone(),
                    },
                ));
                outgoing_broadcasts.push((
                    None,
                    ServerPacket::ChatBroadcast {
                        sender: "Server".to_string(),
                        message: format!("{} left the game", removed.username),
                        is_system: true,
                    },
                ));
            }
        }

        // Re-check 50% sleep threshold in case a non-sleeping player disconnected
        if !disconnected_indices.is_empty() && is_night_ticks(save_data.metadata.day_night_time) {
            let total = clients.iter().filter(|c| c.authenticated).count();
            let sleeping_count = clients
                .iter()
                .filter(|c| c.authenticated && c.is_sleeping)
                .count();
            if total > 0 && sleeping_count > 0 && sleeping_count * 2 >= total {
                save_data.metadata.day_night_time = 1000.0;
                for c in clients.iter_mut() {
                    c.is_sleeping = false;
                }
                outgoing_broadcasts.push((
                    None,
                    ServerPacket::ChatBroadcast {
                        sender: "Server".to_string(),
                        message: "Good morning! Night skipped as players slept.".to_string(),
                        is_system: true,
                    },
                ));
            }
        }

        // Send broadcast packets
        for (exclude_pid, pkt) in outgoing_broadcasts {
            if let Ok(bytes) = pkt.encode() {
                for c in clients.iter_mut() {
                    if c.authenticated && Some(c.player_id) != exclude_pid {
                        let _ = c.stream.write_all(&bytes);
                    }
                }
            }
        }


        // 3. Server 20 TPS tick: advance day/night cycle, hurt timers, and broadcast WorldSnapshot
        let now = Instant::now();
        if now.duration_since(last_tick) >= tick_interval {
            let dt = now.duration_since(last_tick).as_secs_f32().min(0.2);
            last_tick = now;

            // Advance server day/night time (20 ticks/sec, 24000 ticks full cycle)
            save_data.metadata.total_time += dt;
            save_data.metadata.day_night_time =
                (save_data.metadata.day_night_time + 20.0 * dt).rem_euclid(24000.0);
            if !is_night_ticks(save_data.metadata.day_night_time) {
                for c in clients.iter_mut() {
                    c.is_sleeping = false;
                }
            }

            for c in clients.iter_mut() {
                if c.model.hurt_timer > 0.0 {
                    c.model.hurt_timer = (c.model.hurt_timer - dt).max(0.0);
                }
            }

            let all_players: Vec<PlayerModelState> = clients
                .iter()
                .filter(|c| c.authenticated)
                .map(|c| c.model.clone())
                .collect();

            *online_count.lock().unwrap() = all_players.len();

            if !all_players.is_empty() {
                let snap = ServerPacket::WorldSnapshot {
                    day_night_time: save_data.metadata.day_night_time,
                    players: all_players,
                };
                if let Ok(bytes) = snap.encode() {
                    for c in clients.iter_mut() {
                        if c.authenticated {
                            let _ = c.stream.write_all(&bytes);
                        }
                    }
                }
            }
        }

        // 4. Periodic autosave every 30 seconds
        if now.duration_since(last_autosave) >= Duration::from_secs(30) {
            last_autosave = now;
            for c in &clients {
                if c.authenticated {
                    auth_db.update_player_state(
                        &c.username,
                        c.model.skin_id,
                        c.saved_state.clone(),
                    );
                }
            }
            let _ = auth_db.save_to_dir(&world_dir);
            save_data.capture_from_world(&world);
            let _ = save_world_to_dir(&saves_root, &mut save_data);
        }

        thread::sleep(Duration::from_millis(5));
    }

    // Final save on clean server shutdown
    for c in &clients {
        if c.authenticated {
            auth_db.update_player_state(&c.username, c.model.skin_id, c.saved_state.clone());
        }
    }
    let _ = auth_db.save_to_dir(&world_dir);
    let _ = perms_manager.save_to_dir(&world_dir);
    let _ = ban_manager.save_to_dir(&world_dir);
    save_data.capture_from_world(&world);
    let _ = save_world_to_dir(&saves_root, &mut save_data);

}

// --- Non-Blocking Multiplayer Client ---

/// Chat log entry displayed in the client HUD / chat overlay.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatEntry {
    pub sender: String,
    pub message: String,
    pub is_system: bool,
    pub age: f32,
}

/// Non-blocking TCP client for connecting to a `MultiplayerServer`.
pub struct MultiplayerClient {
    stream: TcpStream,
    framer: PacketFramer,
    pub server_addr: String,
    pub player_id: u32,
    pub username: String,
    pub skin_id: u8,
    pub session_token: [u8; 16],
    pub remote_players: HashMap<u32, PlayerModelState>,
    pub chat_history: Vec<ChatEntry>,
    pub connected: bool,
}

impl MultiplayerClient {
    /// Connects to a `MultiplayerServer` at `addr_str`, performs the authentication handshake,
    /// and returns `(MultiplayerClient, AuthResponse)`.
    pub fn connect_and_authenticate(
        addr_str: &str,
        username: &str,
        password: &str,
        skin_id: u8,
    ) -> Result<(Self, ServerPacket), String> {
        validate_username(username)?;
        let addrs: Vec<SocketAddr> = addr_str
            .trim()
            .to_socket_addrs()
            .map_err(|e| format!("Invalid server address '{}': {}", addr_str, e))?
            .collect();

        if addrs.is_empty() {
            return Err(format!("Could not resolve server address '{}'", addr_str));
        }

        let mut stream = TcpStream::connect_timeout(&addrs[0], Duration::from_secs(4))
            .map_err(|e| format!("Connection failed: {}", e))?;
        let _ = stream.set_nodelay(true);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(4)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(4)));

        let password_token = derive_client_password_token(username, password);
        let auth_req = ClientPacket::AuthRequest {
            username: username.trim().to_string(),
            password_token,
            skin_id: skin_id % 4,
        };
        let bytes = auth_req.encode().map_err(|e| e.to_string())?;
        stream.write_all(&bytes).map_err(|e| e.to_string())?;
        stream.flush().map_err(|e| e.to_string())?;

        // Wait for AuthResponse frame
        let _ = stream.set_nonblocking(true);
        let mut framer = PacketFramer::new();
        let start = Instant::now();
        let auth_resp = loop {
            if start.elapsed() > Duration::from_secs(5) {
                return Err("Timed out waiting for server authentication response.".to_string());
            }
            let alive = framer
                .read_available(&mut stream)
                .map_err(|e| format!("Connection closed during handshake: {}", e))?;
            if !alive {
                return Err("Server closed connection during authentication.".to_string());
            }
            if let Some(frame) = framer.next_frame().map_err(|e| e.to_string())? {
                let pkt = ServerPacket::decode(&frame).map_err(|e| e.to_string())?;
                if matches!(pkt, ServerPacket::AuthResponse { .. }) {
                    break pkt;
                }
            }
            thread::sleep(Duration::from_millis(5));
        };

        if let ServerPacket::AuthResponse {
            success,
            player_id,
            session_token,
            ref message,
            skin_id: assigned_skin,
            ..
        } = auth_resp
        {
            if !success {
                return Err(message.clone());
            }
            let client = Self {
                stream,
                framer,
                server_addr: addr_str.trim().to_string(),
                player_id,
                username: username.trim().to_string(),
                skin_id: assigned_skin,
                session_token,
                remote_players: HashMap::new(),
                chat_history: vec![ChatEntry {
                    sender: "Server".to_string(),
                    message: message.clone(),
                    is_system: true,
                    age: 0.0,
                }],
                connected: true,
            };
            Ok((client, auth_resp))
        } else {
            Err("Unexpected handshake response from server.".to_string())
        }
    }

    /// Sends a `ClientPacket` to the server.
    pub fn send(&mut self, packet: &ClientPacket) {
        if !self.connected {
            return;
        }
        if let Ok(bytes) = packet.encode() {
            if self.stream.write_all(&bytes).is_err() {
                self.connected = false;
            }
        }
    }

    /// Polls all newly arrived `ServerPacket`s without blocking.
    pub fn poll_packets(&mut self) -> Vec<ServerPacket> {
        let mut received = Vec::new();
        if self.connected {
            match self.framer.read_available(&mut self.stream) {
                Ok(true) => {}
                Ok(false) | Err(_) => {
                    self.connected = false;
                }
            }
        }

        while let Ok(Some(frame)) = self.framer.next_frame() {
            if let Ok(pkt) = ServerPacket::decode(&frame) {
                received.push(pkt);
            }
        }
        received
    }


    /// Disconnects cleanly from the server.
    pub fn disconnect(&mut self) {
        if self.connected {
            let _ = self.send(&ClientPacket::Disconnect);
            self.connected = false;
        }
    }
}

impl Drop for MultiplayerClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

// --- Binary Frame & Serialization Helpers ---

fn wrap_frame(payload: &[u8]) -> io::Result<Vec<u8>> {
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.write_all(NET_MAGIC)?;
    frame.write_all(&(payload.len() as u32).to_le_bytes())?;
    frame.write_all(payload)?;
    Ok(frame)
}

fn write_u8<W: Write>(w: &mut W, v: u8) -> io::Result<()> {
    w.write_all(&[v])
}

fn read_u8<R: Read>(r: &mut R) -> io::Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}

fn write_bool<W: Write>(w: &mut W, v: bool) -> io::Result<()> {
    write_u8(w, if v { 1 } else { 0 })
}

fn read_bool<R: Read>(r: &mut R) -> io::Result<bool> {
    Ok(read_u8(r)? != 0)
}

fn write_u16<W: Write>(w: &mut W, v: u16) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn read_u16<R: Read>(r: &mut R) -> io::Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}

fn write_u32<W: Write>(w: &mut W, v: u32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn write_i32<W: Write>(w: &mut W, v: i32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn read_i32<R: Read>(r: &mut R) -> io::Result<i32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(i32::from_le_bytes(b))
}

fn write_u64<W: Write>(w: &mut W, v: u64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn read_u64<R: Read>(r: &mut R) -> io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

fn write_f32<W: Write>(w: &mut W, v: f32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn read_f32<R: Read>(r: &mut R) -> io::Result<f32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(f32::from_le_bytes(b))
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
    if len > 65536 {
        return Err(io::Error::new(ErrorKind::InvalidData, "String too long"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))
}

fn write_slot<W: Write>(w: &mut W, slot: ItemSlot) -> io::Result<()> {
    write_u16(w, slot.item_id.0)?;
    write_u16(w, slot.count)?;
    write_u16(w, slot.durability)
}

fn read_slot<R: Read>(r: &mut R) -> io::Result<ItemSlot> {
    let id = ItemId(read_u16(r)?);
    let count = read_u16(r)?;
    let durability = read_u16(r)?;
    if id == ItemId::EMPTY || count == 0 {
        Ok(ItemSlot::EMPTY)
    } else {
        Ok(ItemSlot::with_durability(id, count, durability))
    }
}

fn write_player_model<W: Write>(w: &mut W, p: &PlayerModelState) -> io::Result<()> {
    write_u32(w, p.player_id)?;
    write_string(w, &p.username)?;
    write_vec3(w, p.position)?;
    write_vec3(w, p.velocity)?;
    write_f32(w, p.yaw)?;
    write_f32(w, p.pitch)?;
    write_u16(w, p.held_item.0)?;
    write_f32(w, p.walk_anim)?;
    write_f32(w, p.swing_anim)?;
    write_f32(w, p.hurt_timer)?;
    write_bool(w, p.is_sneaking)?;
    write_u8(w, p.skin_id)?;
    write_f32(w, p.health)?;
    write_f32(w, p.max_health)
}

fn read_player_model<R: Read>(r: &mut R) -> io::Result<PlayerModelState> {
    Ok(PlayerModelState {
        player_id: read_u32(r)?,
        username: read_string(r)?,
        position: read_vec3(r)?,
        velocity: read_vec3(r)?,
        yaw: read_f32(r)?,
        pitch: read_f32(r)?,
        held_item: ItemId(read_u16(r)?),
        walk_anim: read_f32(r)?,
        swing_anim: read_f32(r)?,
        hurt_timer: read_f32(r)?,
        is_sneaking: read_bool(r)?,
        skin_id: read_u8(r)?,
        health: read_f32(r)?,
        max_health: read_f32(r)?,
    })
}

fn write_saved_player_state<W: Write>(w: &mut W, p: &SavedPlayerState) -> io::Result<()> {
    write_vec3(w, p.position)?;
    write_vec3(w, p.velocity)?;
    write_f32(w, p.yaw)?;
    write_f32(w, p.pitch)?;
    write_u32(w, p.active_hotbar_slot as u32)?;
    write_f32(w, p.stats.health)?;
    write_f32(w, p.stats.max_health)?;
    write_f32(w, p.stats.hunger)?;
    write_f32(w, p.stats.max_hunger)?;
    write_f32(w, p.stats.saturation)?;
    write_f32(w, p.stats.exhaustion)?;
    write_f32(w, p.stats.breath)?;
    write_f32(w, p.stats.max_breath)?;
    write_bool(w, p.stats.is_dead)?;
    write_string(w, &p.rank)?;
    write_u32(w, p.inventory.slots.len() as u32)?;
    for s in &p.inventory.slots {
        write_slot(w, *s)?;
    }
    Ok(())
}

fn read_saved_player_state<R: Read>(r: &mut R) -> io::Result<SavedPlayerState> {
    let position = read_vec3(r)?;
    let velocity = read_vec3(r)?;
    let yaw = read_f32(r)?;
    let pitch = read_f32(r)?;
    let active_hotbar_slot = (read_u32(r)? as usize).min(8);

    let mut stats = PlayerStats::new();
    stats.health = read_f32(r)?;
    stats.max_health = read_f32(r)?;
    stats.hunger = read_f32(r)?;
    stats.max_hunger = read_f32(r)?;
    stats.saturation = read_f32(r)?;
    stats.exhaustion = read_f32(r)?;
    stats.breath = read_f32(r)?;
    stats.max_breath = read_f32(r)?;
    stats.is_dead = read_bool(r)?;
    let rank = read_string(r).unwrap_or_else(|_| "default".to_string());

    let count = read_u32(r)? as usize;
    let mut inventory = Inventory::default();
    for i in 0..count {
        let s = read_slot(r)?;
        if i < Inventory::TOTAL_COUNT {
            inventory.slots[i] = s;
        }
    }

    Ok(SavedPlayerState {
        position,
        velocity,
        yaw,
        pitch,
        stats,
        inventory,
        active_hotbar_slot,
        rank: if rank.is_empty() { "default".to_string() } else { rank },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_encoding_roundtrip() {
        let pkt = ClientPacket::PlayerStateUpdate {
            position: Vec3::new(10.0, 65.0, -4.0),
            velocity: Vec3::new(1.0, 0.0, -0.5),
            yaw: 1.5,
            pitch: -0.25,
            held_item: ItemId::DIAMOND_SWORD,
            walk_anim: 2.1,
            swing_anim: 0.5,
            is_sneaking: true,
            health: 18.0,
        };
        let framed = pkt.encode().unwrap();
        assert_eq!(&framed[0..4], NET_MAGIC);
        let decoded = ClientPacket::decode(&framed[8..]).unwrap();
        assert_eq!(pkt, decoded);

        let spkt = ServerPacket::PlayerJoined {
            player: PlayerModelState::new(7, "BuilderBob", Vec3::new(2.0, 70.0, 3.0), 2),
        };
        let sframed = spkt.encode().unwrap();
        let sdecoded = ServerPacket::decode(&sframed[8..]).unwrap();
        assert_eq!(spkt, sdecoded);
    }

    #[test]
    fn test_multiplayer_server_auth_and_block_sync() {
        let temp_root = std::env::temp_dir().join("blockgame_mp_server_test");
        let _ = std::fs::remove_dir_all(&temp_root);

        let mut server = MultiplayerServer::start(
            "127.0.0.1:0",
            temp_root.clone(),
            "mp_test_world".to_string(),
            "MP Test World".to_string(),
            424242,
            None,
            |cpos| crate::chunk::Chunk::new(cpos),
        )
        .expect("Server should start on ephemeral port");

        let addr_str = server.local_addr().to_string();

        // 1. Client 1 ("Alice") connects & registers
        let (mut client_alice, auth_alice) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "Alice", "alice_pass", 1)
                .expect("Alice should authenticate");

        if let ServerPacket::AuthResponse {
            success,
            seed,
            skin_id,
            ..
        } = auth_alice
        {
            assert!(success);
            assert_eq!(seed, 424242);
            assert_eq!(skin_id, 1);
        } else {
            panic!("Expected AuthResponse");
        }

        // 2. Duplicate login as "Alice" while Alice is online is rejected
        let dup_res =
            MultiplayerClient::connect_and_authenticate(&addr_str, "alice", "alice_pass", 0);
        assert!(dup_res.is_err());

        // 3. Client 2 ("Bob") connects & registers
        let (mut client_bob, _auth_bob) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "Bob", "bob_pass", 2)
                .expect("Bob should authenticate");

        // 4. Alice places a Diamond Block at (5, 70, 5) and sends a chat message
        client_alice.send(&ClientPacket::BlockChange {
            pos: IVec3::new(5, 70, 5),
            block_id: BlockId::DIAMOND_ORE,
        });
        client_alice.send(&ClientPacket::Chat {
            message: "Hello Bob!".to_string(),
        });

        // Wait briefly for server tick & broadcast
        thread::sleep(Duration::from_millis(120));

        let bob_packets = client_bob.poll_packets();
        let saw_block_update = bob_packets.iter().any(|p| {
            matches!(
                p,
                ServerPacket::BlockUpdate {
                    pos,
                    block_id
                } if *pos == IVec3::new(5, 70, 5) && *block_id == BlockId::DIAMOND_ORE
            )
        });
        let saw_chat = bob_packets.iter().any(|p| {
            matches!(
                p,
                ServerPacket::ChatBroadcast {
                    sender,
                    message,
                    is_system: false,
                } if sender == "Alice" && message == "Hello Bob!"
            )
        });
        let saw_snapshot_with_two_players = bob_packets.iter().any(|p| {
            matches!(
                p,
                ServerPacket::WorldSnapshot { players, .. } if players.len() == 2
            )
        });

        assert!(saw_block_update, "Bob should receive Alice's BlockUpdate");
        assert!(saw_chat, "Bob should receive Alice's chat message");
        assert!(
            saw_snapshot_with_two_players,
            "Bob should receive WorldSnapshot containing both Alice and Bob"
        );

        client_alice.disconnect();
        client_bob.disconnect();
        server.shutdown();

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_multiplayer_item_pickup_authoritative_no_duplication() {
        let temp_root = std::env::temp_dir().join("blockgame_mp_item_pickup_test");
        let _ = std::fs::remove_dir_all(&temp_root);

        let mut server = MultiplayerServer::start(
            "127.0.0.1:0",
            temp_root.clone(),
            "mp_item_test_world".to_string(),
            "MP Item Test World".to_string(),
            999999,
            None,
            |cpos| crate::chunk::Chunk::new(cpos),
        )
        .expect("Server should start on ephemeral port");

        let addr_str = server.local_addr().to_string();

        // 1. Client 1 (Alice) connects
        let (mut client_alice, _auth_alice) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "AliceDrop", "pass", 1)
                .expect("Alice should authenticate");
        let alice_id = client_alice.player_id;
        assert_ne!(alice_id, 0);

        // 2. Client 2 (Bob) connects
        let (mut client_bob, _auth_bob) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "BobDrop", "pass", 2)
                .expect("Bob should authenticate");
        let bob_id = client_bob.player_id;
        assert_ne!(bob_id, 0);

        // 3. Alice spawns an item drop (Diamond) with drop_id
        let diamond_slot = ItemSlot::new(ItemId::DIAMOND, 5);
        let drop_id = ((alice_id as u64) << 40) | 4242;
        let drop_pos = Vec3::new(10.0, 65.0, 10.0);
        let drop_vel = Vec3::new(0.0, 2.5, 0.0);

        client_alice.send(&ClientPacket::SpawnItemDrop {
            drop_id,
            position: drop_pos,
            velocity: drop_vel,
            item: diamond_slot,
        });

        // Wait briefly for server processing & broadcast
        thread::sleep(Duration::from_millis(120));

        // 4. Verify Bob receives ServerPacket::ItemDropSpawned with drop_id
        let bob_packets = client_bob.poll_packets();
        let saw_spawn = bob_packets.iter().any(|p| {
            matches!(
                p,
                ServerPacket::ItemDropSpawned {
                    drop_id: id,
                    position,
                    item,
                    ..
                } if *id == drop_id && *position == drop_pos && *item == diamond_slot
            )
        });
        assert!(saw_spawn, "Bob must receive ItemDropSpawned with matching drop_id");

        // 5. Alice picks up the drop
        client_alice.send(&ClientPacket::PickupItemDrop { drop_id });
        thread::sleep(Duration::from_millis(120));

        // 6. Alice receives ItemDropCollected giving her the diamonds
        let alice_packets = client_alice.poll_packets();
        let alice_collected = alice_packets.iter().any(|p| {
            matches!(
                p,
                ServerPacket::ItemDropCollected {
                    drop_id: id,
                    collector_player_id,
                    item,
                } if *id == drop_id && *collector_player_id == alice_id && *item == diamond_slot
            )
        });
        assert!(alice_collected, "Alice must receive authoritative ItemDropCollected with items");

        // 7. Bob receives ItemDropCollected informing him that Alice collected it
        let bob_packets = client_bob.poll_packets();
        let bob_saw_collected = bob_packets.iter().any(|p| {
            matches!(
                p,
                ServerPacket::ItemDropCollected {
                    drop_id: id,
                    collector_player_id,
                    item,
                } if *id == drop_id && *collector_player_id == alice_id && *item == diamond_slot
            )
        });
        assert!(bob_saw_collected, "Bob must receive ItemDropCollected removing the drop from world");

        // 8. Bob now attempts to pick up the same drop_id (race condition / duplication attempt)
        client_bob.send(&ClientPacket::PickupItemDrop { drop_id });
        thread::sleep(Duration::from_millis(120));

        let bob_packets = client_bob.poll_packets();
        let bob_rejected = bob_packets.iter().any(|p| {
            matches!(
                p,
                ServerPacket::ItemDropCollected {
                    drop_id: id,
                    collector_player_id: 0,
                    item,
                } if *id == drop_id && item.is_empty()
            )
        });
        assert!(
            bob_rejected,
            "Bob's pickup attempt for already collected item must be rejected with 0 items (no duplication)"
        );

        // 9. Client 3 (Charlie) connects; WorldDeltaSync must contain 0 item drops
        let (mut client_charlie, _auth_charlie) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "CharlieDrop", "pass", 3)
                .expect("Charlie should authenticate");
        thread::sleep(Duration::from_millis(120));
        let charlie_packets = client_charlie.poll_packets();
        let saw_empty_sync = charlie_packets.iter().any(|p| {
            matches!(
                p,
                ServerPacket::WorldDeltaSync { item_drops, .. } if item_drops.iter().all(|(id, ..)| *id != drop_id)
            )
        });
        assert!(
            saw_empty_sync,
            "Future players must not receive collected drops in WorldDeltaSync"
        );

        client_alice.disconnect();
        client_bob.disconnect();
        client_charlie.disconnect();
        server.shutdown();

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_server_commands_and_permissions() {
        let temp_root = std::env::temp_dir().join(format!("bg_cmd_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_root);

        let mut server = MultiplayerServer::start(
            "127.0.0.1:0",
            temp_root.clone(),
            "test_cmd_world".to_string(),
            "Test Cmd World".to_string(),
            777,
            None,
            |_| crate::chunk::Chunk::new(ChunkPos(glam::IVec3::ZERO)),
        )
        .expect("Server should start");

        let addr_str = server.local_addr().to_string();

        // 1. Connect admin player (first to join -> auto-assigned admin group)
        let (mut client_admin, _auth_admin) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "AdminUser", "secret1", 0)
                .expect("Admin should authenticate");

        // 2. Connect regular player Bob
        let (mut client_bob, _auth_bob) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "BobUser", "secret2", 1)
                .expect("Bob should authenticate");

        thread::sleep(Duration::from_millis(150));

        // 3. Admin gives Bob 5 diamonds using in-game command
        client_admin.send(&ClientPacket::Chat {
            message: "/give BobUser diamond 5".to_string(),
        });

        thread::sleep(Duration::from_millis(150));
        let bob_pkts = client_bob.poll_packets();
        let gave_diamonds = bob_pkts.iter().any(|p| {
            matches!(
                p,
                ServerPacket::ReceiveItem { item }
                    if item.item_id == ItemId::DIAMOND && item.count == 5
            )
        });
        assert!(gave_diamonds, "Bob must receive diamond item from /give command");

        // 4. Admin teleports Bob to (120, 80, 240)
        client_admin.send(&ClientPacket::Chat {
            message: "/tp BobUser 120 80 240".to_string(),
        });

        thread::sleep(Duration::from_millis(150));
        let bob_pkts = client_bob.poll_packets();
        let teleported = bob_pkts.iter().any(|p| {
            matches!(
                p,
                ServerPacket::TeleportPlayer { position, .. }
                    if (*position - Vec3::new(120.0, 80.0, 240.0)).length() < 0.01
            )
        });
        assert!(teleported, "Bob must receive TeleportPlayer packet to target coordinates");

        // 5. Console issues ban command on Bob
        server.execute_command("ban BobUser Griefing spawn area");

        thread::sleep(Duration::from_millis(200));
        let bob_pkts = client_bob.poll_packets();
        let kicked_for_ban = bob_pkts.iter().any(|p| {
            matches!(
                p,
                ServerPacket::Kick { reason }
                    if reason.contains("Banned") && reason.contains("Griefing")
            )
        });
        assert!(kicked_for_ban, "Bob must receive Kick packet containing ban reason");

        // 6. Bob tries to reconnect while banned; should be rejected
        let reconnect_res =
            MultiplayerClient::connect_and_authenticate(&addr_str, "BobUser", "secret2", 1);
        match reconnect_res {
            Err(err_msg) => {
                assert!(
                    err_msg.contains("Banned") || err_msg.contains("banned"),
                    "Error message must specify ban: {}",
                    err_msg
                );
            }
            Ok(_) => panic!("Banned user must fail login!"),
        }


        // 7. Console unbans Bob
        server.execute_command("unban BobUser");
        thread::sleep(Duration::from_millis(150));

        // 8. Bob can now successfully log back in
        let (mut client_bob2, _auth_bob2) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "BobUser", "secret2", 1)
                .expect("Bob should be able to reconnect after unban");

        // Clean shutdown
        client_admin.disconnect();
        client_bob2.disconnect();
        server.shutdown();

        // 9. Verify persistence files exist
        let world_dir = temp_root.join("test_cmd_world");
        assert!(world_dir.join("permissions.txt").exists(), "permissions.txt must be saved");
        assert!(world_dir.join("bans.txt").exists(), "bans.txt must be saved");

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_multiplayer_sleep_50_percent_threshold() {
        let temp_root = std::env::temp_dir().join("blockgame_net_sleep_test");
        let _ = std::fs::remove_dir_all(&temp_root);
        std::fs::create_dir_all(&temp_root).unwrap();

        let mut meta = WorldMetadata::new("Sleep World", 4242);
        meta.folder_name = "sleep_world".to_string();
        meta.day_night_time = 18000.0; // Midnight (nighttime)
        let save_data = WorldSaveData::new(meta, SavedPlayerState::default());

        let mut server = MultiplayerServer::start(
            "127.0.0.1:0",
            temp_root.clone(),
            "sleep_world".to_string(),
            "Sleep World".to_string(),
            4242,
            Some(save_data),
            |cpos| crate::chunk::Chunk::new(cpos),
        )
        .expect("Failed to start server");

        let addr_str = server.local_addr().to_string();
        let (mut p1, _) = MultiplayerClient::connect_and_authenticate(&addr_str, "SleepA", "pw1", 0).unwrap();
        let (mut p2, _) = MultiplayerClient::connect_and_authenticate(&addr_str, "SleepB", "pw2", 1).unwrap();
        let (mut p3, _) = MultiplayerClient::connect_and_authenticate(&addr_str, "SleepC", "pw3", 0).unwrap();

        // 1 of 3 sleeping (< 50%): should remain nighttime
        p1.send(&ClientPacket::SetSleeping { sleeping: true });
        thread::sleep(Duration::from_millis(120));
        let pkts = p1.poll_packets();
        let latest_time = pkts.iter().rev().find_map(|p| match p {
            ServerPacket::WorldSnapshot { day_night_time, .. } => Some(*day_night_time),
            _ => None,
        }).expect("Expected WorldSnapshot");
        assert!(is_night_ticks(latest_time), "1/3 sleeping must not skip night, got {}", latest_time);

        // 2 of 3 sleeping (>= 50%): should advance to daytime (~1000.0)
        p2.send(&ClientPacket::SetSleeping { sleeping: true });
        thread::sleep(Duration::from_millis(120));
        let pkts2 = p3.poll_packets();
        let morning_time = pkts2.iter().rev().find_map(|p| match p {
            ServerPacket::WorldSnapshot { day_night_time, .. } => Some(*day_night_time),
            _ => None,
        }).expect("Expected WorldSnapshot");
        assert!(!is_night_ticks(morning_time), "2/3 sleeping (>=50%) must advance to daytime, got {}", morning_time);

        p1.disconnect();
        p2.disconnect();
        p3.disconnect();
        server.shutdown();
        let _ = std::fs::remove_dir_all(&temp_root);
    }
}

