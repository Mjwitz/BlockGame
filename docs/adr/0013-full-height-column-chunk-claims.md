# Full-Height Column Chunk Claims over Cubic Chunks

Although `blockgame_core` stores terrain in `16×16×16` cubic chunks, both **Claimed Chunks** and **Admin Claims** are keyed by `(chunk_x, chunk_z)` and protect the entire vertical column from bedrock to sky. Server-wide claim rules live in `config/claims.yml`, while world-bound claim ownership, Admin Claim labels, and per-owner Trusted Player lists are persisted per world in `saves/<world_name>/claims.yml`.

## Considered Options

- **3D `16×16×16` cubic chunk claims**: Matches the engine's internal chunk storage, but allows griefers to tunnel directly beneath or build/pour lava directly above a player's claimed build, and forces players to manage vertical claim slices.
- **Arbitrary 2D/3D bounding-box claims (corner-selection tool)**: Allows non-chunk-aligned shapes, but complicates overlap checks, border visualization, and chunk-limit accounting compared to fixed `(chunk_x, chunk_z)` lookups.

## Consequences

- Claim ownership checks are an `O(1)` lookup on `(pos.x.div_euclid(16), pos.z.div_euclid(16))`, ignoring `pos.y`.
- Players cannot stack separate claims vertically in the same `(chunk_x, chunk_z)` column; each column has at most one owner (a player or an Admin Claim).
- Switching worlds on a server never collides claim data because claims live inside `saves/<world_name>/claims.yml`.
