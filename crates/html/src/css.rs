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

/// Разделитель повторных объявлений одного свойства внутри значения.
pub const DECL_SEP: char = char::from_u32(1).unwrap();

/// Служебный ключ со списком свойств В ПОРЯДКЕ ЗАПИСИ.
///
/// Каскад решает порядком объявлений (CSS 2.1 §6.4.1), а словарь его не
/// помнит: `background-color: red; background: green` и обратная запись
/// давали ОДИН исход. Имя начинается со служебного знака — свойства с
/// таким именем в разметке не бывает.
pub const ORDER_KEY: &str = "\u{2}order";

/// Одно правило: с чем сопоставлять и что применять.
#[derive(Clone, Debug)]
pub struct Rule {
    pub sel: Selector,
    pub decls: Decls,
    /// Порядок в исходнике: при равной специфичности выигрывает последнее.
    pub order: usize,
    /// Откуда правило: 0 — таблица агента, 1 — таблица документа.
    ///
    /// Происхождение СТАРШЕ специфичности (CSS Cascade §6.4.4): авторское
    /// правило перебивает умолчание агента, даже когда специфичность у него
    /// ниже. Пока обе таблицы лежали в одном списке и сравнивались только
    /// специфичностью, `* { margin: 0 }` со специфичностью (0,0,0) проигрывал
    /// нашему `p { margin: 6px 0 }` — то есть не работал ни один reset.
    pub origin: u8,
    /// Каскадный слой (css-cascade-5 §6.4): путь индексов от корня слоёв,
    /// собственные правила слоя — с хвостом `u32::MAX`, поэтому они идут
    /// ПОСЛЕ своих подслоёв; правила вне слоёв — `[u32::MAX]`, последний
    /// неявный слой. Обычные объявления сравниваются по возрастанию пути,
    /// важные — по убыванию.
    pub layer: Vec<u32>,
}

thread_local! {
    /// Реестр слоёв документа: полное имя → путь индексов (порядок —
    /// по ПЕРВОМУ объявлению, css-cascade-5 §6.4.3).
    static LAYERS: std::cell::RefCell<HashMap<String, Vec<u32>>> =
        std::cell::RefCell::new(HashMap::new());
    /// Следующий индекс ребёнка у каждого родителя (ключ — полное имя).
    static LAYER_NEXT: std::cell::RefCell<HashMap<String, u32>> =
        std::cell::RefCell::new(HashMap::new());
    /// Текущий слой разбора: полное имя и путь.
    static LAYER_NOW: std::cell::RefCell<(String, Vec<u32>)> =
        const { std::cell::RefCell::new((String::new(), Vec::new())) };
    static LAYER_ANON: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Сбросить реестр слоёв — на входе разбора документа.
pub fn reset_layers() {
    LAYERS.with(|l| l.borrow_mut().clear());
    LAYER_NEXT.with(|l| l.borrow_mut().clear());
    LAYER_NOW.with(|l| *l.borrow_mut() = (String::new(), Vec::new()));
    LAYER_ANON.with(|c| c.set(0));
}

/// Путь слоя по имени (возможно с точками) внутри текущего; регистрирует
/// незнакомые звенья. Пустое имя — анонимный слой, всегда новый.
fn layer_enter_path(name: &str) -> (String, Vec<u32>) {
    let (mut full, mut path) = LAYER_NOW.with(|l| l.borrow().clone());
    let segs: Vec<String> = if name.trim().is_empty() {
        let n = LAYER_ANON.with(|c| {
            let v = c.get();
            c.set(v + 1);
            v
        });
        vec![format!("\u{1}anon{n}")]
    } else {
        name.split('.').map(|s| s.trim().to_string()).collect()
    };
    for seg in segs {
        let child = if full.is_empty() { seg } else { format!("{full}.{seg}") };
        let known = LAYERS.with(|l| l.borrow().get(&child).cloned());
        path = match known {
            Some(p) => p,
            None => {
                let idx = LAYER_NEXT.with(|n| {
                    let mut n = n.borrow_mut();
                    let e = n.entry(full.clone()).or_insert(0);
                    let v = *e;
                    *e += 1;
                    v
                });
                let mut p = path.clone();
                p.push(idx);
                LAYERS.with(|l| l.borrow_mut().insert(child.clone(), p.clone()));
                p
            }
        };
        full = child;
    }
    (full, path)
}

/// Путь для правил ТЕКУЩЕГО места разбора (с хвостом «собственные»).
fn layer_of_rules() -> Vec<u32> {
    let mut p = LAYER_NOW.with(|l| l.borrow().1.clone());
    p.push(u32::MAX);
    p
}

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

    fn parse_compound(raw: &str) -> Option<Selector> {
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
            let name = unescape(rest[..head_end].trim()).to_ascii_lowercase();
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
                sel.attrs.push(parse_attr_sel(&body[..end])?);
                rest = &body[end + 1..];
                continue;
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

fn parse_attr_sel(raw: &str) -> Option<AttrSel> {
    let raw = raw.trim();
    let op_at = raw.char_indices().find(|(i, c)| {
        *c == '=' || matches!(c, '~' | '|' | '^' | '$' | '*') && raw[i + 1..].starts_with('=')
    });
    let Some((i, op_ch)) = op_at else {
        if raw.is_empty() {
            return None;
        }
        return Some(AttrSel {
            name: unescape(raw).to_ascii_lowercase(),
            op: None,
            ci: false,
        });
    };
    let name = raw[..i].trim();
    if name.is_empty() {
        return None;
    }
    let (op, val_start) = match op_ch {
        '=' => (0u8, i + 1),
        '~' => (1, i + 2),
        '|' => (2, i + 2),
        '^' => (3, i + 2),
        '$' => (4, i + 2),
        _ => (5, i + 2),
    };
    let mut value = raw[val_start..].trim();
    let mut ci = false;
    if let Some(stripped) = value
        .strip_suffix('i')
        .or_else(|| value.strip_suffix('I'))
        .map(str::trim_end)
        && (stripped.ends_with('"')
            || stripped.ends_with('\'')
            || stripped.ends_with(char::is_whitespace))
    {
        ci = true;
        value = stripped.trim_end();
    }
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value);
    let name = unescape(name).to_ascii_lowercase();
    Some(AttrSel {
        ci: ci || CI_ATTRS.contains(&name.as_str()),
        name,
        op: Some((op, unescape(value))),
    })
}

/// Атрибуты HTML, значения которых сравниваются БЕЗ учёта регистра даже без
/// флага ` i` (HTML, «Case-sensitivity of selectors»). Перечень закрытый:
/// прочие атрибуты сравниваются посимвольно.
const CI_ATTRS: &[&str] = &[
    "accept", "accept-charset", "align", "alink", "axis", "bgcolor", "charset", "checked", "clear",
    "codetype", "color", "compact", "declare", "defer", "dir", "direction", "disabled", "enctype",
    "face", "frame", "hreflang", "http-equiv", "lang", "language", "link", "media", "method",
    "multiple", "nohref", "noresize", "noshade", "nowrap", "readonly", "rel", "rev", "rules",
    "scope", "scrolling", "selected", "shape", "target", "text", "type", "valign", "valuetype",
    "vlink",
];

/// Где в значении стоит восклицательный знак — вне строк, скобок и
/// экранирования. `content: "!"` пометкой важности не является.
fn top_level_bang(value: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut at = 0usize;
    while at < value.len() {
        let ch = value[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += value[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&value[at..], ch);
                continue;
            }
            _ if at_url(&value[at..]) => {
                at += skip_url(&value[at..]);
                continue;
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            '!' if depth == 0 => return Some(at),
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
}

/// Разбор `style="a: 1; b: 2"`.
pub fn parse_decls(raw: &str) -> Decls {
    let mut out = Decls::new();
    let mut order: Vec<String> = Vec::new();
    for item in split_top_level(raw, ';') {
        // Двоеточие ищется НЕэкранированное: `bac\\kground` — это имя
        // `background`, а `background\\:` — имя с двоеточием внутри, то есть
        // объявление без двоеточия вовсе, и его надо отбросить
        // (`escapes-002`, `escapes-003`).
        let mut colon = None;
        let mut escaped = false;
        for (i, ch) in item.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                ':' => {
                    colon = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let Some(colon) = colon else {
            continue;
        };
        let (k, v) = (&item[..colon], &item[colon + 1..]);
        let key = unescape(k.trim()).to_ascii_lowercase();
        // После восклицательного знака в объявлении стоит ровно `important` и
        // ничего больше; всё прочее делает объявление недействительным, и
        // отбрасывается оно целиком (CSS 2.1 §4.1.8). Пока пометка просто
        // срезалась с конца, `background: red ! fail` доезжало значением
        // `red ! fail`, а разбор цвета брал из него первое слово и красил
        // (`core-syntax-006`).
        let val = v.trim();
        if let Some(bang) = top_level_bang(val)
            && !val[bang + 1..].trim().eq_ignore_ascii_case("important")
        {
            continue;
        }
        // Объявление, в чьём значении лежит НЕгодный url-токен, отбрасывается
        // целиком (CSS Syntax §4.3.6): «доехавшая» часть вроде `red` из
        // `background: red url( { test )` красить не должна (`uri-012`).
        if has_bad_url(val) {
            continue;
        }
        // Пометка важности ОСТАЁТСЯ в значении: снимет её тот, кто раскладывает
        // каскад (`Computed::resolve_with_vars`), а срезав её здесь, мы теряли
        // важность целиком — объявление конкурировало на общих основаниях.
        let val = &unescape_value(val);
        if !key.is_empty() && !val.is_empty() {
            // Повтор того же свойства НЕ затирает прежнее на разборе:
            // действительность значения известна только применению
            // (CSS 2.1 §4.1.7 — недействительное объявление игнорируется,
            // а не гасит предыдущее). Части склеиваются служебным
            // разделителем и применяются по порядку. Пользовательские
            // свойства действительны всегда — последнее побеждает.
            if key.starts_with("--") {
                out.insert(key, val.to_string());
            } else {
                // Порядок записи: имя запоминается при ПЕРВОМ появлении —
                // повтор того же свойства применяется на его месте, внутри
                // склеенного значения. В словарь список кладётся ПОСЛЕ
                // разбора и только к непустому: `parse_decls(...).is_empty()`
                // отличает сломанный синтаксис от целого (`@supports`,
                // `@page`), и служебный ключ не должен делать пустое
                // непустым.
                if !out.contains_key(&key) {
                    order.push(key.clone());
                }
                match out.entry(key) {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        let s = e.get_mut();
                        s.push(DECL_SEP);
                        s.push_str(val);
                    }
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(val.to_string());
                    }
                }
            }
        }
    }
    if !out.is_empty() && !order.is_empty() {
        out.insert(ORDER_KEY.to_string(), order.join(&DECL_SEP.to_string()));
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
    /// Печатный носитель: `@media print` истинен, `screen` — ложен.
    pub print: bool,
}

/// Носитель по умолчанию — печать? Ставит стенд для `*-print`-тестов;
/// приложение всегда экран.
pub static PRINT_MEDIA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Селектор страницы (css-page-3 §page-selectors): имя типа страницы и
/// счётчики псевдоклассов. `:blank` хранится ради специфичности — пустых
/// листов стопка не рождает, и такой селектор ни с чем не совпадает.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageSel {
    pub name: Option<String>,
    pub first: u8,
    pub blank: u8,
    pub left: u8,
    pub right: u8,
}

/// Правило `@page` целиком: список селекторов и объявления в порядке записи.
#[derive(Clone, Debug, Default)]
pub struct PageRule {
    pub sels: Vec<PageSel>,
    pub decls: Vec<(String, String)>,
    /// Вложенные правила марджин-боксов (`@top-left { … }`, css-page-3
    /// §margin-at-rules): имя коробки в нижнем регистре → объявления.
    pub margins: Vec<(String, Vec<(String, String)>)>,
}

/// Шестнадцать марджин-боксов листа (css-page-3 §margin-boxes, Table 1) — по
/// часовой стрелке от левого верхнего угла.
pub const MARGIN_BOXES: [&str; 16] = [
    "top-left-corner",
    "top-left",
    "top-center",
    "top-right",
    "top-right-corner",
    "right-top",
    "right-middle",
    "right-bottom",
    "bottom-right-corner",
    "bottom-right",
    "bottom-center",
    "bottom-left",
    "bottom-left-corner",
    "left-bottom",
    "left-middle",
    "left-top",
];

/// Объявления блока В ПОРЯДКЕ ЗАПИСИ, повтор свойства — отдельной парой
/// (`Decls` порядок помнит только в служебном `ORDER_KEY`).
fn ordered_decls(decls: &Decls) -> Vec<(String, String)> {
    let order = decls.get(ORDER_KEY).cloned().unwrap_or_default();
    order
        .split(DECL_SEP)
        .filter_map(|k| decls.get(k).map(|v| (k, v)))
        .flat_map(|(k, v)| {
            v.split(DECL_SEP)
                .map(move |one| (k.to_string(), one.trim().to_string()))
        })
        .collect()
}

/// Вложенные at-блоки тела `@page`: `(имя без @ в нижнем регистре, тело)`.
fn nested_blocks(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = body.as_bytes();
    let mut i = 0usize;
    let mut depth = 0usize;
    while i < b.len() {
        match b[i] {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'@' if depth == 0 => {
                let start = i + 1;
                let Some(open) = body[start..].find('{').map(|k| start + k) else {
                    break;
                };
                let name = body[start..open].trim().to_ascii_lowercase();
                let mut d = 0usize;
                let mut j = open;
                while j < b.len() {
                    match b[j] {
                        b'{' => d += 1,
                        b'}' => {
                            d -= 1;
                            if d == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                out.push((name, body[open + 1..j.min(b.len())].to_string()));
                i = j + 1;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Все правила `@page` документа в порядке появления — с псевдоклассами и
/// именами. Из них `page_decls_for` собирает объявления конкретного листа.
pub static PAGE_RULES: std::sync::Mutex<Vec<PageRule>> = std::sync::Mutex::new(Vec::new());

/// Снимок без очистки: рендер зовётся на каждом кадре, а правила должны
/// пережить все кадры документа (очистка — на разборе следующего).
pub fn page_rules_snapshot() -> Vec<PageRule> {
    PAGE_RULES.lock().unwrap().clone()
}

/// Голова `@page` → список селекторов; `None` — голова неверна, и правило
/// отбрасывается целиком (css-page-3 §syntax-page-selector, как у обычного
/// списка селекторов). Имя регистрозависимо, псевдоклассы — нет.
fn parse_page_selectors(head: &str) -> Option<Vec<PageSel>> {
    let head = head.trim();
    if head.is_empty() {
        return Some(vec![PageSel::default()]);
    }
    let mut out = Vec::new();
    for part in head.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        let (ident, rest) = match part.find(':') {
            Some(i) => (&part[..i], &part[i..]),
            None => (part, ""),
        };
        let ident = ident.trim_end();
        if !ident
            .chars()
            .all(|ch| ch.is_alphanumeric() || ch == '-' || ch == '_')
        {
            return None;
        }
        let mut sel = PageSel {
            name: (!ident.is_empty()).then(|| ident.to_string()),
            ..PageSel::default()
        };
        for pc in rest.split(':').skip(1) {
            match pc.trim().to_ascii_lowercase().as_str() {
                "first" => sel.first += 1,
                "blank" => sel.blank += 1,
                "left" => sel.left += 1,
                "right" => sel.right += 1,
                _ => return None,
            }
        }
        out.push(sel);
    }
    Some(out)
}

/// Объявления листа `index` (с нуля) с именем типа `name` (`""` — без
/// имени): каскад css-page-3 §cascading-and-page-context. Совпавшие правила
/// идут по возрастанию специфичности (f, g, h) — f: имя типа, g: `:first` и
/// `:blank`, h: `:left` и `:right`, — при равной по порядку записи; объявления
/// сливаются в этом порядке, последнее побеждает. Левые/правые — по
/// направлению прогрессии страниц: при ltr первый лист ПРАВЫЙ
/// (§page-selectors: «if the root element's direction is ltr, then the first
/// page is a right page»), при rtl — левый.
pub fn page_decls_for(index: usize, name: &str, rtl: bool) -> Vec<(String, String)> {
    page_decls_in(&PAGE_RULES.lock().unwrap(), index, name, rtl)
}

/// То же по снимку правил (`page_rules_snapshot`).
pub fn page_decls_in(rules: &[PageRule], index: usize, name: &str, rtl: bool) -> Vec<(String, String)> {
    matching_rules(rules, index, name, rtl)
        .into_iter()
        .flat_map(|i| rules[i].decls.clone())
        .collect()
}

/// Марджин-боксы листа: каскад тот же, что у объявлений листа
/// (`page_decls_in`), объявления каждой коробки сливаются по возрастанию
/// специфичности. Порядок — `MARGIN_BOXES`; коробки без правил не попадают.
pub fn page_margins_in(
    rules: &[PageRule],
    index: usize,
    name: &str,
    rtl: bool,
) -> Vec<(String, Vec<(String, String)>)> {
    let hits = matching_rules(rules, index, name, rtl);
    MARGIN_BOXES
        .iter()
        .filter_map(|b| {
            let list: Vec<(String, String)> = hits
                .iter()
                .flat_map(|&i| {
                    rules[i]
                        .margins
                        .iter()
                        .filter(|(n, _)| n == b)
                        .flat_map(|(_, d)| d.clone())
                })
                .collect();
            (!list.is_empty()).then(|| (b.to_string(), list))
        })
        .collect()
}

/// Номера совпавших с листом правил по возрастанию (специфичность, порядок).
fn matching_rules(rules: &[PageRule], index: usize, name: &str, rtl: bool) -> Vec<usize> {
    let right = (index % 2 == 0) != rtl;
    let mut hits: Vec<((u8, u8, u8), usize)> = Vec::new();
    for (order, r) in rules.iter().enumerate() {
        let spec = r
            .sels
            .iter()
            .filter(|s| {
                s.name.as_deref().is_none_or(|n| n == name)
                    && (s.first == 0 || index == 0)
                    && s.blank == 0
                    && (s.left == 0 || !right)
                    && (s.right == 0 || right)
            })
            .map(|s| {
                (
                    s.name.is_some() as u8,
                    s.first + s.blank,
                    s.left + s.right,
                )
            })
            .max();
        if let Some(sp) = spec {
            hits.push((sp, order));
        }
    }
    hits.sort();
    hits.into_iter().map(|(_, i)| i).collect()
}

/// Забрать правила `@page` прошлого документа (чистка перед разбором
/// следующего, `dom::parse_media`).
pub fn take_page_decls() -> Vec<PageRule> {
    std::mem::take(&mut PAGE_RULES.lock().unwrap())
}

/// Правила `@position-try <dashed-ident> { … }` документа (css-anchor-position-1
/// §fallback-rule): имя → объявления. Тот же пул, что `PAGE_RULES`: копится
/// при разборе листов, чистится на разборе следующего документа
/// (`take_try_rules`), читается на сборке кадра (`anchor::place`). Повтор
/// имени перекрывает — «the last one in document order wins».
pub static TRY_RULES: std::sync::Mutex<Option<HashMap<String, Decls>>> =
    std::sync::Mutex::new(None);

/// Зарегистрированное свойство `@property` (css-properties-values-api-1 §3):
/// синтаксис, наследуется ли, начальное значение.
#[derive(Clone, Debug)]
pub struct Registered {
    pub syntax: String,
    pub inherits: bool,
    pub initial: Option<String>,
}

/// Реестр `@property` последнего разобранного документа.
pub static PROPERTY_RULES: std::sync::Mutex<Option<HashMap<String, Registered>>> =
    std::sync::Mutex::new(None);

pub fn take_property_rules() -> HashMap<String, Registered> {
    PROPERTY_RULES.lock().unwrap().take().unwrap_or_default()
}

/// Копия реестра `@property` — его читает сборка переменных узла.
pub fn property_rules() -> HashMap<String, Registered> {
    PROPERTY_RULES.lock().unwrap().clone().unwrap_or_default()
}

pub fn take_try_rules() -> HashMap<String, Decls> {
    TRY_RULES.lock().unwrap().take().unwrap_or_default()
}

/// Объявления правила `@position-try` по имени (`--x`), копией.
pub fn try_rule(name: &str) -> Option<Decls> {
    TRY_RULES.lock().unwrap().as_ref()?.get(name).cloned()
}

/// Срезать вложенные at-блоки из тела `@page`: остаются только объявления.
fn strip_nested_blocks(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut depth = 0usize;
    for ch in body.chars() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
        if depth == 1 && ch == '{' {
            // начало вложенного блока: выкинуть его @-голову из хвоста out
            if let Some(at) = out.rfind('@') {
                out.truncate(at);
            }
        }
    }
    out
}

impl Default for Media {
    fn default() -> Self {
        let print = PRINT_MEDIA.load(std::sync::atomic::Ordering::Relaxed);
        // Печатный носитель меряется ЛИСТОМ, а не окном (mediaqueries-4
        // §width: «For paged media, this is the width of the page box»).
        // Лист WPT по умолчанию — 5in x 3in = 480x288, и `@page { size }`
        // запрос не меняет (csswg#5437; `media-queries-001-print`: запрос
        // 4in..5in x 2in..3in при `@page { size: 10in }` обязан сработать).
        let (width, height) = if print { (480.0, 288.0) } else { (1280.0, 800.0) };
        Media {
            width,
            height,
            dark: true,
            print,
        }
    }
}

/// Сравнение в диапазонной записи медиа-фичи (mediaqueries-5 §3.3).
#[derive(Clone, Copy, PartialEq)]
enum MqCmp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
}

impl MqCmp {
    /// Перевернуть на другую сторону: в `(100px < width)` фича СПРАВА, и
    /// отношение к ней обратное.
    fn flip(self) -> Self {
        match self {
            MqCmp::Lt => MqCmp::Gt,
            MqCmp::Le => MqCmp::Ge,
            MqCmp::Gt => MqCmp::Lt,
            MqCmp::Ge => MqCmp::Le,
            MqCmp::Eq => MqCmp::Eq,
        }
    }

    fn holds(self, a: f32, b: f32) -> bool {
        match self {
            MqCmp::Lt => a < b,
            MqCmp::Le => a <= b,
            MqCmp::Gt => a > b,
            MqCmp::Ge => a >= b,
            MqCmp::Eq => a == b,
        }
    }
}

/// Первая уравновешенная скобочная группа: тело БЕЗ внешних скобок и хвост.
/// Вложенные скобки (`(not (color))`, `calc(...)`) считаются, а не режутся
/// жадным `strip_suffix(')')`.
fn take_parens(s: &str) -> Option<(&str, &str)> {
    let b = s.as_bytes();
    if b.first() != Some(&b'(') {
        return None;
    }
    let mut depth = 0usize;
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[1..i], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Число медиа-фичи: длина, отношение `a / b`, разрешение и голое число.
///
/// Возвращается ПАРА (числитель, знаменатель), а не частное: отношения
/// сравниваются перекрёстным умножением (css-values-4 §7.2 «If two ratios
/// need to be compared, divide the first number by the second» — но при
/// вырожденном `0/0` деление даёт NaN, и Blink считает крест, см.
/// `media_query_evaluator.cc::CompareAspectRatioValue`). Так `0/0`
/// сравнивается как `0 >= 0`, то есть истинно (`aspect-ratio-003`).
fn mq_operand(raw: &str) -> Option<(f32, f32)> {
    let s = raw.trim();
    // `calc(a / b)` — отношение внутри calc (`mq-calc-008`).
    let body = s
        .strip_prefix("calc(")
        .and_then(|r| r.strip_suffix(')'))
        .unwrap_or(s);
    if let Some((a, b)) = body.split_once('/')
        && let (Some(a), Some(b)) = (mq_scalar(a), mq_scalar(b))
    {
        // Вырожденное отношение с НУЛЕВЫМ знаменателем — бесконечность.
        // `0/0` спека обращает в `1/0` (css-values-4 §7.2 «degenerate
        // ratios»; ассерт `aspect-ratio-002` дословно: «0/0 (which is
        // converted into 1/0) is infinite»), иначе крест дал бы `0 >= 0`
        // и правило под `min-aspect-ratio: 0/0` применилось бы.
        if b == 0.0 {
            return Some((1.0, 0.0));
        }
        return Some((a, b));
    }
    mq_scalar(s).map(|v| (v, 1.0))
}

/// Скаляр: длина в точках, разрешение в dppx или голое число.
fn mq_scalar(raw: &str) -> Option<f32> {
    let s = raw.trim();
    // Разрешение: `dppx` и его короткая запись `x`. Проверять надо ИМЕННО
    // через разбор остатка: `0px` тоже кончается на `x`, и жадное отрезание
    // давало «0p», разбор которого проваливался, а ранний возврат гасил всю
    // фичу (`at-media-whitespace-optional-002`, `mq-invalid-media-type-006`).
    if let Some(v) = s
        .strip_suffix("dppx")
        .or_else(|| s.strip_suffix('x'))
        .and_then(|n| n.trim().parse::<f32>().ok())
    {
        return Some(v);
    }
    if let Some(n) = s.strip_suffix("dpi") {
        return n.trim().parse::<f32>().ok().map(|v| v / 96.0);
    }
    if let Some(n) = s.strip_suffix("dpcm") {
        return n.trim().parse::<f32>().ok().map(|v| v * 2.54 / 96.0);
    }
    match crate::value::Len::parse(s) {
        Some(crate::value::Len::Px(v)) => Some(v),
        Some(crate::value::Len::Em(k)) => Some(k * 16.0),
        // Единицы шрифта в медиа-запросе берутся от НАЧАЛЬНОГО шрифта, а не
        // от корневого элемента (mediaqueries-5 §1.3): `:root{font-size:
        // 30000px}` на них не влияет (`mq-calc-003`, `mq-calc-004`).
        Some(crate::value::Len::Ex(k)) => Some(k * crate::metrics::ch_ex_px("", 16.0).1),
        Some(crate::value::Len::Ch(k)) => Some(k * crate::metrics::ch_ex_px("", 16.0).0),
        Some(crate::value::Len::Calc(id)) => {
            let sum = crate::value::calc_get(id);
            let rest = crate::value::Sum {
                px: 0.0,
                em: 0.0,
                ..sum
            };
            if rest == crate::value::Sum::default() {
                Some(sum.px + sum.em * 16.0)
            } else {
                None
            }
        }
        _ => s.parse::<f32>().ok(),
    }
}

impl Media {
    /// Выполняется ли условие `@media` — mediaqueries-5 §3 «Media Queries»
    /// и §3.1 «Error Handling».
    ///
    /// Логика ТРЁХЗНАЧНАЯ. Нераспознанная скобка — это `<general-enclosed>`,
    /// и её значение «неизвестно», а не «ложь»: `not (unknown)` обязано
    /// остаться «неизвестно» (то есть на верхнем уровне ложью), тогда как
    /// прежний двухзначный разбор переворачивал ложь в истину и применял
    /// правило (`at-media-002`, `negation-001`, `mq-gamut-003`).
    pub fn matches(&self, query: &str) -> bool {
        let q = query.trim();
        let q = q.strip_prefix("@media").unwrap_or(q).trim();
        // Пустой список запросов равен `all` (mediaqueries-5 §2.1:
        // «An empty media query list evaluates to true»): пробел после
        // `@media` необязателен, и `@media{ ... }` обязано примениться
        // (`at-media-whitespace-optional-001/002`).
        if q.is_empty() {
            return true;
        }
        // Список запросов через запятую — «или». Негодный запрос НЕ валит
        // список целиком (§3.1: он заменяется на `not all`), поэтому каждая
        // часть считается сама по себе.
        split_top_level(q, ',')
            .into_iter()
            .any(|alt| self.mq_query(alt) == SupTri::True)
    }

    /// Один `<media-query>`: `[not | only]? <media-type> [and <cond>]?`
    /// либо голое `<media-condition>`.
    fn mq_query(&self, raw: &str) -> SupTri {
        let s = raw.trim().to_ascii_lowercase();
        if s.is_empty() {
            return SupTri::False;
        }
        // Голое условие узнаётся по скобке: `<media-condition>` всегда
        // начинается со скобки либо с `not (`.
        if s.starts_with('(') || s.starts_with("not (") {
            return self.mq_condition(&s);
        }
        let (negate, rest) = if let Some(r) = s.strip_prefix("not ") {
            (true, r.trim())
        } else if let Some(r) = s.strip_prefix("only ") {
            (false, r.trim())
        } else {
            (false, s.as_str())
        };
        let (ty, tail) = match rest.split_once(" and ") {
            Some((t, c)) => (t.trim(), Some(c.trim())),
            None => (rest.trim(), None),
        };
        // `<media-type>` — идентификатор, и слова `only`/`not`/`and`/`or`/
        // `layer` типом БЫТЬ НЕ МОГУТ (mediaqueries-5 §3). Значит `@media and`
        // и `@media not and` — синтаксические ошибки, а ошибочный запрос
        // равен `not all`, то есть ЛОЖЬ даже под отрицанием
        // (`mq-invalid-media-type-001..004`, `-layer-001`).
        if ty.is_empty()
            || matches!(ty, "only" | "not" | "and" | "or" | "layer")
            || !ty.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return SupTri::False;
        }
        let hit = match ty {
            "all" => SupTri::True,
            "screen" if !self.print => SupTri::True,
            "print" if self.print => SupTri::True,
            // Неизвестный, но грамматически годный тип — ЛОЖЬ, а не ошибка:
            // `not unknown` истинно (`at-media-002`).
            _ => SupTri::False,
        };
        // Отрицание относится ко всему `<media-type> and <cond>`, а не к
        // одному типу, поэтому склейка идёт первой.
        let full = match tail {
            None => hit,
            Some(cond) => mq_and(hit, self.mq_condition(cond)),
        };
        if negate { sup_not(full) } else { full }
    }

    /// `<media-condition>`: `not <in-parens>` | `<in-parens> (and ..)*` |
    /// `<in-parens> (or ..)*`. Смешивать `and` и `or` без скобок нельзя
    /// (§3.1), такой запрос негоден целиком.
    fn mq_condition(&self, raw: &str) -> SupTri {
        let s = raw.trim();
        if let Some(rest) = s.strip_prefix("not ") {
            let Some((term, tail)) = take_parens(rest.trim()) else {
                return SupTri::False;
            };
            if !tail.trim().is_empty() {
                return SupTri::False;
            }
            return sup_not(self.mq_in_parens(term));
        }
        let Some((term, mut rest)) = take_parens(s) else {
            return SupTri::False;
        };
        let mut acc = self.mq_in_parens(term);
        let mut op: Option<&str> = None;
        loop {
            let r = rest.trim_start();
            if r.is_empty() {
                return acc;
            }
            let word = if let Some(w) = r.strip_prefix("and ") {
                rest = w;
                "and"
            } else if let Some(w) = r.strip_prefix("or ") {
                rest = w;
                "or"
            } else {
                return SupTri::False;
            };
            if *op.get_or_insert(word) != word {
                return SupTri::False;
            }
            let Some((term, tail)) = take_parens(rest.trim_start()) else {
                return SupTri::False;
            };
            let v = self.mq_in_parens(term);
            acc = if word == "and" {
                mq_and(acc, v)
            } else {
                mq_or(acc, v)
            };
            rest = tail;
        }
    }

    /// `<media-in-parens>` без внешних скобок: вложенное условие, фича или
    /// `<general-enclosed>`.
    fn mq_in_parens(&self, inner: &str) -> SupTri {
        let s = inner.trim();
        if s.starts_with('(') || s.starts_with("not ") {
            return self.mq_condition(s);
        }
        // Диапазонная запись §3.3: `(width > 100px)`, `(100px <= width)`,
        // `(100px < width < 200px)`. Двусторонняя форма — это И двух
        // односторонних.
        if s.contains('<') || s.contains('>') || (s.contains('=') && !s.contains(':')) {
            return self.mq_range(s);
        }
        if let Some((name, value)) = s.split_once(':') {
            return self.mq_feature(name.trim(), value.trim(), MqCmp::Eq);
        }
        // Булева форма `<mf-boolean>`: голое имя истинно, когда значение
        // фичи не «ноль/none» (§2.4.2).
        if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return self.mq_boolean(s);
        }
        // `<general-enclosed>` — «неизвестно» (§3.1): пустые скобки, мусор,
        // сломанный диапазон.
        SupTri::Unknown
    }

    /// Диапазонная запись. Разрез по знакам сравнения; фича может стоять
    /// слева, справа либо между двумя пределами.
    fn mq_range(&self, s: &str) -> SupTri {
        let mut parts: Vec<(&str, MqCmp)> = vec![];
        let mut start = 0usize;
        let b = s.as_bytes();
        let mut at = 0usize;
        while at < b.len() {
            let (cmp, len) = match (b[at], b.get(at + 1)) {
                (b'<', Some(b'=')) => (MqCmp::Le, 2),
                (b'>', Some(b'=')) => (MqCmp::Ge, 2),
                (b'<', _) => (MqCmp::Lt, 1),
                (b'>', _) => (MqCmp::Gt, 1),
                (b'=', _) => (MqCmp::Eq, 1),
                _ => {
                    at += 1;
                    continue;
                }
            };
            parts.push((&s[start..at], cmp));
            at += len;
            start = at;
        }
        // Ни одного знака — это не диапазон, а мусор (`mq-range-001`:
        // `(width 500px)` без знака НЕгоден и обязан стать
        // `<general-enclosed>`, а не ошибкой запроса).
        if parts.is_empty() {
            return SupTri::Unknown;
        }
        let tail = s[start..].trim();
        match parts.len() {
            1 => {
                let (left, cmp) = parts[0];
                let left = left.trim();
                // Фича слева — сравнение как есть; фича справа — обратное.
                if self.mq_numeric(mq_base(left)).is_some() {
                    self.mq_feature(left, tail, cmp)
                } else {
                    self.mq_feature(tail, left, cmp.flip())
                }
            }
            2 => {
                let (lo, c1) = parts[0];
                let (name, c2) = parts[1];
                let a = self.mq_feature(name.trim(), lo.trim(), c1.flip());
                let b = self.mq_feature(name.trim(), tail, c2);
                mq_and(a, b)
            }
            _ => SupTri::Unknown,
        }
    }

    /// `<mf-plain>` и разложенные диапазоны. `min-`/`max-` — те же `>=`/`<=`
    /// (§2.4.1); отрицательные пределы разрешены и просто дают ложь или
    /// истину, а НЕ ошибку (§2.4.1 «false in the negative range»,
    /// `mq-negative-range-001/002`).
    fn mq_feature(&self, name: &str, value: &str, cmp: MqCmp) -> SupTri {
        let name = name.trim();
        let (base, cmp) = if let Some(b) = name.strip_prefix("min-") {
            (b, MqCmp::Ge)
        } else if let Some(b) = name.strip_prefix("max-") {
            (b, MqCmp::Le)
        } else {
            (name, cmp)
        };
        // Перечислимые фичи: значение — ключевое слово из закрытого списка.
        // Слово ВНЕ списка делает скобку `<general-enclosed>`, то есть
        // «неизвестно», а не ложь: `not (color-gamut: dci-p3)` обязано
        // остаться ложью на верхнем уровне (`mq-gamut-003`).
        if let Some(allowed) = mq_keywords(base) {
            if !allowed.contains(&value) {
                return SupTri::Unknown;
            }
            return mq_tri(value == self.mq_keyword(base));
        }
        let Some((num, den)) = self.mq_numeric(base) else {
            return SupTri::Unknown;
        };
        let Some((a, b)) = mq_operand(value) else {
            return SupTri::Unknown;
        };
        // Перекрёстное умножение вместо деления: вырожденное `0/0` иначе
        // даёт NaN и валит любое сравнение (Blink, `CompareAspectRatioValue`).
        mq_tri(cmp.holds(num * b, den * a))
    }

    /// Булева форма: фича истинна, когда её значение не ноль и не `none`.
    fn mq_boolean(&self, name: &str) -> SupTri {
        if mq_keywords(name).is_some() {
            return mq_tri(!matches!(self.mq_keyword(name), "none" | "no-preference"));
        }
        match self.mq_numeric(name) {
            Some((num, den)) => mq_tri(num != 0.0 && den != 0.0),
            None => SupTri::Unknown,
        }
    }

    /// Числовое значение фичи этого носителя как отношение (числитель,
    /// знаменатель). `None` — фича не наша, то есть `<general-enclosed>`.
    fn mq_numeric(&self, name: &str) -> Option<(f32, f32)> {
        Some(match name {
            "width" | "device-width" => (self.width, 1.0),
            "height" | "device-height" => (self.height, 1.0),
            "aspect-ratio" | "device-aspect-ratio" => (self.width, self.height),
            "resolution" => (1.0, 1.0),
            "color" => (8.0, 1.0),
            "color-index" | "monochrome" | "grid" => (0.0, 1.0),
            _ => return None,
        })
    }

    /// Значение перечислимой фичи для этого носителя.
    fn mq_keyword(&self, name: &str) -> &'static str {
        match name {
            "orientation" => {
                if self.width >= self.height {
                    "landscape"
                } else {
                    "portrait"
                }
            }
            "scan" => "progressive",
            "update" => "fast",
            "overflow-block" => {
                if self.print {
                    "paged"
                } else {
                    "scroll"
                }
            }
            "overflow-inline" => {
                if self.print {
                    "none"
                } else {
                    "scroll"
                }
            }
            "hover" | "any-hover" => "hover",
            "pointer" | "any-pointer" => "fine",
            "color-gamut" => "srgb",
            "dynamic-range" | "video-dynamic-range" => "standard",
            "prefers-reduced-motion" | "prefers-reduced-transparency"
            | "prefers-reduced-data" | "prefers-contrast" | "forced-colors" => "no-preference",
            "prefers-color-scheme" => {
                if self.dark {
                    "dark"
                } else {
                    "light"
                }
            }
            _ => "",
        }
    }
}

/// Закрытый список значений перечислимой фичи (mediaqueries-5 §4-§11).
fn mq_keywords(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "orientation" => &["portrait", "landscape"],
        "scan" => &["interlace", "progressive"],
        "update" => &["none", "slow", "fast"],
        "overflow-block" => &["none", "scroll", "optional-paged", "paged"],
        "overflow-inline" => &["none", "scroll"],
        "hover" | "any-hover" => &["none", "hover"],
        "pointer" | "any-pointer" => &["none", "coarse", "fine"],
        "color-gamut" => &["srgb", "p3", "rec2020"],
        "dynamic-range" | "video-dynamic-range" => &["standard", "high"],
        "prefers-color-scheme" => &["light", "dark"],
        "prefers-reduced-motion" | "prefers-reduced-transparency" | "prefers-reduced-data" => {
            &["no-preference", "reduce"]
        }
        "prefers-contrast" => &["no-preference", "less", "more", "custom"],
        "forced-colors" => &["none", "active"],
        _ => return None,
    })
}

/// Имя фичи без приставки диапазона — для проверки «фича это или предел».
fn mq_base(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix("min-").or_else(|| s.strip_prefix("max-")).unwrap_or(s)
}

fn mq_tri(v: bool) -> SupTri {
    if v { SupTri::True } else { SupTri::False }
}

/// Трёхзначные `and`/`or` (mediaqueries-5 §3.1): «неизвестно» побеждает
/// всё, кроме определяющего исхода.
fn mq_and(a: SupTri, b: SupTri) -> SupTri {
    match (a, b) {
        (SupTri::False, _) | (_, SupTri::False) => SupTri::False,
        (SupTri::True, SupTri::True) => SupTri::True,
        _ => SupTri::Unknown,
    }
}

fn mq_or(a: SupTri, b: SupTri) -> SupTri {
    match (a, b) {
        (SupTri::True, _) | (_, SupTri::True) => SupTri::True,
        (SupTri::False, SupTri::False) => SupTri::False,
        _ => SupTri::Unknown,
    }
}

pub fn parse_stylesheet(css: &str) -> Vec<Rule> {
    parse_stylesheet_media(css, Media::default())
}

thread_local! {
    /// Префиксы `@namespace` разбираемой таблицы; `None` — разбор идёт не из
    /// таблицы, и префиксы не проверяются (прежнее поведение).
    static NS_PREFIXES: std::cell::RefCell<Option<std::collections::HashSet<String>>> =
        const { std::cell::RefCell::new(None) };
}

/// Снимает префиксы, когда верхний вызов разбора таблицы кончился (и при панике).
struct NsScope(bool);

impl Drop for NsScope {
    fn drop(&mut self) {
        if self.0 {
            NS_PREFIXES.with(|n| *n.borrow_mut() = None);
        }
    }
}

/// Объявлен ли префикс. Пустой (`|div`) и `*` объявлены всегда.
fn ns_declared(ns: &str) -> bool {
    ns.is_empty()
        || ns == "*"
        || NS_PREFIXES.with(|n| n.borrow().as_ref().is_none_or(|s| s.contains(ns)))
}

/// Префиксы действительных `@namespace` (css-namespaces-3 §2): «must follow
/// all @charset and @import rules and precede all other non-ignored at-rules
/// and style rules … Otherwise the @namespace rule is invalid». Даже пустой
/// `@media {}` или `@supports (…) {}` закрывает пролог (`at-media-003`,
/// `at-supports-045`).
fn declared_prefixes(css: &str) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let cleaned = strip_comments(css);
    let mut rest = cleaned.as_str();
    while let Some((piece, tail)) = next_piece(rest) {
        rest = tail;
        let Piece::Statement { head } = piece else { break };
        let low = head.trim().to_ascii_lowercase();
        if low.starts_with("@charset") || low.starts_with("@import") || low.starts_with("@layer") {
            continue;
        }
        let Some(r) = low.strip_prefix("@namespace") else { break };
        let first = r.split_whitespace().next().unwrap_or("");
        if !first.is_empty()
            && !first.starts_with('"')
            && !first.starts_with('\'')
            && !first.starts_with("url(")
        {
            set.insert(first.to_string());
        }
    }
    set
}

/// `attr(ns|name)` с необъявленным префиксом делает объявление негодным —
/// для оракула `@supports` (`at-supports-namespace-001`: `attr(y|href)`).
fn attr_prefixes_declared(v: &str) -> bool {
    let low = v.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(at) = low[from..].find("attr(") {
        let open = from + at + 5;
        let arg = low[open..]
            .trim_start()
            .split(|ch: char| ch == ')' || ch == ',' || ch.is_whitespace())
            .next()
            .unwrap_or("");
        if let Some((ns, _)) = arg.split_once('|')
            && !ns_declared(ns)
        {
            return false;
        }
        from = open;
    }
    true
}

/// То же, но с известными условиями окружения.
pub fn parse_stylesheet_media(css: &str, media: Media) -> Vec<Rule> {
    // Префиксы считает ВЕРХНИЙ вызов: вложенные группы (`@media`,
    // `@supports`, `@layer`) разбираются тем же входом рекурсивно и видят
    // пролог своей таблицы.
    let top = NS_PREFIXES.with(|n| n.borrow().is_none());
    if top {
        let declared = declared_prefixes(css);
        NS_PREFIXES.with(|n| *n.borrow_mut() = Some(declared));
    }
    let _scope = NsScope(top);
    sheet_rules(css, media)
}

fn sheet_rules(css: &str, media: Media) -> Vec<Rule> {
    let mut out = vec![];
    let cleaned = strip_comments(css);
    let mut rest = cleaned.as_str();
    let mut order = 0usize;
    while let Some((piece, tail)) = next_piece(rest) {
        rest = tail;
        // At-правило-ПРЕДЛОЖЕНИЕ блока не имеет и кончается точкой с запятой:
        // `@import`, `@charset`, `@namespace`, `@layer a, b;`. Ни одно из них
        // ничего не задаёт нашей отрисовке, поэтому запись просто пропускается
        // вместе со всей своей преамбулой.
        let (head, body) = match piece {
            // `@layer a, b.c;` — объявление порядка слоёв без правил
            // (css-cascade-5 §6.4.2): имена регистрируются по месту.
            Piece::Statement { head } => {
                let h = head.trim();
                // `get`, а не срез: шестой байт бывает внутри многобайтового
                // знака (`@chars…` в чужой кодировке, `at-charset-029`), и
                // срез ронял весь стенд паникой.
                if h.len() > 6 && h.get(..6).is_some_and(|p| p.eq_ignore_ascii_case("@layer")) {
                    for name in h[6..].split(',') {
                        let name = name.trim();
                        if !name.is_empty() {
                            let _ = layer_enter_path(name);
                        }
                    }
                }
                continue;
            }
            Piece::Block { head, body } => (head.trim(), body),
        };
        // Незакрытый блок в КОНЦЕ таблицы закрывается неявно (CSS Syntax
        // §5.4.1): правило всё равно действует. Прежде такое правило
        // отбрасывалось целиком — а в наборе оно встречается прямо в тесте
        // (`break-spaces-009`: у `.test` нет закрывающей скобки, и коробка
        // теряла свою ширину вместе со всем остальным).
        // At-правила: тело у них устроено иначе, поэтому обычными правилами
        // их применять нельзя. `@media` и `@supports` разбираются как обёртка
        // над обычными правилами, `@keyframes` — отдельно (см.
        // `parse_keyframes`), остальные пропускаются.
        if head.starts_with('@') {
            // Имя at-правила регистронезависимо (§3.3): `@MeDIa` — то же
            // самое, что `@media` (`case-sensitive-001`).
            let name = head.to_ascii_lowercase();
            let inner = if name.starts_with("@media") {
                media.matches(&name)
            } else if name.starts_with("@page") {
                // Правило с головой (имя, `:first/:left/:right/:blank`,
                // список) — в пул `PAGE_RULES`, каскад по листу решает
                // `page_decls_in`. Имя регистрозависимо (css-page-3
                // §using-named-pages) — из ОРИГИНАЛА головы, не из `name`.
                let flat = strip_nested_blocks(body);
                let decls = parse_decls(&flat);
                // Марджин-боксы — вложенные at-правила с известным именем.
                let margins: Vec<(String, Vec<(String, String)>)> = nested_blocks(body)
                    .into_iter()
                    .filter(|(n, _)| MARGIN_BOXES.contains(&n.as_str()))
                    .map(|(n, b)| (n, ordered_decls(&parse_decls(&b))))
                    .collect();
                if !decls.is_empty() || !margins.is_empty() {
                    // Объявления листа — В ПОРЯДКЕ ЗАПИСИ, повтор свойства —
                    // отдельной парой на своём месте. Словарь `Decls` порядка не
                    // помнит (случайный `RandomState` на процесс), повтор
                    // склеивает через `DECL_SEP`, а `page_box` стенда применяет
                    // пары по очереди: `margin: 0; margin-top: 20vw`
                    // (`page-size-016`) давал разный лист от прогона к прогону,
                    // а `margin: 13px; margin: inherit` (`page-margin-006`) —
                    // значение «13px\u{1}inherit», которое не разбиралось вовсе.
                    // Служебный `ORDER_KEY` в пул больше не попадает.
                    let list = ordered_decls(&decls);
                    if let Some(sels) = parse_page_selectors(&head[5..]) {
                        PAGE_RULES.lock().unwrap().push(PageRule {
                            sels,
                            decls: list,
                            margins,
                        });
                    }
                }
                false
            } else if name.starts_with("@property") {
                // css-properties-values-api-1 §3: правило действительно, только
                // если заданы `syntax` и `inherits`, а при синтаксисе не `*` —
                // ещё и `initial-value`. Имя — с исходным регистром.
                let ident = head["@property".len()..].trim();
                let decls = parse_decls(body);
                let syntax = decls
                    .get("syntax")
                    .map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').trim().to_string());
                let inherits = decls.get("inherits").map(|s| s.trim().to_ascii_lowercase());
                let initial = decls.get("initial-value").map(|s| s.trim().to_string());
                if ident.starts_with("--")
                    && let (Some(syntax), Some(inherits)) = (syntax, inherits)
                    && matches!(inherits.as_str(), "true" | "false")
                    && (syntax == "*" || initial.is_some())
                {
                    PROPERTY_RULES
                        .lock()
                        .unwrap()
                        .get_or_insert_with(HashMap::new)
                        .entry(ident.to_string())
                        .or_insert(Registered {
                            syntax,
                            inherits: inherits == "true",
                            initial,
                        });
                }
                false
            } else if name.starts_with("@position-try") {
                // §fallback-rule: тело — обычные объявления (только вставки,
                // поля, размеры, самовыравнивание, `position-anchor`,
                // `position-area`; лишние здесь безвредны — накладываются на
                // копию стиля кандидата). Имя — с оригинальным регистром.
                let ident = head["@position-try".len()..].trim();
                if ident.starts_with("--") {
                    let decls = parse_decls(body);
                    if !decls.is_empty() {
                        TRY_RULES
                            .lock()
                            .unwrap()
                            .get_or_insert_with(HashMap::new)
                            .insert(ident.to_string(), decls);
                    }
                }
                false
            } else if name.starts_with("@supports") {
                // Условию нужен ОРИГИНАЛ: лоуеркейс головы ломал значения
                // (`(font-family: "Foo")`); имя правила — ASCII, срез безопасен.
                supports(&head[9..])
            } else {
                // Слой — прозрачная обёртка: внутри обычные правила, и вся
                // разница в приоритете, которого у нас пока нет. Отбрасывая
                // блок целиком, мы теряли всю разметку современных наборов
                // стилей — они целиком лежат в `@layer`.
                //
                // `@scope` БЕЗ прелюдии — тоже прозрачная обёртка: корнем
                // области служит РОДИТЕЛЬ владельца таблицы (css-cascade-6
                // §3.1 — «If the <scope-start> is omitted, the scoping root is
                // the parent element of the owner node»), предела нет, и
                // селекторы внутри остаются обычными. Форма с прелюдией
                // (`@scope (.a) to (.b)`) по-прежнему выбрасывается: ей нужен
                // перенос корня области в сам селектор, иначе правило
                // расползётся за свою область.
                name.starts_with("@layer") || name == "@scope"
            };
            // Блок `@layer имя { … }`: правила внутри — в своём слое.
            let layer_block = name.starts_with("@layer");
            let saved_layer = layer_block.then(|| {
                let entered = layer_enter_path(head[6..].trim());
                LAYER_NOW.with(|l| std::mem::replace(&mut *l.borrow_mut(), entered))
            });
            let inner_rules = if inner { parse_stylesheet_media(body, media) } else { vec![] };
            if let Some(saved) = saved_layer {
                LAYER_NOW.with(|l| *l.borrow_mut() = saved);
            }
            if inner {
                for r in inner_rules {
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
        // Список селекторов режется ВНЕ скобок: запятая внутри
        // `:nth-child(2n of #a, #b)` — часть of-списка, а не граница
        // селектора, иначе `#b` становился самостоятельным правилом.
        let parts = split_top_level(head, ',');
        // Пустая часть списка селекторов делает недействительным ВЕСЬ
        // список (CSS 2.1 §4.1.7): `body,,div {}` не применяется ни к кому.
        if parts.iter().any(|one| one.trim().is_empty()) {
            continue;
        }
        // Недействительная часть списка тоже валит ВЕСЬ список (§4.1.7):
        // `p:first-line.two, p.two {}` не применяется ни к кому. Прежде
        // негодная часть просто выбрасывалась, и правило красило соседа
        // (`c25-pseudo-elmnt-000`).
        let sels: Option<Vec<Selector>> = parts.iter().map(|one| Selector::parse(one)).collect();
        let Some(sels) = sels else { continue };
        for sel in sels {
            {
                out.push(Rule {
                    sel,
                    decls: decls.clone(),
                    order,
                    // Разбор не знает, чья это таблица: происхождение ставит
                    // тот, кто её подключает (см. `dom.rs`).
                    origin: 0,
                    layer: layer_of_rules(),
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

/// Выполнено ли условие `@supports` (css-conditional-3 §4).
///
/// Трёхзначная логика: неизвестная конструкция (`general-enclosed`) — не
/// ложь и не истина, а «неизвестно»; на верхнем уровне неизвестное и
/// невалидное равнозначны лжи. Поддержка декларации проверяется ОРАКУЛОМ:
/// разобранное объявление применяется к чистому стилю — изменился, значит
/// свойство и значение наши (той же механикой живёт реестр покрытия).
fn supports(condition: &str) -> bool {
    matches!(supports_condition(condition.trim()), Some(SupTri::True))
}

#[derive(Clone, Copy, PartialEq)]
enum SupTri {
    True,
    False,
    Unknown,
}

fn sup_not(t: SupTri) -> SupTri {
    match t {
        SupTri::True => SupTri::False,
        SupTri::False => SupTri::True,
        SupTri::Unknown => SupTri::Unknown,
    }
}

/// `not <терм>` | `<терм> (and <терм>)*` | `<терм> (or <терм>)*` — уровни
/// не смешиваются: `a and b or c` недействительно целиком.
fn supports_condition(s: &str) -> Option<SupTri> {
    let s = s.trim();
    // `not` — слово: слитное `not(` лексится функцией и уходит в терм.
    if let Some(rest) = s.strip_prefix("not")
        && rest.starts_with(char::is_whitespace)
    {
        let rest = rest.trim_start();
        let (term, tail) = supports_take_term(rest)?;
        if !tail.trim().is_empty() {
            return None;
        }
        return Some(sup_not(supports_eval_term(term)));
    }
    let (term, mut rest) = supports_take_term(s)?;
    let mut acc = supports_eval_term(term);
    let mut op: Option<&str> = None;
    loop {
        let r = rest.trim_start();
        if r.is_empty() {
            return Some(acc);
        }
        let word = if let Some(w) = r.strip_prefix("and") {
            if !w.starts_with(char::is_whitespace) {
                return None;
            }
            rest = w;
            "and"
        } else if let Some(w) = r.strip_prefix("or") {
            if !w.starts_with(char::is_whitespace) {
                return None;
            }
            rest = w;
            "or"
        } else {
            return None;
        };
        if let Some(prev) = op {
            if prev != word {
                return None;
            }
        } else {
            op = Some(word);
        }
        let (term, tail) = supports_take_term(rest.trim_start())?;
        let v = supports_eval_term(term);
        acc = match (word, acc, v) {
            ("and", SupTri::True, SupTri::True) => SupTri::True,
            ("and", SupTri::False, _) | ("and", _, SupTri::False) => SupTri::False,
            ("and", ..) => SupTri::Unknown,
            ("or", SupTri::True, _) | ("or", _, SupTri::True) => SupTri::True,
            ("or", SupTri::False, SupTri::False) => SupTri::False,
            _ => SupTri::Unknown,
        };
        rest = tail;
    }
}

/// Один терм: скобочная группа либо функция `имя(...)`; возврат — тело
/// терма (со скобками функции внутри среза) и хвост после него.
fn supports_take_term(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    // Функция: идентификатор вплотную к скобке.
    let mut name_end = 0;
    while name_end < bytes.len()
        && (bytes[name_end].is_ascii_alphanumeric() || bytes[name_end] == b'-')
    {
        name_end += 1;
    }
    let open = if name_end < bytes.len() && bytes[name_end] == b'(' {
        name_end
    } else if bytes.first() == Some(&b'(') {
        0
    } else {
        return None;
    };
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[..i + 1], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// `selector(<complex-selector>)` (css-conditional-4 §at-supports-ext):
/// ОДИН сложный селектор — список через запятую ложен (`at-supports-selector-004`);
/// внутри `:is()/:where()/:has()/:not()` прощающего разбора при проверке нет —
/// неизвестная часть роняет всё (`…-detecting-invalid-in-logical-combinations`).
/// Псевдоэлементы, которые Blink знает, а наш каскад не исполняет
/// (`::details-content`, `::picker(select)`, `::picker-icon`,
/// `::-webkit-slider-thumb`), для ПРОВЕРКИ снимаются; в `known_pseudo` их не
/// вносим — иначе правила с ними начали бы применяться к самой коробке.
fn supports_selector(args: &str) -> bool {
    let s = args.trim();
    if split_top_level(s, ',').len() > 1 {
        return false;
    }
    let mut s = s.to_string();
    for known in [
        "::details-content",
        "::picker(select)",
        "::picker-icon",
        "::-webkit-slider-thumb",
        "::-webkit-slider-runnable-track",
    ] {
        s = s.replace(known, "");
    }
    // Неизвестные вендорные псевдо — не поддержаны (`::-webkit-asdf`).
    if s.contains("::-webkit-") || s.contains(":-webkit-") {
        return false;
    }
    // Снятый псевдоэлемент мог стоять один: `::picker-icon` → пусто.
    if s.trim().is_empty() || s.ends_with(|ch: char| ch.is_whitespace() || "+>~".contains(ch)) {
        s.push('*');
    }
    selector_strict(&s)
}

/// Селектор годен, и годна КАЖДАЯ часть списков внутри `:is()` и родни.
fn selector_strict(s: &str) -> bool {
    if Selector::parse(s).is_none() {
        return false;
    }
    for f in [":is(", ":where(", ":has(", ":not(", ":matches(", ":any("] {
        let mut from = 0usize;
        while let Some(at) = s[from..].find(f) {
            let open = from + at + f.len();
            let mut depth = 1i32;
            let mut close = None;
            for (i, ch) in s[open..].char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(open + i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(close) = close else { return false };
            for part in split_top_level(&s[open..close], ',') {
                let mut p = part.trim();
                // Относительный селектор `:has(> .a)`: ведущий комбинатор.
                if f == ":has(" {
                    p = p.trim_start_matches(['>', '+', '~']).trim_start();
                }
                if p.is_empty() || !selector_strict(p) {
                    return false;
                }
            }
            from = close;
        }
    }
    true
}

fn supports_eval_term(term: &str) -> SupTri {
    let term = term.trim();
    // Функция `имя(...)`.
    if !term.starts_with('(') {
        let Some(open) = term.find('(') else {
            return SupTri::Unknown;
        };
        let name = term[..open].to_ascii_lowercase();
        let args = &term[open + 1..term.len().saturating_sub(1)];
        return match name.as_str() {
            "selector" => {
                if supports_selector(args) {
                    SupTri::True
                } else {
                    SupTri::False
                }
            }
            "font-format" => {
                let f = args.trim().to_ascii_lowercase();
                if matches!(f.as_str(), "woff" | "woff2" | "truetype" | "opentype") {
                    SupTri::True
                } else {
                    SupTri::False
                }
            }
            // `font-tech(<font-tech>)` — ровно ОДНО слово (css-conditional-5
            // §font-tech): `features-opentype color-COLRv1` и список через
            // запятую — ложь (`at-supports-font-tech-001`). Технологии —
            // то, что открывает DirectWrite; `incremental` — нет.
            "font-tech" => {
                let t = args.trim().to_ascii_lowercase();
                if matches!(
                    t.as_str(),
                    "features-opentype"
                        | "features-aat"
                        | "color-colrv0"
                        | "color-colrv1"
                        | "color-sbix"
                        | "color-cbdt"
                        | "variations"
                        | "palettes"
                ) {
                    SupTri::True
                } else {
                    SupTri::False
                }
            }
            "at-rule" => SupTri::False,
            // `not(...)`/`or(...)` и прочие неизвестные функции — это
            // `<general-enclosed>`, а css-conditional-3 §4 говорит о нём
            // дословно: «The result is false». Не «неизвестно»: иначе
            // `not unknown()` остаётся неизвестным и на верхнем уровне
            // ложным, тогда как обязан быть ИСТИНОЙ (`at-supports-046`).
            _ => SupTri::False,
        };
    }
    let inner = &term[1..term.len() - 1];
    // Скобки вокруг условия.
    if let Some(t) = supports_condition(inner) {
        return t;
    }
    // Точка с запятой внутри скобок: `<declaration>` её не содержит
    // (css-syntax-3 §5.4.4 — `<declaration-value>` не берёт `;` верхнего
    // уровня), значит `(margin: 0;)` — не объявление, а `<general-enclosed>`,
    // то есть ЛОЖЬ (`at-supports-038/039`).
    if split_top_level(inner, ';').len() > 1 {
        return SupTri::False;
    }
    // Декларация: непустой разбор + дельта на чистом стиле.
    let colons = split_top_level(inner, ':');
    if colons.len() >= 2 {
        // Пользовательское свойство поддержано всегда, если объявление
        // разобралось (css-variables-1 §2: значением `--*` служит любой
        // годный `<declaration-value>`). Оракул «дельта на чистом стиле» его
        // не видит: `--foo` не пишет ни в одно поле (`at-supports-044`).
        if colons[0].trim().to_ascii_lowercase().starts_with("--") {
            return if parse_decls(inner).is_empty() {
                SupTri::False
            } else {
                SupTri::True
            };
        }
        // Второе двоеточие ВЕРХНЕГО уровня у обычного свойства значит, что в
        // значение затесалось чужое объявление: `(margin: 0 or padding: 0)`
        // и `(margin: 0 and padding: 0)` — мусор, а не «margin с довеском»
        // (`at-supports-034..037`: каждое условие берётся в СВОИ скобки).
        if colons.len() > 2 {
            return SupTri::False;
        }
        if !attr_prefixes_declared(colons[1]) {
            return SupTri::False;
        }
        // css-variables-1 §3: «If a property contains one or more var()
        // functions, and those functions are syntactically valid, the entire
        // property's grammar must be assumed to be valid at parse time».
        // Оракул «дельта на чистом стиле» такое не видит: без значения
        // переменной объявление не пишет ни в одно поле (`at-supports-044`,
        // `(color: var(--anything) invalid-value)`).
        if colons[1].to_ascii_lowercase().contains("var(") {
            return if parse_decls(inner).is_empty() {
                SupTri::False
            } else {
                SupTri::True
            };
        }
        let decls = parse_decls(inner);
        if decls.is_empty() {
            // Синтаксис объявления сломан (`!bogus`, `!important !important`,
            // `!important green`) — `<general-enclosed>`, то есть ЛОЖЬ
            // (`css-supports-043/044/045`).
            return SupTri::False;
        }
        // Пометка важности к ПОДДЕРЖКЕ отношения не имеет и обязана быть
        // допустима (css-conditional-3 §4: «Property declarations in an
        // @supports rule can have !important specified»). `parse_decls`
        // оставляет её в значении для каскада — здесь она мешает разобрать
        // само значение (`css-supports-004`, `at-supports-007`).
        let clean: crate::css::Decls = decls
            .iter()
            .map(|(k, v)| {
                if k == ORDER_KEY {
                    return (k.clone(), v.clone());
                }
                let parts: Vec<&str> = v
                    .split(DECL_SEP)
                    .map(|part| match top_level_bang(part) {
                        Some(at) => part[..at].trim(),
                        None => part,
                    })
                    .collect();
                (k.clone(), parts.join(&DECL_SEP.to_string()))
            })
            .collect();
        let mut c = crate::computed::Computed::default();
        c.apply_decls(&clean);
        // Счётчик порядка объявлений и номера сторон — БУХГАЛТЕРИЯ каскада, а
        // не значения свойств: они меняются у любого объявления, и без
        // обнуления «поддержанным» выходило всё подряд, включая
        // `(color: rainbow)` (`css-supports-005`, `at-supports-009`).
        c.decl_seq = 0;
        c.side_seq = Default::default();
        return if format!("{c:?}") != format!("{:?}", crate::computed::Computed::default()) {
            SupTri::True
        } else {
            SupTri::False
        };
    }
    // Скобка без двоеточия и без условия — тоже `<general-enclosed>`: ЛОЖЬ
    // (`css-supports-032/033/034/040`).
    SupTri::False
}

/// Имя без экранирования (CSS Syntax §4.3.7).
///
/// `BSL0031 ` — знак по шестнадцатеричному коду, до шести цифр, и один
/// пробел после них съедается как ограничитель. `BSL.` — сама точка, а не
/// разделитель составного селектора. Пока этого не было, `p\\.class`
/// разбирался как тег `p` с классом `class` и совпадал с `p class="class"`,
/// хотя обязан искать тег с точкой в имени, то есть не совпадать ни с чем.
/// Кончается ли накопленный кусок НЕЗАВЕРШЁННЫМ hex-экранированием:
/// обратная косая, за ней от одной до шести шестнадцатеричных цифр.
fn ends_with_open_escape(cur: &str) -> bool {
    let hex_len = cur
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_hexdigit())
        .count();
    if hex_len == 0 || hex_len > 6 {
        return false;
    }
    let mut rest = cur.chars().rev().skip(hex_len);
    // Косая перед цифрами, и она сама не экранирована.
    rest.next() == Some('\\') && rest.next() != Some('\\')
}

pub fn unescape(name: &str) -> String {
    if !name.contains('\\') {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len());
    let mut it = name.chars().peekable();
    while let Some(ch) = it.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6 {
            match it.peek() {
                Some(c) if c.is_ascii_hexdigit() => {
                    hex.push(*c);
                    it.next();
                }
                _ => break,
            }
        }
        if hex.is_empty() {
            // Экранирован обычный знак — он и остаётся, уже без особого
            // значения. Перевод строки экранировать нельзя, но в имени его и
            // не бывает.
            if let Some(c) = it.next() {
                out.push(c);
            }
            continue;
        }
        // Один пробел после цифр — ограничитель кода, а не часть имени.
        if it.peek().is_some_and(|c| c.is_whitespace()) {
            it.next();
        }
        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
            // Нулевой знак и суррогаты заменяются знаком замены (§4.3.7).
            Some(c) if c != '\u{0}' => out.push(c),
            _ => out.push('\u{fffd}'),
        }
    }
    out
}

/// Значение без экранирования — но кавычки не трогая.
///
/// Раскрывается всё, что раскрывается в имени, кроме внутренности строк:
/// `"\\""` — это кавычка ВНУТРИ строки, и раскрыв её, мы получили бы три
/// кавычки подряд и порвали значение (`escapes-001`).
///
/// Обрезать результат НЕЛЬЗЯ: `\\0020yellow` раскрывается в имя с пробелом
/// внутри, а такое значение недействительно; обрезка сделала бы из него
/// `yellow` и применила то, что применять нечего (`escapes-014`).
fn unescape_value(value: &str) -> String {
    if !value.contains('\\') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut at = 0usize;
    while at < value.len() {
        let ch = value[at..].chars().next().unwrap_or('\u{0}');
        if ch == '"' || ch == '\'' {
            let body = at + ch.len_utf8();
            let end = body + skip_string(&value[body..], ch);
            out.push_str(&value[at..end]);
            at = end;
            continue;
        }
        if ch != '\\' {
            out.push(ch);
            at += ch.len_utf8();
            continue;
        }
        let tail = &value[at + ch.len_utf8()..];
        let taken = first_escape(tail);
        let one = unescape(&format!("\\{taken}"));
        // Пробел, полученный из кода, — часть ИМЕНИ, а не отступ, и
        // значение с таким именем недействительно. Наш конвейер
        // обрезает значение при использовании, поэтому раскрытие
        // потеряло бы ровно ту особенность, из-за которой объявление и
        // должно отпасть (`escapes-014`, `color:\\0020yellow`).
        if one.chars().all(char::is_whitespace) {
            out.push(ch);
            out.push_str(taken);
        } else {
            out.push_str(&one);
        }
        at += ch.len_utf8() + taken.len();
    }
    out
}

/// Сколько байт после обратного слэша съедает одно экранирование: до шести
/// шестнадцатеричных цифр и один пробел за ними, либо ровно один знак.
fn first_escape(tail: &str) -> &str {
    let mut end = 0usize;
    let mut digits = 0usize;
    for (i, ch) in tail.char_indices() {
        if digits < 6 && ch.is_ascii_hexdigit() {
            digits += 1;
            end = i + ch.len_utf8();
            continue;
        }
        if digits > 0 && ch.is_whitespace() {
            end = i + ch.len_utf8();
        }
        break;
    }
    if digits == 0 {
        end = tail.chars().next().map_or(0, char::len_utf8);
    }
    &tail[..end]
}

/// Чем кончилась очередная запись таблицы.
enum Piece<'a> {
    /// Правило с телом: заголовок и содержимое фигурных скобок.
    Block { head: &'a str, body: &'a str },
    /// At-правило-предложение: заголовок до точки с запятой, тела нет.
    /// Заголовок пока никем не читается, но остаётся в разборе для симметрии.
    #[allow(dead_code)]
    Statement { head: &'a str },
}

/// Отрезать от таблицы одну запись, вернув её и остаток.
///
/// Скобки ВСЕХ ВИДОВ считаются вместе (§5.4.1): преамбула правила поглощает
/// уравновешенные `[]`, `()` и `{}`, а кончается на точке с запятой или на
/// теле в фигурных скобках — смотря что встретится раньше НА ВЕРХНЕМ УРОВНЕ.
/// Пока искалась просто первая `{`, неизвестное at-правило с мусором в
/// преамбуле (`@foo ] } ) … ;`) уводило разбор внутрь своего мусора, и вся
/// таблица за ним разъезжалась (`matching-brackets-001`, `core-syntax-001`).
/// Начинается ли кусок с at-правила: ведущие `<!--`/`-->` верхнего уровня
/// — пробельные токены (css-syntax-3 §5.4.1), их пропускаем.
fn statement_head(text: &str) -> bool {
    let mut t = text.trim_start();
    loop {
        if let Some(r) = t.strip_prefix("<!--").or_else(|| t.strip_prefix("-->")) {
            t = r.trim_start();
        } else {
            return t.starts_with('@') || t.is_empty();
        }
    }
}

fn next_piece(text: &str) -> Option<(Piece<'_>, &str)> {
    let mut square = 0i32;
    let mut round = 0i32;
    let mut at = 0usize;
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += text[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&text[at..], ch);
                continue;
            }
            _ if at_url(&text[at..]) => {
                at += skip_url(&text[at..]);
                continue;
            }
            // Глубина не уходит в минус: лишняя `]` или `)` — просто знак
            // (§5.4.1), а не закрытие несуществующей скобки. Пока уходила,
            // преамбула `@foo ] } ) …` делала следующую настоящую `[`
            // нулевым уровнем, и точка с запятой ВНУТРИ скобок обрывала
            // at-правило раньше времени (`matching-brackets-001`).
            '[' => square += 1,
            ']' => square = (square - 1).max(0),
            '(' => round += 1,
            ')' => round = (round - 1).max(0),
            // Точка с запятой кончает только AT-правило-предложение. У
            // обычного правила она — часть преамбулы до `{` (css-syntax-3
            // §5.4.3 «consume a qualified rule»): `test; @charset "x";
            // .a, #b { color: red }` — ОДНО правило с негодным селектором, и
            // отбрасывается оно целиком (`at-charset-039`). Прежде `test;`
            // обрывалось на месте, и красное правило оживало.
            ';' if square == 0 && round == 0 && statement_head(text) => {
                let head = &text[..at];
                return Some((Piece::Statement { head }, &text[at + 1..]));
            }
            '{' if square == 0 && round == 0 => {
                let head = &text[..at];
                let rest = &text[at..];
                let (body, tail) = match find_matching(rest) {
                    Some(close) => (&rest[1..close], &rest[close + 1..]),
                    // Незакрытый блок в КОНЦЕ таблицы закрывается неявно
                    // (§5.4.1): правило всё равно действует.
                    None => (&rest[1..], ""),
                };
                return Some((Piece::Block { head, body }, tail));
            }
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
}

/// Есть ли в значении незакавыченная запись `url(…)` с негодным содержимым.
fn has_bad_url(value: &str) -> bool {
    let mut at = 0usize;
    while at < value.len() {
        let ch = value[at..].chars().next().unwrap_or('\0');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += value[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&value[at..], ch);
                continue;
            }
            _ if at_url(&value[at..]) => {
                if url_is_bad(&value[at..]) {
                    return true;
                }
                at += skip_url(&value[at..]);
                continue;
            }
            _ => at += ch.len_utf8(),
        }
    }
    false
}

/// Годен ли url-токен, начавшийся здесь (§4.3.6).
///
/// Незакавыченное содержимое портят кавычка, открывающая скобка, знак
/// управления и непробельный знак после пробела в середине; закавыченная
/// форма — функция со строкой, к токену не относится. Обрыв на конце файла
/// токен НЕ портит.
fn url_is_bad(text: &str) -> bool {
    let mut at = 4; // `url(`
    let bytes = text.as_bytes();
    while at < bytes.len() && bytes[at].is_ascii_whitespace() {
        at += 1;
    }
    if matches!(bytes.get(at), Some(b'"') | Some(b'\'')) {
        return false;
    }
    let mut ws_seen = false;
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap_or('\0');
        match ch {
            ')' => return false,
            '\\' => {
                at += ch.len_utf8();
                at += text[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            c if c.is_ascii_whitespace() => ws_seen = true,
            '"' | '\'' | '(' => return true,
            c if (c as u32) < 0x20 || c as u32 == 0x7f => return true,
            _ if ws_seen => return true,
            _ => {}
        }
        at += ch.len_utf8();
    }
    false
}

/// Начинается ли здесь запись `url(`.
pub(crate) fn at_url(text: &str) -> bool {
    // Сравнение по БАЙТАМ: срез по четвёртому байту может разрезать
    // многобайтовый знак, и обычный срез строки на этом падает.
    let b = text.as_bytes();
    b.len() >= 4 && b[..4].eq_ignore_ascii_case(b"url(")
}

/// Где кончается запись `url(…)`, считая от `u`.
///
/// Незакавыченное содержимое — отдельный вид токена (§4.3.6): фигурная
/// скобка, точка с запятой и начало комментария внутри него ничего не значат.
/// Пока запись разбиралась как обычный текст, `url( { test )` открывал блок,
/// и остаток таблицы съезжал (`uri-012`).
pub(crate) fn skip_url(text: &str) -> usize {
    let mut at = 4; // `url(`
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += text[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&text[at..], ch);
                continue;
            }
            ')' => return at + ch.len_utf8(),
            _ => at += ch.len_utf8(),
        }
    }
    text.len()
}

/// Где кончается строка в кавычках, начавшаяся на `quote`.
///
/// Внутри неё не значат ничего ни скобки, ни точка с запятой, ни начало
/// комментария (CSS Syntax §4.3.5): `content: "}"` не закрывает правило, а
/// `content: "a;b"` — одно объявление. Обратный слэш снимает особость
/// следующего знака, в том числе самой кавычки.
pub(crate) fn skip_string(text: &str, quote: char) -> usize {
    let mut it = text.char_indices();
    while let Some((i, ch)) = it.next() {
        if ch == '\\' {
            it.next();
            continue;
        }
        if ch == quote {
            return i + ch.len_utf8();
        }
        // Незакрытая строка обрывается на переводе строки (§4.3.4): дальше
        // идёт обычный текст, а не бесконечная строка до конца таблицы.
        if ch == '\n' {
            return i;
        }
    }
    text.len()
}

/// Индекс `}` , парный первой `{`.
fn find_matching(from_brace: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut square = 0i32;
    let mut round = 0i32;
    let bytes = from_brace.as_bytes();
    let mut at = 0usize;
    while at < bytes.len() {
        let ch = from_brace[at..].chars().next().unwrap_or('\0');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += from_brace[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&from_brace[at..], ch);
                continue;
            }
            _ if at_url(&from_brace[at..]) => {
                at += skip_url(&from_brace[at..]);
                continue;
            }
            // Внутри `[` и `(` фигурная скобка ничего не закрывает.
            '[' => square += 1,
            ']' => square = (square - 1).max(0),
            '(' => round += 1,
            ')' => round = (round - 1).max(0),
            '{' if square == 0 && round == 0 => depth += 1,
            '}' if square == 0 && round == 0 => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
}

pub(crate) fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut at = 0usize;
    while at < css.len() {
        let ch = css[at..].chars().next().unwrap_or('\0');
        // Кавычки и экранирование сильнее комментария: `content: "/*"` — это
        // текст, а не начало комментария до конца таблицы.
        match ch {
            '\\' => {
                let next = css[at + ch.len_utf8()..].chars().next();
                out.push(ch);
                if let Some(n) = next {
                    out.push(n);
                    at += ch.len_utf8() + n.len_utf8();
                } else {
                    at += ch.len_utf8();
                }
                continue;
            }
            '"' | '\'' => {
                let body = at + ch.len_utf8();
                let end = body + skip_string(&css[body..], ch);
                out.push_str(&css[at..end]);
                at = end;
                continue;
            }
            _ if at_url(&css[at..]) => {
                let end = at + skip_url(&css[at..]);
                out.push_str(&css[at..end]);
                at = end;
                continue;
            }
            _ => {}
        }
        if css[at..].starts_with("/*") {
            // На месте комментария остаётся ПРОБЕЛ: разбор идёт по строке, а
            // не по разборным единицам, и без него соседние значения
            // склеивались в одно (`hsla(120/* … */75%/* … */50%)` выходило
            // `hsla(12075%50%)` — цвет пропадал целиком).
            out.push(' ');
            match css[at + 2..].find("*/") {
                Some(end) => at += 2 + end + 2,
                // Незакрытый комментарий тянется до конца файла (§4.3.2).
                None => return out,
            }
            continue;
        }
        out.push(ch);
        at += ch.len_utf8();
    }
    out
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

/// Разрезание по разделителю, не заходя внутрь скобок: `rgba(0, 0, 0, .5)`
/// содержит запятые, а `grid-template: repeat(2, 1fr)` — и запятые, и скобки.
fn split_top_level(raw: &str, sep: char) -> Vec<&str> {
    let mut out = vec![];
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut at = 0usize;
    while at < raw.len() {
        let ch = raw[at..].chars().next().unwrap_or('\0');
        match ch {
            // Экранированный разделитель разделителем не служит:
            // `background: red\;` — одно объявление со значением `red;`.
            '\\' => {
                at += ch.len_utf8();
                at += raw[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&raw[at..], ch);
                continue;
            }
            _ if at_url(&raw[at..]) => {
                at += skip_url(&raw[at..]);
                continue;
            }
            // Блоки ЛЮБОГО вида непрозрачны: объявление с фигурными скобками
            // внутри (`test { :nested; color: yellow }`) — одно объявление, и
            // недействительное. Пока считались только круглые скобки, его
            // внутренности разбирались как отдельные объявления и применялись
            // (`core-syntax-001`).
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            c if c == sep && depth == 0 => {
                out.push(&raw[start..at]);
                start = at + ch.len_utf8();
            }
            _ => {}
        }
        at += ch.len_utf8();
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
    fn layer_block_is_transparent_and_supports_not_is_honoured() {
        let media = Media::default();
        // Слой — обёртка: правило внутри него живёт.
        let rules = parse_stylesheet_media("@layer base { p { color: red } }", media);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].decls.get("color").map(String::as_str), Some("red"));
        // Отрицание — запасная ветка для движка БЕЗ поддержки; применять её
        // нельзя, иначе применяются обе ветки пары сразу.
        let neg =
            parse_stylesheet_media("@supports not (display: grid) { p { color: red } }", media);
        assert!(neg.is_empty());
        let pos = parse_stylesheet_media("@supports (display: grid) { p { color: red } }", media);
        assert_eq!(pos.len(), 1);
    }

    #[test]
    fn important_stays_in_the_value_for_the_cascade() {
        // Раньше тест закреплял обратное — что пометка срезается при разборе.
        // Именно из-за этого важность не работала вовсе: до каскада значение
        // доезжало неотличимым от обычного. Снимает пометку тот, кто
        // раскладывает каскад (`Computed::resolve_with_vars`).
        let d = parse_decls("color: red !important");
        assert_eq!(d.get("color").map(String::as_str), Some("red !important"));
    }

    #[test]
    fn bang_that_is_not_important_kills_the_declaration() {
        // После восклицательного знака стоит ровно `important` (CSS 2.1
        // §4.1.8); всё прочее делает объявление недействительным целиком.
        let d = parse_decls("color: red ! fail; background: green");
        assert_eq!(d.get("color"), None);
        assert_eq!(d.get("background").map(String::as_str), Some("green"));
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
        // Атрибутные селекторы теперь разбираются.
        let attr = Selector::parse("a[href]").expect("наличие атрибута");
        assert_eq!(attr.attrs.len(), 1);
        assert!(attr.attrs[0].matches(Some("x")));
        assert!(!attr.attrs[0].matches(None));
        let eq = Selector::parse("input[type=\"text\" i]").expect("значение");
        assert!(eq.attrs[0].ci);
        assert!(eq.attrs[0].matches(Some("TEXT")));
        assert!(!eq.attrs[0].matches(Some("password")));
        let lang = Selector::parse("[lang|=en]").expect("дефисное");
        assert!(lang.attrs[0].matches(Some("en-US")));
        assert!(!lang.attrs[0].matches(Some("ent")));
        // Псевдоэлемент разбирается: коробку из него строит `dom.rs`.
        assert_eq!(
            Selector::parse("li::before").and_then(|s| s.pseudo),
            Some("before".to_string())
        );
        // Соседние комбинаторы разбираются: `+` — смежный, `~` — любой раньше.
        let adj = Selector::parse("h1 + p").expect("смежный сосед");
        assert_eq!(adj.tag.as_deref(), Some("p"));
        let prev = adj.prev.expect("сосед");
        assert_eq!(prev.0.tag.as_deref(), Some("h1"));
        assert!(prev.1, "`+` — смежный");
        let anywhere = Selector::parse("img ~ img").expect("общий сосед");
        assert!(!anywhere.prev.expect("сосед").1, "`~` — любой раньше");
        // Смешанная цепочка: предмет `.c`, его сосед `.b`, предок соседа `.a`.
        let mixed = Selector::parse(".a > .b + .c").expect("цепочка");
        assert_eq!(mixed.classes, vec!["c".to_string()]);
        let b = mixed.prev.expect("сосед");
        assert_eq!(b.0.classes, vec!["b".to_string()]);
        assert_eq!(
            b.0.ancestor.as_ref().expect("предок").0.classes,
            vec!["a".to_string()]
        );
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
    parse_keyframes_in(css, None)
}

/// Лежит ли место `at` таблицы внутри группы `@media`/`@supports`, чьё
/// условие ЛОЖНО (css-conditional-3 §2: правила такой группы не действуют —
/// и `@font-face`, и `@keyframes`, а не только правила стиля:
/// `at-media-content-002/003`, `at-supports-content-002/003`).
/// Стек заголовков открытых блоков считается по скобкам, строки пропускаются.
pub(crate) fn in_false_group(css: &str, at: usize, media: Media) -> bool {
    let end = at.min(css.len());
    let b = css.as_bytes();
    let mut heads: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < end {
        match b[i] {
            q @ (b'"' | b'\'') => {
                i += 1;
                i += skip_string(&css[i..], q as char);
                continue;
            }
            b'{' => {
                heads.push((start, i));
                start = i + 1;
            }
            b'}' => {
                heads.pop();
                start = i + 1;
            }
            b';' => start = i + 1,
            _ => {}
        }
        i += 1;
    }
    heads.iter().any(|&(s, e)| {
        let head = css[s..e].trim();
        let low = head.to_ascii_lowercase();
        (low.starts_with("@media") && !media.matches(&low))
            || (low.starts_with("@supports") && !supports(&head[9..]))
    })
}

/// Наборы кадров таблицы. `media` — условия окружения: с ними кадры ложной
/// группы не берутся; `None` (лист агента) — берутся все, как прежде.
pub fn parse_keyframes_in(css: &str, media: Option<Media>) -> HashMap<String, Keyframes> {
    let cleaned = strip_comments(css);
    let mut out: HashMap<String, Keyframes> = HashMap::new();
    let mut rest = cleaned.as_str();
    while let Some(at) = rest.find("@keyframes") {
        let pos = cleaned.len() - rest.len() + at;
        let skip = media.is_some_and(|m| in_false_group(&cleaned, pos, m));
        rest = &rest[at + "@keyframes".len()..];
        let Some(brace) = rest.find('{') else { break };
        let name = rest[..brace].trim().to_string();
        let Some(close) = find_matching(&rest[brace..]) else {
            break;
        };
        let body = &rest[brace + 1..brace + close];
        rest = &rest[brace + close + 1..];
        if skip {
            continue;
        }

        let mut frames: Keyframes = vec![];
        let mut inner = body;
        while let Some(b) = inner.find('{') {
            let stops = inner[..b].trim();
            let Some(c) = find_matching(&inner[b..]) else {
                break;
            };
            // Объявление кадра с `!important` игнорируется ЦЕЛИКОМ
            // (css-animations-1 §3: «declarations in a keyframe rule that are
            // qualified with !important are ignored»). Отсекается ДО разбора:
            // `parse_decls` оставляет из двух одноимённых важное, и обычное
            // `border-color: green` того же кадра пропадало вместе с ним
            // (`important-prop`).
            let plain: Vec<&str> = split_top_level(&inner[b + 1..b + c], ';')
                .into_iter()
                .filter(|d| top_level_bang(d.trim()).is_none())
                .collect();
            let decls = parse_decls(&plain.join(";"));
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

#[cfg(test)]
mod media_probe {
    use super::*;

    /// `0px` кончается на `x`, как короткая запись разрешения `dppx`.
    /// Жадное отрезание давало «0p», разбор проваливался, и вся фича
    /// молча становилась `<general-enclosed>`.
    #[test]
    fn media_without_space_before_paren() {
        let m = Media::default();
        assert!(m.matches("@media(min-width:0px)"), "no-space form");
        assert!(m.matches("@media (min-width:0px)"), "spaced form");
        assert!(m.matches("@media"), "empty query");
    }
}
