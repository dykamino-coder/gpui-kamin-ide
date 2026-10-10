//! Computed::apply_effects, хвост цепочки: container-type, content-visibility, contain-intrinsic-*, mix-blend-mode, isolation, user-select. Ветви в исходном порядке после apply_effects_timing.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_effects_containment(
        &mut self,
        key: &str,
        val: &str,
        v: &str,
        hit: &mut bool,
    ) {
        match key {
            "container-type" => {
                // css-conditional-5 §container-type:
                // `normal | [ [ size | inline-size ] || scroll-state ]`.
                // `size` — «Applies style containment and size containment to
                // the principal box»; `inline-size` — то же, но обособление
                // одной строчной оси. Обособления РАСКЛАДКИ в этом списке НЕТ,
                // и ставить его нельзя: у нас `contain_layout` делает элемент
                // содержащим блоком для `absolute` и `fixed`
                // (`inline::establishes_cb`, `inline::inherit`), а корпус
                // требует обратного — `no-layout-containment-abspos`,
                // `-fixedpos`, `-baseline` (все 0.00) проверяют, что абсолют,
                // `fixed` и базовая линия проходят СКВОЗЬ контейнер.
                //
                // Правило `@container` этим шагом ещё не разбирается: здесь
                // только побочное действие свойства. Оно само по себе отвечает
                // за `contain-size-014` (коробка с `container-type: size`
                // обязана мериться пустой, а росла по `<img height=200>`) и
                // делает истинным `@supports (container-type: …)`, на котором
                // висят `chrome-legacy-skip-recalc` и обе
                // `svg-*-no-size-container`.
                let mut size = false;
                let mut inline = false;
                let mut known = false;
                for w in v.split_whitespace() {
                    match w {
                        "size" => {
                            size = true;
                            known = true;
                        }
                        "inline-size" => {
                            inline = true;
                            known = true;
                        }
                        // `scroll-state` — контейнер по состоянию прокрутки, к
                        // размеру отношения не имеет; `normal` — начальное
                        // значение. Оба грамматически годны и не делают ничего.
                        "scroll-state" | "normal" => known = true,
                        // Слово вне грамматики (например `anchored` из
                        // css-anchor-position-2) делает объявление негодным
                        // ЦЕЛИКОМ (CSS 2.1 §4.1.7), а не «частично годным»:
                        // `anchored-fallback-style-containment` (0.01) обязана
                        // остаться нетронутой.
                        _ => return,
                    }
                }
                if !known {
                    return;
                }
                // Только ВЗВОД: `container-type: normal` не имеет права снять
                // обособление, объявленное в том же блоке через `contain`, —
                // это разные свойства, и начальное значение одного ничего не
                // отменяет у другого.
                if size {
                    self.contain_size = Some(true);
                }
                if inline {
                    self.contain_inline_size = Some(true);
                }
                if size || inline {
                    self.contain_style = Some(true);
                    // css-conditional-5 §container-type: `size`/`inline-size`
                    // делают элемент контейнером запросов размера. Такой
                    // элемент обособлен, и css-grid-2 §subgrid-listing лишает
                    // его подсеточности. Признак ОТДЕЛЬНЫЙ от `contain_size`,
                    // потому что `contain: size` подсетку не отменяет; и
                    // отдельный от `contain_layout`, который у нас делает
                    // элемент содержащим блоком для абсолюта, а корпус требует
                    // обратного (`no-layout-containment-abspos` и родня).
                    self.container_size_query = true;
                }
            }
            "content-visibility" => {
                // `hidden` = size+layout+paint containment, содержимое
                // пропускается целиком (css-contain-2 §4). `auto` для
                // reftest без прокрутки всегда «релевантен» = visible.
                if v.trim() == "hidden" {
                    self.contain_size = Some(true);
                    self.contain_paint = Some(true);
                    self.contain_layout = Some(true);
                    self.skip_content = Some(true);
                }
            }
            "contain-intrinsic-size" => {
                // Одно или два значения; `auto <длина>` — длина как запас.
                let nums: Vec<f32> = v
                    .split_whitespace()
                    .filter(|w| *w != "auto")
                    .filter_map(|w| match Len::parse(w) {
                        Some(Len::Px(px)) => Some(px),
                        _ => None,
                    })
                    .collect();
                self.contain_intrinsic = match nums.as_slice() {
                    [one] => (Some(*one), Some(*one)),
                    [w, h, ..] => (Some(*w), Some(*h)),
                    _ => (None, None),
                };
            }
            "contain-intrinsic-width" => {
                if let Some(Len::Px(w)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.contain_intrinsic.0 = Some(w);
                }
            }
            "contain-intrinsic-height" => {
                if let Some(Len::Px(h)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.contain_intrinsic.1 = Some(h);
                }
            }
            "contain-intrinsic-block-size" => {
                if let Some(Len::Px(h)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.logical().ci_block = Some(h);
                }
            }
            "contain-intrinsic-inline-size" => {
                if let Some(Len::Px(w)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.logical().ci_inline = Some(w);
                }
            }
            "mix-blend-mode" => {
                // Номера совпадают с формулами в шейдере: смешивание считается
                // при сборке буфера группы, поэтому доступны все режимы CSS,
                // включая те, где цвет берётся целиком (тон, насыщенность).
                self.blend = match v {
                    "multiply" => Some(1),
                    "screen" => Some(2),
                    "darken" => Some(3),
                    "lighten" => Some(4),
                    "overlay" => Some(5),
                    "color-dodge" => Some(6),
                    "color-burn" => Some(7),
                    "hard-light" => Some(8),
                    "soft-light" => Some(9),
                    "difference" => Some(10),
                    "exclusion" => Some(11),
                    "hue" => Some(12),
                    "saturation" => Some(13),
                    "color" => Some(14),
                    "luminosity" => Some(15),
                    _ => Some(0),
                }
            }
            "isolation" => self.isolate = Some(v == "isolate"),
            "user-select" | "-webkit-user-select" => self.no_select = Some(matches!(v, "none")),
            _ => *hit = false,
        }
    }
}
