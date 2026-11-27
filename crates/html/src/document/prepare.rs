//! Сборка ресурсов и viewport из единственного исходного DOM.

use super::{DocState, Node, Rc, base_url, viewport};

/// Разбор в СВЕЖЕЕ состояние документа `state` (у следующей ревизии — с замерами
/// абзацев прошлой, `DocState::continuing`): шрифты и
/// `@color-profile` из листов страницы (до разбора: иначе первый же замер
/// ширины пойдёт по подстановке), разметка (XML — своим разборщиком,
/// `dom::xml`), правка адресов от действующей базы.
/// `invalidation`: a rebuild for attribute mutations collects the elements
/// they restyle (`dom::convert_restyling`); `keep` receives the rules of the
/// build for the restyles of the document.
#[allow(clippy::too_many_arguments)]
pub(super) fn parse_document(
    input: &crate::dom::input::Admitted,
    theme_css: &str,
    base: Option<&url::Url>,
    state: Rc<DocState>,
    fonts: &crate::text::metrics::FontContext,
    revision: crate::budget::RevisionBudget,
    invalidation: Option<&mut crate::dom::invalidation::Invalidation>,
    keep: &mut crate::dom::restyle_probe::RestyleCache,
    #[cfg(feature = "engine-bench")] timings: &mut super::BuildTimings,
) -> Result<(Rc<DocState>, Vec<Node>, crate::style::computed::Computed), super::DocumentFailure> {
    let media = state.media();
    let admission = crate::budget::admission::Admission::new(revision.clone());
    state.values.admit_revision(revision, base);
    let vcx = state.values();
    {
        let _span = crate::why::Span::new("document.font_context_install");
        fonts.install(&state.text, vcx);
    }
    let sheets = {
        let _span = crate::why::Span::new("document.style_sources");
        crate::dom::style_sources::collect(input, theme_css)?
    };
    {
        let _span = crate::why::Span::new("document.font_faces");
        let used =
            crate::text::fonts::used_chars::UsedChars::of_document(&input.dom.document, &sheets);
        crate::text::fonts::load_faces_sheets(&sheets, base, &state.text, vcx, &used)?;
    }
    {
        let _span = crate::why::Span::new("document.color_profiles");
        crate::style::values::color_space::load_profiles_sheets(&sheets, base, vcx)?;
    }
    #[cfg(feature = "engine-bench")]
    let style_started = crate::work::Clock::now();
    let styled = {
        crate::work::phase!(_phase, Style);
        crate::dom::convert_restyling(
            input,
            theme_css,
            media,
            &state.values,
            invalidation,
            Some(keep),
        )?
    };
    #[cfg(feature = "engine-bench")]
    {
        timings.style = style_started.elapsed();
    }
    let _span = crate::why::Span::new("document.viewport_resources");
    let (mut nodes, root) = viewport::prepare(styled, vcx, state.root_scroll.is_some());
    if let Some(base) = base {
        base_url::resolve(&mut nodes, base, false, &admission)?;
    }
    state.values.masks.collect(&nodes, vcx);
    Ok((state, nodes, root))
}
