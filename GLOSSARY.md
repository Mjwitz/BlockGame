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
