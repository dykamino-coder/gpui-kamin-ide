//! Клип и вырезание прямоугольников для стилизованной коробки.

use crate::dom::Node;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, Styled, div, px};

/// Fill `outer` minus `hole` (both `(left, top, right, bottom)` in window
/// points) with up to four rectangles whose edges snap to the device grid —
/// the shape of a sharp, square box shadow (css-backgrounds-3 §7.1). An empty
/// or disjoint hole leaves `outer` whole.
pub(crate) fn paint_rect_minus(
    window: &mut gpui::Window,
    (ol, ot, or, ob): (f32, f32, f32, f32),
    (hl, ht, hr, hb): (f32, f32, f32, f32),
    fill: gpui::Hsla,
) {
    let sf = window.scale_factor();
    let snap = |v: f32| (v * sf).round() / sf;
    let mut paint = |x0: f32, y0: f32, x1: f32, y1: f32| {
        let (x0, y0, x1, y1) = (snap(x0), snap(y0), snap(x1), snap(y1));
        if x1 > x0 && y1 > y0 {
            window.paint_quad(gpui::fill(
                gpui::Bounds::from_corners(
                    gpui::point(px(x0), px(y0)),
                    gpui::point(px(x1), px(y1)),
                ),
                fill,
            ));
        }
    };
    if hl >= hr || ht >= hb {
        paint(ol, ot, or, ob);
        return;
    }
    // Above and below the hole: full width.
    paint(ol, ot, or, ht.min(ob));
    paint(ol, hb.max(ot), or, ob);
    // Beside the hole, within its rows.
    let (y0, y1) = (ht.max(ot), hb.min(ob));
    paint(ol, y0, hl.min(or), y1);
    paint(hr.max(ol), y0, or, y1);
}

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
    let clip = c.color_clip()?;
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
        (Some(g), _) => layer.bg(crate::style::apply::fill(g)),
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
