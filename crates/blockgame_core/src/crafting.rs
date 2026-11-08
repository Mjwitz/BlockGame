use crate::item::{ItemId, ItemSlot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftingGrid {
    pub width: usize,
    pub height: usize,
    pub slots: Vec<ItemSlot>,
}

impl CraftingGrid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            slots: vec![ItemSlot::EMPTY; width * height],
        }
    }

    pub fn new_2x2() -> Self {
        Self::new(2, 2)
    }

    pub fn new_3x3() -> Self {
        Self::new(3, 3)
    }

    #[inline]
    pub fn index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    pub fn get(&self, x: usize, y: usize) -> ItemSlot {
        if x < self.width && y < self.height {
            self.slots[self.index(x, y)]
        } else {
            ItemSlot::EMPTY
        }
    }

    pub fn set(&mut self, x: usize, y: usize, slot: ItemSlot) {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            self.slots[idx] = slot;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_empty())
    }

    pub fn clear(&mut self) {
        for s in self.slots.iter_mut() {
            *s = ItemSlot::EMPTY;
        }
    }

    /// Computes the trimmed bounding box `(min_x, min_y, trimmed_w, trimmed_h)`
    /// containing all non-empty items in the grid.
    pub fn bounding_box(&self) -> Option<(usize, usize, usize, usize)> {
        let mut min_x = self.width;
        let mut max_x = 0;
        let mut min_y = self.height;
        let mut max_y = 0;
        let mut found = false;

        for y in 0..self.height {
            for x in 0..self.width {
                if !self.get(x, y).is_empty() {
                    found = true;
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }

        if !found {
            None
        } else {
            Some((min_x, min_y, max_x - min_x + 1, max_y - min_y + 1))
        }
    }
}

#[derive(Debug, Clone)]
pub enum Recipe {
    Shapeless {
        name: &'static str,
        inputs: Vec<ItemId>,
        output: ItemSlot,
    },
    Shaped {
        name: &'static str,
        width: usize,
        height: usize,
        pattern: Vec<Option<ItemId>>, // size = width * height
        output: ItemSlot,
    },
}

impl Recipe {
    pub fn output(&self) -> ItemSlot {
        match self {
            Recipe::Shapeless { output, .. } => *output,
            Recipe::Shaped { output, .. } => *output,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Recipe::Shapeless { name, .. } => name,
            Recipe::Shaped { name, .. } => name,
        }
    }

    /// Checks if this recipe matches the items in `grid`.
    pub fn matches(&self, grid: &CraftingGrid) -> bool {
        match self {
            Recipe::Shapeless { inputs, .. } => {
                let mut grid_items = Vec::new();
                for slot in &grid.slots {
                    if !slot.is_empty() {
                        grid_items.push(slot.item_id);
                    }
                }
                if grid_items.len() != inputs.len() {
                    return false;
                }
                grid_items.sort();
                let mut sorted_inputs = inputs.clone();
                sorted_inputs.sort();
                grid_items == sorted_inputs
            }
            Recipe::Shaped {
                width,
                height,
                pattern,
                ..
            } => {
                let (min_x, min_y, trim_w, trim_h) = match grid.bounding_box() {
                    Some(bb) => bb,
                    None => return false,
                };

                // The trimmed bounding box dimensions must match the recipe dimensions exactly
                if trim_w != *width || trim_h != *height {
                    return false;
                }

                // Check each cell in the trimmed subgrid
                for dy in 0..*height {
                    for dx in 0..*width {
                        let pat_item = pattern[dy * width + dx];
                        let grid_item = grid.get(min_x + dx, min_y + dy).item_id;

                        match pat_item {
                            Some(expected) => {
                                if grid_item != expected {
                                    return false;
                                }
                            }
                            None => {
                                if !grid_item.is_empty() {
                                    return false;
                                }
                            }
                        }
                    }
                }

                true
            }
        }
    }
}

pub struct CraftingManager {
    recipes: Vec<Recipe>,
}

impl Default for CraftingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CraftingManager {
    pub fn new() -> Self {
        let mut cm = Self {
            recipes: Vec::new(),
        };
        cm.register_default_recipes();
        cm
    }

    pub fn register(&mut self, recipe: Recipe) {
        self.recipes.push(recipe);
    }

    /// Finds a matching recipe for the given grid and returns its output.
    pub fn find_match(&self, grid: &CraftingGrid) -> Option<(&Recipe, ItemSlot)> {
        for recipe in &self.recipes {
            if recipe.matches(grid) {
                return Some((recipe, recipe.output()));
            }
        }
        None
    }

    /// Consumes 1 item from each participating ingredient slot in the grid and returns the output stack.
    pub fn craft(&self, grid: &mut CraftingGrid) -> Option<ItemSlot> {
        let (recipe, output) = self.find_match(grid)?;

        match recipe {
            Recipe::Shapeless { .. } => {
                for slot in grid.slots.iter_mut() {
                    if !slot.is_empty() {
                        slot.take(1);
                    }
                }
            }
            Recipe::Shaped { width, height, pattern, .. } => {
                let (min_x, min_y, _, _) = grid.bounding_box()?;
                for dy in 0..*height {
                    for dx in 0..*width {
                        if pattern[dy * width + dx].is_some() {
                            let idx = grid.index(min_x + dx, min_y + dy);
                            grid.slots[idx].take(1);
                        }
                    }
                }
            }
        }

        Some(output)
    }

    fn register_default_recipes(&mut self) {
        // 1. 1 Wood Log -> 4 Planks (Shapeless)
        self.register(Recipe::Shapeless {
            name: "Planks from Wood Log",
            inputs: vec![ItemId::WOOD_LOG],
            output: ItemSlot::new(ItemId::PLANKS, 4),
        });

        // 2. 2 Planks vertically -> 4 Sticks (Shaped 1x2)
        self.register(Recipe::Shaped {
            name: "Sticks from Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::PLANKS), Some(ItemId::PLANKS)],
            output: ItemSlot::new(ItemId::STICK, 4),
        });

        // 3. 4 Planks (2x2) -> 1 Crafting Table
        self.register(Recipe::Shaped {
            name: "Crafting Table",
            width: 2,
            height: 2,
            pattern: vec![
                Some(ItemId::PLANKS), Some(ItemId::PLANKS),
                Some(ItemId::PLANKS), Some(ItemId::PLANKS),
            ],
            output: ItemSlot::new(ItemId::CRAFTING_TABLE, 1),
        });

        // 4. 1 Coal over 1 Stick -> 4 Torches (Shaped 1x2)
        self.register(Recipe::Shaped {
            name: "Torches from Coal",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::COAL), Some(ItemId::STICK)],
            output: ItemSlot::new(ItemId::TORCH, 4),
        });

        // 5. 1 Planks over 1 Stick -> 4 Torches (Survival convenience recipe)
        self.register(Recipe::Shaped {
            name: "Torches from Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::PLANKS), Some(ItemId::STICK)],
            output: ItemSlot::new(ItemId::TORCH, 4),
        });

        // 6. Wooden Pickaxe: 3 Planks top row, 2 Sticks center column (Shaped 3x3)
        self.register(Recipe::Shaped {
            name: "Wooden Pickaxe",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::PLANKS), Some(ItemId::PLANKS), Some(ItemId::PLANKS),
                None,                 Some(ItemId::STICK),  None,
                None,                 Some(ItemId::STICK),  None,
            ],
            output: ItemSlot::new(ItemId::WOODEN_PICKAXE, 1),
        });

        // 7. Stone Pickaxe: 3 Cobblestone top row, 2 Sticks center column (Shaped 3x3)
        self.register(Recipe::Shaped {
            name: "Stone Pickaxe",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::COBBLESTONE), Some(ItemId::COBBLESTONE), Some(ItemId::COBBLESTONE),
                None,                     Some(ItemId::STICK),       None,
                None,                     Some(ItemId::STICK),       None,
            ],
            output: ItemSlot::new(ItemId::STONE_PICKAXE, 1),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shapeless_recipe_planks() {
        let cm = CraftingManager::new();
        let mut grid = CraftingGrid::new_2x2();

        // Place 1 log in bottom-right corner (1, 1)
        grid.set(1, 1, ItemSlot::new(ItemId::WOOD_LOG, 1));

        let res = cm.find_match(&grid);
        assert!(res.is_some());
        let (_, out) = res.unwrap();
        assert_eq!(out.item_id, ItemId::PLANKS);
        assert_eq!(out.count, 4);

        // Perform crafting
        let crafted = cm.craft(&mut grid).unwrap();
        assert_eq!(crafted.item_id, ItemId::PLANKS);
        assert_eq!(crafted.count, 4);
        assert!(grid.is_empty());
    }

    #[test]
    fn test_shaped_recipe_sticks_offset_invariant() {
        let cm = CraftingManager::new();
        let mut grid = CraftingGrid::new_3x3();

        // Place 2 planks vertically in rightmost column (x = 2, y = 1 and y = 2)
        grid.set(2, 1, ItemSlot::new(ItemId::PLANKS, 5));
        grid.set(2, 2, ItemSlot::new(ItemId::PLANKS, 5));

        let res = cm.find_match(&grid);
        assert!(res.is_some(), "Shaped recipe should match regardless of corner placement!");
        let (_, out) = res.unwrap();
        assert_eq!(out.item_id, ItemId::STICK);
        assert_eq!(out.count, 4);

        // Crafting should decrement each participating slot by 1
        cm.craft(&mut grid);
        assert_eq!(grid.get(2, 1).count, 4);
        assert_eq!(grid.get(2, 2).count, 4);
    }

    #[test]
    fn test_shaped_recipe_crafting_table() {
        let cm = CraftingManager::new();
        let mut grid = CraftingGrid::new_2x2();

        grid.set(0, 0, ItemSlot::new(ItemId::PLANKS, 1));
        grid.set(1, 0, ItemSlot::new(ItemId::PLANKS, 1));
        grid.set(0, 1, ItemSlot::new(ItemId::PLANKS, 1));
        grid.set(1, 1, ItemSlot::new(ItemId::PLANKS, 1));

        let res = cm.find_match(&grid);
        assert!(res.is_some());
        let (_, out) = res.unwrap();
        assert_eq!(out.item_id, ItemId::CRAFTING_TABLE);
        assert_eq!(out.count, 1);
    }
}
