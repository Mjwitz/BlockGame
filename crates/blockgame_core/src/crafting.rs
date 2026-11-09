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

        // 8. Iron Pickaxe: 3 Iron Ingots top row, 2 Sticks center column (Shaped 3x3)
        self.register(Recipe::Shaped {
            name: "Iron Pickaxe",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::IRON_INGOT), Some(ItemId::IRON_INGOT), Some(ItemId::IRON_INGOT),
                None,                    Some(ItemId::STICK),      None,
                None,                    Some(ItemId::STICK),      None,
            ],
            output: ItemSlot::new(ItemId::IRON_PICKAXE, 1),
        });

        // 9. Diamond Pickaxe: 3 Diamonds top row, 2 Sticks center column (Shaped 3x3)
        self.register(Recipe::Shaped {
            name: "Diamond Pickaxe",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::DIAMOND), Some(ItemId::DIAMOND), Some(ItemId::DIAMOND),
                None,                  Some(ItemId::STICK),   None,
                None,                  Some(ItemId::STICK),   None,
            ],
            output: ItemSlot::new(ItemId::DIAMOND_PICKAXE, 1),
        });

        // Axes (2x3 trimmed pattern, both left and right orientations)
        let register_axe = |cm: &mut CraftingManager, name_left: &'static str, name_right: &'static str, mat: ItemId, out: ItemId| {
            // Left orientation
            cm.register(Recipe::Shaped {
                name: name_left,
                width: 2,
                height: 3,
                pattern: vec![
                    Some(mat),  Some(mat),
                    Some(mat),  Some(ItemId::STICK),
                    None,       Some(ItemId::STICK),
                ],
                output: ItemSlot::new(out, 1),
            });
            // Right orientation
            cm.register(Recipe::Shaped {
                name: name_right,
                width: 2,
                height: 3,
                pattern: vec![
                    Some(mat),           Some(mat),
                    Some(ItemId::STICK), Some(mat),
                    Some(ItemId::STICK), None,
                ],
                output: ItemSlot::new(out, 1),
            });
        };

        register_axe(self, "Wooden Axe (Left)", "Wooden Axe (Right)", ItemId::PLANKS, ItemId::WOODEN_AXE);
        register_axe(self, "Stone Axe (Left)", "Stone Axe (Right)", ItemId::COBBLESTONE, ItemId::STONE_AXE);
        register_axe(self, "Iron Axe (Left)", "Iron Axe (Right)", ItemId::IRON_INGOT, ItemId::IRON_AXE);
        register_axe(self, "Diamond Axe (Left)", "Diamond Axe (Right)", ItemId::DIAMOND, ItemId::DIAMOND_AXE);

        // Shovels (1x3 trimmed pattern)
        let register_shovel = |cm: &mut CraftingManager, name: &'static str, mat: ItemId, out: ItemId| {
            cm.register(Recipe::Shaped {
                name,
                width: 1,
                height: 3,
                pattern: vec![
                    Some(mat),
                    Some(ItemId::STICK),
                    Some(ItemId::STICK),
                ],
                output: ItemSlot::new(out, 1),
            });
        };

        register_shovel(self, "Wooden Shovel", ItemId::PLANKS, ItemId::WOODEN_SHOVEL);
        register_shovel(self, "Stone Shovel", ItemId::COBBLESTONE, ItemId::STONE_SHOVEL);
        register_shovel(self, "Iron Shovel", ItemId::IRON_INGOT, ItemId::IRON_SHOVEL);
        register_shovel(self, "Diamond Shovel", ItemId::DIAMOND, ItemId::DIAMOND_SHOVEL);

        // Smelting Substitute Recipes (1 Raw Ore -> 1 Ingot)
        self.register(Recipe::Shapeless {
            name: "Iron Ingot from Raw Iron",
            inputs: vec![ItemId::RAW_IRON],
            output: ItemSlot::new(ItemId::IRON_INGOT, 1),
        });

        self.register(Recipe::Shapeless {
            name: "Copper Ingot from Raw Copper",
            inputs: vec![ItemId::RAW_COPPER],
            output: ItemSlot::new(ItemId::COPPER_INGOT, 1),
        });

        self.register(Recipe::Shapeless {
            name: "Gold Ingot from Raw Gold",
            inputs: vec![ItemId::RAW_GOLD],
            output: ItemSlot::new(ItemId::GOLD_INGOT, 1),
        });

        // Hoes (2x3 trimmed pattern, both left and right orientations)
        let register_hoe = |cm: &mut CraftingManager, name_left: &'static str, name_right: &'static str, mat: ItemId, out: ItemId| {
            // Left orientation
            cm.register(Recipe::Shaped {
                name: name_left,
                width: 2,
                height: 3,
                pattern: vec![
                    Some(mat),  Some(mat),
                    None,       Some(ItemId::STICK),
                    None,       Some(ItemId::STICK),
                ],
                output: ItemSlot::new(out, 1),
            });
            // Right orientation
            cm.register(Recipe::Shaped {
                name: name_right,
                width: 2,
                height: 3,
                pattern: vec![
                    Some(mat),           Some(mat),
                    Some(ItemId::STICK), None,
                    Some(ItemId::STICK), None,
                ],
                output: ItemSlot::new(out, 1),
            });
        };

        register_hoe(self, "Wooden Hoe (Left)", "Wooden Hoe (Right)", ItemId::PLANKS, ItemId::WOODEN_HOE);
        register_hoe(self, "Stone Hoe (Left)", "Stone Hoe (Right)", ItemId::COBBLESTONE, ItemId::STONE_HOE);
        register_hoe(self, "Iron Hoe (Left)", "Iron Hoe (Right)", ItemId::IRON_INGOT, ItemId::IRON_HOE);
        register_hoe(self, "Diamond Hoe (Left)", "Diamond Hoe (Right)", ItemId::DIAMOND, ItemId::DIAMOND_HOE);

        // Bread: 3 Wheat in a horizontal row (3x1)
        self.register(Recipe::Shaped {
            name: "Bread",
            width: 3,
            height: 1,
            pattern: vec![Some(ItemId::WHEAT), Some(ItemId::WHEAT), Some(ItemId::WHEAT)],
            output: ItemSlot::new(ItemId::BREAD, 1),
        });

        // Cooking Meats (1 Raw Meat -> 1 Cooked Meat)
        self.register(Recipe::Shapeless {
            name: "Cooked Steak from Raw Beef",
            inputs: vec![ItemId::RAW_BEEF],
            output: ItemSlot::new(ItemId::COOKED_STEAK, 1),
        });

        self.register(Recipe::Shapeless {
            name: "Cooked Porkchop from Raw Porkchop",
            inputs: vec![ItemId::RAW_PORKCHOP],
            output: ItemSlot::new(ItemId::COOKED_PORKCHOP, 1),
        });

        self.register(Recipe::Shapeless {
            name: "Cooked Chicken from Raw Chicken",
            inputs: vec![ItemId::RAW_CHICKEN],
            output: ItemSlot::new(ItemId::COOKED_CHICKEN, 1),
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

    #[test]
    fn test_shaped_recipe_iron_pickaxe_in_3x3() {
        let cm = CraftingManager::new();
        let mut grid = CraftingGrid::new_3x3();

        // Top row: 3 Iron Ingots
        grid.set(0, 0, ItemSlot::new(ItemId::IRON_INGOT, 1));
        grid.set(1, 0, ItemSlot::new(ItemId::IRON_INGOT, 1));
        grid.set(2, 0, ItemSlot::new(ItemId::IRON_INGOT, 1));
        // Center column: 2 Sticks
        grid.set(1, 1, ItemSlot::new(ItemId::STICK, 1));
        grid.set(1, 2, ItemSlot::new(ItemId::STICK, 1));

        let res = cm.find_match(&grid);
        assert!(res.is_some(), "3x3 Iron Pickaxe recipe must match");
        let (_, out) = res.unwrap();
        assert_eq!(out.item_id, ItemId::IRON_PICKAXE);
        assert_eq!(out.count, 1);

        let crafted = cm.craft(&mut grid).unwrap();
        assert_eq!(crafted.item_id, ItemId::IRON_PICKAXE);
        assert!(grid.is_empty());
    }

    #[test]
    fn test_shapeless_recipe_raw_ore_smelting() {
        let cm = CraftingManager::new();
        let mut grid = CraftingGrid::new_2x2();

        grid.set(1, 1, ItemSlot::new(ItemId::RAW_IRON, 3));
        let res = cm.find_match(&grid);
        assert!(res.is_some());
        let (_, out) = res.unwrap();
        assert_eq!(out.item_id, ItemId::IRON_INGOT);

        let crafted = cm.craft(&mut grid).unwrap();
        assert_eq!(crafted.item_id, ItemId::IRON_INGOT);
        assert_eq!(grid.get(1, 1).count, 2);
    }

    #[test]
    fn test_bread_crafting() {
        let cm = CraftingManager::new();
        let mut grid = CraftingGrid::new_3x3();

        // 3 Wheat horizontally
        grid.set(0, 1, ItemSlot::new(ItemId::WHEAT, 2));
        grid.set(1, 1, ItemSlot::new(ItemId::WHEAT, 2));
        grid.set(2, 1, ItemSlot::new(ItemId::WHEAT, 2));

        let res = cm.find_match(&grid);
        assert!(res.is_some());
        let (_, out) = res.unwrap();
        assert_eq!(out.item_id, ItemId::BREAD);
        assert_eq!(out.count, 1);
    }

    #[test]
    fn test_diamond_hoe_crafting() {
        let cm = CraftingManager::new();
        let mut grid = CraftingGrid::new_3x3();

        // Left diamond hoe:
        // [Diamond, Diamond]
        // [None, Stick]
        // [None, Stick]
        grid.set(0, 0, ItemSlot::new(ItemId::DIAMOND, 1));
        grid.set(1, 0, ItemSlot::new(ItemId::DIAMOND, 1));
        grid.set(1, 1, ItemSlot::new(ItemId::STICK, 1));
        grid.set(1, 2, ItemSlot::new(ItemId::STICK, 1));

        let res = cm.find_match(&grid);
        assert!(res.is_some());
        let (_, out) = res.unwrap();
        assert_eq!(out.item_id, ItemId::DIAMOND_HOE);
        assert_eq!(out.count, 1);
    }
}

