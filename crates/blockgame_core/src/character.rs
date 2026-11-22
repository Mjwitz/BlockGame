use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

/// 24-bit sRGB color used for character skin tones, eyes, hair, and base clothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Converts sRGB `0..=255` channels to normalized `[f32; 3]` (`0.0..=1.0`).
    #[inline]
    pub fn to_f32_array(self) -> [f32; 3] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        ]
    }

    /// Converts to RGBA `[f32; 4]` for UI quads.
    #[inline]
    pub fn to_rgba_f32(self, alpha: f32) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            alpha,
        ]
    }

    /// Multiplies each RGB channel by `factor`, clamped to `0..=255`.
    #[inline]
    pub fn scale(self, factor: f32) -> Self {
        Self {
            r: ((self.r as f32) * factor).clamp(0.0, 255.0) as u8,
            g: ((self.g as f32) * factor).clamp(0.0, 255.0) as u8,
            b: ((self.b as f32) * factor).clamp(0.0, 255.0) as u8,
        }
    }

    /// Linearly blends `self` toward `other` by `t` in `0.0..=1.0`.
    #[inline]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;
        Self {
            r: (self.r as f32 * inv + other.r as f32 * t).clamp(0.0, 255.0) as u8,
            g: (self.g as f32 * inv + other.g as f32 * t).clamp(0.0, 255.0) as u8,
            b: (self.b as f32 * inv + other.b as f32 * t).clamp(0.0, 255.0) as u8,
        }
    }
}

/// Anatomical silhouette preset that shapes proportions without changing hitbox or clothing compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum BodyFrame {
    #[default]
    Masculine = 0,
    Feminine = 1,
    Neutral = 2,
}

impl BodyFrame {
    pub const ALL: [Self; 3] = [Self::Masculine, Self::Feminine, Self::Neutral];

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Masculine,
            1 => Self::Feminine,
            2 => Self::Neutral,
            _ => Self::Neutral,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Masculine => "Masculine",
            Self::Feminine => "Feminine",
            Self::Neutral => "Neutral",
        }
    }
}

/// Eye and brow shape presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum EyeStyle {
    #[default]
    Standard = 0,
    Almond = 1,
    Wide = 2,
    Sharp = 3,
}

impl EyeStyle {
    pub const ALL: [Self; 4] = [Self::Standard, Self::Almond, Self::Wide, Self::Sharp];

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Standard,
            1 => Self::Almond,
            2 => Self::Wide,
            3 => Self::Sharp,
            _ => Self::Standard,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Almond => "Almond",
            Self::Wide => "Wide",
            Self::Sharp => "Sharp",
        }
    }
}

/// 3D sculpted hairstyle presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum HairStyle {
    #[default]
    ShortCrop = 0,
    Spiky = 1,
    SweptBangs = 2,
    SidePart = 3,
    Bob = 4,
    LongFlowing = 5,
    Ponytail = 6,
    TwinBuns = 7,
    Curly = 8,
    Bald = 9,
}

impl HairStyle {
    pub const ALL: [Self; 10] = [
        Self::ShortCrop,
        Self::Spiky,
        Self::SweptBangs,
        Self::SidePart,
        Self::Bob,
        Self::LongFlowing,
        Self::Ponytail,
        Self::TwinBuns,
        Self::Curly,
        Self::Bald,
    ];

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::ShortCrop,
            1 => Self::Spiky,
            2 => Self::SweptBangs,
            3 => Self::SidePart,
            4 => Self::Bob,
            5 => Self::LongFlowing,
            6 => Self::Ponytail,
            7 => Self::TwinBuns,
            8 => Self::Curly,
            9 => Self::Bald,
            _ => Self::ShortCrop,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::ShortCrop => "Short Crop",
            Self::Spiky => "Spiky",
            Self::SweptBangs => "Swept Bangs",
            Self::SidePart => "Side Part",
            Self::Bob => "Bob",
            Self::LongFlowing => "Long Flowing",
            Self::Ponytail => "Ponytail",
            Self::TwinBuns => "Twin Buns",
            Self::Curly => "Curly",
            Self::Bald => "Bald",
        }
    }
}

/// Optional facial hair presets (usable on any BodyFrame; inherits `hair_color`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum FacialHairStyle {
    #[default]
    None = 0,
    Stubble = 1,
    Mustache = 2,
    ShortBeard = 3,
    FullBeard = 4,
}

impl FacialHairStyle {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Stubble,
        Self::Mustache,
        Self::ShortBeard,
        Self::FullBeard,
    ];

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::None,
            1 => Self::Stubble,
            2 => Self::Mustache,
            3 => Self::ShortBeard,
            4 => Self::FullBeard,
            _ => Self::None,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Stubble => "Stubble",
            Self::Mustache => "Mustache",
            Self::ShortBeard => "Short Beard",
            Self::FullBeard => "Full Beard",
        }
    }
}

/// Cosmetic base shirt styles (primary + accent RGB colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum ShirtStyle {
    #[default]
    TShirt = 0,
    LongSleeve = 1,
    Tunic = 2,
    Hoodie = 3,
    Jacket = 4,
    Vest = 5,
}

impl ShirtStyle {
    pub const ALL: [Self; 6] = [
        Self::TShirt,
        Self::LongSleeve,
        Self::Tunic,
        Self::Hoodie,
        Self::Jacket,
        Self::Vest,
    ];

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::TShirt,
            1 => Self::LongSleeve,
            2 => Self::Tunic,
            3 => Self::Hoodie,
            4 => Self::Jacket,
            5 => Self::Vest,
            _ => Self::TShirt,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::TShirt => "T-Shirt",
            Self::LongSleeve => "Long Sleeve",
            Self::Tunic => "Tunic",
            Self::Hoodie => "Hoodie",
            Self::Jacket => "Jacket",
            Self::Vest => "Vest",
        }
    }
}

/// Cosmetic base pants styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum PantsStyle {
    #[default]
    Trousers = 0,
    Shorts = 1,
    Skirt = 2,
    Cargo = 3,
    Cuffed = 4,
}

impl PantsStyle {
    pub const ALL: [Self; 5] = [
        Self::Trousers,
        Self::Shorts,
        Self::Skirt,
        Self::Cargo,
        Self::Cuffed,
    ];

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Trousers,
            1 => Self::Shorts,
            2 => Self::Skirt,
            3 => Self::Cargo,
            4 => Self::Cuffed,
            _ => Self::Trousers,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Trousers => "Trousers",
            Self::Shorts => "Shorts",
            Self::Skirt => "Skirt",
            Self::Cargo => "Cargo",
            Self::Cuffed => "Cuffed",
        }
    }
}

/// Cosmetic base shoe styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum ShoeStyle {
    #[default]
    Boots = 0,
    Sneakers = 1,
    Sandals = 2,
    Loafers = 3,
}

impl ShoeStyle {
    pub const ALL: [Self; 4] = [Self::Boots, Self::Sneakers, Self::Sandals, Self::Loafers];

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Boots,
            1 => Self::Sneakers,
            2 => Self::Sandals,
            3 => Self::Loafers,
            _ => Self::Boots,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Boots => "Boots",
            Self::Sneakers => "Sneakers",
            Self::Sandals => "Sandals",
            Self::Loafers => "Loafers",
        }
    }
}

/// Complete visual definition of a player's avatar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CharacterAppearance {
    pub body_frame: BodyFrame,
    pub skin_tone: RgbColor,
    pub eye_style: EyeStyle,
    pub eye_color: RgbColor,
    pub hair_style: HairStyle,
    pub hair_color: RgbColor,
    pub facial_hair: FacialHairStyle,
    pub shirt_style: ShirtStyle,
    pub shirt_primary: RgbColor,
    pub shirt_accent: RgbColor,
    pub pants_style: PantsStyle,
    pub pants_color: RgbColor,
    pub shoe_style: ShoeStyle,
    pub shoe_color: RgbColor,
}

impl Default for CharacterAppearance {
    fn default() -> Self {
        starter_presets()[0].appearance
    }
}

impl CharacterAppearance {
    /// Fixed binary payload size in bytes (7 enum `u8`s + 7 RGB triplets = 28 bytes).
    pub const WIRE_SIZE: usize = 28;

    /// Validates and clamps all style fields so untrusted network data cannot produce invalid values.
    pub fn sanitized(self) -> Self {
        Self {
            body_frame: BodyFrame::from_u8(self.body_frame as u8),
            skin_tone: self.skin_tone,
            eye_style: EyeStyle::from_u8(self.eye_style as u8),
            eye_color: self.eye_color,
            hair_style: HairStyle::from_u8(self.hair_style as u8),
            hair_color: self.hair_color,
            facial_hair: FacialHairStyle::from_u8(self.facial_hair as u8),
            shirt_style: ShirtStyle::from_u8(self.shirt_style as u8),
            shirt_primary: self.shirt_primary,
            shirt_accent: self.shirt_accent,
            pants_style: PantsStyle::from_u8(self.pants_style as u8),
            pants_color: self.pants_color,
            shoe_style: ShoeStyle::from_u8(self.shoe_style as u8),
            shoe_color: self.shoe_color,
        }
    }

    /// Writes the compact 28-byte binary wire representation.
    pub fn write_binary<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let buf = [
            self.body_frame as u8,
            self.skin_tone.r,
            self.skin_tone.g,
            self.skin_tone.b,
            self.eye_style as u8,
            self.eye_color.r,
            self.eye_color.g,
            self.eye_color.b,
            self.hair_style as u8,
            self.hair_color.r,
            self.hair_color.g,
            self.hair_color.b,
            self.facial_hair as u8,
            self.shirt_style as u8,
            self.shirt_primary.r,
            self.shirt_primary.g,
            self.shirt_primary.b,
            self.shirt_accent.r,
            self.shirt_accent.g,
            self.shirt_accent.b,
            self.pants_style as u8,
            self.pants_color.r,
            self.pants_color.g,
            self.pants_color.b,
            self.shoe_style as u8,
            self.shoe_color.r,
            self.shoe_color.g,
            self.shoe_color.b,
        ];
        w.write_all(&buf)
    }

    /// Reads and sanitizes the compact 28-byte binary wire representation.
    pub fn read_binary<R: Read>(r: &mut R) -> io::Result<Self> {
        let mut b = [0u8; Self::WIRE_SIZE];
        r.read_exact(&mut b)?;
        Ok(Self {
            body_frame: BodyFrame::from_u8(b[0]),
            skin_tone: RgbColor::new(b[1], b[2], b[3]),
            eye_style: EyeStyle::from_u8(b[4]),
            eye_color: RgbColor::new(b[5], b[6], b[7]),
            hair_style: HairStyle::from_u8(b[8]),
            hair_color: RgbColor::new(b[9], b[10], b[11]),
            facial_hair: FacialHairStyle::from_u8(b[12]),
            shirt_style: ShirtStyle::from_u8(b[13]),
            shirt_primary: RgbColor::new(b[14], b[15], b[16]),
            shirt_accent: RgbColor::new(b[17], b[18], b[19]),
            pants_style: PantsStyle::from_u8(b[20]),
            pants_color: RgbColor::new(b[21], b[22], b[23]),
            shoe_style: ShoeStyle::from_u8(b[24]),
            shoe_color: RgbColor::new(b[25], b[26], b[27]),
        })
    }

    /// Generates a cohesive randomized CharacterAppearance from a 64-bit seed.
    pub fn randomized(seed: u64) -> Self {
        let mut s = seed ^ 0x9E37_79B9_7F4A_7C15;
        let mut next = || -> usize {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s as usize) & 0x7FFF_FFFF
        };

        let body_frame = BodyFrame::ALL[next() % BodyFrame::ALL.len()];
        let skin_tone = SKIN_TONE_SWATCHES[next() % SKIN_TONE_SWATCHES.len()];
        let eye_style = EyeStyle::ALL[next() % EyeStyle::ALL.len()];
        let eye_color = EYE_COLOR_SWATCHES[next() % EYE_COLOR_SWATCHES.len()];
        let hair_style = HairStyle::ALL[next() % HairStyle::ALL.len()];
        let hair_color = HAIR_COLOR_SWATCHES[next() % HAIR_COLOR_SWATCHES.len()];
        // Weight facial hair toward None (~60%) so random characters feel varied and natural
        let facial_roll = next() % 10;
        let facial_hair = if facial_roll < 6 {
            FacialHairStyle::None
        } else {
            FacialHairStyle::ALL[1 + (facial_roll - 6) % (FacialHairStyle::ALL.len() - 1)]
        };
        let shirt_style = ShirtStyle::ALL[next() % ShirtStyle::ALL.len()];
        let shirt_primary = CLOTHING_COLOR_SWATCHES[next() % CLOTHING_COLOR_SWATCHES.len()];
        let shirt_accent = CLOTHING_COLOR_SWATCHES[next() % CLOTHING_COLOR_SWATCHES.len()];
        let pants_style = PantsStyle::ALL[next() % PantsStyle::ALL.len()];
        let pants_color = CLOTHING_COLOR_SWATCHES[next() % CLOTHING_COLOR_SWATCHES.len()];
        let shoe_style = ShoeStyle::ALL[next() % ShoeStyle::ALL.len()];
        let shoe_color = CLOTHING_COLOR_SWATCHES[next() % CLOTHING_COLOR_SWATCHES.len()];

        Self {
            body_frame,
            skin_tone,
            eye_style,
            eye_color,
            hair_style,
            hair_color,
            facial_hair,
            shirt_style,
            shirt_primary,
            shirt_accent,
            pants_style,
            pants_color,
            shoe_style,
            shoe_color,
        }
    }
}

/// Curated skin tone swatches (natural tones + fantasy tones).
pub const SKIN_TONE_SWATCHES: [RgbColor; 12] = [
    RgbColor::new(247, 224, 206), // Porcelain
    RgbColor::new(236, 198, 170), // Fair
    RgbColor::new(222, 178, 142), // Light Warm
    RgbColor::new(204, 154, 114), // Olive / Tan
    RgbColor::new(178, 126, 88),  // Golden Brown
    RgbColor::new(146, 98, 66),   // Rich Brown
    RgbColor::new(112, 72, 48),   // Deep Brown
    RgbColor::new(78, 50, 36),    // Espresso
    RgbColor::new(142, 194, 152), // Sage (Fantasy)
    RgbColor::new(138, 178, 214), // Frost Blue (Fantasy)
    RgbColor::new(176, 148, 206), // Lavender (Fantasy)
    RgbColor::new(214, 142, 138), // Coral (Fantasy)
];

/// Curated hair color swatches (natural + vibrant).
pub const HAIR_COLOR_SWATCHES: [RgbColor; 12] = [
    RgbColor::new(34, 30, 32),    // Raven Black
    RgbColor::new(72, 48, 34),    // Dark Brown
    RgbColor::new(122, 78, 46),   // Chestnut
    RgbColor::new(176, 122, 68),  // Light Brown
    RgbColor::new(224, 188, 108), // Blonde
    RgbColor::new(186, 78, 42),   // Copper Red
    RgbColor::new(212, 216, 224), // Silver / White
    RgbColor::new(124, 128, 136), // Slate Gray
    RgbColor::new(62, 132, 198),  // Cobalt Blue
    RgbColor::new(56, 158, 124),  // Emerald Teal
    RgbColor::new(152, 82, 188),  // Violet
    RgbColor::new(214, 92, 134),  // Rose Pink
];

/// Curated eye iris color swatches.
pub const EYE_COLOR_SWATCHES: [RgbColor; 10] = [
    RgbColor::new(84, 54, 34),    // Warm Brown
    RgbColor::new(42, 34, 30),    // Deep Obsidian
    RgbColor::new(66, 128, 196),  // Sky Blue
    RgbColor::new(48, 146, 92),   // Forest Green
    RgbColor::new(132, 118, 54),  // Hazel
    RgbColor::new(112, 128, 142), // Steel Gray
    RgbColor::new(196, 144, 48),  // Amber Gold
    RgbColor::new(146, 82, 196),  // Amethyst
    RgbColor::new(196, 62, 68),   // Crimson
    RgbColor::new(64, 188, 194),  // Cyan
];

/// Curated base clothing color swatches.
pub const CLOTHING_COLOR_SWATCHES: [RgbColor; 14] = [
    RgbColor::new(228, 228, 232), // Off-White
    RgbColor::new(148, 154, 164), // Heather Gray
    RgbColor::new(46, 50, 58),    // Charcoal
    RgbColor::new(188, 58, 58),   // Crimson
    RgbColor::new(214, 122, 52),  // Rust Orange
    RgbColor::new(218, 178, 62),  // Ochre Gold
    RgbColor::new(66, 136, 78),   // Forest Green
    RgbColor::new(54, 138, 142),  // Deep Teal
    RgbColor::new(58, 102, 176),  // Cobalt Blue
    RgbColor::new(52, 64, 108),   // Navy Denim
    RgbColor::new(118, 74, 162),  // Plum
    RgbColor::new(188, 92, 134),  // Berry Rose
    RgbColor::new(132, 92, 62),   // Leather Brown
    RgbColor::new(184, 164, 132), // Khaki Sand
];

/// A named Character Appearance preset (either a read-only Starter Preset or a saved custom preset).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterPreset {
    pub name: String,
    pub appearance: CharacterAppearance,
    #[serde(default)]
    pub is_starter: bool,
}

/// Built-in read-only Starter Presets spanning all three Body Frames.
pub fn starter_presets() -> Vec<CharacterPreset> {
    vec![
        CharacterPreset {
            name: "Wayfarer".to_string(),
            is_starter: true,
            appearance: CharacterAppearance {
                body_frame: BodyFrame::Neutral,
                skin_tone: RgbColor::new(222, 178, 142),
                eye_style: EyeStyle::Standard,
                eye_color: RgbColor::new(66, 128, 196),
                hair_style: HairStyle::SweptBangs,
                hair_color: RgbColor::new(122, 78, 46),
                facial_hair: FacialHairStyle::None,
                shirt_style: ShirtStyle::Tunic,
                shirt_primary: RgbColor::new(54, 138, 142),
                shirt_accent: RgbColor::new(218, 178, 62),
                pants_style: PantsStyle::Trousers,
                pants_color: RgbColor::new(52, 64, 108),
                shoe_style: ShoeStyle::Boots,
                shoe_color: RgbColor::new(112, 72, 48),
            },
        },
        CharacterPreset {
            name: "Forest Ranger".to_string(),
            is_starter: true,
            appearance: CharacterAppearance {
                body_frame: BodyFrame::Feminine,
                skin_tone: RgbColor::new(204, 154, 114),
                eye_style: EyeStyle::Almond,
                eye_color: RgbColor::new(48, 146, 92),
                hair_style: HairStyle::Ponytail,
                hair_color: RgbColor::new(186, 78, 42),
                facial_hair: FacialHairStyle::None,
                shirt_style: ShirtStyle::Vest,
                shirt_primary: RgbColor::new(66, 136, 78),
                shirt_accent: RgbColor::new(228, 228, 232),
                pants_style: PantsStyle::Cargo,
                pants_color: RgbColor::new(132, 92, 62),
                shoe_style: ShoeStyle::Boots,
                shoe_color: RgbColor::new(78, 50, 36),
            },
        },
        CharacterPreset {
            name: "Gold Miner".to_string(),
            is_starter: true,
            appearance: CharacterAppearance {
                body_frame: BodyFrame::Masculine,
                skin_tone: RgbColor::new(178, 126, 88),
                eye_style: EyeStyle::Sharp,
                eye_color: RgbColor::new(84, 54, 34),
                hair_style: HairStyle::ShortCrop,
                hair_color: RgbColor::new(72, 48, 34),
                facial_hair: FacialHairStyle::ShortBeard,
                shirt_style: ShirtStyle::Jacket,
                shirt_primary: RgbColor::new(188, 58, 58),
                shirt_accent: RgbColor::new(218, 178, 62),
                pants_style: PantsStyle::Cuffed,
                pants_color: RgbColor::new(46, 50, 58),
                shoe_style: ShoeStyle::Boots,
                shoe_color: RgbColor::new(92, 62, 42),
            },
        },
        CharacterPreset {
            name: "Stargazer".to_string(),
            is_starter: true,
            appearance: CharacterAppearance {
                body_frame: BodyFrame::Feminine,
                skin_tone: RgbColor::new(146, 98, 66),
                eye_style: EyeStyle::Wide,
                eye_color: RgbColor::new(196, 144, 48),
                hair_style: HairStyle::TwinBuns,
                hair_color: RgbColor::new(34, 30, 32),
                facial_hair: FacialHairStyle::None,
                shirt_style: ShirtStyle::Hoodie,
                shirt_primary: RgbColor::new(118, 74, 162),
                shirt_accent: RgbColor::new(228, 228, 232),
                pants_style: PantsStyle::Skirt,
                pants_color: RgbColor::new(46, 50, 58),
                shoe_style: ShoeStyle::Sneakers,
                shoe_color: RgbColor::new(228, 228, 232),
            },
        },
        CharacterPreset {
            name: "Grove Warden".to_string(),
            is_starter: true,
            appearance: CharacterAppearance {
                body_frame: BodyFrame::Masculine,
                skin_tone: RgbColor::new(236, 198, 170),
                eye_style: EyeStyle::Standard,
                eye_color: RgbColor::new(112, 128, 142),
                hair_style: HairStyle::Spiky,
                hair_color: RgbColor::new(224, 188, 108),
                facial_hair: FacialHairStyle::Stubble,
                shirt_style: ShirtStyle::LongSleeve,
                shirt_primary: RgbColor::new(58, 102, 176),
                shirt_accent: RgbColor::new(184, 164, 132),
                pants_style: PantsStyle::Trousers,
                pants_color: RgbColor::new(148, 154, 164),
                shoe_style: ShoeStyle::Sneakers,
                shoe_color: RgbColor::new(52, 64, 108),
            },
        },
        CharacterPreset {
            name: "Artisan".to_string(),
            is_starter: true,
            appearance: CharacterAppearance {
                body_frame: BodyFrame::Neutral,
                skin_tone: RgbColor::new(112, 72, 48),
                eye_style: EyeStyle::Almond,
                eye_color: RgbColor::new(64, 188, 194),
                hair_style: HairStyle::Curly,
                hair_color: RgbColor::new(212, 216, 224),
                facial_hair: FacialHairStyle::None,
                shirt_style: ShirtStyle::TShirt,
                shirt_primary: RgbColor::new(214, 122, 52),
                shirt_accent: RgbColor::new(46, 50, 58),
                pants_style: PantsStyle::Shorts,
                pants_color: RgbColor::new(184, 164, 132),
                shoe_style: ShoeStyle::Sandals,
                shoe_color: RgbColor::new(132, 92, 62),
            },
        },
    ]
}

/// Client-owned global character profile holding the active appearance and saved custom presets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterProfile {
    pub active_appearance: CharacterAppearance,
    #[serde(default)]
    pub selected_preset_name: Option<String>,
    #[serde(default)]
    pub custom_presets: Vec<CharacterPreset>,
}

impl Default for CharacterProfile {
    fn default() -> Self {
        let starters = starter_presets();
        Self {
            active_appearance: starters[0].appearance,
            selected_preset_name: Some(starters[0].name.clone()),
            custom_presets: Vec::new(),
        }
    }
}

impl CharacterProfile {
    pub const MAX_CUSTOM_PRESETS: usize = 24;

    /// Returns all Starter Presets followed by saved custom presets.
    pub fn all_presets(&self) -> Vec<CharacterPreset> {
        let mut list = starter_presets();
        for p in &self.custom_presets {
            list.push(CharacterPreset {
                name: p.name.clone(),
                appearance: p.appearance.sanitized(),
                is_starter: false,
            });
        }
        list
    }

    /// Saves or overwrites a named custom preset (never overwrites a read-only Starter Preset).
    /// Returns the actual saved preset name.
    pub fn save_custom_preset(&mut self, raw_name: &str, appearance: CharacterAppearance) -> String {
        let trimmed = raw_name.trim();
        let base_name = if trimmed.is_empty() {
            format!("Custom {}", self.custom_presets.len() + 1)
        } else {
            trimmed.chars().take(24).collect::<String>()
        };

        // Ensure we never collide with a read-only starter preset name
        let starters = starter_presets();
        let final_name = if starters
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(&base_name))
        {
            format!("{} (Custom)", base_name)
        } else {
            base_name
        };

        let clean_appearance = appearance.sanitized();
        if let Some(existing) = self
            .custom_presets
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(&final_name))
        {
            existing.name = final_name.clone();
            existing.appearance = clean_appearance;
            existing.is_starter = false;
        } else {
            if self.custom_presets.len() >= Self::MAX_CUSTOM_PRESETS {
                self.custom_presets.remove(0);
            }
            self.custom_presets.push(CharacterPreset {
                name: final_name.clone(),
                appearance: clean_appearance,
                is_starter: false,
            });
        }

        self.active_appearance = clean_appearance;
        self.selected_preset_name = Some(final_name.clone());
        final_name
    }

    /// Deletes a custom preset by name. Returns true if removed.
    pub fn delete_custom_preset(&mut self, name: &str) -> bool {
        let before = self.custom_presets.len();
        self.custom_presets
            .retain(|p| !p.name.eq_ignore_ascii_case(name));
        let removed = self.custom_presets.len() != before;
        if removed
            && self
                .selected_preset_name
                .as_deref()
                .map(|s| s.eq_ignore_ascii_case(name))
                .unwrap_or(false)
        {
            self.selected_preset_name = None;
        }
        removed
    }

    /// Loads the profile from a YAML file, falling back to `Default` if missing or invalid.
    pub fn load(path: &Path) -> Self {
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };
        match serde_yaml::from_str::<Self>(&contents) {
            Ok(mut profile) => {
                profile.active_appearance = profile.active_appearance.sanitized();
                for p in &mut profile.custom_presets {
                    p.appearance = p.appearance.sanitized();
                    p.is_starter = false;
                }
                profile
            }
            Err(_) => Self::default(),
        }
    }

    /// Saves the profile as YAML to disk.
    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(yaml) = serde_yaml::to_string(self) {
            let _ = fs::write(path, yaml);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_round_trip_and_sanitization() {
        let appearance = starter_presets()[2].appearance;
        let mut buf = Vec::new();
        appearance.write_binary(&mut buf).unwrap();
        assert_eq!(buf.len(), CharacterAppearance::WIRE_SIZE);

        let decoded = CharacterAppearance::read_binary(&mut buf.as_slice()).unwrap();
        assert_eq!(decoded, appearance);

        // Corrupted enum IDs clamp cleanly to valid defaults
        let mut corrupt = buf.clone();
        corrupt[0] = 99;  // BodyFrame
        corrupt[4] = 250; // EyeStyle
        corrupt[8] = 123; // HairStyle
        corrupt[12] = 77; // FacialHairStyle
        corrupt[13] = 88; // ShirtStyle
        corrupt[20] = 66; // PantsStyle
        corrupt[24] = 55; // ShoeStyle
        let sanitized = CharacterAppearance::read_binary(&mut corrupt.as_slice()).unwrap();
        assert_eq!(sanitized.body_frame, BodyFrame::Neutral);
        assert_eq!(sanitized.eye_style, EyeStyle::Standard);
        assert_eq!(sanitized.hair_style, HairStyle::ShortCrop);
        assert_eq!(sanitized.facial_hair, FacialHairStyle::None);
        assert_eq!(sanitized.shirt_style, ShirtStyle::TShirt);
        assert_eq!(sanitized.pants_style, PantsStyle::Trousers);
        assert_eq!(sanitized.shoe_style, ShoeStyle::Boots);
    }

    #[test]
    fn test_starter_presets_span_all_body_frames() {
        let starters = starter_presets();
        assert!(starters.len() >= 4);
        assert!(starters.iter().all(|p| p.is_starter));
        for frame in BodyFrame::ALL {
            assert!(
                starters.iter().any(|p| p.appearance.body_frame == frame),
                "missing starter preset for {:?}",
                frame
            );
        }
    }

    #[test]
    fn test_profile_yaml_save_load_and_custom_presets() {
        let mut profile = CharacterProfile::default();
        let custom_app = CharacterAppearance::randomized(42);
        let saved_name = profile.save_custom_preset("Wayfarer", custom_app);
        // Should not overwrite starter preset name
        assert_eq!(saved_name, "Wayfarer (Custom)");
        assert_eq!(profile.custom_presets.len(), 1);

        let temp = std::env::temp_dir().join("blockgame_test_character_profile.yaml");
        profile.save(&temp);
        let loaded = CharacterProfile::load(&temp);
        let _ = fs::remove_file(&temp);

        assert_eq!(loaded.active_appearance, custom_app);
        assert_eq!(loaded.custom_presets.len(), 1);
        assert_eq!(loaded.custom_presets[0].name, "Wayfarer (Custom)");
    }
}
