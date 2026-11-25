# Achievements

BlockGame features **42 profile-wide Achievements** spanning all 8 gameplay pillars plus **5 Hidden Achievements**. Unlocks and lifetime **Career Stats** are stored globally in your local profile (`achievements.yaml`), so progress carries across all eligible singleplayer worlds and multiplayer servers and maps 1:1 to future Steam achievements.

## Viewing & Unlocking Achievements

- **How to open**: Open the **Achievements** screen from the **Main Menu**, the **Pause Menu**, or by pressing **`J`** in-game (remappable in **Settings → Controls**).
- **HUD Toasts**: Unlocking an Achievement displays a queued top-right banner with an item icon, title, description, and celebratory chime. Simultaneous unlocks play sequentially in FIFO order.
- **Multiplayer Chat Broadcasts**: On multiplayer servers with `broadcast-achievements=true` in `server.properties` (enabled by default), unlocks are announced in chat (`<Player> unlocked [Title]!`) without spoiling Hidden Achievement descriptions.

## Achievement Eligibility

Achievements and Career Stats are earned only during eligible **Survival** play:

- **Creative Mode**: Worlds created in Creative Mode start with **Achievement Eligibility** disabled.
- **Multiplayer Admin / Cheat Commands**: Executing cheat commands (`/give`, `/gamemode`, `/tp`, `/tphere`, `/skills set|addxp|reset`, `/resetplayer`) disables Achievement Eligibility for the **targeted player's session** without affecting other Survival players on the server.
- **Allowed Utility Commands**: Standard navigation and inspection commands (`/spawn`, `/sethome`, `/home`, `/tpa`, `/skills`) preserve Achievement Eligibility.

---

## Achievement Roster

### Survival & Crafting (5)

| Achievement | Requirement | Tracking |
| :--- | :--- | :--- |
| **Timber!** | Chop your first log. | One-shot |
| **Getting to Work** | Craft a Crafting Table. | One-shot |
| **Into the Fire** | Smelt an ingot in a Furnace. | One-shot |
| **Sweet Dreams** | Sleep through the night in a bed. | One-shot |
| **Shining Knight** | Equip a full set of Diamond Armor. | One-shot |

### Mining & Exploration (5)

| Achievement | Requirement | Tracking |
| :--- | :--- | :--- |
| **Diamonds!** | Mine your first Diamond Ore. | One-shot |
| **Earthmover** | Mine 1,000 blocks. | Career Stat (`1,000`) |
| **Mountain Breaker** | Mine 10,000 blocks. | Career Stat (`10,000`) |
| **Wanderer** | Visit 10 different biomes. | Career Stat (`10 / 20`) |
| **World Explorer** | Visit all 20 biomes. | Career Stat (`20 / 20`) |

### Farming, Ranching & Cooking (5)

| Achievement | Requirement | Tracking |
| :--- | :--- | :--- |
| **Green Thumb** | Harvest 50 mature crops. | Career Stat (`50`) |
| **Rich Soil** | Enrich farmland with Fertilizer. | One-shot |
| **Matchmaker** | Breed 5 animals using Breeding Feed. | Career Stat (`5`) |
| **Home Cook** | Prepare a meal in a Cooking Pot. | One-shot |
| **Master Chef** | Prepare all 8 Cooking Pot dishes. | Career Stat (`8 / 8`) |

### Fishing (4)

| Achievement | Requirement | Tracking |
| :--- | :--- | :--- |
| **Hook, Line & Sinker** | Catch your first fish. | One-shot |
| **Seasoned Angler** | Catch 50 fish. | Career Stat (`50`) |
| **Master Angler** | Catch all 10 fish species. | Career Stat (`10 / 10`) |
| **Top of the Line** | Craft or obtain a Diamond Fishing Rod. | One-shot |

### Combat & Biome Bosses (7)

| Achievement | Requirement | Tracking |
| :--- | :--- | :--- |
| **Monster Hunter** | Defeat your first hostile monster. | One-shot |
| **Night Watch** | Defeat 100 hostile monsters. | Career Stat (`100`) |
| **Sun Slayer** | Defeat the Sun Pharaoh in the Pyramid of the Blazing Sun. | One-shot |
| **Icebreaker** | Defeat the Frost Titan in the Citadel of Eternal Frost. | One-shot |
| **Weed Whacker** | Defeat the Overgrown Golem in the Ziggurat of the Wilds. | One-shot |
| **Light in the Dark** | Defeat the Shadow Lich in the Cathedral of Whispers. | One-shot |
| **Conqueror of the Realms** | Defeat all 4 Biome Bosses. | Career Stat (`4 / 4`) |

### Villages & Trading (4)

| Achievement | Requirement | Tracking |
| :--- | :--- | :--- |
| **Open for Business** | Complete your first trade with a Shopkeeper. | One-shot |
| **Big Spender** | Spend 500 Gold Coins with Shopkeepers. | Career Stat (`500`) |
| **VIP Customer** | Reach Shop Tier 5 with any Shopkeeper. | One-shot |
| **Upgraded** | Apply an Enhancement to a piece of gear. | One-shot |

### Power & Automation (4)

| Achievement | Requirement | Tracking |
| :--- | :--- | :--- |
| **Power Up** | Place your first EU generator (Coal Generator, Solar Panel, Advanced Solar Panel, or Wind Turbine). | One-shot |
| **High Voltage** | Smelt an item in an Electric Furnace. | One-shot |
| **Digital Hoarder** | Open a powered Storage Terminal connected to a chest. | One-shot |
| **Pack Rat** | Craft a Diamond Backpack. | One-shot |

### MMO Skills (3)

| Achievement | Requirement | Tracking |
| :--- | :--- | :--- |
| **Apprentice** | Reach level 25 in any skill. | One-shot |
| **Journeyman** | Reach level 100 in any skill. | One-shot |
| **Grandmaster** | Reach level 500 in any skill. | One-shot |

---

## Hidden Achievements (5)

In-game, locked **Hidden Achievements** appear as `???` until you discover them. Expand the spoiler below if you want to see their titles and unlock criteria.

<details>
<summary><strong>Click to reveal Hidden Achievements (Spoilers)</strong></summary>

<br>

| Achievement | Requirement | Tracking |
| :--- | :--- | :--- |
| **Doctor's Orders** | Cure Food Poisoning or Poison by drinking Herbal Tea. | One-shot |
| **Wanted** | Provoke a Village Guard by attacking a Village NPC. | One-shot |
| **Got Milk?** | Milk a cow with a Bucket. | One-shot |
| **Bountiful Vine** | Harvest a Melon or Pumpkin grown from a Stem Crop. | One-shot |
| **Trophy Collector** | Collect all 4 Boss Trophy Cores. | Career Stat (`4 / 4`) |

</details>
