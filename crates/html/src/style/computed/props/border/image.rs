//! Рамка-картинка: разбор пяти свойств border-image* в одну запись BorderImage (Computed::set_border_image).

use super::*;

impl Computed {
    /// Толщина рамки, как её видит модель коробки. Начальный `border-style`
    /// — `none`, а рамка без рисунка вычисляется в ноль (css-backgrounds-3
    /// §4.3), поэтому заданная, но не нарисованная толщина не занимает места.
    /// Разбор всех пяти свойств рамки-картинки в одну запись.
    ///
    /// Сокращение несёт до трёх частей через косую: `<источник> <срез> /
    /// <ширина> / <вылет>` и укладку в конце. Отдельные свойства дополняют ту
    /// же запись, поэтому она заводится по первому же из них.
    pub(crate) fn set_border_image(&mut self, name: &str, v: &str) {
        let mut image = self.border_image.clone().unwrap_or(BorderImage {
            src: String::new(),
            slice: [BorderImageSlice::Pct(1.0); 4],
            fill: false,
            width: [BorderImageWidth::Times(1.0); 4],
            outset: [0.0; 4],
            repeat: (Tiling::None, Tiling::None),
        });
        let mut set = |part: &str, value: &str| match part {
            "border-image-source" => {
                let value = value.trim();
                if value == "none" {
                    image.src.clear();
                } else if value.starts_with("linear-gradient(")
                    || value.starts_with("radial-gradient(")
                    || value.starts_with("conic-gradient(")
                {
                    // Источником может быть любой `<image>`, включая градиент
                    // (css-backgrounds-3 §6.1). Запись хранится как есть:
                    // растрирует её загрузчик картинок.
                    image.src = value.to_string();
                } else if let Some(url) = parse_url(value) {
                    image.src = url;
                }
            }
            "border-image-slice" => {
                image.fill = value.split_whitespace().any(|w| w == "fill");
                let nums: Vec<&str> = value.split_whitespace().filter(|w| *w != "fill").collect();
                let one = |t: &str| match t.strip_suffix('%') {
                    Some(n) => n
                        .parse()
                        .ok()
                        .map(|k: f32| BorderImageSlice::Pct(k / 100.0)),
                    None => t.parse().ok().map(BorderImageSlice::Px),
                };
                if let Some(sides) = four(&nums, one) {
                    image.slice = sides;
                }
            }
            "border-image-width" => {
                let one = |t: &str| {
                    if t == "auto" {
                        return Some(BorderImageWidth::Auto);
                    }
                    if let Some(n) = t.strip_suffix('%') {
                        return n
                            .parse()
                            .ok()
                            .map(|k: f32| BorderImageWidth::Pct(k / 100.0));
                    }
                    // Голое число — множитель толщины рамки, и проверяется ДО
                    // Len: та принимает числа без единиц как точки, и
                    // `border-image-width: 1` превращался в рамку 1px.
                    if let Ok(k) = t.parse::<f32>() {
                        return Some(BorderImageWidth::Times(k));
                    }
                    match Len::parse(t) {
                        Some(Len::Px(v)) => Some(BorderImageWidth::Px(v)),
                        _ => None,
                    }
                };
                let words: Vec<&str> = value.split_whitespace().collect();
                if let Some(sides) = four(&words, one) {
                    image.width = sides;
                }
            }
            "border-image-outset" => {
                let one = |t: &str| match Len::parse(t) {
                    Some(Len::Px(v)) => Some(v),
                    // Голое число — тоже множитель толщины рамки, но её здесь
                    // ещё нет; берём как точки, это ближе всего к правде.
                    _ => t.parse().ok(),
                };
                let words: Vec<&str> = value.split_whitespace().collect();
                if let Some(sides) = four(&words, one) {
                    image.outset = sides;
                }
            }
            "border-image-repeat" => {
                let one = |t: &str| match t {
                    "stretch" => Some(Tiling::None),
                    "repeat" => Some(Tiling::Repeat),
                    "round" => Some(Tiling::Round),
                    "space" => Some(Tiling::Space),
                    _ => None,
                };
                let mut it = value.split_whitespace();
                if let (Some(x), y) = (it.next().and_then(one), it.next().and_then(one)) {
                    image.repeat = (x, y.unwrap_or(x));
                }
            }
            _ => {}
        };
        if name == "border-image" {
            // Части сокращения разделены косой: источник со срезом, ширина,
            // вылет. Укладка и `fill` живут в своих частях и находятся по
            // ключевым словам.
            // Косая режет части ТОЛЬКО вне скобок: в адресе она разделяет
            // каталоги, и запись рвалась по первому же пути (`url(C:/tmp/…)`).
            let mut parts: Vec<&str> = vec![];
            let mut depth = 0i32;
            let mut from = 0usize;
            for (at, ch) in v.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    '/' if depth == 0 => {
                        parts.push(&v[from..at]);
                        from = at + 1;
                    }
                    _ => {}
                }
            }
            parts.push(&v[from..]);
            let head = parts.first().copied().unwrap_or("");
            // Слова головы режутся ВНЕ скобок: у градиента-источника пробелы
            // внутри (`linear-gradient(green, green)`), и он должен остаться
            // одним словом.
            let words = split_outside_parens(head);
            let (urls, rest): (Vec<&String>, Vec<&String>) = words.iter().partition(|w| {
                w.starts_with("url(")
                    || w.starts_with("linear-gradient(")
                    || w.starts_with("radial-gradient(")
                    || w.starts_with("conic-gradient(")
                    || w.as_str() == "none"
            });
            if let Some(src) = urls.first() {
                set("border-image-source", src);
            }
            let (repeat, slice): (Vec<&&String>, Vec<&&String>) = rest
                .iter()
                .partition(|w| matches!(w.as_str(), "stretch" | "repeat" | "round" | "space"));
            if !slice.is_empty() {
                let joined: Vec<&str> = slice.iter().map(|w| w.as_str()).collect();
                set("border-image-slice", &joined.join(" "));
            }
            if !repeat.is_empty() {
                let joined: Vec<&str> = repeat.iter().map(|w| w.as_str()).collect();
                set("border-image-repeat", &joined.join(" "));
            }
            // Укладка (`stretch|repeat|round|space`) по грамматике `||` может
            // стоять и ПОСЛЕ ширины без своей косой: `/ 0px space round`.
            // Слова укладки вынимаются из хвостовых частей, остаток — ширина
            // и вылет.
            let mut tail_repeat: Vec<String> = vec![];
            let strip = |part: &str, reps: &mut Vec<String>| -> String {
                let (found, rest): (Vec<&str>, Vec<&str>) = part
                    .split_whitespace()
                    .partition(|w| matches!(*w, "stretch" | "repeat" | "round" | "space"));
                reps.extend(found.into_iter().map(str::to_string));
                rest.join(" ")
            };
            let width = parts.get(1).map(|p| strip(p, &mut tail_repeat));
            let outset = parts.get(2).map(|p| strip(p, &mut tail_repeat));
            if let Some(width) = width.filter(|w| !w.is_empty()) {
                set("border-image-width", &width);
            }
            if let Some(outset) = outset.filter(|o| !o.is_empty()) {
                set("border-image-outset", &outset);
            }
            if !tail_repeat.is_empty() {
                set("border-image-repeat", &tail_repeat.join(" "));
            }
        } else {
            set(name, v);
        }
        // Запись хранится и БЕЗ источника: лонгхенды приходят в любом порядке,
        // и `border-image-slice` до `border-image-source` иначе выбрасывался —
        // источник, пришедший следом, получал срезы по умолчанию (вся
        // картинка), и рамка рисовалась четырьмя сжатыми копиями образа.
        // Не рисовать и не подавлять обычную рамку при пустом источнике —
        // забота потребителей.
        self.border_image = Some(image);
    }
}
