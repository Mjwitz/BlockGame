# Shared Core Simulation with Server-Authoritative Multiplayer Ticking

Crop growth, farmland hydration/fertilization, animal AI/herding/breeding/egg-laying, and cooking/furnace station ticking will live in shared simulation modules inside `blockgame_core`, executed locally by `WorldManager` in singleplayer and authoritatively by `MultiplayerServer` (20 TPS) in multiplayer. Multiplayer clients will disable their local ticks (`!is_multiplayer`) and render server-pushed updates (`BlockUpdate`, `ContainerSync`, and new animal sync/interaction packets) so all players see consistent crop states, animals, and shared cooking stations without refactoring singleplayer into an embedded local server.

## Considered Options

- **Keep ticking client-side in `WorldManager`**: Leaves crops, animals, and cooking pots desynchronized across players and allows conflicting container overwrites.
- **Run an embedded `MultiplayerServer` for singleplayer**: Guarantees a single code path, but requires a much larger client/save/networking refactor outside the scope of the farming and hunger revamp.

## Consequences

- `MultiplayerServer` must track active chunks around connected players and persist mutated `animals`, `planted_crops`, `active_farmland`, and `cooking_pots` on autosave and shutdown.
- Any future world-ticking system (e.g., weather effects, new stations) should follow this same `blockgame_core` shared-simulation pattern.
