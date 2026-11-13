use std::path::Path;
use winit::keyboard::KeyCode;

/// Remappable gameplay keybindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinds {
    pub forward: KeyCode,
    pub backward: KeyCode,
    pub left: KeyCode,
    pub right: KeyCode,
    pub jump: KeyCode,
    pub sprint: KeyCode,
    pub inventory: KeyCode,
    pub flight: KeyCode,
    pub chat: KeyCode,
    pub player_list: KeyCode,
    pub third_person: KeyCode,
    pub debug_hud: KeyCode,
}

impl Default for KeyBinds {
    fn default() -> Self {
        Self {
            forward: KeyCode::KeyW,
            backward: KeyCode::KeyS,
            left: KeyCode::KeyA,
            right: KeyCode::KeyD,
            jump: KeyCode::Space,
            sprint: KeyCode::ControlLeft,
            inventory: KeyCode::KeyE,
            flight: KeyCode::KeyF,
            chat: KeyCode::KeyT,
            player_list: KeyCode::Tab,
            third_person: KeyCode::F5,
            debug_hud: KeyCode::F3,
        }
    }
}

impl KeyBinds {
    pub const COUNT: usize = 12;

    pub fn labels_and_keys(&self) -> [(&'static str, &'static str); Self::COUNT] {
        [
            ("Move Forward", key_display_name(self.forward)),
            ("Move Backward", key_display_name(self.backward)),
            ("Strafe Left", key_display_name(self.left)),
            ("Strafe Right", key_display_name(self.right)),
            ("Jump / Fly Up", key_display_name(self.jump)),
            ("Sprint", key_display_name(self.sprint)),
            ("Inventory", key_display_name(self.inventory)),
            ("Toggle Flight", key_display_name(self.flight)),
            ("Open Chat", key_display_name(self.chat)),
            ("Player List", key_display_name(self.player_list)),
            ("3rd Person", key_display_name(self.third_person)),
            ("Debug HUD", key_display_name(self.debug_hud)),
        ]
    }

    pub fn set_by_index(&mut self, index: usize, key: KeyCode) {
        match index {
            0 => self.forward = key,
            1 => self.backward = key,
            2 => self.left = key,
            3 => self.right = key,
            4 => self.jump = key,
            5 => self.sprint = key,
            6 => self.inventory = key,
            7 => self.flight = key,
            8 => self.chat = key,
            9 => self.player_list = key,
            10 => self.third_person = key,
            11 => self.debug_hud = key,
            _ => {}
        }
    }
}

pub fn key_code_to_str(key: KeyCode) -> Option<&'static str> {
    Some(match key {
        KeyCode::KeyA => "KeyA",
        KeyCode::KeyB => "KeyB",
        KeyCode::KeyC => "KeyC",
        KeyCode::KeyD => "KeyD",
        KeyCode::KeyE => "KeyE",
        KeyCode::KeyF => "KeyF",
        KeyCode::KeyG => "KeyG",
        KeyCode::KeyH => "KeyH",
        KeyCode::KeyI => "KeyI",
        KeyCode::KeyJ => "KeyJ",
        KeyCode::KeyK => "KeyK",
        KeyCode::KeyL => "KeyL",
        KeyCode::KeyM => "KeyM",
        KeyCode::KeyN => "KeyN",
        KeyCode::KeyO => "KeyO",
        KeyCode::KeyP => "KeyP",
        KeyCode::KeyQ => "KeyQ",
        KeyCode::KeyR => "KeyR",
        KeyCode::KeyS => "KeyS",
        KeyCode::KeyT => "KeyT",
        KeyCode::KeyU => "KeyU",
        KeyCode::KeyV => "KeyV",
        KeyCode::KeyW => "KeyW",
        KeyCode::KeyX => "KeyX",
        KeyCode::KeyY => "KeyY",
        KeyCode::KeyZ => "KeyZ",
        KeyCode::Digit0 => "Digit0",
        KeyCode::Digit1 => "Digit1",
        KeyCode::Digit2 => "Digit2",
        KeyCode::Digit3 => "Digit3",
        KeyCode::Digit4 => "Digit4",
        KeyCode::Digit5 => "Digit5",
        KeyCode::Digit6 => "Digit6",
        KeyCode::Digit7 => "Digit7",
        KeyCode::Digit8 => "Digit8",
        KeyCode::Digit9 => "Digit9",
        KeyCode::Space => "Space",
        KeyCode::ShiftLeft => "ShiftLeft",
        KeyCode::ShiftRight => "ShiftRight",
        KeyCode::ControlLeft => "ControlLeft",
        KeyCode::ControlRight => "ControlRight",
        KeyCode::AltLeft => "AltLeft",
        KeyCode::AltRight => "AltRight",
        KeyCode::Tab => "Tab",
        KeyCode::Enter => "Enter",
        KeyCode::Backquote => "Backquote",
        KeyCode::Slash => "Slash",
        KeyCode::ArrowUp => "ArrowUp",
        KeyCode::ArrowDown => "ArrowDown",
        KeyCode::ArrowLeft => "ArrowLeft",
        KeyCode::ArrowRight => "ArrowRight",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        _ => return None,
    })
}

pub fn str_to_key_code(s: &str) -> Option<KeyCode> {
    Some(match s.trim() {
        "KeyA" => KeyCode::KeyA,
        "KeyB" => KeyCode::KeyB,
        "KeyC" => KeyCode::KeyC,
        "KeyD" => KeyCode::KeyD,
        "KeyE" => KeyCode::KeyE,
        "KeyF" => KeyCode::KeyF,
        "KeyG" => KeyCode::KeyG,
        "KeyH" => KeyCode::KeyH,
        "KeyI" => KeyCode::KeyI,
        "KeyJ" => KeyCode::KeyJ,
        "KeyK" => KeyCode::KeyK,
        "KeyL" => KeyCode::KeyL,
        "KeyM" => KeyCode::KeyM,
        "KeyN" => KeyCode::KeyN,
        "KeyO" => KeyCode::KeyO,
        "KeyP" => KeyCode::KeyP,
        "KeyQ" => KeyCode::KeyQ,
        "KeyR" => KeyCode::KeyR,
        "KeyS" => KeyCode::KeyS,
        "KeyT" => KeyCode::KeyT,
        "KeyU" => KeyCode::KeyU,
        "KeyV" => KeyCode::KeyV,
        "KeyW" => KeyCode::KeyW,
        "KeyX" => KeyCode::KeyX,
        "KeyY" => KeyCode::KeyY,
        "KeyZ" => KeyCode::KeyZ,
        "Digit0" => KeyCode::Digit0,
        "Digit1" => KeyCode::Digit1,
        "Digit2" => KeyCode::Digit2,
        "Digit3" => KeyCode::Digit3,
        "Digit4" => KeyCode::Digit4,
        "Digit5" => KeyCode::Digit5,
        "Digit6" => KeyCode::Digit6,
        "Digit7" => KeyCode::Digit7,
        "Digit8" => KeyCode::Digit8,
        "Digit9" => KeyCode::Digit9,
        "Space" => KeyCode::Space,
        "ShiftLeft" => KeyCode::ShiftLeft,
        "ShiftRight" => KeyCode::ShiftRight,
        "ControlLeft" => KeyCode::ControlLeft,
        "ControlRight" => KeyCode::ControlRight,
        "AltLeft" => KeyCode::AltLeft,
        "AltRight" => KeyCode::AltRight,
        "Tab" => KeyCode::Tab,
        "Enter" => KeyCode::Enter,
        "Backquote" => KeyCode::Backquote,
        "Slash" => KeyCode::Slash,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        "F1" => KeyCode::F1,
        "F2" => KeyCode::F2,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "F5" => KeyCode::F5,
        "F6" => KeyCode::F6,
        "F7" => KeyCode::F7,
        "F8" => KeyCode::F8,
        "F9" => KeyCode::F9,
        "F10" => KeyCode::F10,
        "F11" => KeyCode::F11,
        "F12" => KeyCode::F12,
        _ => return None,
    })
}

pub fn key_display_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::KeyA => "A",
        KeyCode::KeyB => "B",
        KeyCode::KeyC => "C",
        KeyCode::KeyD => "D",
        KeyCode::KeyE => "E",
        KeyCode::KeyF => "F",
        KeyCode::KeyG => "G",
        KeyCode::KeyH => "H",
        KeyCode::KeyI => "I",
        KeyCode::KeyJ => "J",
        KeyCode::KeyK => "K",
        KeyCode::KeyL => "L",
        KeyCode::KeyM => "M",
        KeyCode::KeyN => "N",
        KeyCode::KeyO => "O",
        KeyCode::KeyP => "P",
        KeyCode::KeyQ => "Q",
        KeyCode::KeyR => "R",
        KeyCode::KeyS => "S",
        KeyCode::KeyT => "T",
        KeyCode::KeyU => "U",
        KeyCode::KeyV => "V",
        KeyCode::KeyW => "W",
        KeyCode::KeyX => "X",
        KeyCode::KeyY => "Y",
        KeyCode::KeyZ => "Z",
        KeyCode::Digit0 => "0",
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6",
        KeyCode::Digit7 => "7",
        KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        KeyCode::Space => "Space",
        KeyCode::ShiftLeft => "L-Shift",
        KeyCode::ShiftRight => "R-Shift",
        KeyCode::ControlLeft => "L-Ctrl",
        KeyCode::ControlRight => "R-Ctrl",
        KeyCode::AltLeft => "L-Alt",
        KeyCode::AltRight => "R-Alt",
        KeyCode::Tab => "Tab",
        KeyCode::Enter => "Enter",
        KeyCode::Backquote => "`",
        KeyCode::Slash => "/",
        KeyCode::ArrowUp => "Up",
        KeyCode::ArrowDown => "Down",
        KeyCode::ArrowLeft => "Left",
        KeyCode::ArrowRight => "Right",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        _ => "Unknown",
    }
}

/// Persistent client-side configuration settings.
#[derive(Debug, Clone, PartialEq)]
pub struct GameSettings {
    /// Chunk render radius around the player (`2..=12`).
    pub render_distance: i32,
    /// Vertical field of view in degrees (`50.0..=110.0`).
    pub fov_degrees: f32,
    /// Mouse look sensitivity multiplier (`0.2..=2.5`, where `1.0` = default).
    pub mouse_sensitivity: f32,
    /// Master audio volume (`0.0..=1.0`).
    pub master_volume: f32,
    /// Sound effects volume (`0.0..=1.0`).
    pub sfx_volume: f32,
    /// Background music volume (`0.0..=1.0`).
    pub music_volume: f32,
    /// Invert vertical mouse look axis.
    pub invert_y: bool,
    /// Borderless fullscreen window mode.
    pub fullscreen: bool,
    /// Vertical synchronization (VSync).
    pub vsync: bool,
    /// First-person held item / hand view bobbing while walking.
    pub view_bobbing: bool,
    /// Show top-left FPS and coordinate debug HUD overlay (also toggled via F3).
    pub show_debug_hud: bool,
    /// Customizable gameplay keybindings.
    pub keybinds: KeyBinds,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            render_distance: 4,
            fov_degrees: 60.0,
            mouse_sensitivity: 1.0,
            master_volume: 0.8,
            sfx_volume: 0.9,
            music_volume: 0.65,
            invert_y: false,
            fullscreen: false,
            vsync: true,
            view_bobbing: true,
            show_debug_hud: false,
            keybinds: KeyBinds::default(),
        }
    }
}

impl GameSettings {
    pub const MIN_RENDER_DISTANCE: i32 = 2;
    pub const MAX_RENDER_DISTANCE: i32 = 12;
    pub const MIN_FOV: f32 = 50.0;
    pub const MAX_FOV: f32 = 110.0;
    pub const MIN_SENSITIVITY: f32 = 0.2;
    pub const MAX_SENSITIVITY: f32 = 2.5;

    /// Clamps all numeric settings to valid ranges.
    pub fn clamp_all(&mut self) {
        self.render_distance = self
            .render_distance
            .clamp(Self::MIN_RENDER_DISTANCE, Self::MAX_RENDER_DISTANCE);
        self.fov_degrees = self.fov_degrees.clamp(Self::MIN_FOV, Self::MAX_FOV);
        self.mouse_sensitivity = self
            .mouse_sensitivity
            .clamp(Self::MIN_SENSITIVITY, Self::MAX_SENSITIVITY);
        self.master_volume = self.master_volume.clamp(0.0, 1.0);
        self.sfx_volume = self.sfx_volume.clamp(0.0, 1.0);
        self.music_volume = self.music_volume.clamp(0.0, 1.0);
    }

    /// Loads settings from a `key=value` config file, falling back to defaults for missing keys.
    pub fn load(path: &Path) -> Self {
        let mut settings = Self::default();
        let Ok(contents) = std::fs::read_to_string(path) else {
            return settings;
        };

        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let val = val.trim();
            match key {
                "render_distance" => {
                    if let Ok(v) = val.parse::<i32>() {
                        settings.render_distance = v;
                    }
                }
                "fov_degrees" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.fov_degrees = v;
                        }
                    }
                }
                "mouse_sensitivity" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.mouse_sensitivity = v;
                        }
                    }
                }
                "master_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.master_volume = v;
                        }
                    }
                }
                "sfx_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.sfx_volume = v;
                        }
                    }
                }
                "music_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.music_volume = v;
                        }
                    }
                }
                "invert_y" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.invert_y = v;
                    }
                }
                "fullscreen" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.fullscreen = v;
                    }
                }
                "vsync" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.vsync = v;
                    }
                }
                "view_bobbing" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.view_bobbing = v;
                    }
                }
                "show_debug_hud" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.show_debug_hud = v;
                    }
                }
                "key_forward" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.forward = k;
                    }
                }
                "key_backward" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.backward = k;
                    }
                }
                "key_left" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.left = k;
                    }
                }
                "key_right" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.right = k;
                    }
                }
                "key_jump" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.jump = k;
                    }
                }
                "key_sprint" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.sprint = k;
                    }
                }
                "key_inventory" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.inventory = k;
                    }
                }
                "key_flight" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.flight = k;
                    }
                }
                "key_chat" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.chat = k;
                    }
                }
                "key_player_list" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.player_list = k;
                    }
                }
                "key_third_person" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.third_person = k;
                    }
                }
                "key_debug_hud" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.debug_hud = k;
                    }
                }
                _ => {}
            }
        }

        settings.clamp_all();
        settings
    }

    /// Saves settings to a `key=value` config file.
    pub fn save(&self, path: &Path) {
        let content = format!(
            "# BlockGame Client Settings\n\
             render_distance={}\n\
             fov_degrees={:.1}\n\
             mouse_sensitivity={:.2}\n\
             master_volume={:.2}\n\
             sfx_volume={:.2}\n\
             music_volume={:.2}\n\
             invert_y={}\n\
             fullscreen={}\n\
             vsync={}\n\
             view_bobbing={}\n\
             show_debug_hud={}\n\
             key_forward={}\n\
             key_backward={}\n\
             key_left={}\n\
             key_right={}\n\
             key_jump={}\n\
             key_sprint={}\n\
             key_inventory={}\n\
             key_flight={}\n\
             key_chat={}\n\
             key_player_list={}\n\
             key_third_person={}\n\
             key_debug_hud={}\n",
            self.render_distance,
            self.fov_degrees,
            self.mouse_sensitivity,
            self.master_volume,
            self.sfx_volume,
            self.music_volume,
            self.invert_y,
            self.fullscreen,
            self.vsync,
            self.view_bobbing,
            self.show_debug_hud,
            key_code_to_str(self.keybinds.forward).unwrap_or("KeyW"),
            key_code_to_str(self.keybinds.backward).unwrap_or("KeyS"),
            key_code_to_str(self.keybinds.left).unwrap_or("KeyA"),
            key_code_to_str(self.keybinds.right).unwrap_or("KeyD"),
            key_code_to_str(self.keybinds.jump).unwrap_or("Space"),
            key_code_to_str(self.keybinds.sprint).unwrap_or("ControlLeft"),
            key_code_to_str(self.keybinds.inventory).unwrap_or("KeyE"),
            key_code_to_str(self.keybinds.flight).unwrap_or("KeyF"),
            key_code_to_str(self.keybinds.chat).unwrap_or("KeyT"),
            key_code_to_str(self.keybinds.player_list).unwrap_or("Tab"),
            key_code_to_str(self.keybinds.third_person).unwrap_or("F5"),
            key_code_to_str(self.keybinds.debug_hud).unwrap_or("F3"),
        );
        if let Err(e) = std::fs::write(path, content) {
            log::warn!("Failed to save settings to {:?}: {}", path, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_clamp() {
        let mut keybinds = KeyBinds::default();
        keybinds.forward = KeyCode::ArrowUp;
        keybinds.jump = KeyCode::ShiftLeft;

        let mut s = GameSettings {
            render_distance: 99,
            fov_degrees: 150.0,
            mouse_sensitivity: 0.01,
            master_volume: 1.5,
            sfx_volume: -0.2,
            music_volume: 0.45,
            invert_y: true,
            fullscreen: true,
            vsync: false,
            view_bobbing: false,
            show_debug_hud: true,
            keybinds,
        };
        s.clamp_all();
        assert_eq!(s.render_distance, GameSettings::MAX_RENDER_DISTANCE);
        assert_eq!(s.fov_degrees, GameSettings::MAX_FOV);
        assert_eq!(s.mouse_sensitivity, GameSettings::MIN_SENSITIVITY);
        assert_eq!(s.master_volume, 1.0);
        assert_eq!(s.sfx_volume, 0.0);

        let temp_path = std::env::temp_dir().join("blockgame_test_settings.cfg");
        s.save(&temp_path);
        let loaded = GameSettings::load(&temp_path);
        let _ = std::fs::remove_file(&temp_path);

        assert_eq!(loaded.render_distance, s.render_distance);
        assert!((loaded.fov_degrees - s.fov_degrees).abs() < 0.11);
        assert!((loaded.mouse_sensitivity - s.mouse_sensitivity).abs() < 0.02);
        assert!((loaded.master_volume - s.master_volume).abs() < 0.02);
        assert!((loaded.sfx_volume - s.sfx_volume).abs() < 0.02);
        assert!((loaded.music_volume - s.music_volume).abs() < 0.02);
        assert_eq!(loaded.invert_y, s.invert_y);
        assert_eq!(loaded.fullscreen, s.fullscreen);
        assert_eq!(loaded.vsync, s.vsync);
        assert_eq!(loaded.view_bobbing, s.view_bobbing);
        assert_eq!(loaded.show_debug_hud, s.show_debug_hud);
        assert_eq!(loaded.keybinds.forward, KeyCode::ArrowUp);
        assert_eq!(loaded.keybinds.jump, KeyCode::ShiftLeft);
    }
}
