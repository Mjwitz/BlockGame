# Per-NPC Shopkeeper Progression with Split Stock Scopes and Persistent Mortality

Village state, Shopkeepers, and Village Guards will be persisted in a dedicated `saves/<world>/villages.bin` file and simulated in shared `blockgame_core` modules (ticked locally in singleplayer and authoritatively at 20 TPS by `MultiplayerServer` per ADR-0004). Each Shopkeeper tracks **Shop Tier** (1–5), **Tiers 1–3 purchase stock**, and **daily sell/buyback coin limits** per individual player (`(npc_id, player_username)`), while **Tiers 4–5 stock** (unique items and Enhancements) is **shared globally per NPC across all players** on a 3-in-game-day restock cycle; NPC deaths are permanent with no automatic respawn.

## Considered Options

- **Global per-player progression across all Shopkeepers of a profession**: Removes the sting of losing a Shopkeeper, undermining the core design goal of incentivizing players to fortify and defend their villages.
- **Fully shared stock across all tiers (1–5)**: Allows a single player on a multiplayer server to buy out basic supplies (food, torches, starter gear) and starve everyone else.
- **Fully per-player stock across all tiers (1–5)**: Eliminates scarcity and competition for endgame Tier 4–5 unique items and Enhancements in multiplayer.

## Consequences

- Losing a high-tier Shopkeeper permanently destroys every player's unlocked Shop Tier with that specific NPC, making lighting, walls, and Village Guards genuinely valuable.
- Multiplayer shop transactions (`Buy`, `Sell`, `Enhance`) must be validated atomically on `MultiplayerServer` so concurrent players cannot race-duplicate shared Tier 4–5 stock or exceed daily per-player sell budgets.
- `MonsterEntity` targeting in `blockgame_core` must be generalized beyond a single `player_pos` so hostile monsters can actively target nearby Shopkeepers and Village Guards, and Village Guards can retaliate per-player when a player damages any Village NPC.
