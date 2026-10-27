//! Сборка ресурсов и viewport из единственного исходного DOM.

use super::{DocState, Media, Node, Rc, base_url, viewport};

/// Разбор в СВЕЖЕЕ состояние документа носителем `media`: шрифты и
/// `@color-profile` из листов страницы (до разбора: иначе первый же замер
/// ширины пойдёт по подстановке), разметка (XML — своим разборщиком,
/// `dom::xml`), правка адресов от действующей базы.
pub(super) fn parse_document(
    input: &crate::dom::input::Admitted,
    theme_css: &str,
    base: Option<&url::Url>,
    media: Media,
    fonts: &crate::text::metrics::FontContext,
    revision: crate::budget::RevisionBudget,
    #[cfg(feature = "engine-bench")] timings: &mut super::BuildTimings,
) -> Result<(Rc<DocState>, Vec<Node>, crate::style::computed::Computed), super::DocumentFailure> {
    let state = DocState::new(media);
    state.budget.replace(Some(revision));
    state.images.borrow_mut().set_base(base.cloned());
    let vcx = state.values();
    fonts.install(vcx);
    let sheets = crate::dom::style_sources::collect(input, theme_css)?;
    crate::text::fonts::load_faces_sheets(&sheets, base, vcx);
    crate::style::values::color_space::load_profiles_sheets(&sheets, base, vcx);
    #[cfg(feature = "engine-bench")]
    let style_started = std::time::Instant::now();
    let styled = crate::dom::convert(input, theme_css, media, &state)?;
    #[cfg(feature = "engine-bench")]
    {
        timings.style = style_started.elapsed();
    }
    let (mut nodes, root) = viewport::prepare(styled, vcx);
    if let Some(base) = base {
        base_url::resolve(&mut nodes, base, false);
    }
    state.masks.collect(&nodes, vcx);
    Ok((state, nodes, root))
}
