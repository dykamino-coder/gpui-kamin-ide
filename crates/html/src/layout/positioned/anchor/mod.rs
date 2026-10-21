//! Якорное позиционирование (css-anchor-position-1), шаг 1: `anchor-name`,
//! `position-anchor` и функция `anchor()` во вставках абсолютной коробки.
//!
//! Порядок вычислений. Раскладка под нами считает рамки ВСЕХ коробок до
//! подготовки кадра, а `prepaint` идёт в порядке дерева; внепоточные коробки
//! при этом дописываются ПОСЛЕДНИМИ детьми своего слоя
//! (`Positioning::late_close`/`cb_close`/`icb_close`). Поэтому:
//! * якорь снимает свою рамку пробой-канвасом (`probe_for`, по образцу
//!   `containing_block::spot_probe`/`edge_probe`) в своём `prepaint` и кладёт её в
//!   реестр кадра;
//! * позиционированная коробка, идущая позже, читает реестр в СВОЁМ
//!   `prepaint` и сдвигается через `with_element_offset` — тем же приёмом,
//!   что `LatePlace`.
//!
//! Якорь, которого в реестре ещё нет (он ниже по дереву или позже в том же
//! слое), по спеке и не acceptable («laid out strictly before»): функция
//! падает на запасное значение.
//!
//! Раскладке `anchor()`-вставка отдаётся НУЛЁМ (`apply::len_to_gpui`):
//! коробка встаёт к краю содержащего блока, а сдвиг до якоря дорисовывает
//! `AnchorPlace`. Тот же ноль делает запасное значение тривиальным:
//! `anchor(left, 20px)` в `right` — это сдвиг на −20 точек от нулевой вставки.
//! Статически неразрешимые функции (нет имени и якоря по умолчанию; имя,
//! которого в документе нет) сводятся ДО сборки (`settle_static`): их место
//! — статическая позиция, а её выбирает сборщик дерева по `edge_set`.

mod state;
pub(crate) use probe::Entry;
pub(crate) use state::AnchorState;
mod area;
mod geometry;
mod inline;
pub use inline::{prepare_inline_abs, register_inline_anchor, register_inline_cb};
pub(crate) mod parse;
pub(crate) mod place;
mod plan;
pub(crate) mod resolve;
pub(crate) mod settle;

use gpui::{AnyElement, Bounds, Pixels};

use crate::style::computed::Align;
use crate::style::values::value::{AnchorFn, Len};
use area::Tracks;
use settle::DefaultAnchor;
mod probe;
pub use probe::key_of;
pub use probe::probe_for;
mod tf_stack;
pub(super) use tf_stack::containing_bounds;
pub use tf_stack::tf_pop;
pub use tf_stack::tf_push;
use tf_stack::{tf_map, tf_under};
mod frame;
pub use frame::next_seq;
pub use frame::reset;
use frame::{reframe_if_stale, request_rebuild};

/// Запись якоря в реестре кадра: рамка (border box), маска обрезки в точке
/// пробы и `visibility: hidden` (§position-visibility: anchor-visible), свой
/// `node_id` и ближайший содержащий блок (приемлемость якоря, §target).
#[derive(Clone, Copy, Debug)]
pub struct AnchorRec {
    pub rect: Bounds<Pixels>,
    pub clip: Bounds<Pixels>,
    pub hidden: bool,
    pub id: u64,
    pub cb: u64,
    /// Та же рамка ПОСЛЕ трансформов предков и своего (§2 «Determining the
    /// Anchor»: «includes … transforms … the axis-aligned bounding
    /// rectangle»), и номер самого внутреннего трансформа в стеке `TF`
    /// (0 — трансформов нет, `rect_tf == rect`).
    pub rect_tf: Bounds<Pixels>,
    pub tf_top: u32,
}

/// Итог размещения кандидата на подготовке кадра: сдвиг коробки, переполнил
/// ли край коробки полей inset-modified containing block (§fallback,
/// `position-visibility: no-overflow`) и размер IMCB по осям
/// (`position-try-order`).
#[derive(Clone, Copy, Debug)]
struct Placement {
    dx: f32,
    dy: f32,
    overflow: bool,
    imcb: (f32, f32),
    /// Размер клетки `position-area` по осям — она и есть содержащий блок
    /// коробки (§position-area: «makes that the box's containing block»);
    /// `(0, 0)` без области.
    cell: (f32, f32),
}

/// Одна вставка с `anchor()` и поле коробки по этой стороне: функция
/// выравнивает край ПОЛЕЙ («edge of the … inset-modified containing block»),
/// а `prepaint` видит рамку без полей.
#[derive(Clone)]
struct SidePlan {
    f: std::rc::Rc<AnchorFn>,
    margin: f32,
}

pub struct AnchorPlan {
    values: crate::document::state::ValueDoc,
    /// По часовой: top, right, bottom, left — как `Sides`.
    sides: [Option<SidePlan>; 4],
    default_anchor: Option<DefaultAnchor>,
    /// Перевёрнуты ли оси письма СОДЕРЖАЩЕГО БЛОКА (`start`/`end`/`<pct>`)
    /// и СОБСТВЕННОГО письма коробки (`self-start`/`self-end`): `(x, y)`.
    cb_flipped: (bool, bool),
    own_flipped: (bool, bool),
    /// `position-area` в физических осях `(x, y)` — только при якоре по
    /// умолчанию (§position-area: иначе «this value has no effect»).
    area: Option<(Tracks, Tracks)>,
    /// Авторские вставки (края клетки; `auto` → 0) и поля в точках — по часовой.
    inset: [Option<Len>; 4],
    margin: [f32; 4],
    align_self: Option<Align>,
    justify_self: Option<Align>,
    /// Приставка `safe` у `align-self`/`justify-self` (css-align §overflow-values).
    safe_align: bool,
    safe_justify: bool,
    cb_vertical: bool,
    /// Содержащий блок: `node_id` в реестре `CB`; 0 или `fixed` — окно.
    cb_node: u64,
    fixed: bool,
    /// Порядок сборки коробки (`Computed::anchor_seq`): якорь из реестра
    /// прошлого кадра годен, только если собран РАНЬШЕ неё (`last_named`).
    seq: u32,
    /// Ссылается ли коробка на якорь по умолчанию — `position-area`,
    /// `anchor()` без имени, `anchor-center` (§position-visibility: anchor-valid).
    refs_default: bool,
}

/// Заместитель абсолютной коробки с якорными вставками: своей коробки не
/// заводит (отдаёт `layout_id` ребёнка, как `LatePlace`), сдвиг считает на
/// подготовке кадра — к этому моменту якори раньше по дереву уже в реестре.
pub struct AnchorPlace {
    child: AnyElement,
    /// Кандидаты: [0] — базовые стили, дальше — `position-try-fallbacks`.
    plans: Vec<AnchorPlan>,
    order: u8,
    key: u64,
    visibility: u8,
    /// Values of the document the plans' `anchor()` lengths come from.
    values: crate::document::state::ValueDoc,
}

#[cfg(test)]
mod tests {
    //! Модульные проверки якорного позиционирования: `match-parent`.

    use super::settle::match_parent;
    use crate::style::computed::PositionAnchor;

    #[test]
    fn match_parent_takes_named_default_anchor_of_abspos_parent() {
        let p = Some((Some(PositionAnchor::Named("--a".into())), false, None));
        assert_eq!(
            match_parent(&p),
            (PositionAnchor::Named("--a".into()), None)
        );
    }

    #[test]
    fn match_parent_without_abspos_parent_has_no_default_anchor() {
        assert_eq!(match_parent(&None), (PositionAnchor::None, None));
        let none = Some((Some(PositionAnchor::None), false, Some(7)));
        assert_eq!(match_parent(&none), (PositionAnchor::None, None));
    }

    #[test]
    fn match_parent_inherits_implicit_anchor() {
        let auto = Some((Some(PositionAnchor::Auto), false, Some(7)));
        assert_eq!(match_parent(&auto), (PositionAnchor::Auto, Some(7)));
        // `normal` + `position-area` behaves as `auto`.
        let normal = Some((None, true, Some(9)));
        assert_eq!(match_parent(&normal), (PositionAnchor::Auto, Some(9)));
        let plain = Some((None, false, Some(9)));
        assert_eq!(match_parent(&plain), (PositionAnchor::None, None));
    }
}

#[cfg(test)]
mod state_tests;
