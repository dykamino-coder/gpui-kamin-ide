//! Поток полос: дети и охранники полос.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_host::band_margins;
use crate::layout::float::band_measured::has_ruby;
use crate::render::{RenderOpts, block_level_in_flow, own_context, replaced_tag};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement};
mod lift;
pub(super) use lift::band_flow_rest_lift;
use lift::flow_interior_plain;
mod kids;
pub(super) use kids::band_kids;
mod build_kid;
use build_kid::build_band_kid;

thread_local! {
    /// Письмо содержащего блока, для которого `wrap_floats` собирает хост:
    /// 0 — горизонтальное, 1 — `vertical-rl`, 2 — `vertical-lr`. Им гейты
    /// измеряемого хоста отличают ортогональный поток от своего.
    pub(crate) static BAND_WM: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

thread_local! {
    /// Высота содержащего блока хоста в точках, если задана: от неё доли
    /// высоты детей (§10.5). Хост несёт её атрибутом `cbh`.
    pub(crate) static BAND_CBH: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

thread_local! {
    /// Слой `::first-line` содержащего блока (`inherited.first_line` в
    /// `blocks()`): измеряемый хост — синтетический узел, своего слоя у него
    /// нет, и `element` отдал бы детям `None`. Хост, с которого начинается
    /// содержимое блока, несёт слой узлом (`wrap_floats`).
    pub(crate) static BAND_FL: std::cell::RefCell<Option<Computed>> = const { std::cell::RefCell::new(None) };
}

/// Вернуть прежний слой первой строки по выходе из `blocks()`.
pub(crate) struct BandFlGuard(pub(crate) Option<Computed>);

impl Drop for BandFlGuard {
    fn drop(&mut self) {
        BAND_FL.with(|f| *f.borrow_mut() = self.0.take());
    }
}

thread_local! {
    /// Ширина содержащего блока хоста в точках, если задана. В вертикальном
    /// письме это его БЛОЧНЫЙ размер: от неё доли `block-size` флоатов
    /// (`width` после перевода логических свойств, §10.5 по блочной оси).
    /// Хост несёт её атрибутом `cbw`.
    pub(crate) static BAND_CBW: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// Вернуть прежнюю ширину содержащего блока хоста по выходе из `blocks()`.
pub(crate) struct BandCbwGuard(pub(crate) Option<f32>);

impl Drop for BandCbwGuard {
    fn drop(&mut self) {
        BAND_CBW.with(|w| w.set(self.0));
    }
}

/// Вернуть прежнюю высоту содержащего блока хоста по выходе из `blocks()`.
pub(crate) struct BandCbhGuard(pub(crate) Option<f32>);

impl Drop for BandCbhGuard {
    fn drop(&mut self) {
        BAND_CBH.with(|h| h.set(self.0));
    }
}

/// Вернуть прежнее письмо хоста по выходе из `blocks()`.
pub(crate) struct BandWmGuard(pub(crate) u8);

impl Drop for BandWmGuard {
    fn drop(&mut self) {
        BAND_WM.with(|w| w.set(self.0));
    }
}

/// Письмо коробки отличается от письма содержащего блока хоста —
/// ортогональный поток (css-writing-modes-4 §7.3): shrink-to-fit и место по
/// чужой оси каркас пробы не считает.
pub(super) fn band_orthogonal(c: &Computed) -> bool {
    // Письмо наследуется: незаданное у коробки — письмо содержащего блока,
    // ортогональна только коробка, ЗАДАВШАЯ другое.
    let wm = BAND_WM.with(std::cell::Cell::get);
    c.vertical.is_some_and(|v| v != (wm != 0))
        || (c.vertical == Some(true) && c.vertical_rl.is_some_and(|r| r != (wm == 1)))
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
pub(super) fn band_flow_rest(rest: Vec<Node>, em: f32) -> Option<Vec<Node>> {
    band_flow_rest_lift(rest, em, None)
}

/// Сборка измеряемого хоста: каждому ребёнку — построитель, который
/// `band_flow` зовёт на каждую пробу и на `prepaint`.
pub(super) fn band_flow_host(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let count: usize = e.attr("count").and_then(|c| c.parse().ok()).unwrap_or(0);
    let em: f32 = e
        .attr("em")
        .and_then(|c| c.parse().ok())
        .unwrap_or(opts.base_size());
    let cbh: Option<f32> = e.attr("cbh").and_then(|c| c.parse().ok());
    let cbw: Option<f32> = e.attr("cbw").and_then(|c| c.parse().ok());
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
        e.attr("adjoining-start") == Some("1"),
    );
    let flow =
        crate::layout::float::band_flow::BandFlow::new(kids, e.attr("inflow-height") != Some("1"));
    if inherited.vertical == Some(true) {
        flow.vertical(inherited.vertical_rl == Some(true))
            .into_any_element()
    } else {
        flow.into_any_element()
    }
}
