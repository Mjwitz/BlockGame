use glam::Vec3;

pub struct DayNightCycle {
    /// Global time in ticks from 0.0 to 24000.0
    pub time: f32,
    /// Rate of time progression (ticks per second). Default 20.0 = 20-minute day.
    pub speed: f32,
}

impl Default for DayNightCycle {
    fn default() -> Self {
        Self::new()
    }
}

impl DayNightCycle {
    pub fn new() -> Self {
        Self {
            time: 6000.0, // Start at noon
            speed: 20.0,
        }
    }

    pub fn set_time(&mut self, ticks: f32) {
        self.time = ticks.rem_euclid(24000.0);
    }

    pub fn update(&mut self, dt: f32) {
        self.time = (self.time + self.speed * dt).rem_euclid(24000.0);
    }

    /// Normalized celestial progress from 0.0 to 1.0
    #[inline]
    pub fn progress(&self) -> f32 {
        self.time / 24000.0
    }

    /// Sun directional unit vector
    pub fn sun_direction(&self) -> Vec3 {
        let angle = self.progress() * std::f32::consts::TAU;
        Vec3::new(angle.cos(), angle.sin(), 0.25).normalize()
    }

    /// Normalized sun height in the sky (-1.0 at midnight, 1.0 at noon)
    #[inline]
    pub fn sun_height(&self) -> f32 {
        let angle = self.progress() * std::f32::consts::TAU;
        angle.sin()
    }

    /// Sunlight intensity multiplier (0.0 at night to 1.0 at noon)
    pub fn sun_intensity(&self) -> f32 {
        self.sun_height().clamp(0.0, 1.0)
    }

    /// Atmospheric sky and fog color
    pub fn sky_color(&self) -> [f32; 4] {
        let h = self.sun_height();

        if h > 0.2 {
            // Full Day: Sky Blue
            [0.53, 0.81, 0.98, 1.0]
        } else if h > -0.1 {
            // Sunset / Sunrise: Interpolate golden orange to dusk violet
            let factor = (h + 0.1) / 0.3; // 0.0 at dusk, 1.0 at daylight
            let r = lerp(0.88, 0.53, factor);
            let g = lerp(0.48, 0.81, factor);
            let b = lerp(0.32, 0.98, factor);
            [r, g, b, 1.0]
        } else {
            // Night: Deep Midnight Navy
            let night_factor = ((-h - 0.1) / 0.9).clamp(0.0, 1.0);
            let r = lerp(0.15, 0.04, night_factor);
            let g = lerp(0.12, 0.05, night_factor);
            let b = lerp(0.28, 0.12, night_factor);
            [r, g, b, 1.0]
        }
    }

    /// Direct light color (sunlight / moonlight) and intensity
    pub fn light_color_and_intensity(&self) -> [f32; 4] {
        let h = self.sun_height();

        if h > 0.1 {
            // Warm bright sunlight
            [1.0, 0.96, 0.88, self.sun_intensity()]
        } else if h > -0.1 {
            // Crimson sunset direct light
            let factor = (h + 0.1) / 0.2;
            let r = lerp(1.0, 1.0, factor);
            let g = lerp(0.5, 0.96, factor);
            let b = lerp(0.2, 0.88, factor);
            [r, g, b, self.sun_intensity().max(0.2)]
        } else {
            // Cool pale moonlight
            let moon_intensity = ((-h) * 0.35).clamp(0.1, 0.35);
            [0.45, 0.55, 0.75, moon_intensity]
        }
    }
}

#[inline(always)]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}
