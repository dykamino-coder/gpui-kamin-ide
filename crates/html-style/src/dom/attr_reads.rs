//! Атрибуты, которые таблицы читают вне селекторов (`attr()`, css-values-5 §7.7).
//!
//! `attr()` подставляет атрибут ТОГО элемента, к которому применилось
//! объявление (у псевдоэлемента — его исходного элемента), и только по имени.
//! Поэтому зависимость стиля от атрибута вне селекторов — это множество имён:
//! правка атрибута с другим именем не меняет ни одной подстановки, и
//! прицельный рестайл ей не отказывает. Прежде любое `attr(` в листе (даже в
//! комментарии) отправляло каждую правку документа в полную пересборку.
//!
//! Имена берутся из текста листов, а не из разобранных правил: так в счёт
//! входят и `@keyframes`, `@property`, и объявления, которых разбор не принял.
//! Лишнее имя (из комментария или строки) только сужает прицельный путь, но
//! не делает его неверным. Имя, которое из текста не вычитать (`attr(var(--n))`,
//! экранирование), означает «любой атрибут».

/// Имена атрибутов, которые читает `attr()` листов документа.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttrReads {
    /// Какое-то `attr()` называет атрибут не литералом: читается любой.
    any: bool,
    /// Локальные имена, ASCII строчными (HTML §2.3 сравнивает имена без регистра).
    names: Vec<String>,
}

impl AttrReads {
    /// Добавить имена всех `attr()` текста `css`.
    pub fn scan(&mut self, css: &str) {
        let bytes = css.as_bytes();
        let mut at = 0;
        while let Some(found) = find_attr_call(&bytes[at..]) {
            let args = at + found;
            match attr_name(&css[args..]) {
                Some(name) => {
                    if !self.names.iter().any(|n| *n == name) {
                        self.names.push(name);
                    }
                }
                None => self.any = true,
            }
            at = args;
        }
    }

    /// Ни одно `attr()` не читает атрибутов.
    pub fn is_empty(&self) -> bool {
        !self.any && self.names.is_empty()
    }

    /// Читает ли какое-то `attr()` атрибут из `changed` (имена ASCII строчными).
    pub fn reads_any_of<'a>(&self, mut changed: impl Iterator<Item = &'a str>) -> bool {
        if self.is_empty() {
            return false;
        }
        changed.any(|name| self.any || self.names.iter().any(|n| n.eq_ignore_ascii_case(name)))
    }
}

/// Смещение первого знака после `attr(` (имя функции без регистра,
/// css-syntax-3 §4.3.4) или `None`.
fn find_attr_call(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(5)
        .position(|w| w[..4].eq_ignore_ascii_case(b"attr") && w[4] == b'(')
        .map(|i| i + 5)
}

/// Локальное имя `<attr-name>` в начале аргументов `attr()`:
/// `[ <ident>? '|' ]? <ident>`; `None` — имя не литерал.
fn attr_name(args: &str) -> Option<String> {
    let rest = args.trim_start();
    let (first, rest) = ident(rest);
    let rest_trim = rest.trim_start();
    // Префикс пространства имён (`attr(xlink|href)`, `attr(*|title)`, `attr(|x)`).
    if let Some(after) = rest_trim.strip_prefix('|') {
        let (local, _) = ident(after);
        return (!local.is_empty()).then(|| local.to_ascii_lowercase());
    }
    if first.is_empty() {
        // `attr(*|x)`: универсальный префикс.
        let after = rest.strip_prefix('*')?.trim_start().strip_prefix('|')?;
        let (local, _) = ident(after);
        return (!local.is_empty()).then(|| local.to_ascii_lowercase());
    }
    // Экранирование в имени не разбираем: любой атрибут.
    if rest.starts_with('\\') {
        return None;
    }
    Some(first.to_ascii_lowercase())
}

/// Ведущий идентификатор (без экранирований) и остаток.
fn ident(s: &str) -> (&str, &str) {
    let end = s
        .char_indices()
        .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii()))
        .map_or(s.len(), |(i, _)| i);
    // Функция (`var(`) — не имя.
    if s[end..].starts_with('(') {
        return ("", s);
    }
    s.split_at(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reads(css: &str) -> AttrReads {
        let mut r = AttrReads::default();
        r.scan(css);
        r
    }

    #[test]
    fn names_of_attr_calls_are_collected_case_insensitively() {
        let r = reads(
            "a::after{content:\" (\" ATTR(Href) \")\"} \
             abbr::before{content:attr( title string, \"?\")} \
             q{--n: attr(data-n type(<number>))}",
        );
        assert_eq!(r.names, ["href", "title", "data-n"]);
        assert!(!r.any);
        assert!(r.reads_any_of(["data-n"].into_iter()));
        assert!(!r.reads_any_of(["class", "style"].into_iter()));
    }

    #[test]
    fn a_namespace_prefix_names_the_local_attribute() {
        assert_eq!(reads("x{content:attr(xlink|href)}").names, ["href"]);
        assert_eq!(reads("x{content:attr(*|title)}").names, ["title"]);
        assert_eq!(reads("x{content:attr(|lang)}").names, ["lang"]);
    }

    #[test]
    fn a_name_that_is_not_a_literal_reads_every_attribute() {
        for css in ["x{content:attr(var(--n))}", "x{content:attr(\\63 lass)}", "x{content:attr()}"] {
            let r = reads(css);
            assert!(r.any, "{css}");
            assert!(r.reads_any_of(["class"].into_iter()), "{css}");
        }
    }

    #[test]
    fn sheets_without_attr_read_nothing() {
        let r = reads("p{color:red} .attr{margin:0} [title]{x:y}");
        assert!(r.is_empty());
        assert!(!r.reads_any_of(["title"].into_iter()));
    }
}
