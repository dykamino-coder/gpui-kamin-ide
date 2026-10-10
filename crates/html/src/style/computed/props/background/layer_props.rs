//! Computed::apply_background, хвост цепочки: background-repeat, origin/clip, size, position(-x/-y), object-position, background-attachment. Ветви в исходном порядке после box-shadow и background-image.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_background_layers(
        &mut self,
        key: &str,
        val: &str,
        v: &str,
        hit: &mut bool,
    ) {
        match key {
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
                let mut it = v.split_whitespace();
                self.bg_repeat = match (it.next(), it.next()) {
                    (Some("repeat-x"), None) => Some(BgRepeat::RepeatX),
                    (Some("repeat-y"), None) => Some(BgRepeat::RepeatY),
                    (Some(x), Some(y)) => match (word(x), word(y)) {
                        (Some(x), Some(y)) => Some(BgRepeat::Axes(x, y)),
                        _ => None,
                    },
                    (Some(x), None) => word(x).map(|t| BgRepeat::Axes(t, t)),
                    _ => None,
                }
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
                self.bg_origin = match v.trim() {
                    "border-box" => Some(BgClip::BorderBox),
                    "content-box" => Some(BgClip::ContentBox),
                    _ => None,
                }
            }
            "background-clip" | "-webkit-background-clip" => {
                self.bg_clip = match v.trim() {
                    "padding-box" => Some(BgClip::PaddingBox),
                    "content-box" => Some(BgClip::ContentBox),
                    "text" => Some(BgClip::Text),
                    "border-area" => Some(BgClip::BorderArea),
                    _ => None,
                }
            }
            "background-size" => {
                self.bg_size = match v {
                    "cover" => BgSize::Cover,
                    "contain" => BgSize::Contain,
                    _ => {
                        // Отрицательная длина делает декларацию невалидной
                        // целиком (css-backgrounds-3 §3.9) — размер не трогать.
                        let neg = |l: &Option<Len>| matches!(l, Some(Len::Px(v) | Len::Pct(v)) if *v < 0.0);
                        // Резка вне скобок и процентная смесь: `calc(50px +
                        // 50%) calc(100% - 30px)` — две длины, а не шесть слов;
                        // `background::len_px` в растре складывает доли с
                        // точками сам (css-values-4 §10.9).
                        let words = split_outside_parens(v);
                        let mut it = words.iter().map(String::as_str);
                        let w = it.next().and_then(Len::parse_mixed);
                        let h = it.next().and_then(Len::parse_mixed);
                        if neg(&w) || neg(&h) {
                            return;
                        }
                        if w.is_none() && h.is_none() {
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
                self.bg_pos = parse_pos_words(v);
            }
            // `object-position` — та же грамматика, но для замещаемого
            // содержимого (css-images-3 §5.2).
            "object-view-box" => {
                self.object_view_box = parse_view_box(v);
            }
            "object-position" => {
                self.object_position = Some(parse_pos_words(v));
            }
            // Пооосевые продольные свойства (css-backgrounds-4 §4.1):
            // одна ось, вторая не трогается.
            "background-position-x" | "background-position-y" => {
                let val = match v {
                    "left" | "top" => Some(Len::Pct(0.0)),
                    "center" => Some(Len::Pct(0.5)),
                    "right" | "bottom" => Some(Len::Pct(1.0)),
                    other => Len::parse(other),
                };
                if val.is_some() {
                    if key.ends_with("-x") {
                        self.bg_pos.x = val;
                    } else {
                        self.bg_pos.y = val;
                    }
                }
            }
            "background-attachment" => {
                // `fixed` привязывает плитку к ОБЛАСТИ ПРОСМОТРА: считается
                // она от окна, а красится всё равно только внутри коробки
                // (css-backgrounds-3 §3.10).
                self.bg_fixed = Some(v.eq_ignore_ascii_case("fixed"));
            }
            _ => *hit = false,
        }
    }
}
