# Split Opaque, Cutout, and Translucent Voxel Render Passes

`blockgame_render` splits chunk geometry and `wgpu` pipelines into three passes: **Opaque** (solid terrain blocks with `blend: None`, backface culling, depth write on, and no `discard` in the fragment shader), **Cutout** (alpha-tested foliage, flowers, tall grass, crops, leaves, and glass with `discard`, `blend: None`, `cull_mode: None`, and depth write on), and **Translucent** (Water and Ice with `ALPHA_BLENDING`, depth write off, and back-to-front chunk sorting). Running `discard` and `ALPHA_BLENDING` on a single shared terrain pipeline disabled hardware Early-Z rejection for every solid voxel in the world, doubled cross-quad plant geometry to compensate for backface culling, and caused severe overdraw in flat, dense-foliage biomes such as Flower Meadow, Savanna, and Jungle.

## Considered Options

- **Keep a single opaque+cutout pipeline and reduce biome foliage density**: Avoids adding a third chunk mesh buffer and render pass, but compromises biome visual identity while leaving Early-Z disabled for all solid terrain and keeping double-sided 4-quad plant geometry.
- **Alpha-to-coverage with MSAA**: Smooths cutout edges without `discard`, but requires multisampled render targets and hurts fill-rate on integrated/low-end GPUs and WebGL2 targets.

## Consequences

- `ChunkMesh` stores separate `vertices`/`indices` (opaque), `cutout_vertices`/`cutout_indices` (alpha-tested), and `water_vertices`/`water_indices` (translucent) streams, uploaded as independent GPU buffers only when non-empty.
- Solid terrain draws first in front-to-back chunk order with hardware Early-Z active and zero alpha blending, populating the depth buffer before cutout foliage is shaded.
- Cross-quad plants (flowers, tall grass, crops, sugar cane, dripstone, glow crystal) emit 2 quads (8 vertices, 12 indices) instead of 4 quads (16 vertices, 24 indices) because the Cutout pipeline uses `cull_mode: None`, halving foliage vertex/index volume with identical visuals.
- Having a dedicated Cutout pass makes distance-based foliage culling in low-end graphics modes a zero-remesh per-chunk skip at draw time.
