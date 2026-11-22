# Farming, Cooking, and Hunger

BlockGame features a full agricultural, ranching, and culinary loop connecting crop farming, passive soil fertilization, multi-ingredient cooking, and tiered hunger/saturation survival.

---

## Crops & Natural Discovery

Seven crops can be discovered in natural biome patches or harvested from wild grass and cultivated on tilled farmland (or water-adjacent soil for Sugar Cane):

| Crop | Planting Method | Growth Behavior | Harvest Output |
| :--- | :--- | :--- | :--- |
| **Wheat** | Plant **Wheat Seeds** on Farmland | 4 growth stages | Wheat + Wheat Seeds |
| **Carrots** | Plant **Carrot** directly on Farmland | 4 growth stages | Carrots |
| **Potatoes** | Plant **Potato** directly on Farmland | 4 growth stages | Potatoes (roast into Baked Potato) |
| **Tomatoes** | Plant **Tomato Seeds** on Farmland | 4 growth stages | Tomatoes + Tomato Seeds |
| **Melons** | Plant **Melon Seeds** on Farmland | 4-stage **Stem Crop** — once mature, spawns a **Melon** block onto an adjacent cardinal air tile over grass, dirt, or farmland; stem stays intact | Melon Slices (craft 9 into Melon block or 1 into Melon Seeds) |
| **Pumpkins** | Plant **Pumpkin Seeds** on Farmland | 4-stage **Stem Crop** — spawns an adjacent **Pumpkin** block while keeping the stem intact | Pumpkin (craft into Pumpkin Seeds or Pumpkin Pie) |
| **Sugar Cane** | Plant directly on **Sand, Dirt, or Grass** cardinally adjacent to **Water** | Grows upward up to **3 blocks tall** | Sugar Cane (craft into **Sugar**) |

### Growth Rules & Farmland Hydration
- **Light Requirement**: Crops require a light level of at least **9** (sunlight or nearby torches) to advance growth stages.
- **Hydration**: Farmland within 4 blocks of water becomes **Moist Farmland** and grows crops at full speed (randomized per stage, averaging ~5–8 minutes to full maturity).
- **Dry Farmland**: Unhydrated farmland pauses crop growth and reverts back to Dirt after **5 seconds** if empty or **10 seconds** if a crop is planted on top (popping the crop as an item drop).
- **Active Simulation**: Crops, stems, sugar cane, and farmland tick in active chunks around players (locally in singleplayer; authoritatively on the server in multiplayer).

---

## Passive Soil Fertilizers

Instead of instant crop growth, BlockGame uses **passive soil-buff Fertilizers** applied directly to Farmland (`Right Click` with Fertilizer in hand):

| Fertilizer | Crafting Recipe | Effect | Duration |
| :--- | :--- | :--- | :--- |
| **Compost** (×2) | Any mix of **4 organic items** (seeds, crops, leaves, flowers, rotten flesh, raw foods) | **1.5×** crop growth speed | 1 harvest cycle (or 1 Stem Crop fruit spawn) |
| **Bone Fertilizer** (×2) | **1× Bone + 1× Compost** (shapeless) | **2.0×** crop growth speed + guaranteed **+1 bonus crop yield** | 1 harvest cycle (or 1 Stem Crop fruit spawn) |

- **Upgrading**: Applying Bone Fertilizer onto Composted farmland upgrades it to Bone-Fertilized farmland (downgrading is not allowed).
- **Persistence**: Fertilizer tier is stored directly in the farmland block and persists across saves, but is lost if dry farmland reverts to Dirt.

---

## Cooking Pot & Furnace Roasting

Simple roasting (steaks, porkchops, cooked mutton, cooked chicken, baked potatoes, cooked fish) takes place in a **Furnace** or **Electric Furnace**. Multi-ingredient meals and remedies are prepared in a **Cooking Pot**.

### Crafting & Using the Cooking Pot
- **Recipe**: `5× Iron Ingot + 2× Planks` (3×3 Crafting Table).
- **Interface**: `Right Click` the Cooking Pot to open its **4-slot shapeless ingredient tray**, **fuel slot** (coal, charcoal, wood, planks, sticks), and **output slot**.
- **Cook Time**: ~10 seconds per dish while fueled. Lit Cooking Pots emit warm blocklight and save their inventory in world container data.
- **Liquid Buckets**: Recipes using a **Water Bucket** or **Milk Bucket** automatically return an empty **Bucket** in the ingredient tray when cooked.
- **JEI Recipe Browser**: Hover any meal or ingredient and press `R` / `U` to browse Cooking Pot recipes directly in-game.
- **Stacking**: Cooked meals require no bowls and stack up to **16**.

### Cooking Pot Recipes

| Dish | Ingredients (Shapeless) | Output | Notes |
| :--- | :--- | :--- | :--- |
| **Tomato Soup** | 2× Tomato | 1× Tomato Soup | Warm mid-tier meal |
| **Vegetable Stew** | Carrot + Potato + Tomato | 1× Vegetable Stew | Strong plant-based saturation |
| **Hearty Meat Stew** | (Raw Beef *or* Raw Porkchop *or* Raw Mutton) + Carrot + Potato | 1× Hearty Meat Stew | Top-tier hunger & saturation |
| **Omelette** | 2× Egg + (Tomato *or* Carrot) | 1× Omelette | Quick protein meal |
| **Fisherman's Stew** | (Raw Carp *or* Raw Trout *or* Raw Salmon) + Carrot + Potato | 1× Fish Stew | Hearty seafood meal |
| **Fruit Salad** | Apple + Melon Slice + Sugar | 1× Fruit Salad | Light, sweet recovery |
| **Sweet Cake** | Milk Bucket + Egg + Sugar + Wheat | 1× Sweet Cake | High-tier dessert (returns empty Bucket) |
| **Herbal Tea** | Water Bucket + Sugar + (Leaves *or* Poppy *or* Dandelion) | 2× Herbal Tea | **Cures Food Poisoning & Poison**; drinkable even at full hunger (returns empty Bucket) |

---

## Hunger, Saturation & Eating

### Hold-to-Eat Interaction
- **Hold `Right Click`** while holding any edible food or drink (~**1.2 seconds** for standard foods, **0.8 seconds** for Cookies) to consume it.
- While eating, you play a first-person chomp animation with crunch audio, move at reduced speed, and see a **progress bar** under the crosshair.
- Releasing `Right Click` or switching hotbar slots cancels eating without consuming the item (taking damage does *not* cancel eating).
- **Farming Skill Bonus**: Higher Farming skill levels boost hunger and saturation restored by foods.

### Reading the Hunger & Saturation Bar (`ST`)
- The HUD `"ST"` gauge uses a **two-tone fill** so you can read both Hunger and Saturation at a glance:
  - **Bright Gold** (`0 .. saturation`): Your current **Saturation** buffer (must deplete before Hunger drops).
  - **Warm Orange** (`saturation .. hunger`): Your remaining **Hunger** (`0–20`).
- Cooked meals and stews provide dramatically higher **Saturation** than raw crops or snacks, keeping you healed and sprinting far longer between meals.

### Status Effects & Cures
Unsafe foods can inflict lightweight negative status effects with dedicated HUD feedback and countdown timers:
- **Food Poisoning**: Inflicted by **Rotten Flesh** (80% chance) and **Raw Chicken** (30% chance). Tints the `"ST"` bar sickly green and rapidly drains hunger/saturation via exhaustion.
- **Poison**: Inflicted by **Pufferfish** (100% chance). Tints the HP bar green and ticks non-lethal damage down to 1 HP.
- **Cure**: Drink **Herbal Tea** (brewed in the Cooking Pot) to immediately cleanse both Food Poisoning and Poison. *(Note: **Milk Buckets** are strictly cooking ingredients and are not drinkable.)*
