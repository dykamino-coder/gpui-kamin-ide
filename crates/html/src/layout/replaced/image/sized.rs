//! Размеры картинки по заданным ширине/высоте и соотношению сторон.

use super::used_size::{Picture, PictureSizing};
use crate::defaults::{DEFAULT_REPLACED_HEIGHT_PX, DEFAULT_REPLACED_WIDTH_PX};
use crate::style::values::value::Len;
use crate::tolerance::COORD_EPS;

impl PictureSizing<'_> {
    /// Рисунок по заданным сторонам и суженная коробка содержимого: когда предел
    /// изменил выведенную сторону (§10.4), коробка обязана ужаться вместе с рисунком.
    pub(crate) fn picture(&self) -> (Picture, Option<(f32, f32)>) {
        let style = self.style;
        let (sub_w, sub_h) = self.sub;
        let (max_w, max_h) = (self.max_w, self.max_h);
        let mut узкая = None;
        let picture = if let (Some(Len::Px(w)), Some(Len::Px(h))) = (
            style.reset_box_layout.width.clone(),
            style.reset_box_layout.height.clone(),
        ) {
            let raster = ((w - sub_w).max(1.0), (h - sub_h).max(1.0));
            // Элемент ГИБКОГО контейнера: коробку задаёт раскладка (рост,
            // сжатие — css-flexbox-1 §9.7), и картинка заполняет её
            // (`object-fit: fill`, css-images-3 §5.5), а не держит
            // объявленную ширину. Прежде коробка `flex: 5` росла до 122.5,
            // а картинка оставалась 10 точек (`flexbox-basic-img-horiz-001`,
            // `-vert-001`).
            Picture::Authored {
                raster,
                size: (!style.reset_flex.flex_item).then_some((w, h)),
            }
        } else if let (Some(Len::Px(_)), None | Some(Len::Auto), Some(ratio)) = (
            style.reset_box_layout.width.clone(),
            style.reset_box_layout.height.clone(),
            self.width_limit_in_layout(),
        ) {
            // §10.4: a width limit in percentages the build could not resolve
            // (the containing block's width comes from flex or grid layout)
            // clamps the named width in layout, and the height follows the
            // ratio of the clamped width (§10.6.2): the picture fills the box
            // width layout gives it, as Blink sizes the replaced content box
            // from its used width. Sized here from the unclamped width, the
            // picture kept 900×753 in a 513 column: the box clamped its width
            // only, its height grew to the picture's, and the overflow widened
            // the scrollport around it.
            Picture::FullWidth { ratio: Some(ratio) }
        } else if let (Some(Len::Px(w)), None | Some(Len::Auto)) = (
            style.reset_box_layout.width.clone(),
            style.reset_box_layout.height.clone(),
        ) {
            // Заданная ширина + auto-высота: высота из соотношения (§10.6.2),
            // без соотношения — своя, резерв 150.
            // §10.7: пределы зажимают и НАЗВАННУЮ сторону тоже. Коробку
            // `apply` уже зажал, а картинка шла как написана и вылезала за неё.
            let cw = self.limit(
                (w - sub_w).max(0.0),
                self.clamp(style.reset_box_layout.min_width.clone(), sub_w),
                max_w,
            );
            let ch = match self.ratio() {
                Some(r) if r > 0.0 => self.transfer(cw, r, true),
                _ => self
                    .natural
                    .and_then(|i| i.h)
                    .unwrap_or(DEFAULT_REPLACED_HEIGHT_PX),
            };
            // §10.4: пределы держат ВЫВЕДЕННУЮ сторону тоже — заданная
            // остаётся как написана, а высота из соотношения обязана влезть
            // в свой потолок и пол. Правка по спеке: на снимках она ничего не
            // меняет.
            let ch_free = ch;
            let ch = self.limit(
                ch,
                self.clamp(style.reset_box_layout.min_height.clone(), sub_h),
                max_h,
            );
            // §10.4: когда потолок или пол ИЗМЕНИЛИ выведенную сторону,
            // заданная пересчитывается по соотношению — коробка остаётся
            // пропорциональной, а не растягивается. `width: 200px` при
            // `max-height: 50px` у картинки 100×50 — это 100×50, а не
            // 200×50.
            let cw = match self.ratio() {
                Some(r) if r > 0.0 && (ch - ch_free).abs() > COORD_EPS => {
                    let w = self.limit(
                        self.transfer(ch, r, false),
                        self.clamp(style.reset_box_layout.min_width.clone(), sub_w),
                        max_w,
                    );
                    // Коробка ужимается ТОЛЬКО когда предел и правда изменил
                    // выведенную сторону: иначе высота у неё остаётся `auto`,
                    // и подстановка ломала замещённые без пределов вовсе
                    // (`replaced-intrinsic-004`).
                    узкая = Some((w, ch));
                    w
                }
                _ => cw,
            };
            Picture::Used {
                raster: (cw.max(1.0), ch.max(1.0)),
                size: (cw, ch),
            }
        } else if let (None | Some(Len::Auto), Some(Len::Px(h))) = (
            style.reset_box_layout.width.clone(),
            style.reset_box_layout.height.clone(),
        ) {
            // Зеркально: заданная высота + auto-ширина (§10.3.2).
            let ch = self.limit(
                (h - sub_h).max(0.0),
                self.clamp(style.reset_box_layout.min_height.clone(), sub_h),
                max_h,
            );
            let cw = match self.ratio() {
                Some(r) if r > 0.0 => self.transfer(ch, r, false),
                _ => self
                    .natural
                    .and_then(|i| i.w)
                    .unwrap_or(DEFAULT_REPLACED_WIDTH_PX),
            };
            let cw_free = cw;
            let cw = self.limit(
                cw,
                self.clamp(style.reset_box_layout.min_width.clone(), sub_w),
                max_w,
            );
            // Зеркально §10.4: изменённая ширина тянет за собой высоту.
            let ch = match self.ratio() {
                Some(r) if r > 0.0 && (cw - cw_free).abs() > COORD_EPS => {
                    let h = self.limit(
                        self.transfer(cw, r, true),
                        self.clamp(style.reset_box_layout.min_height.clone(), sub_h),
                        max_h,
                    );
                    узкая = Some((cw, h));
                    h
                }
                _ => ch,
            };
            Picture::Used {
                raster: (cw.max(1.0), ch.max(1.0)),
                size: (cw, ch),
            }
        } else if let (Some(Len::Pct(_)), None | Some(Len::Auto)) = (
            style.reset_box_layout.width.clone(),
            style.reset_box_layout.height.clone(),
        ) {
            // Доля ширины при auto-высоте: ширину даёт содержащий блок, высоту
            // — собственное соотношение сторон (§10.3.2, §10.6.2). Раньше доля
            // не попадала ни в одну ветку, и замещаемый рисовался СВОИМ
            // пикселем (`absolute-replaced-width-006`: 15×15 вместо 96×96).
            // Долю по этой оси УЖЕ поставил хозяин (`styled_div(e)` несёт
            // стиль элемента целиком), поэтому картинке остаётся заполнить
            // его: `relative(kw)` внутри давал долю ОТ ДОЛИ — `width: 50%`
            // выходило четвертью содержащего блока.
            Picture::FullWidth {
                ratio: self.ratio().filter(|r| *r > 0.0),
            }
        } else if let (None | Some(Len::Auto), Some(Len::Pct(_))) = (
            style.reset_box_layout.width.clone(),
            style.reset_box_layout.height.clone(),
        ) {
            // Зеркально: доля высоты при auto-ширине.
            Picture::FullHeight {
                ratio: self.ratio().filter(|r| *r > 0.0),
            }
        } else if !matches!(style.reset_box_layout.width, None | Some(Len::Auto))
            && !matches!(style.reset_box_layout.height, None | Some(Len::Auto))
        {
            // Обе стороны заданы, но не обе в точках: каждая ось — своим
            // значением. `width:100%; height:100%` растягивается на коробку
            // (sizing-percentages-replaced-orthogonal-001), а смешанная
            // запись `width:50%; height:15px` раньше падала в size_full и
            // ТЕРЯЛА пиксельную сторону (inline-replaced-width-011..015).
            match (
                style.reset_box_layout.width.clone(),
                style.reset_box_layout.height.clone(),
            ) {
                (Some(Len::Pct(_)), Some(Len::Px(h))) => Picture::FullWidthPxHeight(h),
                (Some(Len::Px(w)), Some(Len::Pct(_))) => Picture::PxWidthFullHeight(w),
                _ => Picture::Full,
            }
        } else if !matches!(style.reset_box_layout.width, Some(Len::Px(_)))
            && !matches!(style.reset_box_layout.height, Some(Len::Px(_)))
        {
            self.auto_picture()
        } else {
            Picture::Natural
        };
        (picture, узкая)
    }

    /// The ratio, when only layout can apply the width limits: one of them is
    /// a percentage the build left unresolved, and no height limit could
    /// change the derived height back (those are applied here, §10.4).
    fn width_limit_in_layout(&self) -> Option<f32> {
        let layout = &self.style.reset_box_layout;
        let pct = |l: Option<Len>| matches!(l, Some(Len::Pct(_)));
        let none = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
        (!self.style.reset_flex.flex_item
            && (pct(layout.max_width.clone()) || pct(layout.min_width.clone()))
            && none(layout.max_height.clone())
            && none(layout.min_height.clone()))
        .then(|| self.ratio())
        .flatten()
        .filter(|r| *r > 0.0)
    }
}
