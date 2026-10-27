//! Сборка вложенного дерева и явный fallback отказавшего документа.

use crate::layout::options::RenderOpts;
use gpui::{AnyElement, IntoElement, ParentElement};

pub(crate) fn rejected(
    failure: crate::dom::outcome::DocumentFailure,
    opts: &RenderOpts,
) -> AnyElement {
    if opts.cx.doc().embedded_failure.get().is_none() {
        opts.cx.doc().embedded_failure.set(Some(failure));
    }
    opts.cx.doc().record_diagnostic((&failure).into());
    gpui::div().child(failure.to_string()).into_any_element()
}

/// Сборка вложенного документа (рамка, SVG с `<foreignObject>`) в ЕГО
/// области: `prepare` правит верхний уровень, дерево собирается с его
/// областью просмотра и солью, глубина вложенности растёт на время сборки,
/// элементы уходят под `Scoped` — его фазы GPUI тоже в его области.
pub(crate) fn build_embedded(
    embedded: crate::document::Embedded,
    viewport: (f32, f32),
    opts: &RenderOpts,
    prepare: impl FnOnce(Vec<crate::dom::Node>) -> Vec<crate::dom::Node>,
) -> Vec<AnyElement> {
    opts.cx.doc().observe_child(embedded.state.clone());
    let nodes = prepare(embedded.nodes);
    let mut sub = opts.clone();
    sub.viewport = viewport;
    sub.cx.position.state = Default::default();
    sub.cx.contracts.frame_depth += 1;
    sub.cx.clamp.input = None;
    sub.cx.values.doc = crate::layout::cx::BuildDoc(embedded.state.clone());
    // Свой слой ICB на вложенный документ: его абсолюты без позиционированного
    // предка держатся ЕГО начального содержащего блока — области просмотра
    // рамки, а не внешней страницы (HTML §4.8.5 «nested browsing context»;
    // `abs-pos-non-replaced-icb-*`: коробка с `right: 80%` улетала в левый
    // верхний угол внешнего документа).
    let icb_layer = crate::layout::positioned::layer_scope::LayerScope::icb(&sub.cx.position.state);
    // Вычисленные стили вложенного документа — от его корня.
    crate::style::cascade::materialize::materialize(
        &nodes,
        sub.root_style().style_rc(),
        sub.cx.vcx(),
    );
    let mut kids = sub
        .cx
        .contracts
        .build
        .blocks(&nodes, &sub.root_style(), &sub);
    kids.extend(icb_layer.finish());
    if let Some(limit) = embedded.state.traversal_limit() {
        kids.insert(0, gpui::div().child(limit.to_string()).into_any_element());
    }
    kids.into_iter()
        .map(|k| crate::document::state::Scoped::wrap(&embedded.state, k))
        .collect()
}
