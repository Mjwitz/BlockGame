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
    /// When `Some((chest_pos, slot_idx))`, represents a single unique / non-stackable item
    /// (such as a Backpack or tool) that is displayed individually and never stacked.
    pub unique_source: Option<(IVec3, usize)>,
    /// Item durability or unique backpack ID (`0` for standard stacked items).
    pub durability: u16,
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

/// Aggregates stackable items while keeping non-stackable / unique items (like Backpacks and tools)
/// as separate individual entries, then filters by `search` and sorts by `sort_mode`.
pub fn list_network_items(
    world: &World,
    chests: &[IVec3],
    search: &str,
    sort_mode: StorageSortMode,
    registry: &ItemRegistry,
) -> Vec<AggregatedStorageItem> {
    let mut totals: HashMap<ItemId, u32> = HashMap::new();
    let mut unique_items: Vec<AggregatedStorageItem> = Vec::new();

    for &cpos in chests {
        if let Some(slots) = world.get_chest(cpos) {
            for (slot_idx, slot) in slots.iter().enumerate() {
                if slot.is_empty() {
                    continue;
                }
                let max_stack = registry
                    .get(slot.item_id)
                    .map(|d| d.max_stack)
                    .unwrap_or(64);
                let is_unique = max_stack <= 1
                    || slot.durability > 0
                    || BackpackState::tier_for_item(slot.item_id) > 0;
                if is_unique {
                    unique_items.push(AggregatedStorageItem {
                        item_id: slot.item_id,
                        total_count: slot.count as u32,
                        unique_source: Some((cpos, slot_idx)),
                        durability: slot.durability,
                    });
                } else {
                    *totals.entry(slot.item_id).or_insert(0) += slot.count as u32;
                }
            }
        }
    }

    let q = search.trim().to_lowercase();
    let matches_search = |id: ItemId| -> bool {
        if q.is_empty() {
            return true;
        }
        registry
            .get(id)
            .map(|d| d.display_name.to_lowercase().contains(&q))
            .unwrap_or(false)
    };

    let mut items: Vec<AggregatedStorageItem> = totals
        .into_iter()
        .filter(|(id, _)| matches_search(*id))
        .map(|(item_id, total_count)| AggregatedStorageItem {
            item_id,
            total_count,
            unique_source: None,
            durability: 0,
        })
        .collect();

    items.extend(
        unique_items
            .into_iter()
            .filter(|entry| matches_search(entry.item_id)),
    );

    let source_key = |src: Option<(IVec3, usize)>| {
        src.map(|(p, i)| (1u8, p.y, p.x, p.z, i))
            .unwrap_or((0, 0, 0, 0, 0))
    };

    match sort_mode {
        StorageSortMode::CountDesc => {
            items.sort_by(|a, b| {
                b.total_count
                    .cmp(&a.total_count)
                    .then_with(|| a.item_id.0.cmp(&b.item_id.0))
                    .then_with(|| a.durability.cmp(&b.durability))
                    .then_with(|| source_key(a.unique_source).cmp(&source_key(b.unique_source)))
            });
        }
        StorageSortMode::NameAsc => {
            items.sort_by(|a, b| {
                let na = registry.get(a.item_id).map(|d| d.display_name).unwrap_or("");
                let nb = registry.get(b.item_id).map(|d| d.display_name).unwrap_or("");
                na.cmp(nb)
                    .then_with(|| a.item_id.0.cmp(&b.item_id.0))
                    .then_with(|| a.durability.cmp(&b.durability))
                    .then_with(|| source_key(a.unique_source).cmp(&source_key(b.unique_source)))
            });
        }
        StorageSortMode::ItemIdAsc => {
            items.sort_by(|a, b| {
                a.item_id
                    .0
                    .cmp(&b.item_id.0)
                    .then_with(|| a.durability.cmp(&b.durability))
                    .then_with(|| source_key(a.unique_source).cmp(&source_key(b.unique_source)))
            });
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
    let is_unique = max_stack <= 1
        || slot.durability > 0
        || BackpackState::tier_for_item(slot.item_id) > 0;

    // Pass 1: merge into existing partial stacks of the same item (only for stackable non-unique items)
    if !is_unique {
        for &cpos in chests {
            let chest = world.get_or_create_chest(cpos);
            for s in chest.iter_mut() {
                if !s.is_empty()
                    && s.item_id == slot.item_id
                    && s.durability == 0
                    && s.count < max_stack
                {
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
                    enhancement: slot.enhancement,
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

/// Withdraws a specific unique / non-stackable item from its exact chest slot so its unique ID
/// and contents are preserved without duplication.
pub fn withdraw_unique_from_network(
    world: &mut World,
    chest_pos: IVec3,
    slot_idx: usize,
) -> ItemSlot {
    let Some(chest) = world.get_chest_mut(chest_pos) else {
        return ItemSlot::EMPTY;
    };
    if slot_idx >= chest.len() || chest[slot_idx].is_empty() {
        return ItemSlot::EMPTY;
    }
    std::mem::replace(&mut chest[slot_idx], ItemSlot::EMPTY)
}

/// Withdraws up to `max_count` of stackable `item_id` from `chests`.
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
    let mut taken_enhancement = None;

    for &cpos in chests {
        let Some(chest) = world.get_chest_mut(cpos) else {
            continue;
        };
        for s in chest.iter_mut() {
            if !s.is_empty() && s.item_id == item_id {
                if taken > 0 && s.enhancement != taken_enhancement {
                    continue;
                }
                let need = max_count - taken;
                let pull = s.count.min(need);
                if taken == 0 {
                    taken_durability = s.durability;
                    taken_enhancement = s.enhancement;
                }
                s.count -= pull;
                taken += pull;
                if s.count == 0 {
                    *s = ItemSlot::EMPTY;
                }
                if taken >= max_count || taken_durability > 0 || taken_enhancement.is_some() {
                    return ItemSlot {
                        item_id,
                        count: taken,
                        durability: taken_durability,
                        enhancement: taken_enhancement,
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
            enhancement: taken_enhancement,
        }
    } else {
        ItemSlot::EMPTY
    }
}

/// Single Backpack container instance (Tier 1: 18 slots, Tier 2: 27 slots, Tier 3: 36 slots).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BackpackState {
    /// 0 = uninitialized, 1 = Leather (18), 2 = Iron (27), 3 = Diamond (36).
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

    pub fn is_backpack_item(item_id: ItemId) -> bool {
        Self::tier_for_item(item_id) > 0
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

    pub fn title_with_id(&self, id: u16) -> String {
        match self.tier {
            1 => format!("Leather Backpack #{} (18 Slots)", id),
            2 => format!("Iron Backpack #{} (27 Slots)", id),
            3 => format!("Diamond Backpack #{} (36 Slots)", id),
            _ => format!("Backpack #{}", id),
        }
    }

    /// Ensures the backpack has at least `tier` capacity while preserving all stored items.
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

/// Stores all unique Backpack inventories in the world keyed by their unique `backpack_id: u16`
/// (which is carried on each backpack `ItemSlot::durability`).
#[derive(Debug, Clone, PartialEq)]
pub struct BackpackRegistry {
    pub next_id: u16,
    pub bags: HashMap<u16, BackpackState>,
}

impl Default for BackpackRegistry {
    fn default() -> Self {
        Self {
            next_id: 1,
            bags: HashMap::new(),
        }
    }
}

impl BackpackRegistry {
    /// Allocates a fresh unique backpack ID (>= 1) and creates an empty bag for `tier`.
    pub fn allocate_id(&mut self, tier: u8) -> u16 {
        let mut id = self.next_id.max(1);
        while self.bags.contains_key(&id) {
            id = id.wrapping_add(1).max(1);
        }
        self.next_id = id.wrapping_add(1).max(1);
        let mut bag = BackpackState::default();
        bag.ensure_tier(tier.max(1));
        self.bags.insert(id, bag);
        id
    }

    /// Ensures a backpack with `id` exists and has at least `tier` capacity.
    pub fn ensure_bag(&mut self, id: u16, tier: u8) -> &mut BackpackState {
        let bag = self.bags.entry(id).or_default();
        bag.ensure_tier(tier.max(1));
        if id >= self.next_id {
            self.next_id = id.saturating_add(1).max(1);
        }
        bag
    }

    /// Assigns a unique ID to a backpack `ItemSlot` if it does not have one yet (`durability == 0`),
    /// and ensures its backing storage exists. Returns the backpack's unique ID (or `None` if not a backpack).
    pub fn ensure_slot_id(&mut self, slot: &mut ItemSlot) -> Option<u16> {
        let tier = BackpackState::tier_for_item(slot.item_id);
        if tier == 0 {
            return None;
        }
        slot.count = 1;
        if slot.durability == 0 {
            slot.durability = self.allocate_id(tier);
        } else {
            self.ensure_bag(slot.durability, tier);
        }
        Some(slot.durability)
    }

    pub fn get(&self, id: u16) -> Option<&BackpackState> {
        self.bags.get(&id)
    }

    pub fn get_mut(&mut self, id: u16) -> Option<&mut BackpackState> {
        self.bags.get_mut(&id)
    }
}
