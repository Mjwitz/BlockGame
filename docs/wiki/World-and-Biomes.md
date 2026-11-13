# World and Biomes

## 16 Procedural Biomes
Worlds are generated deterministically from a 32-bit seed using multi-octave 2D heightmaps, 3D cave carving, and temperature/moisture noise. Each biome has its own terrain profile, blocks, vegetation, and atmospheric fog color:

| Biome | Characteristics |
| :--- | :--- |
| **Plains** | Rolling grasslands, flowers, and oak trees |
| **Forest** | Dense oak/birch woodland |
| **Dark Forest** | Thick canopy with dark oak trees and shadowy fog |
| **Taiga** | Cool evergreen spruce forests |
| **Snowy Tundra** | Snow-covered terrain and frozen water |
| **Mountains** | High-altitude stone peaks and steep cliffs |
| **Desert** | Sand dunes, sandstone, and cacti |
| **Badlands** | Terracotta canyons and red sand |
| **Savanna** | Golden grass and acacia-style warm plains |
| **Jungle** | Tall tropical trees and lush foliage |
| **Swamp** | Shallow murky water, clay, and moss-tinted fog |
| **Mushroom Fields** | Mycelium islands with giant red & brown mushrooms |
| **Beach** | Sandy coastal shorelines |
| **River** | Winding freshwater channels |
| **Ocean** | Deep marine basins |
| **Coral Reef** | Vibrant turquoise lagoons with coral structures |

## World Physics & Lighting
- **Voxel Lighting**: BFS flood-fill propagation for both **sky light** (0–15) and **emissive blocklight** (e.g., Torches, Furnaces).
- **Water & Buckets**: Flowing water inflow physics and Buckets for picking up and placing water.
- **Gravity & Support**: Sand and Gravel fall when unsupported; attached blocks (torches, crops, flowers) break if their supporting block is removed.
- **Modular World Saves**: Singleplayer and server worlds are stored under `saves/<world_name>/` with chunk regions, world metadata, and per-player data.
