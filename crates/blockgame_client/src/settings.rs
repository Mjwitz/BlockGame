use std::path::Path;

/// Persistent client-side configuration settings.
#[derive(Debug, Clone, PartialEq)]
pub struct GameSettings {
    /// Chunk render radius around the player (`2..=12`).
    pub render_distance: i32,
    /// Vertical field of view in degrees (`50.0..=110.0`).
    pub fov_degrees: f32,
    /// Mouse look sensitivity multiplier (`0.2..=2.5`, where `1.0` = default).
    pub mouse_sensitivity: f32,
    /// Master audio volume (`0.0..=1.0`).
    pub master_volume: f32,
    /// Sound effects volume (`0.0..=1.0`).
    pub sfx_volume: f32,
    /// Background music volume (`0.0..=1.0`).
    pub music_volume: f32,
    /// Invert vertical mouse look axis.
    pub invert_y: bool,
    /// Borderless fullscreen window mode.
    pub fullscreen: bool,
    /// Vertical synchronization (VSync).
    pub vsync: bool,
    /// First-person held item / hand view bobbing while walking.
    pub view_bobbing: bool,
    /// Show top-left FPS and coordinate debug HUD overlay (also toggled via F3).
    pub show_debug_hud: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            render_distance: 4,
            fov_degrees: 60.0,
            mouse_sensitivity: 1.0,
            master_volume: 0.8,
            sfx_volume: 0.9,
            music_volume: 0.65,
            invert_y: false,
            fullscreen: false,
            vsync: true,
            view_bobbing: true,
            show_debug_hud: false,
        }
    }
}

impl GameSettings {
    pub const MIN_RENDER_DISTANCE: i32 = 2;
    pub const MAX_RENDER_DISTANCE: i32 = 12;
    pub const MIN_FOV: f32 = 50.0;
    pub const MAX_FOV: f32 = 110.0;
    pub const MIN_SENSITIVITY: f32 = 0.2;
    pub const MAX_SENSITIVITY: f32 = 2.5;

    /// Clamps all numeric settings to valid ranges.
    pub fn clamp_all(&mut self) {
        self.render_distance = self
            .render_distance
            .clamp(Self::MIN_RENDER_DISTANCE, Self::MAX_RENDER_DISTANCE);
        self.fov_degrees = self.fov_degrees.clamp(Self::MIN_FOV, Self::MAX_FOV);
        self.mouse_sensitivity = self
            .mouse_sensitivity
            .clamp(Self::MIN_SENSITIVITY, Self::MAX_SENSITIVITY);
        self.master_volume = self.master_volume.clamp(0.0, 1.0);
        self.sfx_volume = self.sfx_volume.clamp(0.0, 1.0);
        self.music_volume = self.music_volume.clamp(0.0, 1.0);
    }

    /// Loads settings from a `key=value` config file, falling back to defaults for missing keys.
    pub fn load(path: &Path) -> Self {
        let mut settings = Self::default();
        let Ok(contents) = std::fs::read_to_string(path) else {
            return settings;
        };

        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let val = val.trim();
            match key {
                "render_distance" => {
                    if let Ok(v) = val.parse::<i32>() {
                        settings.render_distance = v;
                    }
                }
                "fov_degrees" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.fov_degrees = v;
                        }
                    }
                }
                "mouse_sensitivity" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.mouse_sensitivity = v;
                        }
                    }
                }
                "master_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.master_volume = v;
                        }
                    }
                }
                "sfx_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.sfx_volume = v;
                        }
                    }
                }
                "music_volume" => {
                    if let Ok(v) = val.parse::<f32>() {
                        if v.is_finite() {
                            settings.music_volume = v;
                        }
                    }
                }
                "invert_y" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.invert_y = v;
                    }
                }
                "fullscreen" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.fullscreen = v;
                    }
                }
                "vsync" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.vsync = v;
                    }
                }
                "view_bobbing" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.view_bobbing = v;
                    }
                }
                "show_debug_hud" => {
                    if let Ok(v) = val.parse::<bool>() {
                        settings.show_debug_hud = v;
                    }
                }
                _ => {}
            }
        }

        settings.clamp_all();
        settings
    }

    /// Saves settings to a `key=value` config file.
    pub fn save(&self, path: &Path) {
        let content = format!(
            "# BlockGame Client Settings\n\
             render_distance={}\n\
             fov_degrees={:.1}\n\
             mouse_sensitivity={:.2}\n\
             master_volume={:.2}\n\
             sfx_volume={:.2}\n\
             music_volume={:.2}\n\
             invert_y={}\n\
             fullscreen={}\n\
             vsync={}\n\
             view_bobbing={}\n\
             show_debug_hud={}\n",
            self.render_distance,
            self.fov_degrees,
            self.mouse_sensitivity,
            self.master_volume,
            self.sfx_volume,
            self.music_volume,
            self.invert_y,
            self.fullscreen,
            self.vsync,
            self.view_bobbing,
            self.show_debug_hud
        );
        if let Err(e) = std::fs::write(path, content) {
            log::warn!("Failed to save settings to {:?}: {}", path, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_clamp() {
        let mut s = GameSettings {
            render_distance: 99,
            fov_degrees: 150.0,
            mouse_sensitivity: 0.01,
            master_volume: 1.5,
            sfx_volume: -0.2,
            music_volume: 0.45,
            invert_y: true,
            fullscreen: true,
            vsync: false,
            view_bobbing: false,
            show_debug_hud: true,
        };
        s.clamp_all();
        assert_eq!(s.render_distance, GameSettings::MAX_RENDER_DISTANCE);
        assert_eq!(s.fov_degrees, GameSettings::MAX_FOV);
        assert_eq!(s.mouse_sensitivity, GameSettings::MIN_SENSITIVITY);
        assert_eq!(s.master_volume, 1.0);
        assert_eq!(s.sfx_volume, 0.0);

        let temp_path = std::env::temp_dir().join("blockgame_test_settings.cfg");
        s.save(&temp_path);
        let loaded = GameSettings::load(&temp_path);
        let _ = std::fs::remove_file(&temp_path);

        assert_eq!(loaded.render_distance, s.render_distance);
        assert!((loaded.fov_degrees - s.fov_degrees).abs() < 0.11);
        assert!((loaded.mouse_sensitivity - s.mouse_sensitivity).abs() < 0.02);
        assert!((loaded.master_volume - s.master_volume).abs() < 0.02);
        assert!((loaded.sfx_volume - s.sfx_volume).abs() < 0.02);
        assert!((loaded.music_volume - s.music_volume).abs() < 0.02);
        assert_eq!(loaded.invert_y, s.invert_y);
        assert_eq!(loaded.fullscreen, s.fullscreen);
        assert_eq!(loaded.vsync, s.vsync);
        assert_eq!(loaded.view_bobbing, s.view_bobbing);
        assert_eq!(loaded.show_debug_hud, s.show_debug_hud);
    }
}
