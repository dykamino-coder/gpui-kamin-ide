//! Computed::apply_one: filter, blend, isolation, animation, transition, will-change, contain*, content, counters, quotes, lists, svg, cursor and the rest.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};
mod containment;
mod timing;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_effects(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // `pre` сохраняет переводы строк — это не то же самое, что запрет
            // переноса: раньше `pre` помечался как `nowrap`, и текст склеивался
            // в одну строку.
            "cursor" => self.cursor = Some(v.to_string()),
            // Заливка SVG-геометрии: свойство презентации доезжает до
            // разметки при растеризации (SVG 2 §presentation attributes).
            "fill" => self.svg_fill = Some(v.to_string()),
            // Обводка — то же семейство. Значение уходит в разметку как есть:
            // разбирать цвет здесь незачем, его знает usvg.
            "stroke" => self.svg_stroke = Some(v.to_string()),
            // Вычисленное `stroke-width` — «the absolute length, or percentage»
            // (fill-stroke-3 §stroke-width): `calc()` из точек сворачивается
            // здесь. usvg его не понимает и рисовал толщину по умолчанию 1
            // (`zoom/stroke`: эталон `calc(6px * var(--scale))`). В Blink —
            // `UnzoomedLength` через `ConvertUnzoomedLength`
            // (css_properties.json5:6150-6159).
            "stroke-width" => {
                self.svg_stroke_width = Some(match crate::style::values::value::calc_pct_px(v) {
                    Some((pct, px)) if pct == 0.0 => format!("{px}"),
                    _ => v.to_string(),
                });
            }
            // `x`/`y` — геометрические СВОЙСТВА фигуры (SVG 2 §Geometry).
            // У HTML-коробки таких свойств нет, поэтому имена свободны, а в
            // разметку они уходят только внутри SVG-поддерева (гейт в svg.rs).
            "x" => self.svg_x = crate::style::values::value::Len::parse(v),
            "y" => self.svg_y = crate::style::values::value::Len::parse(v),
            "caption-side" => self.caption_bottom = Some(v.eq_ignore_ascii_case("bottom")),
            "list-style-position" => {
                self.list_style_inside = Some(v.trim() == "inside");
            }
            "list-style" | "list-style-type" => list_style::apply(self, key, v),
            "backdrop-filter" => {
                // Тот же `<filter-value-list>`, что у `filter`
                // (filter-effects-2 §BackdropFilterProperty): разбор общий,
                // размытие идёт своим проходом, цветовые функции — матрицей.
                let mut tmp = Self::default();
                tmp.apply_one("filter", v);
                let f = tmp.filter.unwrap_or_else(Filter::neutral);
                self.backdrop_blur = (f.blur > 0.0).then_some(f.blur);
                self.backdrop_color = f.color_matrix().map(|_| Filter { blur: 0.0, ..f });
                // Корень подложки — любое значение, кроме `none`
                // (filter-effects-2 Overview.bs:119; Blink
                // paint_property_tree_builder.cc:1846): тождественная
                // `invert(0)` матрицы не даёт, но корнем остаётся
                // (`backdrop-filter-backdrop-root-backdrop-filter`).
                self.backdrop_filter_set = !v.trim().eq_ignore_ascii_case("none");
                // `url(#id)` — SVG `<filter>` (`render::svg_filter_matrix`).
                self.backdrop_ref = tmp.filter_ref;
            }
            // `view-transition-name` не `none` — корень подложки
            // (css-view-transitions-1 Overview.bs:577-582 «Form a backdrop
            // root»; Blink paint_property_tree_builder.cc:1858-1862
            // `NeedsEffectForViewTransition`).
            "view-transition-name" => {
                self.vt_name = !v.trim().eq_ignore_ascii_case("none");
            }
            // css-will-change-1 §2.1: обещанное свойство даёт коробке то, что
            // дало бы его неначальное значение, — содержащий блок для
            // `absolute`/`fixed` и контекст наложения (`will-change-fixpos-cb-*`,
            // `-abspos-cb-*`, `-fixedpos-cb-*`, `-stacking-context-z-index-2/3`).
            // `position` даёт блок только абсолютам (`-fixpos-cb-position-1`).
            // `auto`, `scroll-position`, `contents` и прочие свойства — ноль:
            // `will-change: height` не меняет ничего (`-fixpos-cb-height-1`).
            // Здесь же признак корня подложки (filter-effects-2
            // Overview.bs:122: «will-change specifying any property that
            // would create a Backdrop Root on non-initial value»). Арма ОДНА:
            // вторая с тем же ключом в этом `match` недостижима — так с
            // b47ecf2 разряды `wc::*` не ставились вовсе.
            "will-change" => {
                let mut bits = 0u8;
                let mut root = false;
                for part in v.split(',') {
                    let name = part.trim().to_ascii_lowercase();
                    root |= matches!(
                        name.as_str(),
                        "opacity"
                            | "filter"
                            | "mask"
                            | "mask-image"
                            | "-webkit-mask"
                            | "-webkit-mask-image"
                            | "mask-border"
                            | "clip-path"
                            | "-webkit-clip-path"
                            | "backdrop-filter"
                            | "-webkit-backdrop-filter"
                            | "mix-blend-mode"
                            | "view-transition-name"
                    );
                    bits |= match name.as_str() {
                        "transform"
                        | "translate"
                        | "rotate"
                        | "scale"
                        | "perspective"
                        | "-webkit-perspective"
                        | "transform-style"
                        | "offset-path"
                        | "contain" => wc::BOX,
                        "filter" | "backdrop-filter" | "-webkit-backdrop-filter" => {
                            wc::CB_ABS | wc::CB_FIXED | wc::STACK
                        }
                        "position" => wc::CB_ABS | wc::STACK,
                        "opacity"
                        | "isolation"
                        | "mix-blend-mode"
                        | "clip-path"
                        | "-webkit-clip-path"
                        | "mask"
                        | "mask-image"
                        | "-webkit-mask"
                        | "-webkit-mask-image"
                        | "mask-border"
                        | "view-transition-name" => wc::STACK,
                        "z-index" => wc::STACK_Z,
                        _ => 0,
                    };
                }
                self.will_change = bits;
                self.will_change_root = root;
            }
            // Свисающая пунктуация: знак выходит ЗА край коробки, чтобы край
            // текста читался ровным. Значения складываются: `first last`.
            // `zoom` (css-viewport-1 §zoom-property): число или доля, ноль
            // читается единицей («A 0 value is treated as if it was 1»),
            // отрицательное недействительно. `normal`/`reset` — старые слова
            // IE/WebKit, равны единице. Здесь ТОЛЬКО запись поля: применяет
            // его проход `zoom::resolve`, слияние стилей поля не читает.
            "zoom" => {
                let k = match v {
                    "normal" | "reset" => Some(1.0),
                    _ => match v.strip_suffix('%') {
                        Some(p) => p.trim().parse::<f32>().ok().map(|p| p / 100.0),
                        None => v.parse::<f32>().ok(),
                    },
                };
                if let Some(k) = k.filter(|k| k.is_finite() && *k >= 0.0) {
                    self.zoom = Some(if k == 0.0 { 1.0 } else { k });
                }
            }

            // --- Прочее ------------------------------------------------------
            "pointer-events" => self.pointer_events_none = Some(v == "none"),
            "table-layout" => self.table_fixed = Some(v == "fixed"),

            // --- Псевдоэлементы и шрифт ---------------------------------------
            "counter-reset" | "counter-increment" | "counter-set" => counters::apply(self, key, v),
            "quotes" => quotes::apply(self, v),
            "content" => {
                match v {
                    // ПУСТАЯ строка — не то же самое, что `none`: коробка
                    // псевдоэлемента создаётся, просто в ней нет знаков. На
                    // этом стоит целый приём эталонов WPT — `::after` с
                    // `content: ""` и `inset: 0` накрывает красное зелёным
                    // (`overflow-wrap-anywhere-001` и родня).
                    "none" | "normal" => {
                        self.content = None;
                        self.content_none = Some(v == "none");
                    }
                    // Негодная запись НЕ применяется вовсе, прежнее значение
                    // остаётся (CSS 2.1 §4.1.8): иначе мусор вроде
                    // `counter(a,b,c)` печатался литералом и `counters-002`
                    // показывал слово FAIL.
                    other => {
                        if let Some(list) = parse_content(other) {
                            self.content = Some(list);
                        }
                    }
                }
            }
            "accent-color" => self.accent_color = Color::parse(v),

            _ => self.apply_effects_timing(key, val, v, hit),
        }
    }
}
