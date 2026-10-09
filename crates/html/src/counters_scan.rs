//! Начальное значение обратного счётчика: предварительный обход области.
//!
//! `counter-reset: reversed(имя)` без числа означает «столько, сколько в
//! области шагов» (css-lists-3 §instantiating-counters): значение нужно ЗНАТЬ
//! в момент создания счётчика, а элементы, которые его двигают, идут дальше
//! по документу. Поэтому область обходится заранее — отдельным лёгким
//! проходом, который разрешает стиль, но ничего не рисует.
//!
//! Вложенная область исключается из подсчёта; сброс на последующем брате
//! завершает область создателя (css-lists-3 §4.3, §4.4.2).

#[path = "counters_scan_values.rs"]
mod values;
use values::{decl_has, decl_value};

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
    scope_depth: usize,
}

/// Имя тега узла.
fn tag_of(h: &Handle) -> String {
    match &h.data {
        NodeData::Element { name, .. } => name.local.to_string(),
        _ => String::new(),
    }
}

/// Whether this node establishes a separate scope for this counter.
fn instantiates(style: &Computed, h: &Handle, name: &str) -> bool {
    if style.display == Some(Display::Contents) {
        return false;
    }
    if decl_has(&style.counter_reset, name) {
        return true;
    }
    // Списочный контейнер заводит `list-item` правилом таблицы агента — но
    // только пока авторский `counter-reset` его не перебил.
    name == "list-item"
        && style.counter_reset.is_none()
        && matches!(tag_of(h).as_str(), "ol" | "ul" | "menu" | "dir")
}

/// Resolve counter directives and display with the rendering cascade.
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
        .find(|a| &*a.name.local == "style")
        .map(|a| crate::css::parse_decls(&a.value))
        .unwrap_or_default();
    let mut matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| crate::dom::matches(&r.sel, me, path, sibs))
        .collect();
    let mut style = Computed::resolve_with_vars(&mut matched, &inline_decls, vars);
    crate::dom::inherit_counter_decls(&mut style, path.last().map(|p| &p.counter_style));
    crate::dom::apply_value_hint(&mut style, me);
    Some(style)
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
    let mut style = Computed::resolve_with_vars(&mut matched, &Decls::new(), vars);
    crate::dom::inherit_counter_decls(&mut style, Some(&me.counter_style));
    // Без содержимого коробки нет, а значит нет и счётчиков.
    (crate::dom::host_content(&style, me).is_some() && style.display != Some(Display::None))
        .then_some(style)
}

impl Scan<'_> {
    /// Шаг алгоритма для одного узла: сумма отрицаний увеличений, последний
    /// ненулевой шаг и обрыв на `counter-set`.
    fn step(&mut self, style: &Computed, is_item: bool) {
        // CSS Lists 3 §4.4.2: the first counter-set ends the scan, including
        // that element's pseudo-elements and ancestors' trailing pseudos.
        if self.done || style.display == Some(Display::Contents) {
            return;
        }
        let neg = match decl_value(&style.counter_increment, self.name, 1) {
            Some(v) => -v,
            // Неявный шаг пункта у обратного счётчика равен −1, значит его
            // отрицание — плюс единица.
            None if self.name == "list-item" && is_item => 1,
            None => 0,
        };
        if neg != 0 {
            self.last = neg;
        }
        if let Some(set) = decl_value(&style.counter_set, self.name, 0) {
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
        let mut computed_me = me.clone();
        computed_me.counter_style = crate::dom::counter_snapshot(&style);
        let me = &computed_me;
        // Узел без коробки счётчиков не трогает (css-lists-3
        // §counters-without-boxes).
        if style.display == Some(Display::None) && style.col_role.is_none() {
            return;
        }
        if !root && instantiates(&style, h, self.name) {
            // A sibling reset obscures this counter for all following siblings
            // (§4.3); a descendant reset only excludes its own subtree.
            // Blink core/css/counters_attachment_context.cc:159-169.
            self.done |= path.len() == self.scope_depth;
            return;
        }
        let is_item = tag == "li" || style.display == Some(Display::ListItem);
        self.step(&style, is_item);
        // CSS Containment 2 §3.4: descendant increments create local counters;
        // they cannot contribute to an outer reversed counter's initial value.
        if style.contain_style == Some(true) && style.display != Some(Display::Contents) {
            return;
        }
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
                    rc: None,
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
) -> i32 {
    let mut scan = Scan {
        rules,
        vars,
        name,
        num: 0,
        last: 0,
        done: false,
        scope_depth: path.len(),
    };
    scan.node(creator, me, path, sibs, true);
    // Following siblings remain in scope until another reset obscures it.
    if !scan.done {
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
