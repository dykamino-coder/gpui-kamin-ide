//! Начальное значение обратного счётчика: предварительный обход области.
//!
//! `counter-reset: reversed(имя)` без числа означает «столько, сколько в
//! области шагов» (css-lists-3 §instantiating-counters): значение нужно ЗНАТЬ
//! в момент создания счётчика, а элементы, которые его двигают, идут дальше
//! по документу. Поэтому область обходится заранее — отдельным лёгким
//! проходом, который разрешает стиль, но ничего не рисует.
//!
//! От Blink (`counters_attachment_context.cc`
//! `CalculateInitialValueForReversed`) отличаемся тремя местами, и каждое
//! проверено парой набора: свой `counter-increment` создатель учитывает
//! (Blink его пропускает — `counter-reset-reversed-pseudo-003`), пунктом
//! считается всякий `display: list-item`, а не только `<li>`, и неявный
//! счётчик списочного контейнера в счёт входит.

use crate::computed::{Computed, Display};
use crate::css::{Decls, Rule};
use crate::dom::{Ancestor, Sibs, Spot, ancestor_of, census_of, matches_ignoring_pseudo};
use markup5ever_rcdom::{Handle, NodeData};

/// Состояние обхода: набранная сумма и последний ненулевой шаг.
struct Scan<'a> {
    rules: &'a [Rule],
    vars: &'a Decls,
    name: &'a str,
    num: i64,
    last: i32,
    done: bool,
}

/// Значение директивы для нашего имени: у `counter-increment: a 2 b` для
/// имени `a` это 2, для `b` — единица по умолчанию.
fn decl_value(decl: &Option<String>, name: &str) -> Option<i32> {
    let text = decl.as_deref()?;
    let mut it = text.split_whitespace().peekable();
    let mut found = None;
    while let Some(word) = it.next() {
        let value = match it.peek().and_then(|n| n.parse::<i32>().ok()) {
            Some(v) => {
                it.next();
                v
            }
            None => 1,
        };
        if word == name {
            found = Some(value);
        }
    }
    found
}

/// Названо ли имя в объявлении сброса — в том числе обратной записью.
fn decl_has(decl: &Option<String>, name: &str) -> bool {
    let reversed = format!("reversed({name})");
    decl.as_deref().is_some_and(|t| {
        t.split_whitespace()
            .any(|w| w == name || w == reversed.as_str())
    })
}

/// Имя тега узла.
fn tag_of(h: &Handle) -> String {
    match &h.data {
        NodeData::Element { name, .. } => name.local.to_string(),
        _ => String::new(),
    }
}

/// Заводит ли узел счётчик этого имени — тогда его поддерево лежит в СВОЕЙ
/// области и в чужой счёт не входит.
fn instantiates(style: &Computed, h: &Handle, name: &str) -> bool {
    if decl_has(&style.counter_reset, name) {
        return true;
    }
    // Списочный контейнер заводит `list-item` правилом таблицы агента — но
    // только пока авторский `counter-reset` его не перебил.
    name == "list-item"
        && style.counter_reset.is_none()
        && matches!(tag_of(h).as_str(), "ol" | "ul" | "menu" | "dir")
}

/// Стиль узла для обхода: тот же каскад, что и в отрисовке, но без
/// презентационных атрибутов, слоя наведения и псевдострок — обходу нужны
/// только директивы счётчиков и `display`.
fn scan_style(
    h: &Handle,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    rules: &[Rule],
    vars: &Decls,
) -> Option<Computed> {
    let NodeData::Element { attrs, .. } = &h.data else {
        return None;
    };
    let inline_decls: Decls = attrs
        .borrow()
        .iter()
        .find(|a| a.name.local.as_ref() == "style")
        .map(|a| crate::css::parse_decls(&a.value))
        .unwrap_or_default();
    let mut matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| crate::dom::matches(&r.sel, me, path, sibs))
        .collect();
    Some(Computed::resolve_with_vars(
        &mut matched,
        &inline_decls,
        vars,
    ))
}

/// Стиль псевдоэлемента, если правила дают ему коробку.
fn pseudo_style(
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    which: &str,
    rules: &[Rule],
    vars: &Decls,
) -> Option<Computed> {
    let mut matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref() == Some(which))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    if matched.is_empty() {
        return None;
    }
    let style = Computed::resolve_with_vars(&mut matched, &Decls::new(), vars);
    // Без содержимого коробки нет, а значит нет и счётчиков.
    (style.content.is_some() && style.display != Some(Display::None)).then_some(style)
}

impl Scan<'_> {
    /// Шаг алгоритма для одного узла: сумма отрицаний увеличений, последний
    /// ненулевой шаг и обрыв на `counter-set`.
    fn step(&mut self, style: &Computed, is_item: bool) {
        let neg = match decl_value(&style.counter_increment, self.name) {
            Some(v) => -v,
            // Неявный шаг пункта у обратного счётчика равен −1, значит его
            // отрицание — плюс единица.
            None if self.name == "list-item" && is_item => 1,
            None => 0,
        };
        if neg != 0 {
            self.last = neg;
        }
        if let Some(set) = decl_value(&style.counter_set, self.name) {
            self.num += i64::from(set);
            self.done = true;
            return;
        }
        self.num += i64::from(neg);
    }

    fn node(&mut self, h: &Handle, me: &Ancestor, path: &[Ancestor], sibs: Sibs, root: bool) {
        if self.done {
            return;
        }
        let tag = tag_of(h);
        if crate::dom::DROP_TAGS.contains(&tag.as_str()) {
            return;
        }
        let Some(style) = scan_style(h, me, path, sibs, self.rules, self.vars) else {
            return;
        };
        // Узел без коробки счётчиков не трогает (css-lists-3
        // §counters-without-boxes).
        if style.display == Some(Display::None) {
            return;
        }
        if !root && instantiates(&style, h, self.name) {
            // Чужая область: её поддерево в наш счёт не входит вовсе.
            return;
        }
        let is_item = tag == "li" || style.display == Some(Display::ListItem);
        self.step(&style, is_item);
        // Порядок как в отрисовке: `::before`, дети, `::after`.
        if let Some(st) = pseudo_style(me, path, Sibs::EMPTY, "before", self.rules, self.vars) {
            self.step(&st, false);
        }
        {
            let children = h.children.borrow();
            let (spots, all) = census_of(&children);
            let mut inner_path: Vec<Ancestor> = path.to_vec();
            inner_path.push(me.clone());
            let mut pos = 0usize;
            for (child, spot) in children.iter().zip(&spots) {
                if spot.index == 0 {
                    continue;
                }
                let kid = Sibs {
                    all: &all,
                    pos,
                    is_elem: true,
                };
                self.node(child, &all[pos], &inner_path, kid, false);
                pos += 1;
                if self.done {
                    break;
                }
            }
        }
        if let Some(st) = pseudo_style(me, path, Sibs::EMPTY, "after", self.rules, self.vars) {
            self.step(&st, false);
        }
    }
}

/// Начальное значение обратного счётчика, созданного на этом узле.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reversed_initial(
    rules: &[Rule],
    vars: &Decls,
    name: &str,
    creator: &Handle,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    level: &[Handle],
    spots: &[Spot],
    pos: usize,
    escapes: bool,
) -> i32 {
    let mut scan = Scan {
        rules,
        vars,
        name,
        num: 0,
        last: 0,
        done: false,
    };
    scan.node(creator, me, path, sibs, true);
    // Область счётчика включает последующих братьев создателя — но только
    // если запись переживёт выход из него (см. `Counters::escapes_creator`).
    if escapes && !scan.done {
        let mut idx = sibs.pos;
        for (i, sib) in level.iter().enumerate().skip(pos + 1) {
            let Some(spot) = spots.get(i) else { continue };
            if spot.index == 0 {
                continue;
            }
            idx += 1;
            let Some(a) = ancestor_of(sib, *spot) else {
                continue;
            };
            scan.node(sib, &a, path, sibs.at(idx), false);
            if scan.done {
                break;
            }
        }
    }
    (scan.num + i64::from(scan.last)).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
