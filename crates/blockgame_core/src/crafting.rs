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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeCategory {
    Crafting3x3,
    Crafting2x2,
    Smelting,
    WorldGen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSourceInfo {
    pub location: &'static str,
    pub tool_required: &'static str,
    pub notes: &'static str,
}

pub fn get_item_source_info(item_id: ItemId) -> Option<ItemSourceInfo> {
    match item_id {
        ItemId::DIRT => Some(ItemSourceInfo {
            location: "Plains, Forests, and Riverbanks",
            tool_required: "Shovel (or by hand)",
            notes: "Abundant surface soil block.",
        }),
        ItemId::GRASS => Some(ItemSourceInfo {
            location: "Surface of temperate and lush biomes",
            tool_required: "Shovel with Silk Touch",
            notes: "Spreads green grass to adjacent dirt.",
        }),
        ItemId::STONE => Some(ItemSourceInfo {
            location: "Underground, cliffs, and mountain peaks",
            tool_required: "Pickaxe (Silk Touch drops Stone)",
            notes: "Abundant everywhere beneath the soil.",
        }),
        ItemId::COBBLESTONE => Some(ItemSourceInfo {
            location: "Mined from Stone underground",
            tool_required: "Pickaxe (Wooden or better)",
            notes: "Drops when mining stone without Silk Touch.",
        }),
        ItemId::SAND => Some(ItemSourceInfo {
            location: "Deserts, Beaches, and Ocean floor",
            tool_required: "Shovel (or by hand)",
            notes: "Abundant along water bodies and desert dunes.",
        }),
        ItemId::GRAVEL => Some(ItemSourceInfo {
            location: "Beaches, riverbeds, and underground",
            tool_required: "Shovel",
            notes: "Gravity-affected block; may yield Flint.",
        }),
        ItemId::SANDSTONE => Some(ItemSourceInfo {
            location: "Beneath desert sand dunes",
            tool_required: "Pickaxe (Wooden or better)",
            notes: "Natural compacted desert sedimentary rock.",
        }),
        ItemId::RED_SAND => Some(ItemSourceInfo {
            location: "Badlands valleys and red dunes",
            tool_required: "Shovel (or by hand)",
            notes: "Warm crimson sand unique to badlands.",
        }),
        ItemId::TERRACOTTA => Some(ItemSourceInfo {
            location: "Badlands canyon mesas and cliffs",
            tool_required: "Pickaxe (Wooden or better)",
            notes: "Forms colorful stratified mesa cliffs.",
        }),
        ItemId::PODZOL => Some(ItemSourceInfo {
            location: "Floor of Taiga evergreen forests",
            tool_required: "Shovel with Silk Touch",
            notes: "Rich needle-litter soil in taiga biomes.",
        }),
        ItemId::MYCELIUM => Some(ItemSourceInfo {
            location: "Rare Mushroom Fields island biomes",
            tool_required: "Shovel with Silk Touch",
            notes: "Very rare spongy turf; breeds giant mushrooms.",
        }),
        ItemId::SNOW_BLOCK => Some(ItemSourceInfo {
            location: "Snowy Slopes and Frozen Peaks",
            tool_required: "Shovel",
            notes: "Covers high cold mountain summits.",
        }),
        ItemId::ICE => Some(ItemSourceInfo {
            location: "Frozen oceans and high mountain lakes",
            tool_required: "Pickaxe with Silk Touch",
            notes: "Forms over cold surface water.",
        }),
        ItemId::CLAY => Some(ItemSourceInfo {
            location: "Riverbeds, lakes, and shallow coasts",
            tool_required: "Shovel",
            notes: "Found under water; smelt into bricks.",
        }),
        ItemId::LILY_PAD => Some(ItemSourceInfo {
            location: "Surface of swamps and calm rivers",
            tool_required: "Hand (instant harvest)",
            notes: "Floats atop calm river and lake water.",
        }),
        ItemId::CACTUS => Some(ItemSourceInfo {
            location: "Deserts and arid badlands",
            tool_required: "Hand or Weapon (caution: spikes!)",
            notes: "Grows on desert sand; deals contact damage.",
        }),
        ItemId::WOOD_LOG => Some(ItemSourceInfo {
            location: "Oak trees in Plains and Forests",
            tool_required: "Axe (or by hand)",
            notes: "Fell oak tree trunks to harvest.",
        }),
        ItemId::BIRCH_LOG => Some(ItemSourceInfo {
            location: "Birch trees in Birch Forest biomes",
            tool_required: "Axe (or by hand)",
            notes: "Distinctive white-bark forest trees.",
        }),
        ItemId::SPRUCE_LOG => Some(ItemSourceInfo {
            location: "Conifer trees in Taiga biomes",
            tool_required: "Axe (or by hand)",
            notes: "Evergreen conifer trees in cold regions.",
        }),
        ItemId::ACACIA_LOG => Some(ItemSourceInfo {
            location: "Acacia trees in Savanna biomes",
            tool_required: "Axe (or by hand)",
            notes: "Grows in warm arid savanna plains.",
        }),
        ItemId::JUNGLE_LOG => Some(ItemSourceInfo {
            location: "Jungle rainforest canopy trees",
            tool_required: "Axe (or by hand)",
            notes: "Tall towering trees in tropical rainforests.",
        }),
        ItemId::DARK_OAK_LOG => Some(ItemSourceInfo {
            location: "Dark Forest biomes",
            tool_required: "Axe (or by hand)",
            notes: "Thick canopy trees with dense dark wood.",
        }),
        ItemId::LEAVES => Some(ItemSourceInfo {
            location: "Oak tree canopies in Forests",
            tool_required: "Shears (or naturally decays)",
            notes: "Drops Apples and Saplings when cleared.",
        }),
        ItemId::BIRCH_LEAVES => Some(ItemSourceInfo {
            location: "Birch tree canopies",
            tool_required: "Shears (or naturally decays)",
            notes: "Foliage of birch trees.",
        }),
        ItemId::SPRUCE_LEAVES => Some(ItemSourceInfo {
            location: "Evergreen pine needles in Taiga",
            tool_required: "Shears (or naturally decays)",
            notes: "Dense needle foliage of taiga conifers.",
        }),
        ItemId::ACACIA_LEAVES => Some(ItemSourceInfo {
            location: "Flat canopies in Savanna biomes",
            tool_required: "Shears (or naturally decays)",
            notes: "Savanna flat-top canopy leaves.",
        }),
        ItemId::JUNGLE_LEAVES => Some(ItemSourceInfo {
            location: "Jungle rainforest canopies",
            tool_required: "Shears (or naturally decays)",
            notes: "Lush tropical rainforest tree foliage.",
        }),
        ItemId::DARK_OAK_LEAVES => Some(ItemSourceInfo {
            location: "Dark Forest dense roof",
            tool_required: "Shears (or naturally decays)",
            notes: "Dense canopy foliage blocking out the sky.",
        }),
        ItemId::RED_MUSHROOM_BLOCK => Some(ItemSourceInfo {
            location: "Giant red mushrooms in Mushroom Fields",
            tool_required: "Axe (or by hand)",
            notes: "Found on rare mushroom field islands.",
        }),
        ItemId::BROWN_MUSHROOM_BLOCK => Some(ItemSourceInfo {
            location: "Giant brown mushrooms in Mushroom Fields",
            tool_required: "Axe (or by hand)",
            notes: "Found on rare mushroom field islands.",
        }),
        ItemId::MUSHROOM_STEM => Some(ItemSourceInfo {
            location: "Stems of giant mushrooms",
            tool_required: "Axe (or by hand)",
            notes: "Thick fungal stalks in Mushroom Fields.",
        }),
        ItemId::COAL_ORE => Some(ItemSourceInfo {
            location: "Underground rock strata and cliff veins",
            tool_required: "Pickaxe (Wooden or better)",
            notes: "Mined for Coal fuel and torches.",
        }),
        ItemId::COPPER_ORE => Some(ItemSourceInfo {
            location: "Underground mineral deposits (Y: 10-60)",
            tool_required: "Pickaxe (Stone or better)",
            notes: "Drops Raw Copper; smelt into Copper Ingots.",
        }),
        ItemId::IRON_ORE => Some(ItemSourceInfo {
            location: "Underground rock strata (Y: 5-55)",
            tool_required: "Pickaxe (Stone or better)",
            notes: "Drops Raw Iron; smelt into Iron Ingots.",
        }),
        ItemId::GOLD_ORE => Some(ItemSourceInfo {
            location: "Deep underground caves (Y <= 30)",
            tool_required: "Pickaxe (Iron or better)",
            notes: "Drops Raw Gold; smelt into Gold Ingots.",
        }),
        ItemId::DIAMOND_ORE => Some(ItemSourceInfo {
            location: "Deep underground near bedrock (Y <= 16)",
            tool_required: "Pickaxe (Iron or better)",
            notes: "Rare precious mineral, drops Diamonds.",
        }),
        ItemId::RAW_COPPER => Some(ItemSourceInfo {
            location: "Mined from Copper Ore veins",
            tool_required: "Pickaxe (Stone or better)",
            notes: "Smelt in a furnace into Copper Ingots.",
        }),
        ItemId::RAW_IRON => Some(ItemSourceInfo {
            location: "Mined from Iron Ore veins",
            tool_required: "Pickaxe (Stone or better)",
            notes: "Smelt in a furnace into Iron Ingots.",
        }),
        ItemId::RAW_GOLD => Some(ItemSourceInfo {
            location: "Mined from Gold Ore veins",
            tool_required: "Pickaxe (Iron or better)",
            notes: "Smelt in a furnace into Gold Ingots.",
        }),
        ItemId::COAL => Some(ItemSourceInfo {
            location: "Mined from Coal Ore veins",
            tool_required: "Pickaxe (Wooden or better)",
            notes: "Primary furnace fuel and torch ingredient.",
        }),
        ItemId::DIAMOND => Some(ItemSourceInfo {
            location: "Mined from Diamond Ore deep underground",
            tool_required: "Pickaxe (Iron or better)",
            notes: "Top-tier gemstone for armor and tools.",
        }),
        ItemId::TALL_GRASS => Some(ItemSourceInfo {
            location: "Plains and Forest surface floor",
            tool_required: "Shears (drops grass) or Hand",
            notes: "Breaks into Wheat Seeds when harvested.",
        }),
        ItemId::SEAGRASS => Some(ItemSourceInfo {
            location: "Ocean floors and river bottoms",
            tool_required: "Shears (drops seagrass) or Hand",
            notes: "Underwater marine vegetation.",
        }),
        ItemId::BUCKET => Some(ItemSourceInfo {
            location: "Crafted with 3 Iron Ingots in a V-shape",
            tool_required: "Crafting Table",
            notes: "Used to scoop and transport water.",
        }),
        ItemId::WATER_BUCKET => Some(ItemSourceInfo {
            location: "Filled by right-clicking water with an empty Bucket",
            tool_required: "Bucket + Water Source",
            notes: "Places water block and cascades downward.",
        }),
        ItemId::CORAL_BLOCK => Some(ItemSourceInfo {
            location: "Warm coral reef ocean shallows",
            tool_required: "Pickaxe with Silk Touch",
            notes: "Vibrant living underwater coral structures.",
        }),
        ItemId::TUBE_CORAL_BLOCK => Some(ItemSourceInfo {
            location: "Warm coral reef ocean shallows",
            tool_required: "Pickaxe with Silk Touch",
            notes: "Bright blue tube corals in warm shallows.",
        }),
        ItemId::APPLE => Some(ItemSourceInfo {
            location: "Decaying Oak leaves in Forests",
            tool_required: "Hand / Leaf decay",
            notes: "Nutritious fruit dropped when clearing oak trees.",
        }),
        ItemId::WHEAT_SEEDS => Some(ItemSourceInfo {
            location: "Harvesting Tall Grass in plains",
            tool_required: "Hand (break tall grass)",
            notes: "Plant on Farmland near water to grow Wheat.",
        }),
        ItemId::RAW_BEEF => Some(ItemSourceInfo {
            location: "Defeating Cows in Plains & Forests",
            tool_required: "Weapon / Hand",
            notes: "Cook in a furnace into Steak.",
        }),
        ItemId::RAW_PORKCHOP => Some(ItemSourceInfo {
            location: "Defeating Pigs in Plains & Forests",
            tool_required: "Weapon / Hand",
            notes: "Cook in a furnace into Cooked Porkchop.",
        }),
        ItemId::RAW_CHICKEN => Some(ItemSourceInfo {
            location: "Defeating Chickens in Plains",
            tool_required: "Weapon / Hand",
            notes: "Cook in a furnace into Cooked Chicken.",
        }),
        ItemId::LEATHER => Some(ItemSourceInfo {
            location: "Defeating Cows in Plains & Forests",
            tool_required: "Weapon / Hand",
            notes: "Used for crafting books, armor, and gear.",
        }),
        ItemId::FEATHER => Some(ItemSourceInfo {
            location: "Defeating Chickens in Plains",
            tool_required: "Weapon / Hand",
            notes: "Dropped by chickens; used to craft arrows.",
        }),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayRecipe {
    pub name: &'static str,
    pub category: RecipeCategory,
    pub inputs: [Option<ItemId>; 9],
    pub output: ItemSlot,
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
    Smelting {
        name: &'static str,
        input: ItemId,
        output: ItemSlot,
    },
}

impl Recipe {
    pub fn output(&self) -> ItemSlot {
        match self {
            Recipe::Shapeless { output, .. } => *output,
            Recipe::Shaped { output, .. } => *output,
            Recipe::Smelting { output, .. } => *output,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Recipe::Shapeless { name, .. } => name,
            Recipe::Shaped { name, .. } => name,
            Recipe::Smelting { name, .. } => name,
        }
    }

    pub fn to_display_recipe(&self) -> DisplayRecipe {
        match self {
            Recipe::Shapeless {
                name,
                inputs,
                output,
            } => {
                let mut grid = [None; 9];
                for (i, &inp) in inputs.iter().take(9).enumerate() {
                    grid[i] = Some(inp);
                }
                let category = if inputs.len() <= 4 {
                    RecipeCategory::Crafting2x2
                } else {
                    RecipeCategory::Crafting3x3
                };
                DisplayRecipe {
                    name,
                    category,
                    inputs: grid,
                    output: *output,
                }
            }
            Recipe::Shaped {
                name,
                width,
                height,
                pattern,
                output,
            } => {
                let mut grid = [None; 9];
                let w = *width;
                let h = *height;
                for dy in 0..h.min(3) {
                    for dx in 0..w.min(3) {
                        grid[dy * 3 + dx] = pattern[dy * w + dx];
                    }
                }
                let category = if w <= 2 && h <= 2 {
                    RecipeCategory::Crafting2x2
                } else {
                    RecipeCategory::Crafting3x3
                };
                DisplayRecipe {
                    name,
                    category,
                    inputs: grid,
                    output: *output,
                }
            }
            Recipe::Smelting {
                name,
                input,
                output,
            } => {
                let mut grid = [None; 9];
                grid[4] = Some(*input); // Centered in the 3x3 display grid
                DisplayRecipe {
                    name,
                    category: RecipeCategory::Smelting,
                    inputs: grid,
                    output: *output,
                }
            }
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
            Recipe::Smelting { .. } => false, // Smelting recipes only work in a furnace, never in crafting grids!
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

    pub fn recipes(&self) -> &[Recipe] {
        &self.recipes
    }

    /// Finds all display recipes that produce `item_id`.
    pub fn get_recipes_for_output(&self, item_id: ItemId) -> Vec<DisplayRecipe> {
        self.recipes
            .iter()
            .filter(|r| r.output().item_id == item_id)
            .map(|r| r.to_display_recipe())
            .collect()
    }

    /// Finds all display recipes that use `item_id` as an input ingredient.
    pub fn get_recipes_using_input(&self, item_id: ItemId) -> Vec<DisplayRecipe> {
        self.recipes
            .iter()
            .filter(|r| match r {
                Recipe::Shapeless { inputs, .. } => inputs.contains(&item_id),
                Recipe::Shaped { pattern, .. } => pattern.contains(&Some(item_id)),
                Recipe::Smelting { input, .. } => *input == item_id,
            })
            .map(|r| r.to_display_recipe())
            .collect()
    }

    /// Returns all registered recipes in display format.
    pub fn get_all_display_recipes(&self) -> Vec<DisplayRecipe> {
        self.recipes.iter().map(|r| r.to_display_recipe()).collect()
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
            Recipe::Shaped {
                width,
                height,
                pattern,
                ..
            } => {
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
            Recipe::Smelting { .. } => {
                unreachable!("Smelting recipes cannot be crafted in a crafting grid")
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
        self.register(Recipe::Shapeless {
            name: "Birch Planks from Birch Log",
            inputs: vec![ItemId::BIRCH_LOG],
            output: ItemSlot::new(ItemId::BIRCH_PLANKS, 4),
        });
        self.register(Recipe::Shapeless {
            name: "Spruce Planks from Spruce Log",
            inputs: vec![ItemId::SPRUCE_LOG],
            output: ItemSlot::new(ItemId::SPRUCE_PLANKS, 4),
        });
        self.register(Recipe::Shapeless {
            name: "Acacia Planks from Acacia Log",
            inputs: vec![ItemId::ACACIA_LOG],
            output: ItemSlot::new(ItemId::ACACIA_PLANKS, 4),
        });
        self.register(Recipe::Shapeless {
            name: "Jungle Planks from Jungle Log",
            inputs: vec![ItemId::JUNGLE_LOG],
            output: ItemSlot::new(ItemId::JUNGLE_PLANKS, 4),
        });
        self.register(Recipe::Shapeless {
            name: "Dark Oak Planks from Dark Oak Log",
            inputs: vec![ItemId::DARK_OAK_LOG],
            output: ItemSlot::new(ItemId::DARK_OAK_PLANKS, 4),
        });

        // 2. 2 Planks vertically -> 4 Sticks (Shaped 1x2)
        self.register(Recipe::Shaped {
            name: "Sticks from Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::PLANKS), Some(ItemId::PLANKS)],
            output: ItemSlot::new(ItemId::STICK, 4),
        });
        self.register(Recipe::Shaped {
            name: "Sticks from Birch Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::BIRCH_PLANKS), Some(ItemId::BIRCH_PLANKS)],
            output: ItemSlot::new(ItemId::STICK, 4),
        });
        self.register(Recipe::Shaped {
            name: "Sticks from Spruce Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::SPRUCE_PLANKS), Some(ItemId::SPRUCE_PLANKS)],
            output: ItemSlot::new(ItemId::STICK, 4),
        });
        self.register(Recipe::Shaped {
            name: "Sticks from Acacia Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::ACACIA_PLANKS), Some(ItemId::ACACIA_PLANKS)],
            output: ItemSlot::new(ItemId::STICK, 4),
        });
        self.register(Recipe::Shaped {
            name: "Sticks from Jungle Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::JUNGLE_PLANKS), Some(ItemId::JUNGLE_PLANKS)],
            output: ItemSlot::new(ItemId::STICK, 4),
        });
        self.register(Recipe::Shaped {
            name: "Sticks from Dark Oak Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::DARK_OAK_PLANKS), Some(ItemId::DARK_OAK_PLANKS)],
            output: ItemSlot::new(ItemId::STICK, 4),
        });

        // 3. 4 Planks (2x2) -> 1 Crafting Table
        self.register(Recipe::Shaped {
            name: "Crafting Table",
            width: 2,
            height: 2,
            pattern: vec![
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
            ],
            output: ItemSlot::new(ItemId::CRAFTING_TABLE, 1),
        });
        self.register(Recipe::Shaped {
            name: "Crafting Table (Birch)",
            width: 2,
            height: 2,
            pattern: vec![
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
            ],
            output: ItemSlot::new(ItemId::CRAFTING_TABLE, 1),
        });
        self.register(Recipe::Shaped {
            name: "Crafting Table (Spruce)",
            width: 2,
            height: 2,
            pattern: vec![
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
            ],
            output: ItemSlot::new(ItemId::CRAFTING_TABLE, 1),
        });
        self.register(Recipe::Shaped {
            name: "Crafting Table (Acacia)",
            width: 2,
            height: 2,
            pattern: vec![
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
            ],
            output: ItemSlot::new(ItemId::CRAFTING_TABLE, 1),
        });
        self.register(Recipe::Shaped {
            name: "Crafting Table (Jungle)",
            width: 2,
            height: 2,
            pattern: vec![
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
            ],
            output: ItemSlot::new(ItemId::CRAFTING_TABLE, 1),
        });
        self.register(Recipe::Shaped {
            name: "Crafting Table (Dark Oak)",
            width: 2,
            height: 2,
            pattern: vec![
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
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
        self.register(Recipe::Shaped {
            name: "Torches from Birch Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::BIRCH_PLANKS), Some(ItemId::STICK)],
            output: ItemSlot::new(ItemId::TORCH, 4),
        });
        self.register(Recipe::Shaped {
            name: "Torches from Spruce Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::SPRUCE_PLANKS), Some(ItemId::STICK)],
            output: ItemSlot::new(ItemId::TORCH, 4),
        });
        self.register(Recipe::Shaped {
            name: "Torches from Acacia Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::ACACIA_PLANKS), Some(ItemId::STICK)],
            output: ItemSlot::new(ItemId::TORCH, 4),
        });
        self.register(Recipe::Shaped {
            name: "Torches from Jungle Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::JUNGLE_PLANKS), Some(ItemId::STICK)],
            output: ItemSlot::new(ItemId::TORCH, 4),
        });
        self.register(Recipe::Shaped {
            name: "Torches from Dark Oak Planks",
            width: 1,
            height: 2,
            pattern: vec![Some(ItemId::DARK_OAK_PLANKS), Some(ItemId::STICK)],
            output: ItemSlot::new(ItemId::TORCH, 4),
        });

        // 6. Wooden Pickaxe: 3 Planks top row, 2 Sticks center column (Shaped 3x3)
        self.register(Recipe::Shaped {
            name: "Wooden Pickaxe",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
                None,
                Some(ItemId::STICK),
                None,
                None,
                Some(ItemId::STICK),
                None,
            ],
            output: ItemSlot::new(ItemId::WOODEN_PICKAXE, 1),
        });
        self.register(Recipe::Shaped {
            name: "Wooden Pickaxe (Birch)",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
                None,
                Some(ItemId::STICK),
                None,
                None,
                Some(ItemId::STICK),
                None,
            ],
            output: ItemSlot::new(ItemId::WOODEN_PICKAXE, 1),
        });
        self.register(Recipe::Shaped {
            name: "Wooden Pickaxe (Spruce)",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
                None,
                Some(ItemId::STICK),
                None,
                None,
                Some(ItemId::STICK),
                None,
            ],
            output: ItemSlot::new(ItemId::WOODEN_PICKAXE, 1),
        });
        self.register(Recipe::Shaped {
            name: "Wooden Pickaxe (Acacia)",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
                None,
                Some(ItemId::STICK),
                None,
                None,
                Some(ItemId::STICK),
                None,
            ],
            output: ItemSlot::new(ItemId::WOODEN_PICKAXE, 1),
        });
        self.register(Recipe::Shaped {
            name: "Wooden Pickaxe (Jungle)",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
                None,
                Some(ItemId::STICK),
                None,
                None,
                Some(ItemId::STICK),
                None,
            ],
            output: ItemSlot::new(ItemId::WOODEN_PICKAXE, 1),
        });
        self.register(Recipe::Shaped {
            name: "Wooden Pickaxe (Dark Oak)",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
                None,
                Some(ItemId::STICK),
                None,
                None,
                Some(ItemId::STICK),
                None,
            ],
            output: ItemSlot::new(ItemId::WOODEN_PICKAXE, 1),
        });

        // 7. Stone Pickaxe: 3 Cobblestone top row, 2 Sticks center column (Shaped 3x3)
        self.register(Recipe::Shaped {
            name: "Stone Pickaxe",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::COBBLESTONE),
                Some(ItemId::COBBLESTONE),
                Some(ItemId::COBBLESTONE),
                None,
                Some(ItemId::STICK),
                None,
                None,
                Some(ItemId::STICK),
                None,
            ],
            output: ItemSlot::new(ItemId::STONE_PICKAXE, 1),
        });

        // 8. Iron Pickaxe: 3 Iron Ingots top row, 2 Sticks center column (Shaped 3x3)
        self.register(Recipe::Shaped {
            name: "Iron Pickaxe",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::IRON_INGOT),
                Some(ItemId::IRON_INGOT),
                Some(ItemId::IRON_INGOT),
                None,
                Some(ItemId::STICK),
                None,
                None,
                Some(ItemId::STICK),
                None,
            ],
            output: ItemSlot::new(ItemId::IRON_PICKAXE, 1),
        });

        // 9. Diamond Pickaxe: 3 Diamonds top row, 2 Sticks center column (Shaped 3x3)
        self.register(Recipe::Shaped {
            name: "Diamond Pickaxe",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::DIAMOND),
                Some(ItemId::DIAMOND),
                Some(ItemId::DIAMOND),
                None,
                Some(ItemId::STICK),
                None,
                None,
                Some(ItemId::STICK),
                None,
            ],
            output: ItemSlot::new(ItemId::DIAMOND_PICKAXE, 1),
        });

        // Axes (2x3 trimmed pattern, both left and right orientations)
        let register_axe = |cm: &mut CraftingManager,
                            name_left: &'static str,
                            name_right: &'static str,
                            mat: ItemId,
                            out: ItemId| {
            // Left orientation
            cm.register(Recipe::Shaped {
                name: name_left,
                width: 2,
                height: 3,
                pattern: vec![
                    Some(mat),
                    Some(mat),
                    Some(mat),
                    Some(ItemId::STICK),
                    None,
                    Some(ItemId::STICK),
                ],
                output: ItemSlot::new(out, 1),
            });
            // Right orientation
            cm.register(Recipe::Shaped {
                name: name_right,
                width: 2,
                height: 3,
                pattern: vec![
                    Some(mat),
                    Some(mat),
                    Some(ItemId::STICK),
                    Some(mat),
                    Some(ItemId::STICK),
                    None,
                ],
                output: ItemSlot::new(out, 1),
            });
        };

        register_axe(
            self,
            "Wooden Axe (Left)",
            "Wooden Axe (Right)",
            ItemId::PLANKS,
            ItemId::WOODEN_AXE,
        );
        register_axe(
            self,
            "Wooden Axe (Birch Left)",
            "Wooden Axe (Birch Right)",
            ItemId::BIRCH_PLANKS,
            ItemId::WOODEN_AXE,
        );
        register_axe(
            self,
            "Wooden Axe (Spruce Left)",
            "Wooden Axe (Spruce Right)",
            ItemId::SPRUCE_PLANKS,
            ItemId::WOODEN_AXE,
        );
        register_axe(
            self,
            "Wooden Axe (Acacia Left)",
            "Wooden Axe (Acacia Right)",
            ItemId::ACACIA_PLANKS,
            ItemId::WOODEN_AXE,
        );
        register_axe(
            self,
            "Wooden Axe (Jungle Left)",
            "Wooden Axe (Jungle Right)",
            ItemId::JUNGLE_PLANKS,
            ItemId::WOODEN_AXE,
        );
        register_axe(
            self,
            "Wooden Axe (Dark Oak Left)",
            "Wooden Axe (Dark Oak Right)",
            ItemId::DARK_OAK_PLANKS,
            ItemId::WOODEN_AXE,
        );
        register_axe(
            self,
            "Stone Axe (Left)",
            "Stone Axe (Right)",
            ItemId::COBBLESTONE,
            ItemId::STONE_AXE,
        );
        register_axe(
            self,
            "Iron Axe (Left)",
            "Iron Axe (Right)",
            ItemId::IRON_INGOT,
            ItemId::IRON_AXE,
        );
        register_axe(
            self,
            "Diamond Axe (Left)",
            "Diamond Axe (Right)",
            ItemId::DIAMOND,
            ItemId::DIAMOND_AXE,
        );

        // Shovels (1x3 trimmed pattern)
        let register_shovel =
            |cm: &mut CraftingManager, name: &'static str, mat: ItemId, out: ItemId| {
                cm.register(Recipe::Shaped {
                    name,
                    width: 1,
                    height: 3,
                    pattern: vec![Some(mat), Some(ItemId::STICK), Some(ItemId::STICK)],
                    output: ItemSlot::new(out, 1),
                });
            };

        register_shovel(self, "Wooden Shovel", ItemId::PLANKS, ItemId::WOODEN_SHOVEL);
        register_shovel(self, "Wooden Shovel (Birch)", ItemId::BIRCH_PLANKS, ItemId::WOODEN_SHOVEL);
        register_shovel(self, "Wooden Shovel (Spruce)", ItemId::SPRUCE_PLANKS, ItemId::WOODEN_SHOVEL);
        register_shovel(self, "Wooden Shovel (Acacia)", ItemId::ACACIA_PLANKS, ItemId::WOODEN_SHOVEL);
        register_shovel(self, "Wooden Shovel (Jungle)", ItemId::JUNGLE_PLANKS, ItemId::WOODEN_SHOVEL);
        register_shovel(self, "Wooden Shovel (Dark Oak)", ItemId::DARK_OAK_PLANKS, ItemId::WOODEN_SHOVEL);
        register_shovel(
            self,
            "Stone Shovel",
            ItemId::COBBLESTONE,
            ItemId::STONE_SHOVEL,
        );
        register_shovel(self, "Iron Shovel", ItemId::IRON_INGOT, ItemId::IRON_SHOVEL);
        register_shovel(
            self,
            "Diamond Shovel",
            ItemId::DIAMOND,
            ItemId::DIAMOND_SHOVEL,
        );

        // Furnace Smelting Recipes (1 Raw Ore -> 1 Ingot)
        self.register(Recipe::Smelting {
            name: "Iron Ingot from Raw Iron",
            input: ItemId::RAW_IRON,
            output: ItemSlot::new(ItemId::IRON_INGOT, 1),
        });

        self.register(Recipe::Smelting {
            name: "Iron Ingot from Iron Ore",
            input: ItemId::IRON_ORE,
            output: ItemSlot::new(ItemId::IRON_INGOT, 1),
        });

        self.register(Recipe::Smelting {
            name: "Copper Ingot from Raw Copper",
            input: ItemId::RAW_COPPER,
            output: ItemSlot::new(ItemId::COPPER_INGOT, 1),
        });

        self.register(Recipe::Smelting {
            name: "Copper Ingot from Copper Ore",
            input: ItemId::COPPER_ORE,
            output: ItemSlot::new(ItemId::COPPER_INGOT, 1),
        });

        self.register(Recipe::Smelting {
            name: "Gold Ingot from Raw Gold",
            input: ItemId::RAW_GOLD,
            output: ItemSlot::new(ItemId::GOLD_INGOT, 1),
        });

        self.register(Recipe::Smelting {
            name: "Gold Ingot from Gold Ore",
            input: ItemId::GOLD_ORE,
            output: ItemSlot::new(ItemId::GOLD_INGOT, 1),
        });

        // Hoes (2x3 trimmed pattern, both left and right orientations)
        let register_hoe = |cm: &mut CraftingManager,
                            name_left: &'static str,
                            name_right: &'static str,
                            mat: ItemId,
                            out: ItemId| {
            // Left orientation
            cm.register(Recipe::Shaped {
                name: name_left,
                width: 2,
                height: 3,
                pattern: vec![
                    Some(mat),
                    Some(mat),
                    None,
                    Some(ItemId::STICK),
                    None,
                    Some(ItemId::STICK),
                ],
                output: ItemSlot::new(out, 1),
            });
            // Right orientation
            cm.register(Recipe::Shaped {
                name: name_right,
                width: 2,
                height: 3,
                pattern: vec![
                    Some(mat),
                    Some(mat),
                    Some(ItemId::STICK),
                    None,
                    Some(ItemId::STICK),
                    None,
                ],
                output: ItemSlot::new(out, 1),
            });
        };

        register_hoe(
            self,
            "Wooden Hoe (Left)",
            "Wooden Hoe (Right)",
            ItemId::PLANKS,
            ItemId::WOODEN_HOE,
        );
        register_hoe(
            self,
            "Wooden Hoe (Birch Left)",
            "Wooden Hoe (Birch Right)",
            ItemId::BIRCH_PLANKS,
            ItemId::WOODEN_HOE,
        );
        register_hoe(
            self,
            "Wooden Hoe (Spruce Left)",
            "Wooden Hoe (Spruce Right)",
            ItemId::SPRUCE_PLANKS,
            ItemId::WOODEN_HOE,
        );
        register_hoe(
            self,
            "Wooden Hoe (Acacia Left)",
            "Wooden Hoe (Acacia Right)",
            ItemId::ACACIA_PLANKS,
            ItemId::WOODEN_HOE,
        );
        register_hoe(
            self,
            "Wooden Hoe (Jungle Left)",
            "Wooden Hoe (Jungle Right)",
            ItemId::JUNGLE_PLANKS,
            ItemId::WOODEN_HOE,
        );
        register_hoe(
            self,
            "Wooden Hoe (Dark Oak Left)",
            "Wooden Hoe (Dark Oak Right)",
            ItemId::DARK_OAK_PLANKS,
            ItemId::WOODEN_HOE,
        );
        register_hoe(
            self,
            "Stone Hoe (Left)",
            "Stone Hoe (Right)",
            ItemId::COBBLESTONE,
            ItemId::STONE_HOE,
        );
        register_hoe(
            self,
            "Iron Hoe (Left)",
            "Iron Hoe (Right)",
            ItemId::IRON_INGOT,
            ItemId::IRON_HOE,
        );
        register_hoe(
            self,
            "Diamond Hoe (Left)",
            "Diamond Hoe (Right)",
            ItemId::DIAMOND,
            ItemId::DIAMOND_HOE,
        );

        // Bread: 3 Wheat in a horizontal row (3x1)
        self.register(Recipe::Shaped {
            name: "Bread",
            width: 3,
            height: 1,
            pattern: vec![
                Some(ItemId::WHEAT),
                Some(ItemId::WHEAT),
                Some(ItemId::WHEAT),
            ],
            output: ItemSlot::new(ItemId::BREAD, 1),
        });

        // Furnace Cooking Meats (1 Raw Meat -> 1 Cooked Meat)
        self.register(Recipe::Smelting {
            name: "Cooked Steak from Raw Beef",
            input: ItemId::RAW_BEEF,
            output: ItemSlot::new(ItemId::COOKED_STEAK, 1),
        });

        self.register(Recipe::Smelting {
            name: "Cooked Porkchop from Raw Porkchop",
            input: ItemId::RAW_PORKCHOP,
            output: ItemSlot::new(ItemId::COOKED_PORKCHOP, 1),
        });

        self.register(Recipe::Smelting {
            name: "Cooked Chicken from Raw Chicken",
            input: ItemId::RAW_CHICKEN,
            output: ItemSlot::new(ItemId::COOKED_CHICKEN, 1),
        });

        // Chest: 8 Planks in a 3x3 hollow square ring
        self.register(Recipe::Shaped {
            name: "Chest",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
                None,
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
                Some(ItemId::PLANKS),
            ],
            output: ItemSlot::new(ItemId::CHEST, 1),
        });
        self.register(Recipe::Shaped {
            name: "Chest (Birch)",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
                None,
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
                Some(ItemId::BIRCH_PLANKS),
            ],
            output: ItemSlot::new(ItemId::CHEST, 1),
        });
        self.register(Recipe::Shaped {
            name: "Chest (Spruce)",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
                None,
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
                Some(ItemId::SPRUCE_PLANKS),
            ],
            output: ItemSlot::new(ItemId::CHEST, 1),
        });
        self.register(Recipe::Shaped {
            name: "Chest (Acacia)",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
                None,
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
                Some(ItemId::ACACIA_PLANKS),
            ],
            output: ItemSlot::new(ItemId::CHEST, 1),
        });
        self.register(Recipe::Shaped {
            name: "Chest (Jungle)",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
                None,
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
                Some(ItemId::JUNGLE_PLANKS),
            ],
            output: ItemSlot::new(ItemId::CHEST, 1),
        });
        self.register(Recipe::Shaped {
            name: "Chest (Dark Oak)",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
                None,
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
                Some(ItemId::DARK_OAK_PLANKS),
            ],
            output: ItemSlot::new(ItemId::CHEST, 1),
        });

        // Sandstone: 4 Sand in a 2x2 square
        self.register(Recipe::Shaped {
            name: "Sandstone",
            width: 2,
            height: 2,
            pattern: vec![
                Some(ItemId::SAND),
                Some(ItemId::SAND),
                Some(ItemId::SAND),
                Some(ItemId::SAND),
            ],
            output: ItemSlot::new(ItemId::SANDSTONE, 4),
        });
        self.register(Recipe::Shaped {
            name: "Sandstone from Red Sand",
            width: 2,
            height: 2,
            pattern: vec![
                Some(ItemId::RED_SAND),
                Some(ItemId::RED_SAND),
                Some(ItemId::RED_SAND),
                Some(ItemId::RED_SAND),
            ],
            output: ItemSlot::new(ItemId::SANDSTONE, 4),
        });

        // Furnace: 8 Cobblestone in a 3x3 hollow square ring
        self.register(Recipe::Shaped {
            name: "Furnace",
            width: 3,
            height: 3,
            pattern: vec![
                Some(ItemId::COBBLESTONE),
                Some(ItemId::COBBLESTONE),
                Some(ItemId::COBBLESTONE),
                Some(ItemId::COBBLESTONE),
                None,
                Some(ItemId::COBBLESTONE),
                Some(ItemId::COBBLESTONE),
                Some(ItemId::COBBLESTONE),
                Some(ItemId::COBBLESTONE),
            ],
            output: ItemSlot::new(ItemId::FURNACE, 1),
        });

        // Furnace Smelting: Cobblestone -> Stone
        self.register(Recipe::Smelting {
            name: "Stone from Cobblestone",
            input: ItemId::COBBLESTONE,
            output: ItemSlot::new(ItemId::STONE, 1),
        });

        // Furnace Smelting: Sand -> Glass
        self.register(Recipe::Smelting {
            name: "Glass from Sand",
            input: ItemId::SAND,
            output: ItemSlot::new(ItemId::GLASS, 1),
        });

        // Furnace Smelting: Clay -> Terracotta
        self.register(Recipe::Smelting {
            name: "Terracotta from Clay",
            input: ItemId::CLAY,
            output: ItemSlot::new(ItemId::TERRACOTTA, 1),
        });

        // Mossy Cobblestone: Cobblestone + Leaves (Shapeless)
        self.register(Recipe::Shapeless {
            name: "Mossy Cobblestone from Leaves",
            inputs: vec![ItemId::COBBLESTONE, ItemId::LEAVES],
            output: ItemSlot::new(ItemId::MOSSY_COBBLESTONE, 1),
        });
        self.register(Recipe::Shapeless {
            name: "Mossy Cobblestone from Spruce Leaves",
            inputs: vec![ItemId::COBBLESTONE, ItemId::SPRUCE_LEAVES],
            output: ItemSlot::new(ItemId::MOSSY_COBBLESTONE, 1),
        });
        self.register(Recipe::Shapeless {
            name: "Mossy Cobblestone from Birch Leaves",
            inputs: vec![ItemId::COBBLESTONE, ItemId::BIRCH_LEAVES],
            output: ItemSlot::new(ItemId::MOSSY_COBBLESTONE, 1),
        });
        self.register(Recipe::Shapeless {
            name: "Mossy Cobblestone from Jungle Leaves",
            inputs: vec![ItemId::COBBLESTONE, ItemId::JUNGLE_LEAVES],
            output: ItemSlot::new(ItemId::MOSSY_COBBLESTONE, 1),
        });
        self.register(Recipe::Shapeless {
            name: "Mossy Cobblestone from Dark Oak Leaves",
            inputs: vec![ItemId::COBBLESTONE, ItemId::DARK_OAK_LEAVES],
            output: ItemSlot::new(ItemId::MOSSY_COBBLESTONE, 1),
        });

        // Prismarine: 4 Cobblestone + 1 Clay (Shapeless)
        self.register(Recipe::Shapeless {
            name: "Prismarine",
            inputs: vec![
                ItemId::COBBLESTONE,
                ItemId::COBBLESTONE,
                ItemId::COBBLESTONE,
                ItemId::COBBLESTONE,
                ItemId::CLAY,
            ],
            output: ItemSlot::new(ItemId::PRISMARINE, 4),
        });

        // Sea Lantern: 4 Prismarine + 1 Torch (Shapeless)
        self.register(Recipe::Shapeless {
            name: "Sea Lantern",
            inputs: vec![
                ItemId::PRISMARINE,
                ItemId::PRISMARINE,
                ItemId::PRISMARINE,
                ItemId::PRISMARINE,
                ItemId::TORCH,
            ],
            output: ItemSlot::new(ItemId::SEA_LANTERN, 1),
        });

        // Bucket: 3 Iron Ingots in a V-shape (Shaped 3x2)
        // [Iron Ingot, None, Iron Ingot]
        // [None, Iron Ingot, None]
        self.register(Recipe::Shaped {
            name: "Bucket",
            width: 3,
            height: 2,
            pattern: vec![
                Some(ItemId::IRON_INGOT),
                None,
                Some(ItemId::IRON_INGOT),
                None,
                Some(ItemId::IRON_INGOT),
                None,
            ],
            output: ItemSlot::new(ItemId::BUCKET, 1),
        });
    }
}

/// Returns the smelted/cooked output ItemId for a given input item, if smeltable in a Furnace.
pub fn get_smelting_output(input: ItemId) -> Option<ItemId> {
    match input {
        ItemId::RAW_IRON | ItemId::IRON_ORE => Some(ItemId::IRON_INGOT),
        ItemId::RAW_GOLD | ItemId::GOLD_ORE => Some(ItemId::GOLD_INGOT),
        ItemId::RAW_COPPER | ItemId::COPPER_ORE => Some(ItemId::COPPER_INGOT),
        ItemId::COBBLESTONE => Some(ItemId::STONE),
        ItemId::SAND => Some(ItemId::GLASS),
        ItemId::CLAY => Some(ItemId::TERRACOTTA),
        ItemId::RAW_BEEF => Some(ItemId::COOKED_STEAK),
        ItemId::RAW_PORKCHOP => Some(ItemId::COOKED_PORKCHOP),
        ItemId::RAW_CHICKEN => Some(ItemId::COOKED_CHICKEN),
        _ => None,
    }
}

/// Returns the fuel burn duration (in seconds) for 1 item of the given type.
/// Returns 0.0 if the item is not a valid furnace fuel.
pub fn get_fuel_burn_time(fuel: ItemId) -> f32 {
    match fuel {
        ItemId::COAL => 80.0,
        ItemId::WOOD_LOG
        | ItemId::BIRCH_LOG
        | ItemId::SPRUCE_LOG
        | ItemId::ACACIA_LOG
        | ItemId::JUNGLE_LOG
        | ItemId::DARK_OAK_LOG
        | ItemId::PLANKS
        | ItemId::BIRCH_PLANKS
        | ItemId::SPRUCE_PLANKS
        | ItemId::ACACIA_PLANKS
        | ItemId::JUNGLE_PLANKS
        | ItemId::DARK_OAK_PLANKS
        | ItemId::CRAFTING_TABLE
        | ItemId::CHEST
        | ItemId::FURNACE => 15.0,
        ItemId::STICK
        | ItemId::MUSHROOM_STEM
        | ItemId::RED_MUSHROOM_BLOCK
        | ItemId::BROWN_MUSHROOM_BLOCK => 5.0,
        ItemId::WOODEN_PICKAXE
        | ItemId::WOODEN_AXE
        | ItemId::WOODEN_SHOVEL
        | ItemId::WOODEN_HOE => 10.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chest_crafting() {
        let cm = CraftingManager::new();
        let mut grid = CraftingGrid::new_3x3();
        for y in 0..3 {
            for x in 0..3 {
                if x == 1 && y == 1 {
                    continue;
                }
                grid.set(x, y, ItemSlot::new(ItemId::PLANKS, 1));
            }
        }
        let result = cm.craft(&mut grid);
        assert_eq!(result, Some(ItemSlot::new(ItemId::CHEST, 1)));
        assert!(grid.is_empty());
    }

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
        assert!(
            res.is_some(),
            "Shaped recipe should match regardless of corner placement!"
        );
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
    fn test_raw_ore_and_meat_must_be_smelted() {
        let cm = CraftingManager::new();
        let mut grid = CraftingGrid::new_2x2();

        // Raw Iron in crafting grid produces nothing
        grid.set(1, 1, ItemSlot::new(ItemId::RAW_IRON, 3));
        assert!(
            cm.find_match(&grid).is_none(),
            "Raw Iron cannot be crafted in crafting grid"
        );

        // Raw Beef in crafting grid produces nothing
        grid.set(1, 1, ItemSlot::new(ItemId::RAW_BEEF, 1));
        assert!(
            cm.find_match(&grid).is_none(),
            "Raw Beef cannot be crafted in crafting grid"
        );

        // Raw Porkchop in crafting grid produces nothing
        grid.set(1, 1, ItemSlot::new(ItemId::RAW_PORKCHOP, 1));
        assert!(
            cm.find_match(&grid).is_none(),
            "Raw Porkchop cannot be crafted in crafting grid"
        );

        // Raw Copper in crafting grid produces nothing
        grid.set(1, 1, ItemSlot::new(ItemId::RAW_COPPER, 1));
        assert!(
            cm.find_match(&grid).is_none(),
            "Raw Copper cannot be crafted in crafting grid"
        );

        // Smelting display queries return Smelting category recipes
        let iron_recipes = cm.get_recipes_for_output(ItemId::IRON_INGOT);
        assert!(iron_recipes
            .iter()
            .any(|r| r.category == RecipeCategory::Smelting));

        let steak_recipes = cm.get_recipes_for_output(ItemId::COOKED_STEAK);
        assert!(steak_recipes
            .iter()
            .any(|r| r.category == RecipeCategory::Smelting));

        // Furnace smelting lookup succeeds
        assert_eq!(
            get_smelting_output(ItemId::RAW_IRON),
            Some(ItemId::IRON_INGOT)
        );
        assert_eq!(
            get_smelting_output(ItemId::RAW_BEEF),
            Some(ItemId::COOKED_STEAK)
        );
        assert_eq!(
            get_smelting_output(ItemId::RAW_PORKCHOP),
            Some(ItemId::COOKED_PORKCHOP)
        );
        assert_eq!(
            get_smelting_output(ItemId::RAW_COPPER),
            Some(ItemId::COPPER_INGOT)
        );
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

    #[test]
    fn test_display_recipe_queries() {
        let cm = CraftingManager::new();

        // 1. Recipes producing Diamond Pickaxe
        let dia_pick_recipes = cm.get_recipes_for_output(ItemId::DIAMOND_PICKAXE);
        assert_eq!(dia_pick_recipes.len(), 1);
        assert_eq!(dia_pick_recipes[0].category, RecipeCategory::Crafting3x3);
        assert_eq!(dia_pick_recipes[0].inputs[0], Some(ItemId::DIAMOND));
        assert_eq!(dia_pick_recipes[0].inputs[1], Some(ItemId::DIAMOND));
        assert_eq!(dia_pick_recipes[0].inputs[2], Some(ItemId::DIAMOND));
        assert_eq!(dia_pick_recipes[0].inputs[4], Some(ItemId::STICK));
        assert_eq!(dia_pick_recipes[0].inputs[7], Some(ItemId::STICK));

        // 2. Recipes producing Chest (Oak, Birch, Spruce, Acacia, Jungle, Dark Oak variants)
        let chest_recipes = cm.get_recipes_for_output(ItemId::CHEST);
        assert_eq!(chest_recipes.len(), 6);
        assert_eq!(chest_recipes[0].category, RecipeCategory::Crafting3x3);

        // 3. Recipes producing Planks (from Wood Log)
        let plank_recipes = cm.get_recipes_for_output(ItemId::PLANKS);
        assert_eq!(plank_recipes.len(), 1);
        assert_eq!(plank_recipes[0].output.count, 4);

        // 4. Recipes using Wheat as ingredient
        let wheat_uses = cm.get_recipes_using_input(ItemId::WHEAT);
        assert_eq!(wheat_uses.len(), 1);
        assert_eq!(wheat_uses[0].output.item_id, ItemId::BREAD);

        // 5. Smelting category recognition
        let iron_ingot_recipes = cm.get_recipes_for_output(ItemId::IRON_INGOT);
        assert!(iron_ingot_recipes
            .iter()
            .any(|r| r.category == RecipeCategory::Smelting));

        let terracotta_recipes = cm.get_recipes_for_output(ItemId::TERRACOTTA);
        assert_eq!(terracotta_recipes.len(), 1);
        assert_eq!(terracotta_recipes[0].category, RecipeCategory::Smelting);
    }

    #[test]
    fn test_phase_3_jungle_and_badlands_recipes() {
        let cm = CraftingManager::new();

        // 1. Jungle Log -> 4 Jungle Planks (Shapeless)
        let mut grid_shapeless = CraftingGrid::new_2x2();
        grid_shapeless.set(0, 0, ItemSlot::new(ItemId::JUNGLE_LOG, 1));
        let out = cm.craft(&mut grid_shapeless);
        assert_eq!(out, Some(ItemSlot::new(ItemId::JUNGLE_PLANKS, 4)));

        // 2. Sandstone from 4 Red Sand (Shaped 2x2)
        let mut grid_red_sand = CraftingGrid::new_2x2();
        grid_red_sand.set(0, 0, ItemSlot::new(ItemId::RED_SAND, 1));
        grid_red_sand.set(1, 0, ItemSlot::new(ItemId::RED_SAND, 1));
        grid_red_sand.set(0, 1, ItemSlot::new(ItemId::RED_SAND, 1));
        grid_red_sand.set(1, 1, ItemSlot::new(ItemId::RED_SAND, 1));
        let out_sandstone = cm.craft(&mut grid_red_sand);
        assert_eq!(out_sandstone, Some(ItemSlot::new(ItemId::SANDSTONE, 4)));

        // 3. Furnace smelting: Clay -> Terracotta
        assert_eq!(get_smelting_output(ItemId::CLAY), Some(ItemId::TERRACOTTA));

        // 4. Furnace fuel: Jungle Log and Jungle Planks burn for 15.0s
        assert_eq!(get_fuel_burn_time(ItemId::JUNGLE_LOG), 15.0);
        assert_eq!(get_fuel_burn_time(ItemId::JUNGLE_PLANKS), 15.0);
    }

    #[test]
    fn test_phase_4_mossy_cobble_and_mushroom_recipes() {
        let cm = CraftingManager::new();

        // 1. Cobblestone + Leaves -> Mossy Cobblestone (Shapeless)
        let mut grid = CraftingGrid::new_2x2();
        grid.set(0, 0, ItemSlot::new(ItemId::COBBLESTONE, 1));
        grid.set(1, 0, ItemSlot::new(ItemId::LEAVES, 1));
        let out = cm.craft(&mut grid);
        assert_eq!(out, Some(ItemSlot::new(ItemId::MOSSY_COBBLESTONE, 1)));

        // 2. Cobblestone + Spruce Leaves -> Mossy Cobblestone (Shapeless)
        let mut grid_spruce = CraftingGrid::new_2x2();
        grid_spruce.set(0, 0, ItemSlot::new(ItemId::COBBLESTONE, 1));
        grid_spruce.set(0, 1, ItemSlot::new(ItemId::SPRUCE_LEAVES, 1));
        let out_spruce = cm.craft(&mut grid_spruce);
        assert_eq!(out_spruce, Some(ItemSlot::new(ItemId::MOSSY_COBBLESTONE, 1)));

        // 3. Furnace fuel: Giant mushroom blocks & stems burn for 5.0s
        assert_eq!(get_fuel_burn_time(ItemId::MUSHROOM_STEM), 5.0);
        assert_eq!(get_fuel_burn_time(ItemId::RED_MUSHROOM_BLOCK), 5.0);
        assert_eq!(get_fuel_burn_time(ItemId::BROWN_MUSHROOM_BLOCK), 5.0);
    }

    #[test]
    fn test_phase_5_dark_oak_and_coral_recipes() {
        let cm = CraftingManager::new();

        // 1. Dark Oak Log -> 4 Dark Oak Planks (Shapeless)
        let mut grid_log = CraftingGrid::new_2x2();
        grid_log.set(0, 0, ItemSlot::new(ItemId::DARK_OAK_LOG, 1));
        let out_planks = cm.craft(&mut grid_log);
        assert_eq!(out_planks, Some(ItemSlot::new(ItemId::DARK_OAK_PLANKS, 4)));

        // 2. 4 Dark Oak Planks -> Crafting Table (Shaped 2x2)
        let mut grid_table = CraftingGrid::new_2x2();
        grid_table.set(0, 0, ItemSlot::new(ItemId::DARK_OAK_PLANKS, 1));
        grid_table.set(1, 0, ItemSlot::new(ItemId::DARK_OAK_PLANKS, 1));
        grid_table.set(0, 1, ItemSlot::new(ItemId::DARK_OAK_PLANKS, 1));
        grid_table.set(1, 1, ItemSlot::new(ItemId::DARK_OAK_PLANKS, 1));
        let out_table = cm.craft(&mut grid_table);
        assert_eq!(out_table, Some(ItemSlot::new(ItemId::CRAFTING_TABLE, 1)));

        // 3. 4 Cobblestone + 1 Clay -> 4 Prismarine (Shapeless)
        let mut grid_prismarine = CraftingGrid::new_3x3();
        grid_prismarine.set(0, 0, ItemSlot::new(ItemId::COBBLESTONE, 1));
        grid_prismarine.set(1, 0, ItemSlot::new(ItemId::COBBLESTONE, 1));
        grid_prismarine.set(2, 0, ItemSlot::new(ItemId::COBBLESTONE, 1));
        grid_prismarine.set(0, 1, ItemSlot::new(ItemId::COBBLESTONE, 1));
        grid_prismarine.set(1, 1, ItemSlot::new(ItemId::CLAY, 1));
        let out_prismarine = cm.craft(&mut grid_prismarine);
        assert_eq!(out_prismarine, Some(ItemSlot::new(ItemId::PRISMARINE, 4)));

        // 4. 4 Prismarine + 1 Torch -> 1 Sea Lantern (Shapeless)
        let mut grid_sea_lantern = CraftingGrid::new_3x3();
        grid_sea_lantern.set(0, 0, ItemSlot::new(ItemId::PRISMARINE, 1));
        grid_sea_lantern.set(1, 0, ItemSlot::new(ItemId::PRISMARINE, 1));
        grid_sea_lantern.set(0, 1, ItemSlot::new(ItemId::PRISMARINE, 1));
        grid_sea_lantern.set(1, 1, ItemSlot::new(ItemId::PRISMARINE, 1));
        grid_sea_lantern.set(2, 1, ItemSlot::new(ItemId::TORCH, 1));
        let out_sea_lantern = cm.craft(&mut grid_sea_lantern);
        assert_eq!(out_sea_lantern, Some(ItemSlot::new(ItemId::SEA_LANTERN, 1)));

        // 5. Cobblestone + Dark Oak Leaves -> Mossy Cobblestone
        let mut grid_mossy = CraftingGrid::new_2x2();
        grid_mossy.set(0, 0, ItemSlot::new(ItemId::COBBLESTONE, 1));
        grid_mossy.set(1, 0, ItemSlot::new(ItemId::DARK_OAK_LEAVES, 1));
        let out_mossy = cm.craft(&mut grid_mossy);
        assert_eq!(out_mossy, Some(ItemSlot::new(ItemId::MOSSY_COBBLESTONE, 1)));

        // 6. Fuel burn time: Dark Oak logs & planks burn for 15.0s
        assert_eq!(get_fuel_burn_time(ItemId::DARK_OAK_LOG), 15.0);
        assert_eq!(get_fuel_burn_time(ItemId::DARK_OAK_PLANKS), 15.0);
    }

    #[test]
    fn test_bucket_crafting() {
        let cm = CraftingManager::new();
        let mut grid = CraftingGrid::new_3x3();
        // V-shape placed in bottom two rows
        grid.set(0, 1, ItemSlot::new(ItemId::IRON_INGOT, 1));
        grid.set(2, 1, ItemSlot::new(ItemId::IRON_INGOT, 1));
        grid.set(1, 2, ItemSlot::new(ItemId::IRON_INGOT, 1));

        let output = cm.craft(&mut grid);
        assert_eq!(output, Some(ItemSlot::new(ItemId::BUCKET, 1)));
        assert!(grid.is_empty());
    }
}
