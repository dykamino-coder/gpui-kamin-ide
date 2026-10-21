//! Применение точки среза в порядке дерева коробок: содержимое ДО точки сохраняет своё
//! переполнение (css-overflow-4 §5.3 прячет только то, что ПОСЛЕ точки; Blink помечает такие
//! коробки hidden for paint вместо обрезки контейнера). Опускает клип контейнера до видимой
//! протяжённости и прячет абзацы и поточные блочные коробки за точкой при сборке стилей.

use super::{clamp_cut, clamp_extent, clamp_para};
use crate::document::state::ValueCx;
use crate::layout::cx::LayoutCx;

/// The visible extent of the content before the clamp point reaches below
/// the cut (css-overflow-4 §5.3: only boxes and lines AFTER the clamp point
/// are hidden; Blink marks them hidden for paint instead of clipping the
/// container). Returns that extent from the content top.
pub fn revealed_extent(key: u64, vcx: ValueCx<'_>) -> Option<f32> {
    let cut = clamp_cut(key, vcx).filter(|c| c.is_finite())?;
    clamp_extent(key, vcx).filter(|e| *e > cut + 0.5)
}

/// Extend the container's clip edge down to the revealed extent; `cut` is
/// the used content height and `pad_bottom` the bottom padding (the clip
/// edge is the padding box).
pub fn extend_clip(
    style: &mut gpui::StyleRefinement,
    key: u64,
    cut: f32,
    pad_bottom: f32,
    vcx: ValueCx<'_>,
) {
    if let Some(ext) = revealed_extent(key, vcx) {
        style.overflow_clip_offset = Some([0.0, 0.0, ext - cut - pad_bottom, 0.0]);
    }
}

/// A paragraph numbered after the clamp paragraph follows the clamp point:
/// hidden for paint. A fixed-height float can extend the container past the
/// counted line without extending the separately measured overflow extent
/// (CSS Overflow 4 section 5.3; Blink marks following boxes hidden for paint).
pub fn hidden_after_point(key: u64, seq: u32, vcx: ValueCx<'_>) -> bool {
    clamp_cut(key, vcx).is_some_and(f32::is_finite)
        && match clamp_para(key, vcx) {
            Some((s, _)) => seq > s,
            None => super::line_state::clamp_next(key, vcx).is_some_and(|n| seq >= n),
        }
}

/// An in-flow block box whose paragraphs all follow the clamp point is
/// hidden for paint with them (css-overflow-4 §5.3; Blink hides the boxes
/// after the clamp point instead of clipping): once the clip reveals the
/// overflow of earlier content, its border and background must not show
/// (`line-clamp-auto-047`). Floats, positioned and atomic inline boxes keep
/// the clip.
pub fn hide_box_after_point(
    mut d: gpui::Div,
    outer: Option<(u64, bool)>,
    span: Option<super::ClampSpan>,
    style: &crate::style::computed::Computed,
    is_clamp: bool,
    cx: &LayoutCx,
) -> gpui::Div {
    use crate::style::computed::{Display, Position};
    use gpui::Styled;
    super::close_clamp_span(span.clone(), cx);
    let first = span.as_ref().map(|s| s.get().0);
    let in_flow = style.float.is_none_or(|f| f == 0)
        && !matches!(
            style.position,
            Some(Position::Absolute) | Some(Position::Fixed)
        )
        && !matches!(
            style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
        );
    if let (Some((key, false)), Some(first)) = (outer, first)
        && !is_clamp
        && in_flow
        && hidden_after_point(key, first, cx.vcx())
    {
        d.style().visibility = Some(gpui::Visibility::Hidden);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::state::DocState;
    use crate::text::clamp::{clamp_context, clamp_lines_for, clamp_next_seq, open_clamp_span};

    #[test]
    fn atomic_box_has_no_independent_clamp_span() {
        let mut cx = LayoutCx::default();
        cx.clamp_build = cx.clamp_build.container(903);
        assert!(open_clamp_span(&cx).is_some());
        cx.clamp_atom_depth = 1;
        assert!(open_clamp_span(&cx).is_none());
        let mut inner = cx.clone();
        inner.clamp_atom_depth = 0;
        inner.clamp_build = inner.clamp_build.container(904);
        assert!(open_clamp_span(&inner).is_some());
        assert_eq!(clamp_next_seq(904, &inner), 0);
        assert_eq!(clamp_context(&cx), Some((903, false)));
        assert_eq!(clamp_next_seq(903, &cx), 0);
        cx.clamp_atom_depth = 0;
        assert!(open_clamp_span(&cx).is_some());
    }

    #[test]
    fn counted_point_hides_later_paragraphs_below_a_tall_float() {
        let doc = DocState::new(Default::default());
        let vcx = doc.frame_values();
        let key = 1;
        doc.clamp.cuts.borrow_mut().insert(key, 200.0);
        doc.clamp.extent.borrow_mut().insert(key, 128.0);
        doc.clamp.para.borrow_mut().insert(key, (3, 1));
        assert!(revealed_extent(key, vcx).is_none());
        assert!(!hidden_after_point(key, 3, vcx));
        assert!(hidden_after_point(key, 4, vcx));
        doc.clamp.cuts.borrow_mut().insert(key, f32::INFINITY);
        assert!(!hidden_after_point(key, 4, vcx));
    }

    #[test]
    fn documents_keep_independent_cut_results_and_probe_buffers() {
        let a = DocState::new(Default::default());
        let b = DocState::new(Default::default());
        let retained = a.frame_values().keep();
        let av = retained.vcx();
        let bv = b.frame_values();
        a.clamp.cuts.borrow_mut().insert(1, 50.0);
        a.clamp.para.borrow_mut().insert(1, (2, 1));
        b.clamp.cuts.borrow_mut().insert(1, 100.0);
        b.clamp.para.borrow_mut().insert(1, (4, 2));
        assert_eq!(clamp_cut(1, av), Some(50.0));
        assert_eq!(clamp_cut(1, bv), Some(100.0));
        assert!(hidden_after_point(1, 3, av));
        assert!(!hidden_after_point(1, 3, bv));
        assert!(!std::rc::Rc::ptr_eq(
            &clamp_lines_for(1, av),
            &clamp_lines_for(1, bv)
        ));
        assert!(std::rc::Rc::ptr_eq(
            &clamp_lines_for(1, av),
            &clamp_lines_for(1, av)
        ));
        a.clamp.cuts.borrow_mut().clear();
        assert_eq!(clamp_cut(1, bv), Some(100.0));
    }
}
