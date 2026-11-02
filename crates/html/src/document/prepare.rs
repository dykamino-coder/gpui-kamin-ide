//! Сборка ресурсов и viewport из единственного исходного DOM.

use super::{DocState, Node, Rc, base_url, viewport};

/// Разбор в СВЕЖЕЕ состояние документа `state` (у следующей ревизии — с замерами
/// абзацев прошлой, `DocState::continuing`): шрифты и
/// `@color-profile` из листов страницы (до разбора: иначе первый же замер
/// ширины пойдёт по подстановке), разметка (XML — своим разборщиком,
/// `dom::xml`), правка адресов от действующей базы.
pub(super) fn parse_document(
    input: &crate::dom::input::Admitted,
    theme_css: &str,
    base: Option<&url::Url>,
    state: Rc<DocState>,
    fonts: &crate::text::metrics::FontContext,
    revision: crate::budget::RevisionBudget,
    #[cfg(feature = "engine-bench")] timings: &mut super::BuildTimings,
) -> Result<(Rc<DocState>, Vec<Node>, crate::style::computed::Computed), super::DocumentFailure> {
    let media = state.media();
    let admission = crate::budget::admission::Admission::new(revision.clone());
    state.values.admit_revision(revision, base);
    let vcx = state.values();
    fonts.install(&state.text, vcx);
    let sheets = crate::dom::style_sources::collect(input, theme_css)?;
    crate::text::fonts::load_faces_sheets(&sheets, base, &state.text, vcx)?;
    crate::style::values::color_space::load_profiles_sheets(&sheets, base, vcx)?;
    #[cfg(feature = "engine-bench")]
    let style_started = std::time::Instant::now();
    let styled = {
        crate::work::phase!(_phase, Style);
        crate::dom::convert(input, theme_css, media, &state.values)?
    };
    #[cfg(feature = "engine-bench")]
    {
        timings.style = style_started.elapsed();
    }
    let (mut nodes, root) = viewport::prepare(styled, vcx);
    if let Some(base) = base {
        base_url::resolve(&mut nodes, base, false, &admission)?;
    }
    state.values.masks.collect(&nodes, vcx);
    Ok((state, nodes, root))
}
