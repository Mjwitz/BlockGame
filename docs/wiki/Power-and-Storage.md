# Power and Storage (v0.3.0)

BlockGame v0.3.0 introduces a unified **EU (Energy Unit)** electrical power system, electric machines, upgraded high-capacity chests, a powered linked **Storage Terminal** network, and unique portable **Backpacks**.

---

## Electrical Power System (EU / EU/s)

Every power generator, cable, battery, and electric machine operates on **EU (Energy Units)** with live **EU/s** production and consumption rates. Right-click any power cable, solar panel, wind turbine, or electric lamp (with an empty hand or non-placeable item) to inspect its live grid summary in chat.

### Power Generators

| Generator | Output | Notes |
| :--- | :--- | :--- |
| **Coal Generator** | `+40 EU/s` | Burns Coal, Logs, Planks, or Sticks; right-click to open fuel UI and view grid status |
| **Solar Panel** | `+15 EU/s` | Generates EU during daytime when exposed to open sky |
| **Advanced Solar Panel** | `+60 EU/s` | High-tier solar array (4x output) with clear sky access |
| **Wind Turbine** | Up to `+45 EU/s` | 4-block-tall animated rotating turbine with a bottom power port; output scales with altitude and open air |

### Energy Storage (Battery Boxes)

Store excess EU during peak generation and discharge automatically when demand exceeds generation (e.g., at night):

| Tier | Block | Capacity |
| :--- | :--- | :--- |
| Tier 1 | **Battery Box** | `10,000 EU` |
| Tier 2 | **Advanced Battery Box** | `50,000 EU` |
| Tier 3 | **Quantum Battery Box** | `250,000 EU` |

### Tiered Power Cables

Power cables connect generators, batteries, and machines in 6 directions. Each cable tier enforces a maximum **EU/s throughput cap** on the network segment:

| Tier | Cable | Throughput Cap |
| :--- | :--- | :--- |
| Tier 1 | **Copper Cable** | `200 EU/s` |
| Tier 2 | **Gold Cable** | `1,000 EU/s` |
| Tier 3 | **Diamond Cable** | `5,000 EU/s` |

### Electric Machines

| Machine | Power Draw | Function |
| :--- | :--- | :--- |
| **Electric Furnace** | `-25 EU/s` | High-speed smelting powered purely by EU (no coal/wood fuel needed) |
| **Electric Lamp** | `-5 EU/s` | Bright emissive lighting that automatically lights up when powered |
| **Storage Terminal** | Scales with network | Unified searchable/sortable interface for linked chests (see below) |

---

## Item Storage & Sorting System

### Upgraded Chests

Progress from standard Wooden Chests to multi-page high-capacity chests (with `<` / `>` pagination buttons in the UI and full item drops when broken):

| Chest | Slots | Crafting Upgrade |
| :--- | :--- | :--- |
| **Wooden Chest** | 27 | 8 Wood Planks |
| **Iron Chest** | 45 | 8 Iron Ingots around a Wooden Chest |
| **Gold Chest** | 63 | 8 Gold Ingots around an Iron Chest |
| **Diamond Chest** | 81 | 8 Diamonds around a Gold Chest |

### Powered Storage Terminal & Storage Cables

Link any number of Wooden, Iron, Gold, or Diamond Chests into a single searchable, sortable storage network:

- **Storage Cable** (green 6-way data conduit): Connects chests and terminals across your base without mixing with electrical power cables.
- **Storage Terminal**: Connect to your electrical grid for power and to your chests via Storage Cables (or direct adjacency).
- **Multiple Terminals**: Place multiple Storage Terminals on the same cable network to access your entire item bank from different rooms or floors.
- **Search & Sort**: Click the Search bar to filter items live (right-click to clear), toggle **Sort Mode** (`Sort: Count`, `Sort: Name`, `Sort: ID`), and use `Shift + Left Click` for quick deposits and withdrawals.
- **Complexity-Scaled Power Draw**:
  - `15 EU/s` per **Storage Terminal**
  - `5 EU/s` per linked **Chest**
  - `1 EU/s` per **Storage Cable**
  - The terminal header displays `ONLINE` / `OFFLINE`, exact EU/s draw, linked terminal/chest counts, and total slot usage.

### Unique Portable Backpacks

Carry extra inventory while exploring the world:

| Tier | Backpack | Slots |
| :--- | :--- | :--- |
| Tier 1 | **Leather Backpack** | 18 |
| Tier 2 | **Iron Backpack** | 27 |
| Tier 3 | **Diamond Backpack** | 36 |

- **How to Open**: `Right Click` with a backpack in your hotbar, or press **`B`** anytime to open the backpack in your hotbar/inventory.
- **Per-Instance Unique IDs**: Every backpack has its own unique ID (`#1`, `#2`, ...) and completely separate inventory. Two backpacks of the same type never share contents.
- **No Stacking or Nesting**: Backpacks have a max stack size of `1`, cannot be placed inside other backpacks, and appear as separate individual slots (with `#ID` badges) in Storage Terminals.
- **Upgrading Preserves Items**: Crafting a Leather Backpack into an Iron Backpack (or Iron into Diamond) keeps its unique ID and all items stored inside.
- **Persistence**: All individual backpacks are saved per world in `backpack.bin`.
