//! Слой-градиент в сокращении background (`linear-gradient(…) 1ch 0 / 4ch 1ch no-repeat green`): функция, положение, размер, повтор и цвет нижнего слоя. Тело ветви градиента из Computed::apply_background_shorthand.

use super::*;

impl Computed {
    pub(super) fn background_gradient_layer(&mut self, top: &str, bottom: &str) {
        // Слой сокращения — функция градиента И положение, размер,
        // повтор за ней (`linear-gradient(green, green) 1ch 0 /
        // 4ch 1ch no-repeat`). Вся запись целиком градиентом не
        // разбиралась (хвост за скобкой), и слой пропадал вместе с
        // остальными (`hanging-whitespace-001..004`). Функция —
        // градиент, хвост — свои длинные свойства; при хвосте слой
        // рисуется плиткой (`gradient_raw`).
        let (func, rest) = split_image_func(top);
        let top = func;
        let mut tiled = false;
        if !rest.trim().is_empty() {
            let (pos_part, size_part) = match rest.split_once('/') {
                Some((a, b)) => (a, Some(b)),
                None => (rest, None),
            };
            let mut pos: Vec<String> = vec![];
            for token in split_outside_parens(pos_part) {
                match token.as_str() {
                    "no-repeat" => self.bg_repeat = Some(BgRepeat::NoRepeat),
                    "repeat-x" => self.bg_repeat = Some(BgRepeat::RepeatX),
                    "repeat-y" => self.bg_repeat = Some(BgRepeat::RepeatY),
                    "repeat" => self.bg_repeat = Some(BgRepeat::Repeat),
                    "left" | "right" | "top" | "bottom" | "center" => pos.push(token.clone()),
                    t if Len::parse_mixed(t).is_some() => pos.push(token.clone()),
                    _ => {}
                }
            }
            if !pos.is_empty() {
                self.bg_pos = parse_pos_words(&pos.join(" "));
                tiled = true;
            }
            if let Some(size) = size_part {
                let mut lens: Vec<String> = vec![];
                for token in split_outside_parens(size) {
                    match token.as_str() {
                        "no-repeat" => self.bg_repeat = Some(BgRepeat::NoRepeat),
                        "repeat-x" => self.bg_repeat = Some(BgRepeat::RepeatX),
                        "repeat-y" => self.bg_repeat = Some(BgRepeat::RepeatY),
                        "repeat" => self.bg_repeat = Some(BgRepeat::Repeat),
                        "cover" => self.bg_size = BgSize::Cover,
                        "contain" => self.bg_size = BgSize::Contain,
                        t if Len::parse_mixed(t).is_some() || t == "auto" => {
                            lens.push(token.clone())
                        }
                        _ => {}
                    }
                }
                if !lens.is_empty() {
                    let w = lens.first().and_then(|t| Len::parse_mixed(t));
                    let h = lens.get(1).and_then(|t| Len::parse_mixed(t));
                    self.bg_size = BgSize::Fixed(w, h);
                }
                tiled = true;
            }
            tiled |= self.bg_repeat.is_some();
        }
        self.gradient = parse_gradient(top);
        self.gradient_em = has_font_units(top).then(|| top.to_string());
        // Пространство смешения, которого GPU-путь не выражает
        // (всё, кроме гамма-sRGB и OKLab — css-color-4 §12.1),
        // рисуется растровой плиткой, как у длинного
        // `background-image`: там сырая запись ставится всегда, а
        // сокращение её не заводило, и `in srgb-linear`/`in lch`
        // смешивались в гамма-sRGB. Радиальный растеризатор пока
        // рисует осью (`rasterize_gradient` — `Mode::Axis`), его
        // оставляем на GPU. Сокращение сбрасывает картинку
        // (css-backgrounds-3 §2.1), поэтому иначе — `None`.
        self.gradient_raw = self
            .gradient
            .as_ref()
            .filter(|g| {
                tiled || (!g.radial && !matches!(g.space, GradSpace::Srgb | GradSpace::Oklab))
            })
            .map(|_| top.to_string());
        // Цвет ищется в НИЖНЕМ слое (он один его допускает);
        // при единственном слое нижний == верхний, и цвет стоит
        // там же, рядом с градиентом: `background: linear-… green`.
        for token in split_outside_parens(bottom) {
            if token.contains('(') {
                continue;
            }
            if let Some(c) = Color::parse(&token) {
                self.background = Some(c);
            }
        }
    }
}
