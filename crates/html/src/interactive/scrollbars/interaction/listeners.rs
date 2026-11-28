//! Track clicks set smooth goals; thumb dragging cancels the active trajectory.
use super::*;
use crate::interactive::scroll_animation::{ScrollBehavior, Source};
use gpui::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, px};

impl Interaction {
    pub(super) fn listen(
        &self,
        bars: &[HitBar],
        handle: &ScrollHandle,
        animation: &Scroll,
        window: &mut Window,
    ) {
        let down = self.0.clone();
        let animator = animation.clone();
        let hitbars: Vec<_> = bars.iter().map(|b| (b.bar, b.hitbox.clone())).collect();
        let scroll = handle.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
            if !phase.bubble() || e.button != MouseButton::Left {
                return;
            }
            if let Some((bar, _)) = hitbars.iter().find(|(_, hitbox)| hitbox.is_hovered(window)) {
                let coordinate = f32::from(if bar.axis == 0 {
                    e.position.x
                } else {
                    e.position.y
                });
                if !bar.thumb.contains(&e.position) {
                    let start = f32::from(if bar.axis == 0 {
                        bar.thumb.left()
                    } else {
                        bar.thumb.top()
                    });
                    let page = f32::from(if bar.axis == 0 {
                        scroll.bounds().size.width
                    } else {
                        scroll.bounds().size.height
                    }) * 0.875;
                    let direction = if coordinate < start { 1. } else { -1. }
                        * if bar.reverse { -1. } else { 1. };
                    let lane_start = f32::from(if bar.axis == 0 {
                        bar.lane.left()
                    } else {
                        bar.lane.top()
                    });
                    let lane_end = f32::from(if bar.axis == 0 {
                        bar.lane.right()
                    } else {
                        bar.lane.bottom()
                    });
                    let distance = if bar.classic
                        && (coordinate < lane_start + bar.button
                            || coordinate >= lane_end - bar.button)
                    {
                        40.
                    } else {
                        page
                    };
                    let mut target = animator.target();
                    if bar.axis == 0 {
                        target.x += px(distance * direction);
                    } else {
                        target.y += px(distance * direction);
                    }
                    animator.to(target, ScrollBehavior::Smooth, Source::Keyboard, window, cx);
                    cx.stop_propagation();
                    return;
                }
                animator.cancel();
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
    }
}
