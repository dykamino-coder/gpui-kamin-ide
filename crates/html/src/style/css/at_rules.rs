//! @page (селекторы страниц, поля), @property, @position-try.

use crate::style::css::*;

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
    pub decls: DeclBlock,
    /// Вложенные правила марджин-боксов (`@top-left { … }`, css-page-3
    /// §margin-at-rules): имя коробки в нижнем регистре → объявления.
    pub margins: Vec<(String, DeclBlock)>,
    /// Путь каскадного слоя правила (как `Rule::layer`; неслойное —
    /// `[u32::MAX]`): слой старше специфичности (`layers-002..004`).
    pub layer: Vec<u32>,
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

/// Дописать правило `@page` в реестр текущего документа: правила хранятся
/// в порядке появления, с псевдоклассами и именами; хост забирает их
/// `Document::page_rules` и собирает объявления листа `page_decls_in`.
pub(crate) fn push_page_rule(doc: &crate::document::state::DocState, rule: PageRule) {
    doc.page_rules.borrow_mut().push(rule);
}

/// Голова `@page` → список селекторов; `None` — голова неверна, и правило
/// отбрасывается целиком (css-page-3 §syntax-page-selector, как у обычного
/// списка селекторов). Имя регистрозависимо, псевдоклассы — нет.
pub(super) fn parse_page_selectors(head: &str) -> Option<Vec<PageSel>> {
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
/// имени) по правилам документа: каскад css-page-3 §cascading-and-page-context. Совпавшие правила
/// идут по возрастанию специфичности (f, g, h) — f: имя типа, g: `:first` и
/// `:blank`, h: `:left` и `:right`, — при равной по порядку записи; объявления
/// сливаются в этом порядке, последнее побеждает. Левые/правые — по
/// направлению прогрессии страниц: при ltr первый лист ПРАВЫЙ
/// (§page-selectors: «if the root element's direction is ltr, then the first
/// page is a right page»), при rtl — левый.
pub fn page_decls_in(rules: &[PageRule], index: usize, name: &str, rtl: bool) -> DeclBlock {
    let hits = matching_rules(rules, index, name, rtl);
    in_cascade_order(&hits, |i| vec![&rules[i].decls])
}

/// Марджин-боксы листа: каскад тот же, что у объявлений листа
/// (`page_decls_in`), объявления каждой коробки сливаются по возрастанию
/// специфичности. Порядок — `MARGIN_BOXES`; коробки без правил не попадают.
pub fn page_margins_in(
    rules: &[PageRule],
    index: usize,
    name: &str,
    rtl: bool,
) -> Vec<(String, DeclBlock)> {
    let hits = matching_rules(rules, index, name, rtl);
    MARGIN_BOXES
        .iter()
        .filter_map(|b| {
            let list = in_cascade_order(&hits, |i| {
                rules[i]
                    .margins
                    .iter()
                    .filter(|(n, _)| n == b)
                    .map(|(_, d)| d)
                    .collect()
            });
            (!list.is_empty()).then(|| (b.to_string(), list))
        })
        .collect()
}

/// Совпавшее правило: (слой, специфичность (f, g, h)) и номер правила.
type PageHit<'r> = ((&'r [u32], (u8, u8, u8)), usize);

/// Объявления совпавших правил В ПОРЯДКЕ ПРИМЕНЕНИЯ, последнее побеждает:
/// сперва обычные по возрастанию (слой, специфичность, порядок), затем
/// важные — у них слой сравнивается в обратную сторону (css-cascade-5
/// §cascade-sort).
fn in_cascade_order<'d>(
    hits: &[PageHit<'_>],
    blocks: impl Fn(usize) -> Vec<&'d DeclBlock>,
) -> DeclBlock {
    let mut important: Vec<&PageHit<'_>> = hits.iter().collect();
    important.sort_by(|((la, sa), ia), ((lb, sb), ib)| {
        (std::cmp::Reverse(*la), sa, ia).cmp(&(std::cmp::Reverse(*lb), sb, ib))
    });
    let normal = hits.iter().map(|(_, i)| (*i, false));
    let mut out = DeclBlock::new();
    for (i, imp) in normal.chain(important.into_iter().map(|(_, i)| (*i, true))) {
        for block in blocks(i) {
            for d in block.iter().filter(|d| d.important == imp) {
                out.push(d.name.clone(), d.value.clone(), imp);
            }
        }
    }
    out
}

/// Совпавшие с листом правила по возрастанию (слой, специфичность,
/// порядок): css-cascade-5 §cascade-sort — каскадный слой решает раньше
/// специфичности, и `@page` внутри `@layer` подчиняется ему же.
fn matching_rules<'r>(
    rules: &'r [PageRule],
    index: usize,
    name: &str,
    rtl: bool,
) -> Vec<PageHit<'r>> {
    let right = index.is_multiple_of(2) != rtl;
    let mut hits: Vec<PageHit<'r>> = Vec::new();
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
            .map(|s| (s.name.is_some() as u8, s.first + s.blank, s.left + s.right))
            .max();
        if let Some(sp) = spec {
            hits.push(((r.layer.as_slice(), sp), order));
        }
    }
    hits.sort();
    hits
}

/// Записать правило `@position-try <dashed-ident> { … }`
/// (css-anchor-position-1 §fallback-rule) в реестр текущего документа:
/// имя → объявления, читается на сборке кадра (`anchor::place`). Повтор
/// имени перекрывает — «the last one in document order wins».
pub(crate) fn insert_try_rule(
    doc: &crate::document::state::DocState,
    name: &str,
    decls: DeclBlock,
) {
    doc.try_rules.borrow_mut().insert(name.to_string(), decls);
}

/// Зарегистрированное свойство `@property` (css-properties-values-api-1 §3):
/// синтаксис, наследуется ли, начальное значение.
#[derive(Clone, Debug)]
pub struct Registered {
    pub syntax: String,
    pub inherits: bool,
    pub initial: Option<String>,
}

/// Зарегистрировать `@property` в текущем документе; первая действительная
/// регистрация имени остаётся.
pub(crate) fn register_property(
    doc: &crate::document::state::DocState,
    name: &str,
    reg: Registered,
) {
    doc.property_rules
        .borrow_mut()
        .entry(name.to_string())
        .or_insert(reg);
}
