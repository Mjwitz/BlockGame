use image::{Rgba, RgbaImage};

/// Size of each square tile on the atlas in pixels.
/// BlockGame uses 32x32 tiles matching the Faithful 32x texture pack.
pub const TILE_SIZE: u32 = 32;

/// Number of tiles horizontally and vertically across the atlas grid.
pub const ATLAS_TILES_PER_ROW: u32 = 16;

/// Total width and height of the texture atlas in pixels (32 * 16 = 512).
pub const ATLAS_WIDTH: u32 = TILE_SIZE * ATLAS_TILES_PER_ROW; // 512
pub const ATLAS_HEIGHT: u32 = TILE_SIZE * ATLAS_TILES_PER_ROW; // 512

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
        let eps = 0.00025; // Clamped subpixel offset to prevent texture bleed at 512x512

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
        let image = dynamic_img.to_rgba8();
        let mut atlas = Self { image };

        // Paint procedural tiles for Bucket (242), Water Bucket (243), and Moist Farmland (244)
        paint_bucket_sprite(&mut atlas, 242, false);
        paint_bucket_sprite(&mut atlas, 243, true);
        paint_moist_farmland_tile(&mut atlas, 244, 150);

        // Paint Sword sprites (21 to 26)
        paint_sword_sprite(&mut atlas, 21, [180, 130, 80], [150, 105, 60], [115, 75, 40], [120, 80, 45]);
        paint_sword_sprite(&mut atlas, 22, [185, 185, 190], [140, 140, 145], [100, 100, 105], [110, 110, 115]);
        paint_sword_sprite(&mut atlas, 23, [235, 145, 105], [200, 100, 70], [150, 70, 45], [175, 85, 55]);
        paint_sword_sprite(&mut atlas, 24, [240, 242, 245], [195, 200, 208], [145, 150, 160], [160, 165, 175]);
        paint_sword_sprite(&mut atlas, 25, [160, 250, 240], [85, 220, 210], [45, 170, 160], [50, 180, 170]);
        paint_sword_sprite(&mut atlas, 26, [255, 248, 120], [245, 215, 50], [190, 155, 25], [210, 170, 30]);

        // Copy and tint Copper Tools (27 to 30) from Iron tool icons
        copy_and_tint_tool(&mut atlas, 20, 27, [235, 145, 105], [200, 100, 70], [150, 70, 45]); // Copper Pickaxe
        copy_and_tint_tool(&mut atlas, 143, 28, [235, 145, 105], [200, 100, 70], [150, 70, 45]); // Copper Axe
        copy_and_tint_tool(&mut atlas, 147, 29, [235, 145, 105], [200, 100, 70], [150, 70, 45]); // Copper Shovel
        copy_and_tint_tool(&mut atlas, 157, 30, [235, 145, 105], [200, 100, 70], [150, 70, 45]); // Copper Hoe

        // Copy and tint Gold Tools (32 to 35) from Iron tool icons
        copy_and_tint_tool(&mut atlas, 20, 32, [255, 248, 120], [245, 215, 50], [190, 155, 25]); // Gold Pickaxe
        copy_and_tint_tool(&mut atlas, 143, 33, [255, 248, 120], [245, 215, 50], [190, 155, 25]); // Gold Axe
        copy_and_tint_tool(&mut atlas, 147, 34, [255, 248, 120], [245, 215, 50], [190, 155, 25]); // Gold Shovel
        copy_and_tint_tool(&mut atlas, 157, 35, [255, 248, 120], [245, 215, 50], [190, 155, 25]); // Gold Hoe

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atlas_dimensions_and_tiles() {
        assert_eq!(TILE_SIZE, 32);
        assert_eq!(ATLAS_WIDTH, 512);
        assert_eq!(ATLAS_HEIGHT, 512);

        let atlas = TextureAtlas::generate_default();
        assert_eq!(atlas.image.width(), 512);
        assert_eq!(atlas.image.height(), 512);

        // Verify stone tile (Tile 1) is non-transparent
        let stone_px = atlas.image.get_pixel(32 + 16, 16);
        assert_eq!(stone_px[3], 255);

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
}
