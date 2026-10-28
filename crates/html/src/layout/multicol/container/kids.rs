//! Дети стопки колонок: копии детей при заданной высоте и элементы StackChild.

use crate::dom::Element;
use crate::layout::fragment::push::{avoid_only_monolith, grow_pushed, measured_push};
use crate::layout::multicol::measure::kid_layout_w;
use crate::layout::multicol::stack_child::{multicol_stack_child, nest_candidate};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use crate::tolerance::COORD_EPS;

#[allow(clippy::too_many_arguments)]
// Child records keep their layout and paint callbacks with measured geometry.
#[allow(clippy::type_complexity)]
pub(in crate::layout::multicol) fn column_kids(
    _merged: &BoxStyle,
    kids: Vec<(Element, crate::layout::fragment::Shape)>,
    cols: u16,
    _used_gap: f32,
    col_vert: bool,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    fixed: Option<f32>,
    (line_col_w, layout_col_w): (Option<f32>, Option<f32>),
    opts: &RenderOpts,
) -> (
    Vec<Option<BoxStyle>>,
    Vec<usize>,
    Vec<crate::layout::fragment::types::Par>,
    Vec<(Element, crate::layout::fragment::Shape)>,
) {
    let vcx = opts.cx.frame();
    // Линии остаются в Taffy-дереве контейнера; копия сохраняет flex sizing,
    // фон и static position. Regrow читает их после layout.
    let n = kids.len();
    let kid_parent = vec![None; n];
    let kid_starts = (0..=n).collect();
    let kid_par = vec![crate::layout::fragment::types::Par::default(); n];
    // `break-inside: avoid` без настоящего монолита — у любого
    // ребёнка колонок (`flow::Par::avoid_only`): с верха колонки
    // коробка выше колонки рвётся, а не переполняет её.
    let mut kid_par = kid_par;
    for (p, (c, _)) in kid_par.iter_mut().zip(kids.iter()) {
        if p.group == 0 {
            p.avoid_only = c.style.used().break_inside_avoid && avoid_only_monolith(c);
            p.float = c.marks.float_block;
            p.clears = c.style.used().clear.is_some();
        }
    }
    let _ = (cols, rows, copies, fixed, line_col_w, layout_col_w, col_vert, vcx);
    (kid_parent, kid_starts, kid_par, kids)
}

/// Общие входы детей одной стопки: каждый ребёнок собирается из них заново,
/// когда раскладка предшествующих детей решает его внешнее пространство.
struct Shared {
    e: Element,
    merged: BoxStyle,
    opts: RenderOpts,
    col_vert: bool,
    col_rl: bool,
    widths: (Option<f32>, Option<f32>),
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    fixed: Option<f32>,
    kid_parent: Vec<Option<BoxStyle>>,
    kid_par: Vec<crate::layout::fragment::types::Par>,
    nested_whole: std::cell::RefCell<Vec<u64>>,
    measured_kids: std::cell::RefCell<Vec<(u64, f32)>>,
}

impl Shared {
    fn build(
        &self,
        ix: usize,
        c: &Element,
        prepared: &crate::layout::multicol::stack_child::Prepared,
        (mt, mb): (f32, f32),
        nest: Option<(f32, f32)>,
    ) -> crate::layout::fragment::types::StackChild {
        let (line_col_w, layout_col_w) = self.widths;
        multicol_stack_child(
            ix,
            c.clone(),
            prepared.clone(),
            mt,
            mb,
            &self.e,
            &self.merged,
            &self.opts,
            &self.kid_parent,
            &self.kid_par,
            self.col_vert,
            self.col_rl,
            line_col_w,
            kid_layout_w(c, line_col_w, layout_col_w),
            self.rows,
            self.copies,
            self.fixed,
            nest,
            &self.nested_whole,
            &self.measured_kids,
        )
    }
}

#[allow(clippy::too_many_arguments)]
// The stack is assembled from the same measured child/callback records.
#[allow(clippy::type_complexity)]
pub(in crate::layout::multicol) fn stack_children(
    e: &Element,
    merged: &BoxStyle,
    opts: &RenderOpts,
    cols: u16,
    col_vert: bool,
    col_rl: bool,
    widths: (Option<f32>, Option<f32>),
    rows: Option<crate::layout::fragment::types::Rows>,
    nested_whole: std::cell::RefCell<Vec<u64>>,
    measured_kids: std::cell::RefCell<Vec<(u64, f32)>>,
    copies: usize,
    fixed: Option<f32>,
    kid_parent: Vec<Option<BoxStyle>>,
    kid_par: Vec<crate::layout::fragment::types::Par>,
    kids: Vec<(Element, crate::layout::fragment::Shape)>,
    prepared: Vec<crate::layout::multicol::stack_child::Prepared>,
) -> Vec<crate::layout::fragment::types::StackChild> {
    // Единственный вложенный многоколоночник заданной высоты в балансируемых
    // колонках делит их на ряды своей высоты (`ColumnStack::resume_nested`).
    let balanced = fixed.is_none() && rows.is_none_or(|r| r.cap) && cols > 1 && kids.len() == 1;
    let shared = std::rc::Rc::new(Shared {
        e: e.clone(),
        merged: merged.clone(),
        opts: opts.clone(),
        col_vert,
        col_rl,
        widths,
        rows,
        copies,
        fixed,
        kid_parent,
        kid_par,
        nested_whole,
        measured_kids,
    });
    kids.into_iter()
        .zip(prepared)
        .enumerate()
        .map(|(ix, ((c, s), prepared))| {
            let margins = (s.margin_top, s.margin_bottom);
            let candidate = nest_candidate(ix, &shared.kid_par, col_vert, &c)
                && if balanced {
                    matches!(c.style.used().height, Some(Len::Px(_)))
                        && s.margin_top.abs() < COORD_EPS
                } else {
                    fixed.is_some_and(|h| h > 0.0) && rows.is_none()
                };
            let nest = fixed.filter(|_| candidate).map(|hh| (hh, 0.0));
            let mut child = shared.build(ix, &c, &prepared, margins, nest);
            if candidate {
                let shared = shared.clone();
                child.resume = Some(std::rc::Rc::new(move |nest| {
                    shared.build(ix, &c, &prepared, margins, nest)
                }));
            }
            child
        })
        .collect()
}
