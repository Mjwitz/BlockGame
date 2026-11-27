# Deterministic Region-Seeded Cave Carvers

`blockgame_gen` replaces pure 3D Perlin swiss-cheese/spaghetti cave noise with deterministic, region-seeded worm carvers evaluated statelessly per chunk inside `TerrainGenerator::generate_chunk`. Each 64×64 horizontal region deterministically seeds a cave network composed of three Cave Archetypes (`Winding Passage`, `Vaulted Cavern`, and `Fissure`), and any chunk checks only the bounded neighborhood of regions whose worms can reach it—preserving stateless, thread-safe, chunk-order-independent generation while guaranteeing long, continuous, walkable underground paths and rare dry-land Cave Mouths (~1 per 500–750 blocks).

## Considered Options

- **Tuned pure 3D noise (dual-noise spaghetti + chamber noise)**: Zero region-scan overhead and trivially unbounded, but difficult to guarantee walkable slopes, avoid abrupt pinches/dead-ends, or independently control rare surface Cave Mouth spacing without disturbing deep tunnel density.
- **Multi-chunk mutable carving pass**: Lets carvers walk freely and mutate neighboring chunks directly, but breaks BlockGame's stateless per-chunk worker architecture (`generate_chunk(ChunkPos) -> Chunk`) and introduces chunk-generation ordering dependencies across streaming threads.

## Consequences

- Worm lengths and branch extents must stay within a fixed horizontal search radius so every chunk deterministically sees all carvers that intersect it without missing cross-border segments.
- Changing cave carving and Ore Vein distributions alters newly generated chunks only; already-saved chunks in existing worlds remain unchanged and may show underground seams along old/new chunk borders.
