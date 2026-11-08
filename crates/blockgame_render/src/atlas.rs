use image::{Rgba, RgbaImage};

pub const TILE_SIZE: u32 = 16;
pub const ATLAS_TILES_PER_ROW: u32 = 16;
pub const ATLAS_WIDTH: u32 = TILE_SIZE * ATLAS_TILES_PER_ROW; // 256
pub const ATLAS_HEIGHT: u32 = TILE_SIZE * ATLAS_TILES_PER_ROW; // 256

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
        let eps = 0.0005; // half-pixel/epsilon clamp to prevent bleeding

        let u_min = (tile_x * TILE_SIZE as f32) * pixel_u + eps;
        let v_min = (tile_y * TILE_SIZE as f32) * pixel_v + eps;
        let u_max = ((tile_x + 1.0) * TILE_SIZE as f32) * pixel_u - eps;
        let v_max = ((tile_y + 1.0) * TILE_SIZE as f32) * pixel_v - eps;

        [u_min, v_min, u_max, v_max]
    }

    /// Procedurally bakes a default texture atlas containing all basic voxel blocks.
    pub fn generate_default() -> Self {
        let mut atlas = Self::new();

        // 1: Stone
        atlas.paint_tile(1, |x, y| {
            let n = simple_noise(x, y, 11);
            let val = (120.0 + n * 30.0) as u8;
            Rgba([val, val, val, 255])
        });

        // 2: Dirt
        atlas.paint_tile(2, |x, y| {
            let n = simple_noise(x, y, 22);
            let r = (134.0 + n * 24.0) as u8;
            let g = (96.0 + n * 18.0) as u8;
            let b = (67.0 + n * 14.0) as u8;
            Rgba([r, g, b, 255])
        });

        // 3: Grass Top
        atlas.paint_tile(3, |x, y| {
            let n = simple_noise(x, y, 33);
            let r = (76.0 + n * 20.0) as u8;
            let g = (145.0 + n * 30.0) as u8;
            let b = (54.0 + n * 16.0) as u8;
            Rgba([r, g, b, 255])
        });

        // 4: Grass Side (Green rim on top of dirt)
        atlas.paint_tile(4, |x, y| {
            let fringe = if x % 3 == 0 { 4 } else if x % 2 == 0 { 3 } else { 2 };
            if y < fringe {
                let n = simple_noise(x, y, 33);
                let r = (76.0 + n * 20.0) as u8;
                let g = (145.0 + n * 30.0) as u8;
                let b = (54.0 + n * 16.0) as u8;
                Rgba([r, g, b, 255])
            } else {
                let n = simple_noise(x, y, 22);
                let r = (134.0 + n * 24.0) as u8;
                let g = (96.0 + n * 18.0) as u8;
                let b = (67.0 + n * 14.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 5: Cobblestone
        atlas.paint_tile(5, |x, y| {
            let is_mortar = (y == 0 || y == 5 || y == 10 || y == 15)
                || (y < 5 && x == 7)
                || (y >= 5 && y < 10 && (x == 3 || x == 12))
                || (y >= 10 && y < 15 && x == 8);
            if is_mortar {
                Rgba([65, 65, 65, 255])
            } else {
                let n = simple_noise(x, y, 55);
                let val = (130.0 + n * 35.0) as u8;
                Rgba([val, val, val, 255])
            }
        });

        // 6: Wood Log Side
        atlas.paint_tile(6, |x, y| {
            let is_groove = x % 4 == 0;
            let n = simple_noise(x, y, 66);
            if is_groove {
                Rgba([75, 59, 36, 255])
            } else {
                let r = (103.0 + n * 15.0) as u8;
                let g = (82.0 + n * 12.0) as u8;
                let b = (50.0 + n * 8.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 7: Wood Log Top
        atlas.paint_tile(7, |x, y| {
            let dx = (x as f32) - 7.5;
            let dy = (y as f32) - 7.5;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > 6.8 {
                Rgba([75, 59, 36, 255]) // Bark ring
            } else {
                let ring = ((dist * 2.5).sin() * 0.5 + 0.5) * 20.0;
                let r = (160.0 + ring) as u8;
                let g = (132.0 + ring) as u8;
                let b = (87.0 + ring) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 8: Leaves
        atlas.paint_tile(8, |x, y| {
            let n = simple_noise(x, y, 88);
            let is_hole = (x + y * 3) % 7 == 0;
            if is_hole {
                Rgba([40, 95, 30, 200])
            } else {
                let r = (46.0 + n * 20.0) as u8;
                let g = (110.0 + n * 35.0) as u8;
                let b = (36.0 + n * 16.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 9: Sand
        atlas.paint_tile(9, |x, y| {
            let n = simple_noise(x, y, 99);
            let r = (215.0 + n * 20.0) as u8;
            let g = (200.0 + n * 18.0) as u8;
            let b = (150.0 + n * 15.0) as u8;
            Rgba([r, g, b, 255])
        });

        // 10: Planks
        atlas.paint_tile(10, |x, y| {
            let is_groove = y % 4 == 0;
            let is_nail = (x == 2 || x == 13) && (y % 4 == 2);
            if is_groove {
                Rgba([115, 84, 45, 255])
            } else if is_nail {
                Rgba([70, 50, 25, 255])
            } else {
                let n = simple_noise(x, y, 101);
                let r = (161.0 + n * 16.0) as u8;
                let g = (122.0 + n * 14.0) as u8;
                let b = (73.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 11: Glass
        atlas.paint_tile(11, |x, y| {
            let is_border = x == 0 || x == 15 || y == 0 || y == 15;
            let is_glare = (x + y == 6 && x >= 2 && x <= 4)
                || (x + y == 7 && x >= 3 && x <= 4)
                || (x == 12 && y == 12)
                || (x == 13 && y == 11);
            if is_border {
                Rgba([200, 230, 255, 255])
            } else if is_glare {
                Rgba([245, 250, 255, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 12: Bedrock
        atlas.paint_tile(12, |x, y| {
            let n = simple_noise(x, y, 122);
            let val = (30.0 + n * 35.0) as u8;
            Rgba([val, val, val, 255])
        });

        // 13: Torch
        atlas.paint_tile(13, |x, y| {
            let is_stick = (x == 7 || x == 8) && y >= 6;
            let is_flame_core = (x == 7 || x == 8) && (y >= 2 && y <= 4);
            let is_flame_outer = (x >= 6 && x <= 9) && (y >= 1 && y <= 5);

            if is_flame_core {
                Rgba([255, 255, 180, 255])
            } else if is_flame_outer {
                Rgba([255, 140, 20, 255])
            } else if is_stick {
                Rgba([115, 84, 45, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 14: Stick
        atlas.paint_tile(14, |x, y| {
            let on_diag = (x + y == 14) || (x + y == 15);
            let in_bounds = x >= 3 && x <= 12 && y >= 3 && y <= 12;
            if on_diag && in_bounds {
                Rgba([130, 95, 50, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 15: Wooden Pickaxe
        atlas.paint_tile(15, |x, y| {
            let is_handle = (x + y == 15 || x + y == 16) && x <= 10 && y >= 5;
            let is_head = (y <= 4 && x >= 3 && x <= 13) || (x == 3 && y <= 6) || (x == 13 && y <= 6);
            if is_head {
                Rgba([160, 120, 70, 255]) // Wood head
            } else if is_handle {
                Rgba([100, 70, 35, 255]) // Stick handle
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 16: Stone Pickaxe
        atlas.paint_tile(16, |x, y| {
            let is_handle = (x + y == 15 || x + y == 16) && x <= 10 && y >= 5;
            let is_head = (y <= 4 && x >= 3 && x <= 13) || (x == 3 && y <= 6) || (x == 13 && y <= 6);
            if is_head {
                let n = simple_noise(x, y, 77);
                let val = (130.0 + n * 30.0) as u8;
                Rgba([val, val, val, 255]) // Stone head
            } else if is_handle {
                Rgba([100, 70, 35, 255]) // Stick handle
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 17: Crafting Table
        atlas.paint_tile(17, |x, y| {
            let is_border = x == 0 || x == 15 || y == 0 || y == 15;
            let is_grid = x == 7 || x == 8 || y == 7 || y == 8;
            if is_border {
                Rgba([90, 60, 30, 255])
            } else if is_grid {
                Rgba([120, 85, 45, 255])
            } else {
                let n = simple_noise(x, y, 117);
                let r = (165.0 + n * 15.0) as u8;
                let g = (125.0 + n * 12.0) as u8;
                let b = (75.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 18: Coal
        atlas.paint_tile(18, |x, y| {
            let dx = (x as f32) - 7.5;
            let dy = (y as f32) - 7.5;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 5.5 {
                let n = simple_noise(x, y, 18);
                let val = (35.0 + n * 20.0).max(10.0) as u8;
                Rgba([val, val, (val as f32 * 1.1) as u8, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 19: Iron Ingot
        atlas.paint_tile(19, |x, y| {
            let in_bar = x >= 3 && x <= 12 && y >= 6 && y <= 10;
            let is_highlight = y == 6 && x >= 4 && x <= 11;
            if is_highlight {
                Rgba([240, 245, 255, 255])
            } else if in_bar {
                let n = simple_noise(x, y, 19);
                let val = (190.0 + n * 20.0) as u8;
                Rgba([val, val, (val as f32 * 1.05).min(255.0) as u8, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 20: Iron Pickaxe
        atlas.paint_tile(20, |x, y| {
            let is_handle = (x + y == 15 || x + y == 16) && x <= 10 && y >= 5;
            let is_head = (y <= 4 && x >= 3 && x <= 13) || (x == 3 && y <= 6) || (x == 13 && y <= 6);
            if is_head {
                let is_shine = y == 1 || (x == 3 && y == 2);
                if is_shine {
                    Rgba([245, 250, 255, 255])
                } else {
                    Rgba([190, 195, 205, 255])
                }
            } else if is_handle {
                Rgba([100, 70, 35, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 21: Crosshair UI
        atlas.paint_tile(21, |x, y| {
            let is_center = (x >= 7 && x <= 8 && y >= 4 && y <= 11)
                || (y >= 7 && y <= 8 && x >= 4 && x <= 11);
            let is_border = (x >= 6 && x <= 9 && y >= 3 && y <= 12)
                || (y >= 6 && y <= 9 && x >= 3 && x <= 12);

            if is_center {
                Rgba([245, 245, 245, 230])
            } else if is_border {
                Rgba([20, 20, 20, 140])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 22: Hotbar Slot Frame
        atlas.paint_tile(22, |x, y| {
            let is_outer = x == 0 || x == 15 || y == 0 || y == 15;
            let is_inset = x == 1 || y == 1;
            let is_shadow = x == 14 || y == 14;

            if is_outer {
                Rgba([35, 35, 35, 230])
            } else if is_inset {
                Rgba([140, 140, 140, 200])
            } else if is_shadow {
                Rgba([50, 50, 50, 200])
            } else {
                Rgba([85, 85, 85, 180])
            }
        });

        // 23: Hotbar Selected Highlight Frame
        atlas.paint_tile(23, |x, y| {
            let is_border_outer = x == 0 || x == 15 || y == 0 || y == 15;
            let is_border_inner = x == 1 || x == 14 || y == 1 || y == 14;

            if is_border_outer {
                Rgba([255, 255, 255, 255])
            } else if is_border_inner {
                Rgba([255, 220, 50, 255]) // Gold inner glow
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 24: Solid White Quad (for UI boxes, tooltip backgrounds, tinted overlays)
        atlas.paint_tile(24, |_x, _y| Rgba([255, 255, 255, 255]));

        // 32..=126: ASCII Pixel Font (Tile index == ASCII byte code)
        for c in crate::font::FONT_FIRST_CHAR..=crate::font::FONT_LAST_CHAR {
            let glyph = crate::font::get_char_glyph(c);
            atlas.paint_tile(c as u16, |x, y| {
                let gx = (x / 2).min(7) as usize;
                let gy = (y / 2).min(7) as usize;
                let is_fg = ((glyph[gy] >> (7 - gx)) & 1) == 1;

                if is_fg {
                    Rgba([255, 255, 255, 255])
                } else if x >= 1 && y >= 1 {
                    let sx = ((x - 1) / 2).min(7) as usize;
                    let sy = ((y - 1) / 2).min(7) as usize;
                    let is_shadow = ((glyph[sy] >> (7 - sx)) & 1) == 1;
                    if is_shadow {
                        Rgba([35, 35, 35, 230])
                    } else {
                        Rgba([0, 0, 0, 0])
                    }
                } else {
                    Rgba([0, 0, 0, 0])
                }
            });
        }

        // 128: Crafting Table Side (Wood planks background with hanging saw and hammer tools)
        atlas.paint_tile(128, |x, y| {
            let is_border = x == 0 || x == 15 || y == 0 || y == 15;
            let is_seam = y == 5 || y == 10 || (y < 5 && x == 8) || (y >= 5 && y < 10 && x == 4) || (y >= 10 && x == 11);
            let is_saw_handle = (x >= 3 && x <= 5) && (y >= 2 && y <= 3);
            let is_saw_blade = (x >= 3 && x <= 5 && y >= 4 && y <= 11) || (x == 6 && (y == 5 || y == 7 || y == 9));
            let is_hammer_head = (x >= 9 && x <= 13) && (y >= 4 && y <= 6);
            let is_hammer_handle = (x == 11 && y >= 7 && y <= 12) || (x == 12 && y == 12);

            if is_border {
                Rgba([90, 60, 30, 255])
            } else if is_saw_handle {
                Rgba([130, 80, 35, 255])
            } else if is_saw_blade {
                if x == 4 && y >= 5 && y <= 10 {
                    Rgba([230, 235, 245, 255]) // Saw shine
                } else {
                    Rgba([175, 180, 190, 255])
                }
            } else if is_hammer_head {
                if y == 4 {
                    Rgba([210, 215, 225, 255])
                } else {
                    Rgba([135, 140, 150, 255])
                }
            } else if is_hammer_handle {
                Rgba([115, 75, 35, 255])
            } else if is_seam {
                Rgba([110, 75, 40, 255])
            } else {
                let n = simple_noise(x, y, 128);
                let r = (165.0 + n * 18.0) as u8;
                let g = (125.0 + n * 15.0) as u8;
                let b = (70.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // Common ore fleck coordinate checker
        let is_ore_fleck = |x: u32, y: u32| -> bool {
            (x == 3 && y == 4) || (x == 4 && (y == 4 || y == 5))
                || (x == 5 && y == 5)
                || (x == 10 && y == 3) || (x == 11 && (y == 3 || y == 4))
                || (x == 7 && y == 8) || (x == 8 && (y == 8 || y == 9))
                || (x == 3 && y == 11) || (x == 4 && (y == 11 || y == 12)) || (x == 5 && y == 12)
                || (x == 11 && y == 10) || (x == 12 && (y == 10 || y == 11)) || (x == 13 && y == 11)
        };

        // 129: Coal Ore
        atlas.paint_tile(129, |x, y| {
            if is_ore_fleck(x, y) {
                let n = simple_noise(x, y, 129);
                if n > 0.1 {
                    Rgba([55, 55, 60, 255])
                } else {
                    Rgba([25, 25, 28, 255])
                }
            } else {
                let n = simple_noise(x, y, 11);
                let val = (120.0 + n * 30.0) as u8;
                Rgba([val, val, val, 255])
            }
        });

        // 130: Copper Ore
        atlas.paint_tile(130, |x, y| {
            if is_ore_fleck(x, y) {
                let n = simple_noise(x, y, 130);
                if n > 0.2 {
                    Rgba([75, 175, 150, 255]) // Turquoise oxidized copper speck
                } else if n > -0.1 {
                    Rgba([215, 115, 65, 255]) // Metallic copper
                } else {
                    Rgba([160, 80, 45, 255])
                }
            } else {
                let n = simple_noise(x, y, 11);
                let val = (120.0 + n * 30.0) as u8;
                Rgba([val, val, val, 255])
            }
        });

        // 131: Iron Ore
        atlas.paint_tile(131, |x, y| {
            if is_ore_fleck(x, y) {
                let n = simple_noise(x, y, 131);
                if n > 0.1 {
                    Rgba([235, 205, 180, 255]) // Highlight
                } else {
                    Rgba([190, 155, 125, 255]) // Iron mineral
                }
            } else {
                let n = simple_noise(x, y, 11);
                let val = (120.0 + n * 30.0) as u8;
                Rgba([val, val, val, 255])
            }
        });

        // 132: Gold Ore
        atlas.paint_tile(132, |x, y| {
            if is_ore_fleck(x, y) {
                let n = simple_noise(x, y, 132);
                if n > 0.15 {
                    Rgba([255, 240, 110, 255]) // Gold shine
                } else {
                    Rgba([245, 195, 35, 255]) // Rich gold
                }
            } else {
                let n = simple_noise(x, y, 11);
                let val = (120.0 + n * 30.0) as u8;
                Rgba([val, val, val, 255])
            }
        });

        // 133: Diamond Ore
        atlas.paint_tile(133, |x, y| {
            if is_ore_fleck(x, y) {
                let n = simple_noise(x, y, 133);
                if n > 0.15 {
                    Rgba([210, 255, 255, 255]) // Diamond gleam
                } else {
                    Rgba([65, 225, 235, 255]) // Cyan diamond
                }
            } else {
                let n = simple_noise(x, y, 11);
                let val = (120.0 + n * 30.0) as u8;
                Rgba([val, val, val, 255])
            }
        });

        // 134: Raw Copper Item
        atlas.paint_tile(134, |x, y| {
            let dx = (x as f32) - 7.5;
            let dy = (y as f32) - 7.5;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 5.0 {
                let n = simple_noise(x, y, 134);
                if n > 0.15 {
                    Rgba([75, 175, 145, 255])
                } else if n > -0.15 {
                    Rgba([205, 110, 60, 255])
                } else {
                    Rgba([150, 75, 40, 255])
                }
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 135: Copper Ingot
        atlas.paint_tile(135, |x, y| {
            let in_bar = x >= 3 && x <= 12 && y >= 6 && y <= 10;
            let is_highlight = y == 6 && x >= 4 && x <= 11;
            if is_highlight {
                Rgba([245, 165, 120, 255])
            } else if in_bar {
                let n = simple_noise(x, y, 135);
                let r = (215.0 + n * 20.0) as u8;
                let g = (115.0 + n * 15.0) as u8;
                let b = (65.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 136: Raw Iron Item
        atlas.paint_tile(136, |x, y| {
            let dx = (x as f32) - 7.5;
            let dy = (y as f32) - 7.5;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 5.0 {
                let n = simple_noise(x, y, 136);
                let val = (185.0 + n * 30.0) as u8;
                Rgba([val, (val as f32 * 0.88) as u8, (val as f32 * 0.75) as u8, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 137: Raw Gold Item
        atlas.paint_tile(137, |x, y| {
            let dx = (x as f32) - 7.5;
            let dy = (y as f32) - 7.5;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 5.0 {
                let n = simple_noise(x, y, 137);
                if n > 0.15 {
                    Rgba([255, 230, 85, 255])
                } else {
                    Rgba([235, 185, 30, 255])
                }
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 138: Gold Ingot
        atlas.paint_tile(138, |x, y| {
            let in_bar = x >= 3 && x <= 12 && y >= 6 && y <= 10;
            let is_highlight = y == 6 && x >= 4 && x <= 11;
            if is_highlight {
                Rgba([255, 240, 130, 255])
            } else if in_bar {
                let n = simple_noise(x, y, 138);
                let r = (240.0 + n * 15.0) as u8;
                let g = (195.0 + n * 15.0) as u8;
                let b = (35.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 139: Diamond Gem
        atlas.paint_tile(139, |x, y| {
            let in_gem = (y >= 4 && y <= 6 && x >= 5 && x <= 10)
                || (y == 7 && x >= 4 && x <= 11)
                || (y == 8 && x >= 5 && x <= 10)
                || (y == 9 && x >= 6 && x <= 9)
                || (y == 10 && x >= 7 && x <= 8);
            let is_top_shine = y == 4 && x >= 6 && x <= 9;
            let is_facet_shine = (x == 5 && y == 7) || (x == 6 && y == 8);

            if is_top_shine || is_facet_shine {
                Rgba([215, 255, 255, 255])
            } else if in_gem {
                let n = simple_noise(x, y, 139);
                let r = (50.0 + n * 15.0) as u8;
                let g = (215.0 + n * 20.0) as u8;
                let b = (235.0 + n * 15.0) as u8;
                Rgba([r, g, b, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 140: Diamond Pickaxe
        atlas.paint_tile(140, |x, y| {
            let is_handle = (x + y == 15 || x + y == 16) && x <= 10 && y >= 5;
            let is_head = (y <= 4 && x >= 3 && x <= 13) || (x == 3 && y <= 6) || (x == 13 && y <= 6);
            if is_head {
                let is_shine = y == 1 || (x == 3 && y == 2);
                if is_shine {
                    Rgba([215, 255, 255, 255])
                } else {
                    Rgba([65, 225, 235, 255])
                }
            } else if is_handle {
                Rgba([100, 70, 35, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 141..144: Axes (Wood, Stone, Iron, Diamond)
        let paint_axe = |atlas: &mut TextureAtlas, tile_id: u16, head_color: Rgba<u8>, shine_color: Rgba<u8>| {
            atlas.paint_tile(tile_id, |x, y| {
                let is_handle = (x + y == 15 || x + y == 16) && x <= 11 && y >= 4;
                let is_blade = (x >= 8 && x <= 12 && y >= 2 && y <= 4)
                    || (x >= 6 && x <= 10 && y >= 4 && y <= 8);
                let is_edge = (x == 6 && y >= 4 && y <= 7) || (y == 2 && x >= 8 && x <= 11);

                if is_edge {
                    shine_color
                } else if is_blade {
                    head_color
                } else if is_handle {
                    Rgba([100, 70, 35, 255])
                } else {
                    Rgba([0, 0, 0, 0])
                }
            });
        };
        paint_axe(&mut atlas, 141, Rgba([155, 115, 65, 255]), Rgba([185, 145, 90, 255])); // Wood Axe
        paint_axe(&mut atlas, 142, Rgba([125, 125, 125, 255]), Rgba([165, 165, 165, 255])); // Stone Axe
        paint_axe(&mut atlas, 143, Rgba([190, 195, 205, 255]), Rgba([245, 250, 255, 255])); // Iron Axe
        paint_axe(&mut atlas, 144, Rgba([65, 225, 235, 255]), Rgba([215, 255, 255, 255])); // Diamond Axe

        // 145..148: Shovels (Wood, Stone, Iron, Diamond)
        let paint_shovel = |atlas: &mut TextureAtlas, tile_id: u16, head_color: Rgba<u8>, shine_color: Rgba<u8>| {
            atlas.paint_tile(tile_id, |x, y| {
                let is_handle = (x + y == 15 || x + y == 16) && x <= 10 && y >= 5;
                let dx = (x as i32 - 12).abs();
                let dy = (y as i32 - 3).abs();
                let is_spade = dx + dy <= 2;
                let is_tip = x == 13 && y == 2;

                if is_tip {
                    shine_color
                } else if is_spade {
                    head_color
                } else if is_handle {
                    Rgba([100, 70, 35, 255])
                } else {
                    Rgba([0, 0, 0, 0])
                }
            });
        };
        paint_shovel(&mut atlas, 145, Rgba([155, 115, 65, 255]), Rgba([185, 145, 90, 255])); // Wood Shovel
        paint_shovel(&mut atlas, 146, Rgba([125, 125, 125, 255]), Rgba([165, 165, 165, 255])); // Stone Shovel
        paint_shovel(&mut atlas, 147, Rgba([190, 195, 205, 255]), Rgba([245, 250, 255, 255])); // Iron Shovel
        paint_shovel(&mut atlas, 148, Rgba([65, 225, 235, 255]), Rgba([215, 255, 255, 255])); // Diamond Shovel

        atlas
    }

    fn paint_tile<F>(&mut self, tile_id: u16, mut pixel_fn: F)
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
}

/// Simple pseudo-random hash for procedural pixel textures
fn simple_noise(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = (x.wrapping_mul(374761393))
        ^ (y.wrapping_mul(668265263))
        ^ (seed.wrapping_mul(1274126177));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    (h & 0xFFFF) as f32 / 65535.0 - 0.5
}
