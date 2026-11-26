//! Pluggable gamepad backend trait (`GamepadBackend`), pure-Rust `gilrs` adapter
//! (`GilrsBackend`), Controller Family detection, remappable `ControllerBindings`,
//! and the windowless `InputRouter` state machine (ADR 0008).

use std::collections::{HashMap, HashSet};

use blockgame_render::{ControllerFamily, GamepadButton, GlyphStyle, InputMode};

/// Opaque identifier for a connected gamepad within a `GamepadBackend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GamepadId(pub usize);

/// Metadata for a connected gamepad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GamepadInfo {
    pub id: GamepadId,
    pub name: String,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub family: ControllerFamily,
}

/// Analog axes reported by a `GamepadBackend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
}

/// Normalized gamepad events emitted by any `GamepadBackend`.
#[derive(Debug, Clone, PartialEq)]
pub enum GamepadEvent {
    Connected(GamepadInfo),
    Disconnected(GamepadId),
    ButtonPressed {
        id: GamepadId,
        button: GamepadButton,
    },
    ButtonReleased {
        id: GamepadId,
        button: GamepadButton,
    },
    AxisChanged {
        id: GamepadId,
        axis: GamepadAxis,
        value: f32,
    },
}

/// High-level gameplay feedback events that trigger controller rumble.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RumbleEffect {
    Damage,
    FishingBite,
    BowRelease,
    BlockBreak,
    Explosion,
}

/// Low-level dual-motor rumble command dispatched to a `GamepadBackend`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RumbleRequest {
    pub strong: f32,
    pub weak: f32,
    pub duration_ms: u32,
}

impl RumbleEffect {
    pub fn to_request(self, intensity: f32) -> Option<RumbleRequest> {
        let scale = intensity.clamp(0.0, 1.0);
        if scale <= 0.001 {
            return None;
        }
        let (strong, weak, duration_ms) = match self {
            RumbleEffect::Damage => (0.85, 0.55, 220),
            RumbleEffect::FishingBite => (0.35, 0.70, 160),
            RumbleEffect::BowRelease => (0.30, 0.45, 90),
            RumbleEffect::BlockBreak => (0.12, 0.28, 45),
            RumbleEffect::Explosion => (1.0, 0.85, 360),
        };
        Some(RumbleRequest {
            strong: (strong * scale).clamp(0.0, 1.0),
            weak: (weak * scale).clamp(0.0, 1.0),
            duration_ms,
        })
    }
}

/// Platform-agnostic controller backend boundary (ADR 0008).
/// Implemented by `GilrsBackend` today and designed for future `steamworks-rs` Steam Input.
pub trait GamepadBackend {
    fn poll_events(&mut self) -> Vec<GamepadEvent>;
    fn connected_gamepads(&self) -> Vec<GamepadInfo>;
    fn set_rumble(&mut self, id: GamepadId, request: RumbleRequest);
}

/// Infers a `ControllerFamily` from USB/Bluetooth vendor/product IDs and device name.
pub fn detect_controller_family(
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    name: &str,
) -> ControllerFamily {
    let lower = name.to_ascii_lowercase();

    // Valve (0x28de): distinguish Steam Deck (0x1205) from Steam Controller (0x1102 / 0x1142)
    if vendor_id == Some(0x28de) {
        if product_id == Some(0x1205) || lower.contains("steam deck") || lower.contains("neptune") {
            return ControllerFamily::SteamDeck;
        }
        return ControllerFamily::SteamController;
    }
    if lower.contains("steam deck") {
        return ControllerFamily::SteamDeck;
    }
    if lower.contains("steam controller") || lower.contains("valve") {
        return ControllerFamily::SteamController;
    }

    // Sony Interactive Entertainment (0x054c)
    if vendor_id == Some(0x054c)
        || lower.contains("dualsense")
        || lower.contains("dualshock")
        || lower.contains("playstation")
        || lower.contains("ps5")
        || lower.contains("ps4")
        || lower.contains("ps3")
    {
        return ControllerFamily::PlayStation;
    }

    // Nintendo (0x057e)
    if vendor_id == Some(0x057e)
        || lower.contains("nintendo")
        || lower.contains("switch")
        || lower.contains("pro controller")
        || lower.contains("joy-con")
    {
        return ControllerFamily::Nintendo;
    }

    // Microsoft Xbox (0x045e) and default fallback
    ControllerFamily::Xbox
}

/// Pure-Rust `gilrs` implementation of `GamepadBackend`.
/// Degrades gracefully to a no-op backend if `gilrs` initialization fails (e.g., headless CI).
pub struct GilrsBackend {
    gilrs: Option<gilrs::Gilrs>,
    active_rumble: Vec<gilrs::ff::Effect>,
}

impl GilrsBackend {
    pub fn new() -> Self {
        match gilrs::Gilrs::new() {
            Ok(gilrs) => Self {
                gilrs: Some(gilrs),
                active_rumble: Vec::new(),
            },
            Err(err) => {
                log::warn!("Gamepad backend unavailable (gilrs init failed): {err}");
                Self {
                    gilrs: None,
                    active_rumble: Vec::new(),
                }
            }
        }
    }

    fn map_button(btn: gilrs::Button) -> Option<GamepadButton> {
        match btn {
            gilrs::Button::South => Some(GamepadButton::South),
            gilrs::Button::East => Some(GamepadButton::East),
            gilrs::Button::West => Some(GamepadButton::West),
            gilrs::Button::North => Some(GamepadButton::North),
            gilrs::Button::LeftTrigger => Some(GamepadButton::LeftBumper),
            gilrs::Button::RightTrigger => Some(GamepadButton::RightBumper),
            gilrs::Button::LeftTrigger2 => Some(GamepadButton::LeftTrigger),
            gilrs::Button::RightTrigger2 => Some(GamepadButton::RightTrigger),
            gilrs::Button::LeftThumb => Some(GamepadButton::LeftStick),
            gilrs::Button::RightThumb => Some(GamepadButton::RightStick),
            gilrs::Button::Start => Some(GamepadButton::Start),
            gilrs::Button::Select => Some(GamepadButton::Select),
            gilrs::Button::DPadUp => Some(GamepadButton::DPadUp),
            gilrs::Button::DPadDown => Some(GamepadButton::DPadDown),
            gilrs::Button::DPadLeft => Some(GamepadButton::DPadLeft),
            gilrs::Button::DPadRight => Some(GamepadButton::DPadRight),
            _ => None,
        }
    }

    fn map_axis(axis: gilrs::Axis) -> Option<GamepadAxis> {
        match axis {
            gilrs::Axis::LeftStickX => Some(GamepadAxis::LeftStickX),
            gilrs::Axis::LeftStickY => Some(GamepadAxis::LeftStickY),
            gilrs::Axis::RightStickX => Some(GamepadAxis::RightStickX),
            gilrs::Axis::RightStickY => Some(GamepadAxis::RightStickY),
            gilrs::Axis::LeftZ => Some(GamepadAxis::LeftTrigger),
            gilrs::Axis::RightZ => Some(GamepadAxis::RightTrigger),
            _ => None,
        }
    }

    fn info_from_gamepad(id: gilrs::GamepadId, gp: gilrs::Gamepad<'_>) -> GamepadInfo {
        let vendor_id = gp.vendor_id();
        let product_id = gp.product_id();
        let name = gp.name().to_string();
        let family = detect_controller_family(vendor_id, product_id, &name);
        GamepadInfo {
            id: GamepadId(usize::from(id)),
            name,
            vendor_id,
            product_id,
            family,
        }
    }
}

impl Default for GilrsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl GamepadBackend for GilrsBackend {
    fn poll_events(&mut self) -> Vec<GamepadEvent> {
        let Some(gilrs) = self.gilrs.as_mut() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Some(ev) = gilrs.next_event() {
            let gid = GamepadId(usize::from(ev.id));
            match ev.event {
                gilrs::EventType::Connected => {
                    let info = Self::info_from_gamepad(ev.id, gilrs.gamepad(ev.id));
                    out.push(GamepadEvent::Connected(info));
                }
                gilrs::EventType::Disconnected => {
                    out.push(GamepadEvent::Disconnected(gid));
                }
                gilrs::EventType::ButtonPressed(btn, _) => {
                    if let Some(button) = Self::map_button(btn) {
                        out.push(GamepadEvent::ButtonPressed { id: gid, button });
                    }
                }
                gilrs::EventType::ButtonReleased(btn, _) => {
                    if let Some(button) = Self::map_button(btn) {
                        out.push(GamepadEvent::ButtonReleased { id: gid, button });
                    }
                }
                gilrs::EventType::ButtonChanged(btn, value, _) => {
                    // Some controllers report analog triggers via ButtonChanged(LeftTrigger2/RightTrigger2)
                    match btn {
                        gilrs::Button::LeftTrigger2 => out.push(GamepadEvent::AxisChanged {
                            id: gid,
                            axis: GamepadAxis::LeftTrigger,
                            value,
                        }),
                        gilrs::Button::RightTrigger2 => out.push(GamepadEvent::AxisChanged {
                            id: gid,
                            axis: GamepadAxis::RightTrigger,
                            value,
                        }),
                        _ => {}
                    }
                }
                gilrs::EventType::AxisChanged(axis, value, _) => {
                    if let Some(mapped) = Self::map_axis(axis) {
                        out.push(GamepadEvent::AxisChanged {
                            id: gid,
                            axis: mapped,
                            value,
                        });
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn connected_gamepads(&self) -> Vec<GamepadInfo> {
        let Some(gilrs) = self.gilrs.as_ref() else {
            return Vec::new();
        };
        gilrs
            .gamepads()
            .filter(|(_, gp)| gp.is_connected())
            .map(|(id, gp)| Self::info_from_gamepad(id, gp))
            .collect()
    }

    fn set_rumble(&mut self, id: GamepadId, request: RumbleRequest) {
        let Some(gilrs) = self.gilrs.as_mut() else {
            return;
        };
        let target_gid = gilrs
            .gamepads()
            .find(|(gid, gp)| usize::from(*gid) == id.0 && gp.is_connected() && gp.is_ff_supported())
            .map(|(gid, _)| gid);
        let Some(gid) = target_gid else {
            return;
        };

        use gilrs::ff::{BaseEffect, BaseEffectType, EffectBuilder, Repeat, Replay, Ticks};
        let strong_mag = (request.strong.clamp(0.0, 1.0) * u16::MAX as f32) as u16;
        let weak_mag = (request.weak.clamp(0.0, 1.0) * u16::MAX as f32) as u16;
        let duration = Ticks::from_ms(request.duration_ms.max(10));

        if let Ok(effect) = EffectBuilder::new()
            .add_effect(BaseEffect {
                kind: BaseEffectType::Strong {
                    magnitude: strong_mag,
                },
                scheduling: Replay {
                    play_for: duration,
                    ..Default::default()
                },
                ..Default::default()
            })
            .add_effect(BaseEffect {
                kind: BaseEffectType::Weak {
                    magnitude: weak_mag,
                },
                scheduling: Replay {
                    play_for: duration,
                    ..Default::default()
                },
                ..Default::default()
            })
            .repeat(Repeat::For(duration))
            .gamepads(&[gid])
            .finish(gilrs)
        {
            let _ = effect.play();
            // Keep only the most recent active effect handles so old ones drop cleanly
            if self.active_rumble.len() >= 4 {
                self.active_rumble.remove(0);
            }
            self.active_rumble.push(effect);
        }
    }
}

/// Remappable controller button bindings for gameplay actions (positional).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerBindings {
    pub mine: GamepadButton,
    pub use_item: GamepadButton,
    pub hotbar_next: GamepadButton,
    pub hotbar_prev: GamepadButton,
    pub jump: GamepadButton,
    pub sneak: GamepadButton,
    pub sprint: GamepadButton,
    pub inventory: GamepadButton,
    pub backpack: GamepadButton,
    pub third_person: GamepadButton,
    pub pause: GamepadButton,
    pub player_list: GamepadButton,
    pub quick_menu: GamepadButton,
    pub flight: GamepadButton,
    pub chat: GamepadButton,
    pub push_to_talk: GamepadButton,
}

impl Default for ControllerBindings {
    fn default() -> Self {
        Self {
            mine: GamepadButton::RightTrigger,
            use_item: GamepadButton::LeftTrigger,
            hotbar_next: GamepadButton::RightBumper,
            hotbar_prev: GamepadButton::LeftBumper,
            jump: GamepadButton::South,
            sneak: GamepadButton::East,
            sprint: GamepadButton::LeftStick,
            inventory: GamepadButton::North,
            backpack: GamepadButton::West,
            third_person: GamepadButton::RightStick,
            pause: GamepadButton::Start,
            player_list: GamepadButton::Select,
            quick_menu: GamepadButton::DPadUp,
            flight: GamepadButton::DPadRight,
            chat: GamepadButton::DPadLeft,
            push_to_talk: GamepadButton::DPadDown,
        }
    }
}

impl ControllerBindings {
    pub const COUNT: usize = 16;

    pub fn labels_and_buttons(&self) -> [(&'static str, GamepadButton); Self::COUNT] {
        [
            ("Mine / Attack", self.mine),
            ("Place / Use", self.use_item),
            ("Hotbar Next", self.hotbar_next),
            ("Hotbar Prev", self.hotbar_prev),
            ("Jump / Fly Up", self.jump),
            ("Sneak / Fly Down", self.sneak),
            ("Sprint", self.sprint),
            ("Inventory", self.inventory),
            ("Backpack", self.backpack),
            ("3rd Person", self.third_person),
            ("Pause Menu", self.pause),
            ("Player List", self.player_list),
            ("Quick Menu", self.quick_menu),
            ("Toggle Flight", self.flight),
            ("Open Chat", self.chat),
            ("Push to Talk", self.push_to_talk),
        ]
    }

    pub fn set_by_index(&mut self, index: usize, button: GamepadButton) {
        match index {
            0 => self.mine = button,
            1 => self.use_item = button,
            2 => self.hotbar_next = button,
            3 => self.hotbar_prev = button,
            4 => self.jump = button,
            5 => self.sneak = button,
            6 => self.sprint = button,
            7 => self.inventory = button,
            8 => self.backpack = button,
            9 => self.third_person = button,
            10 => self.pause = button,
            11 => self.player_list = button,
            12 => self.quick_menu = button,
            13 => self.flight = button,
            14 => self.chat = button,
            15 => self.push_to_talk = button,
            _ => {}
        }
    }
}

/// Tuning parameters passed into `InputRouter::update` each frame.
#[derive(Debug, Clone)]
pub struct ControllerConfig {
    pub sensitivity: f32,
    pub invert_y: bool,
    pub deadzone: f32,
    pub rumble_intensity: f32,
    pub glyph_style: GlyphStyle,
    pub swap_confirm_cancel: bool,
    pub sneak_toggle: bool,
    pub bindings: ControllerBindings,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            sensitivity: 1.0,
            invert_y: false,
            deadzone: 0.15,
            rumble_intensity: 0.8,
            glyph_style: GlyphStyle::Auto,
            swap_confirm_cancel: false,
            sneak_toggle: true,
            bindings: ControllerBindings::default(),
        }
    }
}

/// Directional D-pad snap emitted in UI/menu screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpadDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Disconnect action requested when the Active Controller drops mid-session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisconnectAction {
    #[default]
    None,
    PauseSingleplayer,
    ShowMultiplayerToast,
}

/// Frame-by-frame output produced by `InputRouter::update`.
#[derive(Debug, Clone, Default)]
pub struct RouterOutput {
    /// True if a button was pressed while listening for a controller remapping.
    pub captured_rebind_button: Option<GamepadButton>,
    /// Analog movement vector `[strafe_right (-1..1), forward (-1..1)]`.
    pub move_axes: [f32; 2],
    /// Camera look delta in radians `[yaw_delta, pitch_delta]` for this frame.
    pub look_delta: [f32; 2],
    /// Continuous held gameplay states.
    pub mine_held: bool,
    pub use_held: bool,
    pub jump_held: bool,
    pub sneak_held: bool,
    pub sprint_held: bool,
    pub player_list_held: bool,
    pub ptt_held: bool,
    /// Edge-triggered gameplay presses & releases.
    pub mine_pressed: bool,
    pub mine_released: bool,
    pub use_pressed: bool,
    pub use_released: bool,
    pub jump_pressed: bool,
    pub jump_double_tapped: bool,
    pub hotbar_next_pressed: bool,
    pub hotbar_prev_pressed: bool,
    pub inventory_pressed: bool,
    pub backpack_pressed: bool,
    pub third_person_pressed: bool,
    pub pause_pressed: bool,
    pub quick_menu_pressed: bool,
    pub flight_pressed: bool,
    pub chat_pressed: bool,
    /// UI / Menu navigation outputs (active when in UI mode).
    pub virtual_cursor_pos: [f32; 2],
    pub virtual_cursor_moved: bool,
    pub ui_confirm_pressed: bool,
    pub ui_confirm_held: bool,
    pub ui_confirm_released: bool,
    pub ui_cancel_pressed: bool,
    pub ui_west_pressed: bool,
    pub ui_north_pressed: bool,
    pub ui_lt_pressed: bool,
    pub ui_rt_pressed: bool,
    pub ui_lb_pressed: bool,
    pub ui_rb_pressed: bool,
    pub ui_dpad_snap: Option<DpadDirection>,
    /// Right stick `[x, y]` for scrolling lists and rotating 3D Character Creator preview.
    pub ui_right_stick: [f32; 2],
    /// Disconnect handling when Active Controller drops mid-gameplay.
    pub disconnect_action: DisconnectAction,
}

#[derive(Debug, Default, Clone)]
struct PerGamepadState {
    buttons_down: HashSet<GamepadButton>,
    left_stick: [f32; 2],
    right_stick: [f32; 2],
    left_trigger: f32,
    right_trigger: f32,
    lt_digital: bool,
    rt_digital: bool,
}

/// Pure, windowless controller input router (primary testing seam from ADR 0008).
pub struct InputRouter {
    input_mode: InputMode,
    connected: HashMap<GamepadId, GamepadInfo>,
    states: HashMap<GamepadId, PerGamepadState>,
    active_controller: Option<GamepadId>,
    virtual_cursor: [f32; 2],
    sprint_toggled: bool,
    sneak_toggled: bool,
    last_jump_time: f32,
    elapsed_time: f32,
    prev_ui_confirm_held: bool,
    prev_mine_held: bool,
    prev_use_held: bool,
}

impl Default for InputRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl InputRouter {
    const TRIGGER_PRESS_THRESHOLD: f32 = 0.35;
    const TRIGGER_RELEASE_THRESHOLD: f32 = 0.25;
    const DOUBLE_TAP_WINDOW_SECS: f32 = 0.32;
    const BASE_LOOK_SPEED_RAD_PER_SEC: f32 = 2.6;
    const BASE_VIRTUAL_CURSOR_PX_PER_SEC: f32 = 760.0;

    pub fn new() -> Self {
        Self {
            input_mode: InputMode::KeyboardMouse,
            connected: HashMap::new(),
            states: HashMap::new(),
            active_controller: None,
            virtual_cursor: [640.0, 360.0],
            sprint_toggled: false,
            sneak_toggled: false,
            last_jump_time: -10.0,
            elapsed_time: 0.0,
            prev_ui_confirm_held: false,
            prev_mine_held: false,
            prev_use_held: false,
        }
    }

    /// Seeds already-connected gamepads on startup.
    pub fn sync_initial_gamepads(&mut self, backend: &dyn GamepadBackend) {
        for info in backend.connected_gamepads() {
            let id = info.id;
            self.connected.insert(id, info);
            self.states.entry(id).or_default();
            if self.active_controller.is_none() {
                self.active_controller = Some(id);
            }
        }
    }

    pub fn input_mode(&self) -> InputMode {
        self.input_mode
    }

    pub fn active_controller(&self) -> Option<GamepadId> {
        self.active_controller
    }

    pub fn active_controller_info(&self) -> Option<&GamepadInfo> {
        self.active_controller.and_then(|id| self.connected.get(&id))
    }

    pub fn detected_family(&self) -> ControllerFamily {
        self.active_controller_info()
            .map(|i| i.family)
            .unwrap_or(ControllerFamily::Xbox)
    }

    pub fn resolved_glyph_family(&self, glyph_style: GlyphStyle) -> ControllerFamily {
        glyph_style.resolve(self.detected_family())
    }

    /// Resolves `(confirm_button, cancel_button)` for UI screens based on Nintendo convention and override.
    pub fn menu_confirm_cancel_buttons(
        &self,
        config: &ControllerConfig,
    ) -> (GamepadButton, GamepadButton) {
        let family = self.resolved_glyph_family(config.glyph_style);
        let nintendo_default = family == ControllerFamily::Nintendo;
        let east_is_confirm = nintendo_default ^ config.swap_confirm_cancel;
        if east_is_confirm {
            (GamepadButton::East, GamepadButton::South)
        } else {
            (GamepadButton::South, GamepadButton::East)
        }
    }

    pub fn virtual_cursor(&self) -> [f32; 2] {
        self.virtual_cursor
    }

    pub fn set_virtual_cursor(&mut self, pos: [f32; 2]) {
        self.virtual_cursor = pos;
    }

    /// Switches Input Mode to `KeyboardMouse` when keyboard or mouse activity arrives.
    pub fn on_keyboard_mouse_activity(&mut self, cursor_pos: Option<[f32; 2]>) {
        self.input_mode = InputMode::KeyboardMouse;
        if let Some(pos) = cursor_pos {
            self.virtual_cursor = pos;
        }
    }

    /// Resets held toggle states (e.g., when entering menus or dying).
    pub fn reset_gameplay_toggles(&mut self) {
        self.sprint_toggled = false;
        self.sneak_toggled = false;
    }

    /// Dispatches a scaled rumble effect to the Active Controller if in Controller mode.
    pub fn trigger_rumble(
        &self,
        backend: &mut dyn GamepadBackend,
        effect: RumbleEffect,
        rumble_intensity: f32,
    ) {
        if self.input_mode != InputMode::Controller {
            return;
        }
        let Some(id) = self.active_controller else {
            return;
        };
        if let Some(req) = effect.to_request(rumble_intensity) {
            backend.set_rumble(id, req);
        }
    }

    /// Applies radial deadzone and smooth squared response curve to a 2D stick vector.
    pub fn apply_radial_deadzone(x: f32, y: f32, deadzone: f32) -> [f32; 2] {
        let dz = deadzone.clamp(0.05, 0.45);
        let mag = (x * x + y * y).sqrt();
        if mag <= dz || mag <= 1e-6 {
            return [0.0, 0.0];
        }
        let clamped_mag = mag.min(1.0);
        let normalized = ((clamped_mag - dz) / (1.0 - dz)).clamp(0.0, 1.0);
        // Smooth blend of linear + quadratic curve for fine precision + full reach
        let curved = normalized * (0.35 + 0.65 * normalized);
        [(x / mag) * curved, (y / mag) * curved]
    }

    /// Polls `backend`, updates internal state, and emits frame actions for gameplay or UI.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        backend: &mut dyn GamepadBackend,
        dt: f32,
        config: &ControllerConfig,
        in_gameplay: bool,
        is_multiplayer: bool,
        listening_rebind: bool,
        screen_size: [f32; 2],
        ui_scale: f32,
    ) -> RouterOutput {
        self.elapsed_time += dt.max(0.0);
        let mut out = RouterOutput {
            virtual_cursor_pos: self.virtual_cursor,
            ..Default::default()
        };

        let mut just_pressed_by_pad: Vec<(GamepadId, GamepadButton)> = Vec::new();
        let events = backend.poll_events();

        for ev in events {
            match ev {
                GamepadEvent::Connected(info) => {
                    let id = info.id;
                    self.connected.insert(id, info);
                    self.states.entry(id).or_default();
                    if self.active_controller.is_none() {
                        self.active_controller = Some(id);
                    }
                }
                GamepadEvent::Disconnected(id) => {
                    self.connected.remove(&id);
                    self.states.remove(&id);
                    if self.active_controller == Some(id) {
                        self.active_controller = self.connected.keys().next().copied();
                        self.reset_gameplay_toggles();
                        if in_gameplay && self.input_mode == InputMode::Controller {
                            out.disconnect_action = if is_multiplayer {
                                DisconnectAction::ShowMultiplayerToast
                            } else {
                                DisconnectAction::PauseSingleplayer
                            };
                        }
                    }
                }
                GamepadEvent::ButtonPressed { id, button } => {
                    self.activate_controller(id);
                    let st = self.states.entry(id).or_default();
                    if st.buttons_down.insert(button) {
                        just_pressed_by_pad.push((id, button));
                    }
                }
                GamepadEvent::ButtonReleased { id, button } => {
                    if let Some(st) = self.states.get_mut(&id) {
                        st.buttons_down.remove(&button);
                    }
                }
                GamepadEvent::AxisChanged { id, axis, value } => {
                    let mut activated = false;
                    {
                        let st = self.states.entry(id).or_default();
                        match axis {
                            GamepadAxis::LeftStickX => st.left_stick[0] = value,
                            GamepadAxis::LeftStickY => st.left_stick[1] = value,
                            GamepadAxis::RightStickX => st.right_stick[0] = value,
                            GamepadAxis::RightStickY => st.right_stick[1] = value,
                            GamepadAxis::LeftTrigger => {
                                st.left_trigger = value.clamp(0.0, 1.0);
                                if !st.lt_digital && st.left_trigger >= Self::TRIGGER_PRESS_THRESHOLD {
                                    st.lt_digital = true;
                                    st.buttons_down.insert(GamepadButton::LeftTrigger);
                                    just_pressed_by_pad.push((id, GamepadButton::LeftTrigger));
                                    activated = true;
                                } else if st.lt_digital && st.left_trigger < Self::TRIGGER_RELEASE_THRESHOLD {
                                    st.lt_digital = false;
                                    st.buttons_down.remove(&GamepadButton::LeftTrigger);
                                }
                            }
                            GamepadAxis::RightTrigger => {
                                st.right_trigger = value.clamp(0.0, 1.0);
                                if !st.rt_digital && st.right_trigger >= Self::TRIGGER_PRESS_THRESHOLD {
                                    st.rt_digital = true;
                                    st.buttons_down.insert(GamepadButton::RightTrigger);
                                    just_pressed_by_pad.push((id, GamepadButton::RightTrigger));
                                    activated = true;
                                } else if st.rt_digital && st.right_trigger < Self::TRIGGER_RELEASE_THRESHOLD {
                                    st.rt_digital = false;
                                    st.buttons_down.remove(&GamepadButton::RightTrigger);
                                }
                            }
                        }
                        // Stick motion past deadzone activates controller mode & switches Active Controller
                        let stick_active = match axis {
                            GamepadAxis::LeftStickX | GamepadAxis::LeftStickY => {
                                let v = Self::apply_radial_deadzone(
                                    st.left_stick[0],
                                    st.left_stick[1],
                                    config.deadzone,
                                );
                                v[0].abs() > 0.0 || v[1].abs() > 0.0
                            }
                            GamepadAxis::RightStickX | GamepadAxis::RightStickY => {
                                let v = Self::apply_radial_deadzone(
                                    st.right_stick[0],
                                    st.right_stick[1],
                                    config.deadzone,
                                );
                                v[0].abs() > 0.0 || v[1].abs() > 0.0
                            }
                            _ => false,
                        };
                        if stick_active {
                            activated = true;
                        }
                    }
                    if activated {
                        self.activate_controller(id);
                    }
                }
            }
        }

        let Some(active_id) = self.active_controller else {
            return out;
        };

        let active_pressed: Vec<GamepadButton> = just_pressed_by_pad
            .into_iter()
            .filter(|(id, _)| *id == active_id)
            .map(|(_, btn)| btn)
            .collect();

        // When listening for a controller button rebind in Settings, capture the first button press
        if listening_rebind {
            if let Some(&btn) = active_pressed.first() {
                out.captured_rebind_button = Some(btn);
            }
            return out;
        }

        let Some(st) = self.states.get(&active_id).cloned() else {
            return out;
        };

        let left = Self::apply_radial_deadzone(st.left_stick[0], st.left_stick[1], config.deadzone);
        let right =
            Self::apply_radial_deadzone(st.right_stick[0], st.right_stick[1], config.deadzone);

        if in_gameplay {
            let b = &config.bindings;
            out.move_axes = left;

            let y_sign = if config.invert_y { -1.0 } else { 1.0 };
            let sens = config.sensitivity.clamp(0.2, 2.5);
            out.look_delta = [
                right[0] * Self::BASE_LOOK_SPEED_RAD_PER_SEC * sens * dt,
                right[1] * y_sign * Self::BASE_LOOK_SPEED_RAD_PER_SEC * sens * dt,
            ];

            let is_down = |btn: GamepadButton| st.buttons_down.contains(&btn);
            let was_pressed = |btn: GamepadButton| active_pressed.contains(&btn);

            out.mine_held = is_down(b.mine);
            out.use_held = is_down(b.use_item);
            out.jump_held = is_down(b.jump);
            out.player_list_held = is_down(b.player_list);
            out.ptt_held = is_down(b.push_to_talk);

            out.mine_pressed = was_pressed(b.mine);
            out.mine_released = self.prev_mine_held && !out.mine_held;
            out.use_pressed = was_pressed(b.use_item);
            out.use_released = self.prev_use_held && !out.use_held;
            self.prev_mine_held = out.mine_held;
            self.prev_use_held = out.use_held;
            out.jump_pressed = was_pressed(b.jump);
            out.hotbar_next_pressed = was_pressed(b.hotbar_next);
            out.hotbar_prev_pressed = was_pressed(b.hotbar_prev);
            out.inventory_pressed = was_pressed(b.inventory);
            out.backpack_pressed = was_pressed(b.backpack);
            out.third_person_pressed = was_pressed(b.third_person);
            out.pause_pressed = was_pressed(b.pause);
            out.quick_menu_pressed = was_pressed(b.quick_menu);
            out.flight_pressed = was_pressed(b.flight);
            out.chat_pressed = was_pressed(b.chat);

            if out.jump_pressed {
                if self.elapsed_time - self.last_jump_time <= Self::DOUBLE_TAP_WINDOW_SECS {
                    out.jump_double_tapped = true;
                    self.last_jump_time = -10.0;
                } else {
                    self.last_jump_time = self.elapsed_time;
                }
            }

            // Sprint defaults to toggle and clears automatically when the player stops moving
            if was_pressed(b.sprint) {
                self.sprint_toggled = !self.sprint_toggled;
            }
            if left[0].abs() < 0.05 && left[1].abs() < 0.05 {
                self.sprint_toggled = false;
            }
            out.sprint_held = self.sprint_toggled;

            // Sneak: toggle or hold per Controller Settings
            if config.sneak_toggle {
                if was_pressed(b.sneak) {
                    self.sneak_toggled = !self.sneak_toggled;
                }
                out.sneak_held = self.sneak_toggled;
            } else {
                self.sneak_toggled = false;
                out.sneak_held = is_down(b.sneak);
            }
            self.prev_ui_confirm_held = false;
        } else {
            out.mine_released = self.prev_mine_held;
            out.use_released = self.prev_use_held;
            self.prev_mine_held = false;
            self.prev_use_held = false;
            // UI / Menu Hybrid Virtual Cursor navigation
            if self.input_mode == InputMode::Controller {
                let speed = Self::BASE_VIRTUAL_CURSOR_PX_PER_SEC * ui_scale.clamp(0.75, 1.5);
                if left[0].abs() > 0.0 || left[1].abs() > 0.0 {
                    self.virtual_cursor[0] = (self.virtual_cursor[0] + left[0] * speed * dt)
                        .clamp(0.0, screen_size[0].max(1.0));
                    // Stick Y is +up in gilrs, screen Y is +down
                    self.virtual_cursor[1] = (self.virtual_cursor[1] - left[1] * speed * dt)
                        .clamp(0.0, screen_size[1].max(1.0));
                    out.virtual_cursor_moved = true;
                }
            }
            out.virtual_cursor_pos = self.virtual_cursor;
            out.ui_right_stick = right;

            let (confirm_btn, cancel_btn) = self.menu_confirm_cancel_buttons(config);
            let was_pressed = |btn: GamepadButton| active_pressed.contains(&btn);
            let confirm_held = st.buttons_down.contains(&confirm_btn);

            out.ui_confirm_pressed = was_pressed(confirm_btn);
            out.ui_confirm_held = confirm_held;
            out.ui_confirm_released = self.prev_ui_confirm_held && !confirm_held;
            self.prev_ui_confirm_held = confirm_held;

            out.ui_cancel_pressed = was_pressed(cancel_btn) || was_pressed(GamepadButton::Start);
            out.ui_west_pressed = was_pressed(GamepadButton::West);
            out.ui_north_pressed = was_pressed(GamepadButton::North);
            out.ui_lt_pressed = was_pressed(GamepadButton::LeftTrigger);
            out.ui_rt_pressed = was_pressed(GamepadButton::RightTrigger);
            out.ui_lb_pressed = was_pressed(GamepadButton::LeftBumper);
            out.ui_rb_pressed = was_pressed(GamepadButton::RightBumper);

            if was_pressed(GamepadButton::DPadUp) {
                out.ui_dpad_snap = Some(DpadDirection::Up);
            } else if was_pressed(GamepadButton::DPadDown) {
                out.ui_dpad_snap = Some(DpadDirection::Down);
            } else if was_pressed(GamepadButton::DPadLeft) {
                out.ui_dpad_snap = Some(DpadDirection::Left);
            } else if was_pressed(GamepadButton::DPadRight) {
                out.ui_dpad_snap = Some(DpadDirection::Right);
            }
        }

        out
    }

    fn activate_controller(&mut self, id: GamepadId) {
        self.input_mode = InputMode::Controller;
        self.active_controller = Some(id);
    }
}

/// Finds the best snap target center `[x, y]` in `direction` from `current` among `rects` (`[x, y, w, h]`).
pub fn find_dpad_snap_target(
    current: [f32; 2],
    direction: DpadDirection,
    rects: &[[f32; 4]],
) -> Option<[f32; 2]> {
    let mut best: Option<(f32, [f32; 2])> = None;
    for r in rects {
        if r[2] <= 0.0 || r[3] <= 0.0 {
            continue;
        }
        let cx = r[0] + r[2] * 0.5;
        let cy = r[1] + r[3] * 0.5;
        let dx = cx - current[0];
        let dy = cy - current[1];

        let (primary, cross) = match direction {
            DpadDirection::Up => (-dy, dx.abs()),
            DpadDirection::Down => (dy, dx.abs()),
            DpadDirection::Left => (-dx, dy.abs()),
            DpadDirection::Right => (dx, dy.abs()),
        };

        // Must lie strictly in the requested direction (ignore currently-centered item)
        if primary <= 4.0 {
            continue;
        }
        // Favor aligned targets in the same column/row strongly over far diagonal targets
        let score = primary + cross * 2.5;
        if best.is_none_or(|(best_score, _)| score < best_score) {
            best = Some((score, [cx, cy]));
        }
    }
    best.map(|(_, pos)| pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockGamepadBackend {
        queued_events: Vec<GamepadEvent>,
        connected: Vec<GamepadInfo>,
        rumble_log: Vec<(GamepadId, RumbleRequest)>,
    }

    impl MockGamepadBackend {
        fn push(&mut self, ev: GamepadEvent) {
            if let GamepadEvent::Connected(ref info) = ev {
                self.connected.push(info.clone());
            } else if let GamepadEvent::Disconnected(id) = ev {
                self.connected.retain(|g| g.id != id);
            }
            self.queued_events.push(ev);
        }
    }

    impl GamepadBackend for MockGamepadBackend {
        fn poll_events(&mut self) -> Vec<GamepadEvent> {
            std::mem::take(&mut self.queued_events)
        }

        fn connected_gamepads(&self) -> Vec<GamepadInfo> {
            self.connected.clone()
        }

        fn set_rumble(&mut self, id: GamepadId, request: RumbleRequest) {
            self.rumble_log.push((id, request));
        }
    }

    fn make_info(id: usize, vid: Option<u16>, pid: Option<u16>, name: &str) -> GamepadInfo {
        GamepadInfo {
            id: GamepadId(id),
            name: name.to_string(),
            vendor_id: vid,
            product_id: pid,
            family: detect_controller_family(vid, pid, name),
        }
    }

    #[test]
    fn detects_all_five_controller_families_by_vid_pid_and_name() {
        assert_eq!(
            detect_controller_family(Some(0x045e), Some(0x0b12), "Xbox Series X Controller"),
            ControllerFamily::Xbox
        );
        assert_eq!(
            detect_controller_family(Some(0x054c), Some(0x0ce6), "DualSense Wireless Controller"),
            ControllerFamily::PlayStation
        );
        assert_eq!(
            detect_controller_family(Some(0x057e), Some(0x2009), "Pro Controller"),
            ControllerFamily::Nintendo
        );
        assert_eq!(
            detect_controller_family(Some(0x28de), Some(0x1205), "Steam Deck"),
            ControllerFamily::SteamDeck
        );
        assert_eq!(
            detect_controller_family(Some(0x28de), Some(0x1142), "Valve Steam Controller"),
            ControllerFamily::SteamController
        );
        // Unknown generic pad falls back to Xbox
        assert_eq!(
            detect_controller_family(Some(0x1234), Some(0x5678), "Generic USB Gamepad"),
            ControllerFamily::Xbox
        );
    }

    #[test]
    fn input_mode_switches_automatically_and_active_controller_hands_off() {
        let mut backend = MockGamepadBackend::default();
        let mut router = InputRouter::new();
        let cfg = ControllerConfig::default();

        let xbox = make_info(0, Some(0x045e), Some(0x02ea), "Xbox One Controller");
        let ps5 = make_info(1, Some(0x054c), Some(0x0ce6), "DualSense");
        backend.push(GamepadEvent::Connected(xbox));
        backend.push(GamepadEvent::Connected(ps5));

        router.update(&mut backend, 0.016, &cfg, true, false, false, [1280.0, 720.0], 1.0);
        assert_eq!(router.input_mode(), InputMode::KeyboardMouse);
        assert_eq!(router.active_controller(), Some(GamepadId(0)));

        // Pressing a button on pad 1 switches both InputMode and ActiveController
        backend.push(GamepadEvent::ButtonPressed {
            id: GamepadId(1),
            button: GamepadButton::South,
        });
        let out = router.update(&mut backend, 0.016, &cfg, true, false, false, [1280.0, 720.0], 1.0);
        assert_eq!(router.input_mode(), InputMode::Controller);
        assert_eq!(router.active_controller(), Some(GamepadId(1)));
        assert_eq!(router.resolved_glyph_family(GlyphStyle::Auto), ControllerFamily::PlayStation);
        assert!(out.jump_pressed);

        // Keyboard/mouse activity immediately flips InputMode back
        router.on_keyboard_mouse_activity(Some([300.0, 200.0]));
        assert_eq!(router.input_mode(), InputMode::KeyboardMouse);
        assert_eq!(router.virtual_cursor(), [300.0, 200.0]);
    }

    #[test]
    fn disconnect_pauses_singleplayer_and_toasts_multiplayer() {
        let mut backend = MockGamepadBackend::default();
        let mut router = InputRouter::new();
        let cfg = ControllerConfig::default();

        backend.push(GamepadEvent::Connected(make_info(0, Some(0x045e), None, "Xbox")));
        backend.push(GamepadEvent::ButtonPressed {
            id: GamepadId(0),
            button: GamepadButton::South,
        });
        router.update(&mut backend, 0.016, &cfg, true, false, false, [1280.0, 720.0], 1.0);

        // Singleplayer disconnect -> PauseSingleplayer
        backend.push(GamepadEvent::Disconnected(GamepadId(0)));
        let sp_out =
            router.update(&mut backend, 0.016, &cfg, true, false, false, [1280.0, 720.0], 1.0);
        assert_eq!(sp_out.disconnect_action, DisconnectAction::PauseSingleplayer);

        // Reconnect and test Multiplayer disconnect -> ShowMultiplayerToast
        backend.push(GamepadEvent::Connected(make_info(2, Some(0x054c), None, "DualSense")));
        backend.push(GamepadEvent::ButtonPressed {
            id: GamepadId(2),
            button: GamepadButton::South,
        });
        router.update(&mut backend, 0.016, &cfg, true, true, false, [1280.0, 720.0], 1.0);

        backend.push(GamepadEvent::Disconnected(GamepadId(2)));
        let mp_out =
            router.update(&mut backend, 0.016, &cfg, true, true, false, [1280.0, 720.0], 1.0);
        assert_eq!(mp_out.disconnect_action, DisconnectAction::ShowMultiplayerToast);
    }

    #[test]
    fn deadzone_invert_y_sprint_toggle_and_sneak_modes() {
        let mut backend = MockGamepadBackend::default();
        let mut router = InputRouter::new();
        let mut cfg = ControllerConfig::default();
        cfg.deadzone = 0.20;
        cfg.invert_y = true;
        cfg.sneak_toggle = true;

        backend.push(GamepadEvent::Connected(make_info(0, Some(0x045e), None, "Xbox")));
        // Sub-deadzone noise should produce zero movement and zero look
        backend.push(GamepadEvent::AxisChanged {
            id: GamepadId(0),
            axis: GamepadAxis::LeftStickX,
            value: 0.10,
        });
        backend.push(GamepadEvent::AxisChanged {
            id: GamepadId(0),
            axis: GamepadAxis::RightStickY,
            value: 0.10,
        });
        let out = router.update(&mut backend, 0.016, &cfg, true, false, false, [1280.0, 720.0], 1.0);
        assert_eq!(out.move_axes, [0.0, 0.0]);
        assert_eq!(out.look_delta, [0.0, 0.0]);

        // Full right stick up with invert_y=true produces negative pitch delta
        backend.push(GamepadEvent::AxisChanged {
            id: GamepadId(0),
            axis: GamepadAxis::RightStickY,
            value: 1.0,
        });
        backend.push(GamepadEvent::AxisChanged {
            id: GamepadId(0),
            axis: GamepadAxis::LeftStickY,
            value: 1.0,
        });
        backend.push(GamepadEvent::ButtonPressed {
            id: GamepadId(0),
            button: GamepadButton::LeftStick,
        });
        backend.push(GamepadEvent::ButtonPressed {
            id: GamepadId(0),
            button: GamepadButton::East,
        });
        let out = router.update(&mut backend, 0.1, &cfg, true, false, false, [1280.0, 720.0], 1.0);
        assert!(out.look_delta[1] < 0.0, "invert_y should negate pitch delta");
        assert!(out.sprint_held, "sprint toggle should activate while moving");
        assert!(out.sneak_held, "sneak toggle should activate on East press");

        // Releasing stick clears sprint toggle automatically, while sneak toggle stays active
        backend.push(GamepadEvent::AxisChanged {
            id: GamepadId(0),
            axis: GamepadAxis::LeftStickY,
            value: 0.0,
        });
        backend.push(GamepadEvent::ButtonReleased {
            id: GamepadId(0),
            button: GamepadButton::East,
        });
        let out = router.update(&mut backend, 0.1, &cfg, true, false, false, [1280.0, 720.0], 1.0);
        assert!(!out.sprint_held, "sprint toggle should clear when movement stops");
        assert!(out.sneak_held, "sneak toggle should stay on after release");

        // Switching to hold mode clears sneak immediately when not held
        cfg.sneak_toggle = false;
        let out = router.update(&mut backend, 0.1, &cfg, true, false, false, [1280.0, 720.0], 1.0);
        assert!(!out.sneak_held);
    }

    #[test]
    fn nintendo_confirm_cancel_convention_and_swap_override() {
        let mut backend = MockGamepadBackend::default();
        let mut router = InputRouter::new();
        let mut cfg = ControllerConfig::default();

        backend.push(GamepadEvent::Connected(make_info(
            0,
            Some(0x057e),
            Some(0x2009),
            "Pro Controller",
        )));
        backend.push(GamepadEvent::ButtonPressed {
            id: GamepadId(0),
            button: GamepadButton::East,
        });
        let out = router.update(&mut backend, 0.016, &cfg, false, false, false, [1280.0, 720.0], 1.0);
        assert!(out.ui_confirm_pressed, "Nintendo East (A) should confirm in menus");
        assert!(!out.ui_cancel_pressed);

        // With swap_confirm_cancel enabled, East becomes cancel and South becomes confirm
        cfg.swap_confirm_cancel = true;
        backend.push(GamepadEvent::ButtonReleased {
            id: GamepadId(0),
            button: GamepadButton::East,
        });
        router.update(&mut backend, 0.016, &cfg, false, false, false, [1280.0, 720.0], 1.0);
        backend.push(GamepadEvent::ButtonPressed {
            id: GamepadId(0),
            button: GamepadButton::East,
        });
        let out = router.update(&mut backend, 0.016, &cfg, false, false, false, [1280.0, 720.0], 1.0);
        assert!(out.ui_cancel_pressed);
        assert!(!out.ui_confirm_pressed);
    }

    #[test]
    fn rumble_intensity_scales_and_zero_disables_rumble() {
        let mut backend = MockGamepadBackend::default();
        let mut router = InputRouter::new();
        let cfg = ControllerConfig::default();

        backend.push(GamepadEvent::Connected(make_info(0, Some(0x045e), None, "Xbox")));
        backend.push(GamepadEvent::ButtonPressed {
            id: GamepadId(0),
            button: GamepadButton::South,
        });
        router.update(&mut backend, 0.016, &cfg, true, false, false, [1280.0, 720.0], 1.0);

        router.trigger_rumble(&mut backend, RumbleEffect::Damage, 0.0);
        assert!(backend.rumble_log.is_empty(), "0% intensity must not rumble");

        router.trigger_rumble(&mut backend, RumbleEffect::Damage, 0.5);
        assert_eq!(backend.rumble_log.len(), 1);
        let (id, req) = backend.rumble_log[0];
        assert_eq!(id, GamepadId(0));
        assert!((req.strong - 0.425).abs() < 1e-3);
    }

    #[test]
    fn custom_rebind_and_dpad_snap_target_selection() {
        let mut backend = MockGamepadBackend::default();
        let mut router = InputRouter::new();
        let mut cfg = ControllerConfig::default();
        cfg.bindings.jump = GamepadButton::RightBumper;

        backend.push(GamepadEvent::Connected(make_info(0, Some(0x045e), None, "Xbox")));
        backend.push(GamepadEvent::ButtonPressed {
            id: GamepadId(0),
            button: GamepadButton::RightBumper,
        });
        let out = router.update(&mut backend, 0.016, &cfg, true, false, false, [1280.0, 720.0], 1.0);
        assert!(out.jump_pressed);

        // D-pad snap picks aligned neighbor in requested direction
        let rects = [
            [100.0, 100.0, 50.0, 50.0],
            [100.0, 200.0, 50.0, 50.0],
            [300.0, 200.0, 50.0, 50.0],
        ];
        let snapped = find_dpad_snap_target([125.0, 125.0], DpadDirection::Down, &rects);
        assert_eq!(snapped, Some([125.0, 225.0]));
    }

    #[test]
    fn trigger_press_and_release_edges_do_not_latch() {
        let mut backend = MockGamepadBackend::default();
        let mut router = InputRouter::new();
        let cfg = ControllerConfig::default();

        backend.push(GamepadEvent::Connected(make_info(0, Some(0x045e), None, "Xbox")));
        // Press RT (mine) and LT (use) via analog axes
        backend.push(GamepadEvent::AxisChanged {
            id: GamepadId(0),
            axis: GamepadAxis::RightTrigger,
            value: 0.9,
        });
        backend.push(GamepadEvent::AxisChanged {
            id: GamepadId(0),
            axis: GamepadAxis::LeftTrigger,
            value: 0.9,
        });
        let out = router.update(&mut backend, 0.016, &cfg, true, false, false, [1280.0, 720.0], 1.0);
        assert!(out.mine_pressed && out.mine_held && !out.mine_released);
        assert!(out.use_pressed && out.use_held && !out.use_released);

        // Release both triggers below threshold
        backend.push(GamepadEvent::AxisChanged {
            id: GamepadId(0),
            axis: GamepadAxis::RightTrigger,
            value: 0.0,
        });
        backend.push(GamepadEvent::AxisChanged {
            id: GamepadId(0),
            axis: GamepadAxis::LeftTrigger,
            value: 0.0,
        });
        let out = router.update(&mut backend, 0.016, &cfg, true, false, false, [1280.0, 720.0], 1.0);
        assert!(!out.mine_pressed && !out.mine_held && out.mine_released);
        assert!(!out.use_pressed && !out.use_held && out.use_released);
    }
}
