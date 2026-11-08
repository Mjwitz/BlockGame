use std::collections::HashMap;

use crate::block::BlockId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub u16);

impl ItemId {
    pub const EMPTY: Self = Self(0);
    pub const STONE: Self = Self(1);
    pub const DIRT: Self = Self(2);
    pub const GRASS: Self = Self(3);
    pub const COBBLESTONE: Self = Self(4);
    pub const WOOD_LOG: Self = Self(5);
    pub const LEAVES: Self = Self(6);
    pub const SAND: Self = Self(7);
    pub const PLANKS: Self = Self(8);
    pub const GLASS: Self = Self(9);
    pub const TORCH: Self = Self(10);
    pub const STICK: Self = Self(11);
    pub const WOODEN_PICKAXE: Self = Self(12);
    pub const STONE_PICKAXE: Self = Self(13);
    pub const CRAFTING_TABLE: Self = Self(14);
    pub const COAL: Self = Self(15);
    pub const IRON_INGOT: Self = Self(16);
    pub const IRON_PICKAXE: Self = Self(17);

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolTier {
    Wood,
    Stone,
    Iron,
    Diamond,
}

#[derive(Debug, Clone)]
pub struct ItemDef {
    pub id: ItemId,
    pub name: &'static str,
    pub display_name: &'static str,
    pub max_stack: u16,
    pub placeable_block: Option<BlockId>,
    pub durability: Option<u16>,
    pub tool_tier: Option<ToolTier>,
    pub icon_tile: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSlot {
    pub item_id: ItemId,
    pub count: u16,
    pub durability: u16,
}

impl Default for ItemSlot {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl ItemSlot {
    pub const EMPTY: Self = Self {
        item_id: ItemId::EMPTY,
        count: 0,
        durability: 0,
    };

    pub const fn new(item_id: ItemId, count: u16) -> Self {
        Self {
            item_id,
            count,
            durability: 0,
        }
    }

    pub const fn with_durability(item_id: ItemId, count: u16, durability: u16) -> Self {
        Self {
            item_id,
            count,
            durability,
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.item_id.is_empty() || self.count == 0
    }

    #[inline]
    pub fn can_stack_with(&self, other: &Self) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        self.item_id == other.item_id && self.durability == other.durability
    }

    /// Takes up to `amount` items from this slot and returns a new slot with them.
    pub fn take(&mut self, amount: u16) -> ItemSlot {
        if self.is_empty() {
            return ItemSlot::EMPTY;
        }
        let take_amount = amount.min(self.count);
        self.count -= take_amount;
        let taken = ItemSlot {
            item_id: self.item_id,
            count: take_amount,
            durability: self.durability,
        };
        if self.count == 0 {
            *self = ItemSlot::EMPTY;
        }
        taken
    }

    /// Splits the slot into half (rounded up for taken half).
    pub fn split_half(&mut self) -> ItemSlot {
        if self.is_empty() {
            return ItemSlot::EMPTY;
        }
        let half = (self.count + 1) / 2;
        self.take(half)
    }
}

pub struct ItemRegistry {
    items: Vec<Option<ItemDef>>,
    by_name: HashMap<String, ItemId>,
    block_to_item: HashMap<BlockId, ItemId>,
}

impl Default for ItemRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            items: Vec::new(),
            by_name: HashMap::new(),
            block_to_item: HashMap::new(),
        };
        reg.register_default_items();
        reg
    }

    pub fn register(&mut self, def: ItemDef) {
        let idx = def.id.0 as usize;
        if idx >= self.items.len() {
            self.items.resize(idx + 1, None);
        }
        self.by_name.insert(def.name.to_string(), def.id);
        if let Some(block_id) = def.placeable_block {
            self.block_to_item.insert(block_id, def.id);
        }
        self.items[idx] = Some(def);
    }

    pub fn get(&self, id: ItemId) -> Option<&ItemDef> {
        self.items.get(id.0 as usize).and_then(|x| x.as_ref())
    }

    pub fn get_by_name(&self, name: &str) -> Option<&ItemDef> {
        self.by_name.get(name).and_then(|&id| self.get(id))
    }

    pub fn item_for_block(&self, block: BlockId) -> Option<ItemId> {
        self.block_to_item.get(&block).copied()
    }

    pub fn max_stack_size(&self, id: ItemId) -> u16 {
        self.get(id).map(|d| d.max_stack).unwrap_or(64)
    }

    fn register_default_items(&mut self) {
        // Blocks as Placeable Items
        self.register(ItemDef {
            id: ItemId::STONE,
            name: "blockgame:stone",
            display_name: "Stone",
            max_stack: 64,
            placeable_block: Some(BlockId::STONE),
            durability: None,
            tool_tier: None,
            icon_tile: 1,
        });

        self.register(ItemDef {
            id: ItemId::DIRT,
            name: "blockgame:dirt",
            display_name: "Dirt",
            max_stack: 64,
            placeable_block: Some(BlockId::DIRT),
            durability: None,
            tool_tier: None,
            icon_tile: 2,
        });

        self.register(ItemDef {
            id: ItemId::GRASS,
            name: "blockgame:grass_block",
            display_name: "Grass Block",
            max_stack: 64,
            placeable_block: Some(BlockId::GRASS),
            durability: None,
            tool_tier: None,
            icon_tile: 3,
        });

        self.register(ItemDef {
            id: ItemId::COBBLESTONE,
            name: "blockgame:cobblestone",
            display_name: "Cobblestone",
            max_stack: 64,
            placeable_block: Some(BlockId::COBBLESTONE),
            durability: None,
            tool_tier: None,
            icon_tile: 5,
        });

        self.register(ItemDef {
            id: ItemId::WOOD_LOG,
            name: "blockgame:oak_log",
            display_name: "Oak Log",
            max_stack: 64,
            placeable_block: Some(BlockId::WOOD_LOG),
            durability: None,
            tool_tier: None,
            icon_tile: 6,
        });

        self.register(ItemDef {
            id: ItemId::LEAVES,
            name: "blockgame:leaves",
            display_name: "Oak Leaves",
            max_stack: 64,
            placeable_block: Some(BlockId::LEAVES),
            durability: None,
            tool_tier: None,
            icon_tile: 7,
        });

        self.register(ItemDef {
            id: ItemId::SAND,
            name: "blockgame:sand",
            display_name: "Sand",
            max_stack: 64,
            placeable_block: Some(BlockId::SAND),
            durability: None,
            tool_tier: None,
            icon_tile: 8,
        });

        self.register(ItemDef {
            id: ItemId::PLANKS,
            name: "blockgame:planks",
            display_name: "Oak Planks",
            max_stack: 64,
            placeable_block: Some(BlockId::PLANKS),
            durability: None,
            tool_tier: None,
            icon_tile: 9,
        });

        self.register(ItemDef {
            id: ItemId::GLASS,
            name: "blockgame:glass",
            display_name: "Glass",
            max_stack: 64,
            placeable_block: Some(BlockId::GLASS),
            durability: None,
            tool_tier: None,
            icon_tile: 10,
        });

        self.register(ItemDef {
            id: ItemId::TORCH,
            name: "blockgame:torch",
            display_name: "Torch",
            max_stack: 64,
            placeable_block: Some(BlockId::TORCH),
            durability: None,
            tool_tier: None,
            icon_tile: 13,
        });

        // Crafting & Resource Items
        self.register(ItemDef {
            id: ItemId::STICK,
            name: "blockgame:stick",
            display_name: "Stick",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_tier: None,
            icon_tile: 14,
        });

        self.register(ItemDef {
            id: ItemId::WOODEN_PICKAXE,
            name: "blockgame:wooden_pickaxe",
            display_name: "Wooden Pickaxe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(59),
            tool_tier: Some(ToolTier::Wood),
            icon_tile: 15,
        });

        self.register(ItemDef {
            id: ItemId::STONE_PICKAXE,
            name: "blockgame:stone_pickaxe",
            display_name: "Stone Pickaxe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(131),
            tool_tier: Some(ToolTier::Stone),
            icon_tile: 16,
        });

        self.register(ItemDef {
            id: ItemId::CRAFTING_TABLE,
            name: "blockgame:crafting_table",
            display_name: "Crafting Table",
            max_stack: 64,
            placeable_block: Some(BlockId::PLANKS), // placeable as decorative table
            durability: None,
            tool_tier: None,
            icon_tile: 17,
        });

        self.register(ItemDef {
            id: ItemId::COAL,
            name: "blockgame:coal",
            display_name: "Coal",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_tier: None,
            icon_tile: 18,
        });

        self.register(ItemDef {
            id: ItemId::IRON_INGOT,
            name: "blockgame:iron_ingot",
            display_name: "Iron Ingot",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_tier: None,
            icon_tile: 19,
        });

        self.register(ItemDef {
            id: ItemId::IRON_PICKAXE,
            name: "blockgame:iron_pickaxe",
            display_name: "Iron Pickaxe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(250),
            tool_tier: Some(ToolTier::Iron),
            icon_tile: 20,
        });
    }
}

/// Standard Minecraft inventory: 9 hotbar slots + 27 backpack slots = 36 slots total.
#[derive(Debug, Clone)]
pub struct Inventory {
    pub slots: Vec<ItemSlot>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new(36)
    }
}

impl Inventory {
    pub const HOTBAR_COUNT: usize = 9;
    pub const TOTAL_COUNT: usize = 36;

    pub fn new(size: usize) -> Self {
        Self {
            slots: vec![ItemSlot::EMPTY; size],
        }
    }

    /// Pre-populates a starter survival hotbar.
    pub fn starter_inventory() -> Self {
        let mut inv = Self::new(Self::TOTAL_COUNT);
        inv.slots[0] = ItemSlot::new(ItemId::STONE_PICKAXE, 1);
        inv.slots[1] = ItemSlot::new(ItemId::WOOD_LOG, 16);
        inv.slots[2] = ItemSlot::new(ItemId::COBBLESTONE, 32);
        inv.slots[3] = ItemSlot::new(ItemId::TORCH, 16);
        inv.slots[4] = ItemSlot::new(ItemId::DIRT, 64);
        inv.slots[5] = ItemSlot::new(ItemId::PLANKS, 32);
        inv.slots[6] = ItemSlot::new(ItemId::STICK, 8);
        inv.slots[7] = ItemSlot::new(ItemId::GLASS, 16);
        inv.slots[8] = ItemSlot::new(ItemId::CRAFTING_TABLE, 1);
        inv
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_empty())
    }

    #[inline]
    pub fn get(&self, idx: usize) -> ItemSlot {
        self.slots.get(idx).copied().unwrap_or(ItemSlot::EMPTY)
    }

    #[inline]
    pub fn set(&mut self, idx: usize, slot: ItemSlot) {
        if idx < self.slots.len() {
            self.slots[idx] = slot;
        }
    }

    /// Attempts to add items into inventory. Fills existing stacks first, then empty slots.
    /// Returns any remaining items that could not fit.
    pub fn add_item(&mut self, mut slot: ItemSlot, registry: &ItemRegistry) -> Option<ItemSlot> {
        if slot.is_empty() {
            return None;
        }

        let max_stack = registry.max_stack_size(slot.item_id);

        // 1. Fill existing matching stacks
        for s in self.slots.iter_mut() {
            if s.can_stack_with(&slot) && s.count < max_stack {
                let space = max_stack - s.count;
                let add_amt = space.min(slot.count);
                s.count += add_amt;
                slot.count -= add_amt;
                if slot.count == 0 {
                    return None;
                }
            }
        }

        // 2. Place into first empty slots
        for s in self.slots.iter_mut() {
            if s.is_empty() {
                let add_amt = max_stack.min(slot.count);
                *s = ItemSlot {
                    item_id: slot.item_id,
                    count: add_amt,
                    durability: slot.durability,
                };
                slot.count -= add_amt;
                if slot.count == 0 {
                    return None;
                }
            }
        }

        Some(slot)
    }

    /// Swaps the contents of slot `a` and slot `b`.
    pub fn swap(&mut self, a: usize, b: usize) {
        if a < self.slots.len() && b < self.slots.len() && a != b {
            self.slots.swap(a, b);
        }
    }

    /// Transfers items between two slots, stacking if identical or swapping if different.
    pub fn click_slot(
        &mut self,
        slot_idx: usize,
        held_cursor: &mut ItemSlot,
        split_half: bool,
        registry: &ItemRegistry,
    ) {
        if slot_idx >= self.slots.len() {
            return;
        }

        let slot = &mut self.slots[slot_idx];

        if held_cursor.is_empty() {
            // Picking up from slot
            if !slot.is_empty() {
                if split_half {
                    *held_cursor = slot.split_half();
                } else {
                    *held_cursor = *slot;
                    *slot = ItemSlot::EMPTY;
                }
            }
        } else {
            // Placing held item into slot
            if slot.is_empty() {
                if split_half {
                    // Place 1 single item
                    *slot = held_cursor.take(1);
                } else {
                    // Place entire held stack
                    let max_stack = registry.max_stack_size(held_cursor.item_id);
                    let transfer = max_stack.min(held_cursor.count);
                    *slot = held_cursor.take(transfer);
                }
            } else if slot.can_stack_with(held_cursor) {
                let max_stack = registry.max_stack_size(slot.item_id);
                if slot.count < max_stack {
                    if split_half {
                        // Place 1 item into stack
                        slot.count += 1;
                        held_cursor.take(1);
                    } else {
                        // Fill stack
                        let space = max_stack - slot.count;
                        let transfer = space.min(held_cursor.count);
                        slot.count += transfer;
                        held_cursor.take(transfer);
                    }
                }
            } else {
                // Different items: swap held with slot
                if !split_half {
                    std::mem::swap(slot, held_cursor);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_slot_stacking_and_taking() {
        let mut slot = ItemSlot::new(ItemId::COBBLESTONE, 10);
        assert_eq!(slot.count, 10);

        let taken = slot.take(4);
        assert_eq!(taken.count, 4);
        assert_eq!(slot.count, 6);

        let half = slot.split_half();
        assert_eq!(half.count, 3);
        assert_eq!(slot.count, 3);
    }

    #[test]
    fn test_inventory_add_and_stack() {
        let registry = ItemRegistry::new();
        let mut inv = Inventory::new(9);

        // Add 40 logs
        let overflow = inv.add_item(ItemSlot::new(ItemId::WOOD_LOG, 40), &registry);
        assert!(overflow.is_none());
        assert_eq!(inv.get(0).count, 40);

        // Add 30 more logs (should top up slot 0 to 64 and place 6 in slot 1)
        let overflow = inv.add_item(ItemSlot::new(ItemId::WOOD_LOG, 30), &registry);
        assert!(overflow.is_none());
        assert_eq!(inv.get(0).count, 64);
        assert_eq!(inv.get(1).count, 6);
    }
}
