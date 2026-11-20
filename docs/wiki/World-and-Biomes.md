# World and Biomes

## 20 Procedural Biomes
Worlds are generated deterministically from a 32-bit seed using domain-warped Voronoi macro cells (guaranteeing expansive, non-tiny biomes), multi-octave 2D heightmaps, 3D cave carving, and temperature/moisture noise. Biomes feature natural **relief contrast**—from flat grasslands, meadows, and deserts to towering mountains and canyons—and world spawn searches for dry land instead of spawning in rivers.

| Biome | Characteristics |
| :--- | :--- |
| **Plains** | Flat to gently rolling grasslands, flowers, and scattered oak trees |
| **Flower Meadow** | Vibrant wildflower fields with gentle hills |
| **Forest** | Dense oak/birch woodland |
| **Birch Forest** | Bright birch groves with light foliage |
| **Dark Forest** | Thick canopy with dark oak trees and shadowy fog |
| **Taiga** | Cool evergreen spruce forests |
| **Snowy Tundra** | Snow-covered terrain and frozen water |
| **Glacier** | Towering ice formations and frozen peaks |
| **Mountains** | High-altitude stone peaks and steep cliffs |
| **Desert** | Sand dunes, sandstone, and cacti |
| **Red Desert** | Sunlit red sand dunes and warm desert haze |
| **Badlands** | Terracotta canyons and red sand |
| **Savanna** | Golden grass and acacia-style warm plains |
| **Jungle** | Tall tropical trees and lush foliage |
| **Swamp** | Shallow murky water, clay, and moss-tinted fog |
| **Beach** | Sandy coastal shorelines |
| **Stony Shore** | Rocky coastal cliffs and gravel/stone beaches |
| **River** | Winding freshwater channels |
| **Ocean** | Deep marine basins |
| **Coral Reef** | Vibrant turquoise lagoons with coral structures |

## World Physics & Lighting
- **Voxel Lighting**: BFS flood-fill propagation for both **sky light** (0–15) and **emissive blocklight** (e.g., Torches, Furnaces, Sea Lanterns).
- **Translucent Water & Ice**: Water and Ice render in a dedicated translucent pass with proper depth sorting and seamless chunk-border face culling.
- **Water & Buckets**: Flowing water inflow physics and Buckets for picking up and placing water.
- **Gravity & Support**: Sand and Gravel fall when unsupported; attached blocks (torches, crops, flowers) break if their supporting block is removed.
- **Modular World Saves & Block Orientation Persistence**: Singleplayer and server worlds are stored under `saves/<world_name>/` with v2 chunk regions (preserving exact placed facing directions for Stairs, Beds, Furnaces, Chests, Wind Turbines, Generators, Battery Boxes, Electric Machines, and Storage Terminals), world metadata, power grids, unique backpacks (`backpack.bin`), and per-player data (including per-world MMO skills).

## Chunk Streaming & Rendering Performance

To maintain a fluid 60+ FPS while flying or running across the landscape, chunk generation and meshing are split asynchronously:
- **Distance-Prioritized Streaming**: Chunks closest to the player and within the player's look-direction are meshed and loaded first, preventing missing nearby chunks while distant ones load.
- **GPU Upload Budgeting**: Mesh vertex and index buffers are uploaded to the GPU with a per-frame transfer budget, eliminating frame stutters and lag spikes when entering new terrain.
- **Granular World Locks**: Background worker threads read voxel data without stalling the main render thread.

