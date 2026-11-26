use std::path::Path;
use blockgame_render::{GamepadButton, GlyphStyle};
use winit::keyboard::KeyCode;

use crate::controller::{ControllerBindings, ControllerConfig};

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
    pub skills: KeyCode,
    pub achievements: KeyCode,
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
            skills: KeyCode::KeyK,
            achievements: KeyCode::KeyJ,
        }
    }
}

impl KeyBinds {
    pub const COUNT: usize = 15;

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
            ("Skills Menu", key_display_name(self.skills)),
            ("Achievements", key_display_name(self.achievements)),
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
            13 => self.skills = key,
            14 => self.achievements = key,
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
    /// Creature / mob sounds volume (`0.0..=1.0`).
    pub creature_volume: f32,
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
    /// Controller right-stick look sensitivity multiplier (`0.2..=2.5`, where `1.0` = default).
    pub controller_sensitivity: f32,
    /// Invert vertical controller look axis.
    pub controller_invert_y: bool,
    /// Radial stick deadzone (`0.05..=0.45`, default `0.15`).
    pub controller_deadzone: f32,
    /// Controller rumble intensity (`0.0..=1.0`, where `0.0` = Off).
    pub controller_rumble_intensity: f32,
    /// Preferred button glyph icon style (`Auto` or explicit Controller Family override).
    pub controller_glyph_style: GlyphStyle,
    /// Swap menu Confirm and Cancel buttons relative to the active Controller Family default.
    pub controller_swap_confirm_cancel: bool,
    /// Whether controller Sneak uses Toggle (`true`) or Hold (`false`) mode.
    pub controller_sneak_toggle: bool,
    /// Customizable positional controller gameplay bindings.
    pub controller_binds: ControllerBindings,
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
            creature_volume: 0.85,
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
            controller_sensitivity: 1.0,
            controller_invert_y: false,
            controller_deadzone: 0.15,
            controller_rumble_intensity: 0.8,
            controller_glyph_style: GlyphStyle::Auto,
            controller_swap_confirm_cancel: false,
            controller_sneak_toggle: true,
            controller_binds: ControllerBindings::default(),
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
    pub const MIN_DEADZONE: f32 = 0.05;
    pub const MAX_DEADZONE: f32 = 0.45;
    pub const UI_SCALE_PRESETS: [f32; 4] = [0.75, 1.0, 1.25, 1.5];

    /// Builds a `ControllerConfig` snapshot for `InputRouter::update`.
    pub fn controller_config(&self) -> ControllerConfig {
        ControllerConfig {
            sensitivity: self.controller_sensitivity,
            invert_y: self.controller_invert_y,
            deadzone: self.controller_deadzone,
            rumble_intensity: self.controller_rumble_intensity,
            glyph_style: self.controller_glyph_style,
            swap_confirm_cancel: self.controller_swap_confirm_cancel,
            sneak_toggle: self.controller_sneak_toggle,
            bindings: self.controller_binds.clone(),
        }
    }

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
        self.creature_volume = self.creature_volume.clamp(0.0, 1.0);
        self.voice_volume = self.voice_volume.clamp(0.0, 1.0);
        self.controller_sensitivity = self
            .controller_sensitivity
            .clamp(Self::MIN_SENSITIVITY, Self::MAX_SENSITIVITY);
        self.controller_deadzone = self
            .controller_deadzone
            .clamp(Self::MIN_DEADZONE, Self::MAX_DEADZONE);
        self.controller_rumble_intensity = self.controller_rumble_intensity.clamp(0.0, 1.0);
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
                "creature_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.creature_volume = v;
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
                "key_skills" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.skills = k;
                    }
                }
                "key_achievements" => {
                    if let Some(k) = str_to_key_code(val) {
                        settings.keybinds.achievements = k;
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
                "controller_sensitivity" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.controller_sensitivity = v;
                        }
                    }
                }
                "controller_invert_y" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.controller_invert_y = v;
                    }
                }
                "controller_deadzone" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.controller_deadzone = v;
                        }
                    }
                }
                "controller_rumble_intensity" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.controller_rumble_intensity = v;
                        }
                    }
                }
                "controller_glyph_style" => {
                    if let Some(style) = GlyphStyle::from_config_str(val) {
                        settings.controller_glyph_style = style;
                    }
                }
                "controller_swap_confirm_cancel" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.controller_swap_confirm_cancel = v;
                    }
                }
                "controller_sneak_toggle" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.controller_sneak_toggle = v;
                    }
                }
                "pad_mine" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.mine = b; },
                "pad_use_item" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.use_item = b; },
                "pad_hotbar_next" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.hotbar_next = b; },
                "pad_hotbar_prev" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.hotbar_prev = b; },
                "pad_jump" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.jump = b; },
                "pad_sneak" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.sneak = b; },
                "pad_sprint" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.sprint = b; },
                "pad_inventory" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.inventory = b; },
                "pad_backpack" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.backpack = b; },
                "pad_third_person" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.third_person = b; },
                "pad_pause" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.pause = b; },
                "pad_player_list" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.player_list = b; },
                "pad_quick_menu" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.quick_menu = b; },
                "pad_flight" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.flight = b; },
                "pad_chat" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.chat = b; },
                "pad_push_to_talk" => if let Some(b) = GamepadButton::from_config_str(val) { settings.controller_binds.push_to_talk = b; },
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
        let cb = &self.controller_binds;
        let content = format!(
            "# BlockGame Client Settings\n\
             render_distance={}\n\
             fov_degrees={:.1}\n\
             mouse_sensitivity={:.2}\n\
             master_volume={:.2}\n\
             sfx_volume={:.2}\n\
             music_volume={:.2}\n\
             creature_volume={:.2}\n\
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
             key_push_to_talk={}\n\
             key_skills={}\n\
             key_achievements={}\n\
             controller_sensitivity={:.2}\n\
             controller_invert_y={}\n\
             controller_deadzone={:.2}\n\
             controller_rumble_intensity={:.2}\n\
             controller_glyph_style={}\n\
             controller_swap_confirm_cancel={}\n\
             controller_sneak_toggle={}\n\
             pad_mine={}\n\
             pad_use_item={}\n\
             pad_hotbar_next={}\n\
             pad_hotbar_prev={}\n\
             pad_jump={}\n\
             pad_sneak={}\n\
             pad_sprint={}\n\
             pad_inventory={}\n\
             pad_backpack={}\n\
             pad_third_person={}\n\
             pad_pause={}\n\
             pad_player_list={}\n\
             pad_quick_menu={}\n\
             pad_flight={}\n\
             pad_chat={}\n\
             pad_push_to_talk={}\n",
            self.render_distance,
            self.fov_degrees,
            self.mouse_sensitivity,
            self.master_volume,
            self.sfx_volume,
            self.music_volume,
            self.creature_volume,
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
            key_code_to_string(self.keybinds.skills),
            key_code_to_string(self.keybinds.achievements),
            self.controller_sensitivity,
            self.controller_invert_y,
            self.controller_deadzone,
            self.controller_rumble_intensity,
            self.controller_glyph_style.as_config_str(),
            self.controller_swap_confirm_cancel,
            self.controller_sneak_toggle,
            cb.mine.as_config_str(),
            cb.use_item.as_config_str(),
            cb.hotbar_next.as_config_str(),
            cb.hotbar_prev.as_config_str(),
            cb.jump.as_config_str(),
            cb.sneak.as_config_str(),
            cb.sprint.as_config_str(),
            cb.inventory.as_config_str(),
            cb.backpack.as_config_str(),
            cb.third_person.as_config_str(),
            cb.pause.as_config_str(),
            cb.player_list.as_config_str(),
            cb.quick_menu.as_config_str(),
            cb.flight.as_config_str(),
            cb.chat.as_config_str(),
            cb.push_to_talk.as_config_str(),
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
        keybinds.skills = KeyCode::KeyM;
        keybinds.achievements = KeyCode::KeyL;

        let mut controller_binds = ControllerBindings::default();
        controller_binds.jump = GamepadButton::RightBumper;
        controller_binds.sneak = GamepadButton::RightStick;

        let mut s = GameSettings {
            render_distance: 99,
            fov_degrees: 150.0,
            mouse_sensitivity: 0.01,
            master_volume: 1.5,
            sfx_volume: -0.2,
            music_volume: 0.45,
            creature_volume: 0.85,
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
            controller_sensitivity: 5.0,
            controller_invert_y: true,
            controller_deadzone: 0.01,
            controller_rumble_intensity: 0.65,
            controller_glyph_style: GlyphStyle::Nintendo,
            controller_swap_confirm_cancel: true,
            controller_sneak_toggle: false,
            controller_binds,
        };
        s.clamp_all();
        assert_eq!(s.render_distance, GameSettings::MAX_RENDER_DISTANCE);
        assert_eq!(s.fov_degrees, GameSettings::MAX_FOV);
        assert_eq!(s.mouse_sensitivity, GameSettings::MIN_SENSITIVITY);
        assert_eq!(s.master_volume, 1.0);
        assert_eq!(s.sfx_volume, 0.0);
        assert_eq!(s.creature_volume, 0.85);
        assert_eq!(s.controller_sensitivity, GameSettings::MAX_SENSITIVITY);
        assert_eq!(s.controller_deadzone, GameSettings::MIN_DEADZONE);

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
        assert!((loaded.creature_volume - s.creature_volume).abs() < 0.02);
        assert_eq!(loaded.invert_y, s.invert_y);
        assert_eq!(loaded.fullscreen, s.fullscreen);
        assert_eq!(loaded.vsync, s.vsync);
        assert_eq!(loaded.view_bobbing, s.view_bobbing);
        assert_eq!(loaded.show_debug_hud, s.show_debug_hud);
        assert!((loaded.ui_scale - 1.25).abs() < 0.01);
        assert_eq!(loaded.keybinds.forward, KeyCode::Numpad8);
        assert_eq!(loaded.keybinds.jump, KeyCode::Period);
        assert_eq!(loaded.keybinds.inventory, KeyCode::NumpadDecimal);
        assert_eq!(loaded.keybinds.skills, KeyCode::KeyM);
        assert_eq!(loaded.keybinds.achievements, KeyCode::KeyL);
        assert!((loaded.controller_sensitivity - GameSettings::MAX_SENSITIVITY).abs() < 0.02);
        assert!(loaded.controller_invert_y);
        assert!((loaded.controller_deadzone - GameSettings::MIN_DEADZONE).abs() < 0.02);
        assert!((loaded.controller_rumble_intensity - 0.65).abs() < 0.02);
        assert_eq!(loaded.controller_glyph_style, GlyphStyle::Nintendo);
        assert!(loaded.controller_swap_confirm_cancel);
        assert!(!loaded.controller_sneak_toggle);
        assert_eq!(loaded.controller_binds.jump, GamepadButton::RightBumper);
        assert_eq!(loaded.controller_binds.sneak, GamepadButton::RightStick);
    }

    #[test]
    fn legacy_settings_file_uses_controller_defaults() {
        let temp_path = std::env::temp_dir().join("blockgame_test_legacy_settings.cfg");
        std::fs::write(&temp_path, "render_distance=6\nmouse_sensitivity=1.25\n").unwrap();
        let loaded = GameSettings::load(&temp_path);
        let _ = std::fs::remove_file(&temp_path);

        assert_eq!(loaded.render_distance, 6);
        assert_eq!(loaded.controller_glyph_style, GlyphStyle::Auto);
        assert_eq!(loaded.controller_binds, ControllerBindings::default());
        assert!(loaded.controller_sneak_toggle);
    }
}
