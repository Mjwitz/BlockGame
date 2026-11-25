//! Controller domain types and procedural 32×32 pixel-art button glyph tiles
//! for Xbox, PlayStation, Nintendo, Steam Deck, and Steam Controller.

use image::Rgba;

use crate::atlas::{TextureAtlas, TILE_SIZE};

/// Currently active player control scheme, automatically switched on last-used input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    KeyboardMouse,
    Controller,
}

/// Recognized controller hardware profile inferred from VID/PID and device name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ControllerFamily {
    #[default]
    Xbox,
    PlayStation,
    Nintendo,
    SteamDeck,
    SteamController,
}

impl ControllerFamily {
    pub const ALL: [ControllerFamily; 5] = [
        ControllerFamily::Xbox,
        ControllerFamily::PlayStation,
        ControllerFamily::Nintendo,
        ControllerFamily::SteamDeck,
        ControllerFamily::SteamController,
    ];

    pub fn index(self) -> usize {
        match self {
            ControllerFamily::Xbox => 0,
            ControllerFamily::PlayStation => 1,
            ControllerFamily::Nintendo => 2,
            ControllerFamily::SteamDeck => 3,
            ControllerFamily::SteamController => 4,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ControllerFamily::Xbox => "Xbox",
            ControllerFamily::PlayStation => "PlayStation",
            ControllerFamily::Nintendo => "Nintendo",
            ControllerFamily::SteamDeck => "Steam Deck",
            ControllerFamily::SteamController => "Steam Controller",
        }
    }
}

/// Button-icon set shown in HUD prompts, tooltips, and menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GlyphStyle {
    #[default]
    Auto,
    Xbox,
    PlayStation,
    Nintendo,
    SteamDeck,
    SteamController,
}

impl GlyphStyle {
    pub const ALL: [GlyphStyle; 6] = [
        GlyphStyle::Auto,
        GlyphStyle::Xbox,
        GlyphStyle::PlayStation,
        GlyphStyle::Nintendo,
        GlyphStyle::SteamDeck,
        GlyphStyle::SteamController,
    ];

    pub fn cycle_next(self) -> Self {
        match self {
            GlyphStyle::Auto => GlyphStyle::Xbox,
            GlyphStyle::Xbox => GlyphStyle::PlayStation,
            GlyphStyle::PlayStation => GlyphStyle::Nintendo,
            GlyphStyle::Nintendo => GlyphStyle::SteamDeck,
            GlyphStyle::SteamDeck => GlyphStyle::SteamController,
            GlyphStyle::SteamController => GlyphStyle::Auto,
        }
    }

    pub fn resolve(self, detected: ControllerFamily) -> ControllerFamily {
        match self {
            GlyphStyle::Auto => detected,
            GlyphStyle::Xbox => ControllerFamily::Xbox,
            GlyphStyle::PlayStation => ControllerFamily::PlayStation,
            GlyphStyle::Nintendo => ControllerFamily::Nintendo,
            GlyphStyle::SteamDeck => ControllerFamily::SteamDeck,
            GlyphStyle::SteamController => ControllerFamily::SteamController,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            GlyphStyle::Auto => "Auto",
            GlyphStyle::Xbox => "Xbox",
            GlyphStyle::PlayStation => "PlayStation",
            GlyphStyle::Nintendo => "Nintendo",
            GlyphStyle::SteamDeck => "Steam Deck",
            GlyphStyle::SteamController => "Steam Controller",
        }
    }

    pub fn as_config_str(self) -> &'static str {
        match self {
            GlyphStyle::Auto => "auto",
            GlyphStyle::Xbox => "xbox",
            GlyphStyle::PlayStation => "playstation",
            GlyphStyle::Nintendo => "nintendo",
            GlyphStyle::SteamDeck => "steam_deck",
            GlyphStyle::SteamController => "steam_controller",
        }
    }

    pub fn from_config_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(GlyphStyle::Auto),
            "xbox" => Some(GlyphStyle::Xbox),
            "playstation" | "ps" => Some(GlyphStyle::PlayStation),
            "nintendo" | "switch" => Some(GlyphStyle::Nintendo),
            "steam_deck" | "steamdeck" => Some(GlyphStyle::SteamDeck),
            "steam_controller" | "steamcontroller" => Some(GlyphStyle::SteamController),
            _ => None,
        }
    }
}

/// Positional logical gamepad buttons shared across all Controller Families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    LeftStick,
    RightStick,
    Start,
    Select,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

impl GamepadButton {
    pub const COUNT: usize = 16;

    pub const ALL: [GamepadButton; Self::COUNT] = [
        GamepadButton::South,
        GamepadButton::East,
        GamepadButton::West,
        GamepadButton::North,
        GamepadButton::LeftBumper,
        GamepadButton::RightBumper,
        GamepadButton::LeftTrigger,
        GamepadButton::RightTrigger,
        GamepadButton::LeftStick,
        GamepadButton::RightStick,
        GamepadButton::Start,
        GamepadButton::Select,
        GamepadButton::DPadUp,
        GamepadButton::DPadDown,
        GamepadButton::DPadLeft,
        GamepadButton::DPadRight,
    ];

    pub fn index(self) -> usize {
        match self {
            GamepadButton::South => 0,
            GamepadButton::East => 1,
            GamepadButton::West => 2,
            GamepadButton::North => 3,
            GamepadButton::LeftBumper => 4,
            GamepadButton::RightBumper => 5,
            GamepadButton::LeftTrigger => 6,
            GamepadButton::RightTrigger => 7,
            GamepadButton::LeftStick => 8,
            GamepadButton::RightStick => 9,
            GamepadButton::Start => 10,
            GamepadButton::Select => 11,
            GamepadButton::DPadUp => 12,
            GamepadButton::DPadDown => 13,
            GamepadButton::DPadLeft => 14,
            GamepadButton::DPadRight => 15,
        }
    }

    pub fn as_config_str(self) -> &'static str {
        match self {
            GamepadButton::South => "South",
            GamepadButton::East => "East",
            GamepadButton::West => "West",
            GamepadButton::North => "North",
            GamepadButton::LeftBumper => "LeftBumper",
            GamepadButton::RightBumper => "RightBumper",
            GamepadButton::LeftTrigger => "LeftTrigger",
            GamepadButton::RightTrigger => "RightTrigger",
            GamepadButton::LeftStick => "LeftStick",
            GamepadButton::RightStick => "RightStick",
            GamepadButton::Start => "Start",
            GamepadButton::Select => "Select",
            GamepadButton::DPadUp => "DPadUp",
            GamepadButton::DPadDown => "DPadDown",
            GamepadButton::DPadLeft => "DPadLeft",
            GamepadButton::DPadRight => "DPadRight",
        }
    }

    pub fn from_config_str(s: &str) -> Option<Self> {
        match s.trim() {
            "South" => Some(GamepadButton::South),
            "East" => Some(GamepadButton::East),
            "West" => Some(GamepadButton::West),
            "North" => Some(GamepadButton::North),
            "LeftBumper" => Some(GamepadButton::LeftBumper),
            "RightBumper" => Some(GamepadButton::RightBumper),
            "LeftTrigger" => Some(GamepadButton::LeftTrigger),
            "RightTrigger" => Some(GamepadButton::RightTrigger),
            "LeftStick" => Some(GamepadButton::LeftStick),
            "RightStick" => Some(GamepadButton::RightStick),
            "Start" => Some(GamepadButton::Start),
            "Select" => Some(GamepadButton::Select),
            "DPadUp" => Some(GamepadButton::DPadUp),
            "DPadDown" => Some(GamepadButton::DPadDown),
            "DPadLeft" => Some(GamepadButton::DPadLeft),
            "DPadRight" => Some(GamepadButton::DPadRight),
            _ => None,
        }
    }

    /// Short human-readable label for the given Controller Family.
    pub fn short_label(self, family: ControllerFamily) -> &'static str {
        match (family, self) {
            // Face buttons
            (ControllerFamily::Nintendo, GamepadButton::South) => "B",
            (ControllerFamily::Nintendo, GamepadButton::East) => "A",
            (ControllerFamily::Nintendo, GamepadButton::West) => "Y",
            (ControllerFamily::Nintendo, GamepadButton::North) => "X",
            (ControllerFamily::PlayStation, GamepadButton::South) => "Cross",
            (ControllerFamily::PlayStation, GamepadButton::East) => "Circle",
            (ControllerFamily::PlayStation, GamepadButton::West) => "Square",
            (ControllerFamily::PlayStation, GamepadButton::North) => "Triangle",
            (_, GamepadButton::South) => "A",
            (_, GamepadButton::East) => "B",
            (_, GamepadButton::West) => "X",
            (_, GamepadButton::North) => "Y",

            // Bumpers & Triggers
            (ControllerFamily::PlayStation | ControllerFamily::SteamDeck, GamepadButton::LeftBumper) => "L1",
            (ControllerFamily::PlayStation | ControllerFamily::SteamDeck, GamepadButton::RightBumper) => "R1",
            (ControllerFamily::PlayStation | ControllerFamily::SteamDeck, GamepadButton::LeftTrigger) => "L2",
            (ControllerFamily::PlayStation | ControllerFamily::SteamDeck, GamepadButton::RightTrigger) => "R2",
            (ControllerFamily::Nintendo, GamepadButton::LeftBumper) => "L",
            (ControllerFamily::Nintendo, GamepadButton::RightBumper) => "R",
            (ControllerFamily::Nintendo, GamepadButton::LeftTrigger) => "ZL",
            (ControllerFamily::Nintendo, GamepadButton::RightTrigger) => "ZR",
            (_, GamepadButton::LeftBumper) => "LB",
            (_, GamepadButton::RightBumper) => "RB",
            (_, GamepadButton::LeftTrigger) => "LT",
            (_, GamepadButton::RightTrigger) => "RT",

            // Stick clicks
            (ControllerFamily::PlayStation | ControllerFamily::SteamDeck | ControllerFamily::Nintendo, GamepadButton::LeftStick) => "L3",
            (ControllerFamily::PlayStation | ControllerFamily::SteamDeck | ControllerFamily::Nintendo, GamepadButton::RightStick) => "R3",
            (_, GamepadButton::LeftStick) => "LS",
            (_, GamepadButton::RightStick) => "RS",

            // Center buttons
            (ControllerFamily::PlayStation, GamepadButton::Start) => "Options",
            (ControllerFamily::PlayStation, GamepadButton::Select) => "Share",
            (ControllerFamily::Nintendo, GamepadButton::Start) => "+",
            (ControllerFamily::Nintendo, GamepadButton::Select) => "-",
            (ControllerFamily::SteamController, GamepadButton::Start) => "Start",
            (ControllerFamily::SteamController, GamepadButton::Select) => "Back",
            (_, GamepadButton::Start) => "Menu",
            (_, GamepadButton::Select) => "View",

            // D-pad
            (_, GamepadButton::DPadUp) => "D-Up",
            (_, GamepadButton::DPadDown) => "D-Down",
            (_, GamepadButton::DPadLeft) => "D-Left",
            (_, GamepadButton::DPadRight) => "D-Right",
        }
    }
}

/// First tile slot in `TextureAtlas` reserved for controller button glyphs (80 tiles: 640..720).
pub const CONTROLLER_GLYPH_TILE_BASE: u16 = 640;

/// Returns the deterministic `TextureAtlas` tile ID for `(family, button)`.
pub const fn glyph_tile_id(family: ControllerFamily, button: GamepadButton) -> u16 {
    let f = match family {
        ControllerFamily::Xbox => 0,
        ControllerFamily::PlayStation => 1,
        ControllerFamily::Nintendo => 2,
        ControllerFamily::SteamDeck => 3,
        ControllerFamily::SteamController => 4,
    };
    let b = match button {
        GamepadButton::South => 0,
        GamepadButton::East => 1,
        GamepadButton::West => 2,
        GamepadButton::North => 3,
        GamepadButton::LeftBumper => 4,
        GamepadButton::RightBumper => 5,
        GamepadButton::LeftTrigger => 6,
        GamepadButton::RightTrigger => 7,
        GamepadButton::LeftStick => 8,
        GamepadButton::RightStick => 9,
        GamepadButton::Start => 10,
        GamepadButton::Select => 11,
        GamepadButton::DPadUp => 12,
        GamepadButton::DPadDown => 13,
        GamepadButton::DPadLeft => 14,
        GamepadButton::DPadRight => 15,
    };
    CONTROLLER_GLYPH_TILE_BASE + f * (GamepadButton::COUNT as u16) + b
}

/// Paints all 80 procedural 32×32 pixel-art controller button glyphs into the texture atlas.
pub fn paint_controller_glyph_tiles(atlas: &mut TextureAtlas) {
    for family in ControllerFamily::ALL {
        for button in GamepadButton::ALL {
            let tile_id = glyph_tile_id(family, button);
            let mut buf = [[Rgba([0, 0, 0, 0]); TILE_SIZE as usize]; TILE_SIZE as usize];
            draw_glyph_bitmap(&mut buf, family, button);
            atlas.paint_tile(tile_id, |px, py| buf[py as usize][px as usize]);
        }
    }
}

fn draw_glyph_bitmap(
    buf: &mut [[Rgba<u8>; 32]; 32],
    family: ControllerFamily,
    button: GamepadButton,
) {
    match button {
        GamepadButton::South
        | GamepadButton::East
        | GamepadButton::West
        | GamepadButton::North => draw_face_button(buf, family, button),
        GamepadButton::LeftBumper | GamepadButton::RightBumper => {
            draw_bumper_button(buf, family, button)
        }
        GamepadButton::LeftTrigger | GamepadButton::RightTrigger => {
            draw_trigger_button(buf, family, button)
        }
        GamepadButton::LeftStick | GamepadButton::RightStick => {
            draw_stick_button(buf, family, button)
        }
        GamepadButton::Start | GamepadButton::Select => draw_center_button(buf, family, button),
        GamepadButton::DPadUp
        | GamepadButton::DPadDown
        | GamepadButton::DPadLeft
        | GamepadButton::DPadRight => draw_dpad_button(buf, family, button),
    }
}

fn draw_face_button(
    buf: &mut [[Rgba<u8>; 32]; 32],
    family: ControllerFamily,
    button: GamepadButton,
) {
    let (bg, rim, fg) = match (family, button) {
        // Xbox: signature colored face buttons
        (ControllerFamily::Xbox, GamepadButton::South) => (
            Rgba([38, 152, 62, 255]),
            Rgba([140, 235, 155, 255]),
            Rgba([255, 255, 255, 255]),
        ),
        (ControllerFamily::Xbox, GamepadButton::East) => (
            Rgba([196, 48, 48, 255]),
            Rgba([248, 145, 145, 255]),
            Rgba([255, 255, 255, 255]),
        ),
        (ControllerFamily::Xbox, GamepadButton::West) => (
            Rgba([38, 108, 206, 255]),
            Rgba([140, 195, 255, 255]),
            Rgba([255, 255, 255, 255]),
        ),
        (ControllerFamily::Xbox, GamepadButton::North) => (
            Rgba([206, 158, 28, 255]),
            Rgba([255, 232, 130, 255]),
            Rgba([28, 24, 14, 255]),
        ),
        // PlayStation: dark charcoal disc with iconic colored symbols
        (ControllerFamily::PlayStation, GamepadButton::South) => (
            Rgba([34, 38, 48, 255]),
            Rgba([95, 106, 128, 255]),
            Rgba([118, 178, 255, 255]),
        ),
        (ControllerFamily::PlayStation, GamepadButton::East) => (
            Rgba([34, 38, 48, 255]),
            Rgba([95, 106, 128, 255]),
            Rgba([255, 112, 112, 255]),
        ),
        (ControllerFamily::PlayStation, GamepadButton::West) => (
            Rgba([34, 38, 48, 255]),
            Rgba([95, 106, 128, 255]),
            Rgba([242, 132, 212, 255]),
        ),
        (ControllerFamily::PlayStation, GamepadButton::North) => (
            Rgba([34, 38, 48, 255]),
            Rgba([95, 106, 128, 255]),
            Rgba([92, 232, 186, 255]),
        ),
        // Nintendo: Switch Pro dark matte disc with bright cream letters (red accent on East 'A')
        (ControllerFamily::Nintendo, GamepadButton::East) => (
            Rgba([42, 44, 50, 255]),
            Rgba([230, 82, 82, 255]),
            Rgba([250, 248, 242, 255]),
        ),
        (ControllerFamily::Nintendo, _) => (
            Rgba([42, 44, 50, 255]),
            Rgba([175, 180, 190, 255]),
            Rgba([250, 248, 242, 255]),
        ),
        // Steam Deck: matte black with subtle Deck cyan-blue rim
        (ControllerFamily::SteamDeck, _) => (
            Rgba([28, 32, 40, 255]),
            Rgba([98, 184, 245, 255]),
            Rgba([245, 248, 255, 255]),
        ),
        // Steam Controller: glossy dark slate with subtly tinted letters
        (ControllerFamily::SteamController, GamepadButton::South) => (
            Rgba([30, 34, 42, 255]),
            Rgba([140, 150, 170, 255]),
            Rgba([110, 235, 130, 255]),
        ),
        (ControllerFamily::SteamController, GamepadButton::East) => (
            Rgba([30, 34, 42, 255]),
            Rgba([140, 150, 170, 255]),
            Rgba([255, 115, 115, 255]),
        ),
        (ControllerFamily::SteamController, GamepadButton::West) => (
            Rgba([30, 34, 42, 255]),
            Rgba([140, 150, 170, 255]),
            Rgba([115, 185, 255, 255]),
        ),
        (ControllerFamily::SteamController, _) => (
            Rgba([30, 34, 42, 255]),
            Rgba([140, 150, 170, 255]),
            Rgba([255, 224, 105, 255]),
        ),
        _ => (
            Rgba([38, 44, 56, 255]),
            Rgba([140, 150, 170, 255]),
            Rgba([248, 250, 255, 255]),
        ),
    };

    fill_circle(buf, 15.5, 15.5, 13.5, Rgba([12, 14, 18, 230]));
    fill_circle(buf, 15.5, 15.5, 12.2, rim);
    fill_circle(buf, 15.5, 15.5, 10.6, bg);

    if family == ControllerFamily::PlayStation {
        match button {
            GamepadButton::South => draw_ps_cross(buf, fg),
            GamepadButton::East => draw_ps_circle(buf, fg),
            GamepadButton::West => draw_ps_square(buf, fg),
            GamepadButton::North => draw_ps_triangle(buf, fg),
            _ => {}
        }
    } else {
        let ch = match (family, button) {
            (ControllerFamily::Nintendo, GamepadButton::South) => b'B',
            (ControllerFamily::Nintendo, GamepadButton::East) => b'A',
            (ControllerFamily::Nintendo, GamepadButton::West) => b'Y',
            (ControllerFamily::Nintendo, GamepadButton::North) => b'X',
            (_, GamepadButton::South) => b'A',
            (_, GamepadButton::East) => b'B',
            (_, GamepadButton::West) => b'X',
            (_, GamepadButton::North) => b'Y',
            _ => b'?',
        };
        draw_tiny_text_centered(buf, &[ch], 16, 16, 2, fg);
    }
}

fn draw_bumper_button(
    buf: &mut [[Rgba<u8>; 32]; 32],
    family: ControllerFamily,
    button: GamepadButton,
) {
    let rim = family_accent_rim(family);
    let bg = Rgba([38, 44, 56, 255]);
    fill_rounded_rect(buf, 2, 7, 28, 18, 5, Rgba([12, 14, 18, 230]));
    fill_rounded_rect(buf, 3, 8, 26, 16, 4, rim);
    fill_rounded_rect(buf, 4, 9, 24, 14, 3, bg);

    let label = button.short_label(family);
    let scale = if label.len() == 1 { 2 } else { 1 };
    draw_tiny_text_centered(buf, label.as_bytes(), 16, 16, scale, Rgba([248, 250, 255, 255]));
}

fn draw_trigger_button(
    buf: &mut [[Rgba<u8>; 32]; 32],
    family: ControllerFamily,
    button: GamepadButton,
) {
    let rim = family_accent_rim(family);
    let bg = Rgba([46, 52, 66, 255]);
    // Taller trigger silhouette (wider bottom, tapered top)
    fill_rounded_rect(buf, 5, 3, 22, 26, 6, Rgba([12, 14, 18, 230]));
    fill_rounded_rect(buf, 6, 4, 20, 24, 5, rim);
    fill_rounded_rect(buf, 7, 5, 18, 22, 4, bg);

    let label = button.short_label(family);
    draw_tiny_text_centered(buf, label.as_bytes(), 16, 17, 1, Rgba([248, 250, 255, 255]));
}

fn draw_stick_button(
    buf: &mut [[Rgba<u8>; 32]; 32],
    family: ControllerFamily,
    button: GamepadButton,
) {
    let rim = family_accent_rim(family);
    fill_circle(buf, 15.5, 15.5, 13.5, Rgba([12, 14, 18, 230]));
    fill_circle(buf, 15.5, 15.5, 12.2, rim);
    fill_circle(buf, 15.5, 15.5, 10.2, Rgba([34, 38, 48, 255]));
    fill_circle(buf, 15.5, 15.5, 8.2, Rgba([52, 58, 72, 255]));

    let label = button.short_label(family);
    draw_tiny_text_centered(buf, label.as_bytes(), 16, 16, 1, Rgba([250, 252, 255, 255]));
}

fn draw_center_button(
    buf: &mut [[Rgba<u8>; 32]; 32],
    family: ControllerFamily,
    button: GamepadButton,
) {
    let rim = family_accent_rim(family);
    let bg = Rgba([36, 40, 50, 255]);
    let fg = Rgba([245, 248, 255, 255]);

    match family {
        ControllerFamily::Nintendo => {
            fill_circle(buf, 15.5, 15.5, 11.5, Rgba([12, 14, 18, 230]));
            fill_circle(buf, 15.5, 15.5, 10.2, rim);
            fill_circle(buf, 15.5, 15.5, 8.8, bg);
            // Horizontal bar
            fill_rect(buf, 10, 14, 12, 4, fg);
            if button == GamepadButton::Start {
                // Vertical bar for '+'
                fill_rect(buf, 14, 10, 4, 12, fg);
            }
        }
        ControllerFamily::PlayStation => {
            fill_rounded_rect(buf, 4, 8, 24, 16, 7, Rgba([12, 14, 18, 230]));
            fill_rounded_rect(buf, 5, 9, 22, 14, 6, rim);
            fill_rounded_rect(buf, 6, 10, 20, 12, 5, bg);
            let txt = if button == GamepadButton::Start { b"OPT" } else { b"SHR" };
            draw_tiny_text_centered(buf, txt, 16, 16, 1, fg);
        }
        ControllerFamily::SteamController => {
            fill_rounded_rect(buf, 4, 8, 24, 16, 6, Rgba([12, 14, 18, 230]));
            fill_rounded_rect(buf, 5, 9, 22, 14, 5, rim);
            fill_rounded_rect(buf, 6, 10, 20, 12, 4, bg);
            // Right-pointing triangle (Start) or left-pointing triangle (Back)
            for row in 0..9i32 {
                let half = if row <= 4 { row } else { 8 - row };
                let y = (12 + row) as usize;
                for dx in 0..=(half * 2) {
                    let x = if button == GamepadButton::Start {
                        (12 + dx) as usize
                    } else {
                        (19 - dx) as usize
                    };
                    if x < 32 && y < 32 {
                        buf[y][x] = fg;
                    }
                }
            }
        }
        ControllerFamily::Xbox | ControllerFamily::SteamDeck => {
            fill_circle(buf, 15.5, 15.5, 12.0, Rgba([12, 14, 18, 230]));
            fill_circle(buf, 15.5, 15.5, 10.8, rim);
            fill_circle(buf, 15.5, 15.5, 9.4, bg);
            if button == GamepadButton::Start {
                // Hamburger Menu (3 horizontal bars)
                fill_rect(buf, 10, 11, 12, 2, fg);
                fill_rect(buf, 10, 15, 12, 2, fg);
                fill_rect(buf, 10, 19, 12, 2, fg);
            } else {
                // View (two overlapping squares)
                draw_rect_outline(buf, 10, 11, 8, 7, fg);
                draw_rect_outline(buf, 14, 14, 8, 7, fg);
            }
        }
    }
}

fn draw_dpad_button(
    buf: &mut [[Rgba<u8>; 32]; 32],
    family: ControllerFamily,
    button: GamepadButton,
) {
    let base = Rgba([44, 48, 58, 255]);
    let outline = Rgba([14, 16, 22, 235]);
    let hi = family_accent_rim(family);

    // Cross outline
    fill_rect(buf, 11, 3, 10, 26, outline);
    fill_rect(buf, 3, 11, 26, 10, outline);
    // Cross body
    fill_rect(buf, 12, 4, 8, 24, base);
    fill_rect(buf, 4, 12, 24, 8, base);
    // Highlighted direction arm
    match button {
        GamepadButton::DPadUp => fill_rect(buf, 13, 5, 6, 8, hi),
        GamepadButton::DPadDown => fill_rect(buf, 13, 19, 6, 8, hi),
        GamepadButton::DPadLeft => fill_rect(buf, 5, 13, 8, 6, hi),
        GamepadButton::DPadRight => fill_rect(buf, 19, 13, 8, 6, hi),
        _ => {}
    }
}

fn family_accent_rim(family: ControllerFamily) -> Rgba<u8> {
    match family {
        ControllerFamily::Xbox => Rgba([120, 215, 135, 255]),
        ControllerFamily::PlayStation => Rgba([130, 185, 255, 255]),
        ControllerFamily::Nintendo => Rgba([238, 104, 104, 255]),
        ControllerFamily::SteamDeck => Rgba([98, 184, 245, 255]),
        ControllerFamily::SteamController => Rgba([190, 200, 220, 255]),
    }
}

fn fill_circle(buf: &mut [[Rgba<u8>; 32]; 32], cx: f32, cy: f32, r: f32, color: Rgba<u8>) {
    let r2 = r * r;
    for py in 0..32 {
        for px in 0..32 {
            let dx = px as f32 - cx;
            let dy = py as f32 - cy;
            if dx * dx + dy * dy <= r2 {
                buf[py][px] = color;
            }
        }
    }
}

fn fill_rect(buf: &mut [[Rgba<u8>; 32]; 32], x: usize, y: usize, w: usize, h: usize, color: Rgba<u8>) {
    for py in y..(y + h).min(32) {
        for px in x..(x + w).min(32) {
            buf[py][px] = color;
        }
    }
}

fn draw_rect_outline(
    buf: &mut [[Rgba<u8>; 32]; 32],
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: Rgba<u8>,
) {
    if w == 0 || h == 0 {
        return;
    }
    fill_rect(buf, x, y, w, 1, color);
    fill_rect(buf, x, y + h - 1, w, 1, color);
    fill_rect(buf, x, y, 1, h, color);
    fill_rect(buf, x + w - 1, y, 1, h, color);
}

fn fill_rounded_rect(
    buf: &mut [[Rgba<u8>; 32]; 32],
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    r: usize,
    color: Rgba<u8>,
) {
    let x1 = x + w;
    let y1 = y + h;
    for py in y..y1.min(32) {
        for px in x..x1.min(32) {
            let lx = px - x;
            let ly = py - y;
            let in_corner = (lx < r && ly < r && (r - lx) * (r - lx) + (r - ly) * (r - ly) > r * r)
                || (lx >= w - r && ly < r && (lx + 1 - (w - r)) * (lx + 1 - (w - r)) + (r - ly) * (r - ly) > r * r)
                || (lx < r && ly >= h - r && (r - lx) * (r - lx) + (ly + 1 - (h - r)) * (ly + 1 - (h - r)) > r * r)
                || (lx >= w - r && ly >= h - r && (lx + 1 - (w - r)) * (lx + 1 - (w - r)) + (ly + 1 - (h - r)) * (ly + 1 - (h - r)) > r * r);
            if !in_corner {
                buf[py][px] = color;
            }
        }
    }
}

fn draw_ps_cross(buf: &mut [[Rgba<u8>; 32]; 32], color: Rgba<u8>) {
    for i in 0..13usize {
        let a = 10 + i;
        let b = 22 - i;
        for (px, py) in [(a, a), (a, a.saturating_sub(1)), (a, b), (a, b.saturating_sub(1))] {
            if px < 32 && py < 32 {
                buf[py][px] = color;
            }
        }
    }
}

fn draw_ps_circle(buf: &mut [[Rgba<u8>; 32]; 32], color: Rgba<u8>) {
    for py in 0..32 {
        for px in 0..32 {
            let dx = px as f32 - 15.5;
            let dy = py as f32 - 15.5;
            let d = (dx * dx + dy * dy).sqrt();
            if (4.5..=6.6).contains(&d) {
                buf[py][px] = color;
            }
        }
    }
}

fn draw_ps_square(buf: &mut [[Rgba<u8>; 32]; 32], color: Rgba<u8>) {
    draw_rect_outline(buf, 10, 10, 12, 12, color);
    draw_rect_outline(buf, 11, 11, 10, 10, color);
}

fn draw_ps_triangle(buf: &mut [[Rgba<u8>; 32]; 32], color: Rgba<u8>) {
    // Base line
    fill_rect(buf, 9, 20, 14, 2, color);
    // Two slanted sides meeting at (15..16, 9)
    for step in 0..=11i32 {
        let y = (9 + step) as usize;
        let lx = (15 - (step * 6) / 11) as usize;
        let rx = (16 + (step * 6) / 11) as usize;
        if y < 32 {
            if lx < 32 {
                buf[y][lx] = color;
                if lx + 1 < 32 {
                    buf[y][lx + 1] = color;
                }
            }
            if rx < 32 {
                buf[y][rx] = color;
                if rx > 0 {
                    buf[y][rx - 1] = color;
                }
            }
        }
    }
}

/// Compact 5×7 bitmap font for button glyph labels (`A, B, X, Y, L, R, Z, T, S, O, P, H, 1, 2, 3`).
fn tiny_glyph_5x7(ch: u8) -> [u8; 7] {
    match ch {
        b'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        b'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        b'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        b'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        b'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        b'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        b'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        b'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        b'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        b'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        b'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        b'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        b'1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        b'2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        b'3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        _ => [0b01110, 0b10001, 0b00010, 0b00100, 0b00100, 0b00000, 0b00100],
    }
}

fn draw_tiny_text_centered(
    buf: &mut [[Rgba<u8>; 32]; 32],
    text: &[u8],
    cx: usize,
    cy: usize,
    scale: usize,
    color: Rgba<u8>,
) {
    if text.is_empty() {
        return;
    }
    let char_w = 5 * scale;
    let char_h = 7 * scale;
    let gap = scale;
    let total_w = text.len() * char_w + (text.len() - 1) * gap;
    let start_x = cx.saturating_sub(total_w / 2);
    let start_y = cy.saturating_sub(char_h / 2);

    for (idx, &ch) in text.iter().enumerate() {
        let rows = tiny_glyph_5x7(ch);
        let ox = start_x + idx * (char_w + gap);
        for (ry, &bits) in rows.iter().enumerate() {
            for rx in 0..5usize {
                if (bits & (1 << (4 - rx))) != 0 {
                    fill_rect(buf, ox + rx * scale, start_y + ry * scale, scale, scale, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_tile_ids_are_unique_and_in_bounds() {
        let mut seen = std::collections::HashSet::new();
        for family in ControllerFamily::ALL {
            for button in GamepadButton::ALL {
                let id = glyph_tile_id(family, button);
                assert!(id >= CONTROLLER_GLYPH_TILE_BASE);
                assert!(id < 1024);
                assert!(seen.insert(id), "duplicate tile id {id}");
            }
        }
        assert_eq!(seen.len(), 80);
    }

    #[test]
    fn glyph_style_resolution_and_config_round_trip() {
        assert_eq!(
            GlyphStyle::Auto.resolve(ControllerFamily::Nintendo),
            ControllerFamily::Nintendo
        );
        assert_eq!(
            GlyphStyle::PlayStation.resolve(ControllerFamily::Xbox),
            ControllerFamily::PlayStation
        );
        for style in GlyphStyle::ALL {
            assert_eq!(GlyphStyle::from_config_str(style.as_config_str()), Some(style));
        }
    }

    #[test]
    fn nintendo_face_labels_are_swapped_relative_to_xbox() {
        assert_eq!(GamepadButton::South.short_label(ControllerFamily::Xbox), "A");
        assert_eq!(GamepadButton::East.short_label(ControllerFamily::Xbox), "B");
        assert_eq!(GamepadButton::South.short_label(ControllerFamily::Nintendo), "B");
        assert_eq!(GamepadButton::East.short_label(ControllerFamily::Nintendo), "A");
        assert_eq!(GamepadButton::LeftBumper.short_label(ControllerFamily::SteamDeck), "L1");
        assert_eq!(GamepadButton::LeftBumper.short_label(ControllerFamily::SteamController), "LB");
    }

    #[test]
    fn paint_controller_glyph_tiles_writes_visible_pixels_for_all_80_tiles() {
        let atlas = TextureAtlas::generate_default();
        for family in ControllerFamily::ALL {
            for button in GamepadButton::ALL {
                let tile_id = glyph_tile_id(family, button);
                let tx = (tile_id as u32 % crate::atlas::ATLAS_TILES_PER_ROW) * TILE_SIZE;
                let ty = (tile_id as u32 / crate::atlas::ATLAS_TILES_PER_ROW) * TILE_SIZE;
                let mut opaque = 0usize;
                for py in 0..TILE_SIZE {
                    for px in 0..TILE_SIZE {
                        if atlas.image.get_pixel(tx + px, ty + py).0[3] > 0 {
                            opaque += 1;
                        }
                    }
                }
                assert!(
                    opaque > 50,
                    "expected visible pixels for ({:?}, {:?}) at tile {}",
                    family,
                    button,
                    tile_id
                );
            }
        }
    }
}
