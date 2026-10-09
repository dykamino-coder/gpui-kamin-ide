//! Computed::apply_one: background*, box-shadow, object-*, image-orientation.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_background(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {

            // Сокращение несёт всё сразу: `background: #fff url(a.png) no-repeat`
            // — и цвет, и картинку, и режим повтора. Раньше побеждало что-то
            // одно, и картинка терялась при заданном цвете.
            "background" | "background-color" => {
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
                            tiled
                                || (!g.radial
                                    && !matches!(g.space, GradSpace::Srgb | GradSpace::Oklab))
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
                        && (token == "auto" || token == "cover" || token == "contain" || Len::parse_mixed(&token).is_some())
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
                        "scroll" | "fixed" | "local" | "border-box" | "padding-box"
                        | "content-box" => {}
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
            "box-shadow" if has_font_units(v) => self.shadow_raw = Some(v.to_string()),
            "box-shadow" => {
                self.shadow_raw = None;
                if v == "inherit" {
                    self.shadow_inherit = true;
                    return;
                }
                // `inset` в записи означает тень ВНУТРИ фигуры: раньше такая
                // запись просто не рисовалась.
                // Негодная запись ОТБРАСЫВАЕТСЯ целиком, прежняя тень живёт
                // (§7.1 `none | <shadow>#`; `box-shadow-invalid-001`).
                if !box_shadow_valid(v) {
                    return;
                }
                let (inset, outer): (Vec<&str>, Vec<&str>) = crate::style::css::split_args(v)
                    .into_iter()
                    .partition(|one| one.contains("inset"));
                self.shadows = parse_shadows(&outer.join(","));
                self.inset_shadows = parse_shadows(&inset.join(",").replace("inset", " "));
            }
            "object-fit" => self.object_fit = Some(v.to_string()),
            // `image-orientation` (css-images-3 §5.4): `from-image | none |
            // [<angle> || flip]`. Угол со `flip` спека сама помечает
            // необязательным и устаревшим («optional to implement and
            // deprecated»), и в корпусе его не просит ни один рефтест —
            // разбираем два ключевых слова.
            "image-orientation" => self.image_orient_none = Some(v.trim() == "none"),
            "background-image" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::BG_IMAGE;
                    return;
                }
                // `none` ГАСИТ картинку (§14.2.1): ветки под него не было
                // вовсе, и заданный ранее адрес переживал отмену.
                if v.trim().eq_ignore_ascii_case("none") {
                    self.bg_image = None;
                    self.gradient = None;
                    self.gradient_raw = None;
                } else if v.starts_with("linear-gradient(") || v.starts_with("radial-gradient(") {
                    // Негодная запись роняет ОБЪЯВЛЕНИЕ (§4.2), прежняя
                    // картинка живёт: `linear-gradient(green, green)` и следом
                    // четыре негодных угла обязаны оставить зелёный.
                    if let Some(g) = parse_gradient(v) {
                        self.gradient = Some(g);
                        self.gradient_em = has_font_units(v).then(|| v.to_string());
                        // Сырая запись нужна фону РЯДА таблицы: он рисуется
                        // слоем картинки, и градиент туда идёт источником.
                        self.gradient_raw = Some(v.to_string());
                    }
                } else if let Some(rest) = v.strip_prefix("filter(") {
                    // `filter(<image>, <filter-list>)` (filter-effects-1 §12):
                    // фильтр применяется К КАРТИНКЕ, не к элементу — цвета
                    // градиента пересчитываются на месте.
                    let inner = rest.rfind(')').map(|i| &rest[..i]).unwrap_or(rest);
                    let parts = crate::style::css::split_args(inner);
                    if let Some(img) = parts.first().map(|p| p.trim())
                        && (img.starts_with("linear-gradient(")
                            || img.starts_with("radial-gradient("))
                        && let Some(mut g) = parse_gradient(img)
                    {
                        let mut tmp = Self::default();
                        tmp.apply_one("filter", &parts[1..].join(" "));
                        if let Some(f) = tmp.filter {
                            g.from = f.apply(g.from);
                            g.to = f.apply(g.to);
                            for stop in g.stops.iter_mut() {
                                stop.0 = f.apply(stop.0);
                            }
                        }
                        self.gradient = Some(g);
                    } else if let Some(img) = parts.first().map(|p| p.trim())
                        && gradient_as_raster(img)
                    {
                        // Конический и повторяющиеся идут растровой плиткой
                        // (`gradient_as_raster`): фильтр доносится до цветов
                        // стопов прямо в записи — растеризатор получает уже
                        // пересчитанные цвета (filter-effects-1 §12
                        // `filter()`: фильтр применяется к КАРТИНКЕ).
                        let mut tmp = Self::default();
                        tmp.apply_one("filter", &parts[1..].join(" "));
                        self.bg_image = Some(match tmp.filter {
                            Some(f) => filter_gradient_text(img, &f),
                            None => img.to_string(),
                        });
                    }
                } else if gradient_as_raster(v) {
                    // Конический и ПОВТОРЯЮЩИЕСЯ GPU-путь не выражает — они
                    // идут растровой плиткой (css-images-3 §3.6,
                    // css-images-4 §2.3; растеризатор уже есть, а
                    // `background::source` эти записи опознаёт с самого
                    // начала — до `Computed` они просто не доезжали).
                    self.bg_image = Some(v.to_string());
                } else if let Some(rest) = v.strip_prefix("image(") {
                    // `image(<url>? , <color>?)` (css-images-4 §2.4): цвет —
                    // запасной слой; сплошная заливка выражается градиентом
                    // из одного цвета.
                    let inner = rest.rfind(')').map(|i| &rest[..i]).unwrap_or(rest);
                    let mut url = None;
                    for part in crate::style::css::split_args(inner) {
                        let part = part.trim();
                        if let Some(u) = parse_url(part) {
                            url = Some(u);
                        }
                    }
                    match url {
                        Some(u) => self.bg_image = Some(u),
                        None if parse_image_color(v).is_some() => {
                            // A color image has no natural dimensions (CSS Images 4
                            // §2.3). Keep it in the image layer, including currentColor
                            // until the element's text color has been resolved.
                            self.bg_image = Some(v.to_string());
                            self.gradient = None;
                            self.gradient_raw = None;
                        }
                        _ => {}
                    }
                } else if let Some(rest) = v
                    .strip_prefix("image-set(")
                    .or_else(|| v.strip_prefix("-webkit-image-set("))
                {
                    let inner = rest.rfind(')').map(|i| &rest[..i]).unwrap_or(rest);
                    match image_set_pick(inner) {
                        // Функция ПРЕДСТАВЛЯЕТ выбранный `<image>` (§2.5,
                        // шаг 4): он разбирается тем же кодом, что без
                        // обёртки, — адрес, градиент, конический, повтор.
                        // Кандидат приходит СЫРОЙ записью (`url(...)`): голый
                        // путь ни одна ветка не принимает, и прошлый заход
                        // (09.09, +9/−17) терял на этом все адреса.
                        Some(Some(chosen)) => self.apply_one("background-image", &chosen),
                        // Годная запись без пригодных кандидатов — «invalid
                        // image» (шаг 3): слой ГАСНЕТ, прежний не выживает.
                        Some(None) => {
                            self.bg_image = None;
                            self.gradient = None;
                            self.gradient_raw = None;
                        }
                        // Негодная запись (отрицательное разрешение числом —
                        // css-values-4 §6.3): объявление не применяется (§4.2).
                        None => {}
                    }
                } else if v.trim_end().ends_with(')')
                    && let Some(url) = parse_url(v)
                {
                    // Хвост после `url(...)` делает объявление недействительным
                    // (§4.2): `background-image: url(x) repeat` не картинка.
                    self.bg_image = Some(url);
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
