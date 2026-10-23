//! Computed::apply_effects, хвост цепочки: container-type, content-visibility, contain-intrinsic-*,
//! mix-blend-mode, isolation, user-select. Ветви в исходном порядке после apply_effects_timing.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_effects_containment(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
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
                // (`box_predicates::establishes_cb`, `style::cascade::inherit`), а корпус
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
                self.accept_declaration();
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
                if matches!(v.trim(), "visible" | "hidden" | "auto") {
                    self.accept_declaration();
                }
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
            "contain-intrinsic-size"
            | "contain-intrinsic-width"
            | "contain-intrinsic-height"
            | "contain-intrinsic-block-size"
            | "contain-intrinsic-inline-size" => {
                let Some(values) = intrinsic_sizes(v, vcx) else {
                    return;
                };
                if key != "contain-intrinsic-size" && values.len() != 1 {
                    return;
                }
                self.accept_declaration();
                match key {
                    "contain-intrinsic-size" => {
                        self.contain_intrinsic = (values[0], *values.get(1).unwrap_or(&values[0]))
                    }
                    "contain-intrinsic-width" => self.contain_intrinsic.0 = values[0],
                    "contain-intrinsic-height" => self.contain_intrinsic.1 = values[0],
                    "contain-intrinsic-block-size" => self.logical().ci_block = values[0],
                    _ => self.logical().ci_inline = values[0],
                }
            }
            "mix-blend-mode" => {
                css_wide_reset!(self, blend);
                if v != "plus-lighter"
                    && BlendMode::parse(v)
                        .filter(|mode| *mode != BlendMode::Inherit)
                        .is_none()
                {
                    return;
                }
                // Номера совпадают с формулами в шейдере: смешивание считается
                // при сборке буфера группы, поэтому доступны все режимы CSS,
                // включая те, где цвет берётся целиком (тон, насыщенность).
                self.blend = accept_parsed!(
                    self,
                    Some(if v == "plus-lighter" {
                        16
                    } else {
                        BlendMode::parse(v)
                            .filter(|mode| *mode != BlendMode::Inherit)
                            .map_or(0, |mode| mode as u8)
                    })
                )
            }
            "isolation" => {
                self.isolate = apply_parsed!(
                    self,
                    matches!(v, "auto" | "isolate").then_some(v == "isolate")
                )
            }
            "user-select" | "-webkit-user-select" => {
                css_wide_reset!(self, user_select);
                self.user_select = apply_parsed!(self, crate::style::computed::UserSelect::parse(v))
            }
            _ => *hit = false,
        }
    }
}

fn intrinsic_sizes(v: &str, vcx: ValueCx<'_>) -> Option<Vec<Option<f32>>> {
    let words = split_outside_parens(v);
    let mut words = words.iter();
    let mut result = Vec::new();
    while let Some(mut word) = words.next() {
        if word == "auto" {
            word = words.next()?;
        }
        let size = if word == "none" {
            None
        } else {
            match Len::parse(word, vcx)? {
                Len::Px(n) if n >= 0.0 => Some(n),
                _ => return None,
            }
        };
        result.push(size);
    }
    (1..=2).contains(&result.len()).then_some(result)
}
