//! # Presence Tracker
//!
//! Tracks whether a person is present and manages the 30-second disappearance timeout.
//!
//! ## Behaviour
//! - When a person is detected: state → `Present`, timer resets.
//! - When no person for < 30 s: state → `TimingOut(remaining)`.
//! - When no person for ≥ 30 s: state → `Absent` (fan should be turned off).

use std::time::{Duration, Instant};

/// Default timeout before "absent" is declared (30 seconds).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Presence state returned by [`PresenceTracker::update`].
#[derive(Debug, Clone, PartialEq)]
pub enum PresenceState {
    /// A person is currently detected.
    Present,
    /// No person detected but within the timeout window.
    ///
    /// Inner value: remaining time before `Absent` is declared.
    TimingOut(Duration),
    /// No person detected and the timeout has elapsed — turn the fan off.
    Absent,
}

impl PresenceState {
    /// Returns `true` if the fan should be running (Present or TimingOut).
    pub fn should_fan_run(&self) -> bool {
        !matches!(self, PresenceState::Absent)
    }

    /// Convenience: is someone currently detected?
    pub fn is_present(&self) -> bool {
        matches!(self, PresenceState::Present)
    }
}

/// Tracks human presence and the disappearance timeout.
///
/// # Example
/// ```
/// use ctrl_eqip::logic::presence::{PresenceTracker, PresenceState};
///
/// let mut tracker = PresenceTracker::default();
/// assert_eq!(tracker.update(true), PresenceState::Present);
/// assert!(matches!(tracker.update(false), PresenceState::TimingOut(_))); // within 30 s
/// ```
#[derive(Debug)]
pub struct PresenceTracker {
    /// Time of last confirmed detection. None = never seen.
    last_seen: Option<Instant>,
    /// How long to wait after losing sight before declaring Absent.
    timeout: Duration,
}

impl Default for PresenceTracker {
    fn default() -> Self {
        Self::new(DEFAULT_TIMEOUT)
    }
}

impl PresenceTracker {
    /// Create a tracker with a custom timeout.
    pub fn new(timeout: Duration) -> Self {
        Self {
            last_seen: None,
            timeout,
        }
    }

    /// Update the tracker with the latest detection result and return the new state.
    ///
    /// Call this once per inference frame (typically 10–30 fps).
    ///
    /// `person_detected` — whether the AI engine found ≥1 person in this frame.
    pub fn update(&mut self, person_detected: bool) -> PresenceState {
        let now = Instant::now();

        if person_detected {
            self.last_seen = Some(now);
            return PresenceState::Present;
        }

        match self.last_seen {
            None => PresenceState::Absent,
            Some(last) => {
                let elapsed = now.duration_since(last);
                if elapsed >= self.timeout {
                    PresenceState::Absent
                } else {
                    PresenceState::TimingOut(self.timeout - elapsed)
                }
            }
        }
    }

    /// Reset the tracker (e.g. system restart / manual override).
    pub fn reset(&mut self) {
        self.last_seen = None;
    }

    /// Returns the duration since the last detection, or `None` if never seen.
    pub fn time_since_last_seen(&self) -> Option<Duration> {
        self.last_seen.map(|t| t.elapsed())
    }
}

