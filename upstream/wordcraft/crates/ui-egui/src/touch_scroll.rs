//! One-finger scrolling on a touch screen.
//!
//! On a phone a finger dragged across the page scrolls it, with momentum after the finger lifts,
//! the way every other app does. A mouse drag still selects text, and so does a long press
//! followed by a drag. So a finger's press isn't carried out at once: it waits until the finger
//! has moved ([`SLOP`]: a scroll), lifted ([`TouchScroll::take_press`]: a tap, carried out where
//! it landed) or stayed down for [`egui::InputOptions::max_click_duration`] (a long press, carried out
//! the same way, so the drag that follows selects as it does with a mouse).
//!
//! Touches reach the app as pointer events (egui's web runner turns the first finger into a mouse
//! pointer) together with [`egui::Event::Touch`], which is what tells a finger from a mouse here.

use egui::{InputState, Pos2, Rect, TouchPhase, Vec2};

use crate::scroll::Momentum;

/// Finger travel (points) that makes a touch a scroll, not a tap: about Android's touch slop of
/// 8 dp.
const SLOP: f32 = 8.0;
/// A finger that rested this long before lifting doesn't fling (seconds).
const STALE: f64 = 0.1;

#[derive(Clone, Copy, Debug, Default)]
enum Phase {
    #[default]
    Idle,
    /// A finger went down at `start` at time `since`; tap, scroll or long press isn't known yet.
    Waiting { start: Pos2, since: f64 },
    /// The finger is dragging the page; `last` is where it was in the previous frame and `moved`
    /// the time it last moved.
    Scrolling { last: Pos2, moved: f64 },
}

#[derive(Clone, Debug, Default)]
pub struct TouchScroll {
    phase: Phase,
    /// The finger's last known position (a lifted finger has none).
    last_pos: Option<Pos2>,
    momentum: Momentum,
    /// A held-back press to carry out this frame, and whether it is a long press.
    press: Option<(Pos2, bool)>,
    /// The last finger scrolled the page (until the next one lands).
    scrolled: bool,
}

impl TouchScroll {
    /// A finger went down in `area` this frame: hold the press back until it is clear what the
    /// finger is doing. `false` for a mouse, a pen without touch events, a press outside `area`
    /// or a second finger (a pinch zoom), which the caller handles as before.
    pub fn hold_press(&mut self, input: &InputState, area: Rect) -> bool {
        if !input.pointer.primary_pressed() || input.multi_touch().is_some() {
            return false;
        }
        let started = input.events.iter().find_map(|e| match e {
            egui::Event::Touch { phase: TouchPhase::Start, pos, .. } => Some(*pos),
            _ => None,
        });
        let Some(start) = started.filter(|start| area.contains(*start)) else { return false };
        self.momentum.cancel();
        self.scrolled = false;
        // A slow frame batches the whole gesture: the finger may have moved or lifted already.
        self.last_pos = finger_pos(input).or(Some(start));
        self.phase = Phase::Waiting { start, since: input.time };
        true
    }

    /// A held-back press to carry out now, where the finger first landed, and whether the finger
    /// stayed down long enough to be a long press (which selects the word, as a double-click).
    pub fn take_press(&mut self) -> Option<(Pos2, bool)> {
        self.press.take()
    }

    /// This frame's scroll in points (positive: the content moves down and right), from the
    /// finger's travel, then from the momentum left after it lifted. Call it once per frame,
    /// before the page is drawn. `long_press`: seconds a finger rests to count as a long press.
    pub fn frame(&mut self, input: &InputState, long_press: f64) -> Vec2 {
        self.press = None;
        let dt = input.unstable_dt;
        if let Some(pos) = finger_pos(input) {
            self.last_pos = Some(pos);
        }
        // The finger is gone when egui says so; a cancelled touch never releases the pointer.
        let lifted = input.pointer.primary_released() || !input.any_touches();
        let pinching = input.multi_touch().is_some();
        match self.phase {
            Phase::Idle => {
                if input.pointer.primary_pressed() {
                    self.momentum.cancel();
                    Vec2::ZERO
                } else if self.momentum.is_active() {
                    self.momentum.tick(dt)
                } else {
                    Vec2::ZERO
                }
            }
            Phase::Waiting { start, since } => {
                let at = self.last_pos.unwrap_or(start);
                if pinching {
                    self.phase = Phase::Idle;
                } else if at.distance(start) > SLOP {
                    self.phase = Phase::Scrolling { last: start, moved: input.time };
                    self.scrolled = true;
                    return self.frame_scrolling(input, dt);
                } else if lifted || input.time - since > long_press {
                    // A tap, or a long press that goes on to drag-select.
                    self.press = Some((start, !lifted));
                    self.phase = Phase::Idle;
                }
                Vec2::ZERO
            }
            Phase::Scrolling { .. } => self.frame_scrolling(input, dt),
        }
    }

    fn frame_scrolling(&mut self, input: &InputState, dt: f32) -> Vec2 {
        let Phase::Scrolling { last, moved } = self.phase else { return Vec2::ZERO };
        let at = self.last_pos.unwrap_or(last);
        let lifted = input.pointer.primary_released() || !input.any_touches();
        // The content follows the finger (positive moves it down and right, like a wheel delta).
        let delta = at - last;
        let moved = if delta == Vec2::ZERO { moved } else { input.time };
        if delta != Vec2::ZERO {
            self.momentum.feed(delta, dt);
        }
        if input.multi_touch().is_some() {
            self.momentum.cancel();
            self.phase = Phase::Idle;
        } else if lifted {
            // A finger that rested before lifting doesn't fling.
            if input.time - moved > STALE {
                self.momentum.cancel();
            } else {
                self.momentum.release();
            }
            self.phase = Phase::Idle;
        } else {
            self.phase = Phase::Scrolling { last: at, moved };
        }
        delta
    }

    /// A finger is down and not yet known to be a tap or a long press, or is scrolling the page.
    pub fn in_gesture(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    /// The finger that went down last scrolled the page: the drag egui reports for it (possibly
    /// frames late) isn't a reason to take the keyboard.
    pub fn scrolled(&self) -> bool {
        self.scrolled
    }

    /// The page ran into the end of the document: stop coasting.
    pub fn hit_edge(&mut self) {
        self.momentum.cancel();
    }

    /// The page keeps moving, or a press is still undecided: keep repainting.
    pub fn is_active(&self) -> bool {
        !matches!(self.phase, Phase::Idle) || self.momentum.is_active()
    }
}

/// Where the finger is, from this frame's events (a lifted finger leaves the pointer without a
/// position, and a slow frame can hold the whole gesture).
fn finger_pos(input: &InputState) -> Option<Pos2> {
    input
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            egui::Event::Touch { pos, .. } | egui::Event::PointerMoved(pos) => Some(*pos),
            _ => None,
        })
        .or_else(|| input.pointer.latest_pos())
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton, TouchDeviceId, TouchId, TouchPhase};

    /// Runs one frame at `time` with `events`; returns the scroll and any press to carry out.
    fn frame(ctx: &egui::Context, ts: &mut TouchScroll, time: f64, events: Vec<Event>, press_at: Option<Pos2>) -> (Vec2, Option<(Pos2, bool)>) {
        let mut out = (Vec2::ZERO, None);
        let raw = egui::RawInput { time: Some(time), predicted_dt: 1.0 / 60.0, events, ..Default::default() };
        ctx.run_ui(raw, |ui| {
            ui.input(|i| {
                let scroll = ts.frame(i, 0.8);
                if press_at.is_some() && i.pointer.primary_pressed() {
                    let area = Rect::from_min_size(Pos2::ZERO, Vec2::splat(2000.0));
                    assert!(ts.hold_press(i, area), "a finger's press is held back");
                }
                out = (scroll, ts.take_press());
            })
        })
        .drop_without_applying_deltas();
        out
    }

    fn touch(phase: TouchPhase, pos: Pos2) -> Event {
        Event::Touch { device_id: TouchDeviceId(0), id: TouchId(0), phase, pos, force: None }
    }

    fn button(pressed: bool, pos: Pos2) -> Event {
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() }
    }

    fn down(pos: Pos2) -> Vec<Event> {
        vec![Event::PointerMoved(pos), button(true, pos), touch(TouchPhase::Start, pos)]
    }

    fn drag(pos: Pos2) -> Vec<Event> {
        vec![Event::PointerMoved(pos), touch(TouchPhase::Move, pos)]
    }

    fn up(pos: Pos2) -> Vec<Event> {
        vec![button(false, pos), Event::PointerGone, touch(TouchPhase::End, pos)]
    }

    /// A finger dragged up moves the content up (a negative delta), then it coasts on after a
    /// quick flick; nothing is pressed into the document.
    #[test]
    fn a_dragging_finger_scrolls_with_momentum() {
        let ctx = egui::Context::default();
        let mut ts = TouchScroll::default();
        let mut t = 1.0;
        let mut at = Pos2::new(100.0, 500.0);
        assert_eq!(frame(&ctx, &mut ts, t, down(at), Some(at)), (Vec2::ZERO, None));
        let mut total = 0.0;
        for _ in 0..6 {
            t += 1.0 / 60.0;
            at.y -= 20.0;
            let (scroll, press) = frame(&ctx, &mut ts, t, drag(at), None);
            assert_eq!(press, None, "a scroll never presses");
            total += scroll.y;
        }
        assert!((total + 120.0).abs() < 0.01, "the content follows the finger: {total}");
        t += 1.0 / 60.0;
        let (scroll, press) = frame(&ctx, &mut ts, t, up(at), None);
        assert_eq!((scroll, press), (Vec2::ZERO, None));
        t += 1.0 / 60.0;
        let (coast, _) = frame(&ctx, &mut ts, t, vec![], None);
        assert!(coast.y < 0.0, "the page coasts on after the flick: {coast:?}");
        assert!(ts.is_active());
    }

    /// A slow frame can hold the press, the drag and the lift at once: still a scroll.
    #[test]
    fn a_gesture_in_one_frame_is_a_scroll() {
        let ctx = egui::Context::default();
        let mut ts = TouchScroll::default();
        let (from, to) = (Pos2::new(100.0, 500.0), Pos2::new(100.0, 300.0));
        let mut events = down(from);
        events.extend(drag(to));
        events.extend(up(to));
        assert_eq!(frame(&ctx, &mut ts, 1.0, events, Some(from)), (Vec2::ZERO, None));
        let (scroll, press) = frame(&ctx, &mut ts, 1.2, vec![], None);
        assert_eq!(press, None);
        assert!((scroll.y + 200.0).abs() < 0.01, "{scroll:?}");
    }

    /// A tap presses where it landed, once the finger lifts; a long press presses while the
    /// finger is still down, so the drag after it selects.
    #[test]
    fn taps_and_long_presses_are_carried_out() {
        let ctx = egui::Context::default();
        let mut ts = TouchScroll::default();
        let at = Pos2::new(50.0, 60.0);
        frame(&ctx, &mut ts, 1.0, down(at), Some(at));
        let (_, press) = frame(&ctx, &mut ts, 1.1, up(at), None);
        assert_eq!(press, Some((at, false)));
        assert!(!ts.is_active());

        frame(&ctx, &mut ts, 2.0, down(at), Some(at));
        let (_, press) = frame(&ctx, &mut ts, 2.5, vec![], None);
        assert_eq!(press, None, "not long enough yet");
        let (scroll, press) = frame(&ctx, &mut ts, 3.0, vec![], None);
        assert_eq!((scroll, press), (Vec2::ZERO, Some((at, true))));
        let (scroll, _) = frame(&ctx, &mut ts, 3.1, drag(Pos2::new(50.0, 160.0)), None);
        assert_eq!(scroll, Vec2::ZERO, "after a long press the drag is the selection's");
    }

    /// A mouse is not a finger: its press is not held back.
    #[test]
    fn a_mouse_press_is_not_held_back() {
        let ctx = egui::Context::default();
        let mut ts = TouchScroll::default();
        let at = Pos2::new(50.0, 60.0);
        ctx.run_ui(egui::RawInput { time: Some(1.0), events: vec![Event::PointerMoved(at), button(true, at)], ..Default::default() }, |ui| {
            ui.input(|i| assert!(!ts.hold_press(i, Rect::from_min_size(Pos2::ZERO, Vec2::splat(2000.0)))));
        })
        .drop_without_applying_deltas();
        assert!(!ts.is_active());
    }
}
