//! When the web host's unsaved document is handed over (`wordcraft:autosave`): a short quiet
//! time after the last edit, and at least every [`MAX_MS`] while edits keep coming, never twice
//! within [`QUIET_MS`]. Pure timing, so it is tested natively; `web/host.rs` drives it per frame.

/// Quiet time after the last edit before the document is handed over (also the minimum gap).
pub const QUIET_MS: f64 = 3_000.0;
/// The longest the first unsent edit waits while the user keeps editing.
pub const MAX_MS: f64 = 30_000.0;

/// What a frame should do about the autosave.
#[derive(Debug, PartialEq)]
pub enum Tick {
    /// Nothing unsent.
    Idle,
    /// Wake up again after this many milliseconds.
    Wait(f64),
    /// Hand the document over now.
    Send,
}

#[derive(Default)]
pub struct Debounce {
    /// The document revision last seen, and the one last handed over.
    seen: Option<u64>,
    sent: Option<u64>,
    /// When the oldest unsent edit was first seen, and the newest one.
    first: Option<f64>,
    last_edit: f64,
    last_sent: Option<f64>,
}

impl Debounce {
    /// The document is clean or read only: forget what was seen.
    pub fn reset(&mut self) {
        self.seen = None;
        self.sent = None;
        self.first = None;
    }

    /// Something else (the tab was hidden) handed the document over at `rev`.
    pub fn mark_sent(&mut self, rev: u64, now: f64) {
        self.sent = Some(rev);
        self.first = None;
        self.last_sent = Some(now);
    }

    /// Revision `rev` was handed over already.
    pub fn was_sent(&self, rev: u64) -> bool {
        self.sent == Some(rev)
    }

    /// One frame of an unsaved, editable document at revision `rev`, `now` in milliseconds.
    pub fn tick(&mut self, rev: u64, now: f64) -> Tick {
        if self.sent == Some(rev) {
            self.first = None;
            return Tick::Idle;
        }
        if self.seen != Some(rev) {
            self.seen = Some(rev);
            self.last_edit = now;
        }
        let first = *self.first.get_or_insert(now);
        let earliest = self.last_sent.map_or(f64::NEG_INFINITY, |t| t + QUIET_MS);
        let due = earliest.max((self.last_edit + QUIET_MS).min(first + MAX_MS));
        if now >= due {
            self.mark_sent(rev, now);
            Tick::Send
        } else {
            Tick::Wait(due - now)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_edit_is_sent_after_the_quiet_time() {
        let mut d = Debounce::default();
        assert_eq!(d.tick(1, 1_000.0), Tick::Wait(3_000.0));
        assert_eq!(d.tick(1, 3_999.0), Tick::Wait(1.0));
        assert_eq!(d.tick(1, 4_000.0), Tick::Send);
        // Nothing new since: nothing more, however long the page stays dirty.
        assert_eq!(d.tick(1, 100_000.0), Tick::Idle);
    }

    #[test]
    fn typing_keeps_pushing_the_send_back_until_the_longest_wait() {
        let mut d = Debounce::default();
        let mut sent = Vec::new();
        for s in 0..70u64 {
            let now = s as f64 * 1_000.0;
            if d.tick(s + 1, now) == Tick::Send {
                sent.push(now);
            }
        }
        // The first edit (0 s) waits for the 30 s ceiling; the next unsent one (31 s) for its own.
        assert_eq!(sent, vec![30_000.0, 61_000.0]);
    }

    #[test]
    fn a_second_edit_right_after_a_send_waits_the_minimum_gap() {
        let mut d = Debounce::default();
        assert_eq!(d.tick(1, 0.0), Tick::Wait(3_000.0));
        assert_eq!(d.tick(1, 3_000.0), Tick::Send);
        assert_eq!(d.tick(2, 3_100.0), Tick::Wait(3_000.0));
        assert_eq!(d.tick(2, 6_100.0), Tick::Send);
        assert!(d.last_sent.is_some_and(|t| t - 3_000.0 >= QUIET_MS));
    }

    #[test]
    fn clean_or_read_only_forgets_and_a_handover_by_the_tab_counts() {
        let mut d = Debounce::default();
        d.tick(5, 0.0);
        d.reset();
        assert_eq!(d.tick(5, 10_000.0), Tick::Wait(3_000.0), "an unsaved document at the same revision is new again");
        d.mark_sent(5, 10_500.0);
        assert!(d.was_sent(5) && !d.was_sent(6));
        assert_eq!(d.tick(5, 20_000.0), Tick::Idle);
    }
}
