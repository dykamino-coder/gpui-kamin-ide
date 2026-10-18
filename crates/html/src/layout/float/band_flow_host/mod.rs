//! Поток полос: дети и охранники полос.
// owner: A

use crate::dom::{Element, Node};
use crate::hooks::IntoHooked;
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_host::band_margins;
use crate::layout::float::band_measured::has_ruby;
use crate::render::{RenderOpts, block_level_in_flow, own_context, replaced_tag};
use crate::style::computed::writing_mode::WritingMode;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::AnyElement;
mod lift;
pub(super) use lift::band_flow_rest_lift;
use lift::flow_interior_plain;
mod kids;
pub(super) use kids::band_kids;
mod build_kid;
use build_kid::build_band_kid;

/// Письмо коробки отличается от письма содержащего блока хоста —
/// ортогональный поток (css-writing-modes-4 §7.3): shrink-to-fit и место по
/// чужой оси каркас пробы не считает.
pub(super) fn band_orthogonal(c: &Computed, wm: WritingMode) -> bool {
    // Письмо наследуется: незаданное у коробки — письмо содержащего блока,
    // ортогональна только коробка, ЗАДАВШАЯ другое.
    c.vertical.is_some_and(|v| v != wm.vertical)
        || (c.vertical == Some(true) && c.vertical_rl.is_some_and(|r| r != wm.rl))
}

/// Есть ли в поддереве непустой текст.
pub(super) fn subtree_has_text(e: &Element) -> bool {
    e.children.iter().any(|n| match n {
        Node::Text(t) => !t.trim().is_empty(),
        Node::Element(c) => subtree_has_text(c),
    })
}

/// Блок обычного потока для измеряемого хоста (шаг F4): блочного уровня,
/// в потоке, своего контекста не заводит, поля разрешимы.
pub(super) fn band_flow_block(c: &Element, em: f32) -> bool {
    block_level_in_flow(c)
        && !own_context(c)
        && !replaced_tag(c)
        && !c.children.iter().any(has_ruby)
        // Заголовок таблицы вне таблицы — анонимная таблица (§17.2.1), то
        // есть коробка, флоаты не перекрывающая (`clear-applies-to-015`).
        && c.style.is_caption != Some(true)
        && c.tag != "caption"
        && flow_interior_plain(c)
        && (c.style.clear.is_none() || band_clear_supported(c))
        && matches!(
            c.style.display,
            None | Some(Display::Block) | Some(Display::ListItem)
        )
        && band_margins(&c.style, em).is_some()
}

/// Хвост измеряемого хоста с шагом F4: куски `band_piece_m`, блоки обычного
/// потока и строчные прогоны. Подряд идущее строчное содержимое (текст,
/// строчные элементы, атомы) собирается в АНОНИМНЫЙ блок (CSS 2.1 §9.2.1.1)
/// — у него своя строка и свои вырезы. Внепоточный сосед хост отменяет
/// (как у `band_piece`).
pub(super) fn band_flow_rest(rest: Vec<Node>, em: f32, wm: WritingMode) -> Option<Vec<Node>> {
    band_flow_rest_lift(rest, em, None, wm)
}

/// Сборка измеряемого хоста: каждому ребёнку — построитель, который
/// `band_flow` зовёт на каждую пробу и на `prepaint`.
pub(super) fn band_flow_host(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let host = e.marks.shape_flow.unwrap_or_default();
    let count = host.count;
    let em: f32 = host.em.unwrap_or(opts.base_size());
    let (cbh, cbw) = (host.cbh, host.cbw);
    let mut inherited = inherited.clone();
    if let Some(h) = cbh {
        inherited.height = Some(Len::Px(h));
    }
    if let Some(w) = cbw {
        inherited.width = Some(Len::Px(w));
    }
    let inherited = &inherited;
    let kids = band_kids(
        &e.children,
        count,
        inherited,
        opts,
        em,
        e.marks.adjoining_start,
        host.wm,
    );
    let flow = crate::layout::float::band_flow::BandFlow::new(kids, !e.marks.inflow_height);
    if inherited.vertical == Some(true) {
        flow.vertical(inherited.vertical_rl == Some(true)).hooked()
    } else {
        flow.hooked()
    }
}
