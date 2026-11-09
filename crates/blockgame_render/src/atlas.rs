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

        // 17: Crafting Table Top (Rich birch/oak carpenter surface with 3x3 grid & markings)
        atlas.paint_tile(17, |x, y| {
            let is_outer_border = x == 0 || x == 15 || y == 0 || y == 15;
            let is_inner_border = x == 1 || x == 14 || y == 1 || y == 14;
            let is_corner_bracket = (x <= 2 && y <= 2)
                || (x >= 13 && y <= 2)
                || (x <= 2 && y >= 13)
                || (x >= 13 && y >= 13);
            let is_grid_x = x == 5 || x == 10;
            let is_grid_y = y == 5 || y == 10;
            let is_ruler_tick = (y == 1 || y == 2) && (x % 2 == 0);

            if is_corner_bracket {
                Rgba([60, 40, 20, 255]) // Dark iron/wood corner reinforcement
            } else if is_outer_border {
                Rgba([85, 55, 25, 255]) // Outer border frame
            } else if is_ruler_tick {
                Rgba([60, 35, 15, 255]) // Carpenter measuring ticks
            } else if is_inner_border {
                Rgba([110, 75, 38, 255]) // Beveled border
            } else if is_grid_x || is_grid_y {
                Rgba([115, 80, 42, 255]) // Inlaid 3x3 grid lines
            } else {
                let n = simple_noise(x, y, 117);
                let r = (195.0 + n * 18.0) as u8;
                let g = (150.0 + n * 15.0) as u8;
                let b = (95.0 + n * 12.0) as u8;
                Rgba([r, g, b, 255]) // Rich, bright oak carpenter tabletop
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

        // -------------------------------------------------------------
        // 25..27: Player Health Hearts (Empty Container, Full, Half)
        // -------------------------------------------------------------
        let heart_grid = [
            "................",
            "................",
            "...##......##...",
            "..#XX#....#XX#..",
            ".#XXXX#..#XXXX#.",
            ".#XXXXXXXXXXXX#.",
            ".#XXXXXXXXXXXX#.",
            "..#XXXXXXXXXX#..",
            "...#XXXXXXXX#...",
            "....#XXXXXX#....",
            ".....#XXXX#.....",
            "......#XX#......",
            ".......##.......",
            "................",
            "................",
            "................",
        ];

        // 25: HEART_CONTAINER
        atlas.paint_tile(25, |x, y| {
            let row = heart_grid[y as usize].as_bytes();
            match row[x as usize] {
                b'#' => Rgba([35, 12, 12, 255]),
                b'X' => Rgba([65, 40, 40, 220]),
                _ => Rgba([0, 0, 0, 0]),
            }
        });

        // 26: HEART_FULL
        atlas.paint_tile(26, |x, y| {
            let row = heart_grid[y as usize].as_bytes();
            match row[x as usize] {
                b'#' => Rgba([35, 12, 12, 255]),
                b'X' => {
                    let is_shine = (x == 4 && (y == 3 || y == 4)) || (x == 5 && y == 4);
                    let is_shadow = x >= 9 && y >= 7;
                    if is_shine {
                        Rgba([255, 255, 255, 255])
                    } else if is_shadow {
                        Rgba([160, 15, 15, 255])
                    } else {
                        Rgba([230, 25, 25, 255])
                    }
                }
                _ => Rgba([0, 0, 0, 0]),
            }
        });

        // 27: HEART_HALF
        atlas.paint_tile(27, |x, y| {
            let row = heart_grid[y as usize].as_bytes();
            match row[x as usize] {
                b'#' => Rgba([35, 12, 12, 255]),
                b'X' => {
                    if x <= 7 {
                        let is_shine = (x == 4 && (y == 3 || y == 4)) || (x == 5 && y == 4);
                        if is_shine {
                            Rgba([255, 255, 255, 255])
                        } else {
                            Rgba([230, 25, 25, 255])
                        }
                    } else {
                        Rgba([65, 40, 40, 220])
                    }
                }
                _ => Rgba([0, 0, 0, 0]),
            }
        });

        // -------------------------------------------------------------
        // 28..30: Player Hunger Drumsticks (Empty Container, Full, Half)
        // -------------------------------------------------------------
        let drumstick_grid = [
            "................",
            "..........####..",
            "........##MMMM#.",
            ".......#MMMMMM#.",
            "......#MMMMMMM#.",
            ".....#MMMMMMMM#.",
            "....#MMMMMMMM#..",
            "....#MMMMMMM#...",
            "...#SMMMMMM#....",
            "...#SSMMM##.....",
            "..#BSS###.......",
            ".#BB##..........",
            ".#B#............",
            "..#.............",
            "................",
            "................",
        ];

        // 28: HUNGER_CONTAINER
        atlas.paint_tile(28, |x, y| {
            let row = drumstick_grid[y as usize].as_bytes();
            match row[x as usize] {
                b'#' => Rgba([40, 25, 15, 255]),
                b'B' => Rgba([90, 80, 75, 200]),
                b'M' | b'S' => Rgba([60, 42, 32, 220]),
                _ => Rgba([0, 0, 0, 0]),
            }
        });

        // 29: HUNGER_FULL
        atlas.paint_tile(29, |x, y| {
            let row = drumstick_grid[y as usize].as_bytes();
            match row[x as usize] {
                b'#' => Rgba([40, 25, 15, 255]),
                b'B' => Rgba([235, 230, 220, 255]),
                b'M' => {
                    let is_highlight = y <= 4 || (x >= 9 && y <= 6);
                    if is_highlight {
                        Rgba([235, 155, 75, 255])
                    } else {
                        Rgba([185, 95, 30, 255])
                    }
                }
                b'S' => Rgba([125, 55, 18, 255]),
                _ => Rgba([0, 0, 0, 0]),
            }
        });

        // 30: HUNGER_HALF
        atlas.paint_tile(30, |x, y| {
            let row = drumstick_grid[y as usize].as_bytes();
            match row[x as usize] {
                b'#' => Rgba([40, 25, 15, 255]),
                b'B' => Rgba([235, 230, 220, 255]),
                b'M' => {
                    if x >= 8 {
                        let is_highlight = y <= 4 || (x >= 9 && y <= 6);
                        if is_highlight {
                            Rgba([235, 155, 75, 255])
                        } else {
                            Rgba([185, 95, 30, 255])
                        }
                    } else {
                        Rgba([60, 42, 32, 220])
                    }
                }
                b'S' => {
                    if x >= 8 {
                        Rgba([125, 55, 18, 255])
                    } else {
                        Rgba([60, 42, 32, 220])
                    }
                }
                _ => Rgba([0, 0, 0, 0]),
            }
        });

        // 31: Apple Food Item
        atlas.paint_tile(31, |x, y| {
            let is_stem = (x == 7 || x == 8) && (y >= 1 && y <= 3);
            let is_leaf = (x >= 9 && x <= 11) && (y >= 1 && y <= 3);
            let dx = (x as f32) - 7.5;
            let dy = (y as f32) - 8.5;
            let dist = (dx * dx + dy * dy).sqrt();
            let is_apple_body = dist <= 5.2 && y >= 4;

            if is_leaf {
                Rgba([65, 185, 45, 255]) // Green leaf
            } else if is_stem {
                Rgba([115, 75, 35, 255]) // Brown stem
            } else if is_apple_body {
                let is_shine = (x == 5 && y == 6) || (x == 6 && (y == 6 || y == 7));
                let is_shadow = y >= 11 || x >= 11;
                if is_shine {
                    Rgba([255, 130, 130, 255])
                } else if is_shadow {
                    Rgba([160, 15, 15, 255])
                } else {
                    Rgba([225, 25, 25, 255])
                }
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

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

        // 150: Farmland Top (Tilled moist soil with furrows)
        atlas.paint_tile(150, |x, y| {
            let is_furrow = y % 4 == 0;
            let n = simple_noise(x, y, 150);
            if is_furrow {
                let r = (65.0 + n * 12.0) as u8;
                let g = (42.0 + n * 8.0) as u8;
                let b = (25.0 + n * 6.0) as u8;
                Rgba([r, g, b, 255])
            } else {
                let r = (95.0 + n * 15.0) as u8;
                let g = (65.0 + n * 10.0) as u8;
                let b = (40.0 + n * 8.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 151..=154: Wheat stages 0..3
        // Stage 0: tiny green sprouts (y: 12..15)
        atlas.paint_tile(151, |x, y| {
            let is_sprout = y >= 12 && (x == 3 || x == 7 || x == 11 || x == 14);
            if is_sprout {
                let n = simple_noise(x, y, 151);
                let g = (160.0 + n * 25.0) as u8;
                Rgba([70, g, 40, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // Stage 1: small growing stalks (y: 8..15)
        atlas.paint_tile(152, |x, y| {
            let is_stalk = y >= 8 && (x % 3 == 1 || (y >= 10 && x % 2 == 0));
            if is_stalk {
                let n = simple_noise(x, y, 152);
                let g = (175.0 + n * 30.0) as u8;
                Rgba([85, g, 45, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // Stage 2: taller yellowish-green wheat with small heads (y: 4..15)
        atlas.paint_tile(153, |x, y| {
            let is_stem = y >= 4 && (x % 2 == 0 || (y >= 8 && x % 3 == 0));
            let is_head = y >= 4 && y <= 7 && (x % 2 == 0);
            if is_head {
                Rgba([185, 170, 60, 255])
            } else if is_stem {
                Rgba([120, 165, 50, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // Stage 3: full golden ripe wheat (y: 1..15)
        atlas.paint_tile(154, |x, y| {
            let is_wheat = y >= 1 && (x % 2 == 0 || (y >= 6 && x % 2 == 1));
            let is_grain = y >= 1 && y <= 6 && (x >= 2 && x <= 14);
            if is_grain {
                let n = simple_noise(x, y, 154);
                let r = (225.0 + n * 20.0) as u8;
                let g = (185.0 + n * 20.0) as u8;
                Rgba([r, g, 50, 255])
            } else if is_wheat {
                Rgba([190, 155, 45, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 155..=158: Hoes (Wood, Stone, Iron, Diamond)
        let paint_hoe = |atlas: &mut TextureAtlas, tile_id: u16, blade_color: Rgba<u8>, shine_color: Rgba<u8>| {
            atlas.paint_tile(tile_id, |x, y| {
                let is_handle = (x + y == 15 || x + y == 16) && x <= 10 && y >= 5;
                let is_blade = (y == 3 && x >= 9 && x <= 13) || (y == 4 && x >= 12 && x <= 13);
                let is_edge = x == 13 && y == 4;

                if is_edge {
                    shine_color
                } else if is_blade {
                    blade_color
                } else if is_handle {
                    Rgba([100, 70, 35, 255])
                } else {
                    Rgba([0, 0, 0, 0])
                }
            });
        };
        paint_hoe(&mut atlas, 155, Rgba([155, 115, 65, 255]), Rgba([185, 145, 90, 255])); // Wood Hoe
        paint_hoe(&mut atlas, 156, Rgba([125, 125, 125, 255]), Rgba([165, 165, 165, 255])); // Stone Hoe
        paint_hoe(&mut atlas, 157, Rgba([190, 195, 205, 255]), Rgba([245, 250, 255, 255])); // Iron Hoe
        paint_hoe(&mut atlas, 158, Rgba([65, 225, 235, 255]), Rgba([215, 255, 255, 255])); // Diamond Hoe

        // 159: Wheat Seeds
        atlas.paint_tile(159, |x, y| {
            let s1 = (x as i32 - 6).abs() + (y as i32 - 6).abs() <= 2;
            let s2 = (x as i32 - 10).abs() + (y as i32 - 8).abs() <= 2;
            let s3 = (x as i32 - 7).abs() + (y as i32 - 11).abs() <= 2;
            if s1 || s2 || s3 {
                let is_highlight = (x == 6 && y == 5) || (x == 10 && y == 7) || (x == 7 && y == 10);
                if is_highlight {
                    Rgba([190, 215, 110, 255])
                } else {
                    Rgba([120, 145, 65, 255])
                }
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 160: Wheat (Bundle)
        atlas.paint_tile(160, |x, y| {
            let is_sheaf = (x >= 4 && x <= 11 && y >= 3 && y <= 13)
                || (x >= 3 && x <= 12 && y >= 4 && y <= 8);
            let is_tie = y == 9 && x >= 4 && x <= 11;
            let is_top = y <= 5 && is_sheaf;
            if is_tie {
                Rgba([140, 70, 30, 255]) // red/brown string tie
            } else if is_top {
                Rgba([230, 195, 60, 255])
            } else if is_sheaf {
                Rgba([195, 160, 45, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 161: Bread
        atlas.paint_tile(161, |x, y| {
            let in_loaf = x >= 2 && x <= 13 && y >= 5 && y <= 11;
            let is_crust_cut = in_loaf && (x == 5 || x == 8 || x == 11) && y >= 5 && y <= 8;
            let is_top_crust = in_loaf && y == 5;
            if is_crust_cut {
                Rgba([235, 210, 150, 255]) // exposed interior crumb
            } else if is_top_crust {
                Rgba([145, 80, 25, 255])   // baked top crust
            } else if in_loaf {
                Rgba([190, 120, 45, 255])  // golden loaf
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 162: Raw Beef (Deep red marbling steak)
        atlas.paint_tile(162, |x, y| {
            let in_meat = (x >= 3 && x <= 12 && y >= 4 && y <= 12)
                && !(x == 3 && y == 4) && !(x == 12 && y == 12);
            let is_fat = in_meat && ((x + y == 10) || (x == 11 && y >= 5 && y <= 9));
            if is_fat {
                Rgba([245, 235, 225, 255])
            } else if in_meat {
                let n = simple_noise(x, y, 162);
                let r = (185.0 + n * 20.0) as u8;
                Rgba([r, 45, 45, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 163: Cooked Steak (Hearty brown steak with grill marks)
        atlas.paint_tile(163, |x, y| {
            let in_meat = (x >= 3 && x <= 12 && y >= 4 && y <= 12)
                && !(x == 3 && y == 4) && !(x == 12 && y == 12);
            let is_grill = in_meat && (x + y == 10 || x + y == 14 || x + y == 18);
            if is_grill {
                Rgba([50, 30, 20, 255])
            } else if in_meat {
                let n = simple_noise(x, y, 163);
                let r = (115.0 + n * 15.0) as u8;
                let g = (65.0 + n * 10.0) as u8;
                let b = (40.0 + n * 8.0) as u8;
                Rgba([r, g, b, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 164: Raw Porkchop (Pink cutlet with white fat rim)
        atlas.paint_tile(164, |x, y| {
            let in_meat = (x >= 4 && x <= 12 && y >= 5 && y <= 11)
                && !(x == 4 && y == 5);
            let is_fat_rim = in_meat && (x == 12 || y == 5);
            if is_fat_rim {
                Rgba([250, 230, 230, 255])
            } else if in_meat {
                let n = simple_noise(x, y, 164);
                let r = (235.0 + n * 15.0) as u8;
                let g = (145.0 + n * 15.0) as u8;
                let b = (155.0 + n * 15.0) as u8;
                Rgba([r, g, b, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 165: Cooked Porkchop (Warm golden-brown porkchop)
        atlas.paint_tile(165, |x, y| {
            let in_meat = (x >= 4 && x <= 12 && y >= 5 && y <= 11)
                && !(x == 4 && y == 5);
            if in_meat {
                let n = simple_noise(x, y, 165);
                let r = (195.0 + n * 20.0) as u8;
                let g = (130.0 + n * 15.0) as u8;
                let b = (70.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 166: Raw Chicken (Pale poultry drumstick)
        atlas.paint_tile(166, |x, y| {
            let is_bone = (x == 4 && y == 12) || (x == 5 && y == 11) || (x == 3 && y == 13);
            let is_drum = (x >= 6 && x <= 12 && y >= 4 && y <= 10)
                || (x >= 8 && x <= 11 && y >= 3 && y <= 5);
            if is_bone {
                Rgba([240, 235, 230, 255])
            } else if is_drum {
                Rgba([230, 185, 175, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 167: Cooked Chicken (Crisp roasted drumstick)
        atlas.paint_tile(167, |x, y| {
            let is_bone = (x == 4 && y == 12) || (x == 5 && y == 11) || (x == 3 && y == 13);
            let is_drum = (x >= 6 && x <= 12 && y >= 4 && y <= 10)
                || (x >= 8 && x <= 11 && y >= 3 && y <= 5);
            if is_bone {
                Rgba([240, 235, 230, 255])
            } else if is_drum {
                let n = simple_noise(x, y, 167);
                let r = (180.0 + n * 20.0) as u8;
                let g = (95.0 + n * 15.0) as u8;
                Rgba([r, g, 35, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 168: Leather (Hide shape)
        atlas.paint_tile(168, |x, y| {
            let in_hide = (x >= 4 && x <= 11 && y >= 4 && y <= 11)
                || (x >= 2 && x <= 13 && (y == 4 || y == 11));
            if in_hide {
                let n = simple_noise(x, y, 168);
                let r = (145.0 + n * 18.0) as u8;
                let g = (85.0 + n * 12.0) as u8;
                let b = (45.0 + n * 8.0) as u8;
                Rgba([r, g, b, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 169: Feather (White plumage with quill stem)
        atlas.paint_tile(169, |x, y| {
            let is_stem = x + y == 15 && x >= 3 && x <= 12;
            let is_vane = (x + y >= 13 && x + y <= 17) && x >= 5 && x <= 13 && y <= 10;
            if is_stem {
                Rgba([200, 195, 185, 255])
            } else if is_vane {
                let n = simple_noise(x, y, 169);
                let val = (235.0 + n * 20.0) as u8;
                Rgba([val, val, val, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        // 170: Cow Skin (Black and white spotted hide)
        atlas.paint_tile(170, |x, y| {
            let is_black_spot = (x >= 3 && x <= 8 && y >= 2 && y <= 6)
                || (x >= 9 && x <= 14 && y >= 8 && y <= 13)
                || (x <= 4 && y >= 10);
            if is_black_spot {
                let n = simple_noise(x, y, 170);
                let val = (35.0 + n * 15.0) as u8;
                Rgba([val, val, val, 255])
            } else {
                let n = simple_noise(x, y, 171);
                let val = (230.0 + n * 20.0) as u8;
                Rgba([val, val, val, 255])
            }
        });

        // 171: Pig Skin (Soft warm pink)
        atlas.paint_tile(171, |x, y| {
            let n = simple_noise(x, y, 171);
            let r = (240.0 + n * 15.0) as u8;
            let g = (165.0 + n * 15.0) as u8;
            let b = (175.0 + n * 15.0) as u8;
            Rgba([r, g, b, 255])
        });

        // 172: Chicken Skin (White plumage with red wattle / yellow beak accents)
        atlas.paint_tile(172, |x, y| {
            let is_wattle = (x == 7 || x == 8) && y >= 11 && y <= 13;
            let is_beak = (x >= 6 && x <= 9) && y >= 8 && y <= 10;
            if is_wattle {
                Rgba([220, 40, 40, 255])
            } else if is_beak {
                Rgba([245, 185, 30, 255])
            } else {
                let n = simple_noise(x, y, 172);
                let val = (235.0 + n * 18.0) as u8;
                Rgba([val, val, val, 255])
            }
        });

        // 173: Chest Top (Dark oak lid with iron corner studs)
        atlas.paint_tile(173, |x, y| {
            let is_border = x == 0 || x == 15 || y == 0 || y == 15;
            let is_corner_stud = (x <= 1 && y <= 1) || (x >= 14 && y <= 1) || (x <= 1 && y >= 14) || (x >= 14 && y >= 14);
            if is_corner_stud {
                Rgba([45, 45, 50, 255]) // Iron corner stud
            } else if is_border {
                Rgba([65, 45, 25, 255]) // Dark oak frame
            } else {
                let n = simple_noise(x, y, 173);
                let r = (160.0 + n * 18.0) as u8;
                let g = (115.0 + n * 14.0) as u8;
                let b = (65.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 174: Chest Front (Oak planks, horizontal lid seam, golden latch)
        atlas.paint_tile(174, |x, y| {
            let is_border = x == 0 || x == 15 || y == 0 || y == 15;
            let is_lid_seam = y == 5;
            let is_latch = (x == 7 || x == 8) && (y >= 4 && y <= 7);
            let is_latch_keyhole = (x == 7 || x == 8) && y == 6;

            if is_latch_keyhole {
                Rgba([40, 30, 10, 255]) // Keyhole
            } else if is_latch {
                Rgba([235, 195, 55, 255]) // Golden lock latch
            } else if is_lid_seam {
                Rgba([45, 30, 15, 255]) // Dark lid opening seam
            } else if is_border {
                Rgba([65, 45, 25, 255]) // Dark oak frame
            } else {
                let n = simple_noise(x, y, 174);
                let r = (160.0 + n * 18.0) as u8;
                let g = (115.0 + n * 14.0) as u8;
                let b = (65.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 175: Chest Side & Back (Oak planks with lid seam and dark frame)
        atlas.paint_tile(175, |x, y| {
            let is_border = x == 0 || x == 15 || y == 0 || y == 15;
            let is_lid_seam = y == 5;
            if is_lid_seam {
                Rgba([45, 30, 15, 255])
            } else if is_border {
                Rgba([65, 45, 25, 255])
            } else {
                let n = simple_noise(x, y, 175);
                let r = (160.0 + n * 18.0) as u8;
                let g = (115.0 + n * 14.0) as u8;
                let b = (65.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 176: Chest Bottom (Dark sturdy oak planks)
        atlas.paint_tile(176, |x, y| {
            let is_border = x == 0 || x == 15 || y == 0 || y == 15;
            if is_border {
                Rgba([55, 38, 20, 255])
            } else {
                let n = simple_noise(x, y, 176);
                let r = (140.0 + n * 15.0) as u8;
                let g = (95.0 + n * 12.0) as u8;
                let b = (55.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            }
        });

        // 177: Chest Item Icon (Isometric 3D chest with golden latch)
        atlas.paint_tile(177, |x, y| {
            let in_chest = x >= 2 && x <= 13 && y >= 3 && y <= 13;
            let is_border = (x == 2 || x == 13 || y == 3 || y == 13) && in_chest;
            let is_lid_seam = y == 7 && in_chest;
            let is_latch = (x == 7 || x == 8) && (y >= 6 && y <= 9);
            let is_latch_keyhole = (x == 7 || x == 8) && y == 8;

            if is_latch_keyhole {
                Rgba([40, 30, 10, 255])
            } else if is_latch {
                Rgba([240, 205, 60, 255])
            } else if is_lid_seam {
                Rgba([45, 30, 15, 255])
            } else if is_border {
                Rgba([65, 45, 25, 255])
            } else if in_chest {
                let n = simple_noise(x, y, 177);
                let r = (165.0 + n * 18.0) as u8;
                let g = (118.0 + n * 14.0) as u8;
                let b = (68.0 + n * 10.0) as u8;
                Rgba([r, g, b, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

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
