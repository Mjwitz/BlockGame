use std::collections::{HashSet, VecDeque};

use glam::IVec3;

use crate::block::BlockId;
use crate::crafting::{get_fuel_burn_time, get_smelting_output};
use crate::item::ItemSlot;
use crate::world::World;

/// Output of a Coal Generator in EU per second while burning fuel.
pub const COAL_GENERATOR_OUTPUT_EU_S: f32 = 40.0;
/// Output of a Tier 1 Solar Panel in EU per second during daytime with clear sky above.
pub const SOLAR_PANEL_OUTPUT_EU_S: f32 = 25.0;
/// Output of a Tier 2 Advanced Solar Panel in EU per second during daytime with clear sky above.
pub const ADVANCED_SOLAR_PANEL_OUTPUT_EU_S: f32 = 75.0;
/// Base output of a Wind Turbine in EU per second at sea level (scales up with elevation).
pub const WIND_TURBINE_BASE_EU_S: f32 = 15.0;
/// Power draw of an Electric Furnace in EU per second while actively smelting.
pub const ELECTRIC_FURNACE_DRAW_EU_S: f32 = 30.0;
/// Smelt time in seconds for an Electric Furnace (2.5x faster than the 10s stone furnace).
pub const ELECTRIC_FURNACE_COOK_TIME: f32 = 4.0;
/// Power draw of an Electric Lamp in EU per second.
pub const ELECTRIC_LAMP_DRAW_EU_S: f32 = 5.0;
/// Tier 1 Battery Box capacity (EU) and max I/O rate (EU/s).
pub const BATTERY_DEFAULT_CAPACITY_EU: f32 = 20_000.0;
pub const BATTERY_MAX_IO_EU_S: f32 = 100.0;
/// Tier 2 Advanced Battery Box capacity (EU) and max I/O rate (EU/s).
pub const ADVANCED_BATTERY_CAPACITY_EU: f32 = 100_000.0;
pub const ADVANCED_BATTERY_MAX_IO_EU_S: f32 = 300.0;
/// Tier 3 Quantum Battery Box capacity (EU) and max I/O rate (EU/s).
pub const QUANTUM_BATTERY_CAPACITY_EU: f32 = 500_000.0;
pub const QUANTUM_BATTERY_MAX_IO_EU_S: f32 = 1_000.0;

pub const fn battery_capacity_eu(block: BlockId) -> f32 {
    match block {
        BlockId::ADVANCED_BATTERY_BOX => ADVANCED_BATTERY_CAPACITY_EU,
        BlockId::QUANTUM_BATTERY_BOX => QUANTUM_BATTERY_CAPACITY_EU,
        _ => BATTERY_DEFAULT_CAPACITY_EU,
    }
}

/// Tier 1 Copper Cable max throughput (EU/s).
pub const COPPER_CABLE_MAX_EU_S: f32 = 200.0;
/// Tier 2 Gold Cable max throughput (EU/s).
pub const GOLD_CABLE_MAX_EU_S: f32 = 1_000.0;
/// Tier 3 Diamond Cable max throughput (EU/s).
pub const DIAMOND_CABLE_MAX_EU_S: f32 = 5_000.0;

pub const fn cable_max_eu_s(block: BlockId) -> f32 {
    match block {
        BlockId::COPPER_CABLE => COPPER_CABLE_MAX_EU_S,
        BlockId::GOLD_CABLE => GOLD_CABLE_MAX_EU_S,
        BlockId::DIAMOND_CABLE => DIAMOND_CABLE_MAX_EU_S,
        _ => f32::INFINITY,
    }
}

pub const fn battery_max_io_eu_s(block: BlockId) -> f32 {
    match block {
        BlockId::ADVANCED_BATTERY_BOX => ADVANCED_BATTERY_MAX_IO_EU_S,
        BlockId::QUANTUM_BATTERY_BOX => QUANTUM_BATTERY_MAX_IO_EU_S,
        _ => BATTERY_MAX_IO_EU_S,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoalGeneratorState {
    pub fuel: ItemSlot,
    pub burn_time: f32,
    pub max_burn_time: f32,
    pub current_output_eu_s: f32,
}

impl Default for CoalGeneratorState {
    fn default() -> Self {
        Self::new()
    }
}

impl CoalGeneratorState {
    pub fn new() -> Self {
        Self {
            fuel: ItemSlot::EMPTY,
            burn_time: 0.0,
            max_burn_time: 0.0,
            current_output_eu_s: 0.0,
        }
    }

    pub fn is_burning(&self) -> bool {
        self.burn_time > 0.0
    }

    pub fn burn_progress(&self) -> f32 {
        if self.max_burn_time <= 0.0 {
            0.0
        } else {
            (self.burn_time / self.max_burn_time).clamp(0.0, 1.0)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatteryState {
    pub stored_eu: f32,
    pub capacity_eu: f32,
    /// Net EU/s flow during the last tick (+ charging, - discharging).
    pub last_net_eu_s: f32,
}

impl Default for BatteryState {
    fn default() -> Self {
        Self::new()
    }
}

impl BatteryState {
    pub fn new() -> Self {
        Self::for_block(BlockId::BATTERY_BOX)
    }

    pub fn for_block(block: BlockId) -> Self {
        Self {
            stored_eu: 0.0,
            capacity_eu: battery_capacity_eu(block),
            last_net_eu_s: 0.0,
        }
    }

    pub fn charge_ratio(&self) -> f32 {
        if self.capacity_eu <= 0.0 {
            0.0
        } else {
            (self.stored_eu / self.capacity_eu).clamp(0.0, 1.0)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElectricFurnaceState {
    pub input: ItemSlot,
    pub output: ItemSlot,
    pub cook_time: f32,
    pub max_cook_time: f32,
    pub is_powered: bool,
}

impl Default for ElectricFurnaceState {
    fn default() -> Self {
        Self::new()
    }
}

impl ElectricFurnaceState {
    pub fn new() -> Self {
        Self {
            input: ItemSlot::EMPTY,
            output: ItemSlot::EMPTY,
            cook_time: 0.0,
            max_cook_time: ELECTRIC_FURNACE_COOK_TIME,
            is_powered: false,
        }
    }

    pub fn cook_progress(&self) -> f32 {
        if self.max_cook_time <= 0.0 {
            0.0
        } else {
            (self.cook_time / self.max_cook_time).clamp(0.0, 1.0)
        }
    }

    pub fn can_smelt(&self) -> bool {
        if self.input.is_empty() {
            return false;
        }
        let Some(result_id) = get_smelting_output(self.input.item_id) else {
            return false;
        };
        self.output.is_empty() || (self.output.item_id == result_id && self.output.count < 64)
    }

    pub fn complete_smelt(&mut self) {
        if let Some(result_id) = get_smelting_output(self.input.item_id) {
            if self.output.is_empty() {
                self.output = ItemSlot::new(result_id, 1);
            } else if self.output.item_id == result_id && self.output.count < 64 {
                self.output.count += 1;
            } else {
                return;
            }
            self.input.count -= 1;
            if self.input.count == 0 {
                self.input = ItemSlot::EMPTY;
            }
        }
    }
}

/// Live summary of a connected electrical network for UI readouts (all rates in EU/s).
#[derive(Debug, Clone, Copy, Default)]
pub struct PowerNetworkSummary {
    pub total_gen_eu_s: f32,
    pub total_demand_eu_s: f32,
    pub total_stored_eu: f32,
    pub total_capacity_eu: f32,
    pub cable_limit_eu_s: Option<f32>,
    pub node_count: usize,
}

const NEIGHBORS_6: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];

/// Computes live output (EU/s) of a Solar Panel (Tier 1 or Tier 2) at `pos`.
pub fn solar_panel_output_eu_s(world: &World, pos: IVec3, is_daytime: bool) -> f32 {
    if !is_daytime {
        return 0.0;
    }
    // Check unobstructed sky above (up to 64 blocks above)
    for dy in 1..=64 {
        let b = world.get_block(pos + IVec3::new(0, dy, 0));
        if !b.is_air() && b != BlockId::GLASS {
            return 0.0;
        }
    }
    if world.get_block(pos) == BlockId::ADVANCED_SOLAR_PANEL {
        ADVANCED_SOLAR_PANEL_OUTPUT_EU_S
    } else {
        SOLAR_PANEL_OUTPUT_EU_S
    }
}

/// Computes live output (EU/s) of a Wind Turbine at `pos` (base power port).
/// The rotor spins 3 blocks above the base, so airflow and elevation are measured at `pos.y + 3`.
pub fn wind_turbine_output_eu_s(world: &World, pos: IVec3) -> f32 {
    let rotor_pos = pos + IVec3::new(0, 3, 0);
    // Needs clear space for the 4-block tower and open airflow around the rotor
    for dy in 1..=3 {
        let b = world.get_block(pos + IVec3::new(0, dy, 0));
        if !b.is_air() {
            return 0.0;
        }
    }
    for offset in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
        let b = world.get_block(rotor_pos + offset);
        if !b.is_air() {
            return 0.0;
        }
    }
    let elevation_bonus = ((rotor_pos.y - 60).clamp(0, 90) as f32) * 0.5;
    WIND_TURBINE_BASE_EU_S + elevation_bonus
}

/// Collects all connected power block positions reachable from `start` via 6-neighbor adjacency.
fn collect_network(world: &World, start: IVec3, visited: &mut HashSet<IVec3>) -> Vec<IVec3> {
    let mut component = Vec::new();
    let mut queue = VecDeque::new();
    if visited.insert(start) {
        queue.push_back(start);
    }
    while let Some(pos) = queue.pop_front() {
        component.push(pos);
        for offset in NEIGHBORS_6 {
            let npos = pos + offset;
            if !visited.contains(&npos) && world.get_block(npos).is_power_block() {
                visited.insert(npos);
                queue.push_back(npos);
            }
        }
    }
    component
}

/// Computes a live read-only summary for the electrical network containing `start_pos`.
pub fn get_network_summary(world: &World, start_pos: IVec3, is_daytime: bool) -> PowerNetworkSummary {
    if !world.get_block(start_pos).is_power_block() {
        return PowerNetworkSummary::default();
    }
    let mut visited = HashSet::new();
    let nodes = collect_network(world, start_pos, &mut visited);
    let mut summary = PowerNetworkSummary {
        node_count: nodes.len(),
        ..Default::default()
    };

    for pos in nodes {
        let block = world.get_block(pos);
        if block.is_cable() {
            let cap = cable_max_eu_s(block);
            summary.cable_limit_eu_s = Some(match summary.cable_limit_eu_s {
                Some(prev) => prev.min(cap),
                None => cap,
            });
        }
        match block {
            BlockId::COAL_GENERATOR => {
                if let Some(gen) = world.coal_generators.get(&pos) {
                    summary.total_gen_eu_s += gen.current_output_eu_s;
                }
            }
            BlockId::SOLAR_PANEL | BlockId::ADVANCED_SOLAR_PANEL => {
                summary.total_gen_eu_s += solar_panel_output_eu_s(world, pos, is_daytime);
            }
            BlockId::WIND_TURBINE => {
                summary.total_gen_eu_s += wind_turbine_output_eu_s(world, pos);
            }
            BlockId::BATTERY_BOX
            | BlockId::ADVANCED_BATTERY_BOX
            | BlockId::QUANTUM_BATTERY_BOX => {
                if let Some(bat) = world.batteries.get(&pos) {
                    summary.total_stored_eu += bat.stored_eu;
                    summary.total_capacity_eu += bat.capacity_eu;
                } else {
                    summary.total_capacity_eu += battery_capacity_eu(block);
                }
            }
            BlockId::ELECTRIC_FURNACE => {
                if let Some(ef) = world.electric_furnaces.get(&pos) {
                    if ef.can_smelt() {
                        summary.total_demand_eu_s += ELECTRIC_FURNACE_DRAW_EU_S;
                    }
                }
            }
            BlockId::ELECTRIC_LAMP | BlockId::ELECTRIC_LAMP_LIT => {
                summary.total_demand_eu_s += ELECTRIC_LAMP_DRAW_EU_S;
            }
            BlockId::STORAGE_TERMINAL => {
                summary.total_demand_eu_s += crate::storage::terminal_power_draw_eu_s(world, pos);
            }
            _ => {}
        }
    }
    summary
}

/// Advances all electrical networks in `world` by `dt` seconds.
///
/// Returns a list of world block positions whose block ID changed (e.g., Electric Lamps
/// toggling between `ELECTRIC_LAMP` and `ELECTRIC_LAMP_LIT`) so callers can update lighting
/// and remesh affected chunks.
pub fn tick_power_networks(world: &mut World, dt: f32, is_daytime: bool) -> Vec<IVec3> {
    if dt <= 0.0 || world.power_nodes.is_empty() {
        world.powered_storage_terminals.clear();
        return Vec::new();
    }

    // Prune any stale positions that are no longer power blocks.
    let all_nodes: Vec<IVec3> = world.power_nodes.iter().copied().collect();
    let mut valid_nodes = Vec::with_capacity(all_nodes.len());
    for pos in all_nodes {
        if world.get_block(pos).is_power_block() {
            valid_nodes.push(pos);
        } else {
            world.power_nodes.remove(&pos);
            world.powered_storage_terminals.remove(&pos);
        }
    }

    let mut visited = HashSet::with_capacity(valid_nodes.len());
    let mut changed_blocks = Vec::new();

    for start in valid_nodes {
        if visited.contains(&start) {
            continue;
        }
        let component = collect_network(world, start, &mut visited);

        // Categorize nodes in this connected network and find the cable throughput limit.
        let mut coal_gens = Vec::new();
        let mut batteries: Vec<(IVec3, BlockId)> = Vec::new();
        let mut electric_furnaces = Vec::new();
        let mut lamps = Vec::new();
        let mut storage_terminals: Vec<(IVec3, f32)> = Vec::new();
        let mut passive_gen_eu_s = 0.0f32;
        let mut cable_limit_eu_s = f32::INFINITY;

        for &pos in &component {
            let block = world.get_block(pos);
            if block.is_cable() {
                cable_limit_eu_s = cable_limit_eu_s.min(cable_max_eu_s(block));
            }
            match block {
                BlockId::COAL_GENERATOR => coal_gens.push(pos),
                BlockId::SOLAR_PANEL | BlockId::ADVANCED_SOLAR_PANEL => {
                    passive_gen_eu_s += solar_panel_output_eu_s(world, pos, is_daytime);
                }
                BlockId::WIND_TURBINE => {
                    passive_gen_eu_s += wind_turbine_output_eu_s(world, pos);
                }
                BlockId::BATTERY_BOX
                | BlockId::ADVANCED_BATTERY_BOX
                | BlockId::QUANTUM_BATTERY_BOX => batteries.push((pos, block)),
                BlockId::ELECTRIC_FURNACE => electric_furnaces.push(pos),
                BlockId::ELECTRIC_LAMP | BlockId::ELECTRIC_LAMP_LIT => lamps.push(pos),
                BlockId::STORAGE_TERMINAL => {
                    let draw = crate::storage::terminal_power_draw_eu_s(world, pos);
                    storage_terminals.push((pos, draw));
                }
                _ => {}
            }
        }

        // Ensure battery states exist and compute total free battery headroom.
        let mut battery_headroom_eu = 0.0f32;
        for &(pos, _block) in &batteries {
            let bat = world.get_or_create_battery(pos);
            bat.last_net_eu_s = 0.0;
            battery_headroom_eu += (bat.capacity_eu - bat.stored_eu).max(0.0);
        }

        // Compute total active consumer demand (EU/s).
        let mut active_furnaces = Vec::new();
        for &pos in &electric_furnaces {
            let ef = world.get_or_create_electric_furnace(pos);
            if ef.can_smelt() {
                active_furnaces.push(pos);
            } else {
                ef.cook_time = 0.0;
                ef.is_powered = false;
            }
        }
        let lamp_demand_eu_s = lamps.len() as f32 * ELECTRIC_LAMP_DRAW_EU_S;
        let furnace_demand_eu_s = active_furnaces.len() as f32 * ELECTRIC_FURNACE_DRAW_EU_S;
        let storage_demand_eu_s: f32 = storage_terminals.iter().map(|&(_, d)| d).sum();
        let total_demand_eu_s = lamp_demand_eu_s + furnace_demand_eu_s + storage_demand_eu_s;

        // Determine whether Coal Generators need to ignite new fuel.
        let network_needs_fuel_gen =
            (total_demand_eu_s > passive_gen_eu_s) || (battery_headroom_eu > 1.0);

        let mut coal_gen_eu_s = 0.0f32;
        for &pos in &coal_gens {
            let gen = world.get_or_create_coal_generator(pos);
            if gen.burn_time > 0.0 {
                gen.burn_time = (gen.burn_time - dt).max(0.0);
                gen.current_output_eu_s = COAL_GENERATOR_OUTPUT_EU_S;
                coal_gen_eu_s += COAL_GENERATOR_OUTPUT_EU_S;
            } else if network_needs_fuel_gen && !gen.fuel.is_empty() {
                let burn_dur = get_fuel_burn_time(gen.fuel.item_id);
                if burn_dur > 0.0 {
                    let consumed_lava_bucket = gen.fuel.item_id == crate::item::ItemId::LAVA_BUCKET;
                    gen.fuel.count -= 1;
                    if gen.fuel.count == 0 {
                        gen.fuel = if consumed_lava_bucket {
                            ItemSlot::new(crate::item::ItemId::BUCKET, 1)
                        } else {
                            ItemSlot::EMPTY
                        };
                    }
                    gen.burn_time = (burn_dur - dt).max(0.0);
                    gen.max_burn_time = burn_dur;
                    gen.current_output_eu_s = COAL_GENERATOR_OUTPUT_EU_S;
                    coal_gen_eu_s += COAL_GENERATOR_OUTPUT_EU_S;
                } else {
                    gen.current_output_eu_s = 0.0;
                }
            } else {
                gen.current_output_eu_s = 0.0;
            }
        }

        // Enforce tiered cable throughput cap (e.g., 200 / 1,000 / 5,000 EU/s).
        let max_transfer_eu = cable_limit_eu_s * dt;
        let total_gen_eu = ((passive_gen_eu_s + coal_gen_eu_s) * dt).min(max_transfer_eu);
        let total_demand_eu = total_demand_eu_s * dt;

        // Balance generation, demand, and batteries (respecting cable & battery I/O caps).
        let mut supplied_eu = total_gen_eu.min(total_demand_eu);
        if total_gen_eu > total_demand_eu && !batteries.is_empty() {
            // Charge batteries with surplus energy.
            let mut surplus_eu = total_gen_eu - total_demand_eu;
            for &(pos, block) in &batteries {
                if surplus_eu <= 0.0 {
                    break;
                }
                let per_bat_cap = battery_max_io_eu_s(block) * dt;
                let bat = world.get_or_create_battery(pos);
                let space = (bat.capacity_eu - bat.stored_eu).max(0.0);
                let add = surplus_eu.min(space).min(per_bat_cap);
                bat.stored_eu += add;
                bat.last_net_eu_s = add / dt;
                surplus_eu -= add;
            }
        } else if total_demand_eu > total_gen_eu && !batteries.is_empty() {
            // Discharge batteries to cover the shortfall (up to remaining cable bandwidth).
            let mut shortfall_eu =
                (total_demand_eu - total_gen_eu).min((max_transfer_eu - supplied_eu).max(0.0));
            for &(pos, block) in &batteries {
                if shortfall_eu <= 0.0 {
                    break;
                }
                let per_bat_cap = battery_max_io_eu_s(block) * dt;
                let bat = world.get_or_create_battery(pos);
                let take = shortfall_eu.min(bat.stored_eu).min(per_bat_cap);
                bat.stored_eu -= take;
                bat.last_net_eu_s = -(take / dt);
                supplied_eu += take;
                shortfall_eu -= take;
            }
        }

        let power_ratio = if total_demand_eu > 0.0 {
            (supplied_eu / total_demand_eu).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Update Storage Terminals powered state (require >= 80% of requested EU/s)
        let storage_online = total_demand_eu > 0.0 && power_ratio >= 0.8;
        for &(pos, _) in &storage_terminals {
            if storage_online {
                world.powered_storage_terminals.insert(pos);
            } else {
                world.powered_storage_terminals.remove(&pos);
            }
        }

        // Update Electric Lamps (require at least 50% power ratio to stay lit).
        let lamps_lit = total_demand_eu > 0.0 && power_ratio >= 0.5;
        for &pos in &lamps {
            let current = world.get_block(pos);
            let target = if lamps_lit {
                BlockId::ELECTRIC_LAMP_LIT
            } else {
                BlockId::ELECTRIC_LAMP
            };
            if current != target {
                world.set_block(pos, target);
                changed_blocks.push(pos);
            }
        }

        // Update Electric Furnaces.
        for &pos in &active_furnaces {
            let ef = world.get_or_create_electric_furnace(pos);
            if power_ratio > 0.05 {
                ef.is_powered = true;
                ef.cook_time += dt * power_ratio;
                if ef.cook_time >= ef.max_cook_time {
                    ef.cook_time -= ef.max_cook_time;
                    ef.complete_smelt();
                }
            } else {
                ef.is_powered = false;
                ef.cook_time = (ef.cook_time - dt * 0.5).max(0.0);
            }
        }
    }

    changed_blocks
}
