//! Селекторы: разбор, специфичность, псевдоклассы и псевдоэлементы.

use crate::style::css::*;

/// Простой селектор — ровно то подмножество, которое встречается на практике.
#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    /// Атрибутные условия: `[href]`, `[type="text"]`, `[lang|=en]`.
    pub attrs: Vec<AttrSel>,
    /// Псевдокласс `:hover` и т.п. — применяется отдельным слоем.
    pub pseudo: Option<String>,
    /// Остальные псевдоклассы компаунда: `li:first-child:last-child` несёт
    /// в `pseudo` последний, прочие копятся здесь и обязаны выполниться все.
    pub also: Vec<String>,
    /// Предок для `.a .b` и `.a > .b`. Прямой ли — во втором поле.
    pub ancestor: Option<Box<(Selector, bool)>>,
    /// Предыдущий сосед для `.a + .b` и `.a ~ .b`. Смежный ли — во втором
    /// поле (`+` — ровно предыдущий, `~` — любой раньше).
    pub prev: Option<Box<(Selector, bool)>>,
    /// Явный универсальный селектор `*` в компаунде: безликий хост тени он не
    /// берёт (selectors-4 §featureless; `*:host` — `featureless-002`).
    pub universal: bool,
}

/// Атрибутный селектор одного условия.
#[derive(Clone, Debug, PartialEq)]
pub struct AttrSel {
    pub name: String,
    /// Операция и значение: `=` 0, `~=` 1, `|=` 2, `^=` 3, `$=` 4, `*=` 5.
    /// Отсутствует — проверяется само наличие атрибута.
    pub op: Option<(u8, String)>,
    /// Регистронезависимое сравнение значений (` i` перед `]`).
    pub ci: bool,
}

impl AttrSel {
    /// Совпадает ли значение атрибута (None = атрибута нет).
    pub fn matches(&self, value: Option<&str>) -> bool {
        let Some(value) = value else { return false };
        let Some((op, want)) = &self.op else {
            return true;
        };
        let (v, w);
        let (value, want): (&str, &str) = if self.ci {
            v = value.to_ascii_lowercase();
            w = want.to_ascii_lowercase();
            (&v, &w)
        } else {
            (value, want)
        };
        match op {
            0 => value == want,
            1 => !want.is_empty() && value.split_whitespace().any(|t| t == want),
            2 => {
                value == want
                    || (value.len() > want.len()
                        && value.starts_with(want)
                        && value.as_bytes()[want.len()] == b'-')
            }
            3 => !want.is_empty() && value.starts_with(want),
            4 => !want.is_empty() && value.ends_with(want),
            _ => !want.is_empty() && value.contains(want),
        }
    }
}

impl Selector {
    /// Специфичность как в CSS: (id, класс+псевдо, тег). Сравнивается лексикографически.
    pub fn specificity(&self) -> (u32, u32, u32) {
        let mut s = (
            self.id.is_some() as u32,
            self.classes.len() as u32 + self.attrs.len() as u32,
            self.tag.is_some() as u32,
        );
        // Псевдокласс — +1 к классам, а `of S` добавляет ещё специфичность
        // самого специфичного селектора списка (селекторы-4 §specificity):
        // `:nth-child(even of .foo, #bar)` весит как псевдо + id.
        for p in self.pseudo.iter().chain(self.also.iter()) {
            let m = pseudo_specificity(p);
            s = (s.0 + m.0, s.1 + m.1, s.2 + m.2);
        }
        if let Some(anc) = &self.ancestor {
            let a = anc.0.specificity();
            s = (s.0 + a.0, s.1 + a.1, s.2 + a.2);
        }
        if let Some(prev) = &self.prev {
            let a = prev.0.specificity();
            s = (s.0 + a.0, s.1 + a.1, s.2 + a.2);
        }
        s
    }

    pub(crate) fn parse_compound(raw: &str) -> Option<Selector> {
        let s = raw.trim();
        if s.is_empty() || s == "*" || s == "*|*" {
            return Some(Selector {
                tag: None,
                id: None,
                classes: vec![],
                attrs: vec![],
                pseudo: None,
                also: vec![],
                ancestor: None,
                prev: None,
                universal: !s.is_empty(),
            });
        }
        let mut sel = Selector {
            tag: None,
            id: None,
            classes: vec![],
            attrs: vec![],
            pseudo: None,
            also: vec![],
            ancestor: None,
            prev: None,
            universal: false,
        };
        // Разделитель ищется ВНЕ скобок: в `:not(:first-child)` двоеточие и
        // точка — часть записи псевдокласса, а не начало следующего куска.
        // Пока это не учитывалось, `:not(...)` разбирался на два бессмысленных
        // псевдокласса и правило не совпадало ни с чем.
        let delim = |s: &str| {
            let mut depth = 0i32;
            let mut escaped = false;
            s.char_indices()
                .find(|(_, ch)| {
                    if escaped {
                        escaped = false;
                        return false;
                    }
                    match ch {
                        '\\' => escaped = true,
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        '.' | '#' | ':' | '[' if depth == 0 => return true,
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
            let raw_name = rest[..head_end].trim();
            let local = raw_name.rsplit_once('|').map_or(raw_name, |(_, local)| local);
            if local != "*" && !selector_tokens::ident(local) {
                return None;
            }
            let name = unescape(raw_name).to_ascii_lowercase();
            // Пространство имён нам чуждо: `*|div` — тот же div, `*|*` —
            // универсал (селекторы-4 §type-nmsp).
            // Пространство имён нам чуждо, но НЕОБЪЯВЛЕННЫЙ префикс делает
            // селектор недействительным (css-namespaces-3 §5: «A type selector
            // … containing a namespace prefix that has not been previously
            // declared is an invalid selector»), а с ним и весь список:
            // `.test1, y|div { red }` после неверного `@namespace y` не красит
            // (`at-media-003`, `at-supports-045`, `at-supports-namespace-001/002`).
            let name = match name.rsplit_once('|') {
                Some((ns, t)) => {
                    if !ns_declared(ns) {
                        return None;
                    }
                    t.to_string()
                }
                None => name,
            };
            sel.universal = name == "*";
            if !name.is_empty() && name != "*" {
                sel.tag = Some(name);
            }
        }
        rest = &rest[head_end..];
        while !rest.is_empty() {
            let kind = rest.as_bytes()[0] as char;
            let body = &rest[1..];
            let end = delim(body);
            let name = &body[..end];
            // Псевдоэлемент стоит ПОСЛЕДНИМ в составной части (Selectors §3):
            // `p:first-line.two` — недействительный селектор, а разбирался как
            // годный, и правило красило чужой абзац (`c25-pseudo-elmnt-000`).
            if sel.pseudo.as_deref().is_some_and(is_pseudo_element) {
                return None;
            }
            if kind == '[' {
                // Атрибутное условие тянется до закрывающей скобки, кавычки
                // внутри — со своим содержимым.
                let mut end = 0usize;
                let mut quote: Option<char> = None;
                for (i, ch) in body.char_indices() {
                    match (quote, ch) {
                        (Some(q), c) if c == q => quote = None,
                        (Some(_), _) => {}
                        (None, '"') | (None, '\'') => quote = Some(ch),
                        (None, ']') => {
                            end = i;
                            break;
                        }
                        _ => {}
                    }
                }
                if end == 0 && !body.starts_with(']') {
                    return None;
                }
                sel.attrs.push(selector_tokens::attr(&body[..end])?);
                rest = &body[end + 1..];
                continue;
            }
            if matches!(kind, '.' | '#') && !selector_tokens::ident(name) {
                return None;
            }
            match kind {
                '.' => sel.classes.push(unescape(name)),
                '#' => sel.id = Some(unescape(name)),
                // `:hover` и `::before` дают одно и то же имя: различать их
                // незачем — псевдоэлементы отбираются по имени.
                ':' => {
                    // Неизвестный псевдокласс или псевдоэлемент делает
                    // селектор недействительным, а с ним и ВЕСЬ список
                    // (Selectors §3.1): `p:invalidPseudoClass, p.test1`
                    // не красит ни того, ни другого. Прежде неизвестное имя
                    // просто не совпадало, и вторая часть списка работала.
                    // Имя псевдокласса сравнивается без учёта регистра
                    // (`:FiRSt-cHIlD` — тот же `:first-child`).
                    let bare = name.trim_start_matches(':').to_ascii_lowercase();
                    if !bare.is_empty() && !known_pseudo(&bare) {
                        return None;
                    }
                    if bare.split('(').next() == Some("lang") {
                        let args = bare.strip_prefix("lang(")?.strip_suffix(')')?;
                        if split_top_level(args, ',')
                            .iter()
                            .any(|arg| selector_tokens::value(arg.trim()).is_none())
                        {
                            return None;
                        }
                    }
                    // Пустышка от второго двоеточия `::after` — не
                    // псевдокласс, копить её нельзя.
                    if let Some(prev) = sel.pseudo.take()
                        && !prev.is_empty()
                    {
                        sel.also.push(prev);
                    }
                    sel.pseudo = Some(unescape(name.trim_start_matches(':')).to_ascii_lowercase())
                }
                _ => return None,
            }
            rest = &body[end..];
        }
        Some(sel)
    }

    /// `.card > .title`, `.card .title`, `div.card` — всё сюда.
    pub fn parse(raw: &str) -> Option<Selector> {
        // Разбивка на составные части и комбинаторы МЕЖДУ ними. Внутри скобок
        // `+` и `~` — не комбинаторы, а часть записи `2n+1` в `:nth-child()`.
        // Пробел — комбинатор потомка, если рядом нет знакового.
        let mut tokens: Vec<Result<String, u8>> = vec![];
        let mut depth = 0i32;
        let mut cur = String::new();
        for ch in raw.chars() {
            match ch {
                '(' | '[' => {
                    depth += 1;
                    cur.push(ch);
                }
                ')' | ']' => {
                    depth -= 1;
                    cur.push(ch);
                }
                _ if depth > 0 => cur.push(ch),
                ' ' | '\t' | '\n' => {
                    // Пробел сразу после hex-экранирования — ограничитель
                    // кода, а не комбинатор: `.css\\0032 p` — это ОДИН
                    // класс `css2p`, unescape ограничитель поглотит.
                    if ends_with_open_escape(&cur) {
                        cur.push(' ');
                        continue;
                    }
                    if !cur.is_empty() {
                        tokens.push(Ok(std::mem::take(&mut cur)));
                    }
                    tokens.push(Err(0));
                }
                '>' | '+' | '~' => {
                    if !cur.is_empty() {
                        tokens.push(Ok(std::mem::take(&mut cur)));
                    }
                    tokens.push(Err(match ch {
                        '>' => 1,
                        '+' => 2,
                        _ => 3,
                    }));
                }
                _ => cur.push(ch),
            }
        }
        if !cur.is_empty() {
            tokens.push(Ok(cur));
        }
        let mut compounds: Vec<String> = vec![];
        let mut combs: Vec<u8> = vec![];
        let mut pending: Option<u8> = None;
        for t in tokens {
            match t {
                Err(k) => {
                    // Пробелы вокруг знакового комбинатора — не «потомок»:
                    // знак сильнее.
                    pending = Some(pending.unwrap_or(0).max(k));
                }
                Ok(c) => {
                    if let Some(k) = pending.take() {
                        if compounds.is_empty() {
                            // Комбинатор до первой части — мусор.
                            if k > 0 {
                                return None;
                            }
                        } else {
                            combs.push(k);
                        }
                    }
                    compounds.push(c);
                }
            }
        }
        if compounds.is_empty() || combs.len() + 1 != compounds.len() {
            return None;
        }
        // Сборка слева направо: у `.a > .b + .c` предметом остаётся `.c`,
        // его сосед — `.b`, а предок соседа — `.a`.
        let mut sel = Selector::parse_compound(&compounds[0])?;
        for (comp, comb) in compounds[1..].iter().zip(&combs) {
            // A pseudo-element must end the complex selector (Selectors §3.1).
            if sel.pseudo.as_deref().is_some_and(is_pseudo_element) {
                return None;
            }
            let mut next = Selector::parse_compound(comp)?;
            match comb {
                0 => next.ancestor = Some(Box::new((sel, false))),
                1 => next.ancestor = Some(Box::new((sel, true))),
                2 => next.prev = Some(Box::new((sel, true))),
                _ => next.prev = Some(Box::new((sel, false))),
            }
            sel = next;
        }
        Some(sel)
    }
}

/// Разбор внутренности атрибутного условия: `name`, `name=value`,
/// `name~="v" i` и родня. Кавычки значения снимаются, ` i` в хвосте —
/// регистронезависимость.
/// Известно ли имя псевдокласса или псевдоэлемента.
///
/// Перечень закрытый: по Selectors §3.1 неизвестное имя роняет весь список
/// селекторов, поэтому сюда входит и то, что мы разбираем, но не исполняем —
/// иначе правило с ним пропало бы целиком.
fn known_pseudo(name: &str) -> bool {
    let head = name.split_once('(').map_or(name, |(h, _)| h);
    is_pseudo_element(head)
        || matches!(
            head,
            "hover"
                | "active"
                | "focus"
                | "focus-visible"
                | "focus-within"
                | "link"
                | "visited"
                | "any-link"
                | "target"
                | "target-within"
                | "root"
                | "empty"
                | "scope"
                | "checked"
                | "indeterminate"
                | "default"
                | "disabled"
                | "enabled"
                | "read-only"
                | "read-write"
                | "required"
                | "optional"
                | "valid"
                | "invalid"
                | "in-range"
                | "out-of-range"
                | "placeholder-shown"
                | "autofill"
                | "open"
                | "modal"
                | "fullscreen"
                | "picture-in-picture"
                | "defined"
                | "host"
                | "host-context"
                | "has-slotted"
                | "first-child"
                | "last-child"
                | "only-child"
                | "first-of-type"
                | "last-of-type"
                | "only-of-type"
                | "nth-child"
                | "nth-last-child"
                | "nth-of-type"
                | "nth-last-of-type"
                | "nth-col"
                | "nth-last-col"
                | "not"
                | "is"
                | "where"
                | "has"
                | "matches"
                | "any"
                | "lang"
                | "dir"
        )
}

/// ПсевдоЭЛЕМЕНТ (а не псевдокласс): после него составная часть кончается.
/// Имя может нести аргумент (`scroll-button(block-end)`) — сравнивается голова.
fn is_pseudo_element(name: &str) -> bool {
    let head = name.split_once('(').map_or(name, |(h, _)| h);
    matches!(
        head,
        "before"
            | "after"
            | "first-line"
            | "first-letter"
            | "marker"
            | "placeholder"
            | "selection"
            | "backdrop"
            | "file-selector-button"
            // css-overflow-5: коробки собирает `dom.rs::scroll_marker_pass`.
            | "scroll-marker"
            | "scroll-marker-group"
            | "scroll-button"
    )
}

/// Список селекторов через запятую вне скобок - для `of S` и `:has()`.
pub(crate) fn split_selector_list(raw: &str) -> Vec<&str> {
    split_top_level(raw, ',')
}

/// Специфичность одного псевдокласса: +1 к классам, а `of S` у
/// `:nth-child`/`:nth-last-child` добавляет покомпонентный вес самого
/// специфичного селектора списка (селекторы-4 §specificity). `:has()` -
/// max по списку аргументов БЕЗ собственного веса псевдокласса.
fn pseudo_specificity(pseudo: &str) -> (u32, u32, u32) {
    if let Some(arg) = pseudo
        .strip_prefix("has(")
        .and_then(|r| r.strip_suffix(')'))
    {
        return split_top_level(arg, ',')
            .into_iter()
            .filter_map(|one| {
                let one = one.trim();
                let rest = one.strip_prefix(['>', '+', '~']).unwrap_or(one);
                Selector::parse(rest).map(|s| s.specificity())
            })
            .max()
            .unwrap_or_default();
    }
    let mut s = (0u32, 1u32, 0u32);
    if let Some((name, arg)) = pseudo.split_once('(')
        && matches!(name, "nth-child" | "nth-last-child")
        && let Some(arg) = arg.strip_suffix(')')
        && let Some((_, list)) = nth_of_parts(arg)
    {
        let m = list
            .iter()
            .map(Selector::specificity)
            .max()
            .unwrap_or_default();
        s = (s.0 + m.0, s.1 + m.1, s.2 + m.2);
    }
    s
}

/// Разбор аргумента `:nth-child(An+B of S)`: An+B-часть и список S.
///
/// `of` ищется вне скобок, регистронезависимо, с границей идентификатора с
/// обеих сторон (перед ним пробел, после — не буква-цифра-дефис: `of.foo`
/// годится). Нет `of` — не of-форма; битая или пустая часть S делает весь
/// псевдокласс несопоставимым (вернётся пустой список — звать не с чем).
pub(crate) fn nth_of_parts(arg: &str) -> Option<(String, Vec<Selector>)> {
    let bytes = arg.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    let mut split = None;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            c if depth == 0
                && c.eq_ignore_ascii_case(&b'o')
                && bytes
                    .get(i + 1)
                    .is_some_and(|f| f.eq_ignore_ascii_case(&b'f'))
                && i > 0
                && bytes[i - 1].is_ascii_whitespace()
                && bytes
                    .get(i + 2)
                    .is_none_or(|c| !c.is_ascii_alphanumeric() && *c != b'-') =>
            {
                split = Some(i);
                break;
            }
            _ => {}
        }
        i += 1;
    }
    let at = split?;
    let anb = arg[..at].trim().to_string();
    let mut list = vec![];
    for one in split_top_level(&arg[at + 2..], ',') {
        let Some(sel) = Selector::parse(one) else {
            return Some((anb, vec![]));
        };
        list.push(sel);
    }
    Some((anb, list))
}
