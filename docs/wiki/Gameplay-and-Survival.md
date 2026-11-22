# Gameplay and Survival

## Health, Armor, Hunger & Drowning
- **Health & Hurt Audio**: Displayed as hearts on the HUD. Taking fall damage, drowning, or monster hits reduces health and triggers a visceral, punchy impact sound (blunt physical transient + 55 Hz sub-punch + human vocal grunt). When hunger is high, health regenerates slowly over time.
- **Hunger, Saturation & Status Effects**: Displayed on the HUD `"ST"` gauge with a **two-tone fill** (bright gold for **Saturation**, warm orange for **Hunger**). Sprinting and healing consume saturation first, then hunger. **Hold `Right Click`** (~1.2s) with food to eat with animation, crunch audio, and a crosshair progress bar. Unsafe foods (Rotten Flesh, Raw Chicken, Pufferfish) can apply **Food Poisoning** or **Poison**, which can be cured with brewed **Herbal Tea**. See **[Farming and Cooking](Farming-and-Cooking)** for full details.
- **Armor**: Displayed above the health bar when equipped. Equip Helmet, Chestplate, Leggings, and Boots in the 4 armor slots on the inventory screen (`E`). Available tiers: **Leather**, **Iron**, **Gold**, and **Diamond**.
- **Drowning**: Diving underwater displays an oxygen bubble bar. Surfacing refills oxygen; running out of oxygen deals drowning damage.

## Tools, Weapons & Durability
- **Tool Tiers**: Wood, Stone, Iron, Gold, and Diamond for Pickaxes, Axes, Shovels, Hoes, and Swords.
- **Mining Hardness**: Blocks have material hardness and preferred tool types (e.g., Pickaxe for Stone/Ores, Axe for Wood, Shovel for Dirt/Sand/Gravel).
- **Bow & Arrow**: Craft a Bow and Arrows, then `Right Click` with the Bow selected to fire arrows at mobs or targets.

## Crafting, Smelting, Cooking, Power & Storage
- **2x2 Inventory Crafting**: Available anytime by pressing `E`.
- **3x3 Crafting Table**: Place and `Right Click` to unlock full 3x3 recipes (tools, armor, beds, bows, buckets, chests, furnaces, cooking pots, power machines, and backpacks).
- **Furnace & Electric Furnace**: Smelt ores and roast simple foods (meats, fish, Baked Potatoes) with coal/wood fuel in a standard Furnace, or use EU power in an **Electric Furnace** (`-25 EU/s`).
- **Cooking Pot**: Craft with `5× Iron Ingot + 2× Planks` to cook multi-ingredient stews, omelettes, Sweet Cake, and **Herbal Tea** using a 4-slot shapeless tray and fuel. See **[Farming and Cooking](Farming-and-Cooking)**.
- **Upgraded Chests**: Store items in **Wooden** (27 slots), **Iron** (45 slots), **Gold** (63 slots), or **Diamond** (81 slots) Chests.
- **Powered Storage & Backpacks**: Link chests together using **Storage Cables** and a powered **Storage Terminal** with live search and sorting, or carry unique **Leather / Iron / Diamond Backpacks** (`B`). See **[Power and Storage](Power-and-Storage)** for full details.

## Day/Night Cycle & Sleeping in Beds
- A full day/night cycle spans `24000` ticks, with a **Sun** and **Moon** moving across the sky to show the time of day.
- **Beds** are 2 blocks long and can be used at dusk/night (`Right Click` on the bed) to sleep and skip to morning:
  - **Singleplayer**: Immediately advances time to morning.
  - **Multiplayer**: Advances time to morning once **at least 50%** of online players are sleeping in beds.
- **Multiplayer Pause Behavior**: In multiplayer servers, opening the Pause Menu (`Esc`) does **not** stop time or freeze the world. You remain in-world and can be attacked or take damage while paused.

## MMO Skills & Leveling (Levels 1 – 1000)

BlockGame features an MMO-style progression system saved **per world and per server** (stored with each world/server's player data). Press `K` to open the **Skills & Milestones** menu or use `/skills` in chat.

| Skill | How XP is Earned | Scaling Perks |
| :--- | :--- | :--- |
| **Combat** | Attacking and defeating monsters | Bonus attack damage, critical hit chance, lifesteal on kill, and damage reduction |
| **Woodcutting** | Chopping logs and tree blocks | Faster chopping speed and bonus log / leaf drops |
| **Mining** | Mining stone and ore blocks | Faster mining speed and bonus ore / stone drops |
| **Fishing** | Catching fish and treasure | Larger catch bar, faster bite speed, and higher rare-catch chance |
| **Digging** | Shoveling dirt, sand, gravel, and clay | Faster digging speed and bonus buried drops |
| **Farming** | Harvesting crops | Bonus crop yield and seed drops |
| **Agility** | Sprinting and swimming | Increased movement speed and reduced fall damage |

Perks start small at early levels and grow stronger as you approach **Level 1000**, with major **Milestone Unlocks** at levels **25, 50, 100, 250, 500, 750, and 1000**.

## Fishing & Tackle

Craft a Fishing Rod and cast into water with `Right Click` to start fishing:
- **4 Rod Tiers**: Wooden, Copper, Iron, and Diamond Fishing Rods — higher tiers increase durability, bite speed, catch bar size, and rare-fish odds.
- **Tackle Upgrades**: Combine **Lines**, **Lures** (such as the Cork Bobber), and **Bait** to customize bite speed, bar size, and treasure/rare fish chances.
- **Interactive Minigame**: When a fish bites, reel in by keeping the catch bar aligned with the fish indicator. Catch **10 distinct fish species** as well as rare treasure items.

## Creative Mode

When creating a singleplayer world, you can choose between **Survival** and **Creative** mode. In multiplayer, admins can switch players with `/gamemode <survival|creative|0|1> [player]`. Creative Mode grants instant block breaking, unlimited flight (`F`), and invulnerability.

## Ambient Music & Contextual Dynamic Soundtrack

BlockGame features a custom stereo MP3 soundtrack with 4 tracks that adapt to what you are doing in the world:

| Track | Title | Trigger Context |
| :--- | :--- | :--- |
| **Track 1** | *Where Maps End* | Triggers when **running / sprinting across the world adventuring**. |
| **Track 2** | *Measure of the Dark* | Triggers while **mining stone/ores or digging underground**. |
| **Track 3** | *Where the Roots Break Soil* | **Anytime background** track. |
| **Track 4** | *The View From The Ridge* | **Anytime background** track. |

The scheduler avoids repeating the same track back-to-back and provides natural atmospheric pauses between tracks. Track volume can be adjusted via the `Music Volume` slider in Settings.

