//! Доводка вычисленного стиля: логические стороны, единицы окна и шрифта, текстовые и смешанные
//! стили.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod container_units;
mod decor_inset;
mod derived;
mod em;
mod font_size;
mod len_cx;
mod len_px;
pub use len_cx::{LenCx, ViewportCx};
pub use len_px::FontCx;
pub(super) mod image_lengths;
mod logical;
#[cfg(test)]
mod mask_tests;

impl Computed {
    /// Сдвиг из `transform`, который раскладка берёт на себя как
    /// относительное смещение — тем же путём, что и свойство `translate`
    /// (`apply::apply_box`). Чистый сдвиг — это смена начала координат
    /// (css-transforms-1 §transform-rendering), и разложенная на сдвинутом
    /// месте коробка обязана рисоваться байт в байт как сдвинутая: иначе
    /// при дробном масштабе экрана округление раскладки (до сдвига) и
    /// дробный сдвиг матрицей (после) расходились на пиксель — края коробки и
    /// глифы (Blink так же проносит дробное смещение сквозь 2D-сдвиг:
    /// `PaintPropertyTreeBuilder`, subpixel accumulation). Только статичная
    /// блочная коробка — её путь отрисовки один (сборка `render`, блочная ветка
    /// `transformed(animated(e))`), и края у неё не заданы.
    pub fn folded_shift(&self) -> Option<(f32, f32)> {
        use crate::style::computed::inh;
        if !matches!(self.position, None | Some(Position::Static))
            || self.hoisted_block
            || self.rotate_prop.is_some()
            || self.scale_prop.is_some()
            || self.animation.is_some()
            || self.inherit_bits & inh::TRANSFORM != 0
            || !self.plain_block_box
            // A fragmented box is shifted per fragment, but the column
            // layout places only its first fragment by the relative offset
            // (css-break-3 §box-splitting; `css-break/transform-000`): keep
            // the shift in the transform there.
            || self.in_multicol
        {
            return None;
        }
        if let Some((x, y)) = self.translate
            && !matches!((x, y), (Len::Px(_), Len::Px(_)))
        {
            return None;
        }
        self.transform
            .as_ref()?
            .pure_px_shift()
            .filter(|&(x, y)| x != 0.0 || y != 0.0)
    }

    /// Перевести единицы окна в точки.
    ///
    /// Размер окна известен только сборщику дерева, поэтому `vh`/`vw` доживают
    /// до него неразрешёнными — как и `em` до размера шрифта.
    pub fn resolve_viewport(&mut self, viewport: (f32, f32), vcx: ValueCx<'_>) {
        self.resolve_decor_inset_viewport(viewport, vcx);
        let vp = self.viewport_cx(viewport);
        let fix = |l: &mut Option<Len>| {
            if let Some(v) = *l {
                *l = Some(v.fold_viewport(vp, vcx));
            }
        };
        let sides = |s: &mut Sides| {
            for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                fix(one);
            }
        };
        for l in [
            &mut self.width,
            &mut self.height,
            &mut self.min_width,
            &mut self.min_height,
            &mut self.max_width,
            &mut self.max_height,
            &mut self.flex_basis,
            &mut self.svg_path_length,
        ] {
            fix(l);
        }
        // Кегль — в общей группе наследуемых: правится, только если он в
        // единицах окна.
        if let Some(size) = self.font_size {
            let folded = size.fold_viewport(vp, vcx);
            if folded != size {
                self.font_size = Some(folded);
            }
        }
        sides(&mut self.padding);
        sides(&mut self.snap.padding);
        sides(&mut self.snap.margin);
        sides(&mut self.margin);
        // Толщина рамки в единицах окна (`border-bottom: 50vh solid`,
        // `monolithic-overflow-021`): без перевода рамка выходила нулевой.
        sides(&mut self.border_width);
        sides(&mut self.inset);
        self.resolve_radius_lengths(fix);
        for gradient in [self.gradient.as_mut(), self.gradient_current.as_mut()]
            .into_iter()
            .flatten()
        {
            gradient.resolve_lengths(fix, vcx);
        }
        self.bg_lists.resolve_lengths(&fix, vcx);
        for image in self.image_operands() {
            image.resolve_lengths(&fix, vcx);
        }
        if let Some((row, col)) = self.gap.as_mut() {
            fix(row);
            fix(col);
        }
    }

    /// Во сколько раз ИСПОЛЬЗУЕМЫЙ кегль отличается от вычисленного.
    ///
    /// css-fonts-5 §font-size-adjust: `u = (m / m′) s` — желаемая доля `m`,
    /// делённая на ту же метрику шрифта `m′`. `none` и `from-font` (метрика
    /// своего же первого доступного шрифта, отношение ровно 1) кегль не
    /// трогают. Метрику, которую не удалось снять, спека велит не подгонять.
    pub fn used_font_factor(&self, family: &str, vcx: crate::document::state::ValueCx<'_>) -> f32 {
        // Дескриптор `size-adjust` масштабирует ВСЕ метрики лица; поверх него
        // `font-size-adjust` сводит метрику к заданной доле, и множитель
        // дескриптора сокращается: `m / (m′·k) · k = m / m′`
        // (`size-adjust-02/03`). `from-font` — метрика того же лица, то есть
        // остаётся один дескриптор.
        // Множитель — свойство лица, лицо выбирает наклон запроса.
        let slope = if self.oblique == Some(true) {
            2
        } else if self.italic == Some(true) {
            1
        } else {
            0
        };
        let size_adjust = crate::text::fonts::matching::select(family, self, vcx).map_or_else(
            || crate::text::fonts::size_adjust(family, slope, vcx),
            |f| f.adjust,
        );
        match self.font_size_adjust {
            Some((metric, want)) if want.is_finite() => {
                match crate::text::metrics::adjust_aspect(family, metric, vcx) {
                    Some(have) if have > 0.0 => want / have,
                    _ => size_adjust,
                }
            }
            _ => size_adjust,
        }
    }

    /// Итоговые возможности OpenType куска — в порядке старшинства
    /// css-fonts-4 §7.2: `font-variant-*` и прочие свойства, затем свойство
    /// `font-feature-settings`. Повтор тега схлопывается, побеждает
    /// последний: прежде в gpui уходило `liga 0, …, liga 1`, а
    /// `apply_font_features` дописывал после них ещё `liga 0`
    /// (`font-features-across-space-3`).
    pub fn used_features(&self, vcx: crate::document::state::ValueCx<'_>) -> Vec<(String, u32)> {
        // Шаг 2 §7.2 — дескриптор правила `@font-face`, МЛАДШЕ свойств.
        let mut all: Vec<(String, u32)> =
            crate::text::fonts::face_features(self.font_family.as_deref().unwrap_or(""), self, vcx);
        // Подбор веса грани по CSS (css-fonts-4 §5.2 шаг 4), а не по
        // расстоянию DirectWrite: свой тег, его читает подбор грани в gpui.
        let (tag, on) = crate::text::fonts::CSS_WEIGHT_TAG;
        all.insert(0, (tag.to_string(), on));
        all.extend(self.font_features.iter().cloned());
        // Запрет подмены начертания (`font-synthesis-weight/-style: none`,
        // css-fonts-4 §font-synthesis): свои теги `nsyw`/`nsys`, их читает
        // подбор грани в gpui.
        for (tag, allowed) in [("nsyw", self.font_synth.0), ("nsys", self.font_synth.1)] {
            if allowed == Some(false) {
                all.push((tag.to_string(), 1));
            }
        }
        all.extend(crate::text::fonts::alternates::resolve(
            self.font_family.as_deref().unwrap_or(""),
            self.font_alternates.as_ref(),
        ));
        self.add_kerning_feature(&mut all);
        // Шаг 4 §7.2: «setting a non-default value for the letter-spacing
        // property disables optional ligatures» (css-text-3 §8.2). Старше
        // `font-variant-ligatures`, младше `font-feature-settings`
        // (`font-feature-resolution-001/002`: `fvl-1 ls-1` — без лигатуры,
        // `ls-1 ffs-1` — с ней).
        if matches!(self.letter_spacing, Some(Len::Px(v) | Len::Em(v)) if v != 0.0)
            || (self.justify_chars == Some(2)
                && (self.text_align == Some(TextAlign::Justify)
                    || self.text_align_last == Some(Some(TextAlign::Justify))))
        {
            for tag in ["liga", "clig", "dlig", "hlig"] {
                all.push((tag.to_string(), 0));
            }
        }
        if let Some(settings) = &self.font_settings {
            all.extend(settings.iter().cloned());
        }
        let mut out: Vec<(String, u32)> = Vec::with_capacity(all.len());
        for (tag, value) in all {
            out.retain(|(t, _)| *t != tag);
            out.push((tag, value));
        }
        let slope = if self.oblique == Some(true) {
            2
        } else if self.italic == Some(true) {
            1
        } else {
            0
        };
        out.extend(crate::text::fonts::metric_overrides::features(
            self.font_family.as_deref().unwrap_or(""),
            slope,
            vcx,
        ));
        out
    }
}
