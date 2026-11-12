use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use glam::Vec3;

use crate::{
    item::{Inventory, ItemId, ItemSlot},
    player::PlayerStats,
    save::SavedPlayerState,
};

const AUTH_MAGIC: &[u8; 4] = b"BGAU";
const AUTH_VERSION: u32 = 1;

/// Standard FIPS 180-4 SHA-256 round constants.
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Computes the cryptographic SHA-256 digest of `data` in pure Rust.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Validates a multiplayer username.
/// Rules: 3..=16 characters, ASCII alphanumeric or `_`.
pub fn validate_username(username: &str) -> Result<(), String> {
    let trimmed = username.trim();
    if trimmed.len() < 3 {
        return Err("Username must be at least 3 characters long.".to_string());
    }
    if trimmed.len() > 16 {
        return Err("Username cannot exceed 16 characters.".to_string());
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err("Username may only contain letters, numbers, and underscores.".to_string());
    }
    Ok(())
}

/// Derives a client-side authentication token from `(username, password)` so raw passwords
/// are never transmitted over the network in plaintext.
pub fn derive_client_password_token(username: &str, password: &str) -> [u8; 32] {
    let canonical_user = username.trim().to_ascii_lowercase();
    let mut buf = Vec::with_capacity(canonical_user.len() + password.len() + 32);
    buf.extend_from_slice(b"BlockGameClientAuth_v1:");
    buf.extend_from_slice(canonical_user.as_bytes());
    buf.push(b':');
    buf.extend_from_slice(password.as_bytes());

    let mut digest = sha256(&buf);
    // 256 rounds of key stretching on the client side
    for round in 0..256u32 {
        let mut round_buf = [0u8; 36];
        round_buf[..32].copy_from_slice(&digest);
        round_buf[32..36].copy_from_slice(&round.to_le_bytes());
        digest = sha256(&round_buf);
    }
    digest
}

/// Server-side salted key-stretching hash (1,000 rounds of SHA-256) over `client_token` and `salt`.
pub fn hash_server_password(client_token: &[u8; 32], salt: &[u8; 16]) -> [u8; 32] {
    let mut initial = [0u8; 48];
    initial[..32].copy_from_slice(client_token);
    initial[32..48].copy_from_slice(salt);
    let mut digest = sha256(&initial);

    for round in 0..1000u32 {
        let mut buf = [0u8; 52];
        buf[..32].copy_from_slice(&digest);
        buf[32..48].copy_from_slice(salt);
        buf[48..52].copy_from_slice(&round.to_le_bytes());
        digest = sha256(&buf);
    }
    digest
}

/// Constant-time comparison of two 32-byte digests to prevent timing side-channels.
pub fn constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Generates a pseudo-random 16-byte salt or session token using system entropy and SplitMix64.
pub fn generate_random_16_bytes(counter: u64) -> [u8; 16] {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e3779b97f4a7c15);

    let mut z = nanos ^ counter.wrapping_mul(0xbf58476d1ce4e5b9);
    let mut next_u64 = || {
        z = z.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = z;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        x ^ (x >> 31)
    };

    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&next_u64().to_le_bytes());
    out[8..16].copy_from_slice(&next_u64().to_le_bytes());
    out
}

/// Persistent account record stored on the multiplayer server.
#[derive(Debug, Clone, PartialEq)]
pub struct UserAccount {
    pub username: String,
    pub salt: [u8; 16],
    pub password_hash: [u8; 32],
    pub skin_id: u8,
    pub player_state: SavedPlayerState,
}

/// Server-side authentication & player account database (`users.dat`).
#[derive(Debug, Clone, Default)]
pub struct AuthDatabase {
    /// Keyed by lowercase canonical username
    pub accounts: HashMap<String, UserAccount>,
    nonce_counter: u64,
}

impl AuthDatabase {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            nonce_counter: 1,
        }
    }

    /// Authenticates an existing user or registers a new user account if the username is unclaimed.
    /// Returns `(UserAccount, session_token, is_new_registration)`.
    pub fn authenticate_or_register(
        &mut self,
        username: &str,
        client_password_token: &[u8; 32],
        requested_skin_id: u8,
        default_spawn: Vec3,
    ) -> Result<(UserAccount, [u8; 16], bool), String> {
        validate_username(username)?;
        let display_name = username.trim().to_string();
        let key = display_name.to_ascii_lowercase();

        self.nonce_counter = self.nonce_counter.wrapping_add(1);
        let session_token = generate_random_16_bytes(self.nonce_counter);

        if let Some(account) = self.accounts.get_mut(&key) {
            let expected = hash_server_password(client_password_token, &account.salt);
            if !constant_time_eq_32(&expected, &account.password_hash) {
                return Err(format!(
                    "Invalid password for registered user '{}'.",
                    account.username
                ));
            }
            account.skin_id = requested_skin_id % 4;
            Ok((account.clone(), session_token, false))
        } else {
            self.nonce_counter = self.nonce_counter.wrapping_add(1);
            let salt = generate_random_16_bytes(self.nonce_counter ^ 0x517cc1b727220a95);
            let password_hash = hash_server_password(client_password_token, &salt);

            let mut player_state = SavedPlayerState::default();
            player_state.position = default_spawn;

            let account = UserAccount {
                username: display_name,
                salt,
                password_hash,
                skin_id: requested_skin_id % 4,
                player_state,
            };
            self.accounts.insert(key, account.clone());
            Ok((account, session_token, true))
        }
    }

    /// Updates a user's persisted player state (position, inventory, health/hunger, skin).
    pub fn update_player_state(&mut self, username: &str, skin_id: u8, state: SavedPlayerState) {
        let key = username.trim().to_ascii_lowercase();
        if let Some(account) = self.accounts.get_mut(&key) {
            account.skin_id = skin_id % 4;
            account.player_state = state;
        }
    }

    /// Saves the authentication database to `<world_dir>/users.dat` and individual player files to `<world_dir>/players/<username>.dat`.
    pub fn save_to_dir(&self, world_dir: &Path) -> io::Result<()> {
        fs::create_dir_all(world_dir)?;

        // 1. Save each player's state into their dedicated individual file in players/
        for account in self.accounts.values() {
            let _ = crate::save::save_player_data(world_dir, &account.username, &account.player_state);
        }

        // 2. Persist account credentials to users.dat
        let file = File::create(world_dir.join("users.dat"))?;
        let mut w = BufWriter::new(file);

        w.write_all(AUTH_MAGIC)?;
        w.write_all(&AUTH_VERSION.to_le_bytes())?;
        w.write_all(&(self.accounts.len() as u32).to_le_bytes())?;

        for account in self.accounts.values() {
            let name_bytes = account.username.as_bytes();
            w.write_all(&(name_bytes.len() as u32).to_le_bytes())?;
            w.write_all(name_bytes)?;
            w.write_all(&account.salt)?;
            w.write_all(&account.password_hash)?;
            w.write_all(&[account.skin_id])?;

            let p = &account.player_state;
            for f in [
                p.position.x,
                p.position.y,
                p.position.z,
                p.velocity.x,
                p.velocity.y,
                p.velocity.z,
                p.yaw,
                p.pitch,
                p.stats.health,
                p.stats.max_health,
                p.stats.hunger,
                p.stats.max_hunger,
                p.stats.saturation,
                p.stats.exhaustion,
                p.stats.breath,
                p.stats.max_breath,
            ] {
                w.write_all(&f.to_le_bytes())?;
            }
            w.write_all(&[if p.stats.is_dead { 1 } else { 0 }])?;
            w.write_all(&(p.active_hotbar_slot as u32).to_le_bytes())?;

            w.write_all(&(p.inventory.slots.len() as u32).to_le_bytes())?;
            for slot in &p.inventory.slots {
                w.write_all(&slot.item_id.0.to_le_bytes())?;
                w.write_all(&slot.count.to_le_bytes())?;
                w.write_all(&slot.durability.to_le_bytes())?;
            }
        }

        w.flush()
    }

    /// Loads the authentication database from `<world_dir>/users.dat` and individual player files.
    pub fn load_from_dir(world_dir: &Path) -> io::Result<Self> {
        let path = world_dir.join("users.dat");
        if !path.exists() {
            return Ok(Self::new());
        }

        let file = File::open(path)?;
        let mut r = BufReader::new(file);

        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if &magic != AUTH_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid users.dat magic header",
            ));
        }

        let version = read_u32(&mut r)?;
        if version != AUTH_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported users.dat version: {}", version),
            ));
        }

        let count = read_u32(&mut r)? as usize;
        let mut db = Self::new();

        for _ in 0..count {
            let name_len = read_u32(&mut r)? as usize;
            let mut name_buf = vec![0u8; name_len];
            r.read_exact(&mut name_buf)?;
            let username = String::from_utf8(name_buf)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            let mut salt = [0u8; 16];
            r.read_exact(&mut salt)?;
            let mut password_hash = [0u8; 32];
            r.read_exact(&mut password_hash)?;
            let mut skin_buf = [0u8; 1];
            r.read_exact(&mut skin_buf)?;
            let skin_id = skin_buf[0];

            let position = Vec3::new(read_f32(&mut r)?, read_f32(&mut r)?, read_f32(&mut r)?);
            let velocity = Vec3::new(read_f32(&mut r)?, read_f32(&mut r)?, read_f32(&mut r)?);
            let yaw = read_f32(&mut r)?;
            let pitch = read_f32(&mut r)?;

            let mut stats = PlayerStats::new();
            stats.health = read_f32(&mut r)?;
            stats.max_health = read_f32(&mut r)?;
            stats.hunger = read_f32(&mut r)?;
            stats.max_hunger = read_f32(&mut r)?;
            stats.saturation = read_f32(&mut r)?;
            stats.exhaustion = read_f32(&mut r)?;
            stats.breath = read_f32(&mut r)?;
            stats.max_breath = read_f32(&mut r)?;

            let mut dead_buf = [0u8; 1];
            r.read_exact(&mut dead_buf)?;
            stats.is_dead = dead_buf[0] != 0;

            let active_hotbar_slot = (read_u32(&mut r)? as usize).min(8);
            let slot_count = read_u32(&mut r)? as usize;
            let mut inventory = Inventory::default();
            for i in 0..slot_count {
                let id = ItemId(read_u16(&mut r)?);
                let cnt = read_u16(&mut r)?;
                let dur = read_u16(&mut r)?;
                if i < Inventory::TOTAL_COUNT && id != ItemId::EMPTY && cnt > 0 {
                    inventory.slots[i] = ItemSlot::with_durability(id, cnt, dur);
                }
            }

            // Check dedicated player file in players/<username>.dat
            let player_state = if let Ok(Some(loaded_state)) = crate::save::load_player_data(world_dir, &username) {
                loaded_state
            } else if world_dir.join("players").exists() {
                // players/ directory exists but this player file was deleted (reset player)
                SavedPlayerState {
                    position,
                    velocity: Vec3::ZERO,
                    yaw,
                    pitch,
                    stats: PlayerStats::new(),
                    inventory: Inventory::starter_inventory(),
                    active_hotbar_slot: 0,
                    rank: "default".to_string(),
                }
            } else {
                // Legacy world: fallback to player state saved in users.dat
                SavedPlayerState {
                    position,
                    velocity,
                    yaw,
                    pitch,
                    stats,
                    inventory,
                    active_hotbar_slot,
                    rank: "default".to_string(),
                }
            };

            let key = username.to_ascii_lowercase();
            db.accounts.insert(
                key,
                UserAccount {
                    username,
                    salt,
                    password_hash,
                    skin_id,
                    player_state,
                },
            );
        }

        Ok(db)
    }
}

fn read_u16<R: Read>(r: &mut R) -> io::Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}

fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_f32<R: Read>(r: &mut R) -> io::Result<f32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(f32::from_le_bytes(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_nist_vector() {
        // NIST SHA-256 test vector for "abc":
        // ba7816bf 8f01cfea 414140de 5dae2223 b00361a3 96177a9c b410ff61 f20015ad
        let digest = sha256(b"abc");
        let expected: [u8; 32] = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea,
            0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
            0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c,
            0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_username_validation() {
        assert!(validate_username("Alice_99").is_ok());
        assert!(validate_username("ab").is_err()); // Too short
        assert!(validate_username("this_username_is_way_too_long_123").is_err()); // Too long
        assert!(validate_username("bad name!").is_err()); // Invalid chars
    }

    #[test]
    fn test_auth_registration_and_login_persistence() {
        let mut db = AuthDatabase::new();
        let spawn = Vec3::new(0.5, 70.0, 0.5);

        let token_correct = derive_client_password_token("MinerSteve", "secret123");
        let token_wrong = derive_client_password_token("MinerSteve", "wrongpass");

        // 1. First login registers the user
        let (acc, _session, is_new) = db
            .authenticate_or_register("MinerSteve", &token_correct, 2, spawn)
            .expect("First login should register");
        assert!(is_new);
        assert_eq!(acc.username, "MinerSteve");
        assert_eq!(acc.skin_id, 2);

        // 2. Returning login with wrong password fails
        let err = db.authenticate_or_register("minersteve", &token_wrong, 1, spawn);
        assert!(err.is_err());

        // 3. Returning login with right password succeeds
        let (acc2, _session2, is_new2) = db
            .authenticate_or_register("MinerSteve", &token_correct, 3, spawn)
            .expect("Returning login should succeed");
        assert!(!is_new2);
        assert_eq!(acc2.skin_id, 3);

        // 4. Save and load roundtrip
        let temp_dir = std::env::temp_dir().join("blockgame_auth_test_dir");
        let _ = fs::remove_dir_all(&temp_dir);
        db.save_to_dir(&temp_dir).unwrap();

        let mut loaded_db = AuthDatabase::load_from_dir(&temp_dir).unwrap();
        let (acc3, _, is_new3) = loaded_db
            .authenticate_or_register("MinerSteve", &token_correct, 3, spawn)
            .unwrap();
        assert!(!is_new3);
        assert_eq!(acc3.username, "MinerSteve");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
