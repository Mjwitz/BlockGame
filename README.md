# BlockGame

A procedural voxel sandbox survival game inspired by Minecraft, written in **Rust** with **wgpu** for modern cross-platform graphics on **Linux** and **Windows** (plus dedicated multiplayer server support).

![BlockGame Procedural Landscape](landscape_render.png)

---

## Features

- **Voxel Engine & Lighting**: $16 \times 16 \times 16$ chunked world with hidden-face culling, greedy meshing, vertex ambient occlusion, and BFS flood-fill sunlight/blocklight propagation (including sky light passing through leaves).
- **16 Procedural Biomes**: Plains, Forest, Desert, Mountains, Snowy Tundra, Swamp, Savanna, Ocean, Jungle, Badlands, Beach, River, Taiga, Mushroom Fields, Dark Forest, and Coral Reef — each with distinct terrain, vegetation, and atmospheric fog.
- **Day/Night Cycle & Beds**: Dynamic skybox with moving sun and moon, dusk-to-dawn monster spawning, and 2-block-long beds to skip the night (requires $\ge 50\%$ of players sleeping in multiplayer).
- **Survival, Crafting & Smelting**:
  - Health, hunger, fall damage, drowning, and underwater visual effects.
  - 2x2 inventory crafting, 3x3 Crafting Table, Furnace smelting, Chests, and an integrated JEI-style recipe/usage browser (`R` / `U` on hovered items).
  - Tool durability across Wood, Stone, Iron, Gold, and Diamond tiers, plus 16-piece Armor sets (Leather, Iron, Gold, Diamond) and Bow & Arrow combat.
- **Passive & Hostile Mobs**:
  - **Passive Animals**: Cows, Pigs, Chickens, and Sheep (shearing/wool drops, plus Wheat breeding with baby scaling, heart particles, and a 5-minute cooldown).
  - **Hostile Monsters**: Zombies, Skeletons (ranged bow AI), and Spiders with light-gated spawning and animated 3D flame visuals when burning in sunlight.
- **Multiplayer & Dedicated Server**: TCP client/server architecture (`blockgame_server`) with modular world saves, player accounts, groups/ranks, permissions, and server console/in-game slash commands.

---

## Repository Structure

```text
BlockGame/
├── .github/workflows/
│   └── release.yml         # Automated Windows & Linux release builds
├── crates/
│   ├── blockgame_core/     # Voxels, chunks, AABB physics, DDA raycasting, mobs, items, net, commands
│   ├── blockgame_gen/      # Seeded terrain noise, caves, 16 biomes, trees & feature generation
│   ├── blockgame_render/   # wgpu pipelines, chunk/mob meshing, celestial sky, 32x atlas, UI
│   ├── blockgame_client/   # Desktop client (winit + wgpu + rodio audio)
│   └── blockgame_server/   # Headless dedicated multiplayer server CLI
├── docs/
│   ├── ARCHITECTURE.md     # Technical architecture blueprint
│   ├── ROADMAP.md          # Early development milestone checklist
│   └── wiki/               # Source pages for the GitHub Wiki
├── assets/                 # Shaders (WGSL), textures, audio assets
└── Cargo.toml              # Cargo workspace configuration
```

---

## Getting Started

### Option 1: Download Prebuilt Executables
Download the latest archive from the [GitHub Releases](https://github.com/Mjwitz/BlockGame/releases) page:
- **Linux (`x86_64`)**: `BlockGame-linux-x86_64.tar.gz` (`blockgame_client`, `blockgame_server`)
- **Windows (`x86_64`)**: `BlockGame-windows-x86_64.zip` (`blockgame_client.exe`, `blockgame_server.exe`)

### Option 2: Build from Source

#### Prerequisites
- [Rust toolchain](https://www.rust-lang.org/) (stable)
- Vulkan, DirectX 12, or Metal compatible GPU drivers (for `blockgame_client`)
- **Linux build dependencies** (Debian/Ubuntu):
  ```bash
  sudo apt-get update
  sudo apt-get install -y build-essential pkg-config libasound2-dev \
    libx11-dev libxi-dev libxcursor-dev libxrandr-dev libwayland-dev libxkbcommon-dev
  ```

#### Build & Run
```bash
# Build release binaries for both client and server
cargo build --release -p blockgame_client -p blockgame_server

# Run the game client
cargo run --release -p blockgame_client

# Run the dedicated multiplayer server
cargo run --release -p blockgame_server -- --port 25565 --world "MyWorld" --seed 1337
```

---

## Quick Controls

| Key / Input | Action |
| :--- | :--- |
| `W A S D` | Move |
| `Space` | Jump / Swim up / Fly up |
| `Left Ctrl` | Sprint |
| `Left Click` | Break block / Attack |
| `Right Click` | Place block / Interact (Bed, Chest, Furnace, Crafting Table) / Shoot Bow / Feed Animal |
| `1` - `9` / Scroll | Select hotbar slot |
| `E` | Open / Close Inventory |
| `R` / `U` | View Recipe / Usages for hovered item (in Inventory) |
| `T` | Open Chat / Slash Commands |
| `F` | Toggle Flight mode |
| `F5` | Toggle First-person / Third-person camera |
| `Tab` | Show online player list (Multiplayer) |
| `Esc` | Close UI / Pause Menu |

---

## Documentation

For detailed guides on gameplay, biomes, mobs, multiplayer server hosting, commands, permissions, and building releases, visit the **[BlockGame Wiki](https://github.com/Mjwitz/BlockGame/wiki)** (source files also tracked in [`docs/wiki/`](docs/wiki/Home.md)).

---

## License

Licensed under either of [Apache License 2.0](http://www.apache.org/licenses/LICENSE-2.0) or [MIT License](http://opensource.org/licenses/MIT) at your option.
