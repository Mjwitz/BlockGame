# Compact Single-Enhancement Representation on `ItemSlot`

Shop-exclusive **Enhancements** (such as *Reinforced*, *Haste*, *Prospector*, *Keen*, *Bulwark*, *Treasure Hunter*, and *Lucky Cast*) will be stored as a single compact `Option<Enhancement>` (1-byte enum) directly on `ItemSlot`, rather than introducing a heap-allocated multi-enchantment map or separate hardcoded `ItemId` variants per enhanced tool. Limiting each item to at most one Enhancement forces meaningful player trade-offs, keeps tooltips and UI clean, preserves `ItemSlot: Copy` (7 bytes on disk/wire), and allows backward-compatible loading of existing world saves by defaulting older slots to `None`.

## Considered Options

- **Heap-allocated attribute/enchantment map (NBT-style)**: Supports stacking multiple modifiers per item, but breaks `ItemSlot: Copy`, increases serialization and network overhead across all inventories/containers/drops, and encourages power-creep stacking.
- **Dedicated `ItemId` per enhanced item variant**: Requires zero changes to `ItemSlot`, but causes a combinatorial explosion of item IDs across every tool/armor/rod tier and Enhancement type.

## Consequences

- `ItemSlot` equality for stacking must include `enhancement` (enhanced tools/gear are unstackable anyway, with `max_stack == 1`).
- Save serializers (`players/*.bin`, `containers.bin`, `backpack.bin`, `entities.bin`) and network packets encoding `ItemSlot` must version-bump and default legacy saves to `enhancement: None`.
- An already-enhanced item cannot receive a second Enhancement, and applying an Enhancement restores the item to full durability.
