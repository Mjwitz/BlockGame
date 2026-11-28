use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;

use crate::permissions::PermissionsManager;

/// Converts a world block coordinate to its full-height `(chunk_x, chunk_z)` claim column (ADR-0013).
#[inline]
pub fn block_pos_to_claim_coord(pos: IVec3) -> (i32, i32) {
    (pos.x.div_euclid(16), pos.z.div_euclid(16))
}

/// Converts a world entity position to its full-height `(chunk_x, chunk_z)` claim column.
#[inline]
pub fn world_pos_to_claim_coord(pos: Vec3) -> (i32, i32) {
    (
        (pos.x.floor() as i32).div_euclid(16),
        (pos.z.floor() as i32).div_euclid(16),
    )
}

/// Ownership of a claimed `(chunk_x, chunk_z)` column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ClaimOwner {
    Player { username: String },
    Admin { label: String },
}

impl ClaimOwner {
    pub fn display_name(&self) -> String {
        match self {
            Self::Player { username } => format!("{}'s Claim", username),
            Self::Admin { label } => {
                if label.trim().is_empty() || label.eq_ignore_ascii_case("Admin Claim") {
                    "Admin Claim".to_string()
                } else {
                    format!("{} (Admin Claim)", label)
                }
            }
        }
    }

    pub fn is_same_owner(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Player { username: a }, Self::Player { username: b }) => {
                a.eq_ignore_ascii_case(b)
            }
            (Self::Admin { label: a }, Self::Admin { label: b }) => a.eq_ignore_ascii_case(b),
            _ => false,
        }
    }
}

/// Ownership relation relative to a viewing client (drives 3D border color coding).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimRelation {
    Own,
    Trusted,
    Other,
    Admin,
}

/// Compact per-chunk claim entry synced to clients for entry/exit notices and 3D chunk borders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncedClaimChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub relation: ClaimRelation,
    pub display_name: String,
    #[serde(default)]
    pub can_modify: bool,
}

/// Server-wide claim rules stored in `config/claims.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimSettings {
    #[serde(default = "default_max_claims")]
    pub default_max_claims: usize,
    #[serde(default = "default_spawn_protection_radius")]
    pub spawn_protection_radius: i32,
}

fn default_max_claims() -> usize {
    16
}

fn default_spawn_protection_radius() -> i32 {
    32
}

impl Default for ClaimSettings {
    fn default() -> Self {
        Self {
            default_max_claims: default_max_claims(),
            spawn_protection_radius: default_spawn_protection_radius(),
        }
    }
}

/// World-bound claim data stored in `saves/<world_name>/claims.yml`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct WorldClaimsYaml {
    #[serde(default)]
    chunks: BTreeMap<String, ClaimOwner>,
    #[serde(default)]
    trust_lists: BTreeMap<String, Vec<String>>,
}

fn encode_coord_key(cx: i32, cz: i32) -> String {
    format!("{},{}", cx, cz)
}

fn decode_coord_key(key: &str) -> Option<(i32, i32)> {
    let (a, b) = key.split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// Authoritative Chunk Claiming manager for Claimed Chunks, Admin Claims, Trusted Players, and Claim Bypass.
#[derive(Debug, Clone, Default)]
pub struct ClaimManager {
    pub settings: ClaimSettings,
    pub chunks: HashMap<(i32, i32), ClaimOwner>,
    /// Keyed by lowercase owner username -> set of lowercase trusted usernames.
    pub trust_lists: HashMap<String, HashSet<String>>,
    /// Per-session admin bypass toggle (lowercase usernames; off by default).
    pub admin_bypass: HashSet<String>,
}

impl ClaimManager {
    pub fn new(settings: ClaimSettings) -> Self {
        Self {
            settings,
            chunks: HashMap::new(),
            trust_lists: HashMap::new(),
            admin_bypass: HashSet::new(),
        }
    }

    /// Loads global rules from `config/claims.yml` and world claims from `saves/<world>/claims.yml`.
    pub fn load(config_dir: &Path, world_dir: &Path) -> Self {
        let cfg_path = config_dir.join("claims.yml");
        let settings = if cfg_path.exists() {
            fs::read_to_string(&cfg_path)
                .ok()
                .and_then(|s| serde_yaml::from_str::<ClaimSettings>(&s).ok())
                .unwrap_or_default()
        } else {
            let def = ClaimSettings::default();
            let _ = crate::config_io::atomic_write_yaml(
                &cfg_path,
                "# BlockGame Chunk Claiming Rules\n# World-bound Claimed Chunks live in saves/<world>/claims.yml.",
                &def,
            );
            def
        };

        let mut mgr = Self::new(settings);
        let world_path = world_dir.join("claims.yml");
        if world_path.exists() {
            if let Ok(text) = fs::read_to_string(&world_path) {
                if let Ok(parsed) = serde_yaml::from_str::<WorldClaimsYaml>(&text) {
                    for (k, v) in parsed.chunks {
                        if let Some(coord) = decode_coord_key(&k) {
                            mgr.chunks.insert(coord, v);
                        }
                    }
                    for (owner, list) in parsed.trust_lists {
                        let set: HashSet<String> =
                            list.into_iter().map(|u| u.trim().to_ascii_lowercase()).collect();
                        mgr.trust_lists.insert(owner.trim().to_ascii_lowercase(), set);
                    }
                }
            }
        }
        mgr
    }

    pub fn save_world_claims(&self, world_dir: &Path) -> io::Result<()> {
        let mut chunks = BTreeMap::new();
        for (&(cx, cz), owner) in &self.chunks {
            chunks.insert(encode_coord_key(cx, cz), owner.clone());
        }
        let mut trust_lists = BTreeMap::new();
        for (owner, set) in &self.trust_lists {
            if !set.is_empty() {
                let mut list: Vec<String> = set.iter().cloned().collect();
                list.sort();
                trust_lists.insert(owner.clone(), list);
            }
        }
        let data = WorldClaimsYaml {
            chunks,
            trust_lists,
        };
        crate::config_io::atomic_write_yaml(
            &world_dir.join("claims.yml"),
            "# BlockGame World Claimed Chunks (full-height 16x16 columns)",
            &data,
        )
    }

    /// Computes a player's effective maximum claimed chunks, honoring `blockgame.claim.limit.<N>`.
    pub fn max_claims_for_player(&self, perms: &PermissionsManager, username: &str) -> usize {
        if perms.has_permission(username, "blockgame.claim.unlimited") {
            return usize::MAX;
        }
        let mut best: Option<usize> = None;
        let canonical = username.trim().to_ascii_lowercase();
        let mut scan_nodes = |nodes: &[String]| {
            for node in nodes {
                let lower = node.trim().to_ascii_lowercase();
                if let Some(suffix) = lower.strip_prefix("blockgame.claim.limit.") {
                    if let Ok(n) = suffix.parse::<usize>() {
                        best = Some(best.map_or(n, |cur| cur.max(n)));
                    }
                }
            }
        };
        if let Some(p) = perms.players.get(&canonical) {
            scan_nodes(&p.permissions);
        }
        for gname in perms.get_effective_groups(&canonical) {
            if let Some(g) = perms.groups.get(&gname) {
                scan_nodes(&g.permissions);
            }
        }
        best.unwrap_or(self.settings.default_max_claims)
    }

    pub fn count_player_claims(&self, username: &str) -> usize {
        self.chunks
            .values()
            .filter(|o| matches!(o, ClaimOwner::Player { username: u } if u.eq_ignore_ascii_case(username)))
            .count()
    }

    pub fn list_player_claims(&self, username: &str) -> Vec<(i32, i32)> {
        let mut list: Vec<(i32, i32)> = self
            .chunks
            .iter()
            .filter_map(|(&coord, owner)| match owner {
                ClaimOwner::Player { username: u } if u.eq_ignore_ascii_case(username) => Some(coord),
                _ => None,
            })
            .collect();
        list.sort();
        list
    }

    pub fn get_owner(&self, coord: (i32, i32)) -> Option<&ClaimOwner> {
        self.chunks.get(&coord)
    }

    pub fn is_trusted(&self, owner: &str, candidate: &str) -> bool {
        owner.eq_ignore_ascii_case(candidate)
            || self
                .trust_lists
                .get(&owner.trim().to_ascii_lowercase())
                .is_some_and(|set| set.contains(&candidate.trim().to_ascii_lowercase()))
    }

    pub fn is_bypassing(&self, username: &str) -> bool {
        self.admin_bypass.contains(&username.trim().to_ascii_lowercase())
    }

    pub fn toggle_admin_bypass(&mut self, username: &str) -> bool {
        let key = username.trim().to_ascii_lowercase();
        if self.admin_bypass.contains(&key) {
            self.admin_bypass.remove(&key);
            false
        } else {
            self.admin_bypass.insert(key);
            true
        }
    }

    pub fn is_in_spawn_protection(&self, coord: (i32, i32), spawn: Vec3) -> bool {
        let r = self.settings.spawn_protection_radius.max(0);
        if r == 0 {
            return false;
        }
        let min_x = coord.0 * 16;
        let max_x = min_x + 15;
        let min_z = coord.1 * 16;
        let max_z = min_z + 15;
        let sx = spawn.x.floor() as i32;
        let sz = spawn.z.floor() as i32;
        let nearest_x = sx.clamp(min_x, max_x);
        let nearest_z = sz.clamp(min_z, max_z);
        let dx = (nearest_x - sx) as i64;
        let dz = (nearest_z - sz) as i64;
        dx * dx + dz * dz <= (r as i64) * (r as i64)
    }

    pub fn claim_chunk(
        &mut self,
        coord: (i32, i32),
        username: &str,
        perms: &PermissionsManager,
        spawn: Vec3,
        intersects_protected_landmark: bool,
    ) -> Result<(), String> {
        if let Some(existing) = self.chunks.get(&coord) {
            return Err(format!(
                "Chunk ({}, {}) is already claimed ({}).",
                coord.0,
                coord.1,
                existing.display_name()
            ));
        }
        if self.is_in_spawn_protection(coord, spawn) {
            return Err("You cannot claim chunks inside the spawn protection area.".to_string());
        }
        if intersects_protected_landmark {
            return Err("You cannot claim chunks inside a Village or Temple.".to_string());
        }
        let current = self.count_player_claims(username);
        let max = self.max_claims_for_player(perms, username);
        if current >= max {
            return Err(format!(
                "Claim limit reached ({}/{} chunks).",
                current, max
            ));
        }
        self.chunks.insert(
            coord,
            ClaimOwner::Player {
                username: username.trim().to_string(),
            },
        );
        Ok(())
    }

    pub fn unclaim_chunk(&mut self, coord: (i32, i32), username: &str) -> Result<(), String> {
        match self.chunks.get(&coord) {
            Some(ClaimOwner::Player { username: u }) if u.eq_ignore_ascii_case(username) => {
                self.chunks.remove(&coord);
                Ok(())
            }
            Some(other) => Err(format!(
                "You do not own this chunk (owned by {}).",
                other.display_name()
            )),
            None => Err("This chunk is not claimed.".to_string()),
        }
    }

    pub fn unclaim_all(&mut self, username: &str) -> usize {
        let before = self.chunks.len();
        self.chunks.retain(|_, o| {
            !matches!(o, ClaimOwner::Player { username: u } if u.eq_ignore_ascii_case(username))
        });
        before - self.chunks.len()
    }

    pub fn admin_claim(&mut self, coord: (i32, i32), label: Option<&str>) -> Result<String, String> {
        let final_label = label
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Admin Claim")
            .to_string();
        self.chunks.insert(
            coord,
            ClaimOwner::Admin {
                label: final_label.clone(),
            },
        );
        Ok(final_label)
    }

    pub fn admin_unclaim(&mut self, coord: (i32, i32)) -> Result<ClaimOwner, String> {
        self.chunks
            .remove(&coord)
            .ok_or_else(|| "This chunk is not claimed.".to_string())
    }

    pub fn trust(&mut self, owner: &str, friend: &str) -> Result<(), String> {
        let okey = owner.trim().to_ascii_lowercase();
        let fkey = friend.trim().to_ascii_lowercase();
        if fkey.is_empty() {
            return Err("Provide a player name to trust.".to_string());
        }
        if okey == fkey {
            return Err("You already have access to your own claims.".to_string());
        }
        self.trust_lists.entry(okey).or_default().insert(fkey);
        Ok(())
    }

    pub fn untrust(&mut self, owner: &str, friend: &str) -> bool {
        let okey = owner.trim().to_ascii_lowercase();
        let fkey = friend.trim().to_ascii_lowercase();
        self.trust_lists
            .get_mut(&okey)
            .is_some_and(|set| set.remove(&fkey))
    }

    pub fn trust_list(&self, owner: &str) -> Vec<String> {
        let okey = owner.trim().to_ascii_lowercase();
        let mut list: Vec<String> = self
            .trust_lists
            .get(&okey)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        list.sort();
        list
    }

    /// Authoritative check for block/container/animal modifications at `(chunk_x, chunk_z)`.
    pub fn check_modify_at(
        &self,
        coord: (i32, i32),
        username: &str,
        can_edit_admin_claims: bool,
    ) -> Result<(), String> {
        let Some(owner) = self.chunks.get(&coord) else {
            return Ok(());
        };
        if self.is_bypassing(username) {
            return Ok(());
        }
        match owner {
            ClaimOwner::Player { username: owner_name } => {
                if self.is_trusted(owner_name, username) {
                    Ok(())
                } else {
                    Err(format!("This chunk is claimed by {}.", owner_name))
                }
            }
            ClaimOwner::Admin { .. } => {
                if can_edit_admin_claims {
                    Ok(())
                } else {
                    Err(format!("This chunk is protected ({}).", owner.display_name()))
                }
            }
        }
    }

    /// PvP is disabled whenever EITHER the attacker OR the target stands in any Claimed Chunk or Admin Claim.
    pub fn is_pvp_allowed(&self, attacker_pos: Vec3, target_pos: Vec3) -> bool {
        let c1 = world_pos_to_claim_coord(attacker_pos);
        let c2 = world_pos_to_claim_coord(target_pos);
        !self.chunks.contains_key(&c1) && !self.chunks.contains_key(&c2)
    }

    /// Blocks fluids from flowing across a claim boundary into a chunk with a different owner.
    pub fn can_fluid_flow_into(&self, from: IVec3, to: IVec3) -> bool {
        let c_from = block_pos_to_claim_coord(from);
        let c_to = block_pos_to_claim_coord(to);
        if c_from == c_to {
            return true;
        }
        match self.chunks.get(&c_to) {
            None => true,
            Some(to_owner) => self
                .chunks
                .get(&c_from)
                .is_some_and(|from_owner| from_owner.is_same_owner(to_owner)),
        }
    }

    pub fn relation_for_viewer(&self, owner: &ClaimOwner, viewer: &str) -> ClaimRelation {
        match owner {
            ClaimOwner::Admin { .. } => ClaimRelation::Admin,
            ClaimOwner::Player { username } => {
                if username.eq_ignore_ascii_case(viewer) {
                    ClaimRelation::Own
                } else if self.is_trusted(username, viewer) {
                    ClaimRelation::Trusted
                } else {
                    ClaimRelation::Other
                }
            }
        }
    }

    /// Builds a compact color-coded ASCII chunk grid centered on `center` for `/claim map`.
    pub fn render_claim_map(&self, center: (i32, i32), viewer: &str) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!(
            "&6Claim Map around ({}, {}) &7[&aY&7=Yours &bT&7=Trusted &cX&7=Other &eA&7=Admin &8.&7=Wild]",
            center.0, center.1
        ));
        for dz in -2..=2 {
            let mut row = String::from("  ");
            for dx in -4..=4 {
                let c = (center.0 + dx, center.1 + dz);
                let is_center = dx == 0 && dz == 0;
                let cell = match self.chunks.get(&c) {
                    None => if is_center { "&f+" } else { "&8." },
                    Some(owner) => match self.relation_for_viewer(owner, viewer) {
                        ClaimRelation::Own => if is_center { "&a@" } else { "&aY" },
                        ClaimRelation::Trusted => if is_center { "&b@" } else { "&bT" },
                        ClaimRelation::Other => if is_center { "&c@" } else { "&cX" },
                        ClaimRelation::Admin => if is_center { "&e@" } else { "&eA" },
                    },
                };
                row.push_str(cell);
                row.push(' ');
            }
            lines.push(row);
        }
        lines
    }

    /// Collects nearby claimed chunks within `radius_chunks` of `center` for client sync (#38).
    pub fn nearby_claims_for_client(
        &self,
        center: (i32, i32),
        radius_chunks: i32,
        viewer: &str,
        can_edit_admin: bool,
    ) -> Vec<SyncedClaimChunk> {
        let mut out = Vec::new();
        for (&(cx, cz), owner) in &self.chunks {
            if (cx - center.0).abs() <= radius_chunks && (cz - center.1).abs() <= radius_chunks {
                out.push(SyncedClaimChunk {
                    chunk_x: cx,
                    chunk_z: cz,
                    relation: self.relation_for_viewer(owner, viewer),
                    display_name: owner.display_name(),
                    can_modify: self.check_modify_at((cx, cz), viewer, can_edit_admin).is_ok(),
                });
            }
        }
        out.sort_by_key(|c| (c.chunk_x, c.chunk_z));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_geometry_trust_pvp_fluid_and_persistence() {
        let root = std::env::temp_dir().join(format!("blockgame_claims_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let cfg_dir = root.join("config");
        let world_dir = root.join("world");
        fs::create_dir_all(&world_dir).unwrap();

        let mut perms = PermissionsManager::new();
        let mut claims = ClaimManager::load(&cfg_dir, &world_dir);
        assert!(cfg_dir.join("claims.yml").exists());

        // Claim outside spawn radius (spawn at 0,0, radius 32 -> chunk (5,5) is safe)
        let spawn = Vec3::new(0.0, 64.0, 0.0);
        assert!(claims.claim_chunk((0, 0), "Alice", &perms, spawn, false).is_err());
        assert!(claims.claim_chunk((5, 5), "Alice", &perms, spawn, true).is_err());
        claims.claim_chunk((5, 5), "Alice", &perms, spawn, false).unwrap();

        // Any Y in (5,5) is protected
        assert_eq!(block_pos_to_claim_coord(IVec3::new(80, -30, 80)), (5, 5));
        assert_eq!(block_pos_to_claim_coord(IVec3::new(95, 250, 95)), (5, 5));
        assert!(claims.check_modify_at((5, 5), "Alice", false).is_ok());
        assert!(claims.check_modify_at((5, 5), "Bob", false).is_err());

        // Trust Bob
        claims.trust("Alice", "Bob").unwrap();
        assert!(claims.check_modify_at((5, 5), "Bob", false).is_ok());

        // PvP disabled if EITHER player is inside a claim
        assert!(!claims.is_pvp_allowed(Vec3::new(82.0, 64.0, 82.0), Vec3::new(50.0, 64.0, 50.0)));
        assert!(!claims.is_pvp_allowed(Vec3::new(50.0, 64.0, 50.0), Vec3::new(82.0, 64.0, 82.0)));
        assert!(claims.is_pvp_allowed(Vec3::new(50.0, 64.0, 50.0), Vec3::new(52.0, 64.0, 52.0)));

        // Cross-border fluid flow into claim is blocked
        assert!(!claims.can_fluid_flow_into(IVec3::new(79, 64, 80), IVec3::new(80, 64, 80)));
        assert!(claims.can_fluid_flow_into(IVec3::new(80, 64, 80), IVec3::new(81, 64, 80)));

        // Admin Claim allowed inside spawn
        claims.admin_claim((0, 0), Some("Spawn Plaza")).unwrap();
        assert!(claims.check_modify_at((0, 0), "Bob", false).is_err());
        assert!(claims.check_modify_at((0, 0), "Admin", true).is_ok());

        // Claim Bypass toggle
        assert!(claims.check_modify_at((5, 5), "Charlie", false).is_err());
        assert!(claims.toggle_admin_bypass("Charlie"));
        assert!(claims.check_modify_at((5, 5), "Charlie", false).is_ok());

        // Permission override limit
        let _ = perms.add_player_permission("Alice", "blockgame.claim.limit.32");
        assert_eq!(claims.max_claims_for_player(&perms, "Alice"), 32);

        // Save and reload world-bound claims
        claims.save_world_claims(&world_dir).unwrap();
        let reloaded = ClaimManager::load(&cfg_dir, &world_dir);
        assert_eq!(reloaded.count_player_claims("Alice"), 1);
        assert!(reloaded.is_trusted("Alice", "Bob"));
        assert!(matches!(
            reloaded.get_owner((0, 0)),
            Some(ClaimOwner::Admin { label }) if label == "Spawn Plaza"
        ));

        let _ = fs::remove_dir_all(&root);
    }
}
