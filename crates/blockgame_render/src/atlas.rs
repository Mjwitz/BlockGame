use image::{Rgba, RgbaImage};

/// Size of each square tile on the atlas in pixels.
/// Roots & Ridges uses 32x32 tiles for its handcrafted retro pixel art style.
pub const TILE_SIZE: u32 = 32;

/// Number of tiles horizontally and vertically across the atlas grid.
pub const ATLAS_TILES_PER_ROW: u32 = 16;

/// Total width and height of the texture atlas in pixels.
/// The atlas grid is 16 tiles wide by 64 tiles high (512x2048), providing 1024 total tile slots.
pub const ATLAS_WIDTH: u32 = TILE_SIZE * ATLAS_TILES_PER_ROW; // 512
pub const ATLAS_HEIGHT: u32 = TILE_SIZE * ATLAS_TILES_PER_ROW * 4; // 2048

/// Embedded handcrafted Roots & Ridges 32x retro pixel art texture atlas.
const ROOTS_ATLAS_PNG: &[u8] = include_bytes!("../assets/roots_atlas_512.png");

pub struct TextureAtlas {
    pub image: RgbaImage,
}

impl Default for TextureAtlas {
    fn default() -> Self {
        Self::generate_default()
    }
}

impl TextureAtlas {
    pub fn new() -> Self {
        Self {
            image: RgbaImage::new(ATLAS_WIDTH, ATLAS_HEIGHT),
        }
    }

    /// Computes the UV bounding box `[u_min, v_min, u_max, v_max]` for a given tile index.
    /// Insets slightly by a fractional pixel to prevent bleeding across neighboring tiles.
    pub fn get_uv_bounds(tile_id: u16) -> [f32; 4] {
        let tile_x = (tile_id as u32 % ATLAS_TILES_PER_ROW) as f32;
        let tile_y = (tile_id as u32 / ATLAS_TILES_PER_ROW) as f32;

        let pixel_u = 1.0 / ATLAS_WIDTH as f32;
        let pixel_v = 1.0 / ATLAS_HEIGHT as f32;
        let eps = 0.00025; // Clamped subpixel offset to prevent texture bleed

        let u_min = (tile_x * TILE_SIZE as f32) * pixel_u + eps;
        let v_min = (tile_y * TILE_SIZE as f32) * pixel_v + eps;
        let u_max = ((tile_x + 1.0) * TILE_SIZE as f32) * pixel_u - eps;
        let v_max = ((tile_y + 1.0) * TILE_SIZE as f32) * pixel_v - eps;

        [u_min, v_min, u_max, v_max]
    }

    /// Loads the handcrafted Roots & Ridges 32x retro pixel art texture atlas containing
    /// blocks, items, tools, entities, ores, crops, GUI components, and fauna.
    pub fn generate_default() -> Self {
        let dynamic_img = image::load_from_memory(ROOTS_ATLAS_PNG).expect(
            "Failed to load embedded Roots & Ridges 32x texture atlas (assets/roots_atlas_512.png)",
        );
        let src_image = dynamic_img.to_rgba8();
        let mut atlas = Self::new();
        image::imageops::overlay(&mut atlas.image, &src_image, 0, 0);

        // Paint procedural tiles for Bucket (242), Water Bucket (243), and Moist Farmland (244)
        paint_bucket_sprite(&mut atlas, 242, false);
        paint_bucket_sprite(&mut atlas, 243, true);
        paint_moist_farmland_tile(&mut atlas, 244, 150);

        // Paint Sword sprites (256 to 261)
        paint_sword_sprite(&mut atlas, 256, [180, 130, 80], [150, 105, 60], [115, 75, 40], [120, 80, 45]); // Wooden Sword
        paint_sword_sprite(&mut atlas, 257, [185, 185, 190], [140, 140, 145], [100, 100, 105], [110, 110, 115]); // Stone Sword
        paint_sword_sprite(&mut atlas, 258, [235, 145, 105], [200, 100, 70], [150, 70, 45], [175, 85, 55]); // Copper Sword
        paint_sword_sprite(&mut atlas, 259, [240, 242, 245], [195, 200, 208], [145, 150, 160], [160, 165, 175]); // Iron Sword
        paint_sword_sprite(&mut atlas, 260, [160, 250, 240], [85, 220, 210], [45, 170, 160], [50, 180, 170]); // Diamond Sword
        paint_sword_sprite(&mut atlas, 261, [255, 248, 120], [245, 215, 50], [190, 155, 25], [210, 170, 30]); // Gold Sword

        // Copy and tint Copper Tools (262 to 265) from Iron tool icons (Pickaxe: 20, Axe: 142, Shovel: 146, Hoe: 156)
        copy_and_tint_tool(&mut atlas, 20, 262, [235, 145, 105], [200, 100, 70], [150, 70, 45]); // Copper Pickaxe
        copy_and_tint_tool(&mut atlas, 142, 263, [235, 145, 105], [200, 100, 70], [150, 70, 45]); // Copper Axe
        copy_and_tint_tool(&mut atlas, 146, 264, [235, 145, 105], [200, 100, 70], [150, 70, 45]); // Copper Shovel
        copy_and_tint_tool(&mut atlas, 156, 265, [235, 145, 105], [200, 100, 70], [150, 70, 45]); // Copper Hoe

        // Copy and tint Gold Tools (266 to 269) from Iron tool icons (Pickaxe: 20, Axe: 142, Shovel: 146, Hoe: 156)
        copy_and_tint_tool(&mut atlas, 20, 266, [255, 248, 120], [245, 215, 50], [190, 155, 25]); // Gold Pickaxe
        copy_and_tint_tool(&mut atlas, 142, 267, [255, 248, 120], [245, 215, 50], [190, 155, 25]); // Gold Axe
        copy_and_tint_tool(&mut atlas, 146, 268, [255, 248, 120], [245, 215, 50], [190, 155, 25]); // Gold Shovel
        copy_and_tint_tool(&mut atlas, 156, 269, [255, 248, 120], [245, 215, 50], [190, 155, 25]); // Gold Hoe

        // Paint 4 distinct 3D Player Character Skins at tiles 272..287 (4 tiles per skin: HeadFront, HeadSide, Torso, Legs)
        paint_player_skin_tiles(&mut atlas);

        // Paint decorative block & item tiles (288..377)
        paint_decorative_tiles(&mut atlas);

        // Paint fishing rods, lines, lures, bait, fish, and crafted dishes (384..=409)
        paint_fishing_tiles(&mut atlas);
        paint_power_tiles(&mut atlas);
        paint_temple_and_boss_tiles(&mut atlas);
        paint_farming_and_cooking_tiles(&mut atlas);
        paint_village_item_tiles(&mut atlas);

        atlas
    }

    /// Paints or overrides a tile procedurally with a custom per-pixel closure.
    pub fn paint_tile<F>(&mut self, tile_id: u16, mut pixel_fn: F)
    where
        F: FnMut(u32, u32) -> Rgba<u8>,
    {
        let tile_x = (tile_id as u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let tile_y = (tile_id as u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;

        for py in 0..TILE_SIZE {
            for px in 0..TILE_SIZE {
                let color = pixel_fn(px, py);
                self.image.put_pixel(tile_x + px, tile_y + py, color);
            }
        }
    }

    /// Blits a 32x32 image (or sub-image) into the specified tile slot.
    pub fn blit_tile(&mut self, tile_id: u16, tile_image: &RgbaImage) {
        let tile_x = (tile_id as u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let tile_y = (tile_id as u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;

        for py in 0..TILE_SIZE.min(tile_image.height()) {
            for px in 0..TILE_SIZE.min(tile_image.width()) {
                self.image
                    .put_pixel(tile_x + px, tile_y + py, *tile_image.get_pixel(px, py));
            }
        }
    }
}

fn paint_bucket_sprite(atlas: &mut TextureAtlas, tile_id: u16, with_water: bool) {
    atlas.paint_tile(tile_id, |px, py| {
        // Outline & iron colors
        let outline = Rgba([36, 38, 42, 255]);
        let iron_light = Rgba([228, 232, 238, 255]);
        let iron_mid = Rgba([180, 185, 192, 255]);
        let iron_shadow = Rgba([128, 133, 142, 255]);
        let iron_dark = Rgba([82, 86, 94, 255]);

        let cavity_dark = Rgba([52, 54, 60, 255]);
        let cavity_mid = Rgba([70, 74, 82, 255]);
        let cavity_light = Rgba([92, 96, 106, 255]);

        let water_highlight = Rgba([130, 205, 255, 255]);
        let water_top = Rgba([70, 160, 248, 255]);
        let water_mid = Rgba([42, 118, 222, 255]);
        let water_deep = Rgba([26, 82, 182, 255]);

        // Handle ear notches at top:
        if (py == 7 || py == 8) && (px == 7 || px == 8 || px == 23 || px == 24) {
            if px == 8 || px == 23 {
                return outline;
            } else {
                return iron_mid;
            }
        }

        if py < 9 || py > 26 {
            return Rgba([0, 0, 0, 0]);
        }

        // Left and right bounds per row:
        let (left_out, left_in, right_in, right_out) = match py {
            9 => (7, 9, 22, 24),
            10 => (6, 8, 23, 25),
            11..=13 => (6, 8, 23, 25),
            14..=17 => (7, 9, 22, 24),
            18..=21 => (8, 10, 21, 23),
            22..=24 => (9, 11, 20, 22),
            25 => (10, 12, 19, 21),
            26 => (11, 13, 18, 20),
            _ => (0, 0, 0, 0),
        };

        if px < left_out || px > right_out {
            return Rgba([0, 0, 0, 0]);
        }

        // Outer outline
        if px == left_out || px == right_out || py == 9 || py == 26 {
            return outline;
        }

        // Bottom rim line
        if py == 25 && (px == left_out + 1 || px == right_out - 1) {
            return outline;
        }

        // Metal rim / sides
        if px < left_in {
            if px == left_out + 1 {
                return iron_light;
            } else {
                return iron_mid;
            }
        }
        if px > right_in {
            if px == right_out - 1 {
                return iron_shadow;
            } else {
                return iron_dark;
            }
        }

        // Bottom base metal
        if py >= 25 {
            return if px < 16 { iron_mid } else { iron_shadow };
        }

        // Interior cavity
        if with_water {
            // Water surface line at y=11..=12
            if py == 11 {
                if px % 3 == 0 {
                    water_highlight
                } else {
                    water_top
                }
            } else if py == 12 {
                if (px + 1) % 4 == 0 {
                    water_highlight
                } else {
                    water_mid
                }
            } else if py <= 18 {
                if (px + py) % 5 == 0 {
                    water_top
                } else {
                    water_mid
                }
            } else {
                water_deep
            }
        } else {
            // Empty bucket metal interior
            if py <= 12 {
                cavity_dark
            } else if py <= 18 {
                if px < 14 {
                    cavity_dark
                } else if px < 20 {
                    cavity_mid
                } else {
                    cavity_light
                }
            } else {
                cavity_mid
            }
        }
    });
}

fn paint_moist_farmland_tile(atlas: &mut TextureAtlas, dst_tile: u16, src_tile: u16) {
    let src_base_x = (src_tile as u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
    let src_base_y = (src_tile as u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
    let mut pixels = [[Rgba([0, 0, 0, 255]); TILE_SIZE as usize]; TILE_SIZE as usize];
    for py in 0..TILE_SIZE {
        for px in 0..TILE_SIZE {
            let orig = *atlas.image.get_pixel(src_base_x + px, src_base_y + py);
            // Darken and enrich color to represent moist, saturated soil
            let r = ((orig[0] as f32) * 0.48) as u8;
            let g = ((orig[1] as f32) * 0.42) as u8;
            let b = ((orig[2] as f32) * 0.36) as u8;
            pixels[py as usize][px as usize] = Rgba([r, g, b, orig[3]]);
        }
    }
    atlas.paint_tile(dst_tile, |px, py| pixels[py as usize][px as usize]);
}

fn copy_and_tint_tool(
    atlas: &mut TextureAtlas,
    src_tile: u16,
    dst_tile: u16,
    light_col: [u8; 3],
    mid_col: [u8; 3],
    dark_col: [u8; 3],
) {
    let src_x = (src_tile as u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
    let src_y = (src_tile as u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
    let mut pixels = [[Rgba([0u8, 0, 0, 0]); TILE_SIZE as usize]; TILE_SIZE as usize];

    for py in 0..TILE_SIZE {
        for px in 0..TILE_SIZE {
            let p = *atlas.image.get_pixel(src_x + px, src_y + py);
            if p[3] > 0 {
                // Check if pixel belongs to wooden stick handle
                let r = p[0] as i16;
                let g = p[1] as i16;
                let b = p[2] as i16;
                let is_handle = r > b + 25 && g > b + 5 && r < 190 && g < 140;
                let is_dark_outline = r < 45 && g < 45 && b < 45;
                if is_handle || is_dark_outline {
                    pixels[py as usize][px as usize] = p;
                } else {
                    let lum = (p[0] as f32 * 0.30 + p[1] as f32 * 0.59 + p[2] as f32 * 0.11) / 255.0;
                    let (r, g, b) = if lum > 0.70 {
                        (light_col[0], light_col[1], light_col[2])
                    } else if lum > 0.40 {
                        (mid_col[0], mid_col[1], mid_col[2])
                    } else {
                        (dark_col[0], dark_col[1], dark_col[2])
                    };
                    pixels[py as usize][px as usize] = Rgba([r, g, b, p[3]]);
                }
            }
        }
    }

    atlas.paint_tile(dst_tile, |px, py| pixels[py as usize][px as usize]);
}

fn paint_sword_sprite(
    atlas: &mut TextureAtlas,
    tile_id: u16,
    blade_light: [u8; 3],
    blade_mid: [u8; 3],
    blade_dark: [u8; 3],
    guard_col: [u8; 3],
) {
    atlas.paint_tile(tile_id, |px, py| {
        let ix = px as i32;
        let iy = py as i32;
        let outline = Rgba([30, 30, 34, 255]);
        let handle_wood = Rgba([136, 88, 48, 255]);
        let handle_shadow = Rgba([92, 56, 30, 255]);
        let guard = Rgba([guard_col[0], guard_col[1], guard_col[2], 255]);
        let light = Rgba([blade_light[0], blade_light[1], blade_light[2], 255]);
        let mid = Rgba([blade_mid[0], blade_mid[1], blade_mid[2], 255]);
        let dark = Rgba([blade_dark[0], blade_dark[1], blade_dark[2], 255]);

        // Pommel
        if (ix == 5 && iy == 26) || (ix == 6 && iy == 27) {
            return guard;
        }
        if (ix == 4 && (iy == 26 || iy == 27))
            || (ix == 5 && (iy == 25 || iy == 27))
            || (ix == 6 && (iy == 26 || iy == 28))
            || (ix == 7 && iy == 27)
        {
            return outline;
        }

        // Handle grip (diagonal length 4)
        for d in 0..4 {
            let hx = 7 + d;
            let hy = 24 - d;
            if ix == hx && iy == hy {
                return handle_wood;
            }
            if ix == hx - 1 && iy == hy {
                return handle_shadow;
            }
        }
        // Handle outlines
        if (ix == 6 && iy == 24) || (ix == 7 && iy == 25)
            || (ix == 7 && iy == 23) || (ix == 8 && iy == 24)
            || (ix == 8 && iy == 22) || (ix == 9 && iy == 23)
            || (ix == 9 && iy == 21) || (ix == 10 && iy == 22)
        {
            return outline;
        }

        // Crossguard
        let guard_coords = [
            (9, 18), (10, 19), (11, 20), (12, 21), (13, 22),
            (10, 18), (11, 19), (12, 20), (13, 21),
            (8, 19), (14, 21),
        ];
        if guard_coords.contains(&(ix, iy)) {
            return guard;
        }
        let guard_outline = [
            (8, 18), (7, 19), (8, 20), (9, 21),
            (9, 17), (10, 17), (11, 18), (12, 19), (13, 20), (14, 20), (15, 21), (14, 22), (13, 23),
            (12, 23), (11, 22),
        ];
        if guard_outline.contains(&(ix, iy)) {
            return outline;
        }

        // Blade: extends from (12, 19) to (25, 6), tip at (26, 5)
        for step in 0..15 {
            let bx = 12 + step;
            let by = 19 - step;
            if by < 5 || bx > 26 {
                continue;
            }
            if ix == bx && iy == by {
                return mid; // center ridge
            }
            if ix == bx - 1 && iy == by {
                return dark; // bottom-left blade side
            }
            if ix == bx && iy == by - 1 {
                return light; // top-right blade highlight
            }
            // Blade outer edges
            if ix == bx - 2 && iy == by {
                return outline;
            }
            if ix == bx && iy == by - 2 {
                return outline;
            }
        }

        // Tip at (26, 5)
        if ix == 26 && iy == 5 {
            return light;
        }
        if (ix == 27 && iy == 5) || (ix == 26 && iy == 4) || (ix == 27 && iy == 4) || (ix == 25 && iy == 4) || (ix == 27 && iy == 6) {
            return outline;
        }

        Rgba([0, 0, 0, 0])
    });
}

fn paint_player_skin_tiles(atlas: &mut TextureAtlas) {
    struct SkinPalette {
        hair: [u8; 3],
        hair_dark: [u8; 3],
        skin: [u8; 3],
        skin_shadow: [u8; 3],
        eye_iris: [u8; 3],
        shirt: [u8; 3],
        shirt_dark: [u8; 3],
        belt: [u8; 3],
        pants: [u8; 3],
        pants_dark: [u8; 3],
        boots: [u8; 3],
    }

    let palettes = [
        // 0: Explorer Cyan
        SkinPalette {
            hair: [82, 52, 32],
            hair_dark: [58, 36, 22],
            skin: [228, 178, 142],
            skin_shadow: [198, 146, 112],
            eye_iris: [45, 115, 210],
            shirt: [38, 172, 185],
            shirt_dark: [26, 132, 146],
            belt: [75, 52, 34],
            pants: [46, 58, 132],
            pants_dark: [32, 42, 102],
            boots: [54, 52, 58],
        },
        // 1: Crimson Knight
        SkinPalette {
            hair: [44, 34, 30],
            hair_dark: [28, 22, 20],
            skin: [216, 166, 130],
            skin_shadow: [186, 136, 102],
            eye_iris: [215, 145, 35],
            shirt: [188, 42, 48],
            shirt_dark: [142, 28, 34],
            belt: [218, 178, 52],
            pants: [62, 66, 78],
            pants_dark: [44, 48, 58],
            boots: [86, 92, 104],
        },
        // 2: Forest Ranger
        SkinPalette {
            hair: [138, 62, 34],
            hair_dark: [102, 44, 22],
            skin: [236, 188, 156],
            skin_shadow: [204, 156, 124],
            eye_iris: [42, 160, 75],
            shirt: [46, 134, 68],
            shirt_dark: [32, 98, 48],
            belt: [108, 72, 38],
            pants: [92, 64, 42],
            pants_dark: [68, 46, 30],
            boots: [48, 42, 34],
        },
        // 3: Gold Miner
        SkinPalette {
            hair: [210, 168, 62],
            hair_dark: [168, 128, 40],
            skin: [220, 168, 135],
            skin_shadow: [190, 138, 106],
            eye_iris: [40, 90, 175],
            shirt: [218, 165, 38],
            shirt_dark: [176, 128, 24],
            belt: [88, 58, 32],
            pants: [64, 88, 124],
            pants_dark: [46, 66, 96],
            boots: [72, 48, 30],
        },
    ];

    for (idx, p) in palettes.iter().enumerate() {
        let base = 272 + (idx as u16) * 4;

        // Tile base + 0: Head Front (Face with hair bangs, eyes, nose, mouth)
        atlas.paint_tile(base, |px, py| {
            let noise = (((px * 7 + py * 13) % 5) as i16) - 2;
            let clamp = |v: u8, d: i16| (v as i16 + d).clamp(0, 255) as u8;

            // Hair top and bangs
            if py < 8 || (py < 11 && (px < 5 || px > 26 || (px >= 12 && px <= 19 && py < 9))) {
                let col = if (px + py) % 3 == 0 { p.hair_dark } else { p.hair };
                return Rgba([clamp(col[0], noise), clamp(col[1], noise), clamp(col[2], noise), 255]);
            }
            // Eyebrows
            if (py == 12 || py == 13) && ((px >= 6 && px <= 12) || (px >= 19 && px <= 25)) {
                return Rgba([p.hair_dark[0], p.hair_dark[1], p.hair_dark[2], 255]);
            }
            // Eyes (y = 14..=17)
            if py >= 14 && py <= 17 {
                // Left eye: x = 6..=11, Right eye: x = 20..=25
                if (px >= 6 && px <= 7) || (px >= 24 && px <= 25) {
                    return Rgba([248, 248, 252, 255]); // Sclera
                }
                if (px >= 8 && px <= 11) || (px >= 20 && px <= 23) {
                    return Rgba([p.eye_iris[0], p.eye_iris[1], p.eye_iris[2], 255]); // Iris
                }
            }
            // Nose shadow (y = 18..=20, x = 14..=17)
            if py >= 18 && py <= 20 && px >= 14 && px <= 17 {
                return Rgba([p.skin_shadow[0], p.skin_shadow[1], p.skin_shadow[2], 255]);
            }
            // Mouth / smile (y = 23..=24, x = 11..=20)
            if (py == 23 || py == 24) && px >= 11 && px <= 20 {
                return Rgba([138, 82, 68, 255]);
            }
            // Chin / jaw shadow
            let col = if py >= 28 || px <= 2 || px >= 29 {
                p.skin_shadow
            } else {
                p.skin
            };
            Rgba([clamp(col[0], noise), clamp(col[1], noise), clamp(col[2], noise), 255])
        });

        // Tile base + 1: Head Side / Hair / Back
        atlas.paint_tile(base + 1, |px, py| {
            let noise = (((px * 11 + py * 5) % 7) as i16) - 3;
            let clamp = |v: u8, d: i16| (v as i16 + d).clamp(0, 255) as u8;
            if py < 18 || px < 8 || px > 23 {
                let col = if (px / 2 + py / 2) % 2 == 0 { p.hair } else { p.hair_dark };
                Rgba([clamp(col[0], noise), clamp(col[1], noise), clamp(col[2], noise), 255])
            } else {
                Rgba([
                    clamp(p.skin_shadow[0], noise),
                    clamp(p.skin_shadow[1], noise),
                    clamp(p.skin_shadow[2], noise),
                    255,
                ])
            }
        });

        // Tile base + 2: Torso & Arms (Tunic/Jacket with collar and belt)
        atlas.paint_tile(base + 2, |px, py| {
            let noise = (((px * 5 + py * 9) % 5) as i16) - 2;
            let clamp = |v: u8, d: i16| (v as i16 + d).clamp(0, 255) as u8;
            // Neckline V-collar at top center
            if py < 4 && px >= 12 && px <= 19 {
                return Rgba([p.skin_shadow[0], p.skin_shadow[1], p.skin_shadow[2], 255]);
            }
            // Belt at bottom of torso (y = 26..=29)
            if py >= 26 && py <= 29 {
                if px >= 13 && px <= 18 {
                    return Rgba([230, 195, 65, 255]); // Belt buckle
                }
                return Rgba([p.belt[0], p.belt[1], p.belt[2], 255]);
            }
            let is_seam = px <= 2 || px >= 29 || (px == 15 || px == 16);
            let col = if is_seam { p.shirt_dark } else { p.shirt };
            Rgba([clamp(col[0], noise), clamp(col[1], noise), clamp(col[2], noise), 255])
        });

        // Tile base + 3: Legs & Boots
        atlas.paint_tile(base + 3, |px, py| {
            let noise = (((px * 9 + py * 7) % 5) as i16) - 2;
            let clamp = |v: u8, d: i16| (v as i16 + d).clamp(0, 255) as u8;
            if py >= 22 {
                // Boots
                return Rgba([
                    clamp(p.boots[0], noise),
                    clamp(p.boots[1], noise),
                    clamp(p.boots[2], noise),
                    255,
                ]);
            }
            let col = if px <= 2 || px >= 29 || (py >= 12 && py <= 14) {
                p.pants_dark
            } else {
                p.pants
            };
            Rgba([clamp(col[0], noise), clamp(col[1], noise), clamp(col[2], noise), 255])
        });
    }
}

/// Paints all decorative block & item tiles at indices 288..=332.
fn paint_decorative_tiles(atlas: &mut TextureAtlas) {
    let clamp = |base: i32, delta: i32| -> u8 { (base + delta).clamp(0, 255) as u8 };
    let hash = |x: u32, y: u32, seed: u32| -> i32 {
        let h = x
            .wrapping_mul(374761393)
            .wrapping_add(y.wrapping_mul(668265263))
            .wrapping_add(seed.wrapping_mul(2147483647));
        let h = (h ^ (h >> 13)).wrapping_mul(1274126177);
        ((h ^ (h >> 16)) & 0xFF) as i32
    };

    // 288: Bricks
    atlas.paint_tile(288, |px, py| {
        let row = py / 8;
        let in_row_y = py % 8;
        let x_offset = if row % 2 == 1 { 8 } else { 0 };
        let shifted_x = (px + x_offset) % 32;
        let col = shifted_x / 16;
        let in_brick_x = shifted_x % 16;

        // Mortar lines
        if in_row_y == 0 || in_brick_x == 0 {
            let n = (hash(px, py, 288) % 13) - 6;
            return Rgba([clamp(196, n), clamp(192, n), clamp(184, n), 255]);
        }

        let brick_var = (hash(col + row * 2, row, 289) % 21) - 10;
        let px_n = (hash(px, py, 290) % 13) - 6;
        let bevel = if in_row_y == 1 || in_brick_x == 1 {
            14
        } else if in_row_y == 7 || in_brick_x == 15 {
            -16
        } else {
            0
        };
        let d = brick_var + px_n + bevel;
        Rgba([clamp(158, d), clamp(72, d / 2), clamp(58, d / 2), 255])
    });

    // 289..=301: 13 Wool colors
    let palette: [[i32; 3]; 13] = [
        [236, 238, 240], // 0: White
        [176, 46, 38],   // 1: Red
        [240, 128, 34],  // 2: Orange
        [248, 212, 52],  // 3: Yellow
        [128, 199, 31],  // 4: Lime
        [84, 125, 42],   // 5: Green
        [22, 156, 156],  // 6: Cyan
        [52, 74, 178],   // 7: Blue
        [137, 50, 184],  // 8: Purple
        [243, 139, 170], // 9: Pink
        [131, 84, 50],   // 10: Brown
        [100, 104, 108], // 11: Gray
        [32, 34, 38],    // 12: Black
    ];

    for (i, rgb) in palette.iter().enumerate() {
        let tile_id = 289 + i as u16;
        atlas.paint_tile(tile_id, |px, py| {
            let weave = if ((px / 2) + (py / 2)) % 2 == 0 { 6 } else { -6 };
            let fiber = (hash(px, py, tile_id as u32) % 17) - 8;
            let d = weave + fiber;
            Rgba([
                clamp(rgb[0], d),
                clamp(rgb[1], d),
                clamp(rgb[2], d),
                255,
            ])
        });
    }

    // 302..=314: 13 Dye item sprites
    for (i, rgb) in palette.iter().enumerate() {
        let tile_id = 302 + i as u16;
        atlas.paint_tile(tile_id, |px, py| {
            let dx = px as f32 - 15.5;
            let dy = py as f32 - 16.5;
            // Organic dye powder pile / pouch silhouette
            let r2 = (dx * dx) / 1.15 + (dy * dy) * 1.1;
            let wobble = ((hash(px / 2, py / 2, tile_id as u32) % 12) - 6) as f32;
            let dist = r2.sqrt() + wobble * 0.15;
            if dist > 10.5 {
                return Rgba([0, 0, 0, 0]);
            }
            if dist > 9.0 {
                return Rgba([
                    clamp(rgb[0] / 3, 0),
                    clamp(rgb[1] / 3, 0),
                    clamp(rgb[2] / 3, 0),
                    255,
                ]);
            }
            let n = (hash(px, py, tile_id as u32 + 50) % 19) - 9;
            let highlight = if dx < -2.0 && dy < -2.0 { 26 } else if dy > 3.0 { -22 } else { 0 };
            let d = n + highlight;
            Rgba([
                clamp(rgb[0], d),
                clamp(rgb[1], d),
                clamp(rgb[2], d),
                255,
            ])
        });
    }

    // 315: Brick item sprite (single clay brick)
    atlas.paint_tile(315, |px, py| {
        let x = px as i32;
        let y = py as i32;
        // Isometric-ish brick bar
        let in_top = y >= 9 && y <= 16 && x >= 6 + (16 - y) && x <= 25 - (16 - y) / 2;
        let in_front = y > 16 && y <= 23 && x >= 6 && x <= 25;
        if !in_top && !in_front {
            return Rgba([0, 0, 0, 0]);
        }
        let is_edge = y == 9 || y == 23 || x == 6 || x == 25;
        if is_edge {
            return Rgba([92, 38, 28, 255]);
        }
        let n = (hash(px, py, 315) % 13) - 6;
        if in_top {
            Rgba([clamp(182, n), clamp(86, n / 2), clamp(68, n / 2), 255])
        } else {
            Rgba([clamp(146, n), clamp(64, n / 2), clamp(50, n / 2), 255])
        }
    });

    // 316: Oak Door texture
    atlas.paint_tile(316, |px, py| {
        let is_frame = px < 4 || px >= 28 || py < 3 || py >= 29;
        let is_mid_bar = py >= 15 && py <= 18;
        let in_upper_window = !is_frame && py >= 4 && py <= 14;
        let is_lattice = in_upper_window && ((px >= 15 && px <= 16) || (py >= 9 && py <= 10));
        let is_knob = px >= 23 && px <= 26 && py >= 16 && py <= 19;

        if is_knob {
            return Rgba([70, 74, 80, 255]);
        }
        if in_upper_window && !is_lattice {
            // Transparent window cutouts so you can see through!
            return Rgba([0, 0, 0, 0]);
        }
        let grain = (hash(px / 2, py, 316) % 15) - 7;
        let shade = if is_frame || is_mid_bar { -10 } else { 8 };
        let d = grain + shade;
        Rgba([clamp(166, d), clamp(128, d), clamp(78, d), 255])
    });

    // 317: Bed Top (white pillow + red blanket)
    atlas.paint_tile(317, |px, py| {
        let n = (hash(px, py, 317) % 11) - 5;
        if py < 10 {
            // White pillow with subtle border
            let border = if px < 2 || px >= 30 || py < 2 || py >= 8 { -22 } else { 0 };
            Rgba([clamp(238, n + border), clamp(240, n + border), clamp(244, n + border), 255])
        } else if py < 13 {
            // Folded white sheet trim
            Rgba([clamp(225, n), clamp(228, n), clamp(232, n), 255])
        } else {
            // Red blanket
            let fold = if px < 2 || px >= 30 || py >= 30 { -18 } else { 0 };
            Rgba([clamp(182, n + fold), clamp(44, n / 2), clamp(38, n / 2), 255])
        }
    });

    // 318: Bed Side (red blanket + white mattress trim + oak wood frame)
    atlas.paint_tile(318, |px, py| {
        let n = (hash(px, py, 318) % 11) - 5;
        if py < 10 {
            // Red blanket
            let fold = if py == 0 || py == 9 { -14 } else { 0 };
            Rgba([clamp(182, n + fold), clamp(44, n / 2), clamp(38, n / 2), 255])
        } else if py < 16 {
            // White mattress edge / sheet trim
            let shade = if py == 15 { -15 } else { 0 };
            Rgba([clamp(230, n + shade), clamp(232, n + shade), clamp(236, n + shade), 255])
        } else {
            // Solid oak wood bed frame with grain
            let grain = if px % 8 == 0 || py == 16 || py == 31 { -18 } else { 4 };
            Rgba([clamp(158, n + grain), clamp(118, n + grain), clamp(72, n + grain), 255])
        }
    });

    // 319..=325: 7 Flowers
    // 319: Poppy (lush red petals, golden stamen, dark center, broad foliage leaves, sturdy stem)
    paint_flower_tile(atlas, 319, |px, py| {
        let x = px as i32;
        let y = py as i32;
        let dx = x - 16;
        let dy = y - 11;
        let dist = ((dx * dx) as f32 * 1.0 + (dy * dy) as f32 * 1.5).sqrt();
        if dist <= 3.2 {
            let fleck = if (x + y) % 3 == 0 { 40 } else { 0 };
            return Some(Rgba([38 + fleck, 24 + fleck / 2, 28, 255]));
        }
        if dist <= 4.4 && ((x + y) % 2 == 0) {
            return Some(Rgba([245, 205, 50, 255]));
        }
        if dist <= 9.8 && y <= 18 {
            let is_edge = dist > 8.5;
            let fold = ((dx.abs() * 3 + dy * 2) % 7) - 3;
            let base_r = if is_edge { 245 } else { 210 + fold * 8 };
            let base_g = if is_edge { 55 } else { 30 + fold * 3 };
            let base_b = if is_edge { 45 } else { 28 };
            return Some(Rgba([
                base_r.clamp(140, 255) as u8,
                base_g.clamp(20, 90) as u8,
                base_b.clamp(18, 70) as u8,
                255,
            ]));
        }
        let is_left_leaf = x >= 6 && x <= 14 && y >= 19 && y <= 29 && (y - x <= 18) && (x + y >= 26);
        let is_right_leaf = x >= 17 && x <= 25 && y >= 19 && y <= 29 && (x - y <= 4) && (x + y <= 50);
        if is_left_leaf || is_right_leaf {
            let vein = if (x == 10 && y >= 21) || (x == 21 && y >= 21) { 25 } else { 0 };
            let shade = ((x * 3 + y * 2) % 5) * 6;
            return Some(Rgba([
                (55 + vein + shade).clamp(40, 110) as u8,
                (135 + vein + shade).clamp(90, 180) as u8,
                (42 + vein / 2).clamp(30, 80) as u8,
                255,
            ]));
        }
        if x >= 14 && x <= 17 && y >= 16 {
            let shade: i32 = if x == 14 { -18 } else if x == 17 { 18 } else { 0 };
            return Some(Rgba([
                (60 + shade).clamp(40, 95) as u8,
                (138 + shade).clamp(95, 175) as u8,
                (46 + shade / 2).clamp(30, 75) as u8,
                255,
            ]));
        }
        None
    });

    // 320: Dandelion (dense golden-yellow composite head with toothed basal leaves)
    paint_flower_tile(atlas, 320, |px, py| {
        let x = px as i32;
        let y = py as i32;
        let dx = x - 16;
        let dy = y - 11;
        let dist = ((dx * dx) as f32 + (dy * dy) as f32 * 1.3).sqrt();
        if dist <= 9.6 && y <= 17 {
            let is_edge = dist > 8.0;
            let shade = ((dx.abs() * 4 + dy * 3) % 7) - 3;
            let r = if is_edge { 255 } else { 240 + shade * 4 };
            let g = if is_edge { 235 } else { 195 + shade * 10 };
            let b = if is_edge { 80 } else { 24 + (shade + 3) * 6 };
            return Some(Rgba([r.clamp(200, 255) as u8, g.clamp(150, 255) as u8, b.clamp(15, 110) as u8, 255]));
        }
        if y >= 16 && y <= 19 && x >= 12 && x <= 19 && (x - 16).abs() <= (y - 14) {
            return Some(Rgba([72, 145, 48, 255]));
        }
        let is_left_leaf = x >= 5 && x <= 14 && y >= 20 && y <= 31 && (y - x <= 20);
        let is_right_leaf = x >= 17 && x <= 26 && y >= 20 && y <= 31 && (x - y <= 4);
        if is_left_leaf || is_right_leaf {
            let tooth = ((x + y * 2) % 4 == 0) as i32 * 20;
            return Some(Rgba([
                (60 + tooth).clamp(45, 110) as u8,
                (142 + tooth).clamp(100, 185) as u8,
                (44 + tooth / 2).clamp(30, 80) as u8,
                255,
            ]));
        }
        if x >= 14 && x <= 17 && y >= 18 {
            let shade: i32 = if x == 14 { -16 } else if x == 17 { 16 } else { 0 };
            return Some(Rgba([
                (65 + shade).clamp(45, 95) as u8,
                (144 + shade).clamp(100, 175) as u8,
                (48 + shade / 2).clamp(32, 75) as u8,
                255,
            ]));
        }
        None
    });

    // 321: Blue Orchid (vibrant tiered sky-blue blossoms with graceful leaves)
    paint_flower_tile(atlas, 321, |px, py| {
        let x = px as i32;
        let y = py as i32;
        let d1 = ((x - 16) * (x - 16) + (y - 8) * (y - 8)) as f32;
        let d2 = ((x - 12) * (x - 12) + (y - 15) * (y - 15)) as f32;

        if d1 <= 5.5 * 5.5 {
            let dist = d1.sqrt();
            let is_throat = dist <= 2.2;
            if is_throat {
                return Some(Rgba([240, 248, 255, 255]));
            }
            let is_edge = dist > 4.5;
            let shade = ((x + y * 2) % 5) - 2;
            let r = if is_edge { 140 } else { 42 + shade * 6 };
            let g = if is_edge { 220 } else { 165 + shade * 8 };
            let b = if is_edge { 255 } else { 242 };
            return Some(Rgba([r.clamp(25, 180) as u8, g.clamp(120, 240) as u8, b.clamp(200, 255) as u8, 255]));
        }
        if d2 <= 4.2 * 4.2 {
            let dist = d2.sqrt();
            if dist <= 1.8 {
                return Some(Rgba([240, 248, 255, 255]));
            }
            let is_edge = dist > 3.4;
            let r = if is_edge { 130 } else { 38 };
            let g = if is_edge { 210 } else { 158 };
            return Some(Rgba([r, g, 244, 255]));
        }
        let is_left_leaf = x >= 6 && x <= 14 && y >= 21 && y <= 30 && (y - x <= 18);
        let is_right_leaf = x >= 17 && x <= 25 && y >= 21 && y <= 30 && (x - y <= 2);
        if is_left_leaf || is_right_leaf {
            let shade = ((x * 2 + y) % 4) * 8;
            return Some(Rgba([(52 + shade).clamp(0, 255) as u8, (128 + shade).clamp(0, 255) as u8, 56, 255]));
        }
        if x >= 14 && x <= 17 && y >= 12 {
            let shade: i32 = if x == 14 { -14 } else if x == 17 { 14 } else { 0 };
            return Some(Rgba([
                (55 + shade).clamp(38, 85) as u8,
                (132 + shade).clamp(90, 165) as u8,
                (50 + shade / 2).clamp(32, 75) as u8,
                255,
            ]));
        }
        None
    });

    // 322: Allium (dense spherical pom-pom globe packed with star-like violet florets)
    paint_flower_tile(atlas, 322, |px, py| {
        let x = px as i32;
        let y = py as i32;
        let dx = x - 16;
        let dy = y - 10;
        let dist = ((dx * dx + dy * dy) as f32).sqrt();
        if dist <= 9.2 {
            let is_star_tip = (x % 3 == 0) && (y % 3 == 0);
            let is_shadow = dist > 7.5;
            let pattern = (hash(px, py, 322) % 9) as i32 - 4;
            let r = if is_star_tip { 235 } else if is_shadow { 135 + pattern * 6 } else { 185 + pattern * 8 };
            let g = if is_star_tip { 170 } else if is_shadow { 42 + pattern * 3 } else { 85 + pattern * 6 };
            let b = if is_star_tip { 255 } else if is_shadow { 180 + pattern * 6 } else { 228 + pattern * 5 };
            return Some(Rgba([r.clamp(110, 255) as u8, g.clamp(30, 200) as u8, b.clamp(150, 255) as u8, 255]));
        }
        let is_left_leaf = x >= 7 && x <= 14 && y >= 22 && y <= 31 && (y - x <= 20);
        let is_right_leaf = x >= 17 && x <= 24 && y >= 22 && y <= 31 && (x - y <= 2);
        if is_left_leaf || is_right_leaf {
            return Some(Rgba([60, 142, 52, 255]));
        }
        if x >= 14 && x <= 17 && y >= 17 {
            let shade: i32 = if x == 14 { -16 } else if x == 17 { 16 } else { 0 };
            return Some(Rgba([
                (58 + shade).clamp(40, 90) as u8,
                (136 + shade).clamp(92, 170) as u8,
                (48 + shade / 2).clamp(32, 75) as u8,
                255,
            ]));
        }
        None
    });

    // 323: Orange Tulip (vibrant cup bloom with overlapping petals & clasping leaves)
    paint_flower_tile(atlas, 323, |px, py| {
        paint_tulip_pixel(px, py, [242, 115, 24], [255, 180, 42], [185, 58, 16])
    });

    // 324: Pink Tulip (soft rose-pink petals with pastel highlights & clasping leaves)
    paint_flower_tile(atlas, 324, |px, py| {
        paint_tulip_pixel(px, py, [245, 132, 175], [255, 202, 224], [188, 62, 115])
    });

    // 325: Oxeye Daisy (solid radiant white petals, rich gold button center, and serrated leaves)
    paint_flower_tile(atlas, 325, |px, py| {
        let x = px as i32;
        let y = py as i32;
        let dx = x - 16;
        let dy = y - 11;
        let dist = ((dx * dx) as f32 + (dy * dy) as f32 * 1.2).sqrt();
        if dist <= 3.8 {
            let ring = dist > 2.8;
            let r = if ring { 220 } else { 252 };
            let g = if ring { 165 } else { 210 };
            return Some(Rgba([r, g, 28, 255]));
        }
        if dist <= 9.8 && y <= 18 {
            let is_edge = dist > 8.5;
            let petal_shade = ((dx.abs() * 5 + dy.abs() * 3) % 7) == 0;
            let r = if is_edge { 255 } else if petal_shade { 220 } else { 248 };
            let g = if is_edge { 255 } else if petal_shade { 228 } else { 250 };
            let b = if is_edge { 255 } else if petal_shade { 236 } else { 254 };
            return Some(Rgba([r, g, b, 255]));
        }
        let is_left_leaf = x >= 6 && x <= 14 && y >= 19 && y <= 31 && (y - x <= 20);
        let is_right_leaf = x >= 17 && x <= 25 && y >= 19 && y <= 31 && (x - y <= 4);
        if is_left_leaf || is_right_leaf {
            let shade = ((x + y * 2) % 4) * 8;
            return Some(Rgba([(55 + shade).clamp(0, 255) as u8, (136 + shade).clamp(0, 255) as u8, 48, 255]));
        }
        if x >= 14 && x <= 17 && y >= 16 {
            let shade: i32 = if x == 14 { -16 } else if x == 17 { 16 } else { 0 };
            return Some(Rgba([
                (58 + shade).clamp(40, 90) as u8,
                (138 + shade).clamp(95, 170) as u8,
                (46 + shade / 2).clamp(30, 75) as u8,
                255,
            ]));
        }
        None
    });

    // 326: Pumpkin Side (orange vertical ribs)
    atlas.paint_tile(326, |px, py| {
        let rib = ((px % 8) as i32 - 4).abs() * 6;
        let n = (hash(px, py, 326) % 11) - 5;
        let top_bot = if py < 2 || py >= 30 { -16 } else { 0 };
        let d = n - rib + top_bot;
        Rgba([clamp(228, d), clamp(128, d / 2), clamp(26, d / 3), 255])
    });

    // 327: Pumpkin Top/Bottom
    atlas.paint_tile(327, |px, py| {
        let dx = px as i32 - 16;
        let dy = py as i32 - 16;
        if dx.abs() <= 2 && dy.abs() <= 2 {
            return Rgba([102, 118, 48, 255]); // Woody stem
        }
        let radial = ((dx.abs() == dy.abs()) || dx == 0 || dy == 0) as i32 * -18;
        let n = (hash(px, py, 327) % 11) - 5;
        let d = n + radial;
        Rgba([clamp(220, d), clamp(122, d / 2), clamp(24, d / 3), 255])
    });

    // 328: Jack o'Lantern Front
    atlas.paint_tile(328, |px, py| {
        let x = px as i32;
        let y = py as i32;
        let left_eye = x >= 6 && x <= 11 && y >= 9 && y <= 14 && (y - 9) >= (x - 8).abs();
        let right_eye = x >= 20 && x <= 25 && y >= 9 && y <= 14 && (y - 9) >= (x - 22).abs();
        let nose = x >= 14 && x <= 17 && y >= 15 && y <= 18;
        let mouth = y >= 21 && y <= 26 && x >= 7 && x <= 24
            && !((y == 21 && (x == 11 || x == 19)) || (y == 26 && (x == 14 || x == 15)));
        if left_eye || right_eye || nose || mouth {
            return Rgba([255, 240, 120, 255]); // Glowing carved face
        }
        let rib = ((px % 8) as i32 - 4).abs() * 6;
        let n = (hash(px, py, 328) % 11) - 5;
        let d = n - rib;
        Rgba([clamp(228, d), clamp(128, d / 2), clamp(26, d / 3), 255])
    });

    // 329: Melon Side (green striped rind)
    atlas.paint_tile(329, |px, py| {
        let stripe = if (px % 8) < 3 { -28 } else { 10 };
        let n = (hash(px, py, 329) % 15) - 7;
        let d = stripe + n;
        Rgba([clamp(112, d), clamp(172, d), clamp(56, d / 2), 255])
    });

    // 330: Melon Top/Bottom
    atlas.paint_tile(330, |px, py| {
        let dx = px as i32 - 16;
        let dy = py as i32 - 16;
        if dx.abs() <= 1 && dy.abs() <= 1 {
            return Rgba([82, 108, 42, 255]);
        }
        let stripe = if (dx.abs() + dy.abs()) % 6 < 2 { -24 } else { 8 };
        let n = (hash(px, py, 330) % 13) - 6;
        let d = stripe + n;
        Rgba([clamp(112, d), clamp(172, d), clamp(56, d / 2), 255])
    });

    // 331: Melon Slice item sprite
    atlas.paint_tile(331, |px, py| {
        let x = px as i32 - 16;
        let y = py as i32 - 10;
        let r2 = x * x + y * y;
        if y < 0 || r2 > 14 * 14 {
            return Rgba([0, 0, 0, 0]);
        }
        if r2 > 12 * 12 {
            return Rgba([58, 138, 44, 255]); // Dark green rind
        }
        if r2 > 10 * 10 {
            return Rgba([186, 224, 152, 255]); // Pale pith
        }
        // Seeds
        if (x == -4 && y == 5) || (x == 0 && y == 7) || (x == 4 && y == 5) {
            return Rgba([32, 24, 20, 255]);
        }
        let n = (hash(px, py, 331) % 13) - 6;
        Rgba([clamp(228, n), clamp(64, n / 2), clamp(64, n / 2), 255])
    });

    // 332: Bed item sprite (side-view mini bed)
    atlas.paint_tile(332, |px, py| {
        let x = px as i32;
        let y = py as i32;
        if x < 4 || x > 27 || y < 11 || y > 24 {
            return Rgba([0, 0, 0, 0]);
        }
        if y >= 20 {
            if x <= 7 || x >= 24 {
                return Rgba([130, 94, 54, 255]); // Legs
            }
            return Rgba([0, 0, 0, 0]);
        }
        if y >= 17 {
            return Rgba([158, 118, 72, 255]); // Frame
        }
        if x <= 11 {
            return Rgba([240, 242, 246, 255]); // Pillow
        }
        Rgba([186, 44, 38, 255]) // Red blanket
    });

    // 333: Oak Door Top (frame + 4 recessed transparent window panes)
    atlas.paint_tile(333, |px, py| {
        let is_frame = px < 5 || px >= 27 || py < 4 || py >= 28;
        let in_window = !is_frame;
        let is_lattice = in_window && ((px >= 15 && px <= 16) || (py >= 15 && py <= 16));
        if in_window && !is_lattice {
            return Rgba([0, 0, 0, 0]);
        }
        let grain = (hash(px / 2, py, 333) % 15) - 7;
        let shade = if is_frame { -8 } else { 8 };
        let d = grain + shade;
        Rgba([clamp(166, d), clamp(128, d), clamp(78, d), 255])
    });

    // 334: Oak Door Bottom (solid oak wood panel with recessed panels & iron doorknob)
    atlas.paint_tile(334, |px, py| {
        let is_knob = px >= 23 && px <= 26 && py >= 2 && py <= 5;
        if is_knob {
            return Rgba([70, 74, 80, 255]);
        }
        let is_frame = px < 5 || px >= 27 || py < 3 || py >= 29 || (py >= 14 && py <= 17);
        let grain = (hash(px / 2, py, 334) % 15) - 7;
        let shade = if is_frame { 8 } else { -12 };
        let d = grain + shade;
        Rgba([clamp(166, d), clamp(128, d), clamp(78, d), 255])
    });

    // 335: Oak Stairs item icon, 336: Cobblestone Stairs item icon, 337: Brick Stairs item icon
    let stair_styles: [(u16, [i32; 3], [i32; 3]); 3] = [
        (335, [172, 134, 82], [116, 86, 48]),   // Oak
        (336, [132, 134, 138], [86, 88, 92]),   // Cobblestone
        (337, [162, 74, 60], [116, 52, 42]),    // Brick
    ];
    for (tile_id, base, edge) in stair_styles {
        atlas.paint_tile(tile_id, |px, py| {
            let x = px as i32;
            let y = py as i32;
            // L-shaped stair silhouette inside 4..28
            let in_bottom = x >= 4 && x <= 27 && y >= 16 && y <= 27;
            let in_top_back = x >= 4 && x <= 16 && y >= 4 && y < 16;
            if !in_bottom && !in_top_back {
                return Rgba([0, 0, 0, 0]);
            }
            let is_border = x == 4
                || y == 27
                || (in_top_back && (y == 4 || x == 16))
                || (in_bottom && (x == 27 || (y == 16 && x >= 16)));
            if is_border {
                return Rgba([edge[0] as u8, edge[1] as u8, edge[2] as u8, 255]);
            }
            let n = (hash(px, py, tile_id as u32) % 19) - 9;
            let step_line = if y == 16 || (tile_id == 337 && (y % 6 == 0 || (x + (y / 6) * 6) % 12 == 0)) {
                -18
            } else if y < 16 {
                10
            } else {
                0
            };
            let d = n + step_line;
            Rgba([
                clamp(base[0], d),
                clamp(base[1], d),
                clamp(base[2], d),
                255,
            ])
        });
    }

    // 338: Oak Fence item icon (two vertical posts + two horizontal rails)
    atlas.paint_tile(338, |px, py| {
        let x = px as i32;
        let y = py as i32;
        let in_left_post = x >= 6 && x <= 11 && y >= 4 && y <= 27;
        let in_right_post = x >= 20 && x <= 25 && y >= 4 && y <= 27;
        let in_top_rail = x >= 4 && x <= 27 && y >= 9 && y <= 13;
        let in_bot_rail = x >= 4 && x <= 27 && y >= 19 && y <= 23;
        if !in_left_post && !in_right_post && !in_top_rail && !in_bot_rail {
            return Rgba([0, 0, 0, 0]);
        }
        let grain = (hash(px / 2, py, 338) % 15) - 7;
        let shade = if in_left_post || in_right_post { 10 } else { -12 };
        let d = grain + shade;
        Rgba([clamp(170, d), clamp(132, d), clamp(80, d), 255])
    });

    // 339: Sheep fluffy wool coat
    atlas.paint_tile(339, |px, py| {
        let puff = (hash(px / 3, py / 3, 339) % 18) - 9;
        let fine = (hash(px, py, 3391) % 9) - 4;
        let d = puff + fine;
        Rgba([clamp(242, d), clamp(240, d), clamp(234, d), 255])
    });

    // 340: Sheep face (white wool fringe at top, pale skin face, dark eyes, pink nose)
    atlas.paint_tile(340, |px, py| {
        let x = px as i32;
        let y = py as i32;
        if y <= 7 || (y <= 10 && (x <= 5 || x >= 26)) {
            let puff = (hash(px / 2, py / 2, 340) % 14) - 7;
            return Rgba([clamp(242, puff), clamp(240, puff), clamp(234, puff), 255]);
        }
        // Left eye: sclera + dark pupil
        if y >= 13 && y <= 16 {
            if (x >= 4 && x <= 7) || (x >= 24 && x <= 27) {
                return Rgba([250, 250, 250, 255]);
            }
            if (x >= 8 && x <= 10) || (x >= 21 && x <= 23) {
                return Rgba([34, 34, 40, 255]);
            }
        }
        // Pink nose / snout
        if y >= 22 && y <= 26 && x >= 11 && x <= 20 {
            let n = (hash(px, py, 3402) % 9) - 4;
            return Rgba([clamp(232, n), clamp(166, n), clamp(172, n), 255]);
        }
        let skin = (hash(px / 2, py / 2, 3401) % 9) - 4;
        Rgba([clamp(226, skin), clamp(196, skin), clamp(174, skin), 255])
    });

    // 341: Sheep legs (pale skin with darker hooves at bottom)
    atlas.paint_tile(341, |px, py| {
        let n = (hash(px, py, 341) % 9) - 4;
        if py >= 25 {
            Rgba([clamp(92, n), clamp(82, n), clamp(76, n), 255])
        } else {
            Rgba([clamp(218, n), clamp(188, n), clamp(166, n), 255])
        }
    });

    // 342: Bed Head Top (white pillow + folded sheet trim)
    atlas.paint_tile(342, |px, py| {
        let n = (hash(px, py, 342) % 11) - 5;
        if py < 20 {
            let border = if px < 3 || px >= 29 || py < 3 || py >= 18 { -20 } else { 0 };
            Rgba([clamp(240, n + border), clamp(242, n + border), clamp(246, n + border), 255])
        } else if py < 26 {
            Rgba([clamp(224, n), clamp(228, n), clamp(234, n), 255])
        } else {
            Rgba([clamp(182, n), clamp(44, n / 2), clamp(38, n / 2), 255])
        }
    });

    // 343: Bed Foot Top (red blanket with subtle border)
    atlas.paint_tile(343, |px, py| {
        let n = (hash(px, py, 343) % 11) - 5;
        let fold = if px < 2 || px >= 30 || py >= 29 { -18 } else { 0 };
        Rgba([clamp(182, n + fold), clamp(44, n / 2), clamp(38, n / 2), 255])
    });

    // 344: Bow item sprite (curved oak bow + white bowstring)
    atlas.paint_tile(344, |px, py| {
        let x = px as i32;
        let y = py as i32;
        // Bowstring connecting (6,6) to (25,25)
        if (x + y - 31).abs() <= 0 && x >= 7 && x <= 24 {
            return Rgba([230, 232, 236, 255]);
        }
        // Curved wooden limbs
        let dx = x - 8;
        let dy = y - 23;
        let r2 = dx * dx + dy * dy;
        let on_arc = r2 >= 15 * 15 && r2 <= 19 * 19 && x >= 6 && y <= 26;
        let on_tip = ((x == 6 || x == 7) && (y >= 5 && y <= 8)) || ((x >= 23 && x <= 26) && (y == 24 || y == 25));
        if on_arc || on_tip {
            let n = (hash(px, py, 344) % 13) - 6;
            return Rgba([clamp(148, n), clamp(104, n), clamp(58, n), 255]);
        }
        Rgba([0, 0, 0, 0])
    });

    // 345: Arrow item sprite (diagonal shaft, stone tip top-right, white fletching bottom-left)
    atlas.paint_tile(345, |px, py| {
        let x = px as i32;
        let y = py as i32;
        // Arrowhead top-right
        if (x >= 21 && x <= 27 && y >= 4 && y <= 10) && (x - y >= 13 && x - y <= 20) && (x + y <= 34) {
            let n = (hash(px, py, 345) % 13) - 6;
            return Rgba([clamp(165, n), clamp(170, n), clamp(178, n), 255]);
        }
        // Fletching bottom-left
        if x >= 4 && x <= 10 && y >= 21 && y <= 27 && ((x + y - 31).abs() <= 3) && (x <= 9 && y >= 22) {
            return Rgba([238, 240, 244, 255]);
        }
        // Wooden shaft along x + y == 31
        if (x + y - 31).abs() <= 1 && x >= 6 && x <= 24 {
            let n = (hash(px, py, 3451) % 9) - 4;
            return Rgba([clamp(152, n), clamp(112, n), clamp(66, n), 255]);
        }
        Rgba([0, 0, 0, 0])
    });

    // 346: String item sprite
    atlas.paint_tile(346, |px, py| {
        let x = px as i32 - 16;
        let y = py as i32 - 16;
        let r2 = x * x + y * y;
        if (r2 >= 42 && r2 <= 95) || ((x + y).abs() <= 1 && r2 <= 110) {
            let n = (hash(px, py, 346) % 15) - 7;
            return Rgba([clamp(236, n), clamp(238, n), clamp(242, n), 255]);
        }
        Rgba([0, 0, 0, 0])
    });

    // 347: Bone item sprite
    atlas.paint_tile(347, |px, py| {
        let x = px as i32;
        let y = py as i32;
        let in_shaft = (x + y - 31).abs() <= 2 && x >= 7 && x <= 24;
        let in_knob1 = (x - 7).pow(2) + (y - 23).pow(2) <= 10 || (x - 9).pow(2) + (y - 25).pow(2) <= 10;
        let in_knob2 = (x - 24).pow(2) + (y - 7).pow(2) <= 10 || (x - 22).pow(2) + (y - 9).pow(2) <= 10;
        if in_shaft || in_knob1 || in_knob2 {
            let n = (hash(px, py, 347) % 11) - 5;
            return Rgba([clamp(236, n), clamp(232, n), clamp(216, n), 255]);
        }
        Rgba([0, 0, 0, 0])
    });

    // 348: Rotten Flesh item sprite
    atlas.paint_tile(348, |px, py| {
        let x = px as i32 - 16;
        let y = py as i32 - 16;
        if x * x + (y * 3 / 2) * (y * 3 / 2) <= 115 {
            let n = (hash(px, py, 348) % 19) - 9;
            if (px + py) % 7 == 0 {
                return Rgba([clamp(92, n), clamp(118, n), clamp(58, n), 255]); // Greenish rot
            }
            return Rgba([clamp(146, n), clamp(78, n), clamp(58, n), 255]);
        }
        Rgba([0, 0, 0, 0])
    });

    // 349: Zombie Head/Face (green skin, dark hollow eyes & mouth)
    atlas.paint_tile(349, |px, py| {
        let x = px as i32;
        let y = py as i32;
        if y >= 13 && y <= 16 && ((x >= 6 && x <= 11) || (x >= 20 && x <= 25)) {
            return Rgba([22, 28, 22, 255]); // Hollow eyes
        }
        if y >= 22 && y <= 25 && x >= 11 && x <= 20 {
            return Rgba([42, 62, 36, 255]); // Mouth
        }
        let n = (hash(px / 2, py / 2, 349) % 15) - 7;
        Rgba([clamp(82, n), clamp(134, n), clamp(72, n), 255])
    });

    // 350: Zombie Shirt (teal-blue shirt)
    atlas.paint_tile(350, |px, py| {
        let n = (hash(px / 2, py / 2, 350) % 15) - 7;
        Rgba([clamp(36, n), clamp(128, n), clamp(142, n), 255])
    });

    // 351: Zombie Pants (indigo-blue trousers)
    atlas.paint_tile(351, |px, py| {
        let n = (hash(px / 2, py / 2, 351) % 15) - 7;
        Rgba([clamp(56, n), clamp(54, n), clamp(124, n), 255])
    });

    // 352: Skeleton Skull/Face
    atlas.paint_tile(352, |px, py| {
        let x = px as i32;
        let y = py as i32;
        if y >= 12 && y <= 17 && ((x >= 5 && x <= 11) || (x >= 20 && x <= 26)) {
            return Rgba([24, 24, 28, 255]); // Eye sockets
        }
        if y >= 19 && y <= 22 && x >= 14 && x <= 17 {
            return Rgba([38, 38, 42, 255]); // Nasal cavity
        }
        if y >= 24 && y <= 27 && x >= 8 && x <= 23 && (x % 4 != 0) {
            return Rgba([46, 46, 50, 255]); // Teeth gaps
        }
        let n = (hash(px / 2, py / 2, 352) % 13) - 6;
        Rgba([clamp(228, n), clamp(228, n), clamp(222, n), 255])
    });

    // 353: Skeleton Bones / Ribcage
    atlas.paint_tile(353, |px, py| {
        let n = (hash(px / 2, py / 2, 353) % 13) - 6;
        if py % 6 < 2 && px >= 6 && px <= 25 {
            Rgba([clamp(140, n), clamp(140, n), clamp(136, n), 255])
        } else {
            Rgba([clamp(220, n), clamp(220, n), clamp(214, n), 255])
        }
    });

    // 354: Spider Face (charcoal-black with 8 glowing crimson-red eyes)
    atlas.paint_tile(354, |px, py| {
        let x = px as i32;
        let y = py as i32;
        let is_big_eye = y >= 12 && y <= 16 && ((x >= 6 && x <= 10) || (x >= 21 && x <= 25));
        let is_small_eye = y >= 19 && y <= 22 && ((x >= 9 && x <= 12) || (x >= 19 && x <= 22));
        if is_big_eye || is_small_eye {
            return Rgba([230, 28, 28, 255]); // Glowing red eyes
        }
        let n = (hash(px / 2, py / 2, 354) % 11) - 5;
        Rgba([clamp(34, n), clamp(30, n), clamp(36, n), 255])
    });

    // 355: Spider Carapace / Legs
    atlas.paint_tile(355, |px, py| {
        let n = (hash(px / 2, py / 2, 355) % 13) - 6;
        Rgba([clamp(42, n), clamp(36, n), clamp(44, n), 255])
    });

    // 356..=371: Armor Item Icons (Leather, Iron, Gold, Diamond x Helmet, Chestplate, Leggings, Boots)
    let armor_palettes: [([u8; 3], [u8; 3], [u8; 3]); 4] = [
        ([148, 92, 52], [182, 118, 72], [96, 56, 30]),     // Leather
        ([196, 202, 210], [234, 238, 245], [116, 122, 130]), // Iron
        ([238, 194, 48], [255, 232, 115], [166, 122, 20]), // Gold
        ([72, 216, 208], [148, 246, 242], [32, 146, 142]), // Diamond
    ];
    for (tier_idx, (base, hi, dark)) in armor_palettes.iter().enumerate() {
        let base_tile = 356 + (tier_idx as u16) * 4;
        // Helmet
        atlas.paint_tile(base_tile, |px, py| {
            let x = px as i32;
            let y = py as i32;
            let in_dome = y >= 7 && y <= 14 && x >= 7 && x <= 24;
            let in_sides = y >= 15 && y <= 20 && ((x >= 7 && x <= 11) || (x >= 20 && x <= 24));
            if in_dome || in_sides {
                if y == 7 || x == 7 || x == 24 || y == 20 {
                    Rgba([dark[0], dark[1], dark[2], 255])
                } else if y <= 10 && x <= 18 {
                    Rgba([hi[0], hi[1], hi[2], 255])
                } else {
                    Rgba([base[0], base[1], base[2], 255])
                }
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        // Chestplate
        atlas.paint_tile(base_tile + 1, |px, py| {
            let x = px as i32;
            let y = py as i32;
            let in_shoulders = y >= 6 && y <= 12 && x >= 4 && x <= 27 && !(y <= 8 && x >= 12 && x <= 19);
            let in_torso = y >= 13 && y <= 26 && x >= 9 && x <= 22;
            if in_shoulders || in_torso {
                if x == 4 || x == 27 || y == 6 || y == 26 || (in_torso && (x == 9 || x == 22)) {
                    Rgba([dark[0], dark[1], dark[2], 255])
                } else if y <= 12 && x <= 16 {
                    Rgba([hi[0], hi[1], hi[2], 255])
                } else {
                    Rgba([base[0], base[1], base[2], 255])
                }
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        // Leggings
        atlas.paint_tile(base_tile + 2, |px, py| {
            let x = px as i32;
            let y = py as i32;
            let in_waist = y >= 6 && y <= 11 && x >= 8 && x <= 23;
            let in_legs = y >= 12 && y <= 26 && ((x >= 8 && x <= 14) || (x >= 17 && x <= 23));
            if in_waist || in_legs {
                if y == 6 || y == 26 || x == 8 || x == 23 || (in_legs && (x == 14 || x == 17)) {
                    Rgba([dark[0], dark[1], dark[2], 255])
                } else if y <= 13 && x <= 15 {
                    Rgba([hi[0], hi[1], hi[2], 255])
                } else {
                    Rgba([base[0], base[1], base[2], 255])
                }
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        // Boots
        atlas.paint_tile(base_tile + 3, |px, py| {
            let x = px as i32;
            let y = py as i32;
            let left_boot = (y >= 13 && y <= 24 && x >= 7 && x <= 13) || (y >= 20 && y <= 24 && x >= 5 && x <= 13);
            let right_boot = (y >= 13 && y <= 24 && x >= 18 && x <= 24) || (y >= 20 && y <= 24 && x >= 18 && x <= 26);
            if left_boot || right_boot {
                if y == 24 || y == 13 || x == 5 || x == 26 {
                    Rgba([dark[0], dark[1], dark[2], 255])
                } else if y <= 17 {
                    Rgba([hi[0], hi[1], hi[2], 255])
                } else {
                    Rgba([base[0], base[1], base[2], 255])
                }
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
    }

    // 372: HUD Armor Shield/Chestplate Icon (white/silver tintable)
    atlas.paint_tile(372, |px, py| {
        let x = px as i32;
        let y = py as i32;
        let in_shoulders = y >= 4 && y <= 12 && x >= 3 && x <= 28 && !(y <= 7 && x >= 11 && x <= 20);
        let in_torso = y >= 13 && y <= 27 && x >= 8 && x <= 23;
        if in_shoulders || in_torso {
            if x == 3 || x == 28 || y == 4 || y == 27 || (in_torso && (x == 8 || x == 23)) {
                Rgba([55, 58, 65, 255])
            } else if y <= 11 && x <= 16 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([210, 215, 224, 255])
            }
        } else {
            Rgba([0, 0, 0, 0])
        }
    });

    // 373: Sun (bright warm golden square with soft solar corona)
    atlas.paint_tile(373, |px, py| {
        let x = px as i32;
        let y = py as i32;
        if x >= 6 && x <= 25 && y >= 6 && y <= 25 {
            // Core sun disc
            if x >= 9 && x <= 22 && y >= 9 && y <= 22 {
                Rgba([255, 250, 210, 255])
            } else {
                Rgba([255, 224, 110, 255])
            }
        } else if x >= 3 && x <= 28 && y >= 3 && y <= 28 {
            // Soft outer solar glow
            let dx = (x - 15).abs().max((y - 15).abs());
            let alpha = ((14 - dx) * 28).clamp(0, 140) as u8;
            Rgba([255, 200, 75, alpha])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });

    // 374: Moon (pale silver-white square with subtle craters and soft cool glow)
    atlas.paint_tile(374, |px, py| {
        let x = px as i32;
        let y = py as i32;
        if x >= 7 && x <= 24 && y >= 7 && y <= 24 {
            let is_crater = (x >= 10 && x <= 13 && y >= 10 && y <= 13)
                || (x >= 18 && x <= 21 && y >= 15 && y <= 18)
                || (x >= 12 && x <= 14 && y >= 19 && y <= 21);
            if is_crater {
                Rgba([190, 198, 215, 255])
            } else if x == 7 || x == 24 || y == 7 || y == 24 {
                Rgba([210, 220, 238, 255])
            } else {
                Rgba([242, 245, 255, 255])
            }
        } else if x >= 4 && x <= 27 && y >= 4 && y <= 27 {
            let dx = (x - 15).abs().max((y - 15).abs());
            let alpha = ((13 - dx) * 22).clamp(0, 100) as u8;
            Rgba([190, 210, 245, alpha])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });

    // 375..=377: 3-Frame Animated Flame Sprites for burning mobs
    for frame in 0..3_u16 {
        atlas.paint_tile(375 + frame, |px, py| {
            let x = px as i32;
            let y = py as i32; // 0 = top, 31 = bottom
            let h = 31 - y;    // height from base (0..=31)
            let phase = (frame as f32) * 2.1;

            // Multiple overlapping flickering flame tongues
            let tongue1 = (16.0 + ((h as f32) * 0.45 + phase).sin() * 6.0) as i32;
            let tongue2 = (10.0 + ((h as f32) * 0.65 - phase).sin() * 5.0) as i32;
            let tongue3 = (22.0 + ((h as f32) * 0.55 + phase * 1.3).cos() * 5.0) as i32;

            let max_h1 = 26 + ((frame as i32 * 3) % 5);
            let max_h2 = 20 + (((frame as i32 + 1) * 4) % 6);
            let max_h3 = 21 + (((frame as i32 + 2) * 3) % 6);

            let w1 = ((max_h1 - h).max(0) as f32 * 0.28).ceil() as i32;
            let w2 = ((max_h2 - h).max(0) as f32 * 0.24).ceil() as i32;
            let w3 = ((max_h3 - h).max(0) as f32 * 0.24).ceil() as i32;

            let in_t1 = h <= max_h1 && (x - tongue1).abs() <= w1;
            let in_t2 = h <= max_h2 && (x - tongue2).abs() <= w2;
            let in_t3 = h <= max_h3 && (x - tongue3).abs() <= w3;

            if in_t1 || in_t2 || in_t3 {
                if h <= 8 && (x - 16).abs() <= 7 {
                    // Hot white-yellow core at base
                    Rgba([255, 250, 170, 245])
                } else if h <= 15 {
                    // Bright golden-orange middle
                    Rgba([255, 180, 40, 235])
                } else {
                    // Red-orange flame tips
                    Rgba([245, 75, 20, 210])
                }
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
    }
}

fn paint_flower_tile<F>(atlas: &mut TextureAtlas, tile_id: u16, mut f: F)
where
    F: FnMut(u32, u32) -> Option<Rgba<u8>>,
{
    atlas.paint_tile(tile_id, |px, py| f(px, py).unwrap_or(Rgba([0, 0, 0, 0])));
}

fn paint_tulip_pixel(
    px: u32,
    py: u32,
    main_rgb: [u8; 3],
    hi_rgb: [u8; 3],
    shadow_rgb: [u8; 3],
) -> Option<Rgba<u8>> {
    let x = px as i32;
    let y = py as i32;
    // Tulip cup (rows 4..=17, cols 8..=23)
    if y >= 4 && y <= 17 && x >= 8 && x <= 23 {
        // Notched cup profile at the top
        if y == 4 && !((10..=12).contains(&x) || (15..=17).contains(&x) || (20..=21).contains(&x)) {
            return None;
        }
        if y == 5 && (x == 8 || x == 13 || x == 19 || x == 23) {
            return None;
        }
        let is_tip = y <= 6;
        let is_shadow = (12..=14).contains(&x) || y >= 15;
        let c = if is_tip {
            hi_rgb
        } else if is_shadow {
            shadow_rgb
        } else {
            main_rgb
        };
        let grain = ((x * 3 + y * 2) % 5) - 2;
        return Some(Rgba([
            (c[0] as i32 + grain * 4).clamp(0, 255) as u8,
            (c[1] as i32 + grain * 3).clamp(0, 255) as u8,
            (c[2] as i32 + grain * 2).clamp(0, 255) as u8,
            255,
        ]));
    }
    // Broad wrap-around tulip leaves (rows 16..=31, cols 6..=26)
    let is_left_leaf = x >= 6 && x <= 14 && y >= 16 && y <= 31 && (y - x <= 20) && (x + y >= 25);
    let is_right_leaf = x >= 17 && x <= 25 && y >= 17 && y <= 31 && (x - y <= 4) && (x + y <= 52);
    if is_left_leaf || is_right_leaf {
        let vein = if (x == 10 && y >= 18) || (x == 21 && y >= 19) { 24 } else { 0 };
        return Some(Rgba([
            (62 + vein).clamp(45, 110) as u8,
            (145 + vein).clamp(95, 185) as u8,
            (50 + vein / 2).clamp(35, 80) as u8,
            255,
        ]));
    }
    // Upright stem (cols 14..=17, rows 16..=31 to ground)
    if x >= 14 && x <= 17 && y >= 16 {
        let shade = if x == 14 { -16 } else if x == 17 { 16 } else { 0 };
        return Some(Rgba([
            (60 + shade).clamp(42, 95) as u8,
            (138 + shade).clamp(95, 175) as u8,
            (46 + shade / 2).clamp(30, 75) as u8,
            255,
        ]));
    }
    None
}

fn paint_fishing_tiles(atlas: &mut TextureAtlas) {
    // 384..=387: Fishing Rods (Wooden, Copper, Iron, Diamond)
    let rod_colors = [
        ([165, 115, 65], [110, 72, 38], [210, 210, 210]),  // Wooden
        ([215, 125, 75], [155, 80, 45], [235, 195, 130]),  // Copper
        ([200, 208, 218], [135, 142, 155], [220, 235, 245]), // Iron
        ([85, 225, 215], [40, 165, 155], [250, 225, 95]),  // Diamond
    ];
    for (idx, (main_c, dark_c, reel_c)) in rod_colors.iter().enumerate() {
        atlas.paint_tile(384 + idx as u16, |px, py| {
            let x = px as i32;
            let y = py as i32;
            // Diagonal rod shaft from bottom-left (4,27) to top-right (25,5)
            let dx = x - 4;
            let dy = 27 - y;
            if dx >= 0 && dx <= 21 && (dx - dy).abs() <= 1 && y >= 5 && y <= 27 {
                if (dx - dy) == 1 {
                    return Rgba([dark_c[0], dark_c[1], dark_c[2], 255]);
                }
                return Rgba([main_c[0], main_c[1], main_c[2], 255]);
            }
            // Handle grip at bottom-left
            if x >= 3 && x <= 8 && y >= 23 && y <= 28 && (x + y - 31).abs() <= 2 {
                return Rgba([75, 48, 28, 255]);
            }
            // Reel near handle
            if (x - 10).abs() <= 2 && (y - 22).abs() <= 2 {
                return Rgba([reel_c[0], reel_c[1], reel_c[2], 255]);
            }
            // Hanging fishing line from tip (25,5) down to hook at (25,21)
            if x == 25 && y >= 6 && y <= 19 {
                return Rgba([225, 238, 250, 220]);
            }
            // Red/white mini bobber at bottom of line
            if (x - 25).abs() <= 1 && y >= 20 && y <= 22 {
                return if y == 20 {
                    Rgba([235, 55, 55, 255])
                } else {
                    Rgba([245, 245, 245, 255])
                };
            }
            Rgba([0, 0, 0, 0])
        });
    }

    // 388..=390: Fishing Lines (Braided, Fluorocarbon, Titanium)
    let line_colors = [
        ([70, 175, 85], [40, 115, 50]),   // Braided green
        ([95, 195, 245], [55, 135, 195]), // Fluorocarbon blue
        ([205, 215, 228], [130, 140, 155]), // Titanium silver
    ];
    for (idx, (c1, c2)) in line_colors.iter().enumerate() {
        atlas.paint_tile(388 + idx as u16, |px, py| {
            let x = px as i32;
            let y = py as i32;
            // Spool flanges top & bottom
            if x >= 7 && x <= 24 && (y == 7 || y == 8 || y == 23 || y == 24) {
                return Rgba([130, 92, 54, 255]);
            }
            // Wound line core
            if x >= 9 && x <= 22 && y >= 9 && y <= 22 {
                return if (x + y) % 2 == 0 {
                    Rgba([c1[0], c1[1], c1[2], 255])
                } else {
                    Rgba([c2[0], c2[1], c2[2], 255])
                };
            }
            // Loose line tail
            if x >= 23 && x <= 27 && y == 14 + (x - 23) / 2 {
                return Rgba([c1[0], c1[1], c1[2], 255]);
            }
            Rgba([0, 0, 0, 0])
        });
    }

    // 391: Cork Bobber Lure
    atlas.paint_tile(391, |px, py| {
        let x = px as i32 - 16;
        let y = py as i32 - 16;
        let dist2 = x * x + y * y;
        if dist2 <= 64 {
            if y < 0 {
                Rgba([230, 52, 48, 255])
            } else {
                Rgba([242, 238, 225, 255])
            }
        } else if (x == 0 && y >= -12 && y <= -8) || (x == 0 && y >= 9 && y <= 12) {
            Rgba([160, 120, 70, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });

    // 392: Copper Spinner Lure
    atlas.paint_tile(392, |px, py| {
        let x = px as i32;
        let y = py as i32;
        if (x - 16).abs() <= 4 && y >= 7 && y <= 17 && (x - 16).abs() + (y - 12).abs() / 2 <= 4 {
            return Rgba([235, 145, 85, 255]);
        }
        if x == 16 && y >= 17 && y <= 24 {
            return Rgba([180, 185, 195, 255]);
        }
        if y >= 22 && y <= 26 && (x == 14 || x == 18) {
            return Rgba([230, 75, 55, 255]);
        }
        Rgba([0, 0, 0, 0])
    });

    // 393: Barbed Hook Lure
    atlas.paint_tile(393, |px, py| {
        let x = px as i32;
        let y = py as i32;
        // Eyelets & shaft
        if (x - 18).abs() <= 1 && y >= 6 && y <= 20 {
            return Rgba([195, 202, 212, 255]);
        }
        // Bottom J-curve
        if y >= 20 && y <= 24 && x >= 11 && x <= 18 && (y == 23 || y == 24 || x == 11 || x == 12) {
            return Rgba([195, 202, 212, 255]);
        }
        // Barb point
        if x >= 11 && x <= 14 && y >= 16 && y <= 20 && (x - 11 == 20 - y || x == 11) {
            return Rgba([230, 236, 245, 255]);
        }
        Rgba([0, 0, 0, 0])
    });

    // 394: Gold Treasure Magnet
    atlas.paint_tile(394, |px, py| {
        let x = px as i32 - 16;
        let y = py as i32 - 16;
        // Horseshoe magnet shape
        if y >= -8 && y <= 8 && (x.abs() >= 5 && x.abs() <= 9) {
            return if y >= 4 {
                Rgba([220, 228, 238, 255])
            } else {
                Rgba([245, 205, 55, 255])
            };
        }
        if y >= -11 && y <= -7 && x.abs() <= 9 {
            return Rgba([245, 205, 55, 255]);
        }
        // Diamond sparkle in center
        if x.abs() + y.abs() <= 3 {
            return Rgba([85, 235, 225, 255]);
        }
        Rgba([0, 0, 0, 0])
    });

    // 395: Fishing Bait
    atlas.paint_tile(395, |px, py| {
        let x = px as i32;
        let y = py as i32;
        // Wiggly pink/brown worm bait
        let curve = ((y - 16) as f32 * 0.35).sin() * 3.5;
        if y >= 7 && y <= 25 && ((x as f32) - (16.0 + curve)).abs() <= 2.3 {
            return if y % 3 == 0 {
                Rgba([195, 105, 115, 255])
            } else {
                Rgba([230, 140, 150, 255])
            };
        }
        Rgba([0, 0, 0, 0])
    });

    // 396..=405: 10 Distinct Fish Sprites
    let fish_palettes: [([u8; 3], [u8; 3], [u8; 3]); 10] = [
        ([178, 138, 86], [220, 192, 138], [140, 96, 52]),  // 396: Carp (warm bronze/gold)
        ([150, 185, 220], [220, 232, 245], [95, 130, 175]), // 397: Sardine (silver-blue)
        ([125, 165, 85], [225, 210, 135], [205, 95, 55]),   // 398: Perch (olive & orange fins)
        ([135, 175, 155], [235, 145, 155], [85, 120, 105]), // 399: Trout (rainbow stripe)
        ([75, 135, 70], [185, 215, 165], [50, 95, 48]),     // 400: Bass (largemouth green)
        ([195, 105, 95], [235, 165, 145], [105, 145, 135]), // 401: Salmon (pink-red & teal)
        ([95, 98, 112], [165, 168, 175], [65, 68, 82]),     // 402: Catfish (slate grey)
        ([235, 205, 95], [248, 242, 210], [175, 140, 45]),  // 403: Pufferfish (spiky yellow)
        ([55, 95, 165], [205, 222, 245], [235, 195, 65]),   // 404: Tuna (deep ocean blue)
        ([250, 205, 45], [255, 245, 165], [230, 85, 35]),   // 405: Golden Koi (gold & crimson)
    ];

    for (idx, (body_c, belly_c, fin_c)) in fish_palettes.iter().enumerate() {
        let is_puffer = idx == 7;
        atlas.paint_tile(396 + idx as u16, |px, py| {
            let x = px as i32 - 16;
            let y = py as i32 - 16;
            if is_puffer {
                // Round spiky pufferfish
                let r2 = x * x + y * y;
                if r2 <= 56 {
                    if x == 4 && y == -2 {
                        return Rgba([20, 20, 25, 255]);
                    }
                    if y > 2 {
                        return Rgba([belly_c[0], belly_c[1], belly_c[2], 255]);
                    }
                    return Rgba([body_c[0], body_c[1], body_c[2], 255]);
                }
                if r2 <= 85 && (x.abs() == y.abs() || x == 0 || y == 0) {
                    return Rgba([fin_c[0], fin_c[1], fin_c[2], 255]);
                }
                return Rgba([0, 0, 0, 0]);
            }

            // Standard sleek fish silhouette facing right
            // Body ellipse centered at (1, 0), rx=9, ry=5
            let bx = (x - 1) as f32 / 9.0;
            let by = y as f32 / 5.2;
            if bx * bx + by * by <= 1.0 {
                // Eye
                if (x - 6).abs() <= 1 && (y + 1).abs() <= 1 {
                    return if x == 6 && y == -1 {
                        Rgba([15, 15, 20, 255])
                    } else {
                        Rgba([245, 245, 245, 255])
                    };
                }
                // Belly vs back
                if y >= 1 {
                    return Rgba([belly_c[0], belly_c[1], belly_c[2], 255]);
                }
                return Rgba([body_c[0], body_c[1], body_c[2], 255]);
            }
            // Tail fin on left (-12..-8)
            if x >= -12 && x <= -7 && y.abs() <= (-7 - x) + 1 {
                return Rgba([fin_c[0], fin_c[1], fin_c[2], 255]);
            }
            // Dorsal fin on top
            if x >= -3 && x <= 3 && y >= -8 && y <= -5 && (x + 3) >= (-5 - y) {
                return Rgba([fin_c[0], fin_c[1], fin_c[2], 255]);
            }
            Rgba([0, 0, 0, 0])
        });
    }

    // 406: Cooked Fish Fillet
    atlas.paint_tile(406, |px, py| {
        let x = px as i32 - 16;
        let y = py as i32 - 16;
        let bx = x as f32 / 10.0;
        let by = y as f32 / 5.5;
        if bx * bx + by * by <= 1.0 {
            if (x + y) % 5 == 0 {
                Rgba([135, 78, 35, 255]) // Grill marks
            } else {
                Rgba([218, 158, 92, 255])
            }
        } else {
            Rgba([0, 0, 0, 0])
        }
    });

    // 407: Cooked Salmon
    atlas.paint_tile(407, |px, py| {
        let x = px as i32 - 16;
        let y = py as i32 - 16;
        let bx = x as f32 / 10.0;
        let by = y as f32 / 5.5;
        if bx * bx + by * by <= 1.0 {
            if (x - y) % 5 == 0 {
                Rgba([165, 72, 45, 255])
            } else {
                Rgba([242, 138, 102, 255])
            }
        } else {
            Rgba([0, 0, 0, 0])
        }
    });

    // 408: Sushi Roll
    atlas.paint_tile(408, |px, py| {
        let x = px as i32 - 16;
        let y = py as i32 - 16;
        let r2 = x * x + y * y;
        if r2 <= 64 {
            if r2 <= 12 {
                Rgba([240, 115, 95, 255]) // Salmon center
            } else if r2 <= 42 {
                Rgba([245, 244, 238, 255]) // Rice ring
            } else {
                Rgba([38, 68, 42, 255]) // Seaweed nori wrap
            }
        } else {
            Rgba([0, 0, 0, 0])
        }
    });

    // 409: Fisherman's Stew
    atlas.paint_tile(409, |px, py| {
        let x = px as i32 - 16;
        let y = py as i32 - 16;
        // Wooden bowl bottom
        if y >= 0 && y <= 9 && x.abs() <= 11 - (y / 2) {
            return Rgba([145, 92, 48, 255]);
        }
        // Creamy broth top
        if y >= -3 && y <= 1 && x.abs() <= 10 {
            if (x * x + y * y) % 7 == 0 {
                return Rgba([235, 135, 95, 255]); // Fish chunks
            }
            return Rgba([235, 195, 125, 255]);
        }
        // Rising steam
        if y >= -10 && y <= -5 && (x == -3 || x == 3) {
            return Rgba([235, 240, 245, 175]);
        }
        Rgba([0, 0, 0, 0])
    });
}

/// Paints handcrafted 32x32 textures for the Power & Electric Machine system (tiles 410..=427).
fn paint_power_tiles(atlas: &mut TextureAtlas) {
    let hash = |x: u32, y: u32, seed: u32| -> i32 {
        let mut v = x.wrapping_mul(374761393)
            ^ y.wrapping_mul(668265263)
            ^ seed.wrapping_mul(1442695041);
        v = (v ^ (v >> 13)).wrapping_mul(1274126177);
        ((v ^ (v >> 16)) & 0x7FFF) as i32
    };
    let clamp = |base: i32, d: i32| -> u8 { (base + d).clamp(0, 255) as u8 };

    // 410: Copper Cable (insulated dark-rubber jacket with gleaming copper bands)
    atlas.paint_tile(410, |px, py| {
        let n = (hash(px, py, 410) % 9) - 4;
        let is_band = (px % 8 <= 2) || (py % 8 <= 2);
        let is_rim = px < 2 || px >= 30 || py < 2 || py >= 30;
        if is_rim {
            Rgba([clamp(38, n), clamp(40, n), clamp(46, n), 255])
        } else if is_band {
            let sheen = if (px + py) % 5 == 0 { 22 } else { 0 };
            Rgba([clamp(218, n + sheen), clamp(118, n + sheen / 2), clamp(66, n), 255])
        } else {
            Rgba([clamp(56, n), clamp(58, n), clamp(66, n), 255])
        }
    });

    // 411: Coal Generator Front (dark steel housing, glowing orange firebox grate, copper coils)
    atlas.paint_tile(411, |px, py| {
        let n = (hash(px, py, 411) % 11) - 5;
        let is_border = px < 3 || px >= 29 || py < 3 || py >= 29;
        if is_border {
            let rivet = ((px == 1 || px == 30) && (py % 8 == 4)) || ((py == 1 || py == 30) && (px % 8 == 4));
            let d = if rivet { 28 } else { n };
            return Rgba([clamp(62, d), clamp(66, d), clamp(74, d), 255]);
        }
        // Top copper coil window (py 5..12)
        if py >= 5 && py <= 12 && px >= 5 && px <= 26 {
            if py == 5 || py == 12 || px == 5 || px == 26 {
                return Rgba([34, 36, 42, 255]);
            }
            let coil = if px % 3 == 0 { -22 } else { 18 };
            return Rgba([clamp(212, coil + n), clamp(112, coil / 2 + n), clamp(60, n), 255]);
        }
        // Lower firebox grate with warm amber embers (py 16..26)
        if py >= 16 && py <= 26 && px >= 6 && px <= 25 {
            if py % 3 == 0 || px == 6 || px == 25 {
                return Rgba([32, 34, 38, 255]); // Iron grate bars
            }
            let glow = if py >= 20 { 25 } else { 0 };
            return Rgba([clamp(235, glow + n), clamp(120, glow + n), clamp(32, n), 255]);
        }
        Rgba([clamp(88, n), clamp(92, n), clamp(102, n), 255])
    });

    // 412: Coal Generator Side/Top (riveted steel machine plates + copper vents)
    atlas.paint_tile(412, |px, py| {
        let n = (hash(px, py, 412) % 11) - 5;
        let is_border = px < 3 || px >= 29 || py < 3 || py >= 29;
        if is_border {
            return Rgba([clamp(60, n), clamp(64, n), clamp(72, n), 255]);
        }
        if py >= 9 && py <= 22 && px >= 7 && px <= 24 && py % 4 == 0 {
            return Rgba([clamp(196, n), clamp(104, n), clamp(56, n), 255]);
        }
        Rgba([clamp(92, n), clamp(96, n), clamp(106, n), 255])
    });

    // 413: Solar Panel Top (deep cobalt photovoltaic grid with silver busbars & aluminum frame)
    atlas.paint_tile(413, |px, py| {
        let n = (hash(px, py, 413) % 9) - 4;
        if px < 2 || px >= 30 || py < 2 || py >= 30 {
            return Rgba([clamp(178, n), clamp(186, n), clamp(198, n), 255]); // Aluminum frame
        }
        if px == 15 || px == 16 || py == 15 || py == 16 || px % 8 == 0 || py % 8 == 0 {
            return Rgba([clamp(192, n), clamp(206, n), clamp(222, n), 255]); // Silver busbars
        }
        let shimmer = if (px + py) % 9 == 0 { 16 } else { 0 };
        Rgba([clamp(22, n + shimmer), clamp(52, n + shimmer), clamp(138, n + shimmer), 255])
    });

    // 414: Solar Panel Side (anodized steel frame + copper terminal)
    atlas.paint_tile(414, |px, py| {
        let n = (hash(px, py, 414) % 9) - 4;
        if py >= 12 && py <= 19 && px >= 12 && px <= 19 {
            return Rgba([clamp(210, n), clamp(116, n), clamp(64, n), 255]);
        }
        Rgba([clamp(76, n), clamp(82, n), clamp(94, n), 255])
    });

    // 415: Wind Turbine Front/Hub (silver-white aerodynamic nacelle + copper stator + spinner hub)
    atlas.paint_tile(415, |px, py| {
        let n = (hash(px, py, 415) % 9) - 4;
        let dx = px as i32 - 16;
        let dy = py as i32 - 16;
        let d2 = dx * dx + dy * dy;
        if d2 <= 20 {
            return Rgba([clamp(58, n), clamp(64, n), clamp(74, n), 255]); // Central rotor hub
        }
        if d2 <= 56 {
            return Rgba([clamp(215, n), clamp(118, n), clamp(62, n), 255]); // Copper stator ring
        }
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            return Rgba([clamp(138, n), clamp(144, n), clamp(154, n), 255]);
        }
        Rgba([clamp(218, n), clamp(224, n), clamp(232, n), 255])
    });

    // 416: Wind Turbine Side/Blade (brushed light alloy)
    atlas.paint_tile(416, |px, py| {
        let n = (hash(px, py, 416) % 9) - 4;
        let stripe = if px < 2 || px >= 30 || py < 2 || py >= 30 { -34 } else if py == 15 || py == 16 { -18 } else { 0 };
        Rgba([clamp(212, n + stripe), clamp(218, n + stripe), clamp(226, n + stripe), 255])
    });

    // 419: Battery Box Front (industrial steel cell + bright emerald LED charge bars + copper terminals)
    atlas.paint_tile(419, |px, py| {
        let n = (hash(px, py, 419) % 9) - 4;
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            return Rgba([clamp(56, n), clamp(60, n), clamp(68, n), 255]);
        }
        // Top & bottom copper bus terminals
        if (py <= 5 || py >= 26) && px >= 10 && px <= 21 {
            return Rgba([clamp(218, n), clamp(118, n), clamp(62, n), 255]);
        }
        // Central LED charge gauge recess (px 8..23, py 8..23)
        if px >= 8 && px <= 23 && py >= 8 && py <= 23 {
            if px == 8 || px == 23 || py == 8 || py == 23 {
                return Rgba([24, 28, 34, 255]);
            }
            if py % 4 == 0 {
                return Rgba([28, 34, 38, 255]); // Segment divider
            }
            return Rgba([clamp(52, n), clamp(228, n), clamp(118, n), 255]); // Glowing green energy cells
        }
        Rgba([clamp(82, n), clamp(88, n), clamp(98, n), 255])
    });

    // 420: Battery Box Side/Top (dark steel + yellow/black hazard trim + copper terminal)
    atlas.paint_tile(420, |px, py| {
        let n = (hash(px, py, 420) % 9) - 4;
        if py >= 25 && py <= 28 && px >= 3 && px <= 28 {
            let stripe = ((px + py) / 4) % 2 == 0;
            return if stripe {
                Rgba([228, 186, 36, 255])
            } else {
                Rgba([34, 36, 40, 255])
            };
        }
        if px >= 12 && px <= 19 && py >= 12 && py <= 19 {
            return Rgba([clamp(214, n), clamp(116, n), clamp(62, n), 255]);
        }
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            return Rgba([clamp(54, n), clamp(58, n), clamp(66, n), 255]);
        }
        Rgba([clamp(84, n), clamp(90, n), clamp(100, n), 255])
    });

    // 421: Electric Furnace Front (brushed steel + glowing cyan/amber heating coils + status bar)
    atlas.paint_tile(421, |px, py| {
        let n = (hash(px, py, 421) % 9) - 4;
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            return Rgba([clamp(70, n), clamp(76, n), clamp(88, n), 255]);
        }
        // Top digital status LED strip
        if py >= 5 && py <= 8 && px >= 7 && px <= 24 {
            if px <= 14 {
                return Rgba([58, 220, 245, 255]); // Cyan power indicator
            }
            return Rgba([34, 42, 52, 255]);
        }
        // Glowing electric heating chamber (py 12..25, px 6..25)
        if py >= 12 && py <= 25 && px >= 6 && px <= 25 {
            if py == 12 || py == 25 || px == 6 || px == 25 {
                return Rgba([30, 34, 40, 255]);
            }
            if py == 16 || py == 21 {
                return Rgba([255, 162, 52, 255]); // Glowing heating coils
            }
            return Rgba([clamp(46, n), clamp(52, n), clamp(64, n), 255]);
        }
        Rgba([clamp(118, n), clamp(126, n), clamp(138, n), 255])
    });

    // 422: Electric Furnace Side/Top (brushed steel + copper heat sinks)
    atlas.paint_tile(422, |px, py| {
        let n = (hash(px, py, 422) % 9) - 4;
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            return Rgba([clamp(72, n), clamp(78, n), clamp(90, n), 255]);
        }
        if px >= 8 && px <= 23 && py >= 8 && py <= 23 && px % 4 == 0 {
            return Rgba([clamp(205, n), clamp(112, n), clamp(62, n), 255]);
        }
        Rgba([clamp(124, n), clamp(132, n), clamp(144, n), 255])
    });

    // 423: Electric Lamp Off (frosted unlit glass inside dark iron cage)
    atlas.paint_tile(423, |px, py| {
        let n = (hash(px, py, 423) % 9) - 4;
        let is_cage = px < 3 || px >= 29 || py < 3 || py >= 29 || px == 15 || px == 16 || py == 15 || py == 16;
        if is_cage {
            return Rgba([clamp(54, n), clamp(58, n), clamp(66, n), 255]);
        }
        Rgba([clamp(142, n), clamp(150, n), clamp(160, n), 255])
    });

    // 424: Electric Lamp Lit (radiant warm-white glowing glass inside dark iron cage)
    atlas.paint_tile(424, |px, py| {
        let n = (hash(px, py, 424) % 7) - 3;
        let is_cage = px < 3 || px >= 29 || py < 3 || py >= 29 || px == 15 || px == 16 || py == 15 || py == 16;
        if is_cage {
            return Rgba([clamp(64, n), clamp(62, n), clamp(60, n), 255]);
        }
        let dx = (px as i32 - 16).abs();
        let dy = (py as i32 - 16).abs();
        let bright = if dx < 10 && dy < 10 { 12 } else { -8 };
        Rgba([clamp(252, bright + n), clamp(244, bright + n), clamp(204, bright + n), 255])
    });

    // 425: Machine Base Plate (dark riveted industrial steel)
    atlas.paint_tile(425, |px, py| {
        let n = (hash(px, py, 425) % 9) - 4;
        let rim = if px < 3 || px >= 29 || py < 3 || py >= 29 { -14 } else { 0 };
        Rgba([clamp(58, n + rim), clamp(62, n + rim), clamp(70, n + rim), 255])
    });

    // 426: Copper Wire Item Icon (coiled gleaming orange copper wire on transparent background)
    atlas.paint_tile(426, |px, py| {
        let dx = px as f32 - 15.5;
        let dy = py as f32 - 15.5;
        let r = (dx * dx + dy * dy).sqrt();
        // Coil ring + loose copper wire ends
        let in_coil = (r >= 6.2 && r <= 11.2) || (r >= 3.2 && r <= 5.0);
        let in_lead = (py >= 22 && py <= 26 && px >= 18 && px <= 26)
            || (py >= 6 && py <= 9 && px >= 5 && px <= 13);
        if !in_coil && !in_lead {
            return Rgba([0, 0, 0, 0]);
        }
        let is_edge = (in_coil && ((r - 6.2).abs() < 0.9 || (11.2 - r).abs() < 0.9))
            || (in_lead && (py == 22 || py == 26 || py == 6 || py == 9));
        if is_edge {
            return Rgba([112, 48, 22, 255]);
        }
        let hi = if dx + dy < -2.0 { 26 } else if dx + dy > 4.0 { -22 } else { 0 };
        Rgba([clamp(228, hi), clamp(122, hi / 2), clamp(64, hi / 3), 255])
    });

    // 427: Power Core / Electronic Circuit Item Icon (green PCB + gold traces + glowing cyan core)
    atlas.paint_tile(427, |px, py| {
        let x = px as i32;
        let y = py as i32;
        // Copper connector pins on left/right
        let is_pin = (x >= 3 && x <= 5 || x >= 26 && x <= 28) && (y == 10 || y == 16 || y == 22);
        if is_pin {
            return Rgba([228, 128, 66, 255]);
        }
        if x < 6 || x > 25 || y < 6 || y > 25 {
            return Rgba([0, 0, 0, 0]);
        }
        if x == 6 || x == 25 || y == 6 || y == 25 {
            return Rgba([22, 58, 34, 255]); // Dark PCB outline
        }
        // Central glowing cyan power core
        if x >= 12 && x <= 19 && y >= 12 && y <= 19 {
            if x == 12 || x == 19 || y == 12 || y == 19 {
                return Rgba([235, 195, 52, 255]); // Gold core frame
            }
            let hi = if x <= 15 && y <= 15 { 24 } else { -12 };
            return Rgba([clamp(76, hi), clamp(228, hi), clamp(248, hi), 255]);
        }
        // Gold circuit traces radiating to pins
        if y == 10 || y == 16 || y == 22 || x == 15 || x == 16 {
            return Rgba([232, 192, 54, 255]);
        }
        // Green PCB substrate
        let n = (hash(px, py, 427) % 9) - 4;
        Rgba([clamp(38, n), clamp(126, n), clamp(64, n), 255])
    });

    // 428: Advanced Solar Panel Top (deep violet-indigo monocrystalline cells + gleaming gold busbars & frame)
    atlas.paint_tile(428, |px, py| {
        let n = (hash(px, py, 428) % 9) - 4;
        if px < 2 || px >= 30 || py < 2 || py >= 30 {
            return Rgba([clamp(218, n), clamp(174, n), clamp(52, n), 255]); // Gold alloy frame
        }
        if px == 15 || px == 16 || py == 15 || py == 16 || px % 8 == 0 || py % 8 == 0 {
            return Rgba([clamp(238, n), clamp(202, n), clamp(78, n), 255]); // Gold busbars
        }
        let shimmer = if (px + py) % 7 == 0 { 22 } else { 0 };
        Rgba([clamp(42, n + shimmer), clamp(28, n + shimmer), clamp(118, n + shimmer), 255])
    });

    // 429: Advanced Solar Panel Side (dark steel + gold trim + copper terminal)
    atlas.paint_tile(429, |px, py| {
        let n = (hash(px, py, 429) % 9) - 4;
        if py <= 3 || py >= 28 {
            return Rgba([clamp(212, n), clamp(168, n), clamp(48, n), 255]);
        }
        if py >= 12 && py <= 19 && px >= 12 && px <= 19 {
            return Rgba([clamp(228, n), clamp(126, n), clamp(64, n), 255]);
        }
        Rgba([clamp(54, n), clamp(58, n), clamp(74, n), 255])
    });

    // 430: Advanced Battery Box Front (gold-trimmed steel + bright cyan-blue LED charge bars)
    atlas.paint_tile(430, |px, py| {
        let n = (hash(px, py, 430) % 9) - 4;
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            return Rgba([clamp(214, n), clamp(170, n), clamp(50, n), 255]); // Gold frame
        }
        if (py <= 5 || py >= 26) && px >= 10 && px <= 21 {
            return Rgba([clamp(222, n), clamp(120, n), clamp(64, n), 255]); // Copper busbars
        }
        if px >= 8 && px <= 23 && py >= 8 && py <= 23 {
            if px == 8 || px == 23 || py == 8 || py == 23 {
                return Rgba([20, 26, 36, 255]);
            }
            if py % 4 == 0 {
                return Rgba([24, 34, 46, 255]);
            }
            return Rgba([clamp(46, n), clamp(212, n), clamp(248, n), 255]); // Glowing cyan cells
        }
        Rgba([clamp(68, n), clamp(76, n), clamp(92, n), 255])
    });

    // 431: Advanced Battery Box Side/Top (dark steel + gold trim + copper terminal)
    atlas.paint_tile(431, |px, py| {
        let n = (hash(px, py, 431) % 9) - 4;
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            return Rgba([clamp(206, n), clamp(164, n), clamp(48, n), 255]);
        }
        if px >= 12 && px <= 19 && py >= 12 && py <= 19 {
            return Rgba([clamp(46, n), clamp(208, n), clamp(244, n), 255]);
        }
        Rgba([clamp(64, n), clamp(72, n), clamp(88, n), 255])
    });

    // 432: Quantum Battery Box Front (obsidian-alloy + radiant violet/diamond-cyan plasma core bars)
    atlas.paint_tile(432, |px, py| {
        let n = (hash(px, py, 432) % 9) - 4;
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            let corner = (px < 5 || px >= 27) && (py < 5 || py >= 27);
            if corner {
                return Rgba([clamp(92, n), clamp(238, n), clamp(245, n), 255]); // Diamond corner nodes
            }
            return Rgba([clamp(34, n), clamp(28, n), clamp(48, n), 255]);
        }
        if (py <= 5 || py >= 26) && px >= 10 && px <= 21 {
            return Rgba([clamp(108, n), clamp(242, n), clamp(248, n), 255]); // Diamond conduits
        }
        if px >= 7 && px <= 24 && py >= 7 && py <= 24 {
            if px == 7 || px == 24 || py == 7 || py == 24 {
                return Rgba([18, 14, 28, 255]);
            }
            if py % 4 == 0 {
                return Rgba([28, 20, 44, 255]);
            }
            let pulse = if (px + py) % 6 < 2 { 24 } else { 0 };
            return Rgba([clamp(192, n + pulse), clamp(86, n + pulse), clamp(250, n), 255]); // Radiant violet plasma
        }
        Rgba([clamp(46, n), clamp(40, n), clamp(66, n), 255])
    });

    // 433: Quantum Battery Box Side/Top (obsidian-alloy + diamond-cyan trim + violet core port)
    atlas.paint_tile(433, |px, py| {
        let n = (hash(px, py, 433) % 9) - 4;
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            return Rgba([clamp(76, n), clamp(218, n), clamp(232, n), 255]);
        }
        if px >= 12 && px <= 19 && py >= 12 && py <= 19 {
            return Rgba([clamp(186, n), clamp(82, n), clamp(248, n), 255]);
        }
        Rgba([clamp(42, n), clamp(36, n), clamp(60, n), 255])
    });

    // 434: Gold Cable (Tier 2: dark rubber insulation + gleaming gold conductor bands)
    atlas.paint_tile(434, |px, py| {
        let n = (hash(px, py, 434) % 9) - 4;
        let is_band = (px % 8 <= 2) || (py % 8 <= 2);
        let is_rim = px < 2 || px >= 30 || py < 2 || py >= 30;
        if is_rim {
            Rgba([clamp(36, n), clamp(36, n), clamp(44, n), 255])
        } else if is_band {
            let sheen = if (px + py) % 5 == 0 { 24 } else { 0 };
            Rgba([clamp(236, n + sheen), clamp(192, n + sheen / 2), clamp(46, n), 255])
        } else {
            Rgba([clamp(50, n), clamp(52, n), clamp(62, n), 255])
        }
    });

    // 435: Diamond Cable (Tier 3: obsidian insulation + glowing diamond-cyan superconductive bands)
    atlas.paint_tile(435, |px, py| {
        let n = (hash(px, py, 435) % 9) - 4;
        let is_band = (px % 8 <= 2) || (py % 8 <= 2);
        let is_rim = px < 2 || px >= 30 || py < 2 || py >= 30;
        if is_rim {
            Rgba([clamp(26, n), clamp(24, n), clamp(38, n), 255])
        } else if is_band {
            let sheen = if (px + py) % 5 == 0 { 26 } else { 0 };
            Rgba([clamp(76, n + sheen), clamp(232, n + sheen / 2), clamp(244, n), 255])
        } else {
            Rgba([clamp(38, n), clamp(36, n), clamp(56, n), 255])
        }
    });

    // Helper for metallic chest tiles (front with latch vs side/top)
    let paint_chest = |atlas: &mut TextureAtlas,
                       tile_id: u16,
                       base: [i32; 3],
                       border: [i32; 3],
                       latch: [i32; 3],
                       is_front: bool| {
        atlas.paint_tile(tile_id, |px, py| {
            let n = (hash(px, py, tile_id as u32) % 11) - 5;
            if is_front && px >= 13 && px <= 18 && py >= 10 && py <= 17 {
                let rim = if px == 13 || px == 18 || py == 10 || py == 17 { -30 } else { 15 };
                return Rgba([
                    clamp(latch[0], rim + n / 2),
                    clamp(latch[1], rim + n / 2),
                    clamp(latch[2], rim + n / 2),
                    255,
                ]);
            }
            let is_seam = py == 11 || py == 12;
            let is_border = px < 3 || px >= 29 || py < 3 || py >= 29;
            if is_seam {
                return Rgba([clamp(border[0] - 28, n), clamp(border[1] - 28, n), clamp(border[2] - 28, n), 255]);
            }
            if is_border {
                let rivet = ((px == 1 || px == 30) && (py % 8 == 4)) || ((py == 1 || py == 30) && (px % 8 == 4));
                let d = if rivet { 28 } else { n };
                return Rgba([clamp(border[0], d), clamp(border[1], d), clamp(border[2], d), 255]);
            }
            let hi = if px < 14 && py < 11 { 12 } else { 0 };
            Rgba([clamp(base[0], n + hi), clamp(base[1], n + hi), clamp(base[2], n + hi), 255])
        });
    };

    // 436..=441: Iron, Gold, and Diamond Chests
    paint_chest(atlas, 436, [186, 194, 206], [116, 122, 134], [238, 212, 92], true);
    paint_chest(atlas, 437, [186, 194, 206], [116, 122, 134], [238, 212, 92], false);
    paint_chest(atlas, 438, [238, 194, 46], [166, 118, 22], [92, 232, 245], true);
    paint_chest(atlas, 439, [238, 194, 46], [166, 118, 22], [92, 232, 245], false);
    paint_chest(atlas, 440, [68, 218, 226], [38, 92, 122], [238, 242, 250], true);
    paint_chest(atlas, 441, [68, 218, 226], [38, 92, 122], [238, 242, 250], false);

    // 442: Storage Terminal Front (dark console + glowing teal digital inventory screen)
    atlas.paint_tile(442, |px, py| {
        let n = (hash(px, py, 442) % 9) - 4;
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            return Rgba([clamp(56, n), clamp(64, n), clamp(76, n), 255]);
        }
        // Screen bezel & glowing teal item-grid display (px 5..26, py 5..21)
        if px >= 5 && px <= 26 && py >= 5 && py <= 21 {
            if px == 5 || px == 26 || py == 5 || py == 21 {
                return Rgba([22, 28, 36, 255]);
            }
            let grid_line = (px - 6) % 5 == 0 || (py - 6) % 5 == 0;
            if grid_line {
                return Rgba([32, 110, 128, 255]);
            }
            let cell_lit = ((px / 5) + (py / 5)) % 2 == 0;
            if cell_lit {
                return Rgba([clamp(62, n), clamp(224, n), clamp(238, n), 255]);
            }
            return Rgba([clamp(28, n), clamp(78, n), clamp(98, n), 255]);
        }
        // Lower keyboard / status LED strip (py 24..27)
        if py >= 24 && py <= 27 && px >= 6 && px <= 25 {
            if px <= 10 {
                return Rgba([58, 236, 138, 255]); // Online LED
            }
            let key = if px % 3 == 0 { 20 } else { -12 };
            return Rgba([clamp(46, key), clamp(54, key), clamp(66, key), 255]);
        }
        Rgba([clamp(82, n), clamp(90, n), clamp(104, n), 255])
    });

    // 443: Storage Terminal Side/Top (dark steel console casing + teal data stripe)
    atlas.paint_tile(443, |px, py| {
        let n = (hash(px, py, 443) % 9) - 4;
        if px < 3 || px >= 29 || py < 3 || py >= 29 {
            return Rgba([clamp(54, n), clamp(62, n), clamp(74, n), 255]);
        }
        if py == 15 || py == 16 {
            return Rgba([clamp(52, n), clamp(210, n), clamp(228, n), 255]);
        }
        Rgba([clamp(80, n), clamp(88, n), clamp(102, n), 255])
    });

    // 444: Storage Cable (dark green conduit with glowing emerald/lime data fiber bands)
    atlas.paint_tile(444, |px, py| {
        let n = (hash(px, py, 444) % 9) - 4;
        let is_band = (px % 8 <= 2) || (py % 8 <= 2);
        let is_rim = px < 2 || px >= 30 || py < 2 || py >= 30;
        if is_rim {
            Rgba([clamp(20, n), clamp(52, n), clamp(26, n), 255])
        } else if is_band {
            Rgba([clamp(68, n), clamp(228, n), clamp(96, n), 255])
        } else {
            Rgba([clamp(32, n), clamp(92, n), clamp(44, n), 255])
        }
    });

    // 445..=447: Leather, Iron, and Diamond Wearable Backpack item icons
    let paint_backpack = |atlas: &mut TextureAtlas,
                          tile_id: u16,
                          body: [i32; 3],
                          flap: [i32; 3],
                          buckle: [i32; 3]| {
        atlas.paint_tile(tile_id, |px, py| {
            let x = px as i32;
            let y = py as i32;
            // Top handle loop
            let in_handle = y >= 4 && y <= 7 && ((x >= 11 && x <= 13) || (x >= 18 && x <= 20) || (y <= 5 && x >= 11 && x <= 20));
            // Main rounded pack body
            let in_body = x >= 6 && x <= 25 && y >= 8 && y <= 27
                && !((x == 6 || x == 25) && (y == 8 || y == 27));
            if !in_handle && !in_body {
                return Rgba([0, 0, 0, 0]);
            }
            if in_handle && !in_body {
                return Rgba([clamp(flap[0] - 24, 0), clamp(flap[1] - 24, 0), clamp(flap[2] - 24, 0), 255]);
            }
            let is_edge = x == 6 || x == 25 || y == 8 || y == 27;
            if is_edge {
                return Rgba([clamp(body[0] - 58, 0), clamp(body[1] - 58, 0), clamp(body[2] - 58, 0), 255]);
            }
            let n = (hash(px, py, tile_id as u32) % 9) - 4;
            // Metallic buckle clasps
            if (x >= 10 && x <= 12 || x >= 19 && x <= 21) && y >= 15 && y <= 18 {
                return Rgba([clamp(buckle[0], n), clamp(buckle[1], n), clamp(buckle[2], n), 255]);
            }
            // Top lid flap
            if y <= 15 {
                let hi = if y <= 10 { 14 } else { 0 };
                return Rgba([clamp(flap[0], n + hi), clamp(flap[1], n + hi), clamp(flap[2], n + hi), 255]);
            }
            // Front lower pocket
            if x >= 9 && x <= 22 && y >= 20 && y <= 25 {
                let p_edge = if x == 9 || x == 22 || y == 20 || y == 25 { -24 } else { 8 };
                return Rgba([clamp(flap[0], n + p_edge), clamp(flap[1], n + p_edge), clamp(flap[2], n + p_edge), 255]);
            }
            Rgba([clamp(body[0], n), clamp(body[1], n), clamp(body[2], n), 255])
        });
    };

    paint_backpack(atlas, 445, [148, 92, 46], [176, 114, 60], [232, 194, 62]); // Leather Backpack
    paint_backpack(atlas, 446, [158, 166, 180], [194, 202, 214], [238, 204, 68]); // Iron Backpack
    paint_backpack(atlas, 447, [56, 192, 208], [88, 228, 240], [244, 208, 72]); // Diamond Backpack
}

/// Paints procedural 32x32 tiles for Biome Boss Temples, Portals, Boss Weapons/Armor/Cores,
/// and Boss & Temple Minion entity skins (tiles 448..=486, Issue #9).
fn paint_temple_and_boss_tiles(atlas: &mut TextureAtlas) {
    // Helper for carved brick blocks
    let paint_brick = |atlas: &mut TextureAtlas,
                       tile: u16,
                       base: [u8; 3],
                       mortar: [u8; 3],
                       accent: [u8; 3]| {
        atlas.paint_tile(tile, |px, py| {
            let border = px == 0 || py == 0 || px == 31 || py == 31;
            let row = py / 8;
            let offset = if row % 2 == 0 { 0 } else { 8 };
            let seam = py % 8 == 0 || (px + offset) % 16 == 0;
            let carve = px >= 10 && px <= 21 && py >= 10 && py <= 21
                && (px == 10 || px == 21 || py == 10 || py == 21);
            let n = (((px * 37 + py * 17) ^ (px * 13)) % 14) as i16 - 7;
            let col = if border || seam {
                mortar
            } else if carve {
                accent
            } else {
                [
                    (base[0] as i16 + n).clamp(0, 255) as u8,
                    (base[1] as i16 + n).clamp(0, 255) as u8,
                    (base[2] as i16 + n).clamp(0, 255) as u8,
                ]
            };
            Rgba([col[0], col[1], col[2], 255])
        });
    };

    // 448..=452: Temple masonry blocks
    paint_brick(atlas, 448, [118, 120, 126], [68, 70, 76], [160, 164, 172]); // TEMPLE_STONE
    paint_brick(atlas, 449, [218, 186, 118], [156, 122, 64], [248, 210, 72]); // TEMPLE_SANDSTONE
    paint_brick(atlas, 450, [132, 206, 238], [82, 148, 194], [214, 244, 255]); // TEMPLE_ICE_BRICK
    paint_brick(atlas, 451, [104, 122, 92], [58, 74, 52], [62, 168, 82]); // TEMPLE_MOSS_BRICK
    paint_brick(atlas, 452, [42, 32, 58], [22, 16, 34], [148, 72, 218]); // TEMPLE_OBSIDIAN

    // 453: TEMPLE_PORTAL (swirling golden-violet arcane vortex)
    atlas.paint_tile(453, |px, py| {
        let dx = px as f32 - 15.5;
        let dy = py as f32 - 15.5;
        let r = (dx * dx + dy * dy).sqrt();
        let ang = dy.atan2(dx);
        let swirl = ((r * 0.45 + ang * 3.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let core = (1.0 - (r / 18.0)).clamp(0.0, 1.0);
        let cr = (95.0 + swirl * 115.0 + core * 45.0).min(255.0) as u8;
        let cg = (40.0 + core * 165.0 + swirl * 35.0).min(255.0) as u8;
        let cb = (175.0 + swirl * 75.0).min(255.0) as u8;
        Rgba([cr, cg, cb, 255])
    });

    // 454: ARENA_RETURN_PORTAL (swirling emerald-cyan vortex)
    atlas.paint_tile(454, |px, py| {
        let dx = px as f32 - 15.5;
        let dy = py as f32 - 15.5;
        let r = (dx * dx + dy * dy).sqrt();
        let ang = dy.atan2(dx);
        let swirl = ((r * 0.48 - ang * 3.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let core = (1.0 - (r / 18.0)).clamp(0.0, 1.0);
        let cr = (30.0 + core * 140.0).min(255.0) as u8;
        let cg = (155.0 + swirl * 85.0 + core * 15.0).min(255.0) as u8;
        let cb = (140.0 + swirl * 100.0).min(255.0) as u8;
        Rgba([cr, cg, cb, 255])
    });

    // 455: BOSS_ALTAR (dark runic pedestal with glowing central gem)
    atlas.paint_tile(455, |px, py| {
        let dx = (px as i32 - 16).abs();
        let dy = (py as i32 - 16).abs();
        let manhattan = dx + dy;
        if manhattan <= 6 {
            Rgba([255, 92, 64, 255])
        } else if manhattan <= 9 || px == 2 || px == 29 || py == 2 || py == 29 {
            Rgba([235, 192, 68, 255])
        } else {
            Rgba([46, 40, 56, 255])
        }
    });

    // 456: TEMPLE_BRAZIER (dark iron frame + glowing embers)
    atlas.paint_tile(456, |px, py| {
        let frame = px < 4 || px > 27 || py < 4 || py > 27;
        if frame {
            Rgba([58, 54, 60, 255])
        } else {
            let wave = ((px * 19 + py * 29) % 40) as u8;
            Rgba([250, 140u8.saturating_add(wave), 36, 255])
        }
    });

    // Helper for diagonal boss sword icons (457..=460)
    let paint_boss_sword = |atlas: &mut TextureAtlas,
                            tile: u16,
                            blade: [u8; 3],
                            edge: [u8; 3],
                            hilt: [u8; 3]| {
        atlas.paint_tile(tile, |px, py| {
            let x = px as i32;
            let y = py as i32;
            let diag = (x + y - 31).abs();
            // Diagonal blade from top-right (27, 4) to lower-left (10, 21)
            if x >= 10 && x <= 27 && y >= 4 && y <= 21 && diag <= 2 {
                let c = if diag == 0 { edge } else { blade };
                return Rgba([c[0], c[1], c[2], 255]);
            }
            // Crossguard
            if (x - y).abs() <= 1 && x >= 7 && x <= 14 && y >= 17 && y <= 24 {
                return Rgba([hilt[0], hilt[1], hilt[2], 255]);
            }
            // Handle
            if x >= 4 && x <= 9 && y >= 22 && y <= 27 && diag <= 1 {
                return Rgba([96, 64, 38, 255]);
            }
            Rgba([0, 0, 0, 0])
        });
    };

    paint_boss_sword(atlas, 457, [252, 194, 42], [255, 245, 160], [220, 92, 24]); // SUNBLADE
    paint_boss_sword(atlas, 458, [92, 204, 252], [224, 250, 255], [48, 96, 160]); // FROSTBLADE
    paint_boss_sword(atlas, 459, [52, 198, 86], [176, 255, 168], [110, 78, 42]); // VERDANT_GREATBLADE
    paint_boss_sword(atlas, 460, [146, 56, 232], [228, 174, 255], [46, 36, 62]); // SHADOWFANG_BLADE

    // 461..=464: Boss Armor Icons
    // 461: PHARAOH_CROWN
    atlas.paint_tile(461, |px, py| {
        let in_band = py >= 16 && py <= 24 && px >= 5 && px <= 26;
        let spikes = py >= 8 && py < 16 && (px >= 5 && px <= 9 || px >= 13 && px <= 18 || px >= 22 && px <= 26);
        let gem = py >= 18 && py <= 22 && px >= 14 && px <= 17;
        if gem {
            Rgba([46, 212, 242, 255])
        } else if in_band || spikes {
            Rgba([248, 204, 52, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    // 462: GLACIAL_CHESTPLATE
    atlas.paint_tile(462, |px, py| {
        let torso = px >= 8 && px <= 23 && py >= 6 && py <= 26;
        let sleeves = py >= 6 && py <= 15 && (px >= 3 && px <= 8 || px >= 23 && px <= 28);
        let neck = px >= 12 && px <= 19 && py >= 6 && py <= 9;
        if (torso || sleeves) && !neck {
            let trim = px == 8 || px == 23 || py == 26 || (px >= 14 && px <= 17 && py >= 12 && py <= 18);
            if trim { Rgba([220, 248, 255, 255]) } else { Rgba([72, 176, 236, 255]) }
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    // 463: GOLEM_GREAVES
    atlas.paint_tile(463, |px, py| {
        let waist = px >= 7 && px <= 24 && py >= 6 && py <= 12;
        let left_leg = px >= 7 && px <= 14 && py >= 12 && py <= 27;
        let right_leg = px >= 17 && px <= 24 && py >= 12 && py <= 27;
        if waist || left_leg || right_leg {
            let moss = (px + py) % 5 == 0;
            if moss { Rgba([58, 188, 86, 255]) } else { Rgba([106, 118, 102, 255]) }
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    // 464: LICH_BOOTS
    atlas.paint_tile(464, |px, py| {
        let left = px >= 4 && px <= 13 && py >= 14 && py <= 27;
        let right = px >= 18 && px <= 27 && py >= 14 && py <= 27;
        if left || right {
            if py <= 17 { Rgba([172, 72, 248, 255]) } else { Rgba([46, 36, 64, 255]) }
        } else {
            Rgba([0, 0, 0, 0])
        }
    });

    // 465..=468: Glowing Boss Trophy Cores
    let paint_core = |atlas: &mut TextureAtlas, tile: u16, inner: [u8; 3], outer: [u8; 3]| {
        atlas.paint_tile(tile, |px, py| {
            let dx = px as f32 - 15.5;
            let dy = py as f32 - 15.5;
            let r = (dx * dx + dy * dy).sqrt();
            if r <= 6.5 {
                Rgba([inner[0], inner[1], inner[2], 255])
            } else if r <= 11.5 {
                Rgba([outer[0], outer[1], outer[2], 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
    };
    paint_core(atlas, 465, [255, 248, 168], [248, 168, 32]); // SUN_CORE
    paint_core(atlas, 466, [224, 252, 255], [56, 178, 246]); // FROST_HEART
    paint_core(atlas, 467, [188, 255, 168], [42, 172, 76]); // ANCIENT_VINE_CORE
    paint_core(atlas, 468, [232, 178, 255], [136, 44, 224]); // SOUL_PHYLACTERY

    // 469..=486: Temple Minion & Boss Entity Skins (face + body pairs)
    let paint_mob_pair = |atlas: &mut TextureAtlas,
                          head_tile: u16,
                          body_tile: u16,
                          skin: [u8; 3],
                          eye: [u8; 3],
                          armor: [u8; 3],
                          trim: [u8; 3]| {
        atlas.paint_tile(head_tile, |px, py| {
            let left_eye = px >= 7 && px <= 12 && py >= 11 && py <= 15;
            let right_eye = px >= 19 && px <= 24 && py >= 11 && py <= 15;
            let brow = py >= 7 && py <= 9 && px >= 5 && px <= 26;
            let mouth = px >= 10 && px <= 21 && py >= 21 && py <= 24;
            if left_eye || right_eye {
                Rgba([eye[0], eye[1], eye[2], 255])
            } else if brow {
                Rgba([trim[0], trim[1], trim[2], 255])
            } else if mouth {
                Rgba([30, 24, 28, 255])
            } else {
                Rgba([skin[0], skin[1], skin[2], 255])
            }
        });
        atlas.paint_tile(body_tile, |px, py| {
            let band = py % 8 == 0 || px == 15 || px == 16;
            let c = if band { trim } else { armor };
            Rgba([c[0], c[1], c[2], 255])
        });
    };

    // Minions
    paint_mob_pair(atlas, 469, 470, [212, 194, 148], [255, 190, 40], [192, 174, 130], [138, 118, 82]); // SandMummy
    paint_mob_pair(atlas, 471, 472, [224, 212, 176], [255, 140, 20], [186, 146, 52], [242, 204, 72]); // SunArcher
    paint_mob_pair(atlas, 473, 474, [156, 224, 250], [240, 255, 255], [76, 148, 202], [190, 238, 255]); // FrostWraith
    paint_mob_pair(atlas, 475, 476, [52, 92, 48], [86, 255, 96], [46, 78, 42], [92, 148, 68]); // JungleStalker
    paint_mob_pair(atlas, 477, 478, [44, 38, 56], [196, 86, 255], [38, 32, 50], [122, 52, 196]); // ShadowKnight

    // Bosses
    paint_mob_pair(atlas, 479, 480, [242, 198, 58], [255, 255, 180], [218, 162, 34], [52, 168, 214]); // SunPharaoh
    paint_mob_pair(atlas, 481, 482, [142, 222, 255], [255, 255, 255], [64, 142, 208], [212, 246, 255]); // FrostTitan
    paint_mob_pair(atlas, 483, 484, [96, 112, 92], [82, 255, 112], [82, 102, 76], [46, 182, 78]); // OvergrownGolem
    paint_mob_pair(atlas, 485, 486, [224, 218, 232], [204, 72, 255], [46, 28, 68], [164, 62, 238]); // ShadowLich
}

/// Paints all Issue #12 Farming, Cooking, and Hunger tiles (512..=566).
fn paint_farming_and_cooking_tiles(atlas: &mut TextureAtlas) {
    let put = |atlas: &mut TextureAtlas, tile: u32, x: u32, y: u32, rgba: [u8; 4]| {
        if x < TILE_SIZE && y < TILE_SIZE {
            let ox = (tile % ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let oy = (tile / ATLAS_TILES_PER_ROW) * TILE_SIZE;
            atlas.image.put_pixel(ox + x, oy + y, Rgba(rgba));
        }
    };

    let fill_rect = |atlas: &mut TextureAtlas, tile: u32, x0: u32, y0: u32, w: u32, h: u32, c: [u8; 4]| {
        for dy in 0..h {
            for dx in 0..w {
                put(atlas, tile, x0 + dx, y0 + dy, c);
            }
        }
    };

    let fill_circle = |atlas: &mut TextureAtlas, tile: u32, cx: f32, cy: f32, r: f32, c: [u8; 4]| {
        let r2 = r * r;
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                if dx * dx + dy * dy <= r2 {
                    put(atlas, tile, x, y, c);
                }
            }
        }
    };

    // 1. Fertilized Farmland tops (512..=515) cloned from dry(150) / moist(244) with speckles
    let clone_farmland_with_flecks = |atlas: &mut TextureAtlas, src_tile: u32, dst_tile: u32, tint: [f32; 3], fleck: [u8; 4], fleck2: [u8; 4]| {
        let sx = (src_tile % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let sy = (src_tile / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let dx = (dst_tile % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let dy = (dst_tile / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let p = *atlas.image.get_pixel(sx + x, sy + y);
                let h = (x.wrapping_mul(37) ^ y.wrapping_mul(91) ^ dst_tile.wrapping_mul(17)) % 23;
                let out = if h == 0 {
                    Rgba(fleck)
                } else if h == 7 {
                    Rgba(fleck2)
                } else {
                    Rgba([
                        ((p[0] as f32) * tint[0]).clamp(0.0, 255.0) as u8,
                        ((p[1] as f32) * tint[1]).clamp(0.0, 255.0) as u8,
                        ((p[2] as f32) * tint[2]).clamp(0.0, 255.0) as u8,
                        255,
                    ])
                };
                atlas.image.put_pixel(dx + x, dy + y, out);
            }
        }
    };
    clone_farmland_with_flecks(atlas, 150, 512, [0.92, 0.96, 0.85], [76, 128, 48, 255], [92, 68, 38, 255]); // Compost Dry
    clone_farmland_with_flecks(atlas, 244, 513, [0.88, 0.96, 0.82], [68, 138, 44, 255], [72, 54, 30, 255]); // Compost Moist
    clone_farmland_with_flecks(atlas, 150, 514, [1.02, 1.02, 1.00], [238, 236, 220, 255], [208, 204, 185, 255]); // Bone Dry
    clone_farmland_with_flecks(atlas, 244, 515, [0.96, 0.98, 1.00], [230, 234, 224, 255], [188, 194, 178, 255]); // Bone Moist

    // 2. Crop stages (516..=535): Carrots, Potatoes, Tomatoes, Melon Stem, Pumpkin Stem
    let paint_crop_stage = |atlas: &mut TextureAtlas, tile: u32, stage: u32, leaf_col: [u8; 4], leaf_dark: [u8; 4], fruit_col: Option<[u8; 4]>| {
        let height = 7 + stage * 6;
        let top_y = TILE_SIZE.saturating_sub(height);
        for stalk in [6u32, 13, 20, 26] {
            for y in top_y..TILE_SIZE {
                let wobble = ((y + stalk) % 3) as i32 - 1;
                let x = (stalk as i32 + wobble).clamp(1, 30) as u32;
                put(atlas, tile, x, y, if y % 2 == 0 { leaf_col } else { leaf_dark });
                put(atlas, tile, x + 1, y, leaf_dark);
                if stage >= 1 && y < TILE_SIZE - 4 && y % 4 == 0 {
                    put(atlas, tile, x.saturating_sub(1), y, leaf_col);
                    put(atlas, tile, (x + 2).min(31), y, leaf_col);
                }
            }
        }
        if let Some(fc) = fruit_col {
            if stage >= 2 {
                let r = if stage == 3 { 3.0 } else { 2.0 };
                let spots = if stage == 3 {
                    vec![(8.0, 26.0), (16.0, 24.0), (24.0, 26.0), (12.0, 17.0), (21.0, 18.0)]
                } else {
                    vec![(10.0, 26.0), (21.0, 25.0)]
                };
                for (fx, fy) in spots {
                    fill_circle(atlas, tile, fx, fy, r, fc);
                }
            }
        }
    };

    for s in 0..4u32 {
        paint_crop_stage(atlas, 516 + s, s, [72, 168, 58, 255], [46, 122, 38, 255], Some([238, 126, 32, 255])); // Carrots
        paint_crop_stage(atlas, 520 + s, s, [64, 152, 54, 255], [42, 108, 36, 255], Some([206, 168, 96, 255])); // Potatoes
        paint_crop_stage(atlas, 524 + s, s, [54, 148, 48, 255], [34, 102, 32, 255], Some([222, 46, 38, 255])); // Tomatoes
        paint_crop_stage(atlas, 528 + s, s, [98, 176, 56, 255], [64, 124, 38, 255], Some([168, 214, 72, 255])); // Melon Stem
        paint_crop_stage(atlas, 532 + s, s, [118, 156, 48, 255], [78, 108, 32, 255], Some([228, 142, 36, 255])); // Pumpkin Stem
    }

    // 536: Sugar Cane block (segmented green reed stalks)
    for stalk_x in [5u32, 13, 21, 27] {
        for y in 0..TILE_SIZE {
            let seg = y % 8 == 0;
            let c = if seg { [186, 224, 118, 255] } else if (y + stalk_x) % 2 == 0 { [112, 188, 72, 255] } else { [84, 156, 52, 255] };
            put(atlas, 536, stalk_x, y, c);
            put(atlas, 536, stalk_x + 1, y, c);
            put(atlas, 536, stalk_x + 2, y, [68, 132, 42, 255]);
        }
    }

    // 3. Cooking Pot block faces (537..=541)
    // 537: Top unlit (iron rim + dark interior)
    fill_rect(atlas, 537, 0, 0, 32, 32, [62, 64, 70, 255]);
    fill_rect(atlas, 537, 3, 3, 26, 26, [88, 92, 100, 255]);
    fill_rect(atlas, 537, 6, 6, 20, 20, [34, 36, 40, 255]);
    // 540: Top lit (bubbling golden-amber broth)
    fill_rect(atlas, 540, 0, 0, 32, 32, [62, 64, 70, 255]);
    fill_rect(atlas, 540, 3, 3, 26, 26, [92, 96, 104, 255]);
    fill_rect(atlas, 540, 6, 6, 20, 20, [218, 138, 42, 255]);
    fill_circle(atlas, 540, 12.0, 13.0, 2.5, [250, 214, 102, 255]);
    fill_circle(atlas, 540, 20.0, 18.0, 2.0, [250, 214, 102, 255]);
    // 538: Side unlit (iron pot over stone hearth)
    fill_rect(atlas, 538, 0, 0, 32, 32, [76, 78, 84, 255]);
    fill_rect(atlas, 538, 2, 2, 28, 18, [54, 56, 62, 255]);
    fill_rect(atlas, 538, 0, 4, 32, 3, [98, 102, 110, 255]);
    fill_rect(atlas, 538, 4, 21, 24, 9, [42, 42, 46, 255]);
    // 539: Side lit (glowing embers underneath)
    fill_rect(atlas, 539, 0, 0, 32, 32, [76, 78, 84, 255]);
    fill_rect(atlas, 539, 2, 2, 28, 18, [58, 56, 60, 255]);
    fill_rect(atlas, 539, 0, 4, 32, 3, [104, 106, 114, 255]);
    fill_rect(atlas, 539, 4, 21, 24, 9, [224, 92, 24, 255]);
    fill_rect(atlas, 539, 8, 23, 16, 5, [255, 198, 64, 255]);
    // 541: Bottom
    fill_rect(atlas, 541, 0, 0, 32, 32, [52, 54, 58, 255]);

    // 4. Seeds & Fertilizers (542..=547)
    let paint_seeds = |atlas: &mut TextureAtlas, tile: u32, c1: [u8; 4], c2: [u8; 4]| {
        for (cx, cy) in [(11.0, 14.0), (19.0, 12.0), (15.0, 20.0), (22.0, 21.0), (10.0, 22.0)] {
            fill_circle(atlas, tile, cx, cy, 2.8, c1);
            put(atlas, tile, cx as u32, cy as u32, c2);
        }
    };
    paint_seeds(atlas, 542, [224, 196, 120, 255], [186, 142, 72, 255]); // Tomato Seeds
    paint_seeds(atlas, 543, [44, 54, 38, 255], [88, 104, 72, 255]);    // Melon Seeds
    paint_seeds(atlas, 544, [238, 226, 184, 255], [198, 178, 128, 255]); // Pumpkin Seeds

    // 545: Sugar Cane item icon
    for i in 0..22u32 {
        let x = 6 + i;
        let y = 26 - i;
        let c = if i % 6 == 0 { [192, 232, 128, 255] } else { [104, 182, 68, 255] };
        fill_rect(atlas, 545, x, y, 3, 3, c);
    }
    // 546: Compost (earthy rich mound with green specks)
    fill_circle(atlas, 546, 16.0, 19.0, 9.0, [92, 64, 36, 255]);
    fill_circle(atlas, 546, 15.0, 16.0, 6.5, [112, 78, 44, 255]);
    for (x, y) in [(12, 16), (18, 15), (15, 20), (20, 21), (11, 21)] {
        fill_rect(atlas, 546, x, y, 2, 2, [82, 148, 52, 255]);
    }
    // 547: Bone Fertilizer (pale mineral pouch/powder)
    fill_circle(atlas, 547, 16.0, 19.0, 9.0, [214, 210, 194, 255]);
    fill_circle(atlas, 547, 15.0, 16.0, 6.5, [242, 240, 228, 255]);
    for (x, y) in [(13, 15), (19, 17), (16, 21)] {
        fill_rect(atlas, 547, x, y, 2, 2, [255, 255, 255, 255]);
    }

    // 5. Raw Crops & Animal Products (548..=554)
    // 548: Carrot
    for i in 0..16u32 {
        let r = (1.5 + (i as f32) * 0.25).min(4.5);
        fill_circle(atlas, 548, 8.0 + i as f32, 24.0 - i as f32, r, [242, 128, 28, 255]);
    }
    fill_circle(atlas, 548, 24.0, 8.0, 4.0, [68, 168, 54, 255]);
    // 549: Potato
    fill_circle(atlas, 549, 16.0, 17.0, 8.5, [198, 158, 92, 255]);
    fill_circle(atlas, 549, 14.0, 15.0, 5.5, [218, 182, 114, 255]);
    // 550: Tomato
    fill_circle(atlas, 550, 16.0, 18.0, 8.5, [224, 44, 36, 255]);
    fill_circle(atlas, 550, 13.0, 15.0, 3.0, [248, 96, 82, 255]);
    fill_rect(atlas, 550, 14, 7, 4, 4, [58, 152, 46, 255]);
    // 551: Sugar
    fill_circle(atlas, 551, 16.0, 20.0, 8.5, [238, 240, 246, 255]);
    fill_circle(atlas, 551, 16.0, 16.0, 5.5, [255, 255, 255, 255]);
    // 552: Egg
    fill_circle(atlas, 552, 16.0, 18.0, 7.5, [236, 224, 198, 255]);
    fill_circle(atlas, 552, 16.0, 13.0, 5.5, [246, 238, 218, 255]);
    // 553: Milk Bucket (iron bucket with white milk)
    paint_bucket_sprite(atlas, 553, false);
    fill_rect(atlas, 553, 9, 10, 14, 6, [248, 248, 244, 255]);
    // 554: Raw Mutton
    fill_circle(atlas, 554, 16.0, 17.0, 8.5, [206, 92, 92, 255]);
    fill_circle(atlas, 554, 19.0, 15.0, 3.5, [242, 226, 214, 255]);

    // 6. Furnace & Simple Crafted Foods (555..=558)
    // 555: Baked Potato
    fill_circle(atlas, 555, 16.0, 17.0, 8.5, [156, 104, 46, 255]);
    fill_circle(atlas, 555, 16.0, 15.0, 4.5, [244, 214, 120, 255]);
    // 556: Cooked Mutton
    fill_circle(atlas, 556, 16.0, 17.0, 8.5, [138, 76, 44, 255]);
    fill_circle(atlas, 556, 19.0, 15.0, 3.0, [224, 206, 182, 255]);
    // 557: Cookie
    fill_circle(atlas, 557, 16.0, 16.0, 9.0, [204, 148, 78, 255]);
    for (x, y) in [(12, 13), (19, 12), (14, 19), (20, 18), (16, 15)] {
        fill_rect(atlas, 557, x, y, 2, 2, [74, 44, 24, 255]);
    }
    // 558: Pumpkin Pie
    fill_circle(atlas, 558, 16.0, 18.0, 10.0, [198, 148, 78, 255]);
    fill_circle(atlas, 558, 16.0, 17.0, 7.5, [224, 118, 32, 255]);
    fill_circle(atlas, 558, 16.0, 16.0, 2.5, [252, 246, 232, 255]);

    // 7. Cooking Pot Meals & Herbal Tea (559..=565)
    let paint_dish = |atlas: &mut TextureAtlas, tile: u32, broth: [u8; 4], garnish1: [u8; 4], garnish2: [u8; 4]| {
        fill_circle(atlas, tile, 16.0, 19.0, 10.5, [122, 82, 48, 255]); // ceramic/wood dish rim
        fill_circle(atlas, tile, 16.0, 17.5, 8.2, broth);
        fill_circle(atlas, tile, 13.0, 16.0, 2.2, garnish1);
        fill_circle(atlas, tile, 19.0, 18.0, 2.2, garnish2);
        fill_circle(atlas, tile, 16.0, 20.0, 1.8, garnish1);
    };
    paint_dish(atlas, 559, [214, 56, 38, 255], [244, 188, 102, 255], [72, 168, 56, 255]); // Tomato Soup
    paint_dish(atlas, 560, [176, 122, 52, 255], [238, 128, 32, 255], [74, 164, 58, 255]); // Vegetable Stew
    paint_dish(atlas, 561, [126, 68, 38, 255], [236, 126, 36, 255], [218, 188, 122, 255]); // Hearty Meat Stew
    paint_dish(atlas, 562, [246, 204, 68, 255], [224, 56, 44, 255], [68, 162, 54, 255]);  // Omelette
    paint_dish(atlas, 563, [238, 214, 164, 255], [224, 48, 48, 255], [238, 88, 104, 255]); // Fruit Salad

    // 564: Sweet Cake
    fill_rect(atlas, 564, 6, 14, 20, 12, [196, 142, 74, 255]);
    fill_rect(atlas, 564, 6, 11, 20, 5, [252, 246, 238, 255]);
    fill_circle(atlas, 564, 12.0, 11.0, 2.0, [224, 42, 42, 255]);
    fill_circle(atlas, 564, 20.0, 11.0, 2.0, [224, 42, 42, 255]);

    // 565: Herbal Tea (warm mug with emerald herbal brew + steam)
    fill_rect(atlas, 565, 8, 11, 14, 15, [218, 206, 188, 255]);
    fill_rect(atlas, 565, 10, 13, 10, 4, [72, 186, 104, 255]);
    fill_rect(atlas, 565, 22, 14, 4, 8, [196, 184, 166, 255]);
    fill_rect(atlas, 565, 12, 6, 2, 4, [220, 244, 230, 200]);
    fill_rect(atlas, 565, 17, 5, 2, 5, [220, 244, 230, 200]);

    // 566: Cooking Pot item icon
    fill_rect(atlas, 566, 6, 11, 20, 14, [58, 60, 66, 255]);
    fill_rect(atlas, 566, 5, 9, 22, 3, [94, 98, 108, 255]);
    fill_rect(atlas, 566, 3, 12, 3, 4, [148, 104, 58, 255]); // wooden left handle
    fill_rect(atlas, 566, 26, 12, 3, 4, [148, 104, 58, 255]); // wooden right handle
    fill_rect(atlas, 566, 9, 25, 14, 3, [232, 112, 28, 255]); // ember glow
}

/// Paints procedural 32x32 item icons for Issue #11 (NPC Villages & Trading):
/// 567: Gold Coin, 568: Miner's Lantern, 569: Vanguard Blade, 570: Master Angler's Lure,
/// 571: Golden Apple Pie, 572: Royal Feast, 573: Spiced Elixir.
fn paint_village_item_tiles(atlas: &mut TextureAtlas) {
    // 567: Gold Coin
    atlas.paint_tile(567, |px, py| {
        let dx = px as f32 - 15.5;
        let dy = py as f32 - 15.5;
        let r = (dx * dx + dy * dy).sqrt();
        if r > 11.5 {
            return Rgba([0, 0, 0, 0]);
        }
        if r > 10.2 {
            return Rgba([92, 62, 12, 255]);
        }
        if r > 8.2 {
            return Rgba([226, 174, 34, 255]);
        }
        // Embossed inner coin face with diagonal highlight & center emblem
        let in_emblem = dx.abs() <= 2.2 && dy.abs() <= 4.5 && (dx.abs() >= 1.0 || dy.abs() <= 1.2);
        if in_emblem {
            Rgba([255, 246, 168, 255])
        } else if dx + dy < -4.0 {
            Rgba([255, 232, 92, 255])
        } else {
            Rgba([242, 198, 46, 255])
        }
    });

    // 568: Miner's Lantern
    atlas.paint_tile(568, |px, py| {
        let x = px as i32;
        let y = py as i32;
        // Top ring
        if (6..=9).contains(&y) && (13..=18).contains(&x) && (y == 6 || x == 13 || x == 18) {
            return Rgba([120, 128, 140, 255]);
        }
        // Cap & base
        if ((10..=12).contains(&y) || (24..=26).contains(&y)) && (9..=22).contains(&x) {
            return Rgba([76, 84, 96, 255]);
        }
        // Lantern body + bright glowing core
        if (13..=23).contains(&y) && (10..=21).contains(&x) {
            if x == 10 || x == 21 {
                return Rgba([68, 74, 86, 255]);
            }
            let dist = ((x - 15) * (x - 15) + (y - 18) * (y - 18)) as f32;
            if dist < 12.0 {
                return Rgba([255, 252, 200, 255]);
            }
            return Rgba([140, 238, 245, 255]);
        }
        Rgba([0, 0, 0, 0])
    });

    // 569: Vanguard Blade
    paint_sword_sprite(
        atlas,
        569,
        [235, 246, 255],
        [156, 205, 245],
        [72, 120, 182],
        [242, 196, 52],
    );

    // 570: Master Angler's Lure
    atlas.paint_tile(570, |px, py| {
        let dx = px as f32 - 15.5;
        let dy = py as f32 - 14.0;
        let r = (dx * dx * 1.4 + dy * dy * 0.8).sqrt();
        if r < 7.5 {
            if r > 6.2 {
                return Rgba([72, 50, 18, 255]);
            }
            if dy < 0.0 {
                return Rgba([250, 212, 64, 255]);
            }
            return Rgba([48, 210, 198, 255]);
        }
        // Hook at bottom
        if py >= 21 && py <= 27 && ((px == 15) || (py == 27 && (15..=20).contains(&px)) || (px == 20 && (24..=27).contains(&py))) {
            return Rgba([210, 218, 228, 255]);
        }
        Rgba([0, 0, 0, 0])
    });

    // 571: Golden Apple Pie
    atlas.paint_tile(571, |px, py| {
        let dx = px as f32 - 15.5;
        let dy = (py as f32 - 17.5) * 1.45;
        let r = (dx * dx + dy * dy).sqrt();
        if r > 11.5 {
            return Rgba([0, 0, 0, 0]);
        }
        if r > 9.5 {
            return Rgba([192, 134, 48, 255]);
        }
        if ((px + py) % 5 == 0) || ((px as i32 - py as i32).abs() % 5 == 0) {
            Rgba([255, 224, 92, 255])
        } else {
            Rgba([218, 158, 42, 255])
        }
    });

    // 572: Royal Feast
    atlas.paint_tile(572, |px, py| {
        let dx = px as f32 - 15.5;
        let dy = (py as f32 - 18.0) * 1.5;
        let r = (dx * dx + dy * dy).sqrt();
        if r > 12.5 {
            return Rgba([0, 0, 0, 0]);
        }
        if r > 10.5 {
            return Rgba([235, 196, 62, 255]); // Gold platter rim
        }
        let roast = ((px as f32 - 15.5).powi(2) + (py as f32 - 15.5).powi(2)).sqrt();
        if roast < 6.5 {
            Rgba([176, 86, 42, 255])
        } else {
            Rgba([88, 172, 74, 255]) // Herb garnish
        }
    });

    // 573: Spiced Elixir
    atlas.paint_tile(573, |px, py| {
        let x = px as i32;
        let y = py as i32;
        // Cork & neck
        if (6..=8).contains(&y) && (13..=18).contains(&x) {
            return Rgba([156, 108, 62, 255]);
        }
        if (9..=13).contains(&y) && (12..=19).contains(&x) {
            return Rgba([200, 225, 238, 255]);
        }
        // Round flask filled with glowing crimson-gold elixir
        let dx = px as f32 - 15.5;
        let dy = py as f32 - 20.0;
        let r = (dx * dx + dy * dy).sqrt();
        if r <= 8.5 {
            if r > 7.2 {
                return Rgba([185, 212, 228, 255]);
            }
            if py < 17 {
                return Rgba([230, 240, 248, 180]);
            }
            if dx < -1.0 {
                return Rgba([255, 132, 68, 255]);
            }
            return Rgba([212, 46, 58, 255]);
        }
        Rgba([0, 0, 0, 0])
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atlas_dimensions_and_tiles() {
        assert_eq!(TILE_SIZE, 32);
        assert_eq!(ATLAS_WIDTH, 512);
        assert_eq!(ATLAS_HEIGHT, 2048);

        let atlas = TextureAtlas::generate_default();
        assert_eq!(atlas.image.width(), 512);
        assert_eq!(atlas.image.height(), 2048);

        // Verify stone tile (Tile 1) is non-transparent
        let stone_px = atlas.image.get_pixel(32 + 16, 16);
        assert_eq!(stone_px[3], 255);

        // Verify WHITE_TILE (Tile 24) is pure opaque white for clean UI element rendering
        let wx = (24 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let wy = (24 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let white_px = atlas.image.get_pixel(wx + 16, wy + 16);
        assert_eq!(*white_px, image::Rgba([255, 255, 255, 255]));

        // Verify player skin tiles (272..287) are opaque
        for tile_id in 272..288 {
            let tx = (tile_id % ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let ty = (tile_id / ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let px = atlas.image.get_pixel(tx + 16, ty + 16);
            assert_eq!(px[3], 255, "Player skin tile {} must be opaque", tile_id);
        }

        // Verify UV bounds are well-formed within 0.0..=1.0
        let uv = TextureAtlas::get_uv_bounds(1);
        assert!(uv[0] >= 0.0 && uv[0] < uv[2] && uv[2] <= 1.0);
        assert!(uv[1] >= 0.0 && uv[1] < uv[3] && uv[3] <= 1.0);
    }

    #[test]
    fn test_bucket_atlas_tiles() {
        let atlas = TextureAtlas::generate_default();

        // Tile 242 (Bucket): (242 % 16) * 32 = 2 * 32 = 64, (242 / 16) * 32 = 15 * 32 = 480
        let b_x = (242 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let b_y = (242 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let bucket_center = atlas.image.get_pixel(b_x + 16, b_y + 16);
        assert_eq!(bucket_center[3], 255); // Opaque interior

        // Tile 243 (Water Bucket): (243 % 16) * 32 = 3 * 32 = 96, (243 / 16) * 32 = 480
        let wb_x = (243 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let wb_y = (243 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let water_center = atlas.image.get_pixel(wb_x + 16, wb_y + 16);
        assert_eq!(water_center[3], 255);
        // Blue channel should be strong for water bucket interior
        assert!(water_center[2] > 180, "Water blue channel should be high");
    }

    #[test]
    fn test_moist_farmland_atlas_tile() {
        let atlas = TextureAtlas::generate_default();

        let dry_x = (150 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let dry_y = (150 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let dry_px = atlas.image.get_pixel(dry_x + 16, dry_y + 16);

        let moist_x = (244 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let moist_y = (244 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let moist_px = atlas.image.get_pixel(moist_x + 16, moist_y + 16);

        assert_eq!(moist_px[3], 255, "Moist farmland must be opaque");
        assert!(
            moist_px[0] < dry_px[0],
            "Moist farmland should be darker than dry farmland (red)"
        );
        assert!(
            moist_px[1] < dry_px[1],
            "Moist farmland should be darker than dry farmland (green)"
        );
    }

    #[test]
    fn test_decorative_atlas_tiles() {
        let atlas = TextureAtlas::generate_default();
        // Bricks (288), 13 Wools (289..=301), Bed Top (317), Pumpkins (326..=328), Melons (329..=330)
        let opaque_tiles: Vec<u16> = (288..=301)
            .chain(std::iter::once(317))
            .chain(326..=330)
            .collect();
        for tile_id in opaque_tiles {
            let tx = (tile_id as u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let ty = (tile_id as u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let px = atlas.image.get_pixel(tx + 16, ty + 16);
            assert_eq!(px[3], 255, "Decorative tile {} must be opaque", tile_id);
        }
        // Dyes (302..=314) must be opaque at center (16, 16) and transparent at corner (0, 0)
        for tile_id in 302..=314u16 {
            let tx = (tile_id as u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let ty = (tile_id as u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
            assert_eq!(atlas.image.get_pixel(tx + 16, ty + 16)[3], 255);
            assert_eq!(atlas.image.get_pixel(tx, ty)[3], 0);
        }
    }

    #[test]
    fn test_font_ascii_atlas_tiles() {
        let atlas = TextureAtlas::generate_default();

        // Tile 32 (space) should be transparent
        let tx32 = (32u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let ty32 = (32u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                assert_eq!(
                    atlas.image.get_pixel(tx32 + x, ty32 + y)[3],
                    0,
                    "Space tile (32) must be completely transparent"
                );
            }
        }

        // Tiles 33..=126 ('!' to '~') must contain visible white pixel glyphs
        for c in 33..=126u8 {
            let tx = (c as u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let ty = (c as u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let mut non_zero_pixels = 0;
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let px = atlas.image.get_pixel(tx + x, ty + y);
                    if px[3] > 0 {
                        non_zero_pixels += 1;
                        assert_eq!(
                            [px[0], px[1], px[2]],
                            [255, 255, 255],
                            "ASCII char '{}' ({}) pixel must be pure white",
                            c as char,
                            c
                        );
                    }
                }
            }
            assert!(
                non_zero_pixels > 0,
                "ASCII char '{}' ({}) must have visible pixels in the atlas",
                c as char,
                c
            );
        }
    }

    #[test]
    fn test_sea_lantern_and_animal_atlas_tiles() {
        let atlas = TextureAtlas::generate_default();

        // 1. Sea lantern (238) must be 100% opaque, luminescent cyan/aquamarine
        let sl_x = (238u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let sl_y = (238u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let px = atlas.image.get_pixel(sl_x + x, sl_y + y);
                assert_eq!(
                    px[3], 255,
                    "Sea lantern pixel at ({}, {}) must be 100% opaque, not glass",
                    x, y
                );
                assert!(
                    px[2] >= 120,
                    "Sea lantern pixel at ({}, {}) should have prominent blue/cyan channel",
                    x, y
                );
            }
        }

        // 2. Cow tiles (170 hide, 185 head, 186 legs, 240 snout, 241 horns)
        for tile_id in [170, 185, 186, 240, 241] {
            let tx = (tile_id % ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let ty = (tile_id / ATLAS_TILES_PER_ROW) * TILE_SIZE;
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let px = atlas.image.get_pixel(tx + x, ty + y);
                    assert_eq!(px[3], 255, "Cow tile {} must be opaque", tile_id);
                }
            }
        }

        // 3. Pig tiles (171 hide, 187 head, 188 legs) must be opaque pink
        for tile_id in [171, 187, 188] {
            let tx = (tile_id % ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let ty = (tile_id / ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let center_px = atlas.image.get_pixel(tx + 16, ty + 16);
            assert_eq!(center_px[3], 255, "Pig tile {} must be opaque", tile_id);
            assert!(
                center_px[0] > 180 && center_px[0] > center_px[2],
                "Pig tile {} should have warm rosy pink tone",
                tile_id
            );
        }

        // 4. Chicken tiles (172 body/feathers, 189 head, 190 legs)
        for tile_id in [172, 189, 190] {
            let tx = (tile_id % ATLAS_TILES_PER_ROW) * TILE_SIZE;
            let ty = (tile_id / ATLAS_TILES_PER_ROW) * TILE_SIZE;
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let px = atlas.image.get_pixel(tx + x, ty + y);
                    assert_eq!(px[3], 255, "Chicken tile {} must be opaque", tile_id);
                }
            }
        }
    }

    #[test]
    fn test_cactus_and_wheat_seeds_atlas_tiles() {
        let atlas = TextureAtlas::generate_default();

        // 1. Wheat Seeds (159): Transparent canvas with non-empty seed pixels
        let ws_x = (159u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let ws_y = (159u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let mut opaque_seeds = 0;
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let px = atlas.image.get_pixel(ws_x + x, ws_y + y);
                if px[3] > 0 {
                    opaque_seeds += 1;
                }
            }
        }
        assert!(
            opaque_seeds >= 20,
            "Wheat seeds item icon (159) must have visible grain pixels, found {}",
            opaque_seeds
        );
        // Corner must be transparent
        assert_eq!(
            atlas.image.get_pixel(ws_x, ws_y)[3],
            0,
            "Wheat seeds background corner must be transparent"
        );

        // 2. Cactus Side (211): 100% opaque, fluted vertical ribs
        let cs_x = (211u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let cs_y = (211u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let px = atlas.image.get_pixel(cs_x + x, cs_y + y);
                assert_eq!(px[3], 255, "Cactus side must be 100% opaque");
            }
        }
        let groove_green = atlas.image.get_pixel(cs_x, cs_y + 16)[1];
        let ridge_green = atlas.image.get_pixel(cs_x + 1, cs_y + 16)[1];
        assert!(
            ridge_green > groove_green + 30,
            "Cactus side must have fluted vertical ribs (ridge {} > groove {})",
            ridge_green,
            groove_green
        );

        // 3. Cactus Top (212): 100% opaque, pale central pith
        let ct_x = (212u32 % ATLAS_TILES_PER_ROW) * TILE_SIZE;
        let ct_y = (212u32 / ATLAS_TILES_PER_ROW) * TILE_SIZE;
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let px = atlas.image.get_pixel(ct_x + x, ct_y + y);
                assert_eq!(px[3], 255, "Cactus top must be 100% opaque");
            }
        }
        let center_green = atlas.image.get_pixel(ct_x + 16, ct_y + 16)[1];
        let outer_groove_green = atlas.image.get_pixel(ct_x + 10, ct_y + 10)[1];
        assert!(
            center_green > outer_groove_green,
            "Cactus top must have lighter central pith"
        );
    }
}

