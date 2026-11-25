use std::collections::VecDeque;

/// Default display duration (seconds) for a single Achievement unlock toast.
pub const ACHIEVEMENT_TOAST_DURATION_SECS: f32 = 4.5;

/// Single queued Achievement unlock toast rendered on the HUD.
#[derive(Debug, Clone, PartialEq)]
pub struct AchievementToast {
    pub title: String,
    pub description: String,
    pub icon_tile: u16,
    pub elapsed: f32,
    pub duration: f32,
}

impl AchievementToast {
    pub fn new(title: impl Into<String>, description: impl Into<String>, icon_tile: u16) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            icon_tile,
            elapsed: 0.0,
            duration: ACHIEVEMENT_TOAST_DURATION_SECS,
        }
    }

    /// Computes smooth fade-in / hold / fade-out alpha in `[0.0, 1.0]`.
    pub fn alpha(&self) -> f32 {
        let fade_in = 0.25;
        let fade_out = 0.65;
        if self.elapsed < fade_in {
            (self.elapsed / fade_in).clamp(0.0, 1.0)
        } else if self.elapsed > self.duration - fade_out {
            ((self.duration - self.elapsed) / fade_out).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

/// FIFO queue ensuring simultaneous Achievement unlocks display sequentially
/// without overwriting each other.
#[derive(Debug, Default, Clone)]
pub struct AchievementToastQueue {
    active: Option<AchievementToast>,
    pending: VecDeque<AchievementToast>,
}

impl AchievementToastQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueues a toast. Returns `true` if it became immediately active (caller should play chime).
    pub fn push(&mut self, toast: AchievementToast) -> bool {
        if self.active.is_none() {
            self.active = Some(toast);
            true
        } else {
            self.pending.push_back(toast);
            false
        }
    }

    /// Advances the active toast by `dt` seconds. Returns `true` if a new queued
    /// toast was promoted to active (caller should play chime).
    pub fn tick(&mut self, dt: f32) -> bool {
        if let Some(active) = &mut self.active {
            active.elapsed += dt.max(0.0);
            if active.elapsed >= active.duration {
                self.active = self.pending.pop_front();
                return self.active.is_some();
            }
        } else if let Some(next) = self.pending.pop_front() {
            self.active = Some(next);
            return true;
        }
        false
    }

    pub fn active(&self) -> Option<&AchievementToast> {
        self.active.as_ref()
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_toast_queue_displays_simultaneous_unlocks_sequentially() {
        let mut q = AchievementToastQueue::new();
        assert!(q.push(AchievementToast::new("Timber!", "Chop your first log.", 5)));
        assert!(!q.push(AchievementToast::new(
            "Getting to Work",
            "Craft a Crafting Table.",
            14
        )));
        assert_eq!(q.active().unwrap().title, "Timber!");
        assert_eq!(q.pending_len(), 1);

        // Midway through first toast: still showing first
        assert!(!q.tick(2.0));
        assert_eq!(q.active().unwrap().title, "Timber!");
        assert!((q.active().unwrap().alpha() - 1.0).abs() < 0.001);

        // Expire first toast -> promotes second in FIFO order and signals chime
        assert!(q.tick(3.0));
        assert_eq!(q.active().unwrap().title, "Getting to Work");
        assert_eq!(q.pending_len(), 0);

        // Expire second toast -> queue becomes empty
        assert!(!q.tick(5.0));
        assert!(q.active().is_none());
    }
}
