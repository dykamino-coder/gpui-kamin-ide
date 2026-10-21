//! Сборка измеряемого хоста полос из DOM. Вход: синтетический узел с `shape_flow` (число
//! флоатов, `em`, письмо и размеры содержащего блока), собранный `band_host_m`; выход —
//! `BandFlow`, где каждому ребёнку построен построитель (`band_kids`, `build_kid`).
//! Хвост хоста режется на куски, блоки потока и анонимные строчные прогоны
//! (`band_flow_rest`, CSS 2.1 §9.2.1.1). Контекст содержащего блока едет в `ShapeFlow`
//! и в `LayoutCx`, а не в глобальных регистрах.

use crate::dom::{Element, Node};
use crate::hooks::IntoHooked;
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_host::band_margins;
use crate::layout::float::band_measured::has_ruby;
use crate::render::{RenderOpts, block_level_in_flow, own_context, replaced_tag};
use crate::style::computed::writing_mode::WritingMode;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled};
mod inline_run;
mod lift;
mod nested_lift;
pub(super) use lift::band_flow_rest_lift;
pub(in crate::layout::float) use lift::flow_interior_plain;
mod kids;
pub(super) use kids::band_kids;
mod build_kid;
mod float_box;
#[cfg(test)]
mod tests;
use build_kid::build_band_kid;

/// Письмо коробки отличается от письма содержащего блока хоста —
/// ортогональный поток (css-writing-modes-4 §7.3): shrink-to-fit и место по
/// чужой оси каркас пробы не считает.
pub(super) fn band_orthogonal(c: &Computed, wm: WritingMode) -> bool {
    // Письмо наследуется: незаданное у коробки — письмо содержащего блока,
    // ортогональна только коробка, ЗАДАВШАЯ другое.
    c.vertical_decl().is_some_and(|v| v != wm.vertical())
        || (c.wm().vertical()
            && c.rl_decl()
                .is_some_and(|r| r != (wm.vertical() && wm.block_rl())))
}

/// A float with flex, grid or multicol layout: the
/// probe frame does not shrink such a box to fit (CSS 2.1 §10.3.5), so the
/// host sizes it from its intrinsic sizes.
pub(super) fn own_layout_mode(c: &Computed) -> bool {
    crate::layout::multicol::spanner::multicol_container(c)
        || matches!(
            c.display,
            Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::GridLanes)
        )
}

/// CSS 2.1 §10.3.5: an auto-width float is shrink-to-fit. A flex, grid or multicol
/// container does not shrink in the probe frame by itself: it gets the
/// border-box inline size `avail` the host computed from its intrinsic sizes
/// (zero only while those are being measured). `width` keeps the box's own
/// `box-sizing`, and `min-width`/`max-width` still apply after it (§10.4);
/// percentage paddings resolve against `cb`, font units against the box's
/// font in `inherited`.
pub(super) fn fit_own_layout(
    c: &mut Computed,
    inherited: &Computed,
    avail: f32,
    cb: f32,
    vcx: crate::document::state::ValueCx<'_>,
) {
    if !own_layout_mode(c) || avail <= 0.0 || !matches!(c.width, None | Some(Len::Auto)) {
        return;
    }
    let edges = if c.border_box == Some(true) {
        0.0
    } else {
        let merged = crate::style::cascade::inherit::inherit(inherited, c, vcx);
        let font = merged.font_cx();
        let px = |l: Option<Len>| l.and_then(|l| l.to_px(Some(cb), font, vcx)).unwrap_or(0.0);
        let b = c.borders();
        px(b.left) + px(b.right) + px(c.padding.left) + px(c.padding.right)
    };
    c.width = Some(Len::Px((avail - edges).max(0.0)));
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
pub(super) fn band_flow_rest(
    rest: Vec<Node>,
    em: f32,
    wm: WritingMode,
    cb: Option<f32>,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<Vec<Node>> {
    band_flow_rest_lift(rest, em, None, wm, cb, vcx)
}

/// Сборка измеряемого хоста: каждому ребёнку — построитель, который
/// `band_flow` зовёт на каждую пробу и на `prepaint`.
pub(crate) fn band_flow_host(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
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
    let parent = opts.frag.watch.as_ref().map(|w| w.flow.clone());
    let watched = crate::layout::fragment::paginate::watch::Watched::for_lines();
    let probes = watched.flow.clone();
    let mut child_opts = opts.clone();
    child_opts.frag.watch = parent.as_ref().map(|_| watched);
    let kids = band_kids(
        &e.children,
        count,
        inherited,
        &child_opts,
        em,
        e.marks.adjoining_start,
        host.wm,
    );
    let flow = crate::layout::float::band_flow::BandFlow::new(
        kids,
        !e.marks.inflow_height,
        opts.cx.paint.clone(),
    );
    let flow = match parent {
        Some(parent) => flow.fragment_flow(
            parent,
            probes,
            crate::layout::fragment::types::StackAxis::of(inherited.wm()),
        ),
        None => flow,
    };
    if inherited.wm().vertical() {
        return flow.vertical(inherited.wm()).hooked();
    }
    let flow = flow.hooked();
    if !host.icb {
        return flow;
    }
    // The host is an anonymous block-level box: its auto inline size fills
    // the containing block (CSS 2.1 §10.3.3). A measured leaf receives that
    // size from its stretching parent; the embedder's box of the initial
    // containing block does not stretch, so a host at the root fills it
    // explicitly (`float-root`: the floated root sat at the left of a
    // shrunk host).
    gpui::div()
        .w_full()
        .flex()
        .flex_col()
        .child(flow)
        .into_any_element()
}
