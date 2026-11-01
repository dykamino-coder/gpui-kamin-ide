//! Применение к GPUI: метаданные элемента сетки/флекса, которые обязаны
//! переехать на анонимную обёртку размера по содержимому (`render::content_sized`)
//! — ведь в контейнере родителя стоит она. Это флаги ориентации базовой линии
//! (`baseline_x_flags`), `align-self`/`justify-self` с признаком `safe`,
//! проекция на оси вертикального родителя и baseline содержимого сетки.
use crate::style::computed::BoxStyle;

fn grid_mode(c: &BoxStyle) -> u8 {
    if c.context.parent_grid != 0 {
        c.context.parent_grid
    } else if c.context.parent_lanes {
        match (c.context.cb_wm.vertical(), c.context.cb_wm.block_rl()) {
            (false, _) => 1,
            (true, false) => 2,
            (true, true) => 3,
        }
    } else {
        0
    }
}

pub(super) fn baseline_x_flags(c: &BoxStyle) -> Option<u8> {
    let parent = grid_mode(c);
    if parent == 0 {
        if !c.context.parent_flex_grid {
            return None;
        }
        let central =
            c.context.cb_wm.vertical() && !c.context.cb_wm.sideways() && !c.wm().text_sideways();
        if c.wm().vertical() {
            return Some(4 | u8::from(c.wm().block_rl()) | if central { 2 } else { 0 });
        }
        return c
            .context
            .cb_wm
            .vertical()
            .then(|| u8::from(c.context.cb_wm.block_rl()) | if central { 2 } else { 0 });
    }
    let vertical = c.wm().vertical();
    let rl = if c.context.parent_grid != 0
        && matches!(
            c.reset_positioned.position,
            Some(
                crate::style::computed::Position::Absolute
                    | crate::style::computed::Position::Fixed
            )
        ) {
        // No baseline-sharing context: fallback is the item's own self-start.
        super::grid_flow_axes::reversed(c)[0]
    } else if vertical {
        c.wm().block_rl()
    } else {
        parent == 3
    };
    let mut bits = u8::from(rl);
    if parent >= 2 && !c.context.cb_wm.sideways() && !c.wm().text_sideways() {
        bits |= 2;
    }
    if vertical {
        bits |= 4;
    } else if parent == 1 {
        bits |= 8;
    }
    Some(bits)
}

pub(crate) fn project_wrapper(style: &mut gpui::StyleRefinement, c: &BoxStyle) {
    style.baseline_x_flags = baseline_x_flags(c);
    if grid_mode(c) == 0 {
        return;
    }
    style.align_self = c
        .reset_flex
        .align_self
        .map(|a| super::self_align(a, c.reset_flex.align_self_last));
    style.justify_self = c
        .reset_flex
        .justify_self
        .map(|a| super::self_align(a, c.reset_flex.justify_self_last));
    if c.reset_flex.align_self_safe || c.reset_flex.justify_self_safe {
        style.safe_alignment = Some((false, c.reset_flex.align_self_safe, false, false));
        style.safe_justify_alignment = Some((false, c.reset_flex.justify_self_safe));
    }
    super::alignment_axes::abspos_normal(style, c);
    super::alignment_axes::grid_self(style, c);
    super::alignment_axes::project(style, false, grid_mode(c) >= 2);
    super::grid_content_baseline::apply(style, c);
}

#[cfg(test)]
mod tests;
