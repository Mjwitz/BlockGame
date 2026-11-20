use std::collections::{HashMap, HashSet, VecDeque};

use glam::IVec3;

use crate::block::BlockId;
use crate::item::{ItemId, ItemRegistry, ItemSlot};
use crate::world::World;

/// Base EU/s draw per Storage Terminal in a linked storage network.
pub const STORAGE_TERMINAL_EU_S: f32 = 15.0;
/// Additional EU/s draw per linked Chest (Wood/Iron/Gold/Diamond) in the network.
pub const STORAGE_CHEST_LINK_EU_S: f32 = 5.0;
/// Additional EU/s draw per Storage Cable segment in the network.
pub const STORAGE_CABLE_EU_S: f32 = 1.0;

const NEIGHBORS_6: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageSortMode {
    #[default]
    CountDesc,
    NameAsc,
    ItemIdAsc,
}

impl StorageSortMode {
    pub fn next(self) -> Self {
        match self {
            Self::CountDesc => Self::NameAsc,
            Self::NameAsc => Self::ItemIdAsc,
            Self::ItemIdAsc => Self::CountDesc,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CountDesc => "Sort: Count",
            Self::NameAsc => "Sort: Name",
            Self::ItemIdAsc => "Sort: ID",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StorageNetworkInfo {
    pub terminals: Vec<IVec3>,
    pub chests: Vec<IVec3>,
    pub cables: usize,
    pub power_draw_eu_s: f32,
    pub is_powered: bool,
    pub total_slots: usize,
    pub used_slots: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggregatedStorageItem {
    pub item_id: ItemId,
    pub total_count: u32,
}

/// Discovers the connected Storage Network (Terminals + Storage Cables + all adjacent Chests)
/// starting from `start_pos` and computes its complexity-scaled EU/s power requirement.
pub fn collect_storage_network(world: &World, start_pos: IVec3) -> StorageNetworkInfo {
    let start_block = world.get_block(start_pos);
    if !start_block.is_storage_node() {
        return StorageNetworkInfo::default();
    }

    let mut visited_bus = HashSet::new();
    let mut visited_chests = HashSet::new();
    let mut queue = VecDeque::new();
    let mut info = StorageNetworkInfo::default();

    if start_block == BlockId::STORAGE_TERMINAL || start_block == BlockId::STORAGE_CABLE {
        visited_bus.insert(start_pos);
        queue.push_back(start_pos);
    }

    while let Some(pos) = queue.pop_front() {
        let block = world.get_block(pos);
        if block == BlockId::STORAGE_TERMINAL {
            info.terminals.push(pos);
            if world.powered_storage_terminals.contains(&pos) {
                info.is_powered = true;
            }
        } else if block == BlockId::STORAGE_CABLE {
            info.cables += 1;
        }

        for offset in NEIGHBORS_6 {
            let npos = pos + offset;
            let nb = world.get_block(npos);
            if nb == BlockId::STORAGE_TERMINAL || nb == BlockId::STORAGE_CABLE {
                if visited_bus.insert(npos) {
                    queue.push_back(npos);
                }
            } else if nb.is_chest() && visited_chests.insert(npos) {
                info.chests.push(npos);
                let cap = nb.chest_slot_count();
                info.total_slots += cap;
                if let Some(slots) = world.get_chest(npos) {
                    info.used_slots += slots.iter().filter(|s| !s.is_empty()).count();
                }
            }
        }
    }

    // Sort chests deterministically so deposit/withdraw order is stable across frames
    info.chests
        .sort_by_key(|p| (p.y, p.x, p.z));
    info.terminals
        .sort_by_key(|p| (p.y, p.x, p.z));

    info.power_draw_eu_s = info.terminals.len() as f32 * STORAGE_TERMINAL_EU_S
        + info.chests.len() as f32 * STORAGE_CHEST_LINK_EU_S
        + info.cables as f32 * STORAGE_CABLE_EU_S;

    info
}

/// Computes the EU/s draw that a single Storage Terminal contributes to its electrical grid.
/// When multiple terminals share the same storage network, the network's total power cost
/// is split evenly across its terminals so total EU/s drawn equals the exact network cost.
pub fn terminal_power_draw_eu_s(world: &World, terminal_pos: IVec3) -> f32 {
    let info = collect_storage_network(world, terminal_pos);
    if info.terminals.is_empty() {
        STORAGE_TERMINAL_EU_S
    } else {
        info.power_draw_eu_s / info.terminals.len() as f32
    }
}

/// Aggregates, filters by `search`, and sorts all items stored across `chests`.
pub fn list_network_items(
    world: &World,
    chests: &[IVec3],
    search: &str,
    sort_mode: StorageSortMode,
    registry: &ItemRegistry,
) -> Vec<AggregatedStorageItem> {
    let mut totals: HashMap<ItemId, u32> = HashMap::new();
    for &cpos in chests {
        if let Some(slots) = world.get_chest(cpos) {
            for slot in slots {
                if !slot.is_empty() {
                    *totals.entry(slot.item_id).or_insert(0) += slot.count as u32;
                }
            }
        }
    }

    let q = search.trim().to_lowercase();
    let mut items: Vec<AggregatedStorageItem> = totals
        .into_iter()
        .filter(|(id, _)| {
            if q.is_empty() {
                return true;
            }
            registry
                .get(*id)
                .map(|d| d.display_name.to_lowercase().contains(&q))
                .unwrap_or(false)
        })
        .map(|(item_id, total_count)| AggregatedStorageItem {
            item_id,
            total_count,
        })
        .collect();

    match sort_mode {
        StorageSortMode::CountDesc => {
            items.sort_by(|a, b| {
                b.total_count
                    .cmp(&a.total_count)
                    .then_with(|| a.item_id.0.cmp(&b.item_id.0))
            });
        }
        StorageSortMode::NameAsc => {
            items.sort_by(|a, b| {
                let na = registry.get(a.item_id).map(|d| d.display_name).unwrap_or("");
                let nb = registry.get(b.item_id).map(|d| d.display_name).unwrap_or("");
                na.cmp(nb).then_with(|| a.item_id.0.cmp(&b.item_id.0))
            });
        }
        StorageSortMode::ItemIdAsc => {
            items.sort_by_key(|a| a.item_id.0);
        }
    }

    items
}

/// Deposits `slot` into the first matching partial stack or empty slot across `chests`.
/// Returns any leftover items that could not fit, or `None` if everything was stored.
pub fn deposit_into_network(
    world: &mut World,
    chests: &[IVec3],
    mut slot: ItemSlot,
    registry: &ItemRegistry,
) -> Option<ItemSlot> {
    if slot.is_empty() {
        return None;
    }
    let max_stack = registry
        .get(slot.item_id)
        .map(|d| d.max_stack)
        .unwrap_or(64);

    // Pass 1: merge into existing partial stacks of the same item (if stackable & no durability)
    if max_stack > 1 && slot.durability == 0 {
        for &cpos in chests {
            let chest = world.get_or_create_chest(cpos);
            for s in chest.iter_mut() {
                if !s.is_empty() && s.item_id == slot.item_id && s.count < max_stack {
                    let add = (max_stack - s.count).min(slot.count);
                    s.count += add;
                    slot.count -= add;
                    if slot.count == 0 {
                        return None;
                    }
                }
            }
        }
    }

    // Pass 2: place into empty slots
    for &cpos in chests {
        let chest = world.get_or_create_chest(cpos);
        for s in chest.iter_mut() {
            if s.is_empty() {
                let take = slot.count.min(max_stack);
                *s = ItemSlot {
                    item_id: slot.item_id,
                    count: take,
                    durability: slot.durability,
                };
                slot.count -= take;
                if slot.count == 0 {
                    return None;
                }
            }
        }
    }

    Some(slot)
}

/// Withdraws up to `max_count` of `item_id` from `chests`.
pub fn withdraw_from_network(
    world: &mut World,
    chests: &[IVec3],
    item_id: ItemId,
    max_count: u16,
) -> ItemSlot {
    if max_count == 0 || item_id.is_empty() {
        return ItemSlot::EMPTY;
    }
    let mut taken = 0u16;
    let mut taken_durability = 0u16;

    for &cpos in chests {
        let Some(chest) = world.get_chest_mut(cpos) else {
            continue;
        };
        for s in chest.iter_mut() {
            if !s.is_empty() && s.item_id == item_id {
                let need = max_count - taken;
                let pull = s.count.min(need);
                if taken == 0 {
                    taken_durability = s.durability;
                }
                s.count -= pull;
                taken += pull;
                if s.count == 0 {
                    *s = ItemSlot::EMPTY;
                }
                if taken >= max_count || taken_durability > 0 {
                    return ItemSlot {
                        item_id,
                        count: taken,
                        durability: taken_durability,
                    };
                }
            }
        }
    }

    if taken > 0 {
        ItemSlot {
            item_id,
            count: taken,
            durability: taken_durability,
        }
    } else {
        ItemSlot::EMPTY
    }
}

/// Player's wearable portable Backpack state (Tier 1: 18 slots, Tier 2: 27 slots, Tier 3: 36 slots).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BackpackState {
    /// 0 = none equipped, 1 = Leather (18), 2 = Iron (27), 3 = Diamond (36).
    pub tier: u8,
    pub slots: Vec<ItemSlot>,
}

impl BackpackState {
    pub fn tier_for_item(item_id: ItemId) -> u8 {
        match item_id {
            ItemId::LEATHER_BACKPACK => 1,
            ItemId::IRON_BACKPACK => 2,
            ItemId::DIAMOND_BACKPACK => 3,
            _ => 0,
        }
    }

    pub fn slot_count_for_tier(tier: u8) -> usize {
        match tier {
            1 => 18,
            2 => 27,
            3 => 36,
            _ => 0,
        }
    }

    pub fn title(&self) -> &'static str {
        match self.tier {
            1 => "Leather Backpack (18 Slots)",
            2 => "Iron Backpack (27 Slots)",
            3 => "Diamond Backpack (36 Slots)",
            _ => "Backpack",
        }
    }

    /// Equips or upgrades the backpack tier while preserving all stored items.
    pub fn ensure_tier(&mut self, tier: u8) {
        if tier > self.tier {
            self.tier = tier;
        }
        let target_len = Self::slot_count_for_tier(self.tier);
        if self.slots.len() < target_len {
            self.slots.resize(target_len, ItemSlot::EMPTY);
        }
    }
}
