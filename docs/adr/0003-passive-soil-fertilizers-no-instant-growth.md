# Passive Soil Fertilizers Only (No Instant Crop Growth)

Fertilizers (`Compost` and `Bone Fertilizer`) are applied to farmland tiles as passive soil buffs that accelerate growth speed and/or grant bonus yield for a single harvest cycle, and never trigger instant growth-stage skips. Deliberately breaking from Minecraft's instant-growth Bone Meal convention preserves the value of slower, randomized crop growth (~5–8 minutes) and gives players a meaningful reason to plan farms, prepare soil, and craft tiered fertilizers rather than spam-clicking crops to maturity.

## Considered Options

- **Instant growth boost (Minecraft-style Bone Meal)**: Undermines the slower, randomized crop pacing and collapses farming progression into skeleton-bone grinding.
- **Hybrid (instant Bone Meal + passive Compost)**: Keeps two overlapping growth-acceleration systems where instant growth still dominates late-game farming.

## Consequences

- `Bone` crafts into `Bone Fertilizer` (2.0× growth speed and +1 bonus harvest yield) rather than an instant-growth item.
- Fertilizer tier is encoded directly in farmland `BlockId` variants (`FARMLAND_COMPOST*`, `FARMLAND_BONE*`) in chunk persistence, syncing automatically via `BlockUpdate` and clearing on harvest, first Stem Crop fruit spawn, or soil reversion to Dirt.
