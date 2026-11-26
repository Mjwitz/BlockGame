# Pluggable Native Gamepad Backend and Hybrid Virtual-Cursor Controller UI

`blockgame_client` decouples gameplay and UI input from platform controller SDKs behind a `GamepadBackend` trait, shipping first with pure-Rust `gilrs` (Linux `evdev`/`libudev` and Windows XInput/WGI) so all five target Controller Families (`Xbox`, `PlayStation`, `Nintendo`, `Steam Deck`, `Steam Controller`) work in standalone builds without requiring Steamworks, while leaving `steamworks-rs` Steam Input as a future pluggable backend matching [ADR 0007](./0007-client-owned-global-achievements-and-steam-boundary.md). Across BlockGame's custom immediate-mode 2D UI (`UiBuilder`), Controller Input Mode uses a hybrid navigation model: a left-stick Virtual Cursor for immediate compatibility with every existing screen and inventory grid, paired with D-pad focus snapping, bumper tab/page switching, and face-button inventory shortcuts.

## Considered Options

- **Steam Input (`steamworks-rs`) as the primary backend**: Gives native access to Steam Controller trackpads, Steam Deck back paddles, gyro, and Big Picture OSK, but introduces a Steamworks SDK dependency today and leaves non-Steam Linux/Windows builds without controller support.
- **`sdl2` as the native backend**: Broadest out-of-the-box HID mapping database, but adds a native C library dependency to Linux and Windows release builds and CI.
- **Strict D-pad focus navigation without a Virtual Cursor**: Avoids pointer emulation, but requires retrofitting explicit focus graphs onto every custom `UiBuilder` screen (inventory, crafting, containers, shop, JEI sidebar, character creator) before controller play is usable.

## Consequences

- `blockgame_client` targets a unified dual-stick standard layout with per-family pixel-art button glyphs in the UI texture atlas and basic rumble; hardware-specific features (gyro aiming, native trackpad APIs, Steam Deck back-grip paddles, DualSense adaptive triggers) and a custom in-game on-screen keyboard are explicitly deferred to follow-up work (or future Steam Input integration).
- Outside Steam, legacy Steam Controllers rely on Steam's virtual gamepad layer or OS community drivers until a Steam Input `GamepadBackend` implementation is added.
- Linux CI and build docs add `libudev-dev` for `gilrs`.
