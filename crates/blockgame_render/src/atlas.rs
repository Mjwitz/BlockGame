use image::{Rgba, RgbaImage};

/// Size of each square tile on the atlas in pixels.
/// BlockGame uses 32x32 tiles matching the Faithful 32x texture pack.
pub const TILE_SIZE: u32 = 32;

/// Number of tiles horizontally and vertically across the atlas grid.
pub const ATLAS_TILES_PER_ROW: u32 = 16;

/// Total width and height of the texture atlas in pixels.
/// The atlas grid is 16 tiles wide by 32 tiles high (512x1024), providing 512 total tile slots.
pub const ATLAS_WIDTH: u32 = TILE_SIZE * ATLAS_TILES_PER_ROW; // 512
pub const ATLAS_HEIGHT: u32 = TILE_SIZE * ATLAS_TILES_PER_ROW * 2; // 1024

/// Embedded high-resolution Faithful 32x texture atlas.
const FAITHFUL_ATLAS_PNG: &[u8] = include_bytes!("../assets/faithful_atlas_512.png");

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
        let eps = 0.00025; // Clamped subpixel offset to prevent texture bleed at 512x1024

        let u_min = (tile_x * TILE_SIZE as f32) * pixel_u + eps;
        let v_min = (tile_y * TILE_SIZE as f32) * pixel_v + eps;
        let u_max = ((tile_x + 1.0) * TILE_SIZE as f32) * pixel_u - eps;
        let v_max = ((tile_y + 1.0) * TILE_SIZE as f32) * pixel_v - eps;

        [u_min, v_min, u_max, v_max]
    }

    /// Loads the authentic Faithful 32x texture atlas containing all blocks, items, tools,
    /// entities, ores, crops, GUI components, and font glyphs.
    pub fn generate_default() -> Self {
        let dynamic_img = image::load_from_memory(FAITHFUL_ATLAS_PNG).expect(
            "Failed to load embedded Faithful 32x texture atlas (assets/faithful_atlas_512.png)",
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

        // Paint decorative block & item tiles (288..332)
        paint_decorative_tiles(&mut atlas);

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

    // 318: Bed Side (red blanket overhang + white mattress + oak frame & legs)
    atlas.paint_tile(318, |px, py| {
        let n = (hash(px, py, 318) % 11) - 5;
        if py < 12 {
            // Blanket overhang on right 2/3, white pillow/sheet on left 1/3
            if px < 10 {
                Rgba([clamp(234, n), clamp(236, n), clamp(240, n), 255])
            } else {
                Rgba([clamp(176, n), clamp(42, n / 2), clamp(36, n / 2), 255])
            }
        } else if py < 20 {
            // Oak wood bed frame
            Rgba([clamp(158, n), clamp(118, n), clamp(72, n), 255])
        } else {
            // Legs at ends, transparent underneath
            if px < 5 || px >= 27 {
                Rgba([clamp(132, n), clamp(96, n), clamp(56, n), 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        }
    });

    // 319..=325: 7 Flowers
    // 319: Poppy (red)
    paint_flower_tile(atlas, 319, |px, py| {
        let dx = px as i32 - 16;
        let dy = py as i32 - 11;
        let r2 = dx * dx + dy * dy;
        if r2 <= 6 {
            Some(Rgba([45, 30, 25, 255])) // Dark center
        } else if r2 <= 42 {
            Some(Rgba([220, 40, 35, 255])) // Red petals
        } else if px >= 15 && px <= 16 && py >= 14 {
            Some(Rgba([62, 138, 48, 255])) // Stem
        } else if py >= 22 && py <= 26 && ((px >= 12 && px <= 14) || (px >= 17 && px <= 19)) {
            Some(Rgba([72, 156, 56, 255])) // Leaves
        } else {
            None
        }
    });

    // 320: Dandelion (yellow)
    paint_flower_tile(atlas, 320, |px, py| {
        let dx = px as i32 - 16;
        let dy = py as i32 - 12;
        let r2 = dx * dx + dy * dy;
        if r2 <= 30 {
            Some(Rgba([250, 218, 40, 255]))
        } else if px >= 15 && px <= 16 && py >= 16 {
            Some(Rgba([65, 142, 48, 255]))
        } else if py >= 23 && py <= 27 && (px >= 12 && px <= 14) {
            Some(Rgba([76, 162, 58, 255]))
        } else {
            None
        }
    });

    // 321: Blue Orchid
    paint_flower_tile(atlas, 321, |px, py| {
        let dx = px as i32 - 16;
        let dy = py as i32 - 10;
        if dx * dx + dy * dy <= 36 || ((px as i32 - 12).pow(2) + (py as i32 - 16).pow(2) <= 14) {
            Some(Rgba([48, 172, 232, 255]))
        } else if px >= 15 && px <= 16 && py >= 14 {
            Some(Rgba([54, 130, 52, 255]))
        } else {
            None
        }
    });

    // 322: Allium (purple pom-pom)
    paint_flower_tile(atlas, 322, |px, py| {
        let dx = px as i32 - 16;
        let dy = py as i32 - 9;
        let r2 = dx * dx + dy * dy;
        if r2 <= 38 {
            let shade = if (px + py) % 2 == 0 { 20 } else { -12 };
            Some(Rgba([
                clamp(178, shade),
                clamp(88, shade / 2),
                clamp(214, shade),
                255,
            ]))
        } else if px >= 15 && px <= 16 && py >= 14 {
            Some(Rgba([68, 148, 54, 255]))
        } else {
            None
        }
    });

    // 323: Orange Tulip
    paint_flower_tile(atlas, 323, |px, py| {
        paint_tulip_pixel(px, py, [242, 132, 34])
    });

    // 324: Pink Tulip
    paint_flower_tile(atlas, 324, |px, py| {
        paint_tulip_pixel(px, py, [244, 136, 178])
    });

    // 325: Oxeye Daisy
    paint_flower_tile(atlas, 325, |px, py| {
        let dx = px as i32 - 16;
        let dy = py as i32 - 11;
        let r2 = dx * dx + dy * dy;
        if r2 <= 8 {
            Some(Rgba([248, 210, 42, 255])) // Yellow center
        } else if r2 <= 44 && (dx.abs() <= 2 || dy.abs() <= 2 || (dx.abs() - dy.abs()).abs() <= 2) {
            Some(Rgba([244, 246, 250, 255])) // White petals
        } else if px >= 15 && px <= 16 && py >= 14 {
            Some(Rgba([64, 142, 50, 255]))
        } else {
            None
        }
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

fn paint_tulip_pixel(px: u32, py: u32, petal_rgb: [u8; 3]) -> Option<Rgba<u8>> {
    let x = px as i32;
    let y = py as i32;
    if y >= 6 && y <= 14 && x >= 12 && x <= 19 {
        // Tulip cup with notched top
        if y == 6 && (x == 14 || x == 17) {
            return None;
        }
        return Some(Rgba([petal_rgb[0], petal_rgb[1], petal_rgb[2], 255]));
    }
    if x >= 15 && x <= 16 && y >= 15 {
        return Some(Rgba([64, 142, 50, 255]));
    }
    if y >= 21 && y <= 26 && ((x >= 12 && x <= 14) || (x >= 17 && x <= 19)) {
        return Some(Rgba([74, 158, 58, 255]));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atlas_dimensions_and_tiles() {
        assert_eq!(TILE_SIZE, 32);
        assert_eq!(ATLAS_WIDTH, 512);
        assert_eq!(ATLAS_HEIGHT, 1024);

        let atlas = TextureAtlas::generate_default();
        assert_eq!(atlas.image.width(), 512);
        assert_eq!(atlas.image.height(), 1024);

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
}

