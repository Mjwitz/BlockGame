# Stylized Procedural Low-Poly Character Pipeline

We will render player characters using a dedicated vertex-colored `wgpu` pipeline (`CharacterVertex` with position, normal, RGB color, and world lighting) and procedurally generated, articulated low-poly geometry rather than the existing 16×16 voxel texture atlas or imported glTF meshes. This breaks cleanly from the 6-box Minecraft silhouette, supports arbitrary RGB customization without runtime texture packing, and stays lightweight and native to both desktop and WebAssembly targets.

## Considered Options

- **Extend the existing 16×16 voxel atlas**: Too constrained for arbitrary RGB colors and locks characters into a blocky Minecraft aesthetic.
- **CPU-generated dynamic skin textures**: Requires runtime texture allocation, UV unwrapping across varied 3D hairstyles/clothing, and extra GPU texture management per player.
- **Imported glTF skeletal meshes**: Introduces external asset authoring, skinning/rigging pipelines, and larger binary/WASM payloads.
