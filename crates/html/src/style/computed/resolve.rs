//! Доводка вычисленного стиля: логические стороны, единицы окна и шрифта, текстовые и смешанные стили.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod derived;
mod em;
mod logical;

impl Computed {
    // ПРОБОВАЛИ И ОТКАТИЛИ: блокификация под `float` и абсолютным
    // позиционированием (CSS 2.1 §9.7) — сворачивать `inline`, `inline-block`
    // и внутренние табличные виды в блок после каскада.
    //
    // Замер со всеми элементами: CSS2 4614 -> 4615 (+29/-28), oldfront
    // 2336 -> 2324. Без замещаемых (их размеры считает свой путь): CSS2
    // 4614 -> 4618, oldfront 2336 -> 2325. Потери обоих заходов — семья
    // `left-applies-to-*` и плавающие куски строки: у нас плавающий кусок
    // остаётся в общей строке текста нарочно, и блочным он рвёт соединение
    // букв (тот же корень, что у отката в `render.rs::wrap_floats`).
    //
    // Возвращаться, когда у флоатов появится своя коробка блока
    // (`bands.rs` + `BfcFlow`), а не ряд флекса.

    /// Сдвиг из `transform`, который раскладка берёт на себя как
    /// относительное смещение — тем же путём, что и свойство `translate`
    /// (`apply::apply_box`). Чистый сдвиг — это смена начала координат
    /// (css-transforms-1 §transform-rendering), и разложенная на сдвинутом
    /// месте коробка обязана рисоваться байт в байт как сдвинутая: иначе
    /// при дробном масштабе экрана округление раскладки (до сдвига) и
    /// дробный сдвиг матрицей (после) расходились на пиксель — края коробки и
    /// глифы (Blink так же проносит дробное смещение сквозь 2D-сдвиг:
    /// `PaintPropertyTreeBuilder`, subpixel accumulation). Только статичная
    /// блочная коробка — её путь отрисовки один (`render.rs`, блочная ветка
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

    /// Перевести `em` в точки по размеру шрифта.
    ///
    /// Размер шрифта известен только после каскада, поэтому длины в `em`
    /// доживают до этого места неразрешёнными. Для самого `font-size` база —
    /// РОДИТЕЛЬСКИЙ размер, для остального — свой собственный.
    /// Перевести единицы окна в точки.
    ///
    /// Размер окна известен только сборщику дерева, поэтому `vh`/`vw` доживают
    /// до него неразрешёнными — как и `em` до размера шрифта.
    pub fn resolve_viewport(&mut self, viewport: (f32, f32)) {
        let fix = |l: &mut Option<Len>| match *l {
            Some(Len::Vw(k)) => *l = Some(Len::Px(k * viewport.0)),
            Some(Len::Vh(k)) => *l = Some(Len::Px(k * viewport.1)),
            Some(Len::Calc(i)) => {
                let mut s = crate::style::values::value::calc_get(i);
                // Без слагаемых окна складывать нечего: индекс остаётся
                // (арена append-only, `resolve_viewport` идёт на каждом
                // слитом стиле), а `collapse` стёр бы процентную смесь
                // `calc(50% - 3px)` в `None` уже после разбора.
                if s.vw != 0.0 || s.vh != 0.0 {
                    s.px += s.vw * viewport.0 + s.vh * viewport.1;
                    s.vw = 0.0;
                    s.vh = 0.0;
                    *l = s.collapse_mixed();
                }
            }
            _ => {}
        };
        let sides = |s: &mut Sides| {
            for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                match *one {
                    Some(Len::Vw(k)) => *one = Some(Len::Px(k * viewport.0)),
                    Some(Len::Vh(k)) => *one = Some(Len::Px(k * viewport.1)),
                    Some(Len::Calc(i)) => {
                        let mut s = crate::style::values::value::calc_get(i);
                        // То же, что у размеров: смесь с долей доживает.
                        if s.vw != 0.0 || s.vh != 0.0 {
                            s.px += s.vw * viewport.0 + s.vh * viewport.1;
                            s.vw = 0.0;
                            s.vh = 0.0;
                            *one = s.collapse_mixed();
                        }
                    }
                    _ => {}
                }
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
            &mut self.font_size,
        ] {
            fix(l);
        }
        sides(&mut self.padding);
        sides(&mut self.margin);
        // Толщина рамки в единицах окна (`border-bottom: 50vh solid`,
        // `monolithic-overflow-021`): без перевода рамка выходила нулевой.
        sides(&mut self.border_width);
        sides(&mut self.inset);
        self.resolve_radius_lengths(fix);
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
    pub fn used_font_factor(&self, family: &str) -> f32 {
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
        let size_adjust = crate::text::fonts::size_adjust(family, slope);
        match self.font_size_adjust {
            Some((metric, want)) if want.is_finite() => {
                match crate::text::metrics::adjust_aspect(family, metric) {
                    Some(have) if have > 0.0 => want / have,
                    _ => size_adjust,
                }
            }
            _ => size_adjust,
        }
    }

    /// Тот же стиль без коробки — только то, что относится к тексту.
    ///
    /// Нужен там, где абзац разбит на куски: фон, отступы, рамка и размеры
    /// принадлежат абзацу целиком, и повторять их на каждом слове нельзя —
    /// иначе у каждого слова появляется своя подложка и своё поле.
    /// Итоговые возможности OpenType куска — в порядке старшинства
    /// css-fonts-4 §7.2: `font-variant-*` и прочие свойства, затем свойство
    /// `font-feature-settings`. Повтор тега схлопывается, побеждает
    /// последний: прежде в gpui уходило `liga 0, …, liga 1`, а
    /// `apply_font_features` дописывал после них ещё `liga 0`
    /// (`font-features-across-space-3`).
    pub fn used_features(&self) -> Vec<(String, u32)> {
        // Шаг 2 §7.2 — дескриптор правила `@font-face`, МЛАДШЕ свойств.
        let mut all: Vec<(String, u32)> =
            crate::text::fonts::face_features(self.font_family.as_deref().unwrap_or(""));
        all.extend(self.font_features.iter().cloned());
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
        if matches!(self.letter_spacing, Some(Len::Px(v) | Len::Em(v)) if v != 0.0) {
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
        out
    }
}
