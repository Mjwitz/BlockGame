# Gameplay and Survival

## Health, Armor, Hunger & Drowning
- **Health & Hurt Audio**: Displayed as hearts on the HUD. Taking fall damage, drowning, or monster hits reduces health and triggers a visceral, punchy impact sound (blunt physical transient + 55 Hz sub-punch + human vocal grunt). When hunger is high, health regenerates slowly over time.
- **Hunger**: Displayed as food icons on the HUD. Sprinting and healing consume hunger; eating food (Bread, Apples, Cooked/Raw meats, etc.) with `Right Click` restores hunger.
- **Armor**: Displayed above the health bar when equipped. Equip Helmet, Chestplate, Leggings, and Boots in the 4 armor slots on the inventory screen (`E`). Available tiers: **Leather**, **Iron**, **Gold**, and **Diamond**.
- **Drowning**: Diving underwater displays an oxygen bubble bar. Surfacing refills oxygen; running out of oxygen deals drowning damage.

## Tools, Weapons & Durability
- **Tool Tiers**: Wood, Stone, Iron, Gold, and Diamond for Pickaxes, Axes, Shovels, Hoes, and Swords.
- **Mining Hardness**: Blocks have material hardness and preferred tool types (e.g., Pickaxe for Stone/Ores, Axe for Wood, Shovel for Dirt/Sand/Gravel).
- **Bow & Arrow**: Craft a Bow and Arrows, then `Right Click` with the Bow selected to fire arrows at mobs or targets.

## Crafting, Smelting & Storage
- **2x2 Inventory Crafting**: Available anytime by pressing `E`.
- **3x3 Crafting Table**: Place and `Right Click` to unlock full 3x3 recipes (tools, armor, beds, bows, buckets, chests, furnaces).
- **Furnace**: Place and `Right Click` to smelt ores and cook food using fuel (Coal, Wood Planks, Logs, Sticks).
- **Chest**: Place and `Right Click` for persistent 27-slot item storage.

## Day/Night Cycle & Sleeping in Beds
- A full day/night cycle spans `24000` ticks, with a **Sun** and **Moon** moving across the sky to show the time of day.
- **Beds** are 2 blocks long and can be used at dusk/night (`Right Click` on the bed) to sleep and skip to morning:
  - **Singleplayer**: Immediately advances time to morning.
  - **Multiplayer**: Advances time to morning once **at least 50%** of online players are sleeping in beds.
- **Multiplayer Pause Behavior**: In multiplayer servers, opening the Pause Menu (`Esc`) does **not** stop time or freeze the world. You remain in-world and can be attacked or take damage while paused.

## Ambient Music & Contextual Dynamic Soundtrack

BlockGame features a procedural dynamic soundtrack with 6 distinct musical compositions that adapt to what you are doing in the world:

| Track | Name | Duration | Description & Trigger Context |
| :--- | :--- | :--- | :--- |
| **Track 0** | *Felt Reverie* | ~24s | Calm felt piano and warm ambient pads for daytime building |
| **Track 1** | *Solitude* | ~24s | Gentle reflective melodic piano motif |
| **Track 2** | *Awakening* | ~24s | Bright morning ambient progression |
| **Track 3** | *Overland Wanderer* | **2.5 min** | 108 BPM upbeat walking bass, acoustic guitar arpeggios, and expressive flute lead. Triggers when **running / sprinting across the surface in daylight**. |
| **Track 4** | *Echoes of the Deep* | **2.5 min** | 64 BPM cavernous 48–55 Hz sub-bass drone and echoing crystal bell reflections. Triggers when **underground mining**. |
| **Track 5** | *Starlight Vista* | **2.5 min** | 72 BPM lush celestial pad chord progressions with twinkling celesta bells for dusk and stargazing. |

The smart scheduler ensures tracks do not repeat back-to-back and provides natural, atmospheric pauses between compositions. Track volume can be adjusted via the `Music Volume` slider in Settings.

