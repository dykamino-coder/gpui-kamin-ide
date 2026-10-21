//! Группа свойств `Computed::apply_one`: эффекты и всё остальное — `filter`,
//! `backdrop-filter`, смешение, `isolation`, `animation`, `transition`,
//! `will-change`, `contain*`, `zoom`, `content`, счётчики, `quotes`, списки,
//! свойства презентации SVG (`fill`, `stroke`, `x`/`y`), `cursor`,
//! `appearance`, `view-transition-name`. Замыкает цепочку групп; таймлайны
//! и ограничение — в подмодулях `timing`, `containment`.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::{Color, Len};
mod containment;
mod filter;
mod timing;
mod values;

impl Computed {
    pub(crate) fn apply_effects(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        if self.apply_paint_value(key, v, vcx) {
            return;
        }
        match key {
            // `x`/`y` — геометрические СВОЙСТВА фигуры (SVG 2 §Geometry).
            // У HTML-коробки таких свойств нет, поэтому имена свободны, а в
            // разметку они уходят только внутри SVG-поддерева (гейт в svg.rs).
            "x" => {
                self.svg_x = apply_parsed!(self, crate::style::values::value::Len::parse(v, vcx))
            }
            "y" => {
                self.svg_y = apply_parsed!(self, crate::style::values::value::Len::parse(v, vcx))
            }
            "cx" | "cy" | "r" | "rx" | "ry" | "d" | "path-length" => {
                crate::svg::geometry_property(self, key, v, vcx);
            }
            "caption-side" => {
                self.caption_bottom =
                    apply_parsed!(self, matches!(v, "top" | "bottom").then_some(v == "bottom"))
            }
            "list-style-position" => {
                self.list_style_inside = apply_parsed!(
                    self,
                    matches!(v, "inside" | "outside").then_some(v == "inside")
                );
            }
            "list-style" | "list-style-type" => list_style::apply(self, key, v),
            "backdrop-filter" => {
                // Тот же `<filter-value-list>`, что у `filter`
                // (filter-effects-2 §BackdropFilterProperty): разбор общий,
                // размытие идёт своим проходом, цветовые функции — матрицей.
                let Some(parsed) = apply_parsed!(self, filter::ParsedFilter::parse(v, None, vcx))
                else {
                    self.backdrop_blur = None;
                    self.backdrop_color = None;
                    self.backdrop_filter_set = false;
                    self.backdrop_ref = None;
                    return;
                };
                let f = parsed.program.filter.unwrap_or_else(Filter::neutral);
                self.backdrop_blur = (f.blur > 0.0).then_some(f.blur);
                self.backdrop_color = f.color_matrix().map(|_| Filter { blur: 0.0, ..f });
                // Корень подложки — любое значение, кроме `none`
                // (filter-effects-2 Overview.bs:119; Blink
                // paint_property_tree_builder.cc:1846): тождественная
                // `invert(0)` матрицы не даёт, но корнем остаётся
                // (`backdrop-filter-backdrop-root-backdrop-filter`).
                self.backdrop_filter_set = !v.trim().eq_ignore_ascii_case("none");
                // `url(#id)` — SVG `<filter>` (`render::svg_filter_matrix`).
                self.backdrop_ref = parsed.reference;
            }
            // `view-transition-name` не `none` — корень подложки
            // (css-view-transitions-1 Overview.bs:577-582 «Form a backdrop
            // root»; Blink paint_property_tree_builder.cc:1858-1862
            // `NeedsEffectForViewTransition`).
            "view-transition-name" => {
                if !crate::style::css::selector_tokens::ident(v) {
                    return;
                }
                self.vt_name = accept_parsed!(self, !v.trim().eq_ignore_ascii_case("none"));
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
            // вторая с тем же ключом в этом `match` недостижима — так
            // разряды `wc::*` однажды не ставились вовсе.
            "will-change" => {
                let mut bits = 0u8;
                let mut root = false;
                for part in v.split(',') {
                    let name = part.trim().to_ascii_lowercase();
                    if !crate::style::css::selector_tokens::ident(&name) {
                        return;
                    }
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
                self.will_change = accept_parsed!(self, bits);
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
                    self.zoom = accept_parsed!(self, Some(if k == 0.0 { 1.0 } else { k }));
                }
            }

            // --- Прочее ------------------------------------------------------
            "pointer-events" => {
                self.pointer_events_none = apply_parsed!(
                    self,
                    matches!(
                        v,
                        "auto"
                            | "none"
                            | "visiblePainted"
                            | "visibleFill"
                            | "visibleStroke"
                            | "visible"
                            | "painted"
                            | "fill"
                            | "stroke"
                            | "all"
                            | "bounding-box"
                    )
                    .then_some(v == "none")
                )
            }
            "table-layout" => {
                self.table_fixed =
                    apply_parsed!(self, matches!(v, "auto" | "fixed").then_some(v == "fixed"))
            }

            // --- Псевдоэлементы и шрифт ---------------------------------------
            "counter-reset" | "counter-increment" | "counter-set" => {
                if v != "inherit" {
                    match key {
                        "counter-reset" => css_wide_reset!(self, self.counter_reset),
                        "counter-increment" => css_wide_reset!(self, self.counter_increment),
                        _ => css_wide_reset!(self, self.counter_set),
                    }
                }
                if !values::counter_value(v, key == "counter-reset") {
                    return;
                }
                counters::apply(self, key, v, vcx)
            }
            "quotes" => quotes::apply(self, v),
            "content" => {
                match v {
                    "inherit" => self.content_inherit = true,
                    // ПУСТАЯ строка — не то же самое, что `none`: коробка
                    // псевдоэлемента создаётся, просто в ней нет знаков. На
                    // этом стоит целый приём эталонов WPT — `::after` с
                    // `content: ""` и `inset: 0` накрывает красное зелёным
                    // (`overflow-wrap-anywhere-001` и родня).
                    "none" | "normal" => {
                        self.accept_declaration();
                        self.content_inherit = false;
                        self.content = None;
                        self.content_none = Some(v == "none");
                    }
                    // Негодная запись НЕ применяется вовсе, прежнее значение
                    // остаётся (CSS 2.1 §4.1.8): иначе мусор вроде
                    // `counter(a,b,c)` печатался литералом и `counters-002`
                    // показывал слово FAIL.
                    other => {
                        if let Some(list) = parse_content(other, vcx) {
                            self.accept_declaration();
                            self.content_inherit = false;
                            self.content = Some(list);
                        }
                    }
                }
            }
            "accent-color" => {
                self.accent_color = parsed_value!(
                    self,
                    Color::parse(v, vcx)
                        .map(Some)
                        .or_else(|| (v == "auto").then_some(None))
                )
            }
            "appearance" | "-webkit-appearance" => {
                if let Some(a) = crate::style::computed::Appearance::parse(v) {
                    self.appearance = accept_parsed!(self, Some(a));
                }
            }

            _ => self.apply_effects_timing(key, v, hit, vcx),
        }
    }
}
