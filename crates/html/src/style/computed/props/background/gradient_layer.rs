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
                    t if Self::background_position_length(t) => pos.push(token.clone()),
                    _ => {}
                }
            }
            if !pos.is_empty() {
                self.set_background_position(&pos.join(" "));
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
        // Corner keywords need the concrete image size (CSS Images 3 §3.1).
        // Preserve the existing native route for other untiled shorthand
        // gradients: canvas and table backgrounds select it by raw presence.
        self.gradient_raw = self
            .gradient
            .as_ref()
            .filter(|g| {
                tiled
                    || self.gradient_em.is_some()
                    || g.radial
                    || g.stops_raw.iter().any(|(_, _, px)| px.is_some())
                    || (!g.radial
                        && (!matches!(g.space, GradSpace::Srgb | GradSpace::Oklab)
                            || corner_angle(top, (1.0, 1.0)).is_some()))
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
