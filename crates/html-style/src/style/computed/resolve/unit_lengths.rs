//! Share one read/write traversal: probe changes before deriving a group; calc folding creates a new owned expression only when needed.
use super::unit_change::{Target, lengths, sides};
use super::*;

/// Радиус после свёртки: отрицательный в точках — ноль.
fn radius(fix: &impl Fn(&mut Option<Len>), mut radius: Option<Len>) -> Option<Len> {
    fix(&mut radius);
    if let Some(Len::Px(value)) = &mut radius {
        *value = value.max(0.0);
    }
    radius
}

/// Изменит ли `resolve_radius_lengths` радиусы: длина сворачивается или отрицательна.
fn radius_pending(s: &Computed, moves: impl Fn(&Len) -> bool) -> bool {
    let moves = |l: &Len| moves(l) || matches!(l, Len::Px(v) if *v < 0.0);
    let r = &s.reset_border.radius;
    [r.tl.as_ref(), r.tr.as_ref(), r.br.as_ref(), r.bl.as_ref()]
        .into_iter()
        .flatten()
        .any(moves)
        || s.reset_border
            .radius_ell
            .iter()
            .flatten()
            .flatten()
            .any(|(x, y)| moves(x) || moves(y))
}

/// Visit target-style lengths in a deterministic order, creating owned calc expressions as needed.
pub(super) fn walk_unit_lengths<T: Target>(
    target: &mut T,
    moves: impl Fn(&Len) -> bool + Copy,
    fix: impl Fn(&mut Option<Len>) + Copy,
    vcx: ValueCx<'_>,
) {
    lengths(
        target,
        moves,
        fix,
        |s| {
            let l = &*s.reset_box_layout;
            [
                l.width.as_ref(),
                l.height.as_ref(),
                l.min_width.as_ref(),
                l.min_height.as_ref(),
                l.max_width.as_ref(),
                l.max_height.as_ref(),
            ]
        },
        |s, [w, h, min_w, min_h, max_w, max_h]| {
            let l = &mut *s.reset_box_layout;
            (l.width, l.height) = (w, h);
            (l.min_width, l.min_height) = (min_w, min_h);
            (l.max_width, l.max_height) = (max_w, max_h);
        },
    );
    lengths(
        target,
        moves,
        fix,
        |s| [s.reset_flex.flex_basis.as_ref()],
        |s, [basis]| s.reset_flex.flex_basis = basis,
    );
    lengths(
        target,
        moves,
        fix,
        |s| [s.reset_svg_math.svg_path_length.as_ref()],
        |s, [path]| s.reset_svg_math.svg_path_length = path,
    );
    // Кегль — в общей группе наследуемых: правится, только если он в
    // единицах окна или контейнера.
    lengths(
        target,
        moves,
        fix,
        |s| [s.inh.font.font_size.as_ref()],
        |s, [size]| s.inh.font.font_size = size,
    );
    sides(
        target,
        moves,
        fix,
        |s| &s.reset_box_layout.padding,
        |s| &mut s.reset_box_layout.padding,
    );
    sides(
        target,
        moves,
        fix,
        |s| &s.reset_box_layout.snap.padding,
        |s| &mut s.reset_box_layout.snap.padding,
    );
    sides(
        target,
        moves,
        fix,
        |s| &s.reset_box_layout.snap.margin,
        |s| &mut s.reset_box_layout.snap.margin,
    );
    sides(
        target,
        moves,
        fix,
        |s| &s.reset_box_layout.margin,
        |s| &mut s.reset_box_layout.margin,
    );
    // Толщина рамки в единицах окна (`border-bottom: 50vh solid`,
    // `monolithic-overflow-021`): без перевода рамка выходила нулевой.
    sides(
        target,
        moves,
        fix,
        |s| &s.reset_border.border_width,
        |s| &mut s.reset_border.border_width,
    );
    sides(
        target,
        moves,
        fix,
        |s| &s.reset_positioned.inset,
        |s| &mut s.reset_positioned.inset,
    );
    target.rewrite(
        |s| radius_pending(s, moves),
        |s| s.resolve_radius_lengths(fix),
    );
    target.rewrite(
        |s| {
            let group = &*s.reset_background;
            [&group.gradient, &group.gradient_current]
                .into_iter()
                .flatten()
                .any(|gradient| gradient.lengths_pending(moves, vcx))
        },
        |s| {
            let group = &mut *s.reset_background;
            for gradient in [&mut group.gradient, &mut group.gradient_current]
                .into_iter()
                .flatten()
            {
                gradient.resolve_lengths(fix, vcx);
            }
        },
    );
    target.rewrite(
        |s| s.reset_background.bg_lists.lengths_pending(moves, vcx),
        |s| s.reset_background.bg_lists.resolve_lengths(&fix, vcx),
    );
    target.rewrite(
        |s| {
            s.image_operands_ref()
                .any(|image| image.lengths_pending(moves, vcx))
        },
        |s| {
            for image in s.image_operands() {
                image.resolve_lengths(&fix, vcx);
            }
        },
    );
    lengths(
        target,
        moves,
        fix,
        |s| {
            s.reset_flex
                .gap
                .as_ref()
                .map_or([None; 2], |(row, col)| [row.as_ref(), col.as_ref()])
        },
        |s, [row, col]| s.reset_flex.gap = Some((row, col)),
    );
}

impl Computed {
    pub(super) fn resolve_unit_lengths(
        &mut self,
        moves: impl Fn(&Len) -> bool + Copy,
        fix: impl Fn(&mut Option<Len>) + Copy,
        vcx: ValueCx<'_>,
    ) {
        walk_unit_lengths(self, moves, fix, vcx);
    }

    /// Свернуть радиусы скругления; отрицательный радиус в точках — ноль. Группа рамки берётся
    /// на запись, только если радиус изменился.
    pub fn resolve_radius_lengths(&mut self, fix: impl Fn(&mut Option<Len>)) {
        let r = &self.reset_border.radius;
        let now = [r.tl.clone(), r.tr.clone(), r.br.clone(), r.bl.clone()];
        let next = now.clone().map(|corner| radius(&fix, corner));
        if next != now {
            let r = &mut self.reset_border.radius;
            [r.tl, r.tr, r.br, r.bl] = next;
        }
        let Some(ref now) = self.reset_border.radius_ell else {
            return;
        };
        let axis = |v| radius(&fix, Some(v)).unwrap_or(Len::Px(0.0));
        let next = now
            .clone()
            .map(|corner| corner.map(|(x, y)| (axis(x), axis(y))));
        if next != *now {
            self.reset_border.radius_ell = Some(next);
        }
    }
}
