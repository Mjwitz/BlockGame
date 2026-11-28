use std::collections::HashMap;
use std::fs;
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
    character::CharacterAppearance,
    chunk::ChunkPos,
    command::{
        execute_command, CommandOutput, CommandSource, OnlinePlayerInfo, PendingTpaRequest,
        SkillCommandAction,
    },
    entity::{next_local_drop_id, ItemDrop},
    item::{Inventory, ItemId, ItemRegistry, ItemSlot},
    permissions::{BanManager, PermissionsManager},
    player::PlayerStats,
    save::{
        encode_chunk_rle, load_world_from_dir, save_world_to_dir, SavedPlayerState, WorldMetadata,
        WorldSaveData,
    },
    skills::{PlayerSkills, SKILL_COUNT},
    world::{world_to_chunk_coords, FurnaceState, World},
};

pub const NET_MAGIC: &[u8; 4] = b"BGMP";
pub const DEFAULT_SERVER_PORT: u16 = 25565;
const MAX_PACKET_SIZE: usize = 16 * 1024 * 1024; // 16 MB max frame

/// Complete state of a player character for 3D model rendering, animations, and nametags.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerModelState {
    pub player_id: u32,
    pub username: String,
    pub prefix: String,
    pub position: Vec3, // Feet position (AABB min.y)
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub held_item: ItemId,
    pub walk_anim: f32,
    pub swing_anim: f32,
    pub hurt_timer: f32,
    pub is_sneaking: bool,
    pub appearance: CharacterAppearance,
    pub health: f32,
    pub max_health: f32,
}

impl PlayerModelState {
    pub fn new(
        player_id: u32,
        username: impl Into<String>,
        position: Vec3,
        appearance: CharacterAppearance,
    ) -> Self {
        Self {
            player_id,
            username: username.into(),
            prefix: String::new(),
            position,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            held_item: ItemId::EMPTY,
            walk_anim: 0.0,
            swing_anim: 0.0,
            hurt_timer: 0.0,
            is_sneaking: false,
            appearance: appearance.sanitized(),
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
        appearance: CharacterAppearance,
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
    VoiceFrame {
        seq: u32,
        data: Vec<u8>,
    },
    SkillsSync {
        skills: PlayerSkills,
    },
    TempleInteract {
        pos: IVec3,
        temple_id: u64,
        kind: u8,
        overworld_origin: IVec3,
        portal_pos: IVec3,
    },
    AttackTempleMonster {
        monster_id: u32,
        damage: f32,
        knockback: Vec3,
    },
    InteractAnimal {
        animal_id: u32,
        held_item: ItemId,
    },
    AttackAnimal {
        animal_id: u32,
        damage: f32,
        knockback: Vec3,
    },
    CookingPotUpdate {
        pos: IVec3,
        ingredients: [ItemSlot; 4],
        fuel: ItemSlot,
        output: ItemSlot,
        burn_time: f32,
        max_burn_time: f32,
        cook_progress: f32,
    },
    InteractShopkeeper {
        npc_id: u32,
    },
    ShopBuy {
        npc_id: u32,
        offer_idx: u16,
    },
    ShopSell {
        npc_id: u32,
        offer_idx: u16,
    },
    ShopEnhance {
        npc_id: u32,
        inv_slot_idx: u16,
        offer_idx: u16,
    },
    AttackVillageNpc {
        npc_id: u32,
        damage: f32,
        knockback: Vec3,
    },
    RegisterVillageBlueprint {
        village_id: u64,
        theme: crate::village::VillageTheme,
        origin: IVec3,
        shops: Vec<(crate::village::ShopkeeperProfession, Vec3, Vec3)>,
        guard_positions: Vec<Vec3>,
    },
    AchievementUnlocked {
        achievement_id: String,
    },
}

/// Compact network representation of a server-authoritative Village NPC (Shopkeeper or Village Guard).
#[derive(Debug, Clone, PartialEq)]
pub struct SyncedVillageNpcState {
    pub id: u32,
    pub village_id: u64,
    pub is_guard: bool,
    pub profession: Option<crate::village::ShopkeeperProfession>,
    pub name: String,
    pub appearance: CharacterAppearance,
    pub position: Vec3,
    pub yaw: f32,
    pub walk_anim: f32,
    pub swing_anim: f32,
    pub hurt_timer: f32,
    pub health: f32,
    pub max_health: f32,
    pub held_item: ItemId,
}

/// Compact network representation of a server-authoritative passive animal (ADR-0004).
#[derive(Debug, Clone, PartialEq)]
pub struct SyncedAnimalState {
    pub id: u32,
    pub kind: crate::animal::AnimalKind,
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub hurt_timer: f32,
    pub love_timer: f32,
    pub breed_cooldown: f32,
    pub baby_timer: f32,
    pub is_moving: bool,
    pub walk_anim_timer: f32,
}

/// Compact network representation of a server-authoritative boss or temple guardian.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncedMonsterState {
    pub id: u32,
    pub kind: crate::monster::MonsterKind,
    pub position: Vec3,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub hurt_timer: f32,
    pub walk_anim_timer: f32,
    pub phase: u8,
    pub in_combat: bool,
    pub invulnerable: bool,
}

/// Compact network representation of a server-authoritative boss/temple projectile.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncedArrowState {
    pub id: u32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub stuck: bool,
}

/// Maximum distance (in blocks) at which proximity voice chat is relayed and audible.
pub const MAX_VOICE_RANGE: f32 = 48.0;

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
        appearance: CharacterAppearance,
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
        prefix: String,
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
    VoiceBroadcast {
        sender_id: u32,
        seq: u32,
        position: Vec3,
        data: Vec<u8>,
    },
    GameModeUpdate {
        game_mode: crate::player::GameMode,
    },
    SkillsUpdate {
        skills: PlayerSkills,
    },
    TempleMonstersSync {
        monsters: Vec<SyncedMonsterState>,
    },
    BossHudUpdate {
        hud: Option<crate::temple::BossHudState>,
    },
    TempleProtectionSync {
        zones: Vec<crate::temple::TempleBounds>,
    },
    TempleArrowsSync {
        arrows: Vec<SyncedArrowState>,
    },
    AnimalsSync {
        animals: Vec<SyncedAnimalState>,
    },
    CookingPotSync {
        pos: IVec3,
        ingredients: [ItemSlot; 4],
        fuel: ItemSlot,
        output: ItemSlot,
        burn_time: f32,
        max_burn_time: f32,
        cook_progress: f32,
    },
    VillageNpcsSync {
        npcs: Vec<SyncedVillageNpcState>,
    },
    ShopUiSync {
        snapshot: crate::village::ShopUiSnapshot,
        status_message: String,
    },
    InventorySync {
        slots: Vec<ItemSlot>,
    },
    AchievementEligibility {
        enabled: bool,
        reason: Option<String>,
    },
    AchievementEvent {
        event: crate::achievement::AchievementEvent,
    },
    FluidUpdate {
        pos: IVec3,
        block_id: BlockId,
        level: u8,
    },
    ClearChat,
}


impl ClientPacket {
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let mut payload = Vec::with_capacity(128);
        match self {
            Self::AuthRequest {
                username,
                password_token,
                appearance,
            } => {
                write_u8(&mut payload, 1)?;
                write_string(&mut payload, username)?;
                payload.write_all(password_token)?;
                appearance.write_binary(&mut payload)?;
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
            Self::VoiceFrame { seq, data } => {
                write_u8(&mut payload, 13)?;
                write_u32(&mut payload, *seq)?;
                write_u16(&mut payload, data.len().min(2048) as u16)?;
                payload.extend_from_slice(&data[..data.len().min(2048)]);
            }
            Self::SkillsSync { skills } => {
                write_u8(&mut payload, 14)?;
                write_player_skills(&mut payload, skills)?;
            }
            Self::TempleInteract {
                pos,
                temple_id,
                kind,
                overworld_origin,
                portal_pos,
            } => {
                write_u8(&mut payload, 15)?;
                write_ivec3(&mut payload, *pos)?;
                write_u64(&mut payload, *temple_id)?;
                write_u8(&mut payload, *kind)?;
                write_ivec3(&mut payload, *overworld_origin)?;
                write_ivec3(&mut payload, *portal_pos)?;
            }
            Self::AttackTempleMonster {
                monster_id,
                damage,
                knockback,
            } => {
                write_u8(&mut payload, 16)?;
                write_u32(&mut payload, *monster_id)?;
                write_f32(&mut payload, *damage)?;
                write_vec3(&mut payload, *knockback)?;
            }
            Self::InteractAnimal {
                animal_id,
                held_item,
            } => {
                write_u8(&mut payload, 17)?;
                write_u32(&mut payload, *animal_id)?;
                write_u16(&mut payload, held_item.0)?;
            }
            Self::AttackAnimal {
                animal_id,
                damage,
                knockback,
            } => {
                write_u8(&mut payload, 18)?;
                write_u32(&mut payload, *animal_id)?;
                write_f32(&mut payload, *damage)?;
                write_vec3(&mut payload, *knockback)?;
            }
            Self::CookingPotUpdate {
                pos,
                ingredients,
                fuel,
                output,
                burn_time,
                max_burn_time,
                cook_progress,
            } => {
                write_u8(&mut payload, 19)?;
                write_ivec3(&mut payload, *pos)?;
                for slot in ingredients {
                    write_slot(&mut payload, *slot)?;
                }
                write_slot(&mut payload, *fuel)?;
                write_slot(&mut payload, *output)?;
                write_f32(&mut payload, *burn_time)?;
                write_f32(&mut payload, *max_burn_time)?;
                write_f32(&mut payload, *cook_progress)?;
            }
            Self::InteractShopkeeper { npc_id } => {
                write_u8(&mut payload, 20)?;
                write_u32(&mut payload, *npc_id)?;
            }
            Self::ShopBuy { npc_id, offer_idx } => {
                write_u8(&mut payload, 21)?;
                write_u32(&mut payload, *npc_id)?;
                write_u16(&mut payload, *offer_idx)?;
            }
            Self::ShopSell { npc_id, offer_idx } => {
                write_u8(&mut payload, 22)?;
                write_u32(&mut payload, *npc_id)?;
                write_u16(&mut payload, *offer_idx)?;
            }
            Self::ShopEnhance {
                npc_id,
                inv_slot_idx,
                offer_idx,
            } => {
                write_u8(&mut payload, 23)?;
                write_u32(&mut payload, *npc_id)?;
                write_u16(&mut payload, *inv_slot_idx)?;
                write_u16(&mut payload, *offer_idx)?;
            }
            Self::AttackVillageNpc {
                npc_id,
                damage,
                knockback,
            } => {
                write_u8(&mut payload, 24)?;
                write_u32(&mut payload, *npc_id)?;
                write_f32(&mut payload, *damage)?;
                write_vec3(&mut payload, *knockback)?;
            }
            Self::RegisterVillageBlueprint {
                village_id,
                theme,
                origin,
                shops,
                guard_positions,
            } => {
                write_u8(&mut payload, 25)?;
                write_u64(&mut payload, *village_id)?;
                write_u8(&mut payload, theme.to_u8())?;
                write_ivec3(&mut payload, *origin)?;
                write_u32(&mut payload, shops.len() as u32)?;
                for &(prof, home, court) in shops {
                    write_u8(&mut payload, prof.to_u8())?;
                    write_vec3(&mut payload, home)?;
                    write_vec3(&mut payload, court)?;
                }
                write_u32(&mut payload, guard_positions.len() as u32)?;
                for &gp in guard_positions {
                    write_vec3(&mut payload, gp)?;
                }
            }
            Self::AchievementUnlocked { achievement_id } => {
                write_u8(&mut payload, 26)?;
                write_string(&mut payload, achievement_id)?;
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
                let appearance = CharacterAppearance::read_binary(&mut r)?;
                Ok(Self::AuthRequest {
                    username,
                    password_token,
                    appearance,
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
            13 => {
                let seq = read_u32(&mut r)?;
                let len = read_u16(&mut r)? as usize;
                if len > 2048 {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        "VoiceFrame payload too large",
                    ));
                }
                let mut data = vec![0u8; len];
                r.read_exact(&mut data)?;
                Ok(Self::VoiceFrame { seq, data })
            }
            14 => {
                let skills = read_player_skills(&mut r)?;
                Ok(Self::SkillsSync { skills })
            }
            15 => {
                let pos = read_ivec3(&mut r)?;
                let temple_id = read_u64(&mut r)?;
                let kind = read_u8(&mut r)?;
                let overworld_origin = read_ivec3(&mut r)?;
                let portal_pos = read_ivec3(&mut r)?;
                Ok(Self::TempleInteract {
                    pos,
                    temple_id,
                    kind,
                    overworld_origin,
                    portal_pos,
                })
            }
            16 => {
                let monster_id = read_u32(&mut r)?;
                let damage = read_f32(&mut r)?;
                let knockback = read_vec3(&mut r)?;
                Ok(Self::AttackTempleMonster {
                    monster_id,
                    damage,
                    knockback,
                })
            }
            17 => {
                let animal_id = read_u32(&mut r)?;
                let held_item = ItemId(read_u16(&mut r)?);
                Ok(Self::InteractAnimal {
                    animal_id,
                    held_item,
                })
            }
            18 => {
                let animal_id = read_u32(&mut r)?;
                let damage = read_f32(&mut r)?;
                let knockback = read_vec3(&mut r)?;
                Ok(Self::AttackAnimal {
                    animal_id,
                    damage,
                    knockback,
                })
            }
            19 => {
                let pos = read_ivec3(&mut r)?;
                let ingredients = [
                    read_slot(&mut r)?,
                    read_slot(&mut r)?,
                    read_slot(&mut r)?,
                    read_slot(&mut r)?,
                ];
                let fuel = read_slot(&mut r)?;
                let output = read_slot(&mut r)?;
                let burn_time = read_f32(&mut r)?;
                let max_burn_time = read_f32(&mut r)?;
                let cook_progress = read_f32(&mut r)?;
                Ok(Self::CookingPotUpdate {
                    pos,
                    ingredients,
                    fuel,
                    output,
                    burn_time,
                    max_burn_time,
                    cook_progress,
                })
            }
            20 => Ok(Self::InteractShopkeeper {
                npc_id: read_u32(&mut r)?,
            }),
            21 => Ok(Self::ShopBuy {
                npc_id: read_u32(&mut r)?,
                offer_idx: read_u16(&mut r)?,
            }),
            22 => Ok(Self::ShopSell {
                npc_id: read_u32(&mut r)?,
                offer_idx: read_u16(&mut r)?,
            }),
            23 => Ok(Self::ShopEnhance {
                npc_id: read_u32(&mut r)?,
                inv_slot_idx: read_u16(&mut r)?,
                offer_idx: read_u16(&mut r)?,
            }),
            24 => Ok(Self::AttackVillageNpc {
                npc_id: read_u32(&mut r)?,
                damage: read_f32(&mut r)?,
                knockback: read_vec3(&mut r)?,
            }),
            25 => {
                let village_id = read_u64(&mut r)?;
                let theme = crate::village::VillageTheme::from_u8(read_u8(&mut r)?)
                    .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "Invalid VillageTheme"))?;
                let origin = read_ivec3(&mut r)?;
                let scount = (read_u32(&mut r)? as usize).min(16);
                let mut shops = Vec::with_capacity(scount);
                for _ in 0..scount {
                    let prof = crate::village::ShopkeeperProfession::from_u8(read_u8(&mut r)?)
                        .ok_or_else(|| {
                            io::Error::new(ErrorKind::InvalidData, "Invalid ShopkeeperProfession")
                        })?;
                    let home = read_vec3(&mut r)?;
                    let court = read_vec3(&mut r)?;
                    shops.push((prof, home, court));
                }
                let gcount = (read_u32(&mut r)? as usize).min(16);
                let mut guard_positions = Vec::with_capacity(gcount);
                for _ in 0..gcount {
                    guard_positions.push(read_vec3(&mut r)?);
                }
                Ok(Self::RegisterVillageBlueprint {
                    village_id,
                    theme,
                    origin,
                    shops,
                    guard_positions,
                })
            }
            26 => {
                let achievement_id = read_string(&mut r)?;
                Ok(Self::AchievementUnlocked { achievement_id })
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
                appearance,
            } => {
                write_u8(&mut payload, 1)?;
                write_bool(&mut payload, *success)?;
                write_u32(&mut payload, *player_id)?;
                payload.write_all(session_token)?;
                write_string(&mut payload, message)?;
                write_u32(&mut payload, *seed)?;
                write_f32(&mut payload, *day_night_time)?;
                write_saved_player_state(&mut payload, player_state)?;
                appearance.write_binary(&mut payload)?;
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
                prefix,
                message,
                is_system,
            } => {
                write_u8(&mut payload, 10)?;
                write_string(&mut payload, sender)?;
                write_string(&mut payload, prefix)?;
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
            Self::VoiceBroadcast {
                sender_id,
                seq,
                position,
                data,
            } => {
                write_u8(&mut payload, 16)?;
                write_u32(&mut payload, *sender_id)?;
                write_u32(&mut payload, *seq)?;
                write_vec3(&mut payload, *position)?;
                write_u16(&mut payload, data.len().min(2048) as u16)?;
                payload.extend_from_slice(&data[..data.len().min(2048)]);
            }
            Self::GameModeUpdate { game_mode } => {
                write_u8(&mut payload, 17)?;
                write_u8(&mut payload, *game_mode as u8)?;
            }
            Self::SkillsUpdate { skills } => {
                write_u8(&mut payload, 18)?;
                write_player_skills(&mut payload, skills)?;
            }
            Self::TempleMonstersSync { monsters } => {
                write_u8(&mut payload, 19)?;
                write_u32(&mut payload, monsters.len() as u32)?;
                for m in monsters {
                    write_u32(&mut payload, m.id)?;
                    write_u8(&mut payload, m.kind.to_u8())?;
                    write_vec3(&mut payload, m.position)?;
                    write_f32(&mut payload, m.yaw)?;
                    write_f32(&mut payload, m.health)?;
                    write_f32(&mut payload, m.max_health)?;
                    write_f32(&mut payload, m.hurt_timer)?;
                    write_f32(&mut payload, m.walk_anim_timer)?;
                    write_u8(&mut payload, m.phase)?;
                    write_bool(&mut payload, m.in_combat)?;
                    write_bool(&mut payload, m.invulnerable)?;
                }
            }
            Self::BossHudUpdate { hud } => {
                write_u8(&mut payload, 20)?;
                write_bool(&mut payload, hud.is_some())?;
                if let Some(h) = hud {
                    write_string(&mut payload, &h.name)?;
                    write_u8(&mut payload, h.kind.to_u8())?;
                    write_f32(&mut payload, h.health)?;
                    write_f32(&mut payload, h.max_health)?;
                    write_u8(&mut payload, h.phase)?;
                    write_f32(&mut payload, h.distance)?;
                    write_bool(&mut payload, h.invulnerable)?;
                }
            }
            Self::TempleProtectionSync { zones } => {
                write_u8(&mut payload, 21)?;
                write_u32(&mut payload, zones.len() as u32)?;
                for z in zones {
                    write_ivec3(&mut payload, z.min)?;
                    write_ivec3(&mut payload, z.max)?;
                }
            }
            Self::TempleArrowsSync { arrows } => {
                write_u8(&mut payload, 22)?;
                write_u32(&mut payload, arrows.len() as u32)?;
                for a in arrows {
                    write_u32(&mut payload, a.id)?;
                    write_vec3(&mut payload, a.position)?;
                    write_vec3(&mut payload, a.velocity)?;
                    write_bool(&mut payload, a.stuck)?;
                }
            }
            Self::AnimalsSync { animals } => {
                write_u8(&mut payload, 23)?;
                write_u32(&mut payload, animals.len() as u32)?;
                for a in animals {
                    write_u32(&mut payload, a.id)?;
                    write_u8(&mut payload, a.kind.to_u8())?;
                    write_vec3(&mut payload, a.position)?;
                    write_vec3(&mut payload, a.velocity)?;
                    write_f32(&mut payload, a.yaw)?;
                    write_f32(&mut payload, a.health)?;
                    write_f32(&mut payload, a.max_health)?;
                    write_f32(&mut payload, a.hurt_timer)?;
                    write_f32(&mut payload, a.love_timer)?;
                    write_f32(&mut payload, a.breed_cooldown)?;
                    write_f32(&mut payload, a.baby_timer)?;
                    write_bool(&mut payload, a.is_moving)?;
                    write_f32(&mut payload, a.walk_anim_timer)?;
                }
            }
            Self::CookingPotSync {
                pos,
                ingredients,
                fuel,
                output,
                burn_time,
                max_burn_time,
                cook_progress,
            } => {
                write_u8(&mut payload, 24)?;
                write_ivec3(&mut payload, *pos)?;
                for slot in ingredients {
                    write_slot(&mut payload, *slot)?;
                }
                write_slot(&mut payload, *fuel)?;
                write_slot(&mut payload, *output)?;
                write_f32(&mut payload, *burn_time)?;
                write_f32(&mut payload, *max_burn_time)?;
                write_f32(&mut payload, *cook_progress)?;
            }
            Self::VillageNpcsSync { npcs } => {
                write_u8(&mut payload, 25)?;
                write_u32(&mut payload, npcs.len() as u32)?;
                for n in npcs {
                    write_u32(&mut payload, n.id)?;
                    write_u64(&mut payload, n.village_id)?;
                    write_bool(&mut payload, n.is_guard)?;
                    write_u8(&mut payload, n.profession.map_or(255, |p| p.to_u8()))?;
                    write_string(&mut payload, &n.name)?;
                    n.appearance.write_binary(&mut payload)?;
                    write_vec3(&mut payload, n.position)?;
                    write_f32(&mut payload, n.yaw)?;
                    write_f32(&mut payload, n.walk_anim)?;
                    write_f32(&mut payload, n.swing_anim)?;
                    write_f32(&mut payload, n.hurt_timer)?;
                    write_f32(&mut payload, n.health)?;
                    write_f32(&mut payload, n.max_health)?;
                    write_u16(&mut payload, n.held_item.0)?;
                }
            }
            Self::ShopUiSync {
                snapshot,
                status_message,
            } => {
                write_u8(&mut payload, 26)?;
                write_u32(&mut payload, snapshot.npc_id)?;
                write_string(&mut payload, &snapshot.name)?;
                write_u8(&mut payload, snapshot.profession.to_u8())?;
                write_u8(&mut payload, snapshot.shop_tier)?;
                write_u32(&mut payload, snapshot.coins_spent)?;
                write_bool(&mut payload, snapshot.next_tier_coins.is_some())?;
                if let Some(nt) = snapshot.next_tier_coins {
                    write_u32(&mut payload, nt)?;
                }
                write_u64(&mut payload, snapshot.ticks_until_restock)?;
                write_u32(&mut payload, snapshot.daily_sell_remaining)?;
                write_u32(&mut payload, snapshot.daily_sell_limit)?;
                write_u32(&mut payload, snapshot.buy_offers.len() as u32)?;
                for b in &snapshot.buy_offers {
                    write_u16(&mut payload, b.offer_idx)?;
                    write_u8(&mut payload, b.tier)?;
                    write_u16(&mut payload, b.item_id.0)?;
                    write_u16(&mut payload, b.count)?;
                    write_u32(&mut payload, b.price_coins)?;
                    write_u16(&mut payload, b.stock_remaining)?;
                    write_u16(&mut payload, b.max_stock)?;
                    write_bool(&mut payload, b.is_shared)?;
                    write_bool(&mut payload, b.unlocked)?;
                }
                write_u32(&mut payload, snapshot.sell_offers.len() as u32)?;
                for s in &snapshot.sell_offers {
                    write_u16(&mut payload, s.offer_idx)?;
                    write_u16(&mut payload, s.item_id.0)?;
                    write_u16(&mut payload, s.count)?;
                    write_u32(&mut payload, s.payout_coins)?;
                }
                write_u32(&mut payload, snapshot.enhance_offers.len() as u32)?;
                for e in &snapshot.enhance_offers {
                    write_u16(&mut payload, e.offer_idx)?;
                    write_u8(&mut payload, e.tier)?;
                    write_u8(&mut payload, e.enhancement.to_u8())?;
                    write_u32(&mut payload, e.base_price_coins)?;
                    write_u16(&mut payload, e.stock_remaining)?;
                    write_u16(&mut payload, e.max_stock)?;
                    write_bool(&mut payload, e.unlocked)?;
                }
                write_string(&mut payload, status_message)?;
            }
            Self::InventorySync { slots } => {
                write_u8(&mut payload, 27)?;
                write_u32(&mut payload, slots.len() as u32)?;
                for s in slots {
                    write_slot(&mut payload, *s)?;
                }
            }
            Self::AchievementEligibility { enabled, reason } => {
                write_u8(&mut payload, 28)?;
                write_bool(&mut payload, *enabled)?;
                match reason {
                    Some(msg) => {
                        write_bool(&mut payload, true)?;
                        write_string(&mut payload, msg)?;
                    }
                    None => write_bool(&mut payload, false)?,
                }
            }
            Self::AchievementEvent { event } => {
                write_u8(&mut payload, 29)?;
                event.encode(&mut payload)?;
            }
            Self::FluidUpdate {
                pos,
                block_id,
                level,
            } => {
                write_u8(&mut payload, 30)?;
                write_ivec3(&mut payload, *pos)?;
                write_u8(&mut payload, block_id.0)?;
                write_u8(&mut payload, *level & 0x0F)?;
            }
            Self::ClearChat => {
                write_u8(&mut payload, 31)?;
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
                let appearance = CharacterAppearance::read_binary(&mut r)?;
                Ok(Self::AuthResponse {
                    success,
                    player_id,
                    session_token,
                    message,
                    seed,
                    day_night_time,
                    player_state,
                    appearance,
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
                let prefix = read_string(&mut r)?;
                let message = read_string(&mut r)?;
                let is_system = read_bool(&mut r)?;
                Ok(Self::ChatBroadcast {
                    sender,
                    prefix,
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
            16 => {
                let sender_id = read_u32(&mut r)?;
                let seq = read_u32(&mut r)?;
                let position = read_vec3(&mut r)?;
                let len = read_u16(&mut r)? as usize;
                if len > 2048 {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        "VoiceBroadcast payload too large",
                    ));
                }
                let mut data = vec![0u8; len];
                r.read_exact(&mut data)?;
                Ok(Self::VoiceBroadcast {
                    sender_id,
                    seq,
                    position,
                    data,
                })
            }
            17 => {
                let gm = crate::player::GameMode::from_u8(read_u8(&mut r)?);
                Ok(Self::GameModeUpdate { game_mode: gm })
            }
            18 => {
                let skills = read_player_skills(&mut r)?;
                Ok(Self::SkillsUpdate { skills })
            }
            19 => {
                let count = read_u32(&mut r)? as usize;
                let mut monsters = Vec::with_capacity(count.min(256));
                for _ in 0..count {
                    let id = read_u32(&mut r)?;
                    let kind_u8 = read_u8(&mut r)?;
                    let position = read_vec3(&mut r)?;
                    let yaw = read_f32(&mut r)?;
                    let health = read_f32(&mut r)?;
                    let max_health = read_f32(&mut r)?;
                    let hurt_timer = read_f32(&mut r)?;
                    let walk_anim_timer = read_f32(&mut r)?;
                    let phase = read_u8(&mut r)?;
                    let in_combat = read_bool(&mut r)?;
                    let invulnerable = read_bool(&mut r)?;
                    if let Some(kind) = crate::monster::MonsterKind::from_u8(kind_u8) {
                        monsters.push(SyncedMonsterState {
                            id,
                            kind,
                            position,
                            yaw,
                            health,
                            max_health,
                            hurt_timer,
                            walk_anim_timer,
                            phase,
                            in_combat,
                            invulnerable,
                        });
                    }
                }
                Ok(Self::TempleMonstersSync { monsters })
            }
            20 => {
                let has_hud = read_bool(&mut r)?;
                let hud = if has_hud {
                    let name = read_string(&mut r)?;
                    let kind_u8 = read_u8(&mut r)?;
                    let health = read_f32(&mut r)?;
                    let max_health = read_f32(&mut r)?;
                    let phase = read_u8(&mut r)?;
                    let distance = read_f32(&mut r)?;
                    let invulnerable = read_bool(&mut r)?;
                    crate::monster::MonsterKind::from_u8(kind_u8).map(|kind| {
                        crate::temple::BossHudState {
                            name,
                            kind,
                            health,
                            max_health,
                            phase,
                            distance,
                            invulnerable,
                        }
                    })
                } else {
                    None
                };
                Ok(Self::BossHudUpdate { hud })
            }
            21 => {
                let count = read_u32(&mut r)? as usize;
                let mut zones = Vec::with_capacity(count.min(512));
                for _ in 0..count {
                    let min = read_ivec3(&mut r)?;
                    let max = read_ivec3(&mut r)?;
                    zones.push(crate::temple::TempleBounds { min, max });
                }
                Ok(Self::TempleProtectionSync { zones })
            }
            22 => {
                let count = read_u32(&mut r)? as usize;
                let mut arrows = Vec::with_capacity(count.min(512));
                for _ in 0..count {
                    let id = read_u32(&mut r)?;
                    let position = read_vec3(&mut r)?;
                    let velocity = read_vec3(&mut r)?;
                    let stuck = read_bool(&mut r)?;
                    arrows.push(SyncedArrowState {
                        id,
                        position,
                        velocity,
                        stuck,
                    });
                }
                Ok(Self::TempleArrowsSync { arrows })
            }
            23 => {
                let count = read_u32(&mut r)? as usize;
                let mut animals = Vec::with_capacity(count.min(512));
                for _ in 0..count.min(512) {
                    let id = read_u32(&mut r)?;
                    let kind_u8 = read_u8(&mut r)?;
                    let position = read_vec3(&mut r)?;
                    let velocity = read_vec3(&mut r)?;
                    let yaw = read_f32(&mut r)?;
                    let health = read_f32(&mut r)?;
                    let max_health = read_f32(&mut r)?;
                    let hurt_timer = read_f32(&mut r)?;
                    let love_timer = read_f32(&mut r)?;
                    let breed_cooldown = read_f32(&mut r)?;
                    let baby_timer = read_f32(&mut r)?;
                    let is_moving = read_bool(&mut r)?;
                    let walk_anim_timer = read_f32(&mut r)?;
                    if let Some(kind) = crate::animal::AnimalKind::from_u8(kind_u8) {
                        animals.push(SyncedAnimalState {
                            id,
                            kind,
                            position,
                            velocity,
                            yaw,
                            health,
                            max_health,
                            hurt_timer,
                            love_timer,
                            breed_cooldown,
                            baby_timer,
                            is_moving,
                            walk_anim_timer,
                        });
                    }
                }
                Ok(Self::AnimalsSync { animals })
            }
            24 => {
                let pos = read_ivec3(&mut r)?;
                let ingredients = [
                    read_slot(&mut r)?,
                    read_slot(&mut r)?,
                    read_slot(&mut r)?,
                    read_slot(&mut r)?,
                ];
                let fuel = read_slot(&mut r)?;
                let output = read_slot(&mut r)?;
                let burn_time = read_f32(&mut r)?;
                let max_burn_time = read_f32(&mut r)?;
                let cook_progress = read_f32(&mut r)?;
                Ok(Self::CookingPotSync {
                    pos,
                    ingredients,
                    fuel,
                    output,
                    burn_time,
                    max_burn_time,
                    cook_progress,
                })
            }
            25 => {
                let count = (read_u32(&mut r)? as usize).min(2048);
                let mut npcs = Vec::with_capacity(count);
                for _ in 0..count {
                    let id = read_u32(&mut r)?;
                    let village_id = read_u64(&mut r)?;
                    let is_guard = read_bool(&mut r)?;
                    let prof_raw = read_u8(&mut r)?;
                    let profession = if prof_raw == 255 {
                        None
                    } else {
                        crate::village::ShopkeeperProfession::from_u8(prof_raw)
                    };
                    let name = read_string(&mut r)?;
                    let appearance = CharacterAppearance::read_binary(&mut r)?;
                    let position = read_vec3(&mut r)?;
                    let yaw = read_f32(&mut r)?;
                    let walk_anim = read_f32(&mut r)?;
                    let swing_anim = read_f32(&mut r)?;
                    let hurt_timer = read_f32(&mut r)?;
                    let health = read_f32(&mut r)?;
                    let max_health = read_f32(&mut r)?;
                    let held_item = ItemId(read_u16(&mut r)?);
                    npcs.push(SyncedVillageNpcState {
                        id,
                        village_id,
                        is_guard,
                        profession,
                        name,
                        appearance,
                        position,
                        yaw,
                        walk_anim,
                        swing_anim,
                        hurt_timer,
                        health,
                        max_health,
                        held_item,
                    });
                }
                Ok(Self::VillageNpcsSync { npcs })
            }
            26 => {
                let npc_id = read_u32(&mut r)?;
                let name = read_string(&mut r)?;
                let profession = crate::village::ShopkeeperProfession::from_u8(read_u8(&mut r)?)
                    .ok_or_else(|| {
                        io::Error::new(ErrorKind::InvalidData, "Invalid ShopkeeperProfession")
                    })?;
                let shop_tier = read_u8(&mut r)?;
                let coins_spent = read_u32(&mut r)?;
                let has_next = read_bool(&mut r)?;
                let next_tier_coins = if has_next {
                    Some(read_u32(&mut r)?)
                } else {
                    None
                };
                let ticks_until_restock = read_u64(&mut r)?;
                let daily_sell_remaining = read_u32(&mut r)?;
                let daily_sell_limit = read_u32(&mut r)?;

                let bcount = (read_u32(&mut r)? as usize).min(256);
                let mut buy_offers = Vec::with_capacity(bcount);
                for _ in 0..bcount {
                    buy_offers.push(crate::village::ShopBuyEntry {
                        offer_idx: read_u16(&mut r)?,
                        tier: read_u8(&mut r)?,
                        item_id: ItemId(read_u16(&mut r)?),
                        count: read_u16(&mut r)?,
                        price_coins: read_u32(&mut r)?,
                        stock_remaining: read_u16(&mut r)?,
                        max_stock: read_u16(&mut r)?,
                        is_shared: read_bool(&mut r)?,
                        unlocked: read_bool(&mut r)?,
                    });
                }

                let scount = (read_u32(&mut r)? as usize).min(256);
                let mut sell_offers = Vec::with_capacity(scount);
                for _ in 0..scount {
                    sell_offers.push(crate::village::ShopSellEntry {
                        offer_idx: read_u16(&mut r)?,
                        item_id: ItemId(read_u16(&mut r)?),
                        count: read_u16(&mut r)?,
                        payout_coins: read_u32(&mut r)?,
                    });
                }

                let ecount = (read_u32(&mut r)? as usize).min(64);
                let mut enhance_offers = Vec::with_capacity(ecount);
                for _ in 0..ecount {
                    let offer_idx = read_u16(&mut r)?;
                    let tier = read_u8(&mut r)?;
                    let enhancement = crate::item::Enhancement::from_u8(read_u8(&mut r)?)
                        .ok_or_else(|| {
                            io::Error::new(ErrorKind::InvalidData, "Invalid Enhancement")
                        })?;
                    let base_price_coins = read_u32(&mut r)?;
                    let stock_remaining = read_u16(&mut r)?;
                    let max_stock = read_u16(&mut r)?;
                    let unlocked = read_bool(&mut r)?;
                    enhance_offers.push(crate::village::ShopEnhanceEntry {
                        offer_idx,
                        tier,
                        enhancement,
                        base_price_coins,
                        stock_remaining,
                        max_stock,
                        unlocked,
                    });
                }
                let status_message = read_string(&mut r)?;

                Ok(Self::ShopUiSync {
                    snapshot: crate::village::ShopUiSnapshot {
                        npc_id,
                        name,
                        profession,
                        shop_tier,
                        coins_spent,
                        next_tier_coins,
                        ticks_until_restock,
                        daily_sell_remaining,
                        daily_sell_limit,
                        buy_offers,
                        sell_offers,
                        enhance_offers,
                    },
                    status_message,
                })
            }
            27 => {
                let count = (read_u32(&mut r)? as usize).min(128);
                let mut slots = Vec::with_capacity(count);
                for _ in 0..count {
                    slots.push(read_slot(&mut r)?);
                }
                Ok(Self::InventorySync { slots })
            }
            28 => {
                let enabled = read_bool(&mut r)?;
                let has_reason = read_bool(&mut r)?;
                let reason = if has_reason {
                    Some(read_string(&mut r)?)
                } else {
                    None
                };
                Ok(Self::AchievementEligibility { enabled, reason })
            }
            29 => {
                let event = crate::achievement::AchievementEvent::decode(&mut r)?;
                Ok(Self::AchievementEvent { event })
            }
            30 => {
                let pos = read_ivec3(&mut r)?;
                let block_id = BlockId(read_u8(&mut r)?);
                let level = read_u8(&mut r)? & 0x0F;
                Ok(Self::FluidUpdate {
                    pos,
                    block_id,
                    level,
                })
            }
            31 => Ok(Self::ClearChat),
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

/// Inactivity timeout for authenticated multiplayer clients before disconnecting (15 seconds).
pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(15);
/// Inactivity timeout for new connections awaiting authentication (10 seconds).
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(10);

struct ConnectedClient {
    stream: TcpStream,
    addr: SocketAddr,
    framer: PacketFramer,
    authenticated: bool,
    player_id: u32,
    username: String,
    session_token: [u8; 16],
    model: PlayerModelState,
    saved_state: SavedPlayerState,
    is_sleeping: bool,
    last_activity: Instant,
    is_dead: bool,
    achievements_eligible: bool,
    announced_achievements: std::collections::HashSet<String>,
}

/// Authoritative Multiplayer Server managing authentication, persistent user accounts,
/// world state synchronization, block updates, containers, and chat.
pub struct MultiplayerServer {
    bind_addr: SocketAddr,
    stop_flag: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
    online_count: Arc<Mutex<usize>>,
    command_tx: Sender<String>,
    broadcast_achievements: Arc<AtomicBool>,
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
        let config_dir = saves_root.join("config");
        Self::start_with_config_dir(
            bind_addr,
            saves_root,
            config_dir,
            folder_name,
            world_name,
            default_seed,
            initial_world_data,
            chunk_generator,
        )
    }

    pub fn start_with_config_dir<F>(
        bind_addr: impl ToSocketAddrs,
        saves_root: PathBuf,
        config_dir: PathBuf,
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
        let broadcast_achievements = Arc::new(AtomicBool::new(true));
        let broadcast_ach_clone = broadcast_achievements.clone();
        let (command_tx, command_rx) = mpsc::channel();
        let gen_arc: Arc<dyn Fn(ChunkPos) -> crate::chunk::Chunk + Send + Sync> =
            Arc::new(chunk_generator);

        let handle = thread::Builder::new()
            .name("blockgame-mp-server".to_string())
            .spawn(move || {
                run_server_loop(
                    listener,
                    saves_root,
                    config_dir,
                    folder_name,
                    world_name,
                    default_seed,
                    initial_world_data,
                    gen_arc,
                    stop_clone,
                    online_clone,
                    broadcast_ach_clone,
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
            broadcast_achievements,
        })
    }

    /// Configures whether player Achievement unlocks are broadcast in multiplayer chat.
    pub fn set_broadcast_achievements(&self, enabled: bool) {
        self.broadcast_achievements.store(enabled, Ordering::Relaxed);
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
    automessage_mgr: &mut crate::automessage::AutoMessageManager,
    moderation_mgr: &mut crate::moderation::ModerationManager,
    default_spawn: &mut Vec3,
    world_dir: &std::path::Path,
    config_dir: &std::path::Path,
    stop_flag: &Arc<AtomicBool>,
) {
    let actor_name: String = source_pid
        .and_then(|pid| clients.iter().find(|c| c.player_id == pid).map(|c| c.username.clone()))
        .unwrap_or_else(|| "Console".to_string());
    if output.save_permissions {
        let _ = perms_manager.save_to_dir(config_dir);
        for c in clients.iter_mut() {
            if c.authenticated {
                c.model.prefix = perms_manager.get_player_prefix(&c.username);
            }
        }
    }
    if output.save_bans {
        let _ = ban_manager.save_to_dir(config_dir);
    }

    if !output.moderation_actions.is_empty() {
        let mut mod_feedback: Vec<String> = Vec::new();
        for action in output.moderation_actions {
            match action {
                crate::command::ModerationCommandAction::Mute {
                    target,
                    duration_secs,
                    reason,
                } => {
                    let entry = moderation_mgr.mute(&target, &actor_name, duration_secs, &reason);
                    let _ = moderation_mgr.save_mutes(config_dir);
                    let dur_label = duration_secs
                        .map(crate::moderation::format_duration_secs)
                        .unwrap_or_else(|| "permanently".to_string());
                    mod_feedback.push(format!(
                        "Muted '{}' ({}) - Reason: {}",
                        entry.username, dur_label, entry.reason
                    ));
                    if let Some(tc) = clients
                        .iter()
                        .find(|c| c.authenticated && c.username.eq_ignore_ascii_case(&target))
                    {
                        direct_sends.push((
                            tc.player_id,
                            ServerPacket::ChatBroadcast {
                                sender: "Server".to_string(),
                                prefix: String::new(),
                                message: format!(
                                    "&cYou have been muted ({}) by {}: {}",
                                    dur_label, actor_name, entry.reason
                                ),
                                is_system: true,
                            },
                        ));
                    }
                }
                crate::command::ModerationCommandAction::Unmute { target } => {
                    if moderation_mgr.unmute(&target) {
                        let _ = moderation_mgr.save_mutes(config_dir);
                        mod_feedback.push(format!("Unmuted '{}'.", target));
                        if let Some(tc) = clients
                            .iter()
                            .find(|c| c.authenticated && c.username.eq_ignore_ascii_case(&target))
                        {
                            direct_sends.push((
                                tc.player_id,
                                ServerPacket::ChatBroadcast {
                                    sender: "Server".to_string(),
                                    prefix: String::new(),
                                    message: "&aYou have been unmuted.".to_string(),
                                    is_system: true,
                                },
                            ));
                        }
                    } else {
                        mod_feedback.push(format!("Player '{}' is not muted.", target));
                    }
                }
                crate::command::ModerationCommandAction::Warn { target, reason } => {
                    let entry = moderation_mgr.warn(&target, &actor_name, &reason);
                    let _ = moderation_mgr.save_warnings(config_dir);
                    let count = moderation_mgr.get_warnings(&target).len();
                    mod_feedback.push(format!(
                        "Warned '{}' (#{} total) - Reason: {}",
                        entry.username, count, entry.reason
                    ));
                    if let Some(tc) = clients
                        .iter()
                        .find(|c| c.authenticated && c.username.eq_ignore_ascii_case(&target))
                    {
                        direct_sends.push((
                            tc.player_id,
                            ServerPacket::ChatBroadcast {
                                sender: "Server".to_string(),
                                prefix: String::new(),
                                message: format!(
                                    "&e[Warning] &cYou were warned by {}: {}",
                                    actor_name, entry.reason
                                ),
                                is_system: true,
                            },
                        ));
                    }
                }
                crate::command::ModerationCommandAction::Warnings { target } => {
                    let list = moderation_mgr.get_warnings(&target);
                    if list.is_empty() {
                        mod_feedback.push(format!("Player '{}' has no warnings.", target));
                    } else {
                        mod_feedback.push(format!(
                            "Warnings for '{}' ({}):",
                            target,
                            list.len()
                        ));
                        for (i, w) in list.iter().enumerate() {
                            mod_feedback.push(format!(
                                "  {}. [by {}] {}",
                                i + 1,
                                w.warned_by,
                                w.reason
                            ));
                        }
                    }
                }
                crate::command::ModerationCommandAction::ClearWarnings { target } => {
                    let cleared = moderation_mgr.clear_warnings(&target);
                    if cleared > 0 {
                        let _ = moderation_mgr.save_warnings(config_dir);
                    }
                    mod_feedback.push(format!(
                        "Cleared {} warning(s) for '{}'.",
                        cleared, target
                    ));
                }
                crate::command::ModerationCommandAction::WordFilterStatus => {
                    mod_feedback.push(format!(
                        "Word Filter: {} | Mode: {} | Words: {}",
                        if moderation_mgr.word_filter.enabled { "ON" } else { "OFF" },
                        moderation_mgr.word_filter.mode.as_str(),
                        moderation_mgr.word_filter.words.len(),
                    ));
                }
                crate::command::ModerationCommandAction::WordFilterSetEnabled(enabled) => {
                    moderation_mgr.word_filter.enabled = enabled;
                    let _ = moderation_mgr.save_word_filter(config_dir);
                    mod_feedback.push(format!(
                        "Word Filter turned {}.",
                        if enabled { "ON" } else { "OFF" }
                    ));
                }
                crate::command::ModerationCommandAction::WordFilterSetMode(mode) => {
                    moderation_mgr.word_filter.mode = mode;
                    let _ = moderation_mgr.save_word_filter(config_dir);
                    mod_feedback.push(format!("Word Filter mode set to '{}'.", mode.as_str()));
                }
                crate::command::ModerationCommandAction::WordFilterList => {
                    if moderation_mgr.word_filter.words.is_empty() {
                        mod_feedback.push("Word Filter list is empty.".to_string());
                    } else {
                        mod_feedback.push(format!(
                            "Filtered words ({}): {}",
                            moderation_mgr.word_filter.words.len(),
                            moderation_mgr.word_filter.words.join(", ")
                        ));
                    }
                }
                crate::command::ModerationCommandAction::WordFilterAdd(word) => {
                    if moderation_mgr.add_filter_word(&word) {
                        let _ = moderation_mgr.save_word_filter(config_dir);
                        mod_feedback.push(format!("Added '{}' to Word Filter.", word));
                    } else {
                        mod_feedback.push(format!("'{}' is already in Word Filter or empty.", word));
                    }
                }
                crate::command::ModerationCommandAction::WordFilterRemove(word) => {
                    if moderation_mgr.remove_filter_word(&word) {
                        let _ = moderation_mgr.save_word_filter(config_dir);
                        mod_feedback.push(format!("Removed '{}' from Word Filter.", word));
                    } else {
                        mod_feedback.push(format!("'{}' is not in Word Filter.", word));
                    }
                }
                crate::command::ModerationCommandAction::ChatLockToggle(explicit) => {
                    let new_state = explicit.unwrap_or(!moderation_mgr.chat_settings.chat_locked);
                    moderation_mgr.chat_settings.chat_locked = new_state;
                    let _ = moderation_mgr.save_chat_settings(config_dir);
                    outgoing_broadcasts.push((
                        None,
                        ServerPacket::ChatBroadcast {
                            sender: "Server".to_string(),
                            prefix: String::new(),
                            message: if new_state {
                                "&cGlobal Chat Lock enabled by staff.".to_string()
                            } else {
                                "&aGlobal Chat Lock disabled by staff.".to_string()
                            },
                            is_system: true,
                        },
                    ));
                }
                crate::command::ModerationCommandAction::SlowmodeSet(secs) => {
                    moderation_mgr.chat_settings.slowmode_secs = secs;
                    let _ = moderation_mgr.save_chat_settings(config_dir);
                    outgoing_broadcasts.push((
                        None,
                        ServerPacket::ChatBroadcast {
                            sender: "Server".to_string(),
                            prefix: String::new(),
                            message: if secs == 0 {
                                "&aChat Slowmode disabled.".to_string()
                            } else {
                                format!("&eChat Slowmode set to {}s.", secs)
                            },
                            is_system: true,
                        },
                    ));
                }
                crate::command::ModerationCommandAction::ClearChat => {
                    outgoing_broadcasts.push((None, ServerPacket::ClearChat));
                    outgoing_broadcasts.push((
                        None,
                        ServerPacket::ChatBroadcast {
                            sender: "Server".to_string(),
                            prefix: String::new(),
                            message: format!("&7Chat was cleared by {}.", actor_name),
                            is_system: true,
                        },
                    ));
                }
            }
        }
        for line in mod_feedback {
            if let Some(pid) = source_pid {
                direct_sends.push((
                    pid,
                    ServerPacket::ChatBroadcast {
                        sender: "Server".to_string(),
                        prefix: String::new(),
                        message: line,
                        is_system: true,
                    },
                ));
            } else {
                log::info!("[Console] {}", line);
            }
        }
    }

    if !output.automessage_actions.is_empty() {
        let mut am_feedback: Vec<String> = Vec::new();
        let mut am_dirty = false;
        for action in output.automessage_actions {
            match action {
                crate::command::AutoMessageCommandAction::Status => {
                    am_feedback.push(format!(
                        "Auto-Messages: {} | Interval: {}s | Order: {} | Messages: {}",
                        if automessage_mgr.config.enabled { "ON" } else { "OFF" },
                        automessage_mgr.config.interval_secs,
                        automessage_mgr.config.order.as_str(),
                        automessage_mgr.config.messages.len(),
                    ));
                }
                crate::command::AutoMessageCommandAction::SetEnabled(enabled) => {
                    automessage_mgr.set_enabled(enabled);
                    am_dirty = true;
                    am_feedback.push(format!(
                        "Auto-Messages turned {}.",
                        if enabled { "ON" } else { "OFF" }
                    ));
                }
                crate::command::AutoMessageCommandAction::SetInterval(secs) => {
                    match automessage_mgr.set_interval_secs(secs) {
                        Ok(()) => {
                            am_dirty = true;
                            am_feedback.push(format!("Auto-Message interval set to {}s.", secs));
                        }
                        Err(err) => am_feedback.push(format!("[Error] {}", err)),
                    }
                }
                crate::command::AutoMessageCommandAction::SetOrder(order) => {
                    automessage_mgr.set_order(order);
                    am_dirty = true;
                    am_feedback.push(format!("Auto-Message order set to '{}'.", order.as_str()));
                }
                crate::command::AutoMessageCommandAction::List => {
                    if automessage_mgr.config.messages.is_empty() {
                        am_feedback.push("No Auto-Messages configured.".to_string());
                    } else {
                        am_feedback.push(format!(
                            "Auto-Messages ({}, every {}s, {}):",
                            if automessage_mgr.config.enabled { "ON" } else { "OFF" },
                            automessage_mgr.config.interval_secs,
                            automessage_mgr.config.order.as_str(),
                        ));
                        for (i, msg) in automessage_mgr.config.messages.iter().enumerate() {
                            am_feedback.push(format!("  {}. {}", i + 1, msg));
                        }
                    }
                }
                crate::command::AutoMessageCommandAction::Add(msg) => {
                    let count = automessage_mgr.add_message(msg);
                    am_dirty = true;
                    am_feedback.push(format!("Added Auto-Message #{}.", count));
                }
                crate::command::AutoMessageCommandAction::Remove(idx) => {
                    match automessage_mgr.remove_message(idx) {
                        Ok(_) => {
                            am_dirty = true;
                            am_feedback.push(format!("Removed Auto-Message #{}.", idx));
                        }
                        Err(err) => am_feedback.push(format!("[Error] {}", err)),
                    }
                }
            }
        }
        if am_dirty {
            let _ = automessage_mgr.save_to_dir(config_dir);
        }
        for line in am_feedback {
            if let Some(pid) = source_pid {
                direct_sends.push((
                    pid,
                    ServerPacket::ChatBroadcast {
                        sender: "Server".to_string(),
                        prefix: String::new(),
                        message: line,
                        is_system: true,
                    },
                ));
            } else {
                log::info!("[Console] {}", line);
            }
        }
    }

    for (target_pid, msg) in output.direct_messages {
        direct_sends.push((
            target_pid,
            ServerPacket::ChatBroadcast {
                sender: "Server".to_string(),
                prefix: String::new(),
                message: msg,
                is_system: true,
            },
        ));
    }

    if let Some(pos) = output.new_spawn {
        *default_spawn = pos;
        let _ = fs::write(world_dir.join("spawn.txt"), format!("{},{},{}", pos.x, pos.y, pos.z));
    }

    if let Some((pid, home_name, pos, yaw, pitch)) = output.set_home {
        if let Some(c) = clients.iter_mut().find(|c| c.player_id == pid) {
            c.saved_state.homes.insert(home_name.clone(), (pos, yaw, pitch));
            let _ = crate::save::save_player_data(world_dir, &c.username, &c.saved_state);
            let key = c.username.trim().to_ascii_lowercase();
            if let Some(acc) = auth_db.accounts.get_mut(&key) {
                acc.player_state.homes.insert(home_name, (pos, yaw, pitch));
                let _ = auth_db.save_to_dir(world_dir);
            }
        }
    }

    if let Some((pid, home_name)) = output.del_home {
        if let Some(c) = clients.iter_mut().find(|c| c.player_id == pid) {
            c.saved_state.homes.remove(&home_name);
            let _ = crate::save::save_player_data(world_dir, &c.username, &c.saved_state);
            let key = c.username.trim().to_ascii_lowercase();
            if let Some(acc) = auth_db.accounts.get_mut(&key) {
                acc.player_state.homes.remove(&home_name);
                let _ = auth_db.save_to_dir(world_dir);
            }
        }
    }

    for fb in output.feedback {
        if let Some(pid) = source_pid {
            let sender_name = clients
                .iter()
                .find(|c| c.player_id == pid)
                .map(|c| c.username.as_str())
                .unwrap_or("Player");
            log::info!("[Command Feedback -> {}] {}", sender_name, fb);
            direct_sends.push((
                pid,
                ServerPacket::ChatBroadcast {
                    sender: "Server".to_string(),
                    prefix: String::new(),
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
        log::info!("[Broadcast] {}", msg);
        outgoing_broadcasts.push((
            None,
            ServerPacket::ChatBroadcast {
                sender: "Server".to_string(),
                prefix: String::new(),
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

    // Per-player cheat tainting MUST be queued before `gives` / `gamemode_updates` / `skill_actions`
    // so the client receives AchievementEligibility(false) before any cheat-granted item or XP arrives.
    for target_pid in output.cheat_taint_players {
        if let Some(c) = clients.iter_mut().find(|c| c.player_id == target_pid) {
            if c.achievements_eligible {
                c.achievements_eligible = false;
                let reason = "Achievements disabled for this session (admin/cheat command used)."
                    .to_string();
                direct_sends.push((
                    target_pid,
                    ServerPacket::AchievementEligibility {
                        enabled: false,
                        reason: Some(reason.clone()),
                    },
                ));
                direct_sends.push((
                    target_pid,
                    ServerPacket::ChatBroadcast {
                        sender: "Server".to_string(),
                        prefix: String::new(),
                        message: format!("[Achievements] {}", reason),
                        is_system: true,
                    },
                ));
            }
        }
    }

    for (target_pid, slot) in output.gives {
        direct_sends.push((target_pid, ServerPacket::ReceiveItem { item: slot }));
    }

    for (target_pid, new_mode) in output.gamemode_updates {
        if let Some(c) = clients.iter_mut().find(|c| c.player_id == target_pid) {
            c.saved_state.stats.game_mode = new_mode;
            let _ = crate::save::save_player_data(world_dir, &c.username, &c.saved_state);
        }
        direct_sends.push((target_pid, ServerPacket::GameModeUpdate { game_mode: new_mode }));
    }

    for action in output.skill_actions {
        match action {
            SkillCommandAction::View {
                requester_pid,
                skill,
            } => {
                if let Some(c) = clients.iter().find(|c| c.player_id == requester_pid) {
                    let skills = &c.saved_state.skills;
                    let mut lines = Vec::new();
                    if let Some(sk) = skill {
                        let prog = skills.get(sk);
                        let next = PlayerSkills::xp_for_next_level(prog.level);
                        lines.push(format!(
                            "[Skills] {} Lv {}/{} (XP: {}/{})",
                            sk.name(),
                            prog.level,
                            crate::skills::MAX_SKILL_LEVEL,
                            prog.xp,
                            next
                        ));
                        lines.push(format!("  Perks: {}", skills.perks_summary(sk)));
                        for m in sk.milestones() {
                            let tag = if prog.level >= m.level { "[Unlocked]" } else { "[Locked]" };
                            lines.push(format!(
                                "  {} Lv {}: {} - {}",
                                tag, m.level, m.name, m.description
                            ));
                        }
                    } else {
                        lines.push(format!(
                            "[Skills] Total Level: {} (Press K to open Skills UI)",
                            skills.total_level()
                        ));
                        for sk in crate::skills::SkillKind::ALL {
                            let prog = skills.get(sk);
                            let next = PlayerSkills::xp_for_next_level(prog.level);
                            lines.push(format!(
                                "  {}: Lv {}/{} ({}/{} XP) - {}",
                                sk.name(),
                                prog.level,
                                crate::skills::MAX_SKILL_LEVEL,
                                prog.xp,
                                next,
                                skills.perks_summary(sk)
                            ));
                        }
                    }
                    for line in lines {
                        direct_sends.push((
                            requester_pid,
                            ServerPacket::ChatBroadcast {
                                sender: "Server".to_string(),
                                prefix: String::new(),
                                message: line,
                                is_system: true,
                            },
                        ));
                    }
                }
            }
            SkillCommandAction::Set {
                target_pid,
                skill,
                level,
            } => {
                if let Some(c) = clients.iter_mut().find(|c| c.player_id == target_pid) {
                    c.saved_state.skills.set_level(skill, level);
                    let updated = c.saved_state.skills.clone();
                    auth_db.update_player_state(&c.username, c.model.appearance, c.saved_state.clone());
                    let _ = crate::save::save_player_data(world_dir, &c.username, &c.saved_state);
                    let _ = auth_db.save_to_dir(world_dir);
                    direct_sends.push((target_pid, ServerPacket::SkillsUpdate { skills: updated }));
                    direct_sends.push((
                        target_pid,
                        ServerPacket::ChatBroadcast {
                            sender: "Server".to_string(),
                            prefix: String::new(),
                            message: format!(
                                "[Skills] Your {} level was set to {}.",
                                skill.name(),
                                level
                            ),
                            is_system: true,
                        },
                    ));
                }
            }
            SkillCommandAction::AddXp {
                target_pid,
                skill,
                amount,
            } => {
                if let Some(c) = clients.iter_mut().find(|c| c.player_id == target_pid) {
                    let lvl_up = c.saved_state.skills.add_xp(skill, amount);
                    let updated = c.saved_state.skills.clone();
                    auth_db.update_player_state(&c.username, c.model.appearance, c.saved_state.clone());
                    let _ = crate::save::save_player_data(world_dir, &c.username, &c.saved_state);
                    let _ = auth_db.save_to_dir(world_dir);
                    direct_sends.push((target_pid, ServerPacket::SkillsUpdate { skills: updated }));
                    let mut msg = format!(
                        "[Skills] Gained {} {} XP (now Lv {}).",
                        amount,
                        skill.name(),
                        c.saved_state.skills.level(skill)
                    );
                    if let Some(up) = lvl_up {
                        if !up.unlocked_milestones.is_empty() {
                            let names: Vec<String> = up
                                .unlocked_milestones
                                .iter()
                                .map(|m| format!("Lv {} {}", m.level, m.name))
                                .collect();
                            msg.push_str(&format!(" Unlocked: {}", names.join(", ")));
                        }
                    }
                    direct_sends.push((
                        target_pid,
                        ServerPacket::ChatBroadcast {
                            sender: "Server".to_string(),
                            prefix: String::new(),
                            message: msg,
                            is_system: true,
                        },
                    ));
                }
            }
            SkillCommandAction::Reset { target_pid } => {
                if let Some(c) = clients.iter_mut().find(|c| c.player_id == target_pid) {
                    c.saved_state.skills = PlayerSkills::new();
                    let updated = c.saved_state.skills.clone();
                    auth_db.update_player_state(&c.username, c.model.appearance, c.saved_state.clone());
                    let _ = crate::save::save_player_data(world_dir, &c.username, &c.saved_state);
                    let _ = auth_db.save_to_dir(world_dir);
                    direct_sends.push((target_pid, ServerPacket::SkillsUpdate { skills: updated }));
                    direct_sends.push((
                        target_pid,
                        ServerPacket::ChatBroadcast {
                            sender: "Server".to_string(),
                            prefix: String::new(),
                            message: "[Skills] All your skills have been reset to level 1."
                                .to_string(),
                            is_system: true,
                        },
                    ));
                }
            }
        }
    }

    for (target_name, new_rank) in output.rank_updates {
        // Update permissions manager
        let _ = perms_manager.add_player_group(&target_name, &new_rank);
        let _ = perms_manager.save_to_dir(config_dir);

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
        fresh.position = *default_spawn;

        // Reset auth_db account
        auth_db.update_player_state(&target_name, CharacterAppearance::default(), fresh.clone());
        let _ = auth_db.save_to_dir(world_dir);

        // Reset online client if connected
        if let Some(c) = clients.iter_mut().find(|c| c.username.eq_ignore_ascii_case(&target_name)) {
            c.model.position = *default_spawn;
            c.model.velocity = Vec3::ZERO;
            c.model.health = 20.0;
            c.saved_state = fresh;
            direct_sends.push((
                c.player_id,
                ServerPacket::TeleportPlayer {
                    position: *default_spawn,
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

fn find_dry_spawn_from_generator(
    chunk_generator: &Arc<dyn Fn(ChunkPos) -> crate::chunk::Chunk + Send + Sync>,
) -> Vec3 {
    let max_cy = crate::chunk::VERTICAL_CHUNKS - 1;
    let is_dry_ground = |b: BlockId| {
        !b.is_air()
            && b != BlockId::WATER
            && b != BlockId::ICE
            && b != BlockId::LEAVES
            && b != BlockId::BIRCH_LEAVES
            && b != BlockId::SPRUCE_LEAVES
            && b != BlockId::ACACIA_LEAVES
            && b != BlockId::JUNGLE_LEAVES
            && b != BlockId::DARK_OAK_LEAVES
    };

    // Quick check at (0,0) first to exit instantly on empty test chunk generators
    let mut center_has_terrain = false;
    for cy in 0..=max_cy {
        let ch = chunk_generator(ChunkPos::new(0, cy, 0));
        if ch.raw_blocks().iter().any(|b| !b.is_air()) {
            center_has_terrain = true;
            break;
        }
    }
    if !center_has_terrain {
        return Vec3::new(0.5, 76.0, 0.5);
    }

    // Search outward in chunk rings (up to radius 12 chunks = ~192 blocks) for solid dry ground above water level (38)
    for r in 0_i32..=12 {
        for cz in -r..=r {
            for cx in -r..=r {
                if cx.abs() != r && cz.abs() != r {
                    continue;
                }
                let chunks: Vec<crate::chunk::Chunk> = (0..=max_cy)
                    .map(|cy| chunk_generator(ChunkPos::new(cx, cy, cz)))
                    .collect();

                for lz in [4usize, 8, 12] {
                    for lx in [4usize, 8, 12] {
                        let mut surface_y: Option<i32> = None;
                        for cy in (0..=max_cy).rev() {
                            let ch = &chunks[cy as usize];
                            for ly in (0..crate::chunk::CHUNK_SIZE_Y).rev() {
                                let b = ch.get_unchecked(lx, ly, lz);
                                if b.is_air()
                                    || matches!(
                                        b,
                                        BlockId::TALL_GRASS
                                            | BlockId::POPPY
                                            | BlockId::DANDELION
                                            | BlockId::BLUE_ORCHID
                                            | BlockId::ALLIUM
                                            | BlockId::ORANGE_TULIP
                                            | BlockId::PINK_TULIP
                                            | BlockId::OXEYE_DAISY
                                            | BlockId::LEAVES
                                            | BlockId::BIRCH_LEAVES
                                            | BlockId::SPRUCE_LEAVES
                                            | BlockId::ACACIA_LEAVES
                                            | BlockId::JUNGLE_LEAVES
                                            | BlockId::DARK_OAK_LEAVES
                                    )
                                {
                                    continue;
                                }
                                let wy = cy * crate::chunk::CHUNK_SIZE_Y as i32 + ly as i32;
                                if wy >= 40 && wy <= (max_cy * 16 + 10) && is_dry_ground(b) {
                                    surface_y = Some(wy);
                                }
                                break;
                            }
                            if surface_y.is_some() {
                                break;
                            }
                        }

                        if let Some(sy) = surface_y {
                            let wx = cx * crate::chunk::CHUNK_SIZE_X as i32 + lx as i32;
                            let wz = cz * crate::chunk::CHUNK_SIZE_Z as i32 + lz as i32;
                            return Vec3::new(wx as f32 + 0.5, (sy + 2) as f32, wz as f32 + 0.5);
                        }
                    }
                }
            }
        }
    }

    Vec3::new(0.5, 76.0, 0.5)
}

fn run_server_loop(
    listener: TcpListener,
    saves_root: PathBuf,
    config_dir: PathBuf,
    folder_name: String,
    world_name: String,
    default_seed: u32,
    initial_world_data: Option<WorldSaveData>,
    chunk_generator: Arc<dyn Fn(ChunkPos) -> crate::chunk::Chunk + Send + Sync>,
    stop_flag: Arc<AtomicBool>,
    online_count: Arc<Mutex<usize>>,
    broadcast_achievements: Arc<AtomicBool>,
    command_rx: Receiver<String>,
) {
    let world_dir = saves_root.join(&folder_name);
    let _ = fs::create_dir_all(&config_dir);
    let mut save_data = if let Some(data) = initial_world_data {
        data
    } else if let Ok(loaded) = load_world_from_dir(&saves_root, &folder_name) {
        loaded
    } else {
        let mut meta = WorldMetadata::new(world_name, default_seed);
        meta.folder_name = folder_name.clone();
        WorldSaveData::new(meta, SavedPlayerState::default())
    };

    let mut legacy_dirs: Vec<&std::path::Path> = vec![world_dir.as_path()];
    if let Some(parent) = config_dir.parent().filter(|p| !p.as_os_str().is_empty()) {
        legacy_dirs.push(parent);
    }

    let mut auth_db = AuthDatabase::load_from_dir(&world_dir).unwrap_or_default();
    let mut perms_manager = PermissionsManager::load_from_config_dir(&config_dir, &legacy_dirs)
        .unwrap_or_else(|_| PermissionsManager::new());
    let mut ban_manager =
        BanManager::load_from_config_dir(&config_dir, &legacy_dirs).unwrap_or_default();
    let mut automessage_mgr =
        crate::automessage::AutoMessageManager::load_from_dir(&config_dir).unwrap_or_default();
    let mut moderation_mgr = crate::moderation::ModerationManager::load_from_dir(&config_dir);

    let item_registry = ItemRegistry::new();
    let block_registry = crate::block::BlockRegistry::new();
    let mut world = World::new();
    save_data.apply_to_world(&mut world);
    let mut farming_sim = crate::farming::FarmingSimulation::from_save_data(&save_data);
    let mut fluid_sim = crate::fluid::FluidSimulation::new();
    let mut station_sync_accum: f32 = 0.0;
    let mut animal_sync_accum: f32 = 0.0;
    let mut village_sync_accum: f32 = 0.0;
    let mut animal_spawn_accum: f32 = 0.0;
    let mut had_animals = !save_data.animals.is_empty();
    let mut temple_monsters: Vec<crate::monster::MonsterEntity> = Vec::new();
    let mut temple_arrows: Vec<crate::monster::ArrowProjectile> = Vec::new();
    let mut had_temple_monsters = false;
    let mut next_temple_monster_id: u32 = 100_000;
    let mut next_temple_arrow_id: u32 = 100_000;
    let mut next_temple_drop_id: u64 = 900_000;

    let spawn_file = world_dir.join("spawn.txt");
    let mut default_spawn = if let Ok(s) = fs::read_to_string(&spawn_file) {
        let parts: Vec<&str> = s.trim().split(',').collect();
        if parts.len() == 3 {
            if let (Ok(x), Ok(y), Ok(z)) = (
                parts[0].parse::<f32>(),
                parts[1].parse::<f32>(),
                parts[2].parse::<f32>(),
            ) {
                Vec3::new(x, y, z)
            } else {
                find_dry_spawn_from_generator(&chunk_generator)
            }
        } else {
            find_dry_spawn_from_generator(&chunk_generator)
        }
    } else if (save_data.player.position - SavedPlayerState::default().position).length_squared() > 0.25 {
        save_data.player.position
    } else {
        find_dry_spawn_from_generator(&chunk_generator)
    };

    let mut tpa_requests: HashMap<u32, PendingTpaRequest> = HashMap::new();

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
                addr,
                framer: PacketFramer::new(),
                authenticated: false,
                player_id: pid,
                username: String::new(),
                session_token: [0u8; 16],
                model: PlayerModelState::new(pid, "", default_spawn, CharacterAppearance::default()),
                saved_state: SavedPlayerState::default(),
                is_sleeping: false,
                last_activity: Instant::now(),
                is_dead: false,
                achievements_eligible: true,
                announced_achievements: std::collections::HashSet::new(),
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
                    homes: c.saved_state.homes.clone(),
                    ip: c.addr.ip().to_string(),
                })
                .collect();
            let out = execute_command(
                &CommandSource::Console,
                &cmd_line,
                &online_infos,
                &mut perms_manager,
                &mut ban_manager,
                &item_registry,
                default_spawn,
                &mut tpa_requests,
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
                &mut automessage_mgr,
                &mut moderation_mgr,
                &mut default_spawn,
                &world_dir,
                &config_dir,
                &stop_flag,
            );
        }

        // 3. Read & process incoming packets from all connected clients
        let now = Instant::now();
        for idx in 0..clients.len() {
            if clients[idx].is_dead {
                disconnected_indices.push(idx);
                continue;
            }

            let timeout_limit = if clients[idx].authenticated {
                CLIENT_TIMEOUT
            } else {
                AUTH_TIMEOUT
            };
            if now.duration_since(clients[idx].last_activity) >= timeout_limit {
                if clients[idx].authenticated {
                    log::warn!(
                        "Disconnecting player '{}' (id {}) due to network timeout ({:?} with no packets)",
                        clients[idx].username,
                        clients[idx].player_id,
                        now.duration_since(clients[idx].last_activity)
                    );
                } else {
                    log::warn!(
                        "Disconnecting unauthenticated connection (id {}) due to timeout ({:?})",
                        clients[idx].player_id,
                        now.duration_since(clients[idx].last_activity)
                    );
                }
                clients[idx].is_dead = true;
                disconnected_indices.push(idx);
                continue;
            }

            let prev_len = clients[idx].framer.buf.len();
            let alive = {
                let c = &mut clients[idx];
                c.framer.read_available(&mut c.stream).unwrap_or(false)
            };
            if !alive {
                clients[idx].is_dead = true;
                disconnected_indices.push(idx);
                continue;
            }
            if clients[idx].framer.buf.len() > prev_len {
                clients[idx].last_activity = now;
            }

            loop {
                let frame = match clients[idx].framer.next_frame() {
                    Ok(Some(f)) => {
                        clients[idx].last_activity = Instant::now();
                        f
                    }
                    Ok(None) => break,
                    Err(_) => {
                        clients[idx].is_dead = true;
                        disconnected_indices.push(idx);
                        break;
                    }
                };
                let packet = match ClientPacket::decode(&frame) {
                    Ok(p) => p,
                    Err(_) => {
                        clients[idx].is_dead = true;
                        disconnected_indices.push(idx);
                        break;
                    }
                };

                let pid = clients[idx].player_id;
                match packet {
                    ClientPacket::AuthRequest {
                        username,
                        password_token,
                        appearance,
                    } => {
                        let canonical = username.trim().to_ascii_lowercase();
                        let already_online = clients.iter().any(|c| {
                            c.player_id != pid
                                && c.authenticated
                                && !c.is_dead
                                && c.username.to_ascii_lowercase() == canonical
                        });

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
                                    appearance: CharacterAppearance::default(),
                                },
                            ));
                            continue;
                        }

                        let client_ip = clients
                            .iter()
                            .find(|c| c.player_id == pid)
                            .map(|c| c.addr.ip().to_string())
                            .unwrap_or_default();

                        if let Some(ban) = ban_manager.is_ip_banned(&client_ip) {
                            let reason = ban.reason.clone();
                            direct_sends.push((
                                pid,
                                ServerPacket::AuthResponse {
                                    success: false,
                                    player_id: 0,
                                    session_token: [0u8; 16],
                                    message: format!("Banned IP: {}", reason),
                                    seed: 0,
                                    day_night_time: 0.25,
                                    player_state: SavedPlayerState::default(),
                                    appearance: CharacterAppearance::default(),
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
                                    appearance: CharacterAppearance::default(),
                                },
                            ));
                            continue;
                        }

                        match auth_db.authenticate_or_register(
                            &username,
                            &password_token,
                            appearance.sanitized(),
                            default_spawn,
                        ) {
                            Err(err) => {
                                direct_sends.push((
                                    pid,
                                    ServerPacket::AuthResponse {
                                        success: false,
                                        player_id: 0,
                                        session_token: [0u8; 16],
                                        message: err,
                                        seed: 0,
                                        day_night_time: 0.25,
                                        player_state: SavedPlayerState::default(),
                                        appearance: CharacterAppearance::default(),
                                    },
                                ));
                                continue;
                            }
                            Ok((mut account, token, is_new)) => {
                                let mut player_state = account.player_state.clone();

                                // If another session for this user is already connected (e.g. stale/ghost session
                                // from dropped connection or dual login), kick the older session so the reconnecting
                                // player can join immediately without waiting for server restart!
                                for (other_idx, other_c) in clients.iter_mut().enumerate() {
                                    if other_c.player_id != pid
                                        && other_c.authenticated
                                        && other_c.username.eq_ignore_ascii_case(&account.username)
                                    {
                                        log::info!(
                                            "Player '{}' reconnected (new id {}). Replacing previous session (id {}).",
                                            account.username,
                                            pid,
                                            other_c.player_id
                                        );
                                        player_state = other_c.saved_state.clone();
                                        other_c.is_dead = true;
                                        direct_sends.push((
                                            other_c.player_id,
                                            ServerPacket::Kick {
                                                reason: "Logged in from another location.".to_string(),
                                            },
                                        ));
                                        disconnected_indices.push(other_idx);
                                    }
                                }
                                account.player_state = player_state.clone();
                                if perms_manager.players.is_empty() {
                                    let _ = perms_manager.add_player_group(&account.username, "admin");
                                    let _ = perms_manager.save_to_dir(&config_dir);
                                    player_state.rank = "admin".to_string();
                                    auth_db.update_player_state(&account.username, account.appearance, player_state.clone());
                                } else {
                                    if player_state.rank.is_empty() {
                                        player_state.rank = "default".to_string();
                                    }
                                    let groups = perms_manager.get_player_groups(&account.username);
                                    if groups.is_empty() || (groups.len() == 1 && groups[0] == "default" && player_state.rank != "default") {
                                        let _ = perms_manager.add_player_group(&account.username, &player_state.rank);
                                        let _ = perms_manager.save_to_dir(&config_dir);
                                    }
                                }
                                clients[idx].authenticated = true;
                                clients[idx].username = account.username.clone();
                                clients[idx].session_token = token;
                                clients[idx].saved_state = player_state.clone();
                                clients[idx].model = PlayerModelState {

                                    player_id: pid,
                                    username: account.username.clone(),
                                    prefix: perms_manager.get_player_prefix(&account.username),
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
                                    appearance: account.appearance,
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
                                        appearance: account.appearance,
                                    },
                                ));

                                let world_eligible = save_data.metadata.achievements_enabled
                                    && save_data.metadata.game_mode
                                        == crate::player::GameMode::Survival;
                                let player_eligible = world_eligible
                                    && account.player_state.stats.game_mode
                                        == crate::player::GameMode::Survival;
                                clients[idx].achievements_eligible = player_eligible;
                                direct_sends.push((
                                    pid,
                                    ServerPacket::AchievementEligibility {
                                        enabled: player_eligible,
                                        reason: if player_eligible {
                                            None
                                        } else {
                                            Some(
                                                "Achievements are disabled on this server/mode."
                                                    .to_string(),
                                            )
                                        },
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
                                for (&pot_pos, pot) in &world.cooking_pots {
                                    direct_sends.push((
                                        pid,
                                        ServerPacket::CookingPotSync {
                                            pos: pot_pos,
                                            ingredients: pot.ingredients,
                                            fuel: pot.fuel,
                                            output: pot.output,
                                            burn_time: pot.burn_time_remaining,
                                            max_burn_time: pot.max_burn_time,
                                            cook_progress: pot.cook_progress,
                                        },
                                    ));
                                }
                                let join_animals: Vec<SyncedAnimalState> = save_data
                                    .animals
                                    .iter()
                                    .map(|a| SyncedAnimalState {
                                        id: a.id,
                                        kind: a.kind,
                                        position: a.position,
                                        velocity: a.velocity,
                                        yaw: a.yaw,
                                        health: a.health,
                                        max_health: a.max_health,
                                        hurt_timer: a.hurt_timer,
                                        love_timer: a.love_timer,
                                        breed_cooldown: a.breed_cooldown,
                                        baby_timer: a.baby_timer,
                                        is_moving: a.is_moving,
                                        walk_anim_timer: a.walk_anim_timer,
                                    })
                                    .collect();
                                direct_sends.push((
                                    pid,
                                    ServerPacket::AnimalsSync {
                                        animals: join_animals,
                                    },
                                ));
                                let join_npcs = collect_synced_village_npcs(&save_data.villages);
                                if !join_npcs.is_empty() {
                                    direct_sends.push((
                                        pid,
                                        ServerPacket::VillageNpcsSync { npcs: join_npcs },
                                    ));
                                }

                                // Broadcast join event & system chat message
                                log::info!("Player '{}' (id {}) joined the game", clients[idx].username, pid);
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
                                        prefix: String::new(),
                                        message: format!(
                                            "{} joined the game",
                                            clients[idx].username
                                        ),
                                        is_system: true,
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
                            fluid_sim.schedule_exposed_fluids_in_chunk(cpos, &world);
                        }
                        let existing_block = world.get_block(pos);
                        if save_data.temples.is_block_protected(pos, existing_block) {
                            direct_sends.push((
                                pid,
                                ServerPacket::BlockUpdate {
                                    pos,
                                    block_id: existing_block,
                                },
                            ));
                            direct_sends.push((
                                pid,
                                ServerPacket::ChatBroadcast {
                                    sender: String::new(),
                                    prefix: String::new(),
                                    message: "&cThis temple is protected until its boss is defeated!".to_string(),
                                    is_system: true,
                                },
                            ));
                        } else {
                            world.set_block(pos, block_id);
                            if block_id.is_water()
                                || block_id.is_lava()
                                || existing_block.is_water()
                                || existing_block.is_lava()
                            {
                                world.set_fluid_level(pos, crate::fluid::FLUID_SOURCE);
                            }
                            fluid_sim.schedule_block_and_neighbors(pos, &world);
                            if block_id.is_farmland() {
                                farming_sim.track_farmland(pos);
                            } else if block_id.is_farmland_crop() {
                                farming_sim.plant_crop(pos);
                            } else if block_id == BlockId::SUGAR_CANE {
                                farming_sim.track_sugar_cane(pos);
                            } else if block_id.is_cooking_pot() {
                                world.get_or_create_cooking_pot(pos);
                            }
                            if existing_block.is_cooking_pot() && !block_id.is_cooking_pot() {
                                world.remove_cooking_pot(pos);
                            }
                            if let Some(chunk) = world.get_chunk(cpos) {
                                save_data
                                    .modified_chunks
                                    .insert(cpos, encode_chunk_rle(chunk));
                                let facings = crate::save::encode_chunk_facings(chunk);
                                if facings.is_empty() {
                                    save_data.modified_chunk_facings.remove(&cpos);
                                } else {
                                    save_data.modified_chunk_facings.insert(cpos, facings);
                                }
                            }
                            outgoing_broadcasts
                                .push((Some(pid), ServerPacket::BlockUpdate { pos, block_id }));
                        }
                    }
                    ClientPacket::TempleInteract {
                        pos,
                        temple_id,
                        kind,
                        overworld_origin,
                        portal_pos,
                    } if clients[idx].authenticated => {
                        let (cpos, _, _, _) = world_to_chunk_coords(pos);
                        if world.get_chunk(cpos).is_none() {
                            world.insert_chunk(chunk_generator(cpos));
                        }
                        let block = world.get_block(pos);
                        let username = clients[idx].username.clone();
                        let is_portal_interact = block == BlockId::TEMPLE_PORTAL
                            || (temple_id != 0
                                && !crate::temple::is_in_arena_band(pos.as_vec3())
                                && (pos - portal_pos).abs().max_element() <= 3);
                        if is_portal_interact {
                            if let Some(tkind) = crate::temple::TempleKind::from_u8(kind) {
                                let return_pos = clients[idx].model.position;
                                save_data
                                    .temples
                                    .player_return_positions
                                    .insert(username.clone(), return_pos);
                                let state_snapshot = {
                                    let entry = save_data.temples.register_temple(
                                        temple_id,
                                        tkind,
                                        overworld_origin,
                                        portal_pos,
                                    );
                                    entry.clone()
                                };
                                let floor_missing =
                                    world.get_block(state_snapshot.arena_origin) == BlockId::AIR;
                                let needs_build = !state_snapshot.arena_built || floor_missing;
                                if needs_build {
                                    crate::temple::build_arena_into_world(&mut world, &state_snapshot);
                                    if state_snapshot.boss_defeated {
                                        crate::temple::populate_arena_treasure_chest(
                                            &mut world,
                                            &state_snapshot,
                                        );
                                    }
                                    let min_c = world_to_chunk_coords(state_snapshot.arena_bounds.min).0;
                                    let max_c = world_to_chunk_coords(state_snapshot.arena_bounds.max).0;
                                    for cx in min_c.0.x..=max_c.0.x {
                                        for cy in min_c.0.y..=max_c.0.y {
                                            for cz in min_c.0.z..=max_c.0.z {
                                                let acp = ChunkPos::new(cx, cy, cz);
                                                if let Some(ch) = world.get_chunk(acp) {
                                                    save_data
                                                        .modified_chunks
                                                        .insert(acp, encode_chunk_rle(ch));
                                                }
                                            }
                                        }
                                    }
                                    if let Some(t) = save_data.temples.temples.get_mut(&temple_id) {
                                        t.arena_built = true;
                                    }
                                }
                                let has_live_boss = temple_monsters.iter().any(|m| {
                                    !m.dead && m.kind.is_boss() && m.temple_id == Some(temple_id)
                                });
                                if !state_snapshot.boss_defeated && !has_live_boss {
                                    temple_monsters.retain(|m| m.temple_id != Some(temple_id));
                                    let spawned = crate::temple::spawn_arena_encounter(
                                        &state_snapshot,
                                        &mut next_temple_monster_id,
                                    );
                                    let boss_id = spawned.first().map(|m| m.id);
                                    temple_monsters.extend(spawned);
                                    if let Some(t) = save_data.temples.temples.get_mut(&temple_id) {
                                        t.active_boss_id = boss_id;
                                    }
                                }
                                let entry_pos = state_snapshot.arena_entry_spawn();
                                clients[idx].model.position = entry_pos;
                                clients[idx].saved_state.position = entry_pos;
                                direct_sends.push((
                                    pid,
                                    ServerPacket::TeleportPlayer {
                                        position: entry_pos,
                                        yaw: std::f32::consts::FRAC_PI_2,
                                        pitch: 0.0,
                                    },
                                ));
                                outgoing_broadcasts.push((
                                    None,
                                    ServerPacket::TempleProtectionSync {
                                        zones: save_data.temples.active_protected_zones(),
                                    },
                                ));
                                direct_sends.push((
                                    pid,
                                    ServerPacket::ChatBroadcast {
                                        sender: String::new(),
                                        prefix: String::new(),
                                        message: format!(
                                            "&6Entered {}! Defeat {} to unlock the temple!",
                                            tkind.name(),
                                            tkind.boss_kind().name()
                                        ),
                                        is_system: true,
                                    },
                                ));
                            }
                        } else if block == BlockId::ARENA_RETURN_PORTAL {
                            let dest = save_data
                                .temples
                                .player_return_positions
                                .get(&username)
                                .copied()
                                .or_else(|| {
                                    save_data
                                        .temples
                                        .find_temple_by_arena_pos(pos)
                                        .and_then(|tid| save_data.temples.temples.get(&tid))
                                        .map(|t| t.overworld_return_spawn())
                                })
                                .unwrap_or(default_spawn);
                            clients[idx].model.position = dest;
                            clients[idx].saved_state.position = dest;
                            direct_sends.push((
                                pid,
                                ServerPacket::TeleportPlayer {
                                    position: dest,
                                    yaw: 0.0,
                                    pitch: 0.0,
                                },
                            ));
                            direct_sends.push((
                                pid,
                                ServerPacket::ChatBroadcast {
                                    sender: String::new(),
                                    prefix: String::new(),
                                    message: "&aReturned to the overworld temple.".to_string(),
                                    is_system: true,
                                },
                            ));
                        } else if block == BlockId::BOSS_ALTAR {
                            if let Some(tid) = save_data.temples.find_temple_by_arena_pos(pos) {
                                let action = save_data.temples.temples.get(&tid).map(|t| {
                                    (t.boss_defeated, t.rechallenge_cooldown, t.clone())
                                });
                                if let Some((defeated, cd, state_snapshot)) = action {
                                    if !defeated {
                                        direct_sends.push((
                                            pid,
                                            ServerPacket::ChatBroadcast {
                                                sender: String::new(),
                                                prefix: String::new(),
                                                message: "&eThe boss is already awake in this arena!".to_string(),
                                                is_system: true,
                                            },
                                        ));
                                    } else if cd > 0.0 {
                                        direct_sends.push((
                                            pid,
                                            ServerPacket::ChatBroadcast {
                                                sender: String::new(),
                                                prefix: String::new(),
                                                message: format!(
                                                    "&eBoss Altar recharging... {:.0}s remaining.",
                                                    cd.ceil()
                                                ),
                                                is_system: true,
                                            },
                                        ));
                                    } else {
                                        // Reset arena & re-summon boss
                                        temple_monsters.retain(|m| m.temple_id != Some(tid));
                                        crate::temple::build_arena_into_world(&mut world, &state_snapshot);
                                        let spawned = crate::temple::spawn_arena_encounter(
                                            &state_snapshot,
                                            &mut next_temple_monster_id,
                                        );
                                        let boss_id = spawned.first().map(|m| m.id);
                                        temple_monsters.extend(spawned);
                                        if let Some(t) = save_data.temples.temples.get_mut(&tid) {
                                            t.boss_defeated = false;
                                            t.active_boss_id = boss_id;
                                        }
                                        outgoing_broadcasts.push((
                                            None,
                                            ServerPacket::TempleProtectionSync {
                                                zones: save_data.temples.active_protected_zones(),
                                            },
                                        ));
                                        outgoing_broadcasts.push((
                                            None,
                                            ServerPacket::ChatBroadcast {
                                                sender: String::new(),
                                                prefix: String::new(),
                                                message: format!(
                                                    "&c{} has been re-summoned at {}!",
                                                    state_snapshot.kind.boss_kind().name(),
                                                    state_snapshot.kind.name()
                                                ),
                                                is_system: true,
                                            },
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    ClientPacket::AttackTempleMonster {
                        monster_id,
                        damage,
                        knockback,
                    } if clients[idx].authenticated => {
                        let dmg = damage.clamp(0.0, 50.0);
                        if let Some(m) = temple_monsters
                            .iter_mut()
                            .find(|m| !m.dead && m.id == monster_id)
                        {
                            m.take_damage(dmg, knockback);
                            if m.dead {
                                let drop_pos = m.position + Vec3::new(0.0, 1.0, 0.0);
                                for item in m.generate_drops() {
                                    let drop_id = next_temple_drop_id;
                                    next_temple_drop_id += 1;
                                    let vel = Vec3::new(
                                        ((drop_id % 7) as f32 - 3.0) * 0.7,
                                        4.2,
                                        (((drop_id / 7) % 7) as f32 - 3.0) * 0.7,
                                    );
                                    outgoing_broadcasts.push((
                                        None,
                                        ServerPacket::ItemDropSpawned {
                                            drop_id,
                                            position: drop_pos,
                                            velocity: vel,
                                            item,
                                        },
                                    ));
                                }
                                    let is_boss = m.kind.is_boss();
                                    let boss_key = crate::achievement::boss_achievement_key(m.kind)
                                        .map(str::to_string);
                                    let kill_pos = m.position;
                                    let kill_event = crate::achievement::AchievementEvent::MonsterKilled {
                                        is_boss,
                                        boss_key,
                                    };
                                    if is_boss {
                                        for c in &clients {
                                            if c.authenticated
                                                && c.achievements_eligible
                                                && (c.player_id == pid
                                                    || c.model.position.distance(kill_pos)
                                                        <= crate::temple::BOSS_HUD_MAX_DISTANCE)
                                            {
                                                direct_sends.push((
                                                    c.player_id,
                                                    ServerPacket::AchievementEvent {
                                                        event: kill_event.clone(),
                                                    },
                                                ));
                                            }
                                        }
                                    } else if clients[idx].achievements_eligible {
                                        direct_sends.push((
                                            pid,
                                            ServerPacket::AchievementEvent { event: kill_event },
                                        ));
                                    }
                                    if is_boss {
                                    if let Some(tid) = m.temple_id {
                                        if let Some(tkind) = save_data.temples.mark_boss_defeated(tid) {
                                            if let Some(state_snapshot) =
                                                save_data.temples.temples.get(&tid).cloned()
                                            {
                                                crate::temple::populate_arena_treasure_chest(
                                                    &mut world,
                                                    &state_snapshot,
                                                );
                                                let chest_pos = state_snapshot.treasure_chest_pos();
                                                if let Some(slots) =
                                                    world.chest_containers.get(&chest_pos).cloned()
                                                {
                                                    save_data
                                                        .chest_containers
                                                        .insert(chest_pos, slots.clone());
                                                    outgoing_broadcasts.push((
                                                        None,
                                                        ServerPacket::ContainerSync {
                                                            pos: chest_pos,
                                                            is_furnace: false,
                                                            slots,
                                                            burn_time: 0.0,
                                                            max_burn_time: 0.0,
                                                            cook_progress: 0.0,
                                                        },
                                                    ));
                                                }
                                            }
                                            outgoing_broadcasts.push((
                                                None,
                                                ServerPacket::TempleProtectionSync {
                                                    zones: save_data.temples.active_protected_zones(),
                                                },
                                            ));
                                            outgoing_broadcasts.push((
                                                None,
                                                ServerPacket::ChatBroadcast {
                                                    sender: String::new(),
                                                    prefix: String::new(),
                                                    message: crate::temple::boss_victory_message(
                                                        m.kind.name(),
                                                        tkind.name(),
                                                    ),
                                                    is_system: true,
                                                },
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        temple_monsters.retain(|m| !m.dead);
                    }
                    ClientPacket::ContainerUpdate {
                        pos,
                        is_furnace,
                        slots,
                        burn_time,
                        max_burn_time,
                        cook_progress,
                    } if clients[idx].authenticated => {
                        if !is_furnace && save_data.temples.is_chest_locked(pos) {
                            continue;
                        }
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
                    ClientPacket::InteractAnimal {
                        animal_id,
                        held_item,
                    } if clients[idx].authenticated => {
                        if let Some(animal) = save_data.animals.iter_mut().find(|a| a.id == animal_id) {
                            if animal.can_milk(held_item) {
                                direct_sends.push((
                                    pid,
                                    ServerPacket::ReceiveItem {
                                        item: ItemSlot::new(ItemId::MILK_BUCKET, 1),
                                    },
                                ));
                                if clients[idx].achievements_eligible {
                                    direct_sends.push((
                                        pid,
                                        ServerPacket::AchievementEvent {
                                            event: crate::achievement::AchievementEvent::CowMilked,
                                        },
                                    ));
                                }
                            } else {
                                let _ = animal.try_feed(held_item);
                            }
                        }
                    }
                    ClientPacket::AttackAnimal {
                        animal_id,
                        damage,
                        knockback,
                    } if clients[idx].authenticated => {
                        if let Some(animal) = save_data.animals.iter_mut().find(|a| a.id == animal_id) {
                            animal.take_damage(damage.clamp(0.0, 50.0), knockback);
                        }
                    }
                    ClientPacket::CookingPotUpdate {
                        pos,
                        ingredients,
                        fuel,
                        output,
                        burn_time,
                        max_burn_time,
                        cook_progress,
                    } if clients[idx].authenticated => {
                        let pot = world.get_or_create_cooking_pot(pos);
                        pot.ingredients = ingredients;
                        pot.fuel = fuel;
                        pot.output = output;
                        pot.burn_time_remaining = burn_time;
                        pot.max_burn_time = max_burn_time;
                        pot.cook_progress = cook_progress;
                        outgoing_broadcasts.push((
                            Some(pid),
                            ServerPacket::CookingPotSync {
                                pos,
                                ingredients,
                                fuel,
                                output,
                                burn_time,
                                max_burn_time,
                                cook_progress,
                            },
                        ));
                    }
                    ClientPacket::InteractShopkeeper { npc_id } if clients[idx].authenticated => {
                        let world_ticks = save_data.villages.advance_clock(save_data.metadata.day_night_time);
                        let uname = clients[idx].username.clone();
                        if let Some(snapshot) =
                            save_data
                                .villages
                                .build_shop_snapshot(npc_id, &uname, world_ticks)
                        {
                            direct_sends.push((
                                pid,
                                ServerPacket::ShopUiSync {
                                    snapshot,
                                    status_message: String::new(),
                                },
                            ));
                        }
                    }
                    ClientPacket::ShopBuy { npc_id, offer_idx } if clients[idx].authenticated => {
                        let world_ticks = save_data.villages.advance_clock(save_data.metadata.day_night_time);
                        let uname = clients[idx].username.clone();
                        let res = save_data.villages.buy_item(
                            npc_id,
                            &uname,
                            offer_idx,
                            &mut clients[idx].saved_state.inventory,
                            &item_registry,
                            world_ticks,
                        );
                        let (status, trade_spent) = match res {
                            Ok(s) => {
                                let msg = if let Some(tier) = s.leveled_up_to {
                                    format!("Purchased! Shop Tier increased to Tier {}!", tier)
                                } else {
                                    "Purchase complete!".to_string()
                                };
                                (msg, Some(s.coins_delta.unsigned_abs() as u64))
                            }
                            Err(e) => (e.message(), None),
                        };
                        direct_sends.push((
                            pid,
                            ServerPacket::InventorySync {
                                slots: clients[idx].saved_state.inventory.slots.clone(),
                            },
                        ));
                        if let Some(snapshot) =
                            save_data
                                .villages
                                .build_shop_snapshot(npc_id, &uname, world_ticks)
                        {
                            let new_tier = snapshot.shop_tier;
                            let bought_item_id = snapshot
                                .buy_offers
                                .iter()
                                .find(|o| o.offer_idx == offer_idx)
                                .map(|o| o.item_id.0);
                            direct_sends.push((
                                pid,
                                ServerPacket::ShopUiSync {
                                    snapshot,
                                    status_message: status,
                                },
                            ));
                            if let Some(coins_spent) = trade_spent {
                                if clients[idx].achievements_eligible {
                                    direct_sends.push((
                                        pid,
                                        ServerPacket::AchievementEvent {
                                            event: crate::achievement::AchievementEvent::ShopTradeCompleted {
                                                coins_spent,
                                                new_shop_tier: new_tier,
                                            },
                                        },
                                    ));
                                    if let Some(item_id) = bought_item_id {
                                        direct_sends.push((
                                            pid,
                                            ServerPacket::AchievementEvent {
                                                event: crate::achievement::AchievementEvent::ItemObtained {
                                                    item_id,
                                                },
                                            },
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    ClientPacket::ShopSell { npc_id, offer_idx } if clients[idx].authenticated => {
                        let world_ticks = save_data.villages.advance_clock(save_data.metadata.day_night_time);
                        let uname = clients[idx].username.clone();
                        let res = save_data.villages.sell_item(
                            npc_id,
                            &uname,
                            offer_idx,
                            &mut clients[idx].saved_state.inventory,
                            &item_registry,
                            world_ticks,
                        );
                        let (status, sold_ok) = match res {
                            Ok(s) => (format!("Sold for +{} Gold Coins!", s.coins_delta), true),
                            Err(e) => (e.message(), false),
                        };
                        direct_sends.push((
                            pid,
                            ServerPacket::InventorySync {
                                slots: clients[idx].saved_state.inventory.slots.clone(),
                            },
                        ));
                        if let Some(snapshot) =
                            save_data
                                .villages
                                .build_shop_snapshot(npc_id, &uname, world_ticks)
                        {
                            let new_tier = snapshot.shop_tier;
                            direct_sends.push((
                                pid,
                                ServerPacket::ShopUiSync {
                                    snapshot,
                                    status_message: status,
                                },
                            ));
                            if sold_ok && clients[idx].achievements_eligible {
                                direct_sends.push((
                                    pid,
                                    ServerPacket::AchievementEvent {
                                        event: crate::achievement::AchievementEvent::ShopTradeCompleted {
                                            coins_spent: 0,
                                            new_shop_tier: new_tier,
                                        },
                                    },
                                ));
                            }
                        }
                    }
                    ClientPacket::ShopEnhance {
                        npc_id,
                        inv_slot_idx,
                        offer_idx,
                    } if clients[idx].authenticated => {
                        let world_ticks = save_data.villages.advance_clock(save_data.metadata.day_night_time);
                        let uname = clients[idx].username.clone();
                        let res = save_data.villages.enhance_item(
                            npc_id,
                            &uname,
                            inv_slot_idx as usize,
                            offer_idx,
                            &mut clients[idx].saved_state.inventory,
                            &item_registry,
                            world_ticks,
                        );
                        let (status, enhance_spent) = match res {
                            Ok(s) => {
                                let msg = if let Some(tier) = s.leveled_up_to {
                                    format!("Item enhanced! Shop Tier increased to Tier {}!", tier)
                                } else {
                                    "Item enhanced and restored to full durability!".to_string()
                                };
                                (msg, Some(s.coins_delta.unsigned_abs() as u64))
                            }
                            Err(e) => (e.message(), None),
                        };
                        direct_sends.push((
                            pid,
                            ServerPacket::InventorySync {
                                slots: clients[idx].saved_state.inventory.slots.clone(),
                            },
                        ));
                        if let Some(snapshot) =
                            save_data
                                .villages
                                .build_shop_snapshot(npc_id, &uname, world_ticks)
                        {
                            let new_tier = snapshot.shop_tier;
                            direct_sends.push((
                                pid,
                                ServerPacket::ShopUiSync {
                                    snapshot,
                                    status_message: status,
                                },
                            ));
                            if let Some(coins_spent) = enhance_spent {
                                if clients[idx].achievements_eligible {
                                    direct_sends.push((
                                        pid,
                                        ServerPacket::AchievementEvent {
                                            event: crate::achievement::AchievementEvent::ShopTradeCompleted {
                                                coins_spent,
                                                new_shop_tier: new_tier,
                                            },
                                        },
                                    ));
                                    direct_sends.push((
                                        pid,
                                        ServerPacket::AchievementEvent {
                                            event: crate::achievement::AchievementEvent::EnhancementApplied,
                                        },
                                    ));
                                }
                            }
                        }
                    }
                    ClientPacket::AttackVillageNpc {
                        npc_id,
                        damage,
                        knockback,
                    } if clients[idx].authenticated => {
                        let world_ticks = save_data.villages.advance_clock(save_data.metadata.day_night_time);
                        let uname = clients[idx].username.clone();
                        let hit = save_data.villages.damage_npc(
                            npc_id,
                            damage.clamp(0.0, 50.0),
                            knockback,
                            Some(&uname),
                            world_ticks,
                        );
                        if hit.is_some() && clients[idx].achievements_eligible {
                            direct_sends.push((
                                pid,
                                ServerPacket::AchievementEvent {
                                    event: crate::achievement::AchievementEvent::VillageGuardProvoked,
                                },
                            ));
                        }
                    }
                    ClientPacket::AchievementUnlocked { achievement_id }
                        if clients[idx].authenticated =>
                    {
                        if clients[idx].achievements_eligible
                            && broadcast_achievements.load(Ordering::Relaxed)
                        {
                            if let Some(def) =
                                crate::achievement::achievement_by_id(&achievement_id)
                            {
                                if clients[idx]
                                    .announced_achievements
                                    .insert(def.id.to_string())
                                {
                                    outgoing_broadcasts.push((
                                        None,
                                        ServerPacket::ChatBroadcast {
                                            sender: String::new(),
                                            prefix: String::new(),
                                            message: format!(
                                                "{} unlocked [{}]!",
                                                clients[idx].username, def.title
                                            ),
                                            is_system: true,
                                        },
                                    ));
                                }
                            }
                        }
                    }
                    ClientPacket::RegisterVillageBlueprint {
                        village_id,
                        theme,
                        origin,
                        shops,
                        guard_positions,
                    } if clients[idx].authenticated => {
                        let dist_sq = (origin.x as i64) * (origin.x as i64)
                            + (origin.z as i64) * (origin.z as i64);
                        let min_d = crate::village::VILLAGE_MIN_ORIGIN_DIST as i64;
                        let valid_origin = dist_sq >= min_d * min_d
                            && origin.x < crate::temple::ARENA_BAND_START_X
                            && (3..=5).contains(&shops.len())
                            && (2..=3).contains(&guard_positions.len())
                            && shops.iter().all(|(_, h, c)| {
                                h.distance(origin.as_vec3()) <= 28.0
                                    && c.distance(origin.as_vec3()) <= 28.0
                            })
                            && guard_positions
                                .iter()
                                .all(|gp| gp.distance(origin.as_vec3()) <= 24.0);
                        if valid_origin {
                            let world_ticks =
                                save_data.villages.advance_clock(save_data.metadata.day_night_time);
                            let bp = crate::village::VillageSpawnBlueprint {
                                village_id,
                                theme,
                                origin,
                                shops,
                                guard_positions,
                            };
                            if save_data.villages.ensure_village_spawned(&bp, world_ticks) {
                                let npcs = collect_synced_village_npcs(&save_data.villages);
                                outgoing_broadcasts
                                    .push((None, ServerPacket::VillageNpcsSync { npcs }));
                            }
                        }
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
                        let preserved_gm = clients[idx].saved_state.stats.game_mode;
                        clients[idx].saved_state.inventory = inventory;
                        clients[idx].saved_state.active_hotbar_slot = active_hotbar_slot;
                        clients[idx].saved_state.stats = stats;
                        clients[idx].saved_state.stats.game_mode = preserved_gm;
                        auth_db.update_player_state(
                            &clients[idx].username,
                            clients[idx].model.appearance,
                            clients[idx].saved_state.clone(),
                        );
                    }
                    ClientPacket::SkillsSync { skills } if clients[idx].authenticated => {
                        clients[idx].saved_state.skills = skills;
                        auth_db.update_player_state(
                            &clients[idx].username,
                            clients[idx].model.appearance,
                            clients[idx].saved_state.clone(),
                        );
                        let _ = crate::save::save_player_data(
                            &world_dir,
                            &clients[idx].username,
                            &clients[idx].saved_state,
                        );
                    }
                    ClientPacket::AttackPlayer {
                        target_player_id,
                        damage,
                    } if clients[idx].authenticated => {
                        let attacker_name = clients[idx].username.clone();
                        if let Some(target) = clients
                            .iter_mut()
                            .find(|c| c.authenticated && c.player_id == target_player_id)
                        {
                            if target.model.hurt_timer <= 0.0 && target.model.health > 0.0 {
                                target.model.health = (target.model.health - damage).max(0.0);
                                target.model.hurt_timer = 0.45;
                                target.saved_state.stats.health = target.model.health;
                                let new_hp = target.model.health;
                                log::info!(
                                    "Player '{}' attacked '{}' for {:.1} damage (target HP: {:.1})",
                                    attacker_name,
                                    target.username,
                                    damage,
                                    new_hp
                                );
                                if new_hp <= 0.0 {
                                    log::info!(
                                        "Player '{}' was slain by '{}'",
                                        target.username,
                                        attacker_name
                                    );
                                }
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
                                println!("[Command] <{}> {}", clients[idx].username, trimmed);
                                log::info!("[Command] <{}> {}", clients[idx].username, trimmed);
                                let online_infos: Vec<OnlinePlayerInfo> = clients
                                    .iter()
                                    .filter(|c| c.authenticated)
                                    .map(|c| OnlinePlayerInfo {
                                        player_id: c.player_id,
                                        username: c.username.clone(),
                                        position: c.model.position,
                                        yaw: c.model.yaw,
                                        pitch: c.model.pitch,
                                        homes: c.saved_state.homes.clone(),
                                        ip: c.addr.ip().to_string(),
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
                                    default_spawn,
                                    &mut tpa_requests,
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
                                    &mut automessage_mgr,
                                    &mut moderation_mgr,
                                    &mut default_spawn,
                                    &world_dir,
                                    &config_dir,
                                    &stop_flag,
                                );
                            } else {
                                let sender_name = clients[idx].username.clone();
                                let bypass_lock = perms_manager
                                    .has_permission(&sender_name, "blockgame.chat.lock.bypass");
                                let bypass_slowmode = perms_manager
                                    .has_permission(&sender_name, "blockgame.chat.slowmode.bypass");
                                let bypass_filter = perms_manager
                                    .has_permission(&sender_name, "blockgame.chat.filter.bypass");
                                let raw_msg: String = trimmed.chars().take(120).collect();

                                let (decision, mutes_pruned) = moderation_mgr.evaluate_chat_message(
                                    &sender_name,
                                    &raw_msg,
                                    bypass_lock,
                                    bypass_slowmode,
                                    bypass_filter,
                                );
                                if mutes_pruned {
                                    let _ = moderation_mgr.save_mutes(&config_dir);
                                }

                                match decision {
                                    crate::moderation::ChatModerationDecision::Reject(reason) => {
                                        direct_sends.push((
                                            pid,
                                            ServerPacket::ChatBroadcast {
                                                sender: "Server".to_string(),
                                                prefix: String::new(),
                                                message: format!("&c{}", reason),
                                                is_system: true,
                                            },
                                        ));
                                    }
                                    crate::moderation::ChatModerationDecision::Allow(filtered_msg) => {
                                        let prefix = perms_manager.get_player_prefix(&sender_name);
                                        let message = if perms_manager
                                            .has_permission(&sender_name, "blockgame.chat.color")
                                        {
                                            filtered_msg.clone()
                                        } else {
                                            crate::chat::escape_color_codes(&filtered_msg)
                                        };
                                        if prefix.is_empty() {
                                            println!("[Chat] <{}> {}", sender_name, filtered_msg);
                                            log::info!("[Chat] <{}> {}", sender_name, filtered_msg);
                                        } else {
                                            println!("[Chat] <{} {}> {}", prefix, sender_name, filtered_msg);
                                            log::info!("[Chat] <{} {}> {}", prefix, sender_name, filtered_msg);
                                        }
                                        outgoing_broadcasts.push((
                                            None,
                                            ServerPacket::ChatBroadcast {
                                                sender: sender_name,
                                                prefix,
                                                message,
                                                is_system: false,
                                            },
                                        ));
                                    }
                                }
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
                                    prefix: String::new(),
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
                                            prefix: String::new(),
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
                    ClientPacket::VoiceFrame { seq, data } if clients[idx].authenticated => {
                        if !data.is_empty() && data.len() <= 2048 {
                            let sender_pos = clients[idx].model.position;
                            for other in clients.iter() {
                                if other.authenticated
                                    && other.player_id != pid
                                    && other.model.position.distance(sender_pos) <= MAX_VOICE_RANGE
                                {
                                    direct_sends.push((
                                        other.player_id,
                                        ServerPacket::VoiceBroadcast {
                                            sender_id: pid,
                                            seq,
                                            position: sender_pos,
                                            data: data.clone(),
                                        },
                                    ));
                                }
                            }
                        }
                    }
                    ClientPacket::Disconnect => {
                        clients[idx].is_dead = true;
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
                        prefix: String::new(),
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
                    if let Err(e) = c.stream.write_all(&bytes) {
                        if e.kind() != io::ErrorKind::WouldBlock {
                            log::warn!("Write error to client {} ({}): {}", c.player_id, c.username, e);
                            c.is_dead = true;
                        }
                    }
                    let _ = c.stream.flush();
                }
            }
        }

        // Remove disconnected clients (in reverse order to preserve indices)
        for (i, c) in clients.iter().enumerate() {
            if c.is_dead {
                disconnected_indices.push(i);
            }
        }
        disconnected_indices.sort_unstable();
        disconnected_indices.dedup();
        for &idx in disconnected_indices.iter().rev() {
            let removed = clients.remove(idx);
            tpa_requests.retain(|target_id, req| *target_id != removed.player_id && req.requester_id != removed.player_id);
            if removed.authenticated {
                log::info!("Player '{}' (id {}) disconnected", removed.username, removed.player_id);
                auth_db.update_player_state(
                    &removed.username,
                    removed.model.appearance,
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
                        prefix: String::new(),
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
                        prefix: String::new(),
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
                    if c.authenticated && Some(c.player_id) != exclude_pid && !c.is_dead {
                        if let Err(e) = c.stream.write_all(&bytes) {
                            if e.kind() != io::ErrorKind::WouldBlock {
                                log::warn!("Broadcast write error to client {} ({}): {}", c.player_id, c.username, e);
                                c.is_dead = true;
                            }
                        }
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
                .filter(|c| c.authenticated && !c.is_dead)
                .map(|c| c.model.clone())
                .collect();

            *online_count.lock().unwrap() = all_players.len();

            // Tick scheduled Auto-Messages and broadcast when due
            if let Some(auto_msg) = automessage_mgr.tick(dt, !all_players.is_empty()) {
                let pkt = ServerPacket::ChatBroadcast {
                    sender: "Server".to_string(),
                    prefix: String::new(),
                    message: auto_msg,
                    is_system: true,
                };
                if let Ok(bytes) = pkt.encode() {
                    for c in clients.iter_mut() {
                        if c.authenticated && !c.is_dead {
                            if let Err(e) = c.stream.write_all(&bytes) {
                                if e.kind() != io::ErrorKind::WouldBlock {
                                    c.is_dead = true;
                                }
                            }
                        }
                    }
                }
            }

            // Tick temple re-challenge cooldowns and active server-authoritative boss encounters
            save_data.temples.tick_cooldowns(dt);
            let mut server_npc_damage: Vec<(u32, f32, Vec3)> = Vec::new();
            if (!temple_monsters.is_empty() || had_temple_monsters) && !all_players.is_empty() {
                had_temple_monsters = !temple_monsters.is_empty();
                let mut newly_summoned = Vec::new();
                let mut tick_damage_events: Vec<(u32, f32)> = Vec::new();
                let mut tick_announcements: Vec<String> = Vec::new();
                let server_npc_targets = save_data.villages.living_npc_targets();

                for m in temple_monsters.iter_mut() {
                    if m.dead {
                        continue;
                    }
                    // Target the nearest alive authenticated player
                    let nearest = all_players
                        .iter()
                        .min_by(|a, b| {
                            let da = a.position.distance_squared(m.position);
                            let db = b.position.distance_squared(m.position);
                            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    let Some(target) = nearest else { continue; };
                    let outcome = m.update_with_npc_targets(
                        dt,
                        target.position,
                        &server_npc_targets,
                        0.0,
                        &world,
                        &block_registry,
                    );

                    if outcome.damage_to_player > 0.0 {
                        tick_damage_events.push((target.player_id, outcome.damage_to_player));
                    }
                    if let Some(ev) = outcome.damage_to_npc {
                        server_npc_damage.push(ev);
                    }
                    if let Some((origin, vel)) = outcome.spawned_arrow {
                        let aid = next_temple_arrow_id;
                        next_temple_arrow_id = next_temple_arrow_id.wrapping_add(1);
                        temple_arrows.push(crate::monster::ArrowProjectile::new(
                            aid,
                            origin,
                            vel,
                            false,
                            m.kind.attack_damage(),
                        ));
                    }
                    for (origin, vel, dmg) in outcome.extra_arrows {
                        let aid = next_temple_arrow_id;
                        next_temple_arrow_id = next_temple_arrow_id.wrapping_add(1);
                        temple_arrows.push(crate::monster::ArrowProjectile::new(
                            aid, origin, vel, false, dmg,
                        ));
                    }
                    if let Some((center, radius, aoe_dmg)) = outcome.aoe_blast {
                        for p in &all_players {
                            if p.position.distance(center) <= radius {
                                tick_damage_events.push((p.player_id, aoe_dmg));
                            }
                        }
                    }
                    if let Some(new_phase) = outcome.phase_changed {
                        tick_announcements.push(crate::temple::phase_barrier_message(
                            m.kind.name(),
                            new_phase,
                        ));
                    }
                    if outcome.barrier_broken {
                        tick_announcements.push(crate::temple::barrier_shatter_message(
                            m.kind.name(),
                        ));
                    }
                    for (mkind, spos) in outcome.summon_minions {
                        if let Some(tid) = m.temple_id {
                            newly_summoned.push((mkind, spos, tid));
                        }
                    }
                }

                // Cap active summoned minions per temple to avoid unbounded growth
                for (mkind, spos, tid) in newly_summoned {
                    let count_in_temple = temple_monsters
                        .iter()
                        .filter(|m| !m.dead && m.temple_id == Some(tid) && !m.kind.is_boss())
                        .count();
                    if count_in_temple < crate::temple::MAX_MINIONS_PER_TEMPLE {
                        let mid = next_temple_monster_id;
                        next_temple_monster_id = next_temple_monster_id.wrapping_add(1);
                        temple_monsters.push(crate::monster::MonsterEntity::new_temple_mob(
                            mid, mkind, spos, tid,
                        ));
                    }
                }

                // Update server-side boss projectiles
                for arrow in temple_arrows.iter_mut() {
                    arrow.update(dt, &world, &block_registry);
                    if !arrow.dead && !arrow.stuck {
                        for p in &all_players {
                            let center = p.position + Vec3::new(0.0, 0.9, 0.0);
                            if (arrow.position - center).length() < 0.9 {
                                tick_damage_events.push((p.player_id, arrow.damage));
                                arrow.dead = true;
                                break;
                            }
                        }
                    }
                }
                temple_arrows.retain(|a| !a.dead);
                temple_monsters.retain(|m| !m.dead);

                // Apply damage events to players
                for (target_pid, dmg) in tick_damage_events {
                    if let Some(c) = clients
                        .iter_mut()
                        .find(|c| c.authenticated && !c.is_dead && c.player_id == target_pid)
                    {
                        if c.model.hurt_timer <= 0.0 && c.model.health > 0.0 {
                            c.model.health = (c.model.health - dmg).max(0.0);
                            c.model.hurt_timer = 0.45;
                            c.saved_state.stats.health = c.model.health;
                            let pkt = ServerPacket::PlayerDamaged {
                                target_player_id: target_pid,
                                attacker_id: 0,
                                damage: dmg,
                                new_health: c.model.health,
                            };
                            if let Ok(bytes) = pkt.encode() {
                                let _ = c.stream.write_all(&bytes);
                            }
                        }
                    }
                }

                // Broadcast phase announcements
                for msg in tick_announcements {
                    let pkt = ServerPacket::ChatBroadcast {
                        sender: String::new(),
                        prefix: String::new(),
                        message: msg,
                        is_system: true,
                    };
                    if let Ok(bytes) = pkt.encode() {
                        for c in clients.iter_mut() {
                            if c.authenticated && !c.is_dead {
                                let _ = c.stream.write_all(&bytes);
                            }
                        }
                    }
                }

                // Broadcast synced temple monsters, boss projectiles, and per-player 500-block Boss HUD updates
                let synced: Vec<SyncedMonsterState> = temple_monsters
                    .iter()
                    .map(|m| SyncedMonsterState {
                        id: m.id,
                        kind: m.kind,
                        position: m.position,
                        yaw: m.yaw,
                        health: m.health,
                        max_health: m.max_health,
                        hurt_timer: m.hurt_timer,
                        walk_anim_timer: m.walk_anim_timer,
                        phase: m.phase,
                        in_combat: m.in_combat,
                        invulnerable: m.invuln_timer > 0.0,
                    })
                    .collect();
                let synced_arrows: Vec<SyncedArrowState> = temple_arrows
                    .iter()
                    .map(|a| SyncedArrowState {
                        id: a.id,
                        position: a.position,
                        velocity: a.velocity,
                        stuck: a.stuck,
                    })
                    .collect();
                let arrow_bytes = (ServerPacket::TempleArrowsSync {
                    arrows: synced_arrows,
                })
                .encode()
                .ok();
                if let Ok(mbytes) = (ServerPacket::TempleMonstersSync { monsters: synced }).encode() {
                    for c in clients.iter_mut() {
                        if c.authenticated && !c.is_dead {
                            let _ = c.stream.write_all(&mbytes);
                            if let Some(ref abytes) = arrow_bytes {
                                let _ = c.stream.write_all(abytes);
                            }
                            let hud = crate::temple::compute_boss_hud(
                                &temple_monsters,
                                c.model.position,
                            );
                            if let Ok(hbytes) = (ServerPacket::BossHudUpdate { hud }).encode() {
                                let _ = c.stream.write_all(&hbytes);
                            }
                        }
                    }
                }
            }

            // Server-authoritative Farming, Animals, Furnaces, and Cooking Pots (ADR-0004)
            let active_player_positions: Vec<Vec3> = clients
                .iter()
                .filter(|c| c.authenticated && !c.is_dead)
                .map(|c| c.model.position)
                .collect();
            let active_player_lures: Vec<(Vec3, ItemId)> = clients
                .iter()
                .filter(|c| c.authenticated && !c.is_dead)
                .map(|c| {
                    let slot_idx = c.saved_state.active_hotbar_slot.min(8);
                    let held = c.saved_state.inventory.slots[slot_idx].item_id;
                    (c.model.position, held)
                })
                .collect();

            if !active_player_positions.is_empty() {
                let broadcast_pkt = |clients: &mut [ConnectedClient], pkt: &ServerPacket| {
                    if let Ok(bytes) = pkt.encode() {
                        for c in clients.iter_mut() {
                            if c.authenticated && !c.is_dead {
                                let _ = c.stream.write_all(&bytes);
                            }
                        }
                    }
                };

                let day_frac = (save_data.metadata.day_night_time / 24000.0).rem_euclid(1.0);
                let sun_angle = day_frac * std::f32::consts::TAU;
                let sun_intensity = (sun_angle.sin() * 2.0 + 0.5).clamp(0.0, 1.0);

                // A. Crops, Farmland, Fertilizers, Stem Crops, Sugar Cane
                let farm_events = farming_sim.tick(
                    dt,
                    sun_intensity,
                    &active_player_positions,
                    &mut world,
                    &block_registry,
                );
                for ev in farm_events {
                    match ev {
                        crate::farming::FarmingEvent::BlockChanged { pos, block } => {
                            let (cpos, _, _, _) = world_to_chunk_coords(pos);
                            if let Some(chunk) = world.get_chunk(cpos) {
                                save_data
                                    .modified_chunks
                                    .insert(cpos, encode_chunk_rle(chunk));
                            }
                            broadcast_pkt(
                                &mut clients,
                                &ServerPacket::BlockUpdate {
                                    pos,
                                    block_id: block,
                                },
                            );
                        }
                        crate::farming::FarmingEvent::SpawnDrop { pos, item } => {
                            let drop_id = next_local_drop_id();
                            let drop_pos = pos.as_vec3() + Vec3::new(0.5, 0.35, 0.5);
                            let mut drop = ItemDrop::new(drop_pos, item);
                            drop.drop_id = drop_id;
                            save_data.item_drops.push(drop);
                            broadcast_pkt(
                                &mut clients,
                                &ServerPacket::ItemDropSpawned {
                                    drop_id,
                                    position: drop_pos,
                                    velocity: Vec3::new(0.0, 2.0, 0.0),
                                    item,
                                },
                            );
                        }
                    }
                }

                // A2. Authoritative cellular Water & Lava fluid simulation (ADR 0004 / ADR 0010)
                if fluid_sim.has_pending() {
                    let (fluid_modified, fluid_drops) =
                        fluid_sim.tick(dt, &mut world, &block_registry);
                    for pos in fluid_modified {
                        let block = world.get_block(pos);
                        let level = world.get_fluid_level(pos);
                        let (cpos, _, _, _) = world_to_chunk_coords(pos);
                        if let Some(chunk) = world.get_chunk(cpos) {
                            save_data
                                .modified_chunks
                                .insert(cpos, encode_chunk_rle(chunk));
                            let facings = crate::save::encode_chunk_facings(chunk);
                            if facings.is_empty() {
                                save_data.modified_chunk_facings.remove(&cpos);
                            } else {
                                save_data.modified_chunk_facings.insert(cpos, facings);
                            }
                        }
                        broadcast_pkt(
                            &mut clients,
                            &ServerPacket::FluidUpdate {
                                pos,
                                block_id: block,
                                level,
                            },
                        );
                    }
                    for (pos, item) in fluid_drops {
                        if item.is_empty() {
                            continue;
                        }
                        let drop_id = next_local_drop_id();
                        let drop_pos = pos.as_vec3() + Vec3::new(0.5, 0.35, 0.5);
                        let mut drop = ItemDrop::new(drop_pos, item);
                        drop.drop_id = drop_id;
                        save_data.item_drops.push(drop);
                        broadcast_pkt(
                            &mut clients,
                            &ServerPacket::ItemDropSpawned {
                                drop_id,
                                position: drop_pos,
                                velocity: Vec3::new(0.0, 2.0, 0.0),
                                item,
                            },
                        );
                    }
                }

                // B0. Server-authoritative passive animal spawning around connected players
                animal_spawn_accum += dt;
                if animal_spawn_accum >= 2.0 && save_data.animals.len() < 35 {
                    animal_spawn_accum = 0.0;
                    'spawn_outer: for &ppos in &active_player_positions {
                        let pcx = (ppos.x.floor() as i32).div_euclid(16);
                        let pcz = (ppos.z.floor() as i32).div_euclid(16);
                        for dcz in -2..=2 {
                            for dcx in -2..=2 {
                                let col = glam::IVec2::new(pcx + dcx, pcz + dcz);
                                if save_data.animal_spawned_columns.contains(&col) {
                                    continue;
                                }
                                save_data.animal_spawned_columns.insert(col);
                                // Ensure vertical surface chunks exist in server world
                                for cy in 0..crate::chunk::VERTICAL_CHUNKS as i32 {
                                    let cpos = ChunkPos::new(col.x, cy, col.y);
                                    if world.get_chunk(cpos).is_none() {
                                        world.insert_chunk(chunk_generator(cpos));
                                        fluid_sim.schedule_exposed_fluids_in_chunk(cpos, &world);
                                    }
                                }
                                let col_hash = ((col.x as i64).wrapping_mul(73856093)
                                    ^ (col.y as i64).wrapping_mul(19349663)
                                    ^ (save_data.metadata.seed as i64))
                                    as u64;
                                if col_hash % 5 != 0 {
                                    continue;
                                }
                                let kind = match (col_hash >> 4) % 4 {
                                    0 => crate::animal::AnimalKind::Cow,
                                    1 => crate::animal::AnimalKind::Pig,
                                    2 => crate::animal::AnimalKind::Chicken,
                                    _ => crate::animal::AnimalKind::Sheep,
                                };
                                let count = (1 + ((col_hash >> 8) % 3) as usize)
                                    .min(35_usize.saturating_sub(save_data.animals.len()));
                                for n in 0..count {
                                    let lx = (((col_hash >> (12 + n * 4)) & 0xF) as i32).clamp(1, 14);
                                    let lz = (((col_hash >> (16 + n * 4)) & 0xF) as i32).clamp(1, 14);
                                    let wx = col.x * 16 + lx;
                                    let wz = col.y * 16 + lz;
                                    let max_y = (crate::chunk::VERTICAL_CHUNKS * 16 - 3) as i32;
                                    for wy in (40..=max_y).rev() {
                                        let ground = world.get_block(IVec3::new(wx, wy, wz));
                                        if matches!(ground, BlockId::GRASS | BlockId::DIRT)
                                            && world.get_block(IVec3::new(wx, wy + 1, wz)).is_air()
                                            && world.get_block(IVec3::new(wx, wy + 2, wz)).is_air()
                                        {
                                            let aid = save_data.next_animal_id;
                                            save_data.next_animal_id =
                                                save_data.next_animal_id.wrapping_add(1);
                                            save_data.animals.push(
                                                crate::animal::AnimalEntity::new(
                                                    aid,
                                                    kind,
                                                    Vec3::new(
                                                        wx as f32 + 0.5,
                                                        (wy + 1) as f32,
                                                        wz as f32 + 0.5,
                                                    ),
                                                ),
                                            );
                                            break;
                                        }
                                    }
                                    if save_data.animals.len() >= 35 {
                                        break 'spawn_outer;
                                    }
                                }
                            }
                        }
                    }
                }

                // B. Passive Animals (herding, breeding, egg laying, physics, death drops)
                let prev_animal_count = save_data.animals.len();
                let animal_drops = crate::animal::tick_animals(
                    &mut save_data.animals,
                    &mut save_data.next_animal_id,
                    dt,
                    &active_player_lures,
                    &world,
                    &block_registry,
                );
                if save_data.animals.len() > prev_animal_count {
                    for newborn in &save_data.animals[prev_animal_count..] {
                        if newborn.baby_timer > 0.0 {
                            let nearest_idx = clients
                                .iter()
                                .enumerate()
                                .filter(|(_, c)| c.authenticated && c.achievements_eligible)
                                .min_by(|(_, a), (_, b)| {
                                    let da = a.model.position.distance_squared(newborn.position);
                                    let db = b.model.position.distance_squared(newborn.position);
                                    da.total_cmp(&db)
                                })
                                .and_then(|(i, c)| {
                                    if c.model.position.distance(newborn.position) <= 24.0 {
                                        Some(i)
                                    } else {
                                        None
                                    }
                                });
                            if let Some(ci) = nearest_idx {
                                if let Ok(bytes) = (ServerPacket::AchievementEvent {
                                    event: crate::achievement::AchievementEvent::AnimalBred,
                                })
                                .encode()
                                {
                                    let _ = clients[ci].stream.write_all(&bytes);
                                }
                            }
                        }
                    }
                }
                for (drop_pos, item) in animal_drops {
                    let drop_id = next_local_drop_id();
                    let mut drop = ItemDrop::new(drop_pos, item);
                    drop.drop_id = drop_id;
                    save_data.item_drops.push(drop);
                    broadcast_pkt(
                        &mut clients,
                        &ServerPacket::ItemDropSpawned {
                            drop_id,
                            position: drop_pos,
                            velocity: Vec3::new(0.0, 2.5, 0.0),
                            item,
                        },
                    );
                }

                animal_sync_accum += dt;
                if animal_sync_accum >= 0.1 && (!save_data.animals.is_empty() || had_animals) {
                    animal_sync_accum = 0.0;
                    had_animals = !save_data.animals.is_empty();
                    let synced_animals: Vec<SyncedAnimalState> = save_data
                        .animals
                        .iter()
                        .map(|a| SyncedAnimalState {
                            id: a.id,
                            kind: a.kind,
                            position: a.position,
                            velocity: a.velocity,
                            yaw: a.yaw,
                            health: a.health,
                            max_health: a.max_health,
                            hurt_timer: a.hurt_timer,
                            love_timer: a.love_timer,
                            breed_cooldown: a.breed_cooldown,
                            baby_timer: a.baby_timer,
                            is_moving: a.is_moving,
                            walk_anim_timer: a.walk_anim_timer,
                        })
                        .collect();
                    broadcast_pkt(
                        &mut clients,
                        &ServerPacket::AnimalsSync {
                            animals: synced_animals,
                        },
                    );
                }

                // B2. Authoritative NPC Villages (Shopkeepers & Village Guards)
                {
                    let world_ticks =
                        save_data.villages.advance_clock(save_data.metadata.day_night_time);
                    for (npc_id, dmg, kb) in server_npc_damage {
                        let _ = save_data
                            .villages
                            .damage_npc(npc_id, dmg, kb, None, world_ticks);
                    }
                    let mut village_players: Vec<(String, Vec3)> = Vec::new();
                    for c in &clients {
                        if c.authenticated && !c.is_dead && c.model.health > 0.0 {
                            village_players.push((c.username.clone(), c.model.position));
                        } else if c.authenticated && (c.is_dead || c.model.health <= 0.0) {
                            save_data
                                .villages
                                .clear_guard_hostility_on_player_death(&c.username);
                        }
                    }
                    let v_out = save_data.villages.tick(
                        dt,
                        world_ticks,
                        &village_players,
                        &mut temple_monsters,
                        &world,
                        &block_registry,
                    );
                    for (target_user, dmg, _kb) in v_out.guard_damage_to_players {
                        if let Some(c) = clients.iter_mut().find(|c| {
                            c.authenticated
                                && !c.is_dead
                                && c.username.eq_ignore_ascii_case(&target_user)
                        }) {
                            if c.model.hurt_timer <= 0.0 && c.model.health > 0.0 {
                                c.model.health = (c.model.health - dmg).max(0.0);
                                c.model.hurt_timer = 0.45;
                                c.saved_state.stats.health = c.model.health;
                                let pkt = ServerPacket::PlayerDamaged {
                                    target_player_id: c.player_id,
                                    attacker_id: 0,
                                    damage: dmg,
                                    new_health: c.model.health,
                                };
                                if let Ok(bytes) = pkt.encode() {
                                    let _ = c.stream.write_all(&bytes);
                                }
                            }
                        }
                    }
                    village_sync_accum += dt;
                    if village_sync_accum >= 0.15
                        && (!save_data.villages.shopkeepers.is_empty()
                            || !save_data.villages.guards.is_empty())
                    {
                        village_sync_accum = 0.0;
                        let synced_npcs = collect_synced_village_npcs(&save_data.villages);
                        broadcast_pkt(
                            &mut clients,
                            &ServerPacket::VillageNpcsSync { npcs: synced_npcs },
                        );
                    }
                }

                // C. Authoritative Furnaces & Cooking Pots
                station_sync_accum += dt;
                let should_sync_stations = station_sync_accum >= 0.25;
                if should_sync_stations {
                    station_sync_accum = 0.0;
                }

                let furnace_positions: Vec<IVec3> = world.furnaces.keys().copied().collect();
                for fpos in furnace_positions {
                    if let Some(f) = world.furnaces.get_mut(&fpos) {
                        let was_active = f.is_burning() || f.cook_progress > 0.0;
                        let lit_changed = f.update(dt);
                        let is_active = f.is_burning() || f.cook_progress > 0.0;
                        let snapshot = (
                            [f.input, f.fuel, f.output],
                            f.burn_time_remaining,
                            f.max_burn_time,
                            f.cook_progress,
                            f.is_burning(),
                        );
                        if lit_changed {
                            let new_blk = if snapshot.4 {
                                BlockId::FURNACE_LIT
                            } else {
                                BlockId::FURNACE
                            };
                            world.set_block(fpos, new_blk);
                            broadcast_pkt(
                                &mut clients,
                                &ServerPacket::BlockUpdate {
                                    pos: fpos,
                                    block_id: new_blk,
                                },
                            );
                        }
                        if (should_sync_stations && (was_active || is_active)) || lit_changed {
                            broadcast_pkt(
                                &mut clients,
                                &ServerPacket::ContainerSync {
                                    pos: fpos,
                                    is_furnace: true,
                                    slots: snapshot.0.to_vec(),
                                    burn_time: snapshot.1,
                                    max_burn_time: snapshot.2,
                                    cook_progress: snapshot.3,
                                },
                            );
                        }
                    }
                }

                let pot_positions: Vec<IVec3> = world.cooking_pots.keys().copied().collect();
                for ppos in pot_positions {
                    if let Some(pot) = world.cooking_pots.get_mut(&ppos) {
                        let was_active = pot.is_burning() || pot.cook_progress > 0.0;
                        let lit_changed = pot.update(dt);
                        let is_active = pot.is_burning() || pot.cook_progress > 0.0;
                        let snapshot = (
                            pot.ingredients,
                            pot.fuel,
                            pot.output,
                            pot.burn_time_remaining,
                            pot.max_burn_time,
                            pot.cook_progress,
                            pot.is_burning(),
                        );
                        if lit_changed {
                            let new_blk = if snapshot.6 {
                                BlockId::COOKING_POT_LIT
                            } else {
                                BlockId::COOKING_POT
                            };
                            world.set_block(ppos, new_blk);
                            broadcast_pkt(
                                &mut clients,
                                &ServerPacket::BlockUpdate {
                                    pos: ppos,
                                    block_id: new_blk,
                                },
                            );
                        }
                        if (should_sync_stations && (was_active || is_active)) || lit_changed {
                            broadcast_pkt(
                                &mut clients,
                                &ServerPacket::CookingPotSync {
                                    pos: ppos,
                                    ingredients: snapshot.0,
                                    fuel: snapshot.1,
                                    output: snapshot.2,
                                    burn_time: snapshot.3,
                                    max_burn_time: snapshot.4,
                                    cook_progress: snapshot.5,
                                },
                            );
                        }
                    }
                }
            }

            if !all_players.is_empty() {
                let snap = ServerPacket::WorldSnapshot {
                    day_night_time: save_data.metadata.day_night_time,
                    players: all_players,
                };
                if let Ok(bytes) = snap.encode() {
                    for c in clients.iter_mut() {
                        if c.authenticated && !c.is_dead {
                            if let Err(e) = c.stream.write_all(&bytes) {
                                if e.kind() != io::ErrorKind::WouldBlock {
                                    log::warn!("Snapshot write error to client {} ({}): {}", c.player_id, c.username, e);
                                    c.is_dead = true;
                                }
                            }
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
                        c.model.appearance,
                        c.saved_state.clone(),
                    );
                }
            }
            let _ = auth_db.save_to_dir(&world_dir);
            farming_sim.export_to_save_data(&mut save_data);
            save_data.capture_from_world(&world);
            let _ = save_world_to_dir(&saves_root, &mut save_data);
        }

        thread::sleep(Duration::from_millis(5));
    }

    // Final save on clean server shutdown
    for c in &clients {
        if c.authenticated {
            auth_db.update_player_state(&c.username, c.model.appearance, c.saved_state.clone());
        }
    }
    let _ = auth_db.save_to_dir(&world_dir);
    let _ = perms_manager.save_to_dir(&config_dir);
    let _ = ban_manager.save_to_dir(&config_dir);
    farming_sim.export_to_save_data(&mut save_data);
    save_data.capture_from_world(&world);
    let _ = save_world_to_dir(&saves_root, &mut save_data);

}

// --- Non-Blocking Multiplayer Client ---

/// Chat log entry displayed in the client HUD / chat overlay.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatEntry {
    pub sender: String,
    pub prefix: String,
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
    pub local_prefix: String,
    pub appearance: CharacterAppearance,
    pub session_token: [u8; 16],
    pub remote_players: HashMap<u32, PlayerModelState>,
    pub chat_history: Vec<ChatEntry>,
    pub connected: bool,
    pub last_packet_time: Instant,
}

impl MultiplayerClient {
    /// Connects to a `MultiplayerServer` at `addr_str`, performs the authentication handshake,
    /// and returns `(MultiplayerClient, AuthResponse)`.
    pub fn connect_and_authenticate(
        addr_str: &str,
        username: &str,
        password: &str,
        appearance: CharacterAppearance,
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
            appearance: appearance.sanitized(),
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
            appearance: assigned_appearance,
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
                local_prefix: String::new(),
                appearance: assigned_appearance,
                session_token,
                remote_players: HashMap::new(),
                chat_history: vec![ChatEntry {
                    sender: "Server".to_string(),
                    prefix: String::new(),
                    message: message.clone(),
                    is_system: true,
                    age: 0.0,
                }],
                connected: true,
                last_packet_time: Instant::now(),
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
            if let Err(e) = self.stream.write_all(&bytes) {
                if e.kind() != io::ErrorKind::WouldBlock {
                    self.connected = false;
                }
            }
        }
    }

    /// Polls all newly arrived `ServerPacket`s without blocking.
    pub fn poll_packets(&mut self) -> Vec<ServerPacket> {
        let mut received = Vec::new();
        if self.connected {
            let prev_len = self.framer.buf.len();
            match self.framer.read_available(&mut self.stream) {
                Ok(true) => {
                    if self.framer.buf.len() > prev_len {
                        self.last_packet_time = Instant::now();
                    }
                }
                Ok(false) | Err(_) => {
                    self.connected = false;
                }
            }
        }

        while let Ok(Some(frame)) = self.framer.next_frame() {
            if let Ok(pkt) = ServerPacket::decode(&frame) {
                self.last_packet_time = Instant::now();
                received.push(pkt);
            }
        }

        if self.connected && self.last_packet_time.elapsed() >= CLIENT_TIMEOUT {
            log::warn!(
                "Multiplayer connection timed out (no packets received for {:?})",
                self.last_packet_time.elapsed()
            );
            self.connected = false;
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

fn collect_synced_village_npcs(
    villages: &crate::village::VillageManager,
) -> Vec<SyncedVillageNpcState> {
    let mut out = Vec::with_capacity(villages.shopkeepers.len() + villages.guards.len());
    for sk in &villages.shopkeepers {
        if sk.dead {
            continue;
        }
        out.push(SyncedVillageNpcState {
            id: sk.id,
            village_id: sk.village_id,
            is_guard: false,
            profession: Some(sk.profession),
            name: sk.name.clone(),
            appearance: sk.appearance,
            position: sk.position,
            yaw: sk.yaw,
            walk_anim: sk.walk_anim,
            swing_anim: 0.0,
            hurt_timer: sk.hurt_timer,
            health: sk.health,
            max_health: sk.max_health,
            held_item: sk.profession.held_item(),
        });
    }
    for g in &villages.guards {
        if g.dead {
            continue;
        }
        out.push(SyncedVillageNpcState {
            id: g.id,
            village_id: g.village_id,
            is_guard: true,
            profession: None,
            name: g.name.clone(),
            appearance: g.appearance,
            position: g.position,
            yaw: g.yaw,
            walk_anim: g.walk_anim,
            swing_anim: g.swing_anim,
            hurt_timer: g.hurt_timer,
            health: g.health,
            max_health: g.max_health,
            held_item: ItemId::IRON_SWORD,
        });
    }
    out
}

fn write_slot<W: Write>(w: &mut W, slot: ItemSlot) -> io::Result<()> {
    write_u16(w, slot.item_id.0)?;
    write_u16(w, slot.count)?;
    write_u16(w, slot.durability)?;
    write_u8(w, slot.enhancement.map_or(0, |e| e.to_u8()))
}

fn read_slot<R: Read>(r: &mut R) -> io::Result<ItemSlot> {
    let id = ItemId(read_u16(r)?);
    let count = read_u16(r)?;
    let durability = read_u16(r)?;
    let enh = crate::item::Enhancement::from_u8(read_u8(r)?);
    if id == ItemId::EMPTY || count == 0 {
        Ok(ItemSlot::EMPTY)
    } else {
        Ok(ItemSlot::with_enhancement(id, count, durability, enh))
    }
}

fn write_player_model<W: Write>(w: &mut W, p: &PlayerModelState) -> io::Result<()> {
    write_u32(w, p.player_id)?;
    write_string(w, &p.username)?;
    write_string(w, &p.prefix)?;
    write_vec3(w, p.position)?;
    write_vec3(w, p.velocity)?;
    write_f32(w, p.yaw)?;
    write_f32(w, p.pitch)?;
    write_u16(w, p.held_item.0)?;
    write_f32(w, p.walk_anim)?;
    write_f32(w, p.swing_anim)?;
    write_f32(w, p.hurt_timer)?;
    write_bool(w, p.is_sneaking)?;
    p.appearance.write_binary(w)?;
    write_f32(w, p.health)?;
    write_f32(w, p.max_health)
}

fn read_player_model<R: Read>(r: &mut R) -> io::Result<PlayerModelState> {
    Ok(PlayerModelState {
        player_id: read_u32(r)?,
        username: read_string(r)?,
        prefix: read_string(r)?,
        position: read_vec3(r)?,
        velocity: read_vec3(r)?,
        yaw: read_f32(r)?,
        pitch: read_f32(r)?,
        held_item: ItemId(read_u16(r)?),
        walk_anim: read_f32(r)?,
        swing_anim: read_f32(r)?,
        hurt_timer: read_f32(r)?,
        is_sneaking: read_bool(r)?,
        appearance: CharacterAppearance::read_binary(r)?,
        health: read_f32(r)?,
        max_health: read_f32(r)?,
    })
}

fn write_player_skills<W: Write>(w: &mut W, s: &PlayerSkills) -> io::Result<()> {
    write_u8(w, SKILL_COUNT as u8)?;
    for entry in &s.entries {
        write_u16(w, entry.level)?;
        write_u64(w, entry.xp)?;
    }
    Ok(())
}

fn read_player_skills<R: Read>(r: &mut R) -> io::Result<PlayerSkills> {
    let count = read_u8(r)? as usize;
    let mut skills = PlayerSkills::new();
    for i in 0..count {
        let lvl = read_u16(r)?.clamp(1, crate::skills::MAX_SKILL_LEVEL);
        let xp = read_u64(r)?;
        if i < SKILL_COUNT {
            skills.entries[i] = crate::skills::SkillProgress { level: lvl, xp };
        }
    }
    Ok(skills)
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
    write_u32(w, p.homes.len() as u32)?;
    for (name, (pos, yaw, pitch)) in &p.homes {
        write_string(w, name)?;
        write_vec3(w, *pos)?;
        write_f32(w, *yaw)?;
        write_f32(w, *pitch)?;
    }
    write_u8(w, p.stats.game_mode as u8)?;
    write_player_skills(w, &p.skills)?;
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

    let mut homes = std::collections::HashMap::new();
    if let Ok(home_count) = read_u32(r) {
        for _ in 0..home_count {
            if let (Ok(name), Ok(pos), Ok(yaw), Ok(pitch)) = (
                read_string(r),
                read_vec3(r),
                read_f32(r),
                read_f32(r),
            ) {
                homes.insert(name, (pos, yaw, pitch));
            }
        }
    }

    if let Ok(gm) = read_u8(r) {
        stats.game_mode = crate::GameMode::from_u8(gm);
    }

    let skills = read_player_skills(r).unwrap_or_default();

    Ok(SavedPlayerState {
        position,
        velocity,
        yaw,
        pitch,
        stats,
        inventory,
        active_hotbar_slot,
        rank: if rank.is_empty() { "default".to_string() } else { rank },
        homes,
        skills,
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
            player: PlayerModelState::new(7, "BuilderBob", Vec3::new(2.0, 70.0, 3.0), CharacterAppearance::randomized(2)),
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
            MultiplayerClient::connect_and_authenticate(&addr_str, "Alice", "alice_pass", CharacterAppearance::randomized(1))
                .expect("Alice should authenticate");

        if let ServerPacket::AuthResponse {
            success,
            seed,
            appearance,
            ..
        } = auth_alice
        {
            assert!(success);
            assert_eq!(seed, 424242);
            assert_eq!(appearance, CharacterAppearance::randomized(1));
        } else {
            panic!("Expected AuthResponse");
        }

        // 2. Duplicate login as "Alice" while Alice is online is rejected
        let dup_res =
            MultiplayerClient::connect_and_authenticate(&addr_str, "alice", "alice_pass", CharacterAppearance::randomized(0));
        assert!(dup_res.is_err());

        // 3. Client 2 ("Bob") connects & registers
        let (mut client_bob, _auth_bob) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "Bob", "bob_pass", CharacterAppearance::randomized(2))
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
                    prefix: _,
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
            MultiplayerClient::connect_and_authenticate(&addr_str, "AliceDrop", "pass", CharacterAppearance::randomized(1))
                .expect("Alice should authenticate");
        let alice_id = client_alice.player_id;
        assert_ne!(alice_id, 0);

        // 2. Client 2 (Bob) connects
        let (mut client_bob, _auth_bob) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "BobDrop", "pass", CharacterAppearance::randomized(2))
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
            MultiplayerClient::connect_and_authenticate(&addr_str, "CharlieDrop", "pass", CharacterAppearance::randomized(3))
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
            MultiplayerClient::connect_and_authenticate(&addr_str, "AdminUser", "secret1", CharacterAppearance::randomized(0))
                .expect("Admin should authenticate");

        // 2. Connect regular player Bob
        let (mut client_bob, _auth_bob) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "BobUser", "secret2", CharacterAppearance::randomized(1))
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
            MultiplayerClient::connect_and_authenticate(&addr_str, "BobUser", "secret2", CharacterAppearance::randomized(1));
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
            MultiplayerClient::connect_and_authenticate(&addr_str, "BobUser", "secret2", CharacterAppearance::randomized(1))
                .expect("Bob should be able to reconnect after unban");

        // 9. Admin issues /ban-ip on BobUser
        server.execute_command("ban-ip BobUser Malicious behavior");
        thread::sleep(Duration::from_millis(200));

        // Bob receives Kick packet
        let bob_pkts2 = client_bob2.poll_packets();
        let kicked_ip = bob_pkts2.iter().any(|p| {
            matches!(
                p,
                ServerPacket::Kick { reason } if reason.contains("Banned IP")
            )
        });
        assert!(kicked_ip, "Bob must receive kick packet for IP ban");

        // 10. Someone tries to log in from the same IP with a DIFFERENT username (Alt account)
        let alt_res =
            MultiplayerClient::connect_and_authenticate(&addr_str, "BobAltAccount", "diff_pass", CharacterAppearance::randomized(0));
        match alt_res {
            Err(err_msg) => {
                assert!(
                    err_msg.contains("Banned IP") || err_msg.contains("banned"),
                    "Alt account must be blocked by IP ban, got: {}",
                    err_msg
                );
            }
            Ok(_) => panic!("Alt account on banned IP must not be allowed to authenticate!"),
        }

        // 11. Console unbans the IP address
        let local_ip = client_admin.server_addr.split(':').next().unwrap_or("127.0.0.1");
        server.execute_command(&format!("unban-ip {}", local_ip));
        thread::sleep(Duration::from_millis(150));

        // 12. Alt account can now connect after IP unban
        let (mut client_alt, _) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "BobAltAccount", "diff_pass", CharacterAppearance::randomized(0))
                .expect("Alt account should be able to connect after IP unban");

        // Clean shutdown
        client_admin.disconnect();
        client_bob2.disconnect();
        client_alt.disconnect();
        server.shutdown();

        // 13. Verify persistence files exist in config/
        let config_dir = temp_root.join("config");
        assert!(config_dir.join("permissions.yml").exists(), "config/permissions.yml must be saved");
        assert!(config_dir.join("bans.txt").exists(), "config/bans.txt must be saved");
        assert!(config_dir.join("banned-ips.txt").exists(), "config/banned-ips.txt must be saved");

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_multiplayer_config_dir_migration_and_atomic_persistence() {
        let temp_root = std::env::temp_dir().join("blockgame_net_config_migrate_test");
        let _ = std::fs::remove_dir_all(&temp_root);
        let saves_root = temp_root.join("saves");
        let world_dir = saves_root.join("migrate_world");
        let config_dir = temp_root.join("config");
        std::fs::create_dir_all(&world_dir).unwrap();

        // Seed legacy files at server root (permissions.yml, banned-ips.txt) and world_dir (bans.txt)
        let mut legacy_perms = PermissionsManager::new();
        legacy_perms.add_player_group("LegacyAdmin", "admin").unwrap();
        legacy_perms.save_to_dir(&temp_root).unwrap();

        let mut legacy_bans = BanManager::new();
        legacy_bans.ban("LegacyGriefer", "Old grief", "Console");
        legacy_bans.save_to_dir(&world_dir).unwrap();
        // Put an IP ban in server root banned-ips.txt to verify fallback across legacy dirs
        std::fs::remove_file(world_dir.join("banned-ips.txt")).unwrap();
        let mut root_ip_bans = BanManager::new();
        root_ip_bans.ban_ip("10.0.0.99", "Legacy IP ban", "Console");
        root_ip_bans.save_to_dir(&temp_root).unwrap();

        let mut meta = WorldMetadata::new("Migrate World", 1234);
        meta.folder_name = "migrate_world".to_string();
        let save_data = WorldSaveData::new(meta, SavedPlayerState::default());

        let mut server = MultiplayerServer::start_with_config_dir(
            "127.0.0.1:0",
            saves_root,
            config_dir.clone(),
            "migrate_world".to_string(),
            "Migrate World".to_string(),
            1234,
            Some(save_data),
            |cpos| crate::chunk::Chunk::new(cpos),
        )
        .expect("Failed to start server with custom config_dir");

        let addr_str = server.local_addr().to_string();

        // Migrated ban should immediately block LegacyGriefer
        let ban_attempt = MultiplayerClient::connect_and_authenticate(
            &addr_str,
            "LegacyGriefer",
            "pass",
            CharacterAppearance::randomized(0),
        );
        assert!(ban_attempt.is_err(), "Migrated ban must block LegacyGriefer");

        server.shutdown();

        // Verify files were migrated into config/ non-destructively
        assert!(config_dir.join("permissions.yml").exists());
        assert!(config_dir.join("bans.txt").exists());
        assert!(config_dir.join("banned-ips.txt").exists());
        assert!(temp_root.join("permissions.yml").exists(), "Legacy root file kept non-destructively");

        let reloaded_perms = PermissionsManager::load_from_dir(&config_dir).unwrap();
        assert!(reloaded_perms.has_permission("LegacyAdmin", "blockgame.command.ban"));

        let reloaded_bans = BanManager::load_from_dir(&config_dir).unwrap();
        assert!(reloaded_bans.is_banned("LegacyGriefer").is_some());
        assert!(reloaded_bans.is_ip_banned("10.0.0.99").is_some());

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_multiplayer_automessages_commands_and_persistence() {
        let temp_root = std::env::temp_dir().join("blockgame_net_automessage_test");
        let _ = std::fs::remove_dir_all(&temp_root);
        let saves_root = temp_root.join("saves");
        let config_dir = temp_root.join("config");
        std::fs::create_dir_all(&saves_root).unwrap();

        let mut meta = WorldMetadata::new("AutoMsg World", 999);
        meta.folder_name = "automsg_world".to_string();
        let save_data = WorldSaveData::new(meta, SavedPlayerState::default());

        let mut server = MultiplayerServer::start_with_config_dir(
            "127.0.0.1:0",
            saves_root,
            config_dir.clone(),
            "automsg_world".to_string(),
            "AutoMsg World".to_string(),
            999,
            Some(save_data),
            |cpos| crate::chunk::Chunk::new(cpos),
        )
        .expect("Failed to start server");

        let addr_str = server.local_addr().to_string();
        let (mut admin, _) = MultiplayerClient::connect_and_authenticate(
            &addr_str,
            "AdminAM",
            "pass",
            CharacterAppearance::randomized(0),
        )
        .expect("Admin should connect");

        // Configure Auto-Messages via commands
        admin.send(&ClientPacket::Chat {
            message: "/automessage add &bJoin https://blockgame.dev".to_string(),
        });
        admin.send(&ClientPacket::Chat {
            message: "/automessage interval 5".to_string(),
        });
        admin.send(&ClientPacket::Chat {
            message: "/automessage order random".to_string(),
        });
        admin.send(&ClientPacket::Chat {
            message: "/automessage on".to_string(),
        });
        thread::sleep(Duration::from_millis(200));
        let pkts = admin.poll_packets();
        assert!(
            pkts.iter().any(|p| matches!(p, ServerPacket::ChatBroadcast { message, .. } if message.contains("Auto-Messages turned ON"))),
            "Expected Auto-Messages ON confirmation"
        );

        admin.disconnect();
        server.shutdown();

        let reloaded = crate::automessage::AutoMessageManager::load_from_dir(&config_dir).unwrap();
        assert!(reloaded.config.enabled);
        assert_eq!(reloaded.config.interval_secs, 5);
        assert_eq!(reloaded.config.order, crate::automessage::AutoMessageOrder::Random);
        assert!(reloaded
            .config
            .messages
            .iter()
            .any(|m| m == "&bJoin https://blockgame.dev"));

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_multiplayer_chat_moderation_suite() {
        let temp_root = std::env::temp_dir().join("blockgame_net_moderation_test");
        let _ = std::fs::remove_dir_all(&temp_root);
        let saves_root = temp_root.join("saves");
        let config_dir = temp_root.join("config");
        std::fs::create_dir_all(&saves_root).unwrap();

        let mut meta = WorldMetadata::new("Mod World", 777);
        meta.folder_name = "mod_world".to_string();
        let save_data = WorldSaveData::new(meta, SavedPlayerState::default());

        let mut server = MultiplayerServer::start_with_config_dir(
            "127.0.0.1:0",
            saves_root,
            config_dir.clone(),
            "mod_world".to_string(),
            "Mod World".to_string(),
            777,
            Some(save_data),
            |cpos| crate::chunk::Chunk::new(cpos),
        )
        .expect("Failed to start server");

        let addr_str = server.local_addr().to_string();
        let (mut admin, _) = MultiplayerClient::connect_and_authenticate(
            &addr_str,
            "AdminMod",
            "pass",
            CharacterAppearance::randomized(0),
        )
        .expect("Admin should connect");
        let (mut bob, _) = MultiplayerClient::connect_and_authenticate(
            &addr_str,
            "BobMod",
            "pass",
            CharacterAppearance::randomized(1),
        )
        .expect("Bob should connect");
        thread::sleep(Duration::from_millis(120));
        let _ = admin.poll_packets();
        let _ = bob.poll_packets();

        // 1. Word Filter (Censor mode, then Block mode, plus Admin bypass)
        admin.send(&ClientPacket::Chat {
            message: "/wordfilter add badword".to_string(),
        });
        admin.send(&ClientPacket::Chat {
            message: "/wordfilter on".to_string(),
        });
        thread::sleep(Duration::from_millis(120));
        let _ = admin.poll_packets();
        let _ = bob.poll_packets();

        bob.send(&ClientPacket::Chat {
            message: "This has badword inside".to_string(),
        });
        thread::sleep(Duration::from_millis(120));
        let admin_pkts = admin.poll_packets();
        assert!(
            admin_pkts.iter().any(|p| matches!(p, ServerPacket::ChatBroadcast { sender, message, .. } if sender == "BobMod" && message.contains("***"))),
            "Word filter censor mode must replace badword with ***"
        );

        admin.send(&ClientPacket::Chat {
            message: "/wordfilter mode block".to_string(),
        });
        thread::sleep(Duration::from_millis(100));
        let _ = admin.poll_packets();
        let _ = bob.poll_packets();

        bob.send(&ClientPacket::Chat {
            message: "Another badword message".to_string(),
        });
        thread::sleep(Duration::from_millis(120));
        let bob_pkts = bob.poll_packets();
        assert!(
            bob_pkts.iter().any(|p| matches!(p, ServerPacket::ChatBroadcast { message, is_system: true, .. } if message.contains("Word Filter"))),
            "Block mode must reject Bob's message"
        );

        // 2. Warning + online notification + history
        admin.send(&ClientPacket::Chat {
            message: "/warn BobMod Watch your language".to_string(),
        });
        thread::sleep(Duration::from_millis(120));
        let bob_warn_pkts = bob.poll_packets();
        assert!(
            bob_warn_pkts.iter().any(|p| matches!(p, ServerPacket::ChatBroadcast { message, is_system: true, .. } if message.contains("Watch your language"))),
            "Online target must receive Warning notice"
        );

        // 3. Mute + block chat
        admin.send(&ClientPacket::Chat {
            message: "/mute BobMod 10m Spamming".to_string(),
        });
        thread::sleep(Duration::from_millis(120));
        let _ = admin.poll_packets();
        let _ = bob.poll_packets();

        bob.send(&ClientPacket::Chat {
            message: "Can I talk?".to_string(),
        });
        thread::sleep(Duration::from_millis(120));
        let bob_muted_pkts = bob.poll_packets();
        assert!(
            bob_muted_pkts.iter().any(|p| matches!(p, ServerPacket::ChatBroadcast { message, is_system: true, .. } if message.contains("muted"))),
            "Muted player must be blocked from chatting"
        );
        let admin_saw_bob = admin.poll_packets().iter().any(|p| matches!(p, ServerPacket::ChatBroadcast { sender, .. } if sender == "BobMod"));
        assert!(!admin_saw_bob, "Muted player's chat must not broadcast");

        // 4. Global Chat Lock & Slowmode & ClearChat
        admin.send(&ClientPacket::Chat {
            message: "/unmute BobMod".to_string(),
        });
        admin.send(&ClientPacket::Chat {
            message: "/chatlock on".to_string(),
        });
        thread::sleep(Duration::from_millis(120));
        let _ = admin.poll_packets();
        let _ = bob.poll_packets();

        bob.send(&ClientPacket::Chat {
            message: "Locked?".to_string(),
        });
        thread::sleep(Duration::from_millis(120));
        assert!(
            bob.poll_packets().iter().any(|p| matches!(p, ServerPacket::ChatBroadcast { message, is_system: true, .. } if message.contains("locked"))),
            "Global Chat Lock must block regular player"
        );

        admin.send(&ClientPacket::Chat {
            message: "/clearchat".to_string(),
        });
        thread::sleep(Duration::from_millis(120));
        assert!(
            bob.poll_packets().iter().any(|p| matches!(p, ServerPacket::ClearChat)),
            "ClearChat packet must be broadcast to clients"
        );

        bob.disconnect();
        admin.disconnect();
        server.shutdown();

        // Verify independent config files persisted cleanly
        let reloaded = crate::moderation::ModerationManager::load_from_dir(&config_dir);
        assert_eq!(reloaded.get_warnings("BobMod").len(), 1);
        assert!(reloaded.chat_settings.chat_locked);
        assert!(reloaded.word_filter.enabled);
        assert_eq!(reloaded.word_filter.mode, crate::moderation::WordFilterMode::Block);

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
        let (mut p1, _) = MultiplayerClient::connect_and_authenticate(&addr_str, "SleepA", "pw1", CharacterAppearance::randomized(0)).unwrap();
        let (mut p2, _) = MultiplayerClient::connect_and_authenticate(&addr_str, "SleepB", "pw2", CharacterAppearance::randomized(1)).unwrap();
        let (mut p3, _) = MultiplayerClient::connect_and_authenticate(&addr_str, "SleepC", "pw3", CharacterAppearance::randomized(0)).unwrap();

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

    #[test]
    fn test_multiplayer_ungraceful_disconnect_removal_and_reconnect() {
        let temp_root = std::env::temp_dir().join("blockgame_net_ungraceful_dc_test");
        let _ = std::fs::remove_dir_all(&temp_root);
        std::fs::create_dir_all(&temp_root).unwrap();

        let mut server = MultiplayerServer::start(
            "127.0.0.1:0",
            temp_root.clone(),
            "dc_test_world".to_string(),
            "Disconnect Test World".to_string(),
            12345,
            None,
            |cpos| crate::chunk::Chunk::new(cpos),
        )
        .expect("Failed to start server");

        let addr_str = server.local_addr().to_string();

        // 1. Player connects and authenticates
        let (mut client, _) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "UngracefulUser", "pass123", CharacterAppearance::randomized(0))
                .expect("Initial connection failed");

        thread::sleep(Duration::from_millis(100));
        assert_eq!(server.online_player_count(), 1, "Player should be counted as online");

        // 2. Ungraceful disconnect: drop the client socket without sending ClientPacket::Disconnect
        // Set connected = false so drop() doesn't send ClientPacket::Disconnect
        client.connected = false;
        drop(client);

        // 3. Within 1.5s, the server tick should attempt a write or detect EOF,
        // marking the client as dead and removing them from online_count without requiring a server restart.
        let start = Instant::now();
        let mut cleaned_up = false;
        while start.elapsed() < Duration::from_secs(2) {
            thread::sleep(Duration::from_millis(50));
            if server.online_player_count() == 0 {
                cleaned_up = true;
                break;
            }
        }
        assert!(
            cleaned_up,
            "Server must remove ungracefully disconnected player without restarting, but online_player_count remained {}",
            server.online_player_count()
        );

        // 4. The player can now immediately reconnect without getting 'User is already logged in'
        let (mut client2, _) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "UngracefulUser", "pass123", CharacterAppearance::randomized(0))
                .expect("Player must be able to reconnect after ungraceful disconnect");
        thread::sleep(Duration::from_millis(100));
        assert_eq!(server.online_player_count(), 1);

        client2.disconnect();
        server.shutdown();
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_skills_packet_roundtrip_and_server_sync() {
        let mut custom_skills = crate::skills::PlayerSkills::new();
        custom_skills.set_level(crate::skills::SkillKind::Mining, 125);
        custom_skills.add_xp(crate::skills::SkillKind::Combat, 750);

        let cpkt = ClientPacket::SkillsSync {
            skills: custom_skills.clone(),
        };
        let framed = cpkt.encode().unwrap();
        let cdecoded = ClientPacket::decode(&framed[8..]).unwrap();
        assert_eq!(cpkt, cdecoded);

        let spkt = ServerPacket::SkillsUpdate {
            skills: custom_skills.clone(),
        };
        let sframed = spkt.encode().unwrap();
        let sdecoded = ServerPacket::decode(&sframed[8..]).unwrap();
        assert_eq!(spkt, sdecoded);

        // Full server roundtrip: client syncs skills, server persists per-server and handles /skills set
        let temp_root = std::env::temp_dir().join("blockgame_net_skills_sync_test");
        let _ = std::fs::remove_dir_all(&temp_root);
        std::fs::create_dir_all(&temp_root).unwrap();

        let mut server = MultiplayerServer::start(
            "127.0.0.1:0",
            temp_root.clone(),
            "skills_sync_world".to_string(),
            "Skills Sync World".to_string(),
            7777,
            None,
            |cpos| crate::chunk::Chunk::new(cpos),
        )
        .expect("Server should start");

        let addr_str = server.local_addr().to_string();
        let (mut client, auth) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "Skiller", "pw123", CharacterAppearance::randomized(0))
                .expect("Client should authenticate");
        if let ServerPacket::AuthResponse { player_state, .. } = auth {
            assert_eq!(player_state.skills.level(crate::skills::SkillKind::Mining), 1);
        } else {
            panic!("Expected AuthResponse");
        }

        // Send skill progress from client
        client.send(&ClientPacket::SkillsSync {
            skills: custom_skills.clone(),
        });
        thread::sleep(Duration::from_millis(120));

        // Server executes `/skills set Skiller farming 300` -> pushes SkillsUpdate packet
        server.execute_command("skills set Skiller farming 300");
        thread::sleep(Duration::from_millis(120));
        let pkts = client.poll_packets();
        let updated = pkts.iter().rev().find_map(|p| match p {
            ServerPacket::SkillsUpdate { skills } => Some(skills.clone()),
            _ => None,
        });
        let updated = updated.expect("Expected ServerPacket::SkillsUpdate after /skills set");
        assert_eq!(updated.level(crate::skills::SkillKind::Mining), 125);
        assert_eq!(updated.level(crate::skills::SkillKind::Farming), 300);

        // Reconnect and confirm server remembers both synced Mining 125 and admin-set Farming 300
        client.disconnect();
        thread::sleep(Duration::from_millis(120));
        let (mut client2, auth2) =
            MultiplayerClient::connect_and_authenticate(&addr_str, "Skiller", "pw123", CharacterAppearance::randomized(0))
                .expect("Client should reconnect");
        if let ServerPacket::AuthResponse { player_state, .. } = auth2 {
            assert_eq!(player_state.skills.level(crate::skills::SkillKind::Mining), 125);
            assert_eq!(player_state.skills.level(crate::skills::SkillKind::Farming), 300);
        } else {
            panic!("Expected AuthResponse on reconnect");
        }

        client2.disconnect();
        server.shutdown();
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_dry_spawn_selection_avoids_river() {
        let gen_closure: Arc<dyn Fn(ChunkPos) -> crate::chunk::Chunk + Send + Sync> =
            Arc::new(|cpos: ChunkPos| {
                let mut ch = crate::chunk::Chunk::new(cpos);
                let origin = cpos.to_world_origin();
                for lz in 0..crate::chunk::CHUNK_SIZE_Z {
                    for lx in 0..crate::chunk::CHUNK_SIZE_X {
                        let is_dry_island = cpos.0.x == 1 && cpos.0.z == 0;
                        let surface_y = if is_dry_island { 46 } else { 35 };
                        for ly in 0..crate::chunk::CHUNK_SIZE_Y {
                            let wy = origin.y + ly as i32;
                            let b = if wy == 0 {
                                BlockId::BEDROCK
                            } else if wy < surface_y {
                                BlockId::STONE
                            } else if wy <= 38 {
                                BlockId::WATER
                            } else {
                                BlockId::AIR
                            };
                            ch.set(lx, ly, lz, b);
                        }
                    }
                }
                ch
            });

        let spawn = find_dry_spawn_from_generator(&gen_closure);
        assert!(spawn.x >= 16.0 && spawn.x < 32.0, "Expected spawn on dry chunk (cx=1, cz=0), got {:?}", spawn);
        assert!(spawn.y >= 46.0, "Expected dry elevation >= 46, got {:?}", spawn);
    }

    #[test]
    fn test_temple_packets_roundtrip() {
        let c1 = ClientPacket::TempleInteract {
            pos: IVec3::new(100, 64, -200),
            temple_id: 4242,
            kind: crate::temple::TempleKind::DarkForest.to_u8(),
            overworld_origin: IVec3::new(100, 63, -203),
            portal_pos: IVec3::new(100, 64, -200),
        };
        let bytes = c1.encode().unwrap();
        assert_eq!(ClientPacket::decode(&bytes[8..]).unwrap(), c1);

        let c2 = ClientPacket::AttackTempleMonster {
            monster_id: 100_005,
            damage: 14.5,
            knockback: Vec3::new(1.0, 0.0, 0.0),
        };
        let bytes = c2.encode().unwrap();
        assert_eq!(ClientPacket::decode(&bytes[8..]).unwrap(), c2);

        let s1 = ServerPacket::TempleMonstersSync {
            monsters: vec![SyncedMonsterState {
                id: 100_005,
                kind: crate::monster::MonsterKind::ShadowLich,
                position: Vec3::new(200_000.5, 33.0, 12.5),
                yaw: 1.25,
                health: 210.0,
                max_health: 320.0,
                hurt_timer: 0.2,
                walk_anim_timer: 3.4,
                phase: 2,
                in_combat: true,
                invulnerable: true,
            }],
        };
        let bytes = s1.encode().unwrap();
        assert_eq!(ServerPacket::decode(&bytes[8..]).unwrap(), s1);

        let s2 = ServerPacket::BossHudUpdate {
            hud: Some(crate::temple::BossHudState {
                name: "Shadow Lich".to_string(),
                kind: crate::monster::MonsterKind::ShadowLich,
                health: 210.0,
                max_health: 320.0,
                phase: 2,
                distance: 18.5,
                invulnerable: true,
            }),
        };
        let bytes = s2.encode().unwrap();
        assert_eq!(ServerPacket::decode(&bytes[8..]).unwrap(), s2);

        let s3 = ServerPacket::TempleProtectionSync {
            zones: vec![crate::temple::TempleBounds {
                min: IVec3::new(-12, 40, -12),
                max: IVec3::new(12, 58, 12),
            }],
        };
        let bytes = s3.encode().unwrap();
        assert_eq!(ServerPacket::decode(&bytes[8..]).unwrap(), s3);

        let s4 = ServerPacket::TempleArrowsSync {
            arrows: vec![SyncedArrowState {
                id: 77,
                position: Vec3::new(24_001.5, 34.0, -4.25),
                velocity: Vec3::new(14.0, 2.5, -6.0),
                stuck: false,
            }],
        };
        let bytes = s4.encode().unwrap();
        assert_eq!(ServerPacket::decode(&bytes[8..]).unwrap(), s4);
    }

    #[test]
    fn test_achievement_packets_roundtrip() {
        let c = ClientPacket::AchievementUnlocked {
            achievement_id: "boss_sun_pharaoh".to_string(),
        };
        let bytes = c.encode().unwrap();
        assert_eq!(ClientPacket::decode(&bytes[8..]).unwrap(), c);

        let s1 = ServerPacket::AchievementEligibility {
            enabled: false,
            reason: Some("Creative world".to_string()),
        };
        let bytes = s1.encode().unwrap();
        assert_eq!(ServerPacket::decode(&bytes[8..]).unwrap(), s1);

        let s2 = ServerPacket::AchievementEvent {
            event: crate::achievement::AchievementEvent::MonsterKilled {
                is_boss: true,
                boss_key: Some("frost_titan".to_string()),
            },
        };
        let bytes = s2.encode().unwrap();
        assert_eq!(ServerPacket::decode(&bytes[8..]).unwrap(), s2);
    }

    #[test]
    fn test_multiplayer_achievement_broadcast_dedup_toggle_and_per_player_taint() {
        let temp_root = std::env::temp_dir().join(format!(
            "blockgame_mp_ach_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_root);
        std::fs::create_dir_all(&temp_root).unwrap();

        let server = MultiplayerServer::start(
            "127.0.0.1:0",
            temp_root.clone(),
            "mp_ach_world".to_string(),
            "MP Ach World".to_string(),
            42,
            None,
            |cpos| crate::chunk::Chunk::new(cpos),
        )
        .expect("start server");
        let addr = server.local_addr().to_string();

        let (mut alice, _) = MultiplayerClient::connect_and_authenticate(
            &addr,
            "Alice",
            "pass1234",
            CharacterAppearance::default(),
        )
        .expect("alice connect");
        let (mut bob, _) = MultiplayerClient::connect_and_authenticate(
            &addr,
            "Bob",
            "pass1234",
            CharacterAppearance::default(),
        )
        .expect("bob connect");

        // Drain initial join packets and confirm Alice receives eligible=true
        std::thread::sleep(std::time::Duration::from_millis(80));
        let alice_pkts = alice.poll_packets();
        assert!(
            alice_pkts.iter().any(|p| matches!(
                p,
                ServerPacket::AchievementEligibility { enabled: true, .. }
            )),
            "Expected initial AchievementEligibility(true) on join"
        );
        let _ = bob.poll_packets();

        // 1. Alice unlocks first_timber -> broadcast once, duplicate suppressed
        alice.send(&ClientPacket::AchievementUnlocked {
            achievement_id: "first_timber".to_string(),
        });
        alice.send(&ClientPacket::AchievementUnlocked {
            achievement_id: "first_timber".to_string(),
        });
        std::thread::sleep(std::time::Duration::from_millis(80));
        let bob_pkts = bob.poll_packets();
        let timber_msgs: Vec<_> = bob_pkts
            .iter()
            .filter(|p| {
                matches!(
                    p,
                    ServerPacket::ChatBroadcast { message, is_system: true, .. }
                        if message.contains("Alice unlocked [Timber!]!")
                )
            })
            .collect();
        assert_eq!(timber_msgs.len(), 1, "Expected exactly 1 deduplicated broadcast");

        // 2. Disable broadcast_achievements -> no chat broadcast for craft_table
        server.set_broadcast_achievements(false);
        alice.send(&ClientPacket::AchievementUnlocked {
            achievement_id: "craft_table".to_string(),
        });
        std::thread::sleep(std::time::Duration::from_millis(80));
        let bob_pkts = bob.poll_packets();
        assert!(
            !bob_pkts.iter().any(|p| {
                matches!(
                    p,
                    ServerPacket::ChatBroadcast { message, .. }
                        if message.contains("Getting to Work")
                )
            }),
            "Expected broadcast to be suppressed when broadcast_achievements=false"
        );

        // 3. Per-player cheat tainting: /give Alice diamond 1 taints Alice, not Bob
        server.set_broadcast_achievements(true);
        server.execute_command("give Alice diamond 1");
        std::thread::sleep(std::time::Duration::from_millis(80));
        let alice_pkts = alice.poll_packets();
        let bob_pkts = bob.poll_packets();
        let taint_pos = alice_pkts
            .iter()
            .position(|p| matches!(p, ServerPacket::AchievementEligibility { enabled: false, .. }))
            .expect("Alice should be tainted after /give");
        let recv_pos = alice_pkts
            .iter()
            .position(|p| matches!(p, ServerPacket::ReceiveItem { .. }))
            .expect("Alice should receive the /give item");
        assert!(
            taint_pos < recv_pos,
            "AchievementEligibility(false) must arrive before ReceiveItem"
        );
        assert!(
            !bob_pkts.iter().any(|p| matches!(
                p,
                ServerPacket::AchievementEligibility { enabled: false, .. }
            )),
            "Bob must remain eligible when only Alice was targeted"
        );

        drop(alice);
        drop(bob);
        drop(server);
        let _ = std::fs::remove_dir_all(&temp_root);
    }
}

