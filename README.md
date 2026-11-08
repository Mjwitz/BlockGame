# BlockGame 🧱

A high-performance procedural voxel sandbox engine inspired by Minecraft, engineered in **Rust** with **wgpu** for modern graphics and cross-platform native + WebAssembly execution.

---

## 🌟 Architectural Highlights

- **Voxel Engine**: Voxels represented as compact data arrays (`u8` identifiers) in $16 \times 16 \times 16$ chunks, with bitwise coordinate indexing.
- **Rendering**: Hidden-face culling, greedy meshing, vertex ambient occlusion, and packed vertex buffers rendered via modern WebGPU / Vulkan / Metal pipelines.
- **Procedural World Generation**: Deterministic seed-based multi-octave 2D heightmaps, 3D cave noise carving, temperature/moisture biome mapping, and asynchronous multithreaded generation.
- **Physics & Interaction**: Axis-Aligned Bounding Box (AABB) continuous collision response and Amanatides-Woo Fast Voxel Traversal (DDA) raycasting for instant block placement and destruction.
- **Atmospheric Lighting**: BFS flood-fill voxel light propagation (sunlight and emissive blocklight) coupled with a dynamic day/night celestial cycle and distance fog.
- **Platform Support**: Native desktop (Linux, Windows, macOS) and web browser via WebAssembly (`wasm32-unknown-unknown`).

---

## 📁 Repository Structure

```text
BlockGame/
├── docs/
│   ├── ARCHITECTURE.md     # In-depth architectural blueprint and technical specifications
│   └── ROADMAP.md          # 5-stage milestone roadmap and progress checklist
├── crates/
│   ├── blockgame_core/     # Pure voxel data structures, AABB physics, DDA raycasting, items
│   ├── blockgame_gen/      # Procedural noise, heightmaps, caves, biomes, feature placement
│   ├── blockgame_render/   # wgpu rendering pipeline, meshing, shaders, texture atlas
│   └── blockgame_client/   # Native winit desktop app & WebAssembly entrypoints
├── assets/                 # Textures, shaders (WGSL), and sounds
└── Cargo.toml              # Cargo workspace definition
```

---

## 🚀 Development Milestones

1. **Milestone 1**: The Cube & The Chunk — $16 \times 16 \times 16$ chunk meshing, face culling, texture atlas, and wgpu pipeline.
2. **Milestone 2**: Infinite World — Deterministic terrain generator, dynamic chunk loader, and multithreading.
3. **Milestone 3**: Player Physics & Interaction — AABB player physics, DDA raycasting, block breaking & placing.
4. **Milestone 4**: Atmosphere & Lighting — Dynamic day/night cycle, skybox, BFS sunlight/blocklight propagation.
5. **Milestone 5**: Gameplay Loop — Inventory, hotbar, drag-and-drop UI, crafting engine, item drops.

Detailed specifications and progress criteria are documented in [docs/ROADMAP.md](docs/ROADMAP.md) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## 🛠️ Building & Running

### Prerequisites
- [Rust toolchain](https://www.rust-lang.org/) (edition 2021 or 2024, stable)
- Vulkan, DirectX 12, or Metal graphics drivers

### Native Desktop
```bash
cargo run --bin blockgame_client
```

### WebAssembly (WASM)
```bash
# Add WebAssembly target
rustup target add wasm32-unknown-unknown

# Build for web using wasm-pack / trunk
trunk serve
```

---

## 📜 License
Licensed under either of Apache License 2.0 or MIT license at your option.
