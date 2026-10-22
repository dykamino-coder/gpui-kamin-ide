//! Computed::apply_background, хвост цепочки: background-repeat, origin/clip, size,
//! position(-x/-y), object-position, background-attachment. Ветви в исходном порядке после
//! box-shadow и background-image.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_background_layers(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            "background-blend-mode" => {
                if let Some(mode) = BlendMode::parse(v) {
                    self.background_blend = accept_parsed!(self, Some(mode));
                } else if matches!(v, "initial" | "unset" | "revert" | "revert-layer") {
                    self.background_blend = None;
                }
            }
            // --- Фоновая картинка --------------------------------------------
            // Запись бывает и ПОосевой: `repeat space`, `round no-repeat`.
            // Один keyword задаёт обе оси, два — свою каждой.
            "background-repeat" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::BG_REPEAT;
                    return;
                }
                let word = |w: &str| match w {
                    "no-repeat" => Some(Tiling::None),
                    "space" => Some(Tiling::Space),
                    "round" => Some(Tiling::Round),
                    "repeat" => Some(Tiling::Repeat),
                    _ => None,
                };
                if v.split_whitespace().count() > 2 {
                    return;
                }
                let mut it = v.split_whitespace();
                self.bg_repeat = apply_parsed!(
                    self,
                    match (it.next(), it.next()) {
                        (Some("repeat-x"), None) => Some(BgRepeat::RepeatX),
                        (Some("repeat-y"), None) => Some(BgRepeat::RepeatY),
                        (Some(x), Some(y)) => match (word(x), word(y)) {
                            (Some(x), Some(y)) => Some(BgRepeat::Axes(x, y)),
                            _ => None,
                        },
                        (Some(x), None) => word(x).map(|t| BgRepeat::Axes(t, t)),
                        _ => None,
                    }
                )
            }
            // Область покраски фона. `border-box` — умолчание, поэтому оно
            // же и сбрасывает признак: свойство наследуемым не является, но
            // перебить заданное ранее в том же наборе обязано.
            // Откуда отсчитывается картинка. Умолчание — внутренний край
            // рамки, и `padding-box` его же и означает.
            "background-origin" if v.trim() == "inherit" => {
                self.inherit_bits |= inh::BG_ORIGIN;
            }
            "background-clip" | "-webkit-background-clip" if v.trim() == "inherit" => {
                self.inherit_bits |= inh::BG_CLIP;
            }
            "background-size" if v.trim() == "inherit" => {
                self.inherit_bits |= inh::BG_SIZE;
            }
            "background-origin" => {
                self.bg_origin = parsed_value!(
                    self,
                    match v.trim() {
                        "border-box" => Some(Some(BgClip::BorderBox)),
                        "content-box" => Some(Some(BgClip::ContentBox)),
                        "padding-box" => Some(None),
                        _ => None,
                    }
                );
                if v.trim() == "padding-box" {
                    self.accept_declaration();
                }
            }
            "background-clip" | "-webkit-background-clip" => {
                self.bg_clip = parsed_value!(
                    self,
                    match v.trim() {
                        "padding-box" => Some(Some(BgClip::PaddingBox)),
                        "content-box" => Some(Some(BgClip::ContentBox)),
                        "text" => Some(Some(BgClip::Text)),
                        "border-area" => Some(Some(BgClip::BorderArea)),
                        "border-box" => Some(None),
                        _ => None,
                    }
                );
                if v.trim() == "border-box" {
                    self.accept_declaration();
                }
            }
            "background-size" => {
                self.bg_size = match v {
                    "cover" => accept_parsed!(self, BgSize::Cover),
                    "contain" => accept_parsed!(self, BgSize::Contain),
                    _ if self.css_wide_value => accept_parsed!(self, BgSize::default()),
                    _ => {
                        // Отрицательная длина делает декларацию невалидной
                        // целиком (css-backgrounds-3 §3.9) — размер не трогать.
                        let neg = |l: &Option<Len>| matches!(l, Some(Len::Px(v) | Len::Pct(v)) if *v < 0.0);
                        // Резка вне скобок и процентная смесь: `calc(50px +
                        // 50%) calc(100% - 30px)` — две длины, а не шесть слов;
                        // `background::len_px` в растре складывает доли с
                        // точками сам (css-values-4 §10.9).
                        let words = split_outside_parens(v);
                        if !(1..=2).contains(&words.len()) {
                            return;
                        }
                        let sizes = words
                            .iter()
                            .map(|w| {
                                size_range::parse_nonnegative_size(w, vcx).filter(|l| {
                                    !matches!(
                                        l,
                                        Len::MinContent | Len::MaxContent | Len::FitContent
                                    )
                                })
                            })
                            .collect::<Option<Vec<_>>>();
                        let sizes = parsed_value!(self, sizes);
                        let w = sizes.first().copied();
                        let h = sizes.get(1).copied();
                        if neg(&w) || neg(&h) {
                            return;
                        }
                        if v == "auto" {
                            BgSize::Auto
                        } else {
                            BgSize::Fixed(w, h)
                        }
                    }
                }
            }
            "background-position" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::BG_POS;
                    return;
                }
                css_wide_reset!(self, bg_pos, bg_position_math);
                if !position_valid(v, vcx) {
                    return;
                }
                self.set_background_position(v, vcx);
            }
            // `object-position` — та же грамматика, но для замещаемого
            // содержимого (css-images-3 §5.2).
            "object-view-box" => {
                self.object_view_box = parsed_value!(
                    self,
                    parse_view_box(v, vcx)
                        .map(Some)
                        .or_else(|| (v == "none").then_some(None))
                );
                if v == "none" {
                    self.accept_declaration();
                }
            }
            "object-position" => {
                if !position_valid(v, vcx) {
                    return;
                }
                self.object_position = accept_parsed!(self, Some(parse_pos_words(v, vcx)));
            }
            // Пооосевые продольные свойства (css-backgrounds-4 §4.1):
            // одна ось, вторая не трогается.
            "background-position-x" | "background-position-y" => {
                let val = match v {
                    "left" | "top" => Some(Len::Pct(0.0)),
                    "center" => Some(Len::Pct(0.5)),
                    "right" | "bottom" => Some(Len::Pct(1.0)),
                    other => Len::parse(other, vcx),
                };
                if val.is_none() {
                    return;
                }
                self.note_parsed(&val);
                self.set_background_position_axis(usize::from(key.ends_with("-y")), v, val, vcx);
            }
            "background-attachment" => {
                // `fixed` привязывает плитку к ОБЛАСТИ ПРОСМОТРА: считается
                // она от окна, а красится всё равно только внутри коробки
                // (css-backgrounds-3 §3.10).
                self.bg_fixed = apply_parsed!(
                    self,
                    matches!(v, "fixed" | "scroll" | "local").then_some(v == "fixed")
                );
            }
            _ => *hit = false,
        }
    }
}

pub(in crate::style::computed::props) fn position_valid(v: &str, vcx: ValueCx<'_>) -> bool {
    let words = split_outside_parens(v);
    (1..=4).contains(&words.len())
        && words.iter().all(|w| {
            matches!(w.as_str(), "left" | "right" | "top" | "bottom" | "center")
                || (Computed::background_position_length(w, vcx)
                    && !matches!(
                        w.as_str(),
                        "auto" | "min-content" | "max-content" | "fit-content"
                    ))
                || Len::parse_mixed(w, vcx).is_some_and(|l| {
                    !matches!(
                        l,
                        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
                    )
                })
        })
}
