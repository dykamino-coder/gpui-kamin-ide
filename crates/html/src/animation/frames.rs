//! GPUI-обёртки переходов и применение анимации к элементу.
//! Выборка и интерполяция CSS-кадров принадлежат style::animation.

mod frozen;
pub(crate) use crate::style::animation::sample_at;
pub(crate) use frozen::bake_frozen;

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::{AnyElement, IntoElement};

/// Обернуть элемент плавным переходом, если он задан.
///
/// Поддерево пересобирается по доле перехода — иначе смешанный стиль некуда
/// применить: у собранного элемента стиль уже зафиксирован.
pub(crate) fn transitioned(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    let seconds = e.style.used().transition?;
    let hover = e.hover.clone()?;
    let id = e.state_id("html-transition", opts.cx.vcx());
    let node = e.clone();
    let inherited = inherited.clone();
    let doc = opts.cx.doc().clone();
    let opts = opts.clone();
    let build = std::rc::Rc::new(move |k: f32| {
        let mut mixed = node.clone();
        mixed.style = node.style.used().blend(hover.cascaded(), k).into();
        // Слой наведения снят: его роль уже сыграла доля перехода, иначе
        // стиль прыгнул бы поверх плавного.
        mixed.hover = None;
        mixed.style.used_mut().transition = None;
        opts.cx.contracts.build.element(&mixed, &inherited, &opts)
    });
    Some(crate::animation::transition::Transition::new(id, seconds, build, doc).into_any_element())
}
