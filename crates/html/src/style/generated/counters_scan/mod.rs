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

mod values;
mod walk;
use crate::style::generated::counters_scan::values::{decl_has, decl_value};

use crate::style::computed::{Computed, Display};
use crate::style::css::{Rule, VarMap};
use crate::style::select::{Ancestor, Sibs, Spot, ancestor_of};
use markup5ever_rcdom::{Handle, NodeData};
mod style;
use style::{pseudo_style, scan_style};

/// Состояние обхода: набранная сумма и последний ненулевой шаг.
struct Scan<'a> {
    tree: &'a crate::dom::tree_index::TreeIndex,
    vcx: crate::document::state::ValueCx<'a>,
    rules: &'a [Rule],
    vars: &'a VarMap,
    name: &'a str,
    num: i64,
    last: i64,
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
            Some(v) => v.saturating_neg(),
            // Неявный шаг пункта у обратного счётчика равен −1, значит его
            // отрицание — плюс единица.
            None if self.name == "list-item" && is_item => 1,
            None => 0,
        };
        if neg != 0 {
            self.last = neg;
        }
        if let Some(set) = decl_value(&style.counter_set, self.name, 0) {
            self.num = self.num.saturating_add(set);
            self.done = true;
            return;
        }
        self.num = self.num.saturating_add(neg);
    }

    fn enter(
        &mut self,
        h: &Handle,
        me: &Ancestor,
        path: &[Ancestor],
        sibs: Sibs,
        root: bool,
    ) -> Option<Ancestor> {
        let vcx = self.vcx;
        if self.done {
            return None;
        }
        let tag = tag_of(h);
        if crate::dom::DROP_TAGS.contains(&tag.as_str()) {
            return None;
        }
        let Some(style) = scan_style(h, me, path, sibs, self.rules, self.vars, vcx) else {
            return None;
        };
        let mut computed_me = me.clone();
        crate::dom::content::snapshot_generated(&mut computed_me, &style, path.last());
        let me = &computed_me;
        // Узел без коробки счётчиков не трогает (css-lists-3
        // §counters-without-boxes).
        if style.display == Some(Display::None) && style.col_role.is_none() {
            return None;
        }
        if !root && instantiates(&style, h, self.name) {
            // A sibling reset obscures this counter for all following siblings
            // (§4.3); a descendant reset only excludes its own subtree.
            // Blink core/css/counters_attachment_context.cc:159-169.
            self.done |= path.len() == self.scope_depth;
            return None;
        }
        // CSS Lists 3 §4.6: only a box with `display: list-item` increments
        // `list-item`; an `li` keeps that role only while no author display
        // replaces the UA `list-item` (`li { display: block }`).
        let is_item =
            (tag == "li" && style.display.is_none()) || style.display == Some(Display::ListItem);
        self.step(&style, is_item);
        // CSS Containment 2 §3.4: descendant increments create local counters;
        // they cannot contribute to an outer reversed counter's initial value.
        if style.contain_style == Some(true) && style.display != Some(Display::Contents) {
            return None;
        }
        // Порядок как в отрисовке: `::before`, дети, `::after`.
        if let Some(st) = pseudo_style(me, path, Sibs::EMPTY, "before", self.rules, self.vars, vcx)
        {
            self.step(&st, false);
        }
        Some(computed_me)
    }
}

/// Начальное значение обратного счётчика, созданного на этом узле.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reversed_initial(
    tree: &crate::dom::tree_index::TreeIndex,
    rules: &[Rule],
    vars: &VarMap,
    name: &str,
    creator: &Handle,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    level: &[Handle],
    spots: &[Spot],
    pos: usize,
    vcx: crate::document::state::ValueCx<'_>,
) -> i64 {
    let mut scan = Scan {
        tree,
        vcx,
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
            let Some(a) = ancestor_of(sib, *spot, tree) else {
                continue;
            };
            scan.node(sib, &a, path, sibs.at(idx), false);
            if scan.done {
                break;
            }
        }
    }
    scan.num.saturating_add(scan.last)
}
