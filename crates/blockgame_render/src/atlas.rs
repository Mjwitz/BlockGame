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

        // Paint procedural tiles for Bucket (242) and Water Bucket (243)
        paint_bucket_sprite(&mut atlas, 242, false);
        paint_bucket_sprite(&mut atlas, 243, true);

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
}
