//! Клип и вырезание прямоугольников для стилизованной коробки.

use crate::dom::Node;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, Styled, div, px};

/// То же, но со стилем, уже разрешённым по родителю.
///
/// Наследование даёт две вещи: текстовые свойства и разрешённые `em` — без
/// него отступ в `em` считался бы от базового кегля, а не от своего.
/// Слой фона, обрезанного внутренним краем коробки (`background-clip`).
///
/// Коробка в раскладке красит весь свой прямоугольник, включая рамку и поля,
/// а `padding-box`/`content-box` требуют красить меньше. Поэтому фон снимается
/// с коробки (см. `apply::apply_paint`) и рисуется отдельным слоем, вжатым
/// внутрь: на рамку, а для `content-box` ещё и на поля. Слой идёт ПЕРВЫМ
/// ребёнком — порядок детей задаёт порядок рисования, и содержимое остаётся
/// поверх фона.
///
/// `text` слоя не даёт вовсе: фон по форме глифов мы не рисуем, и закрасить
/// вместо него всю коробку — заметно хуже, чем не красить (тесты на него
/// прямо пишут «no red» про залитый прямоугольник).
pub(crate) fn clip_layer(c: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    let vcx = opts.cx.vcx();
    if crate::style::computed::single_corner::eligible(c) {
        return None;
    }
    let clip = c.color_clip(vcx)?;
    if c.gradient.is_none() && c.background.is_none() {
        return None;
    }
    // `border-area` красит рамка (`background::border_paint`): слой во весь
    // padding-box лёг бы внутрь кольца.
    if matches!(
        clip,
        crate::style::computed::BgClip::Text | crate::style::computed::BgClip::BorderArea
    ) {
        return None;
    }
    let size = own_size(c, opts);
    let family = c.font_family.clone().unwrap_or_default();
    let px_of = |l: Option<Len>| crate::text::metrics::spacing_px(l, &family, size);
    let border = c.borders();
    // Абсолютный слой в раскладке отсчитывается уже от padding-box
    // (`vendor/taffy/src/compute/block.rs`, как в CSS 2.1 §10.1): рамку
    // вычитать второй раз нельзя — проба `probe-bg-clipinset` давала 60×60
    // вместо 100×100 при рамке 20px.
    let pad = |p: Option<Len>| {
        if clip == crate::style::computed::BgClip::ContentBox {
            px_of(p)
        } else {
            0.0
        }
    };
    let mut layer = div()
        .absolute()
        .top(px(pad(c.padding.top)))
        .right(px(pad(c.padding.right)))
        .bottom(px(pad(c.padding.bottom)))
        .left(px(pad(c.padding.left)));
    layer = match (&c.gradient, c.background) {
        (Some(g), _) => layer.bg(crate::paint::apply::fill(g)),
        (None, Some(bg)) => layer.bg(gpui::Background::from(bg.to_hsla())),
        _ => return None,
    };
    // Скругление внутреннего края меньше внешнего ровно на толщину рамки
    // (css-backgrounds-3 §5.4): слой со скруглением коробки вылезал бы
    // уголками за неё.
    let inner = |r: Option<Len>, a: Option<Len>, b: Option<Len>| {
        let cut = px_of(a).max(px_of(b));
        (px_of(r) - cut).max(0.0)
    };
    layer = layer
        .rounded_tl(px(inner(c.radius.tl, border.top, border.left)))
        .rounded_tr(px(inner(c.radius.tr, border.top, border.right)))
        .rounded_br(px(inner(c.radius.br, border.bottom, border.right)))
        .rounded_bl(px(inner(c.radius.bl, border.bottom, border.left)));
    Some(layer.into_any_element())
}

/// Объявлен ли где-то в поддереве СВОЙ `visibility: visible`.
///
/// По §11.2 потомок скрытого элемента виден, если объявил видимость сам.
/// Читается собственное значение узла, до слияния: `Some(false)` приходит
/// только из авторского CSS — прочие места ставят лишь `Some(true)`.
pub(super) fn shows_inside(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(_) => false,
        Node::Element(e) => e.style.hidden == Some(false) || shows_inside(&e.children),
    })
}
