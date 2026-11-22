# Mobs and Breeding

## Passive Animals & Ranching
Four passive animal species roam the world and tie directly into farming and cooking:

| Animal | Drops | Breeding Feed (Lure & Breed) | Special Ranching Behavior |
| :--- | :--- | :--- | :--- |
| **Cow** | Leather, Raw Beef | **Wheat** | `Right Click` with an empty **Bucket** to collect a **Milk Bucket** (used in Cooking Pot recipes like Sweet Cake) |
| **Pig** | Raw Porkchop | **Carrot** or **Potato** | — |
| **Chicken** | Feathers, Raw Chicken | Any **Seeds** (Wheat, Tomato, Melon, Pumpkin) | Passively lays **Eggs** every 5–10 minutes |
| **Sheep** | Wool (Beds), **1–2 Raw Mutton** | **Wheat** | Raw Mutton can be roasted into Cooked Mutton or cooked in Hearty Meat Stew |

### Herding & Animal Breeding
1. **Herding**: Hold a species' preferred **Breeding Feed** in your active hotbar slot to lure nearby animals of that species within **10 blocks** to follow you into pens.
2. **Breeding**: `Right Click` two adult animals of the same species with their Breeding Feed.
3. Both animals enter love mode (floating heart particles) and produce a **baby animal** that grows into an adult over time.
4. After breeding, animals have a **5-minute (300-second) cooldown** before they can breed again.
5. **Persistence & Multiplayer**: Animal love timers, breed cooldowns, baby growth timers, and Chicken egg timers persist across world saves (`entities.bin`), and animal simulation runs authoritatively on the server in multiplayer.

---

## Hostile Monsters
Three hostile monster types spawn in darkness and at night:

| Monster | Behavior | Drops |
| :--- | :--- | :--- |
| **Zombie** | Melee pursuer; chases nearby players and deals contact damage. | Rotten Flesh |
| **Skeleton** | Ranged archer; maintains distance and fires arrows at players. | Bones, Arrows |
| **Spider** | Fast, low-profile melee attacker. | String |

### Spawning & Light Rules
- **Darkness & Night Spawning**: Monsters spawn at night/dusk or in dark underground caves where blocklight and skylight are low.
- **Torches Prevent Spawns**: Placing Torches raises local blocklight and prevents monsters from spawning nearby.
- **Leaf Light Transmission**: Sky light passes through tree leaves, preventing daytime monster spawns under trees.
- **Sunlight Burning**: Zombies and Skeletons catch fire in direct daytime sunlight and display **animated 3D flame visuals** while burning.

---

## Creature Sounds & Spatial Audio

Every animal and monster features procedural sound design for both ambient life and combat interactions:

| Creature | Ambient Sound | Hurt Sound |
| :--- | :--- | :--- |
| **Cow** | Low harmonic resonant "Moo" | Sharp stressed guttural groan |
| **Pig** | Frequency-modulated snorting "Oink" | High-pitched distress squeal |
| **Chicken** | Short rhythmic multi-burst "Cluck" | Frantic high-pitched flapping squawk |
| **Sheep** | Formant-filtered trembling "Baa" | Quick rising distress bleat |
| **Zombie** | Low guttural undead throat moan | Visceral raspy undead groan |
| **Skeleton** | Dry percussive bone-rattle clatter | Sharp crack of striking bone |
| **Spider** | Dual-band noise-filtered hiss | Aggressive snapping chitin screech |

### 3D Spatial Audio & Volume Settings
- **Spatial Positioning**: Creature sounds are positioned in 3D relative to the player's head orientation and distance, giving full directional awareness of approaching mobs or grazing herds.
- **Creature Volume Slider**: You can adjust animal and monster audio volume independently in **Settings -> Creature Volume** without altering footsteps or music.

