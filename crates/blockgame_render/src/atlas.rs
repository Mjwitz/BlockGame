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
        let dynamic_img = image::load_from_memory(FAITHFUL_ATLAS_PNG)
            .expect("Failed to load embedded Faithful 32x texture atlas (assets/faithful_atlas_512.png)");
        let image = dynamic_img.to_rgba8();
        Self { image }
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
                self.image.put_pixel(tile_x + px, tile_y + py, *tile_image.get_pixel(px, py));
            }
        }
    }
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
}
