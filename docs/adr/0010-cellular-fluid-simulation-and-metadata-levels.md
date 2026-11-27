# Cellular Fluid Simulation and Per-Voxel Metadata Levels

`blockgame_core` stores Water and Lava flow levels (`0` = Source Block, `1..=7` = horizontal decay distance, `8` = falling stream) in `Chunk`'s existing per-voxel `u8` metadata array (`facings`, widened from 2-bit masking to 4-bit `0x0F` when persisting/storing fluid metadata while preserving `0..=3` for directional blocks) and drives Water (7-block spread, ~4 ticks/s, 2×2 infinite sources) and Lava (3-block spread, ~1.3 ticks/s, no infinite sources) via a scheduled frontier-queue cellular fluid simulator (`FluidSimulation`). This avoids breaking `BlockId::WATER`/`BlockId::LAVA` identity checks across the codebase and preserves backward compatibility with v2 chunk saves without allocating a second 4 KB array per chunk.

## Considered Options

- **Dedicated flowing `BlockId` variants (`WATER_1..7`, `LAVA_1..3`, etc.)**: Keeps `facings` strictly 2-bit, but adds 12+ new block IDs and forces every `b == BlockId::WATER` / `b.is_lava()` check, renderer branch, and serializer to handle a family of IDs.
- **Separate `[u8; 4096]` fluid-level array on `Chunk`**: Clean separation of concerns, but doubles per-chunk metadata memory and requires a v3 chunk save format migration.

## Consequences

- `Chunk::set_facing` / `encode_chunk_facings` / `apply_chunk_facings` preserve the lower 4 bits (`0x0F`) so fluid levels `0..=8` round-trip through v2 chunk saves without breaking existing directional block facings (`0..=3`).
- Only `fluid_level == 0` (Source Block) can be scooped with a Bucket; removing or blocking a Source Block schedules neighbor ticks so downstream Flowing Fluid drains automatically.
- `ChunkMesher` reads each fluid voxel's metadata level to step down the top surface height (`7/8` down to `1/8`) and `PlayerPhysics` / `ItemDrop` sample the local flow gradient for gentle Water current push.
