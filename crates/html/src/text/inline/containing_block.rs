//! Containing block for inline; split out to keep the owning module within 250 lines.

use super::{InlineCb, OverlayAt, Piece};
use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::IntoElement;

thread_local! {
    /// Глубина позиционированных строчных предков текущего сбора кусков.
    pub(super) static INLINE_CB_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Атом, который строится прямо сейчас, лежит внутри такого предка.
    pub(super) static ATOM_CB: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Атом ушёл абсолютом с краями от строчного содержащего блока.
    pub(super) static ABS_CB_TAKEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Строящийся атом — внутри позиционированного строчного (одноразово).
pub(crate) fn take_atom_cb() -> bool {
    ATOM_CB.with(|c| c.replace(false))
}

/// Вернуть признак `take_atom_cb` перед постройкой атома.
pub(crate) fn set_atom_cb(v: bool) {
    ATOM_CB.with(|c| c.set(v));
}

/// Отметить, что атом построен для строчного содержащего блока.
pub(crate) fn note_abs_cb() {
    ABS_CB_TAKEN.with(|c| c.set(true));
}

/// Забрать отметку `note_abs_cb`.
pub(crate) fn take_abs_cb() -> bool {
    ABS_CB_TAKEN.with(|c| c.replace(false))
}

/// Отметить куски-абсолюты с краями (`OverlayAt::edges`) содержимого
/// строчной коробки `e`, у которых содержащего блока ещё нет: ближайший
/// позиционированный предок — она.
pub(super) fn mark_inline_cb(pieces: Vec<Piece>, e: &Element) -> Vec<Piece> {
    if !pieces
        .iter()
        .any(|p| matches!(p, Piece::Overlay(_, how) if how.edges && how.cb.is_none()))
    {
        return pieces;
    }
    static CB_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let id = CB_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let pad = [
        px_of(e.style.padding.top),
        px_of(e.style.padding.right),
        px_of(e.style.padding.bottom),
        px_of(e.style.padding.left),
    ];
    let marker = |start: bool| {
        Piece::Overlay(
            gpui::Empty.into_any_element(),
            OverlayAt {
                cb_marker: Some((id, start)),
                ..Default::default()
            },
        )
    };
    let mut out = Vec::with_capacity(pieces.len() + 2);
    out.push(marker(true));
    out.extend(pieces.into_iter().map(|p| match p {
        Piece::Overlay(el, how) if how.edges && how.cb.is_none() => Piece::Overlay(
            el,
            OverlayAt {
                cb: Some(InlineCb {
                    id,
                    start: 0,
                    end: 0,
                    pad,
                    shift: (0.0, 0.0),
                }),
                ..how
            },
        ),
        other => other,
    }));
    out.push(marker(false));
    out
}

/// Наследование: в CSS вниз идут только текстовые свойства. Бокс-свойства
/// (отступы, фон) принадлежат самому элементу и вниз не передаются.
/// Устанавливает ли коробка содержащий блок для внепоточных потомков
/// (§10.1 п.4 плюс барьеры css-transforms и css-contain).
///
/// Список ШИРЕ, чем `position` не `static`: лишний барьер значит невынесенный
/// элемент, то есть прежнее поведение, а пропущенный — вынос из коробки,
/// которая обязана была его удержать.
pub(crate) fn establishes_cb(c: &Computed) -> bool {
    // Растворённый элемент коробки не даёт и содержащим блоком быть не может
    // (css-display-3 §3.2).
    if matches!(
        c.display,
        Some(crate::style::computed::Display::Contents)
            | Some(crate::style::computed::Display::None)
    ) {
        return false;
    }
    matches!(
        c.position,
        Some(crate::style::computed::Position::Relative)
            | Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
            | Some(crate::style::computed::Position::Sticky)
    ) || c.transform.is_some()
        // css-transforms-2: `preserve-3d` is a containing block for all descendants.
        || c.preserve_3d == Some(true)
        || c.filter.is_some()
        || c.contain_paint == Some(true)
        || c.contain_layout == Some(true)
        // css-will-change-1 §2.1: обещание свойства, которое дало бы блок,
        // даёт его уже сейчас (`will-change-abspos-cb-002/003`).
        || c.will_change & (crate::style::computed::wc::CB_ABS | crate::style::computed::wc::CB_FIXED) != 0
}

/// Корень подложки (filter-effects-2 §BackdropRoot) — без корня документа:
/// его «Backdrop Root Image» и есть весь кадр. Фильтр у потомков
/// наследуется (`inherit`), но они и так под корнем.
pub(crate) fn backdrop_root(c: &Computed) -> bool {
    c.opacity.is_some_and(|o| o < 1.0)
        || c.filter.is_some_and(|f| f != crate::style::computed::Filter::neutral())
        || c.filter_ref.is_some()
        || c.mask_image.is_some()
        || c.clip_ref.is_some()
        || c.clip_polygon.is_some()
        || c.clip_shape.is_some()
        || c.blend.is_some_and(|b| b != 0)
        || c.backdrop_blur.is_some()
        || c.backdrop_color.is_some()
        // Любой `backdrop-filter`, кроме `none` (Overview.bs:119; Blink
        // paint_property_tree_builder.cc:1846), и `view-transition-name`
        // (css-view-transitions-1 Overview.bs:582).
        || c.backdrop_filter_set
        || c.vt_name
        // `clip-path` ЛЮБОЙ формой (Overview.bs:118; Blink
        // paint_property_tree_builder.cc:1887 `ClipPathClip()`): `inset()`,
        // `rect()`, `xywh()` и голая коробка разбором лежат не в
        // `clip_shape`/`clip_polygon` (`backdrop-filter-backdrop-root-clip-path-2`).
        || c.clip_inset.is_some()
        || c.clip_edges.is_some()
        || c.clip_xywh.is_some()
        || c.clip_bare_box
        || c.will_change_root
}
