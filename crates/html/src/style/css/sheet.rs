//! Таблица стилей: правила, каскадные слои, пространства имён.

use crate::style::css::*;

thread_local! {
    /// Реестр слоёв документа: полное имя → путь индексов (порядок —
    /// по ПЕРВОМУ объявлению, css-cascade-5 §6.4.3).
    pub(crate) static LAYERS: std::cell::RefCell<HashMap<String, Vec<u32>>> =
        std::cell::RefCell::new(HashMap::new());
    /// Следующий индекс ребёнка у каждого родителя (ключ — полное имя).
    pub(crate) static LAYER_NEXT: std::cell::RefCell<HashMap<String, u32>> =
        std::cell::RefCell::new(HashMap::new());
    /// Текущий слой разбора: полное имя и путь.
    pub(crate) static LAYER_NOW: std::cell::RefCell<(String, Vec<u32>)> =
        const { std::cell::RefCell::new((String::new(), Vec::new())) };
    pub(crate) static LAYER_ANON: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Сбросить реестр слоёв — на входе разбора документа.
pub fn reset_layers() {
    crate::fonts::alternates::reset();
    LAYERS.with(|l| l.borrow_mut().clear());
    LAYER_NEXT.with(|l| l.borrow_mut().clear());
    LAYER_NOW.with(|l| *l.borrow_mut() = (String::new(), Vec::new()));
    LAYER_ANON.with(|c| c.set(0));
}

/// Путь слоя по имени (возможно с точками) внутри текущего; регистрирует
/// незнакомые звенья. Пустое имя — анонимный слой, всегда новый.
pub(crate) fn layer_enter_path(name: &str) -> (String, Vec<u32>) {
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
pub(crate) fn layer_of_rules() -> Vec<u32> {
    let mut p = LAYER_NOW.with(|l| l.borrow().1.clone());
    p.push(u32::MAX);
    p
}

pub fn parse_stylesheet(css: &str) -> Vec<Rule> {
    parse_stylesheet_media(css, Media::default())
}

thread_local! {
    /// Префиксы `@namespace` разбираемой таблицы; `None` — разбор идёт не из
    /// таблицы, и префиксы не проверяются (прежнее поведение).
    pub(crate) static NS_PREFIXES: std::cell::RefCell<Option<std::collections::HashSet<String>>> =
        const { std::cell::RefCell::new(None) };
}

/// Снимает префиксы, когда верхний вызов разбора таблицы кончился (и при панике).
pub(crate) struct NsScope(bool);

impl Drop for NsScope {
    fn drop(&mut self) {
        if self.0 {
            NS_PREFIXES.with(|n| *n.borrow_mut() = None);
        }
    }
}

/// Объявлен ли префикс. Пустой (`|div`) и `*` объявлены всегда.
pub(crate) fn ns_declared(ns: &str) -> bool {
    ns.is_empty()
        || ns == "*"
        || NS_PREFIXES.with(|n| n.borrow().as_ref().is_none_or(|s| s.contains(ns)))
}

/// Префиксы действительных `@namespace` (css-namespaces-3 §2): «must follow
/// all @charset and @import rules and precede all other non-ignored at-rules
/// and style rules … Otherwise the @namespace rule is invalid». Даже пустой
/// `@media {}` или `@supports (…) {}` закрывает пролог (`at-media-003`,
/// `at-supports-045`).
pub(crate) fn declared_prefixes(css: &str) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let cleaned = strip_comments(css);
    let mut rest = cleaned.as_str();
    loop {
        rest = stylesheet_tokens::start(rest);
        let Some((piece, tail)) = next_piece(rest) else { break };
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
pub(crate) fn attr_prefixes_declared(v: &str) -> bool {
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
    sheet_rules(css, media, top)
}

pub(crate) fn sheet_rules(css: &str, media: Media, top: bool) -> Vec<Rule> {
    let mut out = vec![];
    let cleaned = strip_comments(css);
    let mut rest = cleaned.as_str();
    let mut order = 0usize;
    loop {
        if top {
            rest = stylesheet_tokens::start(rest);
        }
        let Some((piece, tail)) = next_piece(rest) else { break };
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
            } else if name
                .strip_prefix("@font-feature-values")
                .is_some_and(|s| s.starts_with(char::is_whitespace))
            {
                crate::fonts::alternates::register(&head[20..], body, layer_of_rules());
                false
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
            } else if name
                .strip_prefix("@counter-style")
                .is_some_and(|s| s.starts_with(char::is_whitespace))
            {
                // css-counter-styles-3 §3: дескрипторы правила — в реестр
                // документа, повторы — по порядку записи (последнее
                // действительное побеждает). Имя — с исходным регистром.
                let decls = parse_decls(body);
                let descs: Vec<(String, Vec<String>)> = decls
                    .iter()
                    .filter(|(k, _)| k.as_str() != ORDER_KEY)
                    .map(|(k, v)| (k.clone(), v.split(DECL_SEP).map(str::to_string).collect()))
                    .collect();
                crate::counter_style_rules::register(&head["@counter-style".len()..], &descs);
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
