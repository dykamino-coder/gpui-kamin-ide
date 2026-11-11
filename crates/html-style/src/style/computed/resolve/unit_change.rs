//! Приёмники общего обхода длин (`unit_lengths`): стиль, в который изменение пишется, и проверка
//! без записи, которая его только замечает.
use super::*;

/// Изменение одного места обхода: его значения после свёртки.
pub(super) enum Change {
    Sizes([Option<Len>; 6]),
    Basis(Option<Len>),
    PathLength(Option<Len>),
    FontSize(Option<Len>),
    /// `padding`, `scroll-padding`, `scroll-margin`, `margin`; `None` — без изменений.
    Boxes([Option<[Option<Len>; 4]>; 4]),
    BorderWidth([Option<Len>; 4]),
    Inset([Option<Len>; 4]),
    Radius([Option<Len>; 4]),
    RadiusEll([Option<(Len, Len)>; 4]),
    Gradients([Option<Gradient>; 2]),
    BgLists(BgLists),
    /// Все операнды в порядке `Computed::image_operands`.
    Images(Vec<ImageSource>),
    Gap(Option<Len>, Option<Len>),
    DecorInset([DecorLen; 2]),
    /// Отступы украшений в порядке `decors`.
    DecorInsets(Vec<Option<[DecorLen; 2]>>),
}

/// Приёмник обхода: стиль, который читают шаги, и что делать с изменением.
pub(super) trait Target {
    fn style(&self) -> &Computed;
    fn commit(&mut self, change: Change);
    /// Ответ уже известен: остальные шаги не нужны.
    fn done(&self) -> bool {
        false
    }
}

impl Target for Computed {
    fn style(&self) -> &Computed {
        self
    }
    fn commit(&mut self, change: Change) {
        match change {
            Change::Sizes([w, h, min_w, min_h, max_w, max_h]) => {
                let layout = &mut *self.reset_box_layout;
                (layout.width, layout.height) = (w, h);
                (layout.min_width, layout.min_height) = (min_w, min_h);
                (layout.max_width, layout.max_height) = (max_w, max_h);
            }
            Change::Basis(basis) => self.reset_flex.flex_basis = basis,
            Change::PathLength(path) => self.reset_svg_math.svg_path_length = path,
            Change::FontSize(size) => self.inh.font.font_size = size,
            Change::Boxes(boxes) => {
                let layout = &mut *self.reset_box_layout;
                let sides = [
                    &mut layout.padding,
                    &mut layout.snap.padding,
                    &mut layout.snap.margin,
                    &mut layout.margin,
                ];
                for (side, value) in sides.into_iter().zip(boxes) {
                    if let Some([top, right, bottom, left]) = value {
                        (side.top, side.right) = (top, right);
                        (side.bottom, side.left) = (bottom, left);
                    }
                }
            }
            Change::BorderWidth([top, right, bottom, left]) => {
                let side = &mut self.reset_border.border_width;
                (side.top, side.right, side.bottom, side.left) = (top, right, bottom, left);
            }
            Change::Inset([top, right, bottom, left]) => {
                let side = &mut self.reset_positioned.inset;
                (side.top, side.right, side.bottom, side.left) = (top, right, bottom, left);
            }
            Change::Radius([tl, tr, br, bl]) => {
                let radius = &mut self.reset_border.radius;
                (radius.tl, radius.tr, radius.br, radius.bl) = (tl, tr, br, bl);
            }
            Change::RadiusEll(ell) => self.reset_border.radius_ell = Some(ell),
            Change::Gradients([gradient, current]) => {
                let group = &mut *self.reset_background;
                (group.gradient, group.gradient_current) = (gradient, current);
            }
            Change::BgLists(lists) => self.reset_background.bg_lists = lists,
            Change::Images(images) => {
                for (slot, image) in self.image_operands().zip(images) {
                    *slot = image;
                }
            }
            Change::Gap(row, col) => self.reset_flex.gap = Some((row, col)),
            Change::DecorInset(inset) => self.reset_text_box.td_inset = Some(Some(inset)),
            Change::DecorInsets(insets) => {
                for (decor, inset) in self.reset_text_box.decors.iter_mut().zip(insets) {
                    decor.inset = inset;
                }
            }
        }
    }
}

/// Проверка без записи: обход записал бы изменение или встретил длину, которую отметил `seen`.
pub(super) struct Probe<'a> {
    pub(super) style: &'a Computed,
    pub(super) seen: &'a std::cell::Cell<bool>,
    pub(super) changed: bool,
}

impl Target for Probe<'_> {
    fn style(&self) -> &Computed {
        self.style
    }
    fn commit(&mut self, _: Change) {
        self.changed = true;
    }
    fn done(&self) -> bool {
        self.changed || self.seen.get()
    }
}

/// Один шаг обхода: прочитать стиль приёмника и отдать ему изменение, если оно есть.
pub(super) fn step<T: Target>(target: &mut T, read: impl FnOnce(&Computed) -> Option<Change>) {
    if target.done() {
        return;
    }
    if let Some(change) = read(target.style()) {
        target.commit(change);
    }
}
