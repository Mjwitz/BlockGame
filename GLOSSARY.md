# BlockGame

A Rust/wgpu voxel sandbox game supporting singleplayer worlds and multiplayer servers.

## Language

### Character Customization

**Character Appearance**:
The complete visual definition of a player's avatar—body frame, skin tone, facial features, hairstyle, and base clothing with their colors.
_Avoid_: Skin, avatar config, player look

**Character Preset**:
A named, client-saved Character Appearance stored in the player's local profile that can be selected before entering a singleplayer world or multiplayer server.
_Avoid_: Saved skin, loadout, character slot

**Body Frame**:
One of the three anatomical silhouette presets (Masculine, Feminine, Neutral) that shapes the character's proportions without changing the gameplay hitbox or restricting hair and clothing choices.
_Avoid_: Gender, body type, sex

**Base Clothing**:
Purely cosmetic apparel (shirt, pants, shoes) defined on a Character Appearance, completely separate from inventory items and gameplay armor.
_Avoid_: Armor, gear, outfit items

**Starter Preset**:
One of the built-in Character Presets shipped with the game (spanning all Body Frames) that players can select directly or use as a starting point for customization.
_Avoid_: Default skin, hardcoded skin

**Character Portrait**:
A 2D face graphic derived procedurally from a player's Character Appearance for display in menus and multiplayer player lists.
_Avoid_: Face tile, skin icon, avatar thumbnail

### Farming, Cooking, and Hunger

**Breeding Feed**:
The species-specific crop or seed item that lures an animal toward a player holding it and places the animal into love mode for breeding.
_Avoid_: Animal food, bait, lure item

**Fertilizer**:
A passive soil amendment applied to farmland that enriches the next harvest cycle rather than triggering instant crop growth.
_Avoid_: Bone meal, instant grow

**Cooking Pot**:
A fueled cooking station that combines multiple ingredients into a prepared meal.
_Avoid_: Stove, oven, cauldron

**Stem Crop**:
A crop whose mature stem produces a separate harvestable fruit block on an adjacent tile while the stem remains intact.
_Avoid_: Vine crop, bush crop

**Food Poisoning**:
A temporary negative status effect from unsafe food that accelerates hunger drain.
_Avoid_: Hunger debuff, nausea

**Poison**:
A temporary negative status effect that deals periodic non-lethal health damage.
_Avoid_: Venom, bleed

**Saturation**:
The visible energy reserve displayed as a bright gold fill within the Hunger gauge that drains before Hunger itself decreases.
_Avoid_: Hidden hunger, fullness buffer

### Villages & Trading

**Village**:
A rare, biome-themed settlement generated in the overworld with a central square, shops, Shopkeepers, and Village Guards.
_Avoid_: Town, settlement, outpost

**Shopkeeper**:
A mortal, profession-themed NPC tethered to a Village shop who trades with players for Gold Coins and unlocks higher-tier offers per individual player.
_Avoid_: Villager, merchant, vendor, trader

**Village Guard**:
A mortal combat NPC stationed in a Village who defends it against hostile monsters and retaliates against players who attack any Village NPC.
_Avoid_: Town guard, sentry, defender

**Gold Coin**:
The physical currency item earned by selling goods to Shopkeepers and spent to purchase items or apply Enhancements.
_Avoid_: Money, emerald, wallet

**Enhancement**:
A shop-exclusive modifier applied to an existing tool, weapon, armor piece, or fishing rod (requiring the base item and Gold Coins) that grants a permanent gameplay bonus.
_Avoid_: Enchantment, affix, perk

**Shop Tier**:
A per-player, per-Shopkeeper progression rank (Tiers 1–5) leveled by spending Gold Coins with that Shopkeeper to unlock higher-tier goods, unique items, and Enhancements.
_Avoid_: Merchant level, reputation, shop rank

### Achievements

**Achievement**:
A profile-wide milestone earned during eligible Survival play, tracked globally across all singleplayer worlds and multiplayer servers and mapped 1:1 to a Steam achievement.
_Avoid_: Advancement, trophy, badge, quest

**Career Stat**:
A lifetime cumulative counter in a player's profile that aggregates eligible gameplay activity across worlds and servers to drive progress-based Achievements and Steam stats.
_Avoid_: Player stat, world stat, counter, metric

**Hidden Achievement**:
An Achievement whose title and unlock criteria remain obscured in the Achievements screen until the player earns it.
_Avoid_: Secret achievement, mystery badge, easter egg

**Achievement Eligibility**:
The permanent Survival-only status of a world or multiplayer session that permits earning Achievements and incrementing Career Stats, forfeited if Creative mode or admin/cheat commands are used.
_Avoid_: Cheat flag, survival lock, ranked mode

### Controller & Input

**Input Mode**:
The currently active control scheme (`Keyboard & Mouse` or `Controller`), switched automatically on the most recent input to drive cursor behavior and on-screen prompts.
_Avoid_: Control mode, device mode, gamepad mode

**Controller Family**:
One of the five recognized controller hardware profiles (`Xbox`, `PlayStation`, `Nintendo`, `Steam Deck`, `Steam Controller`) inferred from a connected gamepad's identity to choose default glyphs and menu conventions.
_Avoid_: Controller type, pad brand, device model

**Glyph Style**:
The button-icon set (`Auto`, `Xbox`, `PlayStation`, `Nintendo`, `Steam Deck`, or `Steam Controller`) shown in HUD prompts, tooltips, and menus, either derived from the active Controller Family or overridden in Settings.
_Avoid_: Button icons, prompt theme, icon pack

**Virtual Cursor**:
The left-stick-driven pointer used on menus, inventory, and container screens in Controller Input Mode, paired with D-pad/bumper focus snapping and face-button shortcuts.
_Avoid_: Fake mouse, gamepad mouse, software cursor

**Active Controller**:
The single connected gamepad currently driving Controller Input Mode, dynamically reassigned whenever another connected gamepad produces input.
_Avoid_: Primary gamepad, slot 0, player 1 controller

### Underground & Caves

**Cave Mouth**:
A natural surface opening on dry land where an underground tunnel breaches the terrain, sealed off from ocean and river beds so surface water does not flood the cave.
_Avoid_: Cave entrance, surface hole, tunnel opening

**Ore Vein**:
A compact, irregular underground cluster of matching ore blocks whose spawn frequency and cluster size follow a depth-dependent curve.
_Avoid_: Ore blob, ore pocket, ore sphere

**Cave Archetype**:
One of the three underground carving shapes (`Winding Passage`, `Vaulted Cavern`, or `Fissure`) combined within a cave network to vary exploration.
_Avoid_: Cave type, tunnel style, cave shape

**Cave Zone**:
The regional underground atmosphere (`Dry`, `Damp`, or `Deep Volcanic`) governing cave wall materials, moisture, pools, and decorative formations.
_Avoid_: Cave biome, underground biome, cave theme

**Deepstone**:
A darker, tougher stone stratum forming the deepest underground layer where Diamonds and Lava Pools concentrate.
_Avoid_: Deepslate, dark stone, bedrock stone

**Dripstone**:
Pointed mineral formations hanging from cave ceilings (stalactites) or rising from cave floors (stalagmites).
_Avoid_: Cave spike, icicle rock, speleothem

**Glow Crystal**:
A rare, faintly luminous crystalline formation found on cave walls and ceilings that serves as a natural underground landmark.
_Avoid_: Cave crystal, glowstone, luminescent gem

**Lava Pool**:
A natural underground basin of molten Lava collected in the lowest cave floors.
_Avoid_: Magma lake, lava lake, fire pool

**Burning**:
A temporary status effect triggered by contact with Lava that deals periodic damage after exiting Lava until it expires or is extinguished by Water.
_Avoid_: On fire, ignite, scorch
