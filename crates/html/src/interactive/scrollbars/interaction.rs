//! HTML-only scrollbar hover fading and thumb dragging; state survives document repaint.
use super::modern::Bar;
use gpui::{
    Hitbox, HitboxBehavior, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ScrollHandle, Window, px,
};
use std::{cell::RefCell, rc::Rc, time::Instant};

#[derive(Clone)]
pub struct Interaction(Rc<RefCell<State>>);
impl Default for Interaction {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(State::new(Instant::now()))))
    }
}
struct State {
    from: f32,
    target: f32,
    since: Instant,
    drag: Option<(Bar, f32, f32)>,
}
impl State {
    fn new(now: Instant) -> Self {
        Self {
            from: 0.45,
            target: 0.45,
            since: now,
            drag: None,
        }
    }
    fn alpha(&self, now: Instant) -> f32 {
        let t = (now.duration_since(self.since).as_secs_f32() / 0.12).clamp(0.0, 1.0);
        self.from + (self.target - self.from) * t * t * (3.0 - 2.0 * t)
    }
    fn hover(&mut self, hovered: bool, now: Instant) {
        let target = if self.drag.is_some() {
            1.0
        } else if hovered {
            0.7
        } else {
            0.45
        };
        if self.target != target {
            self.from = self.alpha(now);
            self.target = target;
            self.since = now;
        }
    }
}
pub struct HitBar {
    bar: Bar,
    hitbox: Hitbox,
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepaint(
    freeze: bool,
    c: &crate::style::computed::Computed,
    handle: &ScrollHandle,
    selected: [bool; 2],
    range: [f32; 2],
    insets: [f32; 4],
    bounds: gpui::Bounds<gpui::Pixels>,
    window: &mut Window,
) -> Vec<HitBar> {
    if freeze {
        return Vec::new();
    }
    super::modern::bars(c, handle, selected, range, insets, bounds)
        .into_iter()
        .map(|bar| HitBar {
            hitbox: window.insert_hitbox(bar.lane, HitboxBehavior::Normal),
            bar,
        })
        .collect()
}
impl Interaction {
    pub(crate) fn paint(
        &self,
        freeze: bool,
        bars: &[HitBar],
        handle: &ScrollHandle,
        window: &mut Window,
    ) -> f32 {
        if freeze {
            return 0.45;
        }
        let hovered = bars.iter().any(|bar| bar.hitbox.is_hovered(window));
        let now = Instant::now();
        let mut state = self.0.borrow_mut();
        state.hover(hovered, now);
        let alpha = state.alpha(now);
        if (alpha - state.target).abs() > 0.001 {
            window.request_animation_frame();
        }
        drop(state);
        if bars.is_empty() {
            return alpha;
        }
        let down = self.0.clone();
        let hitbars: Vec<_> = bars.iter().map(|b| (b.bar, b.hitbox.clone())).collect();
        let scroll = handle.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
            if !phase.bubble() || e.button != MouseButton::Left {
                return;
            }
            if let Some((bar, _)) = hitbars
                .iter()
                .find(|(bar, hitbox)| hitbox.is_hovered(window) && bar.thumb.contains(&e.position))
            {
                let coordinate = f32::from(if bar.axis == 0 {
                    e.position.x
                } else {
                    e.position.y
                });
                let offset = scroll.offset();
                let start = -f32::from(if bar.axis == 0 { offset.x } else { offset.y });
                down.borrow_mut().drag = Some((*bar, coordinate, start));
                window.refresh();
                cx.stop_propagation();
            }
        });
        let moving = self.0.clone();
        let hitboxes: Vec<_> = bars.iter().map(|b| b.hitbox.clone()).collect();
        let scroll = handle.clone();
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
            if !phase.bubble() {
                return;
            }
            let mut state = moving.borrow_mut();
            let hovered = hitboxes.iter().any(|b| b.is_hovered(window));
            let previous = state.target;
            state.hover(hovered, Instant::now());
            if previous != state.target {
                window.refresh();
            }
            if let Some((bar, coordinate, start)) = state.drag {
                let pointer = f32::from(if bar.axis == 0 {
                    e.position.x
                } else {
                    e.position.y
                });
                let delta = (pointer - coordinate) * bar.maximum / bar.travel.max(1.0);
                let position =
                    (start + if bar.reverse { -delta } else { delta }).clamp(0.0, bar.maximum);
                let mut offset = scroll.offset();
                if bar.axis == 0 {
                    offset.x = px(-position);
                } else {
                    offset.y = px(-position);
                }
                scroll.set_offset(offset);
                window.refresh();
                cx.stop_propagation();
            }
        });
        let up = self.0.clone();
        window.on_mouse_event(move |e: &MouseUpEvent, phase, window, cx| {
            if phase.bubble()
                && e.button == MouseButton::Left
                && up.borrow_mut().drag.take().is_some()
            {
                window.refresh();
                cx.stop_propagation();
            }
        });
        alpha
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn hover_transition_preserves_rest_state_and_reverses_smoothly() {
        let now = Instant::now();
        let mut state = State::new(now);
        assert_eq!(state.alpha(now), 0.45);
        state.hover(true, now);
        assert!((state.alpha(now + Duration::from_millis(60)) - 0.575).abs() < 0.001);
        assert_eq!(state.alpha(now + Duration::from_millis(120)), 0.7);
        state.hover(false, now + Duration::from_millis(120));
        assert!((state.alpha(now + Duration::from_millis(240)) - 0.45).abs() < 0.001);
        state.hover(true, now + Duration::from_millis(240));
        let reversal = now + Duration::from_millis(300);
        let midpoint = state.alpha(reversal);
        state.hover(false, reversal);
        assert_eq!(state.alpha(reversal), midpoint);
        assert!((state.alpha(reversal + Duration::from_millis(120)) - 0.45).abs() < 0.001);
    }
    #[test]
    fn dragging_is_brightest_and_releases_back_to_hover() {
        let now = Instant::now();
        let mut state = State::new(now);
        state.drag = Some((
            Bar {
                axis: 1,
                lane: Default::default(),
                thumb: Default::default(),
                travel: 10.0,
                maximum: 20.0,
                reverse: false,
            },
            0.0,
            0.0,
        ));
        state.hover(true, now);
        assert_eq!(state.alpha(now + Duration::from_millis(120)), 1.0);
        state.drag = None;
        state.hover(true, now + Duration::from_millis(120));
        assert_eq!(state.alpha(now + Duration::from_millis(240)), 0.7);
    }
}
