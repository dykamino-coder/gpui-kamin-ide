//! Сокращение background и background-color: inherit, валидность записи, список слоёв, цвет/картинка/положение/размер верхнего слоя. Тело ветви из Computed::apply_background.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_background_shorthand(
        &mut self,
        key: &str,
        val: &str,
        v: &str,
        hit: &mut bool,
    ) {
        if v == "inherit" {
            self.background_inherit = true;
            // Сокращение наследует ВЕСЬ фон, а не только цвет
            // (css-backgrounds-3 §2.1): картинку, повтор, положение и
            // размер. Флага два, потому что `background-color:
            // inherit` чужую картинку тащить не должен.
            self.background_all_inherit = key == "background";
            return;
        }
        // Сокращение СБРАСЫВАЕТ все свои длинные свойства
        // (css-backgrounds-3 §2.1): `background-color: red;
        // background: bottom fixed` оставляет фон ПРОЗРАЧНЫМ, а
        // прежде красный переживал сокращение. Сбрасывает только
        // `background`; `background-color` трогает лишь цвет.
        // Сокращение СБРАСЫВАЕТ свои длинные свойства
        // (css-backgrounds-3 §2.1): `background-color: red;
        // background: bottom fixed` оставляет фон ПРОЗРАЧНЫМ.
        // Сбрасывается только ЦВЕТ: остальные части разбор ниже берёт
        // из записи не полностью, и полный сброс терял то, чего он не
        // умеет прочесть обратно (замерено: девять пар
        // `textarea-pre-wrap-*` уходили 0.00 → 0.76).
        // Негодное объявление не сбрасывает ничего (§4.1.7).
        if key == "background" && background_shorthand_valid(v) {
            self.background = None;
            self.background_rcs = None;
        }
        if key == "background-color" || background_shorthand_valid(v) {
            self.bg_explicit = true;
        }
        // Цвет от `currentColor` решается не здесь: цвет элемента
        // известен только после каскада, а запись наследуется
        // нерешённой — и голое слово, и относительная функция, и
        // `color-mix` с ним (css-color-4 §7.1, css-color-5 §4.1).
        let low = v.to_ascii_lowercase();
        if !v.contains("gradient(")
            && (low == "currentcolor"
                || low.contains("(from ")
                || (low.starts_with("color-mix(") && low.contains("currentcolor")))
        {
            self.background_rcs = Some(v.to_string());
            return;
        }
        // Фон — СПИСОК слоёв через запятую (css-backgrounds-3 §3.10):
        // первый рисуется ПОВЕРХ остальных, цвет разрешён только
        // последнему. Рисуем верхний слой и цвет нижнего: своего места
        // под остальные у нас пока нет, но терять их молча нельзя —
        // список целиком уходил в разбор ОДНОГО слоя, тот его не
        // понимал, и фон пропадал весь (`background-attachment-margin-
        // root-001`: страница выходила пустой).
        let layers = background_layers(v);
        let top = layers.first().copied().unwrap_or(v);
        let bottom = layers.last().copied().unwrap_or(v);
        if top.starts_with("linear-gradient(") || top.starts_with("radial-gradient(") {
            self.background_gradient_layer(top, bottom);
            return;
        }
        // Сокращение принимает ЛЮБОЙ `<image>` (css-backgrounds-3
        // §3.10), в том числе конический и повторяющиеся: длинное
        // свойство их уже отдаёт растровой плиткой, а тут запись
        // молча падала в разбор слов и терялась целиком — оттого
        // эталоны `image-set-*-gradient-rendering-ref` выходили
        // ПУСТЫМИ и сходились с пустым же тестом.
        let v = top;
        if let Some(image) = split_outside_parens(v)
            .iter()
            .find(|t| parse_image_color(t).is_some())
        {
            self.apply_one("background-image", image);
        } else if gradient_as_raster(v) {
            self.bg_image = Some(v.to_string());
        } else if let Some(url) = parse_url(v) {
            self.bg_image = Some(url);
        }
        // Значение режется по пробелам ВНЕ скобок: иначе
        // `rgba(0, 0, 0, .5)` распадался на куски, ни один из которых
        // не цвет, и фон терялся целиком — самая частая запись
        // полупрозрачной подложки.
        // Слова и длины ПОЛОЖЕНИЯ копятся отдельно и решаются одним
        // разбором: ключевое слово несёт СВОЮ ось (css-backgrounds-3
        // §3.6), поэтому `bottom repeat-x` и `repeat-x bottom` — одно
        // и то же. Разбор тот же, что у отдельного свойства, иначе
        // тест и эталон разойдутся механикой, а не раскладкой.
        let mut pos: Vec<String> = vec![];
        // `<bg-position> [ / <bg-size> ]?` (css-backgrounds-3 §3.10):
        // the words after the slash are the SIZE. They used to fall
        // into the position (`top left / 100% auto` positioned at
        // `top left 100%`, size stayed auto — `background-334`).
        let mut tokens: Vec<String> = vec![];
        for token in split_outside_parens(v) {
            if token.contains('(') || !token.contains('/') {
                tokens.push(token);
                continue;
            }
            let (a, b) = token.split_once('/').unwrap_or((token.as_str(), ""));
            if !a.is_empty() {
                tokens.push(a.to_string());
            }
            tokens.push("/".to_string());
            if !b.is_empty() {
                tokens.push(b.to_string());
            }
        }
        let mut size_words: Option<Vec<String>> = None;
        for token in tokens {
            if token == "/" {
                size_words = Some(vec![]);
                continue;
            }
            if let Some(words) = size_words.as_mut()
                && words.len() < 2
                && (token == "auto"
                    || token == "cover"
                    || token == "contain"
                    || Len::parse_mixed(&token).is_some())
            {
                words.push(token);
                continue;
            }
            match token.as_str() {
                "no-repeat" => self.bg_repeat = Some(BgRepeat::NoRepeat),
                "repeat-x" => self.bg_repeat = Some(BgRepeat::RepeatX),
                "repeat-y" => self.bg_repeat = Some(BgRepeat::RepeatY),
                "repeat" => self.bg_repeat = Some(BgRepeat::Repeat),
                "cover" => self.bg_size = BgSize::Cover,
                "contain" => self.bg_size = BgSize::Contain,
                // Привязка и коробки положением НЕ являются: слово
                // съедается здесь, иначе уедет в разбор положения.
                "scroll" | "fixed" | "local" | "border-box" | "padding-box" | "content-box" => {}
                t if t.starts_with("url(") => {}
                // Ключевые слова осей и длины — это положение.
                "left" | "right" | "top" | "bottom" | "center" => pos.push(token.clone()),
                t if Len::parse_mixed(t).is_some() => pos.push(token.clone()),
                t => {
                    if let Some(c) = Color::parse(t) {
                        self.background = Some(c);
                    }
                }
            }
        }
        // Положение ставится ТОЛЬКО когда слово о нём в записи есть:
        // разбор пустой строки отдаёт «по центру», и каждый фон без
        // положения уехал бы в середину коробки.
        if !pos.is_empty() {
            self.bg_pos = parse_pos_words(&pos.join(" "));
        }
        if let Some(words) = size_words.filter(|w| !w.is_empty()) {
            self.apply_one("background-size", &words.join(" "));
        }
        // Цвет живёт в НИЖНЕМ слое списка — верхний его не допускает.
        if layers.len() > 1 {
            for token in split_outside_parens(bottom) {
                if let Some(c) = Color::parse(&token) {
                    self.background = Some(c);
                }
            }
        }
    }
}
