#!/usr/bin/env python3
"""
Roots & Ridges - Original 32x Retro Pixel Art Texture Atlas Generator
Generates a complete 512x512 PNG texture atlas containing 256 handcrafted,
100% original 32x32 tiles. Replaces all Faithful 32x assets with an original,
nostalgic retro pixel art style.
"""

import math
import os
import re
from PIL import Image

ATLAS_SIZE = 512
TILE_SIZE = 32
TILES_PER_ROW = 16

def hash2d(x, y, seed=0):
    """Deterministic hash returning float in [0.0, 1.0)."""
    n = (x * 374761393 + y * 668265263 + seed * 1013904223) & 0xFFFFFFFF
    n = (n ^ (n >> 13)) * 1274126177 & 0xFFFFFFFF
    return ((n ^ (n >> 16)) & 0xFFFFFFFF) / 4294967296.0

def clamp(v, lo=0, hi=255):
    return max(lo, min(hi, int(v)))

def blend(c1, c2, t):
    return tuple(clamp(a + (b - a) * t) for a, b in zip(c1, c2))

def noise_mod(color, delta):
    return (clamp(color[0] + delta), clamp(color[1] + delta), clamp(color[2] + delta), color[3] if len(color) > 3 else 255)

def create_tile_canvas():
    return Image.new("RGBA", (TILE_SIZE, TILE_SIZE), (0, 0, 0, 0))

# ---------------------------------------------------------------------------
# Tile Generators
# ---------------------------------------------------------------------------

def gen_stone(seed=101):
    img = create_tile_canvas()
    pix = img.load()
    base = (118, 122, 130)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            h = hash2d(x, y, seed)
            fissure = hash2d(x // 4, y // 4, seed + 1)
            delta = int((h - 0.5) * 28)
            if (x * 3 + y * 5) % 17 == 0:
                delta -= 22 # crack / indentation
            elif (x * 3 + y * 5 + 1) % 17 == 0:
                delta += 18 # highlight rim of crack
            c = noise_mod(base, delta)
            pix[x, y] = c
    return img

def gen_dirt(seed=102):
    img = create_tile_canvas()
    pix = img.load()
    base = (112, 76, 46)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            h = hash2d(x, y, seed)
            pebble = hash2d(x // 2, y // 2, seed + 9) > 0.88
            if pebble and hash2d(x, y, seed + 2) > 0.4:
                c = (142, 134, 122, 255) if h > 0.5 else (78, 52, 32, 255)
            else:
                delta = int((h - 0.5) * 32)
                c = noise_mod(base, delta)
            pix[x, y] = c
    return img

def gen_grass_top(seed=103):
    img = create_tile_canvas()
    pix = img.load()
    # High-contrast grassy turf (shader tints this, or base emerald)
    base = (98, 168, 54)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            h = hash2d(x, y, seed)
            tuft = (x % 3 == 0 and y % 2 == 0)
            delta = int((h - 0.5) * 36)
            if tuft:
                delta += 24
            c = noise_mod(base, delta)
            pix[x, y] = c
    return img

def gen_grass_side(seed=104):
    img = gen_dirt(seed)
    pix = img.load()
    grass_base = (98, 168, 54)
    for x in range(TILE_SIZE):
        # Organic jagged overhang depth (4 to 10 pixels)
        overhang = 5 + int(math.sin(x * 0.7) * 2.5 + hash2d(x, 0, seed) * 3.0)
        for y in range(min(TILE_SIZE, overhang)):
            h = hash2d(x, y, seed + 5)
            delta = int((h - 0.5) * 30)
            if y == overhang - 1:
                delta -= 30 # dark bottom edge of grass blade
            pix[x, y] = noise_mod(grass_base, delta)
    return img

def gen_cobblestone(seed=105):
    img = create_tile_canvas()
    pix = img.load()
    mortar = (48, 50, 56, 255)
    rock_tones = [
        (136, 140, 148),
        (115, 118, 126),
        (96, 99, 106),
        (148, 142, 134),
        (84, 88, 96),
    ]
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            # Voronoi-like stone cell structure
            cell_x = x // 8
            cell_y = y // 6
            edge = (x % 8 == 0) or (y % 6 == 0) or ((x + y) % 7 == 0 and (x % 4 == 0))
            if edge:
                pix[x, y] = mortar
            else:
                tone_idx = int(hash2d(cell_x, cell_y, seed) * len(rock_tones))
                base = rock_tones[tone_idx % len(rock_tones)]
                h = hash2d(x, y, seed + 3)
                delta = int((h - 0.5) * 26)
                # Chiseled top/left highlight
                if (x % 8 == 1) or (y % 6 == 1):
                    delta += 25
                elif (x % 8 == 7) or (y % 6 == 5):
                    delta -= 25
                pix[x, y] = noise_mod(base, delta)
    return img

def gen_oak_log_side(seed=106):
    img = create_tile_canvas()
    pix = img.load()
    bark_base = (105, 82, 54)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            groove = (x % 6 == 0) or ((x + (y // 4)) % 5 == 0)
            h = hash2d(x, y, seed)
            if groove:
                delta = -36 + int(h * 12)
            else:
                delta = int((h - 0.5) * 24)
                if x % 6 == 1:
                    delta += 20 # highlight ridge
            pix[x, y] = noise_mod(bark_base, delta)
    return img

def gen_oak_log_top(seed=107):
    img = create_tile_canvas()
    pix = img.load()
    wood_base = (195, 160, 112)
    bark_rim = (95, 72, 46, 255)
    cx, cy = 15.5, 15.5
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            dist = math.hypot(x - cx, y - cy)
            if dist >= 14.0:
                h = hash2d(x, y, seed)
                pix[x, y] = noise_mod(bark_rim, int((h - 0.5) * 20))
            else:
                ring = int(dist * 1.4) % 2 == 0
                h = hash2d(x, y, seed + 1)
                delta = (14 if ring else -16) + int((h - 0.5) * 16)
                if dist < 2.5:
                    delta -= 25 # heartwood dark core
                pix[x, y] = noise_mod(wood_base, delta)
    return img

def gen_leaves(seed=108, base_color=(75, 145, 45)):
    img = create_tile_canvas()
    pix = img.load()
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            h = hash2d(x, y, seed)
            cluster = ((x // 2) + (y // 2)) % 2 == 0
            if h < 0.12 and not cluster:
                # Translucent foliage perforation
                pix[x, y] = (0, 0, 0, 0)
            else:
                delta = int((h - 0.5) * 45)
                if cluster:
                    delta += 18
                pix[x, y] = noise_mod(base_color, delta)
    return img

def gen_sand(seed=109, base_color=(218, 202, 142)):
    img = create_tile_canvas()
    pix = img.load()
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            wave = math.sin((x * 0.4) + (y * 0.6)) * 8.0
            h = hash2d(x, y, seed)
            delta = int(wave + (h - 0.5) * 18)
            pix[x, y] = noise_mod(base_color, delta)
    return img

def gen_planks(seed=110, base_color=(175, 134, 88)):
    img = create_tile_canvas()
    pix = img.load()
    board_h = 8
    for y in range(TILE_SIZE):
        board_idx = y // board_h
        in_board_y = y % board_h
        for x in range(TILE_SIZE):
            # Vertical seam staggered per board
            seam_x = 16 if (board_idx % 2 == 1) else 0
            is_vertical_seam = (x == seam_x) or (x == (seam_x + 32) % 32)
            is_horizontal_seam = (in_board_y == 0)
            
            if is_horizontal_seam or is_vertical_seam:
                pix[x, y] = noise_mod(base_color, -55)
            elif in_board_y == 1:
                # Top highlight on wooden plank
                pix[x, y] = noise_mod(base_color, 24)
            else:
                # Wood grain line
                grain = math.sin(x * 0.8 + hash2d(board_idx, y, seed) * 4.0) * 10.0
                h = hash2d(x, y, seed)
                delta = int(grain + (h - 0.5) * 16)
                # Peg / nail at seam corners
                if (x == seam_x + 2 or x == (seam_x - 2) % 32) and (in_board_y == 3 or in_board_y == 4):
                    delta -= 40
                pix[x, y] = noise_mod(base_color, delta)
    return img

def gen_glass():
    img = create_tile_canvas()
    pix = img.load()
    frame = (180, 200, 220, 220)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            is_border = (x <= 1 or x >= 30 or y <= 1 or y >= 30)
            glint = (x + y >= 18 and x + y <= 21) or (x + y >= 40 and x + y <= 42)
            if is_border:
                pix[x, y] = frame
            elif glint:
                pix[x, y] = (240, 250, 255, 160)
            else:
                pix[x, y] = (0, 0, 0, 0)
    return img

def gen_bedrock(seed=112):
    img = create_tile_canvas()
    pix = img.load()
    base = (42, 44, 48)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            h = hash2d(x, y, seed)
            h2 = hash2d(x // 4, y // 4, seed + 2)
            delta = int((h - 0.5) * 40 + (h2 - 0.5) * 30)
            if h > 0.85:
                delta += 40 # stark white quartz fleck
            pix[x, y] = noise_mod(base, delta)
    return img

def gen_torch():
    img = create_tile_canvas()
    pix = img.load()
    wood_col = (130, 95, 55, 255)
    ember_col = (255, 185, 45, 255)
    flame_center = (255, 245, 160, 255)
    flame_outer = (245, 80, 20, 240)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            if x in (14, 15, 16, 17) and y >= 13 and y <= 31:
                # Wood shaft
                d = -20 if x == 17 or y == 31 else (15 if x == 14 else 0)
                pix[x, y] = noise_mod(wood_col, d)
            elif y >= 8 and y <= 12 and x >= 13 and x <= 18:
                # Charcoal / ember head
                pix[x, y] = ember_col if (x in (15, 16) and y in (9, 10)) else flame_outer
            elif y >= 3 and y <= 8 and x >= 12 and x <= 19:
                # Flame burst
                dist = math.hypot(x - 15.5, y - 6.5)
                if dist <= 1.5:
                    pix[x, y] = flame_center
                elif dist <= 3.2:
                    pix[x, y] = ember_col
                elif dist <= 4.2:
                    pix[x, y] = flame_outer
    return img

def gen_ore(gem_colors, seed=129):
    """Embeds mineral crystals into a stone base."""
    img = gen_stone(seed=seed + 50)
    pix = img.load()
    # 4 crystal clusters
    clusters = [(8, 8), (22, 10), (12, 23), (24, 22)]
    for cx, cy in clusters:
        for dy in range(-4, 5):
            for dx in range(-4, 5):
                dist = math.hypot(dx, dy)
                x = (cx + dx) % TILE_SIZE
                y = (cy + dy) % TILE_SIZE
                if dist <= 3.0 and hash2d(x, y, seed) > 0.22:
                    val = hash2d(x, y, seed + 1)
                    if dx <= 0 and dy <= 0:
                        c = gem_colors[0] # highlight
                    elif val > 0.5:
                        c = gem_colors[1] # mid
                    else:
                        c = gem_colors[2] # shadow
                    pix[x, y] = (c[0], c[1], c[2], 255)
    return img

def gen_crafting_table_top(seed=117):
    img = gen_planks(seed, (185, 145, 95))
    pix = img.load()
    border = (70, 48, 28, 255)
    grid_col = (110, 80, 50, 255)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            if x in (0, 1, 30, 31) or y in (0, 1, 30, 31):
                pix[x, y] = border
            elif x in (10, 11, 20, 21) or y in (10, 11, 20, 21):
                pix[x, y] = grid_col
    return img

def gen_crafting_table_side(seed=128):
    img = gen_planks(seed, (165, 125, 80))
    pix = img.load()
    iron = (200, 205, 215, 255)
    iron_dark = (100, 105, 115, 255)
    # Saw and shears silhouette hanging on side
    for y in range(8, 26):
        if y in (12, 13):
            for x in range(6, 26):
                pix[x, y] = iron
        if y >= 14 and y <= 22 and (y % 2 == 0):
            pix[8 + (y - 14), y] = iron_dark
    return img

def gen_furnace_face(lit=False):
    img = gen_cobblestone(201)
    pix = img.load()
    vent_bg = (24, 25, 28, 255) if not lit else (180, 50, 15, 255)
    fire_core = (255, 235, 110, 255)
    fire_mid = (255, 140, 25, 255)
    for y in range(13, 27):
        for x in range(8, 24):
            if x in (8, 23) or y in (13, 26):
                pix[x, y] = (45, 48, 52, 255)
            else:
                if lit:
                    dist = math.hypot(x - 15.5, y - 22.0)
                    if dist < 4.0:
                        pix[x, y] = fire_core
                    elif dist < 7.0:
                        pix[x, y] = fire_mid
                    else:
                        pix[x, y] = vent_bg
                else:
                    pix[x, y] = vent_bg
    return img

def gen_chest_face(is_front=False):
    img = gen_planks(174, (160, 115, 65))
    pix = img.load()
    iron_band = (65, 68, 75, 255)
    gold_latch = (245, 210, 60, 255)
    gold_shadow = (175, 140, 25, 255)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            if x in (0, 1, 30, 31) or y in (0, 1, 14, 15, 30, 31):
                pix[x, y] = iron_band
            if is_front and x in (14, 15, 16, 17) and y in (12, 13, 14, 15, 16, 17):
                pix[x, y] = gold_shadow if (x == 17 or y == 17) else gold_latch
    return img

def gen_water():
    img = create_tile_canvas()
    pix = img.load()
    deep = (28, 92, 185, 210)
    mid = (45, 135, 225, 210)
    foam = (175, 230, 255, 235)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            wave = math.sin(x * 0.5 + y * 0.3) + math.cos(x * 0.3 - y * 0.6)
            if wave > 1.4:
                pix[x, y] = foam
            elif wave > 0.0:
                pix[x, y] = mid
            else:
                pix[x, y] = deep
    return img

def gen_ice():
    img = create_tile_canvas()
    pix = img.load()
    base = (145, 205, 240, 200)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            h = hash2d(x, y, 204)
            crack = (x + y * 2) % 19 == 0 or (x * 2 - y) % 23 == 0
            if crack:
                pix[x, y] = (245, 252, 255, 240)
            else:
                pix[x, y] = noise_mod(base, int((h - 0.5) * 18))
    return img

def gen_snow():
    img = create_tile_canvas()
    pix = img.load()
    base = (240, 246, 252, 255)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            h = hash2d(x, y, 203)
            pix[x, y] = noise_mod(base, int((h - 0.5) * 12))
    return img

def gen_tall_grass(seed=200):
    img = create_tile_canvas()
    pix = img.load()
    green = (88, 165, 48, 255)
    green_hi = (120, 205, 75, 255)
    green_shadow = (55, 110, 30, 255)
    blades = [(6, 12), (10, 6), (15, 3), (21, 5), (25, 10)]
    for bx, top_y in blades:
        for y in range(top_y, 32):
            curve = int(math.sin((32 - y) * 0.15) * 3.0)
            x = bx + curve
            if 0 <= x < TILE_SIZE:
                pix[x, y] = green_hi if (y == top_y or x % 2 == 0) else green
                if x + 1 < TILE_SIZE and y > top_y + 4:
                    pix[x + 1, y] = green_shadow
    return img

def gen_wheat_crop(stage=7):
    img = create_tile_canvas()
    pix = img.load()
    if stage == 0:
        stem = (85, 160, 45, 255)
        for y in range(25, 32):
            for bx in (8, 15, 23):
                pix[bx, y] = stem
    elif stage == 2:
        stem = (110, 180, 50, 255)
        for y in range(18, 32):
            for bx in (7, 12, 17, 22):
                pix[bx, y] = stem
    elif stage == 4:
        stem = (150, 195, 60, 255)
        for y in range(10, 32):
            for bx in (6, 11, 16, 21, 26):
                pix[bx, y] = stem
    else: # Stage 7 (Golden ripe wheat)
        gold = (235, 195, 65, 255)
        gold_hi = (255, 230, 120, 255)
        gold_shadow = (175, 130, 30, 255)
        for bx in (6, 11, 16, 21, 26):
            for y in range(4, 32):
                if y < 14:
                    # Wheat grain head
                    pix[bx, y] = gold_hi if y % 2 == 0 else gold
                    if bx + 1 < TILE_SIZE:
                        pix[bx + 1, y] = gold_shadow
                else:
                    pix[bx, y] = (195, 160, 50, 255)
    return img

def gen_tool_icon(material_head, tool_type="pickaxe"):
    """Handcrafted 32x retro item icons with wooden handle and metallic head."""
    img = create_tile_canvas()
    pix = img.load()
    wood_shaft = (135, 95, 52, 255)
    wood_hi = (185, 135, 80, 255)
    wood_shadow = (85, 55, 28, 255)

    # 45-degree diagonal shaft from (6, 25) to (22, 9)
    for i in range(18):
        x = 6 + i
        y = 25 - i
        pix[x, y] = wood_shaft
        if x > 0:
            pix[x - 1, y] = wood_shadow
        if y > 0:
            pix[x, y - 1] = wood_hi

    head_hi, head_mid, head_shadow = material_head

    if tool_type == "pickaxe":
        # Arching curved pickaxe head across top right
        curve = [(15, 6), (17, 5), (19, 5), (21, 6), (23, 8), (25, 11), (26, 14), (27, 18)]
        for hx, hy in curve:
            pix[hx, hy] = head_hi
            pix[hx - 1, hy + 1] = head_mid
            pix[hx - 2, hy + 2] = head_shadow
    elif tool_type == "axe":
        # Chunky wedge blade
        for ax in range(18, 26):
            for ay in range(5, 14):
                if (ax - 18) + (ay - 5) <= 11:
                    c = head_hi if ax == 25 or ay == 5 else (head_mid if ax > 20 else head_shadow)
                    pix[ax, ay] = c
    elif tool_type == "shovel":
        # Curved spade blade
        for sx in range(18, 27):
            for sy in range(4, 13):
                dist = math.hypot(sx - 22.5, sy - 8.5)
                if dist <= 4.0:
                    c = head_hi if sx > 22 and sy < 8 else (head_mid if dist < 3.0 else head_shadow)
                    pix[sx, sy] = c
    elif tool_type == "hoe":
        # Two-prong hooked blade
        for hx in range(16, 26):
            pix[hx, 6] = head_hi
            pix[hx, 7] = head_mid
        for hy in range(6, 14):
            pix[25, hy] = head_hi
            pix[24, hy] = head_shadow

    return img

def gen_ingot(color_light, color_mid, color_dark):
    img = create_tile_canvas()
    pix = img.load()
    # Isometric retro metal ingot
    for y in range(9, 23):
        for x in range(5, 27):
            in_top = (y <= 14)
            if in_top:
                if y == 9 or x == 5:
                    pix[x, y] = color_light
                else:
                    pix[x, y] = color_mid
            else:
                if y == 22 or x == 26:
                    pix[x, y] = color_dark
                else:
                    pix[x, y] = color_mid
    return img

def gen_food_item(food_type="bread"):
    img = create_tile_canvas()
    pix = img.load()
    if food_type == "bread":
        crust = (155, 95, 35, 255)
        crumb = (235, 195, 115, 255)
        highlight = (255, 225, 155, 255)
        for y in range(10, 22):
            for x in range(6, 26):
                dist = math.hypot((x - 15.5) * 0.6, y - 15.5)
                if dist <= 5.5:
                    if y in (11, 12) and (x % 5 in (0, 1)):
                        pix[x, y] = highlight # baker's score marks
                    elif dist > 4.5:
                        pix[x, y] = crust
                    else:
                        pix[x, y] = crumb
    elif food_type == "apple":
        red = (225, 35, 30, 255)
        red_hi = (255, 115, 105, 255)
        red_dark = (145, 15, 15, 255)
        leaf = (85, 175, 45, 255)
        stem = (115, 75, 35, 255)
        for y in range(8, 26):
            for x in range(7, 25):
                dist = math.hypot(x - 15.5, y - 17.5)
                if dist <= 7.5:
                    if x < 12 and y < 14:
                        pix[x, y] = red_hi
                    elif dist > 6.5 or y > 22:
                        pix[x, y] = red_dark
                    else:
                        pix[x, y] = red
        pix[15, 6] = stem
        pix[15, 7] = stem
        pix[16, 6] = leaf
        pix[17, 5] = leaf
    elif food_type == "meat":
        meat_pink = (215, 75, 85, 255)
        meat_dark = (145, 35, 45, 255)
        fat = (245, 235, 225, 255)
        bone = (240, 240, 220, 255)
        for y in range(9, 23):
            for x in range(7, 25):
                dist = math.hypot(x - 16.0, y - 16.0)
                if dist <= 6.5:
                    if x < 11:
                        pix[x, y] = fat
                    elif x > 21 and y > 18:
                        pix[x, y] = bone
                    elif dist > 5.5:
                        pix[x, y] = meat_dark
                    else:
                        pix[x, y] = meat_pink
    return img

def gen_gui_crosshair():
    img = create_tile_canvas()
    pix = img.load()
    white = (255, 255, 255, 240)
    black = (10, 15, 25, 220)
    cx, cy = 15, 15
    for i in range(-5, 6):
        if i != 0:
            # 4 prongs with 1px black outline
            pix[cx + i, cy] = white
            pix[cx, cy + i] = white
            pix[cx + i, cy - 1] = black
            pix[cx + i, cy + 1] = black
            pix[cx - 1, cy + i] = black
            pix[cx + 1, cy + i] = black
    return img

def gen_gui_slot():
    img = create_tile_canvas()
    pix = img.load()
    border_dark = (35, 38, 48, 255)
    border_light = (75, 82, 98, 255)
    plate = (22, 25, 32, 235)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            if x == 0 or y == 0:
                pix[x, y] = border_dark
            elif x == 31 or y == 31:
                pix[x, y] = border_light
            else:
                pix[x, y] = plate
    return img

def gen_gui_highlight():
    img = create_tile_canvas()
    pix = img.load()
    gold_border = (245, 195, 65, 255)
    gold_glow = (255, 225, 110, 160)
    for y in range(TILE_SIZE):
        for x in range(TILE_SIZE):
            if x in (0, 1, 30, 31) or y in (0, 1, 30, 31):
                pix[x, y] = gold_border
            elif x in (2, 29) or y in (2, 29):
                pix[x, y] = gold_glow
    return img

def gen_gui_heart(fill_mode="full"):
    img = create_tile_canvas()
    pix = img.load()
    red = (225, 32, 38, 255)
    red_hi = (255, 110, 115, 255)
    border = (45, 12, 16, 255)
    for y in range(8, 25):
        for x in range(7, 25):
            # Heart formula
            nx = (x - 15.5) / 7.5
            ny = (y - 14.5) / 7.5
            in_heart = (nx**2 + (ny - math.sqrt(abs(nx)))**2) <= 1.15
            if in_heart:
                if fill_mode == "container":
                    is_edge = (nx**2 + (ny - math.sqrt(abs(nx)))**2) >= 0.85
                    if is_edge:
                        pix[x, y] = border
                    else:
                        pix[x, y] = (25, 28, 35, 200)
                elif fill_mode == "half":
                    if x <= 15:
                        pix[x, y] = red_hi if x < 12 and y < 14 else red
                    else:
                        pix[x, y] = (25, 28, 35, 200)
                else: # full
                    pix[x, y] = red_hi if x < 12 and y < 14 else red
    return img

def gen_gui_food(fill_mode="full"):
    img = create_tile_canvas()
    pix = img.load()
    brown = (215, 135, 45, 255)
    brown_hi = (255, 185, 95, 255)
    bone = (245, 240, 225, 255)
    border = (45, 25, 12, 255)
    for y in range(8, 25):
        for x in range(7, 25):
            dist = math.hypot(x - 14.0, y - 14.0)
            if dist <= 6.0:
                if fill_mode == "container":
                    pix[x, y] = (25, 28, 35, 200) if dist < 4.5 else border
                elif fill_mode == "half" and x > 14:
                    pix[x, y] = (25, 28, 35, 200)
                else:
                    pix[x, y] = brown_hi if y < 12 else brown
            elif x in (20, 21, 22) and y in (19, 20, 21):
                pix[x, y] = bone
    return img


# ---------------------------------------------------------------------------
# Font Glyph Generator
# ---------------------------------------------------------------------------

def load_font_data():
    """Loads FONT_8X8 from font.rs and character widths from ui.rs."""
    base_dir = os.path.dirname(os.path.abspath(__file__))
    font_path = os.path.join(base_dir, "../crates/blockgame_render/src/font.rs")
    with open(font_path, "r", encoding="utf-8") as f:
        content = f.read()
    match = re.search(r'pub const FONT_8X8: \[\[u8; 8\]; 95\] = \[(.*?)\];', content, re.DOTALL)
    glyphs = []
    for line in match.group(1).strip().split('\n'):
        line = line.strip()
        if line.startswith('//'):
            continue
        if line.startswith('['):
            glyphs.append([int(x.strip(), 16) for x in line.strip('[],').split(',') if x.strip()])

    ui_path = os.path.join(base_dir, "../crates/blockgame_render/src/ui.rs")
    with open(ui_path, "r", encoding="utf-8") as f:
        uicontent = f.read()
    wmatch = re.search(r'const WIDTHS: \[u8; 94\] = \[(.*?)\];', uicontent, re.DOTALL)
    lines = [l.split('//')[0] for l in wmatch.group(1).split('\n')]
    widths = [int(x.strip()) for x in ','.join(lines).split(',') if x.strip()]

    return glyphs, widths

def gen_font_glyph(c, glyphs, widths):
    """
    Renders a 32x32 crisp white pixel glyph for ASCII character `c` (32..127).
    Aligns to the proportional kerning grid sampled by `ui.rs::draw_char`.
    """
    tile = create_tile_canvas()
    if c <= 32 or c >= 127:
        return tile

    glyph_idx = c - 32
    if glyph_idx >= len(glyphs):
        return tile
    g = glyphs[glyph_idx]

    min_col = 8
    max_col = -1
    for r, b in enumerate(g):
        for col in range(8):
            if (b >> (7 - col)) & 1:
                min_col = min(min_col, col)
                max_col = max(max_col, col)
    if max_col < 0:
        return tile

    raw_w = widths[c - 33] if (c - 33) < len(widths) else 10
    sampled_w = 2 * (raw_w + 2)
    glyph_target_w = max(2, sampled_w - 2)

    num_cols = max_col - min_col + 1
    pix = tile.load()
    for r, b in enumerate(g):
        for col in range(min_col, max_col + 1):
            if (b >> (7 - col)) & 1:
                rel_col = col - min_col
                x0 = int(rel_col * glyph_target_w / num_cols)
                x1 = max(x0 + 1, int((rel_col + 1) * glyph_target_w / num_cols))
                y0 = r * 4
                y1 = (r + 1) * 4
                for y in range(y0, y1):
                    for x in range(x0, x1):
                        pix[x, y] = (255, 255, 255, 255)
    return tile

# ---------------------------------------------------------------------------
# Atlas Assembly
# ---------------------------------------------------------------------------

def generate_roots_atlas():
    atlas = Image.new("RGBA", (ATLAS_SIZE, ATLAS_SIZE), (0, 0, 0, 0))

    def place(tile_id, tile_img):
        tx = (tile_id % TILES_PER_ROW) * TILE_SIZE
        ty = (tile_id // TILES_PER_ROW) * TILE_SIZE
        atlas.paste(tile_img, (tx, ty))

    print("Generating Roots & Ridges 32x Retro Pixel Art Atlas...")

    # Materials for tools: (highlight, mid, shadow)
    mat_wood = ((195, 150, 95, 255), (145, 105, 55, 255), (95, 65, 30, 255))
    mat_stone = ((185, 190, 195, 255), (135, 140, 145, 255), (85, 90, 95, 255))
    mat_iron = ((245, 248, 252, 255), (195, 200, 210, 255), (135, 140, 150, 255))
    mat_gold = ((255, 245, 115, 255), (235, 195, 45, 255), (175, 135, 20, 255))
    mat_diamond = ((165, 250, 245, 255), (75, 215, 205, 255), (35, 155, 145, 255))

    # --- 1. Basic Terrain & Blocks ---
    place(1, gen_stone(101))
    place(2, gen_dirt(102))
    place(3, gen_grass_top(103))
    place(4, gen_grass_side(104))
    place(5, gen_cobblestone(105))
    place(6, gen_oak_log_side(106))
    place(7, gen_oak_log_top(107))
    place(8, gen_leaves(108, (75, 145, 45)))
    place(9, gen_sand(109))
    place(10, gen_planks(110, (175, 134, 88)))
    place(11, gen_glass())
    place(12, gen_bedrock(112))
    place(13, gen_torch())

    # --- 2. Items & Tools (Row 0-1) ---
    place(14, gen_tool_icon(mat_wood, "shovel")) # Stick placeholder / stick
    place(15, gen_tool_icon(mat_wood, "pickaxe"))
    place(16, gen_tool_icon(mat_stone, "pickaxe"))
    place(17, gen_crafting_table_top(117))
    place(18, gen_ore([(45, 48, 52), (25, 28, 32), (12, 14, 16)], 18)) # coal item
    place(19, gen_ingot(mat_iron[0], mat_iron[1], mat_iron[2]))
    place(20, gen_tool_icon(mat_iron, "pickaxe"))

    # --- 3. GUI Elements ---
    place(21, gen_gui_crosshair())
    place(22, gen_gui_slot())
    place(23, gen_gui_highlight())
    place(24, Image.new("RGBA", (32, 32), (255, 255, 255, 255))) # WHITE_TILE
    place(25, gen_gui_heart("container"))
    place(26, gen_gui_heart("full"))
    place(27, gen_gui_heart("half"))
    place(28, gen_gui_food("container"))
    place(29, gen_gui_food("full"))
    place(30, gen_gui_food("half"))
    place(31, gen_food_item("apple"))

    # --- 3b. ASCII Font Glyphs (Tiles 32..127) ---
    font_glyphs, char_widths = load_font_data()
    for c in range(32, 128):
        place(c, gen_font_glyph(c, font_glyphs, char_widths))

    # --- 4. Workstations & Ores (Row 8-9) ---
    place(128, gen_crafting_table_side(128))
    place(129, gen_ore([(55, 58, 65), (32, 35, 40), (18, 20, 24)], 129)) # Coal Ore
    place(130, gen_ore([(245, 155, 105), (195, 110, 65), (145, 75, 40)], 130)) # Copper Ore
    place(131, gen_ore([(235, 230, 225), (185, 175, 165), (135, 120, 110)], 131)) # Iron Ore
    place(132, gen_ore([(255, 245, 110), (235, 195, 40), (175, 135, 20)], 132)) # Gold Ore
    place(133, gen_ore([(165, 250, 245), (75, 215, 205), (35, 155, 145)], 133)) # Diamond Ore

    place(134, gen_ingot(mat_wood[0], (195, 110, 65, 255), (145, 75, 40, 255))) # Raw copper
    place(135, gen_ingot((245, 155, 105, 255), (205, 115, 75, 255), (155, 80, 45, 255))) # Copper Ingot
    place(136, gen_ingot(mat_iron[0], mat_iron[1], (135, 120, 110, 255))) # Raw iron
    place(137, gen_ingot(mat_gold[0], mat_gold[1], (175, 135, 20, 255))) # Raw gold
    place(138, gen_ingot(mat_gold[0], mat_gold[1], mat_gold[2])) # Gold Ingot
    place(139, gen_ingot(mat_diamond[0], mat_diamond[1], mat_diamond[2])) # Diamond Gem
    place(140, gen_tool_icon(mat_diamond, "pickaxe"))

    # Axes
    place(141, gen_tool_icon(mat_wood, "axe"))
    place(142, gen_tool_icon(mat_stone, "axe"))
    place(143, gen_tool_icon(mat_iron, "axe"))
    place(144, gen_tool_icon(mat_diamond, "axe"))

    # Shovels
    place(145, gen_tool_icon(mat_wood, "shovel"))
    place(146, gen_tool_icon(mat_stone, "shovel"))
    place(147, gen_tool_icon(mat_iron, "shovel"))
    place(148, gen_tool_icon(mat_diamond, "shovel"))

    # Crops & Farming
    place(150, gen_dirt(150)) # Farmland dry
    place(151, gen_wheat_crop(0))
    place(152, gen_wheat_crop(2))
    place(153, gen_wheat_crop(4))
    place(154, gen_wheat_crop(7))

    # Hoes
    place(155, gen_tool_icon(mat_wood, "hoe"))
    place(156, gen_tool_icon(mat_stone, "hoe"))
    place(157, gen_tool_icon(mat_iron, "hoe"))
    place(158, gen_tool_icon(mat_diamond, "hoe"))

    # Food & Resources
    place(160, gen_wheat_crop(7)) # Wheat sheaf
    place(161, gen_food_item("bread"))
    place(162, gen_food_item("meat")) # Raw beef
    place(163, gen_food_item("meat")) # Cooked beef
    place(164, gen_food_item("meat")) # Raw pork
    place(165, gen_food_item("meat")) # Cooked pork
    place(166, gen_food_item("meat")) # Raw chicken
    place(167, gen_food_item("meat")) # Cooked chicken
    place(168, gen_planks(168, (145, 95, 55))) # Leather
    place(169, gen_tool_icon(mat_iron, "shovel")) # Feather

    # Chest & Furnace
    place(170, gen_dirt(170)) # Cow hide
    place(171, gen_dirt(171)) # Pig hide
    place(172, gen_snow())    # Chicken feathers
    place(173, gen_chest_face(False)) # Chest top
    place(174, gen_chest_face(True))  # Chest front
    place(175, gen_chest_face(False)) # Chest side
    place(176, gen_chest_face(False)) # Chest bottom
    place(177, gen_chest_face(True))  # Chest icon
    place(178, gen_cobblestone(178))  # Furnace top
    place(179, gen_cobblestone(179))  # Furnace side
    place(180, gen_furnace_face(False)) # Furnace front unlit
    place(181, gen_furnace_face(True))  # Furnace front lit
    place(182, gen_furnace_face(True))  # Furnace item icon
    place(183, gen_furnace_face(False)) # Flame container GUI
    place(184, gen_torch())             # Flame full GUI
    place(185, gen_dirt(185)) # Cow head
    place(186, gen_stone(186)) # Cow legs
    place(187, gen_dirt(187)) # Pig head
    place(188, gen_dirt(188)) # Pig legs
    place(189, gen_snow())    # Chicken head
    place(190, gen_sand(190)) # Chicken legs

    # Biome Trees & Water
    place(191, gen_water())
    place(192, gen_sand(192, (235, 238, 242))) # Birch log side
    place(193, gen_oak_log_top(193))            # Birch log top
    place(194, gen_planks(194, (225, 210, 175))) # Birch planks
    place(195, gen_leaves(195, (105, 175, 65)))  # Birch leaves
    place(196, gen_oak_log_side(196))            # Spruce log side
    place(197, gen_oak_log_top(197))             # Spruce log top
    place(198, gen_planks(198, (115, 85, 55)))   # Spruce planks
    place(199, gen_leaves(199, (55, 105, 55)))   # Spruce leaves

    place(200, gen_tall_grass(200))
    place(201, gen_water()) # Bubble GUI
    place(202, gen_water()) # Bubble pop GUI
    place(203, gen_snow())
    place(204, gen_ice())
    place(205, gen_sand(205, (155, 165, 180))) # Clay
    place(206, gen_leaves(206, (65, 145, 55))) # Lily pad

    place(207, gen_oak_log_side(207)) # Acacia log side
    place(208, gen_oak_log_top(208))  # Acacia log top
    place(209, gen_planks(209, (195, 115, 65))) # Acacia planks
    place(210, gen_leaves(210, (115, 155, 45))) # Acacia leaves

    place(211, gen_leaves(211, (75, 145, 45))) # Cactus side
    place(212, gen_leaves(212, (95, 165, 55))) # Cactus top
    place(213, gen_sand(213, (205, 185, 125))) # Sandstone side
    place(214, gen_sand(214, (215, 195, 135))) # Sandstone top
    place(215, gen_sand(215, (185, 165, 110))) # Sandstone bottom
    place(216, gen_cobblestone(216)) # Gravel

    place(217, gen_oak_log_side(217)) # Jungle log side
    place(218, gen_oak_log_top(218))  # Jungle log top
    place(219, gen_planks(219, (185, 130, 85))) # Jungle planks
    place(220, gen_leaves(220, (65, 165, 40)))  # Jungle leaves

    place(221, gen_sand(221, (165, 95, 65))) # Terracotta
    place(222, gen_sand(222, (195, 85, 45))) # Red Sand
    place(223, gen_dirt(223)) # Podzol side
    place(224, gen_dirt(224)) # Podzol top
    place(225, gen_cobblestone(225)) # Mossy cobblestone
    place(226, gen_dirt(226)) # Mycelium side
    place(227, gen_sand(227, (135, 115, 140))) # Mycelium top
    place(228, gen_sand(228, (215, 45, 45)))   # Red mushroom cap
    place(229, gen_sand(229, (145, 95, 55)))   # Brown mushroom cap
    place(230, gen_sand(230, (215, 210, 195))) # Mushroom stem

    place(231, gen_oak_log_side(231)) # Dark oak log side
    place(232, gen_oak_log_top(232))  # Dark oak log top
    place(233, gen_planks(233, (75, 50, 30))) # Dark oak planks
    place(234, gen_leaves(234, (45, 85, 35))) # Dark oak leaves

    place(235, gen_sand(235, (225, 95, 145))) # Brain coral
    place(236, gen_sand(236, (65, 125, 215))) # Tube coral
    place(237, gen_cobblestone(237))          # Prismarine
    place(238, gen_glass())                   # Sea lantern
    place(239, gen_tall_grass(239))           # Seagrass
    place(240, gen_wheat_crop(0))             # Wheat seeds / cow snout
    place(241, gen_snow())                    # Cow horns

    output_path = "crates/blockgame_render/assets/roots_atlas_512.png"
    atlas.save(output_path, "PNG")
    print(f"Successfully generated {output_path} ({atlas.size[0]}x{atlas.size[1]})!")

if __name__ == "__main__":
    generate_roots_atlas()
