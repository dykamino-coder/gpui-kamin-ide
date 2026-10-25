//! Измеряемые части ребёнка стопки колонок: пагинация по раскладке (`paginate`), рекордер
//! главной копии с шириной её раскладки до укладки (`breaks_w`) и распорки роста по
//! раскладке копии (`push::Regrow`) с пересборкой копий из правленого дерева.

use super::build::build_copy;
use crate::dom::Element;
use crate::layout::fragment::line_shape::side_margin_wrap;
use crate::layout::fragment::paginate::Paginate;
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::fragment::push::{Regrow, measured_push};
use crate::layout::fragment::types::{BreaksAt, StackChild};
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;
use std::rc::Rc;

/// Что ребёнку стопки даёт раскладка его копии.
pub(super) struct Measured {
    pub(super) paginate: Option<Paginate>,
    pub(super) watched: Rc<Watched>,
    pub(super) breaks_w: Option<BreaksAt>,
    /// У ребёнка свой план фрагментов (клон украшений, целый или рядами
    /// вложенный многоколоночник).
    pub(super) own: bool,
    /// Высота ребёнка — его разложенный border box (`Regrow::laid_h`).
    pub(super) laid_h: bool,
    /// Ширина раскладки копии до укладки (`StackChild::breaks_w`).
    pub(super) push_w: Option<f32>,
}

impl Measured {
    /// Поставить измеряемые части в `child`. Распорки роста ставит раскладка
    /// копии ребёнку, чьи точки разрыва она даёт (`measure` или `breaks_w`
    /// с `measured_push`), без своего плана фрагментов и не размечаемому по
    /// сетке (`paginate`). `cols` — `(col_vert, col_rl, line_col_w)`,
    /// `sizes` — мера `(h, over)` для сборки копии.
    pub(super) fn attach(
        self,
        child: &mut StackChild,
        opts: &RenderOpts,
        cols: (bool, bool, Option<f32>),
        merged: &BoxStyle,
        copy: &Element,
        sizes: (f32, f32),
    ) {
        #[cfg(feature = "wpt-harness")]
        {
            child.probe = opts.frag.probe.clone();
        }
        let (col_vert, col_rl, line_col_w) = cols;
        child.breaks_w = self
            .breaks_w
            .filter(|_| child.measure.is_none() && !child.span);
        let regrows = !self.own
            && self.paginate.is_none()
            && self.watched.has_blocks()
            && (child.measure.is_some()
                || (child.breaks_w.is_some() && measured_push(copy, self.push_w, col_vert)));
        child.paginate = self.paginate;
        if regrows {
            let watched = self.watched.clone();
            let merged = merged.clone();
            let (h, over) = sizes;
            let build = move |dom: &Element, part: usize, opts_b: &RenderOpts| {
                let vcx = opts_b.cx.vcx();
                let mut inner = box_style(&merged, &dom.style, vcx);
                inner.fragment_clip_ancestor = false;
                let mut opts = opts_b.in_stack_copy();
                opts.frag.watch = Some(if part == 0 {
                    watched.clone()
                } else {
                    watched.for_part(part)
                });
                let el = build_copy(
                    &opts,
                    col_vert,
                    col_rl,
                    line_col_w,
                    &[],
                    None,
                    &merged,
                    dom,
                    h,
                    over,
                    None,
                    0.0,
                    &inner,
                    false,
                    part == 0,
                    part,
                    false,
                );
                side_margin_wrap(el, dom, col_vert)
            };
            child.regrow = Some(Regrow {
                axis: if col_vert {
                    if col_rl {
                        crate::layout::fragment::types::StackAxis::VerticalRl
                    } else {
                        crate::layout::fragment::types::StackAxis::VerticalLr
                    }
                } else {
                    crate::layout::fragment::types::StackAxis::Horizontal
                },
                dom: copy.clone(),
                source: copy.clone(),
                build: Rc::new(build),
                opts: opts.clone(),
                laid: Default::default(),
                floats: Vec::new(),
                float_leads: Vec::new(),
                space: None,
                laid_h: self.laid_h,
                watched: self.watched.clone(),
            });
        }
        child.watched = Some(self.watched);
    }
}
