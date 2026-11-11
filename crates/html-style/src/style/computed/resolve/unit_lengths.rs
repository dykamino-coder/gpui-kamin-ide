//! Общий обход длин в единицах окна и контейнера.
//!
//! Обход один для записи и для проверки без записи (`Computed::has_viewport_lengths`): каждый
//! шаг читает стиль, сворачивает длины в копиях и отдаёт приёмнику (`unit_change::Target`) только
//! изменение. Группа пишется, только если значение изменилось: общая группа без единиц окна
//! остаётся общей (Blink `ComputedStyle` setters сравнивают перед записью, `computed_style.h`
//! `SetBorderImageOutset`).
use super::unit_change::{Change, Target, step};
use super::*;

/// The lengths of `now` after `fix`, in the same order; `None` when none changed.
pub(super) fn fixed<const N: usize>(
    fix: impl Fn(&mut Option<Len>),
    now: [Option<Len>; N],
) -> Option<[Option<Len>; N]> {
    let next = now.map(|mut value| {
        fix(&mut value);
        value
    });
    (next != now).then_some(next)
}

fn sides_of(s: &Sides) -> [Option<Len>; 4] {
    [s.top, s.right, s.bottom, s.left]
}

/// Радиус после свёртки: отрицательный в точках — ноль.
fn radius(fix: &impl Fn(&mut Option<Len>), mut radius: Option<Len>) -> Option<Len> {
    fix(&mut radius);
    if let Some(Len::Px(value)) = &mut radius {
        *value = value.max(0.0);
    }
    radius
}

fn walk_radius_lengths<T: Target>(target: &mut T, fix: &impl Fn(&mut Option<Len>)) {
    step(target, |s| {
        let r = &s.reset_border.radius;
        let now = [r.tl, r.tr, r.br, r.bl];
        let next = now.map(|corner| radius(fix, corner));
        (next != now).then_some(Change::Radius(next))
    });
    step(target, |s| {
        let now = s.reset_border.radius_ell?;
        let next = now.map(|corner| {
            corner.map(|(x, y)| {
                let axis = |v| radius(fix, Some(v)).unwrap_or(Len::Px(0.0));
                (axis(x), axis(y))
            })
        });
        (next != now).then_some(Change::RadiusEll(next))
    });
}

/// Обход длин стиля приёмника. Порядок свёртки постоянный: `calc` пишет в арену кадра по порядку.
pub(super) fn walk_unit_lengths<T: Target>(
    target: &mut T,
    fix: impl Fn(&mut Option<Len>) + Copy,
    vcx: ValueCx<'_>,
) {
    step(target, |s| {
        let l = &*s.reset_box_layout;
        let now = [
            l.width,
            l.height,
            l.min_width,
            l.min_height,
            l.max_width,
            l.max_height,
        ];
        fixed(fix, now).map(Change::Sizes)
    });
    step(target, |s| {
        fixed(fix, [s.reset_flex.flex_basis]).map(|[basis]| Change::Basis(basis))
    });
    step(target, |s| {
        fixed(fix, [s.reset_svg_math.svg_path_length]).map(|[path]| Change::PathLength(path))
    });
    // Кегль — в общей группе наследуемых: правится, только если он в
    // единицах окна или контейнера.
    step(target, |s| {
        let size = s.inh.font.font_size?;
        fixed(fix, [Some(size)]).map(|[size]| Change::FontSize(size))
    });
    step(target, |s| {
        let l = &*s.reset_box_layout;
        let next = [&l.padding, &l.snap.padding, &l.snap.margin, &l.margin]
            .map(|sides| fixed(fix, sides_of(sides)));
        next.iter()
            .any(Option::is_some)
            .then_some(Change::Boxes(next))
    });
    // Толщина рамки в единицах окна (`border-bottom: 50vh solid`,
    // `monolithic-overflow-021`): без перевода рамка выходила нулевой.
    step(target, |s| {
        fixed(fix, sides_of(&s.reset_border.border_width)).map(Change::BorderWidth)
    });
    step(target, |s| {
        fixed(fix, sides_of(&s.reset_positioned.inset)).map(Change::Inset)
    });
    walk_radius_lengths(target, &fix);
    step(target, |s| {
        let group = &*s.reset_background;
        let now = [&group.gradient, &group.gradient_current];
        if !now
            .iter()
            .any(|g| g.as_ref().is_some_and(Gradient::has_lengths))
        {
            return None;
        }
        let mut next = now.map(Clone::clone);
        for gradient in next.iter_mut().flatten() {
            gradient.resolve_lengths(fix, vcx);
        }
        (next.each_ref() != now).then_some(Change::Gradients(next))
    });
    step(target, |s| {
        let now = &s.reset_background.bg_lists;
        if now.is_empty() {
            return None;
        }
        let mut next = now.clone();
        next.resolve_lengths(&fix, vcx);
        (next != *now).then_some(Change::BgLists(next))
    });
    step(target, |s| {
        if !s.image_operands_ref().any(ImageSource::has_lengths) {
            return None;
        }
        let mut next: Vec<ImageSource> = s.image_operands_ref().cloned().collect();
        for image in &mut next {
            image.resolve_lengths(&fix, vcx);
        }
        next.iter()
            .ne(s.image_operands_ref())
            .then_some(Change::Images(next))
    });
    step(target, |s| {
        let (row, col) = s.reset_flex.gap?;
        fixed(fix, [row, col]).map(|[row, col]| Change::Gap(row, col))
    });
}

impl Computed {
    pub(super) fn resolve_unit_lengths(
        &mut self,
        fix: impl Fn(&mut Option<Len>) + Copy,
        vcx: ValueCx<'_>,
    ) {
        walk_unit_lengths(self, fix, vcx);
    }

    /// Свернуть радиусы скругления; отрицательный радиус в точках — ноль.
    pub fn resolve_radius_lengths(&mut self, fix: impl Fn(&mut Option<Len>)) {
        walk_radius_lengths(self, &fix);
    }
}
