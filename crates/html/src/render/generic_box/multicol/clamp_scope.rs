//! Многоколоночник внутри clamp-контейнера (css-overflow-4 §max-lines: «Only line boxes in the
//! same block formatting context are counted»; строки колонок лежат в независимых контекстах
//! и бюджет не расходуют). Сама коробка — поточный блок контекста контейнера: её проба границ
//! и скрытие за точкой среза те же, что у прочих независимых коробок общего пути
//! (`generic_children`), которого многоколоночные маршруты не проходят.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::render::generic_box::child_probes;
use crate::style::computed::BoxStyle;
use crate::text::clamp::{
    ClampSpan, clamp_context, clamp_next_seq, hide_box_after_point, open_clamp_span,
};
use gpui::ParentElement;

pub(super) struct ClampScope {
    outer: (u64, bool),
    inner: Box<RenderOpts>,
    span: Option<ClampSpan>,
}

impl ClampScope {
    /// Граница независимого контекста; `None` вне clamp-контейнера.
    pub(super) fn enter(opts: &RenderOpts) -> Option<Self> {
        let outer = clamp_context(&opts.cx)?;
        let inner = opts.inner(|cx| cx.clamp.build = cx.clamp.build.bfc());
        let span = open_clamp_span(&inner.cx);
        // Колонки строят свои абзацы лишь при раскладке, после всех
        // абзацев сборки: место коробки в порядке дерева держит номер,
        // взятый сейчас, — иначе следующий абзац получил бы тот же номер,
        // и коробку перед точкой среза нельзя было бы отличить от
        // коробки после неё.
        if span.is_some() {
            clamp_next_seq(outer.0, &inner.cx);
        }
        Some(Self { outer, inner, span })
    }

    /// Настройки потомков: их строки в счёт контейнера не входят.
    pub(super) fn opts(&self) -> &RenderOpts {
        &self.inner
    }

    /// Коробка многоколоночника в контексте контейнера: проба её границ и
    /// скрытие, когда она целиком следует за точкой среза (§5.3).
    pub(super) fn close(
        scope: Option<&Self>,
        d: gpui::Div,
        e: &Element,
        merged: &BoxStyle,
    ) -> gpui::Div {
        let Some(scope) = scope else { return d };
        let mut kids = Vec::new();
        child_probes(
            e,
            merged,
            &scope.inner,
            &mut kids,
            true,
            false,
            scope.span.clone(),
        );
        hide_box_after_point(
            d.children(kids),
            Some(scope.outer),
            scope.span.clone(),
            merged,
            false,
            &scope.inner.cx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Node;
    use crate::text::clamp::{ClampInput, ClampPlan};
    use gpui::Styled;
    use std::rc::Rc;

    fn opts(plan: Option<ClampPlan>) -> RenderOpts {
        let mut opts = RenderOpts {
            text: Default::default(),
            viewport: (800., 600.),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        opts.cx.clamp.build = opts.cx.clamp.build.container(5);
        opts.cx.clamp.input = plan.map(|p| ClampInput::Apply(Rc::new(p)));
        // Paragraph 0 of the container precedes the multicol.
        clamp_next_seq(5, &opts.cx);
        opts
    }

    #[test]
    fn multicol_lines_leave_the_count_and_its_box_keeps_its_tree_place() {
        let plain = RenderOpts {
            cx: Default::default(),
            ..opts(None)
        };
        assert!(ClampScope::enter(&plain).is_none());
        let outer = opts(None);
        let scope = ClampScope::enter(&outer).expect("inside a clamp container");
        assert_eq!(clamp_context(&scope.opts().cx), Some((5, true)));
        // The box holds number 1, the next paragraph in tree order gets 2.
        assert_eq!(clamp_next_seq(5, &outer.cx), 2);
        // Inside an atomic inline nothing is numbered.
        let mut atom = opts(None);
        atom.cx.clamp.atom_depth = 1;
        assert!(ClampScope::enter(&atom).is_some());
        assert_eq!(clamp_next_seq(5, &atom.cx), 1);
    }

    #[test]
    fn multicol_after_the_clamp_point_is_hidden_and_before_it_is_painted() {
        let tree = crate::dom::parse("<div style='columns:2'>a<br>b</div>", "");
        let Node::Element(e) = &tree[0] else {
            panic!("multicol")
        };
        for (para, hidden) in [(Some((0, 1)), true), (Some((2, 1)), false)] {
            let outer = opts(Some(ClampPlan {
                nodes: Vec::new(),
                sizes: Vec::new(),
                para,
                next: None,
                extent: None,
                cut: 10.,
            }));
            let scope = ClampScope::enter(&outer).expect("inside a clamp container");
            let mut d = ClampScope::close(Some(&scope), gpui::div(), e, &BoxStyle::default());
            let visibility = d.style().visibility;
            assert_eq!(visibility == Some(gpui::Visibility::Hidden), hidden);
        }
    }
}
