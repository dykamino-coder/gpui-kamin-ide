//! Сокращение background и background-color: inherit, валидность записи, список слоёв,
//! цвет/картинка/положение/размер верхнего слоя. Тело ветви из Computed::apply_background.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_background_shorthand(&mut self, key: &str, v: &str, vcx: ValueCx<'_>) {
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
        // умеет прочесть обратно (`textarea-pre-wrap-*`).
        // Негодное объявление не сбрасывает ничего (§4.1.7).
        if key == "background" && background_shorthand_valid(v, vcx) {
            self.accept_declaration();
            self.set_background_color(None);
        }
        if key == "background-color" || background_shorthand_valid(v, vcx) {
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
            let value = accept_parsed!(
                self,
                crate::style::values::color_space::StyleColor::parse_value(v, vcx)
            );
            if value.is_some() {
                self.set_background_color(value);
            }
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
            self.background_gradient_layer(top, bottom, vcx);
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
            .into_iter()
            .find_map(|token| ImageSource::parse(&token, vcx))
        {
            self.bg_image = Some(image);
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
                    || Len::parse_mixed(&token, vcx).is_some())
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
                "fixed" => self.bg_fixed = Some(true),
                "scroll" | "local" => self.bg_fixed = Some(false),
                "border-box" | "padding-box" | "content-box" => {}
                t if t.starts_with("url(") => {}
                // Ключевые слова осей и длины — это положение.
                "left" | "right" | "top" | "bottom" | "center" => pos.push(token.clone()),
                t if Self::background_position_length(t, vcx) => pos.push(token.clone()),
                t => {
                    if let Some(c) = StyleColor::parse_value(t, vcx) {
                        self.accept_declaration();
                        self.set_background_color(Some(c));
                    }
                }
            }
        }
        // Положение ставится ТОЛЬКО когда слово о нём в записи есть:
        // разбор пустой строки отдаёт «по центру», и каждый фон без
        // положения уехал бы в середину коробки.
        if !pos.is_empty() {
            self.set_background_position(&pos.join(" "), vcx);
        }
        if let Some(words) = size_words.filter(|w| !w.is_empty()) {
            self.apply_one("background-size", &words.join(" "), vcx);
        }
        // Цвет живёт в НИЖНЕМ слое списка — верхний его не допускает.
        if layers.len() > 1 {
            for token in split_outside_parens(bottom) {
                if let Some(c) = StyleColor::parse_value(&token, vcx) {
                    self.set_background_color(Some(c));
                }
            }
        }
    }
}
