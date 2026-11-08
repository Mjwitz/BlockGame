# BlockGame Development Roadmap & Tracking

This document outlines the 5 development milestones for **BlockGame**, with clear deliverables and technical acceptance criteria for each phase.

---

## 🚩 Milestone 1: The Cube & The Chunk (Foundation)
**Objective:** Render a single $16 \times 16 \times 16$ chunk of blocks using a single unified mesh with hidden-face culling, ambient occlusion, and a texture atlas.

- [x] **Workspace Setup & Core Types**
  - [x] Initialize Cargo workspace (`blockgame_core`, `blockgame_render`, `blockgame_client`).
  - [x] Implement `BlockId`, `BlockRegistry`, and `Chunk` ($16 \times 16 \times 16$ flat array).
  - [x] Fast 3D to 1D index mapping: `x + (z * 16) + (y * 256)`.
- [x] **Meshing & Culling**
  - [x] Neighbor inspection algorithm (6 directions: $+X, -X, +Y, -Y, +Z, -Z$).
  - [x] Hidden-face culling (omit internal faces between opaque voxels).
  - [x] Quad generation with compact vertex structure (`u8` packed attributes).
  - [x] Vertex Ambient Occlusion (query 3 corner neighbors per quad vertex).
- [x] **Rendering Pipeline (wgpu)**
  - [x] Initialize `wgpu::Device`, `wgpu::Queue`, and `wgpu::Surface`.
  - [x] Create texture atlas loader / generator with 16x16 pixel textures.
  - [x] WGSL block shader with texture sampling and basic directional shading.
  - [x] Camera matrix and orbit/free-look controls for inspection.
  - [x] Acceptance criteria: Render a test chunk with grass, dirt, stone, and wood with high FPS and zero duplicate or internal faces.

---

## 🚩 Milestone 2: Infinite World & Procedural Generation
**Objective:** Add a flying spectator camera, dynamic chunk loading/unloading around the player, and deterministic 2D/3D noise terrain.

- [x] **World Manager & Spatial Partitioning**
  - [x] `ChunkPos(i32, i32, i32)` and `World` coordinate transforms.
  - [x] Hash-map or spatial grid of loaded chunks.
  - [x] Render distance radius loader (load chunks in radius $R$, unload at $R + 2$).
- [x] **Procedural Terrain Pipeline**
  - [x] Deterministic seed-based Perlin/Simplex noise generation.
  - [x] 2D Fractal Brownian Motion (fBm) surface heightmap.
  - [x] Surface block distribution (Grass top, 3 layers Dirt, Stone beneath, Bedrock floor).
  - [x] 3D cave carving noise pass.
- [x] **Asynchronous Multithreaded Meshing**
  - [x] Background thread pool for chunk generation and mesh building (`rayon`).
  - [x] Message queue to deliver completed vertex/index buffers to render thread without frame drops.
  - [x] Neighbor chunk boundary resolution for seamless culling across borders.

---

## 🚩 Milestone 3: Player Physics & Interaction
**Objective:** First-person player controller with AABB voxel physics, gravity, jumping, DDA raycasting, and real-time block breaking/placing.

- [x] **Player Controller & AABB Physics**
  - [x] First-person camera with mouse look (pitch & yaw clamping).
  - [x] Player AABB ($0.6 \times 1.8 \times 0.6$ blocks) with velocity and gravity.
  - [x] Axis-separated swept AABB collision resolution against world voxels.
  - [x] Ground detection, jumping, and friction/damping.
- [x] **Voxel Raycasting (DDA)**
  - [x] Fast Voxel Traversal algorithm implementation.
  - [x] Target block detection up to reach distance (5 blocks).
  - [x] Target face normal calculation (identifies which face the player aims at).
  - [x] Wireframe bounding box overlay on targeted block.
- [x] **Block Modification**
  - [x] Left-click: Destroy targeted block (set to Air).
  - [x] Right-click: Place active block at target face position (with player collision check).
  - [x] Immediate dirty chunk marking and synchronous/fast mesh rebuild.
  - [x] Rebuild adjacent chunks if the modified block is on a chunk boundary ($x=0, 15$, etc.).

---

## 🚩 Milestone 4: Atmosphere, Lighting & Day/Night Cycle
**Objective:** Implement dynamic celestial day/night cycle, atmospheric skybox/fog, and 2-channel BFS flood-fill voxel lighting.

- [x] **Voxel Lighting Engine**
  - [x] Dual-channel light map: 4-bit Sunlight ($0\text{--}15$) + 4-bit Blocklight ($0\text{--}15$) packed in `u8`.
  - [x] Vertical sunlight casting from chunk ceiling to first opaque voxel.
  - [x] Breadth-First Search (BFS) light propagation queue (attenuation by 1 per step).
  - [x] BFS light flood-fill for lateral sunlight diffusion under trees/overhangs.
  - [x] Torch / emissive block placement (`BlockId::TORCH`, luminance 14) and local light propagation.
- [x] **Dynamic Day/Night Cycle & Atmosphere**
  - [x] Global `WorldTime` ticker ($0\text{--}24000$) with orbital sun/moon progression.
  - [x] Sun/moon directional light vectors and color shifts (golden noon, crimson dusk, pale moonlight).
  - [x] Dynamic gradient skybox and distance fog matching time of day and sun elevation.
  - [x] Shader dual-channel blending: $\max(\text{Sunlight} \times \text{SunFactor}, \text{Blocklight}) \times \text{AO} + \text{Ambient}$.
  - [x] Interactive hotkeys: `T` to advance time of day, `0` to equip torches.

---

## 🚩 Milestone 5: Gameplay Loop, Inventory & Crafting
**Objective:** Data-driven item system, inventory UI, hotbar, drag-and-drop, and 2D/3D recipe matching.

- [ ] **Item & Inventory Architecture**
  - [ ] Item registry: stack size, placeable block ID, durability, tool tier.
  - [ ] `ItemSlot` and `Inventory` structures (9 hotbar slots + 27 backpack slots).
  - [ ] Active hotbar slot selection with number keys ($1\text{--}9$) and mouse scroll.
- [ ] **UI Overlay (egui or custom Web/WASM UI)**
  - [ ] HUD crosshair and hotbar slot renderer with item counts.
  - [ ] Inventory screen with cursor stack, stack splitting, and quick-move.
- [ ] **Crafting Engine**
  - [ ] Shapeless recipe matcher (item multiset equality).
  - [ ] Shaped recipe matcher ($2 \times 2$ and $3 \times 3$ grid matching with offset normalization).
  - [ ] Basic survival recipes: Wood $\to$ Planks $\to$ Sticks $\to$ Wooden Pickaxe/Torches.
- [ ] **Entities & Polish**
  - [ ] Floating item drops with pickup magnetism and despawn timer.
  - [ ] Basic sound triggers (footsteps, dig, place).
  - [ ] Delta-compressed chunk saving to disk.
