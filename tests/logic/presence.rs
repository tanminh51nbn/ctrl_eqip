use ctrl_eqip::logic::presence::{PresenceTracker, PresenceState};
use std::time::Duration;
use std::thread::sleep;

#[test]
fn present_when_detected() {
    let mut tracker = PresenceTracker::default();
    assert_eq!(tracker.update(true), PresenceState::Present);
}

#[test]
fn timing_out_after_loss() {
    let mut tracker = PresenceTracker::default();
    tracker.update(true); // person seen
    let state = tracker.update(false); // lost — within 30s window
    assert!(matches!(state, PresenceState::TimingOut(_)));
}

#[test]
fn absent_when_never_seen() {
    let mut tracker = PresenceTracker::default();
    assert_eq!(tracker.update(false), PresenceState::Absent);
}

#[test]
fn absent_after_short_timeout() {
    // Use a short timeout for fast testing
    let mut tracker = PresenceTracker::new(Duration::from_millis(50));
    tracker.update(true);
    sleep(Duration::from_millis(60));
    assert_eq!(tracker.update(false), PresenceState::Absent);
}

#[test]
fn reset_clears_state() {
    let mut tracker = PresenceTracker::default();
    tracker.update(true);
    tracker.reset();
    assert!(tracker.time_since_last_seen().is_none());
    assert_eq!(tracker.update(false), PresenceState::Absent);
}
