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
    pub push_to_talk: KeyCode,
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
            push_to_talk: KeyCode::KeyV,
        }
    }
}

impl KeyBinds {
    pub const COUNT: usize = 13;

    pub fn labels_and_keys(&self) -> [(&'static str, String); Self::COUNT] {
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
            ("Push to Talk", key_display_name(self.push_to_talk)),
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
            12 => self.push_to_talk = key,
            _ => {}
        }
    }
}

macro_rules! define_key_table {
    ($(($variant:ident, $display:expr)),* $(,)?) => {
        pub fn key_code_to_string(key: KeyCode) -> String {
            match key {
                $(KeyCode::$variant => stringify!($variant).to_string(),)*
                other => format!("{:?}", other),
            }
        }

        pub fn str_to_key_code(s: &str) -> Option<KeyCode> {
            match s.trim() {
                $(stringify!($variant) => Some(KeyCode::$variant),)*
                _ => None,
            }
        }

        pub fn key_display_name(key: KeyCode) -> String {
            match key {
                $(KeyCode::$variant => $display.to_string(),)*
                other => format!("{:?}", other),
            }
        }
    };
}

define_key_table!(
    // Letters
    (KeyA, "A"), (KeyB, "B"), (KeyC, "C"), (KeyD, "D"), (KeyE, "E"),
    (KeyF, "F"), (KeyG, "G"), (KeyH, "H"), (KeyI, "I"), (KeyJ, "J"),
    (KeyK, "K"), (KeyL, "L"), (KeyM, "M"), (KeyN, "N"), (KeyO, "O"),
    (KeyP, "P"), (KeyQ, "Q"), (KeyR, "R"), (KeyS, "S"), (KeyT, "T"),
    (KeyU, "U"), (KeyV, "V"), (KeyW, "W"), (KeyX, "X"), (KeyY, "Y"),
    (KeyZ, "Z"),
    // Top-row digits
    (Digit0, "0"), (Digit1, "1"), (Digit2, "2"), (Digit3, "3"), (Digit4, "4"),
    (Digit5, "5"), (Digit6, "6"), (Digit7, "7"), (Digit8, "8"), (Digit9, "9"),
    // Numpad
    (Numpad0, "Num 0"), (Numpad1, "Num 1"), (Numpad2, "Num 2"), (Numpad3, "Num 3"),
    (Numpad4, "Num 4"), (Numpad5, "Num 5"), (Numpad6, "Num 6"), (Numpad7, "Num 7"),
    (Numpad8, "Num 8"), (Numpad9, "Num 9"),
    (NumpadAdd, "Num +"), (NumpadSubtract, "Num -"),
    (NumpadMultiply, "Num *"), (NumpadDivide, "Num /"),
    (NumpadDecimal, "Num ."), (NumpadEnter, "Num Enter"),
    (NumpadEqual, "Num ="), (NumpadComma, "Num ,"),
    (NumLock, "NumLock"),
    // Punctuation & Symbols
    (Period, "."), (Comma, ","), (Slash, "/"), (Backslash, "\\"),
    (IntlBackslash, "\\"), (Semicolon, ";"), (Quote, "'"),
    (BracketLeft, "["), (BracketRight, "]"),
    (Minus, "-"), (Equal, "="), (Backquote, "`"),
    // Whitespace, Modifiers & System
    (Space, "Space"), (Tab, "Tab"), (Enter, "Enter"), (Backspace, "Backspace"),
    (CapsLock, "CapsLock"),
    (ShiftLeft, "L-Shift"), (ShiftRight, "R-Shift"),
    (ControlLeft, "L-Ctrl"), (ControlRight, "R-Ctrl"),
    (AltLeft, "L-Alt"), (AltRight, "R-Alt"),
    (SuperLeft, "L-Super"), (SuperRight, "R-Super"),
    (ContextMenu, "Menu"),
    // Navigation & Editing
    (Insert, "Insert"), (Delete, "Delete"),
    (Home, "Home"), (End, "End"),
    (PageUp, "PgUp"), (PageDown, "PgDn"),
    (ArrowUp, "Up"), (ArrowDown, "Down"),
    (ArrowLeft, "Left"), (ArrowRight, "Right"),
    (PrintScreen, "PrtSc"), (ScrollLock, "ScrLk"), (Pause, "Pause"),
    // Function keys
    (F1, "F1"), (F2, "F2"), (F3, "F3"), (F4, "F4"),
    (F5, "F5"), (F6, "F6"), (F7, "F7"), (F8, "F8"),
    (F9, "F9"), (F10, "F10"), (F11, "F11"), (F12, "F12"),
    (F13, "F13"), (F14, "F14"), (F15, "F15"), (F16, "F16"),
    (F17, "F17"), (F18, "F18"), (F19, "F19"), (F20, "F20"),
    (F21, "F21"), (F22, "F22"), (F23, "F23"), (F24, "F24"),
);

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
    /// UI scale factor (`0.75`, `1.0`, `1.25`, `1.5`).
    pub ui_scale: f32,
    /// Proximity voice chat mode (`0` = Off, `1` = Push-to-Talk, `2` = Voice Activity).
    pub voice_mode: u8,
    /// Proximity voice chat playback volume (`0.0..=1.0`).
    pub voice_volume: f32,
    /// Selected microphone device name (empty = system default).
    pub voice_input_device: String,
    /// Central server directory URL for discoverable multiplayer servers.
    pub directory_url: String,
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
            ui_scale: 1.0,
            voice_mode: 1,
            voice_volume: 1.0,
            voice_input_device: String::new(),
            directory_url: blockgame_core::directory::DEFAULT_DIRECTORY_URL.to_string(),
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
    pub const UI_SCALE_PRESETS: [f32; 4] = [0.75, 1.0, 1.25, 1.5];

    /// Cycles `voice_mode` through `Off (0) -> Push-to-Talk (1) -> Voice Activity (2) -> Off (0)`.
    pub fn cycle_voice_mode(&mut self) {
        self.voice_mode = (self.voice_mode + 1) % 3;
    }

    /// Cycles `ui_scale` through `0.75x -> 1.0x -> 1.25x -> 1.5x -> 0.75x`.
    pub fn cycle_ui_scale(&mut self) {
        let current_idx = Self::UI_SCALE_PRESETS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (*a - self.ui_scale)
                    .abs()
                    .partial_cmp(&(*b - self.ui_scale).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(1);
        self.ui_scale = Self::UI_SCALE_PRESETS[(current_idx + 1) % Self::UI_SCALE_PRESETS.len()];
    }

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
        self.voice_volume = self.voice_volume.clamp(0.0, 1.0);
        if self.voice_mode > 2 {
            self.voice_mode = 1;
        }
        if !self.ui_scale.is_finite() {
            self.ui_scale = 1.0;
        } else {
            self.ui_scale = Self::UI_SCALE_PRESETS
                .iter()
                .copied()
                .min_by(|a, b| {
                    (*a - self.ui_scale)
                        .abs()
                        .partial_cmp(&(*b - self.ui_scale).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(1.0);
        }
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
                "ui_scale" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.ui_scale = v;
                        }
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
                "key_push_to_talk" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.push_to_talk = k;
                    }
                }
                "voice_mode" => {
                    if let Ok(v) = val.parse::<u8>() {
                        settings.voice_mode = v;
                    }
                }
                "voice_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.voice_volume = v;
                        }
                    }
                }
                "voice_input_device" => {
                    settings.voice_input_device = val.trim().to_string();
                }
                "directory_url" => {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        settings.directory_url = trimmed.to_string();
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
        let safe_mic = self.voice_input_device.replace(['\r', '\n'], " ");
        let safe_dir = self.directory_url.replace(['\r', '\n'], "");
        let content = format!(
            "# BlockGame Client Settings\n\
             render_distance={}\n\
             fov_degrees={:.1}\n\
             mouse_sensitivity={:.2}\n\
             master_volume={:.2}\n\
             sfx_volume={:.2}\n\
             music_volume={:.2}\n\
             voice_volume={:.2}\n\
             voice_mode={}\n\
             voice_input_device={}\n\
             directory_url={}\n\
             invert_y={}\n\
             fullscreen={}\n\
             vsync={}\n\
             view_bobbing={}\n\
             show_debug_hud={}\n\
             ui_scale={:.2}\n\
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
             key_debug_hud={}\n\
             key_push_to_talk={}\n",
            self.render_distance,
            self.fov_degrees,
            self.mouse_sensitivity,
            self.master_volume,
            self.sfx_volume,
            self.music_volume,
            self.voice_volume,
            self.voice_mode,
            safe_mic,
            safe_dir,
            self.invert_y,
            self.fullscreen,
            self.vsync,
            self.view_bobbing,
            self.show_debug_hud,
            self.ui_scale,
            key_code_to_string(self.keybinds.forward),
            key_code_to_string(self.keybinds.backward),
            key_code_to_string(self.keybinds.left),
            key_code_to_string(self.keybinds.right),
            key_code_to_string(self.keybinds.jump),
            key_code_to_string(self.keybinds.sprint),
            key_code_to_string(self.keybinds.inventory),
            key_code_to_string(self.keybinds.flight),
            key_code_to_string(self.keybinds.chat),
            key_code_to_string(self.keybinds.player_list),
            key_code_to_string(self.keybinds.third_person),
            key_code_to_string(self.keybinds.debug_hud),
            key_code_to_string(self.keybinds.push_to_talk),
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
        keybinds.forward = KeyCode::Numpad8;
        keybinds.jump = KeyCode::Period;
        keybinds.inventory = KeyCode::NumpadDecimal;

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
            ui_scale: 1.25,
            voice_mode: 2,
            voice_volume: 0.9,
            voice_input_device: "Studio USB Mic".to_string(),
            directory_url: "http://15.204.81.97:25569".to_string(),
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
        assert!((loaded.ui_scale - 1.25).abs() < 0.01);
        assert_eq!(loaded.keybinds.forward, KeyCode::Numpad8);
        assert_eq!(loaded.keybinds.jump, KeyCode::Period);
        assert_eq!(loaded.keybinds.inventory, KeyCode::NumpadDecimal);
    }
}
