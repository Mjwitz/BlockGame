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



