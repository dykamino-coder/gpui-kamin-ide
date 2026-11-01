//! Доводка вычисленного стиля: логические стороны, единицы окна и шрифта, текстовые и смешанные
//! стили.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};
use crate::values::ValueCx;

mod container_lengths;
mod container_units;
mod decor_inset;
mod derived;
mod em;
mod font_size;
mod glyph_units;
mod len_cx;
mod len_px;
mod unit_lengths;
mod used_font;
mod viewport_lengths;
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
        if !matches!(self.reset_positioned.position, None | Some(Position::Static))
            || self.reset_context.hoisted_block
            || self.reset_transforms.rotate_prop.is_some()
            || self.reset_transforms.scale_prop.is_some()
            || self.reset_context.animation.is_some()
            || self.reset_cascade.inherit_bits & inh::TRANSFORM != 0
            || !self.reset_context.plain_block_box
            // A fragmented box is shifted per fragment, but the column
            // layout places only its first fragment by the relative offset
            // (css-break-3 §box-splitting; `css-break/transform-000`): keep
            // the shift in the transform there.
            || self.reset_fragmentation.in_multicol
        {
            return None;
        }
        if let Some((x, y)) = self.reset_transforms.translate
            && !matches!((x, y), (Len::Px(_), Len::Px(_)))
        {
            return None;
        }
        self.reset_transforms
            .transform
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
        self.resolve_unit_lengths(fix, vcx);
    }

    /// Итоговые возможности OpenType куска — в порядке старшинства
    /// css-fonts-4 §7.2: `font-variant-*` и прочие свойства, затем свойство
    /// `font-feature-settings`. Повтор тега схлопывается, побеждает
    /// последний: прежде в gpui уходило `liga 0, …, liga 1`, а
    /// `apply_font_features` дописывал после них ещё `liga 0`
    /// (`font-features-across-space-3`).
    pub fn used_features(&self, vcx: crate::values::ValueCx<'_>) -> Vec<(String, u32)> {
        // Шаг 2 §7.2 — дескриптор правила `@font-face`, МЛАДШЕ свойств.
        let mut all: Vec<(String, u32)> = vcx.doc.font_snapshot().face_features(
            self.inh.font.font_family.as_deref().unwrap_or(""),
            self.font_request(self.inh.font.font_family.as_deref().unwrap_or(""), 0.0),
        );
        // Подбор веса грани по CSS (css-fonts-4 §5.2 шаг 4), а не по
        // расстоянию DirectWrite: свой тег, его читает подбор грани в gpui.
        let (tag, on) = kamin_font_facts::font_facts::CSS_WEIGHT_TAG;
        all.insert(0, (tag.to_string(), on));
        all.extend(self.inh.font.font_features.iter().cloned());
        // Запрет подмены начертания (`font-synthesis-weight/-style: none`,
        // css-fonts-4 §font-synthesis): свои теги `nsyw`/`nsys`, их читает
        // подбор грани в gpui.
        for (tag, allowed) in [
            ("nsyw", self.inh.font.font_synth.0),
            ("nsys", self.inh.font.font_synth.1),
        ] {
            if allowed == Some(false) {
                all.push((tag.to_string(), 1));
            }
        }
        all.extend(crate::style::computed::font_alternates::resolve(
            self.inh.font.font_family.as_deref().unwrap_or(""),
            self.inh.font.font_alternates.as_ref(),
        ));
        self.add_kerning_feature(&mut all);
        // Шаг 4 §7.2: «setting a non-default value for the letter-spacing
        // property disables optional ligatures» (css-text-3 §8.2). Старше
        // `font-variant-ligatures`, младше `font-feature-settings`
        // (`font-feature-resolution-001/002`: `fvl-1 ls-1` — без лигатуры,
        // `ls-1 ffs-1` — с ней).
        if matches!(self.inh.text_flow.letter_spacing, Some(Len::Px(v) | Len::Em(v)) if v != 0.0)
            || (self.inh.text_flow.justify_chars == Some(2)
                && (self.inh.text_flow.text_align == Some(TextAlign::Justify)
                    || self.inh.text_flow.text_align_last == Some(Some(TextAlign::Justify))))
        {
            for tag in ["liga", "clig", "dlig", "hlig"] {
                all.push((tag.to_string(), 0));
            }
        }
        if let Some(settings) = &self.inh.font.font_settings {
            all.extend(settings.iter().cloned());
        }
        let mut out: Vec<(String, u32)> = Vec::with_capacity(all.len());
        for (tag, value) in all {
            out.retain(|(t, _)| *t != tag);
            out.push((tag, value));
        }
        let slope = if self.inh.font.oblique == Some(true) {
            2
        } else if self.inh.font.italic == Some(true) {
            1
        } else {
            0
        };
        out.extend(
            vcx.doc
                .font_snapshot()
                .metric_features(self.inh.font.font_family.as_deref().unwrap_or(""), slope),
        );
        out
    }
}
