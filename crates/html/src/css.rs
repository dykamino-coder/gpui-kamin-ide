//! Разбор CSS: декларации из `style=""` и правила из `<style>`.
//!
//! Своя реализация вместо `cssparser`: по замеру нашего же кода 82% селекторов —
//! одиночный класс, глубина не больше трёх, комбинаторов `>`/`+` на весь проект
//! четырнадцать. Полноценная CSS-машина здесь не окупается, а лишняя
//! зависимость — окупается ещё меньше.

use std::collections::HashMap;

/// Пара «свойство: значение». Значение хранится сырым — разбор откладывается
/// до момента применения, чтобы неизвестные свойства не стоили ничего.
pub type Decls = HashMap<String, String>;

/// Одно правило: с чем сопоставлять и что применять.
#[derive(Clone, Debug)]
pub struct Rule {
    pub sel: Selector,
    pub decls: Decls,
    /// Порядок в исходнике: при равной специфичности выигрывает последнее.
    pub order: usize,
}

/// Простой селектор — ровно то подмножество, которое встречается на практике.
#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    /// Псевдокласс `:hover` и т.п. — применяется отдельным слоем.
    pub pseudo: Option<String>,
    /// Предок для `.a .b` и `.a > .b`. Прямой ли — во втором поле.
    pub ancestor: Option<Box<(Selector, bool)>>,
}

impl Selector {
    /// Специфичность как в CSS: (id, класс+псевдо, тег). Сравнивается лексикографически.
    pub fn specificity(&self) -> (u32, u32, u32) {
        let mut s = (
            self.id.is_some() as u32,
            self.classes.len() as u32 + self.pseudo.is_some() as u32,
            self.tag.is_some() as u32,
        );
        if let Some(anc) = &self.ancestor {
            let a = anc.0.specificity();
            s = (s.0 + a.0, s.1 + a.1, s.2 + a.2);
        }
        s
    }

    fn parse_compound(raw: &str) -> Option<Selector> {
        let s = raw.trim();
        if s.is_empty() || s == "*" {
            return Some(Selector {
                tag: None,
                id: None,
                classes: vec![],
                pseudo: None,
                ancestor: None,
            });
        }
        let mut sel = Selector {
            tag: None,
            id: None,
            classes: vec![],
            pseudo: None,
            ancestor: None,
        };
        // Разделитель ищется ВНЕ скобок: в `:not(:first-child)` двоеточие и
        // точка — часть записи псевдокласса, а не начало следующего куска.
        // Пока это не учитывалось, `:not(...)` разбирался на два бессмысленных
        // псевдокласса и правило не совпадало ни с чем.
        let delim = |s: &str| {
            let mut depth = 0i32;
            s.char_indices()
                .find(|(_, ch)| {
                    match ch {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        '.' | '#' | ':' if depth == 0 => return true,
                        _ => {}
                    }
                    false
                })
                .map_or(s.len(), |(i, _)| i)
        };
        // Разбираем слева направо: имя тега идёт первым, дальше .класс/#id/:псевдо.
        let mut rest = s;
        let head_end = delim(rest);
        if head_end > 0 {
            sel.tag = Some(rest[..head_end].trim().to_ascii_lowercase());
        }
        rest = &rest[head_end..];
        while !rest.is_empty() {
            let kind = rest.as_bytes()[0] as char;
            let body = &rest[1..];
            let end = delim(body);
            let name = &body[..end];
            match kind {
                '.' => sel.classes.push(name.to_string()),
                '#' => sel.id = Some(name.to_string()),
                // `:hover` и `::before` дают одно и то же имя: различать их
                // незачем — псевдоэлементы отбираются по имени.
                ':' => sel.pseudo = Some(name.trim_start_matches(':').to_ascii_lowercase()),
                _ => return None,
            }
            rest = &body[end..];
        }
        Some(sel)
    }

    /// `.card > .title`, `.card .title`, `div.card` — всё сюда.
    pub fn parse(raw: &str) -> Option<Selector> {
        // Атрибутные селекторы и прочее, чего мы не умеем, отбрасываем целиком:
        // тихо применить половину правила хуже, чем не применить его совсем.
        // Двойное двоеточие — та же запись псевдоэлемента, что одинарная:
        // в разметке пишут , и отбрасывать его значило терять
        // значки и разделители, для которых сборка уже написана.
        // Внутри скобок `+` и `~` — не комбинаторы, а часть записи `2n+1` в
        // `:nth-child()`. Отбраковка по всей строке резала такие правила.
        let outside_parens = |c: char| {
            let mut depth = 0i32;
            raw.chars().any(|ch| {
                match ch {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                depth == 0 && ch == c
            })
        };
        if raw.contains('[') || outside_parens('~') || outside_parens('+') {
            return None;
        }
        let mut parts: Vec<(String, bool)> = vec![];
        for chunk in raw.split('>') {
            let direct = !parts.is_empty();
            let mut first = true;
            for word in chunk.split_whitespace() {
                parts.push((word.to_string(), direct && first));
                first = false;
            }
        }
        let (last, head) = parts.split_last()?;
        let mut sel = Selector::parse_compound(&last.0)?;
        // Флаг «предок обязан быть прямым» принадлежит ПОТОМКУ, а не предку:
        // в `.a > .b` его несёт `.b`. Поэтому при подъёме вверх флаг берётся
        // от текущего узла, а не от того, которого мы сейчас разбираем.
        let mut direct = last.1;
        let mut cursor = &mut sel;
        for (raw_part, part_direct) in head.iter().rev() {
            let parent = Selector::parse_compound(raw_part)?;
            cursor.ancestor = Some(Box::new((parent, direct)));
            direct = *part_direct;
            cursor = &mut cursor.ancestor.as_mut()?.0;
        }
        Some(sel)
    }
}

/// Разбор `style="a: 1; b: 2"`.
pub fn parse_decls(raw: &str) -> Decls {
    let mut out = Decls::new();
    for item in split_top_level(raw, ';') {
        let Some((k, v)) = item.split_once(':') else {
            continue;
        };
        let key = k.trim().to_ascii_lowercase();
        let val = v.trim().trim_end_matches("!important").trim();
        if !key.is_empty() && !val.is_empty() {
            out.insert(key, val.to_string());
        }
    }
    out
}

/// Разбор содержимого `<style>` — список правил в порядке появления.
/// Условия окружения для `@media`.
///
/// Ширина и высота — в точках, тема — тёмная или светлая. Без них правило
/// пропускалось целиком, и разметка, написанная от узкого экрана вверх,
/// навсегда оставалась в узком виде.
#[derive(Clone, Copy, Debug)]
pub struct Media {
    pub width: f32,
    pub height: f32,
    pub dark: bool,
}

impl Default for Media {
    fn default() -> Self {
        Media {
            width: 1280.0,
            height: 800.0,
            dark: true,
        }
    }
}

impl Media {
    /// Выполняется ли условие `@media`.
    ///
    /// Поддержаны те проверки, что встречаются в разметке интерфейсов:
    /// ширина, высота и предпочтение темы. Незнакомую проверку считаем
    /// невыполненной — правило тогда не применяется, как и раньше.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().trim_start_matches("@media").trim();
        // Запятая — это «или».
        query.split(',').any(|alt| {
            alt.split(" and ").all(|part| {
                let part = part.trim().trim_start_matches('(').trim_end_matches(')');
                let Some((name, value)) = part.split_once(':') else {
                    // `screen`, `all` — верны; `print` — нет.
                    return matches!(part.trim(), "screen" | "all" | "");
                };
                let value = value.trim();
                let number = value
                    .trim_end_matches("px")
                    .trim()
                    .parse::<f32>()
                    .unwrap_or(f32::NAN);
                match name.trim() {
                    "min-width" => self.width >= number,
                    "max-width" => self.width <= number,
                    "min-height" => self.height >= number,
                    "max-height" => self.height <= number,
                    "prefers-color-scheme" => (value == "dark") == self.dark,
                    _ => false,
                }
            })
        })
    }
}

pub fn parse_stylesheet(css: &str) -> Vec<Rule> {
    parse_stylesheet_media(css, Media::default())
}

/// То же, но с известными условиями окружения.
pub fn parse_stylesheet_media(css: &str, media: Media) -> Vec<Rule> {
    let mut out = vec![];
    let cleaned = strip_comments(css);
    let mut rest = cleaned.as_str();
    let mut order = 0usize;
    while let Some(brace) = rest.find('{') {
        // At-правило-ПРЕДЛОЖЕНИЕ блока не имеет и кончается точкой с запятой:
        // `@import`, `@charset`, `@namespace`, `@layer a, b;`. Селектор в них
        // не заходит, поэтому заголовком считается только хвост после
        // последней точки с запятой. Пока считался весь кусок до скобки,
        // `@import "…/ahem.css"; .contain { … }` выглядел одним at-правилом,
        // и ПЕРВОЕ настоящее правило таблицы пропадало вместе со своим телом
        // (`letter-spacing-200`: коробки теряли и рамку, и шрифт).
        let head = rest[..brace]
            .rsplit(';')
            .next()
            .unwrap_or("")
            .trim();
        // Незакрытый блок в КОНЦЕ таблицы закрывается неявно (CSS Syntax
        // §5.4.1): правило всё равно действует. Прежде такое правило
        // отбрасывалось целиком — а в наборе оно встречается прямо в тесте
        // (`break-spaces-009`: у `.test` нет закрывающей скобки, и коробка
        // теряла свою ширину вместе со всем остальным).
        let (body, tail) = match find_matching(&rest[brace..]) {
            Some(close) => (&rest[brace + 1..brace + close], &rest[brace + close + 1..]),
            None => (&rest[brace + 1..], ""),
        };
        rest = tail;
        // At-правила: тело у них устроено иначе, поэтому обычными правилами
        // их применять нельзя. `@media` и `@supports` разбираются как обёртка
        // над обычными правилами, `@keyframes` — отдельно (см.
        // `parse_keyframes`), остальные пропускаются.
        if head.starts_with('@') {
            let inner = if head.starts_with("@media") {
                media.matches(head)
            } else {
                // `@supports` проверяет поддержку свойства браузером; своё
                // покрытие мы знаем не в разборе, поэтому считаем условие
                // выполненным — в разметке им включают современный вариант.
                head.starts_with("@supports")
            };
            if inner {
                for r in parse_stylesheet_media(body, media) {
                    out.push(Rule {
                        order: order + r.order,
                        ..r
                    });
                }
                order += 1000;
            }
            continue;
        }
        let decls = parse_decls(body);
        if decls.is_empty() {
            continue;
        }
        for one in head.split(',') {
            if let Some(sel) = Selector::parse(one) {
                out.push(Rule {
                    sel,
                    decls: decls.clone(),
                    order,
                });
                order += 1;
            }
        }
    }
    out
}

/// Аргументы функции CSS через запятую, не заходя внутрь вложенных скобок:
/// `rgba(0,0,0,.4), inset 0 0 2px red` — два аргумента, а не пять.
pub fn split_args(raw: &str) -> Vec<&str> {
    split_top_level(raw, ',')
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Индекс `}` , парный первой `{`.
fn find_matching(from_brace: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in from_brace.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Разрезание по разделителю, не заходя внутрь скобок: `rgba(0, 0, 0, .5)`
/// содержит запятые, а `grid-template: repeat(2, 1fr)` — и запятые, и скобки.
fn split_top_level(raw: &str, sep: char) -> Vec<&str> {
    let mut out = vec![];
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, ch) in raw.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(&raw[start..i]);
                start = i + ch.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&raw[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decls_keep_commas_inside_functions() {
        let d = parse_decls("color: rgba(1, 2, 3, .5); padding : 4px ");
        assert_eq!(
            d.get("color").map(String::as_str),
            Some("rgba(1, 2, 3, .5)")
        );
        assert_eq!(d.get("padding").map(String::as_str), Some("4px"));
    }

    #[test]
    fn important_is_stripped_not_kept_in_value() {
        let d = parse_decls("color: red !important");
        assert_eq!(d.get("color").map(String::as_str), Some("red"));
    }

    #[test]
    fn selector_parts_and_specificity() {
        let s = Selector::parse("div.card#main:hover").unwrap();
        assert_eq!(s.tag.as_deref(), Some("div"));
        assert_eq!(s.id.as_deref(), Some("main"));
        assert_eq!(s.classes, vec!["card".to_string()]);
        assert_eq!(s.pseudo.as_deref(), Some("hover"));
        assert_eq!(s.specificity(), (1, 2, 1));
    }

    #[test]
    fn descendant_and_child_combinators() {
        let s = Selector::parse(".card > .title").unwrap();
        assert_eq!(s.classes, vec!["title".to_string()]);
        let anc = s.ancestor.as_ref().unwrap();
        assert_eq!(anc.0.classes, vec!["card".to_string()]);
        assert!(anc.1, "после > предок обязан быть прямым");

        let s = Selector::parse(".card .title").unwrap();
        assert!(!s.ancestor.as_ref().unwrap().1, "пробел = любой предок");
    }

    #[test]
    fn unsupported_selectors_are_dropped_whole() {
        assert!(Selector::parse("a[href]").is_none());
        // Псевдоэлемент разбирается: коробку из него строит `dom.rs`.
        assert_eq!(
            Selector::parse("li::before").and_then(|s| s.pseudo),
            Some("before".to_string())
        );
        assert!(Selector::parse("h1 + p").is_none());
    }

    #[test]
    fn media_rules_apply_when_the_condition_holds() {
        let css = "
            /* заметка */
            .a { color: red }
            @media (min-width: 10px) { .b { color: blue } }
            .c, .d { padding: 2px }
        ";
        let rules = parse_stylesheet(css);
        let sels: Vec<String> = rules.iter().map(|r| r.sel.classes.join(",")).collect();
        // Условие выполнено при ширине по умолчанию — правило внутри работает.
        assert_eq!(sels, vec!["a", "b", "c", "d"]);
        assert_eq!(rules[0].decls.get("color").map(String::as_str), Some("red"));
    }

    #[test]
    fn media_rules_are_skipped_when_the_condition_fails() {
        let css = "@media (min-width: 2000px) { .b { color: blue } }";
        let rules = parse_stylesheet_media(
            css,
            Media {
                width: 400.0,
                ..Media::default()
            },
        );
        assert!(rules.is_empty(), "узкое окно не берёт правило для широкого");
    }

    #[test]
    fn color_scheme_query_follows_the_theme() {
        let css = "@media (prefers-color-scheme: dark) { .b { color: #fff } }";
        let dark = parse_stylesheet_media(
            css,
            Media {
                dark: true,
                ..Media::default()
            },
        );
        let light = parse_stylesheet_media(
            css,
            Media {
                dark: false,
                ..Media::default()
            },
        );
        assert_eq!(dark.len(), 1);
        assert!(light.is_empty());
    }
}

/// Кадры анимации: доля времени и объявления на этой доле.
pub type Keyframes = Vec<(f32, Decls)>;

/// `@keyframes имя { 0% {…} 100% {…} }` — все наборы кадров таблицы.
///
/// Разбираются отдельно от правил: у `@keyframes` тело состоит не из
/// объявлений, а из вложенных блоков, и общий разборщик такое телом правила
/// не считает.
pub fn parse_keyframes(css: &str) -> HashMap<String, Keyframes> {
    let cleaned = strip_comments(css);
    let mut out: HashMap<String, Keyframes> = HashMap::new();
    let mut rest = cleaned.as_str();
    while let Some(at) = rest.find("@keyframes") {
        rest = &rest[at + "@keyframes".len()..];
        let Some(brace) = rest.find('{') else { break };
        let name = rest[..brace].trim().to_string();
        let Some(close) = find_matching(&rest[brace..]) else {
            break;
        };
        let body = &rest[brace + 1..brace + close];
        rest = &rest[brace + close + 1..];

        let mut frames: Keyframes = vec![];
        let mut inner = body;
        while let Some(b) = inner.find('{') {
            let stops = inner[..b].trim();
            let Some(c) = find_matching(&inner[b..]) else {
                break;
            };
            let decls = parse_decls(&inner[b + 1..b + c]);
            inner = &inner[b + c + 1..];
            for stop in stops.split(',') {
                let at = match stop.trim() {
                    "from" => Some(0.0),
                    "to" => Some(1.0),
                    other => other
                        .trim_end_matches('%')
                        .trim()
                        .parse::<f32>()
                        .ok()
                        .map(|v| v / 100.0),
                };
                if let Some(at) = at {
                    frames.push((at, decls.clone()));
                }
            }
        }
        frames.sort_by(|a, b| a.0.total_cmp(&b.0));
        if !frames.is_empty() {
            out.insert(name, frames);
        }
    }
    out
}

#[cfg(test)]
mod keyframe_tests {
    use super::*;

    #[test]
    fn keyframes_are_read_with_their_stops() {
        let k = parse_keyframes(
            "@keyframes pulse { from { opacity: 0 } 50% { opacity: 1 } to { opacity: 0 } }",
        );
        let frames = k.get("pulse").expect("набор кадров по имени");
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[1].0, 0.5);
        assert_eq!(frames[0].1.get("opacity").map(String::as_str), Some("0"));
    }

    #[test]
    fn a_stylesheet_without_keyframes_gives_nothing() {
        assert!(parse_keyframes(".a { color: red }").is_empty());
    }
}
