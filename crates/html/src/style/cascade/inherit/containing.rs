//! Стадия коробки, содержащий блок: признаки cb_* (многоколонность, подложка, трансформ, письмо),
//! растяжение и определённость высоты.

use super::*;
use crate::document::state::ValueCx;
use crate::style::computed::{BoxStyle, Display as D, Overflow};
mod identity;

pub(super) fn inherit_containing(
    parent: &BoxStyle,
    own: &Computed,
    c: &mut BoxStyle,
    vcx: ValueCx<'_>,
) {
    let excluded = own.first_letter_excluded || parent.first_letter_excluded;
    set!(c.first_letter_excluded, excluded);
    c.cb_ancestor = parent.cb_ancestor || establishes_cb(parent);
    c.cb_depth = parent.cb_depth + u16::from(establishes_cb(parent));
    c.sc_ancestor = crate::style::computed::stacking::nested_context(parent);
    let icb_insets = own.icb_scroll_insets.or(parent.icb_scroll_insets);
    set!(c.icb_scroll_insets, icb_insets);
    let unit_insets = own.viewport_unit_insets.or(parent.viewport_unit_insets);
    set!(c.viewport_unit_insets, unit_insets);
    c.cb_scroll_insets = if establishes_cb(parent) {
        (parent.scroll.used, parent.scroll.native)
    } else {
        parent.cb_scroll_insets
    };
    let clipped = |o| matches!(o, Some(o) if o != Overflow::Visible);
    set!(
        c.fragment_clip_ancestor,
        parent.fragment_clip_ancestor
            || clipped(parent.overflow_x)
            || clipped(parent.overflow_y)
            || parent.contain_paint == Some(true)
            || parent.clip_ref.is_some()
            || parent.clip_shape.is_some()
            || parent.clip_polygon.is_some()
            || parent.clip_inset.is_some()
            || parent.clip_xywh.is_some()
            || parent.clip_rect.is_some()
            || parent.clip_len.is_some()
            || parent.mask_image.is_some()
    );
    set!(
        c.in_multicol,
        parent.in_multicol
            || ((parent.column_count.is_some()
                || parent.column_width.is_some()
                || parent.column_height.is_some())
                && !matches!(
                    parent.display,
                    Some(D::Grid | D::InlineGrid | D::GridLanes | D::Flex | D::InlineFlex)
                ))
    );
    let backdrop = parent.backdrop_root_above || backdrop_root(parent);
    set!(c.backdrop_root_above, backdrop);
    // Ближайший содержащий блок абсолюта по `node_id` — ключ реестра рамок
    // `anchor::CB` (нужен `position-area`); корень даёт 0 = начальный
    // содержащий блок, окно.
    identity::inherit(parent, c);
    // Барьер содержащего блока для `position: fixed` — не только трансформ.
    // css-contain-1 §containment-layout п.1: обособление раскладки делает
    // элемент содержащим блоком для потомков и с `absolute`, И С `fixed`;
    // §containment-paint говорит то же про `contain: paint`. Для абсолюта
    // этот список уже полон (`establishes_cb`), а `fixed` признавал барьером
    // только `transform` и висел от окна вместо предка
    // (`contain-layout-containing-block-fixed-001`,
    // `contain-paint-containing-block-fixed-001`).
    set!(
        c.transform_ancestor,
        parent.transform_ancestor
        || parent.transform.is_some()
        || parent.preserve_3d == Some(true)
        || parent.contain_layout == Some(true)
        || parent.contain_paint == Some(true)
        || crate::style::computed::filter_contains_fixed(parent)
        // css-will-change-1 §2.1: блок для `fixed` — и от обещанного свойства
        // (`will-change-fixpos-cb-*`, `-fixedpos-cb-*`).
        || parent.will_change & crate::style::computed::wc::CB_FIXED != 0
    );
    // css-transforms-1 §6: «Fixed backgrounds on the root element are affected
    // by any transform specified for that element. For all other elements that
    // are effected by a transform (i.e. have a transform applied to them, or to
    // any of their ancestor elements), a value of fixed for the
    // background-attachment property is treated as if it had a value of
    // scroll.» `will-change: transform` promises the same (Blink/Gecko;
    // `transform-fixed-bg-008.tentative`).
    let transformed_above = parent.transformed_ancestor || transformed(parent);
    set!(c.transformed_ancestor, transformed_above);
    if c.bg_fixed == Some(true)
        && (c.transformed_ancestor || (transformed(c) && !own.root_box && !c.root_box))
    {
        set!(c.bg_fixed, Some(false));
    }
    // `will-change: z-index` — контекст наложения ровно там, где `z-index`
    // действует: у позиционированной коробки и у элемента flex/grid
    // (css-flexbox-1 §5.4, css-grid-2 §6.2) — `-stacking-context-z-index-2/3`;
    // у простого блока — нет (`-z-index-4` зелёный и обязан остаться). Это не
    // домножение длин (заметка перед `inherit`): один битовый тест, страница без
    // `will-change` в ветку не заходит.
    if own.will_change & crate::style::computed::wc::STACK_Z != 0
        && (matches!(
            own.position,
            Some(crate::style::computed::Position::Relative)
                | Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
                | Some(crate::style::computed::Position::Sticky)
        ) || matches!(
            parent.display,
            Some(D::Flex) | Some(D::InlineFlex) | Some(D::Grid) | Some(D::InlineGrid)
        ))
    {
        c.will_change |= crate::style::computed::wc::STACK;
    }
    // Inside a turned paragraph the parent is the horizontal clone
    // (`render/paragraph/vertical.rs`); its line mode is the vertical one.
    set!(c.cb_wm, parent.wm().line());
    // Относительный сдвиг строчного предка КОПИТСЯ вниз (§9.4.3: сдвиг несёт
    // с собой всё содержимое коробки). Куски вне потока его получали
    // (`shift_overlays`), а вложенные куски самой строки — нет: сдвиг
    // родителя терялся на первом же слиянии стилей.
    if c.rel_shift.is_none() {
        set!(c.rel_shift, parent.rel_shift);
    }
    if c.box_shift.is_none() {
        set!(c.box_shift, parent.box_shift);
    }
    // §10.5: доля высоты считается только от ОПРЕДЕЛЁННОЙ высоты содержащего
    // блока. Определена она у корня (его блок — начальный), при высоте
    // родителя в точках, при доле от определённого деда и у абсолютной
    // коробки — та задаёт отсчёт сама.
    // Элемент гибкого контейнера, сетки и содержимое ячейки получают
    // определённую высоту от РАСКЛАДКИ (css-flexbox-1 §9.8 растяжение,
    // css-grid-2 дорожки, §17.5.3 ячейка) — признака у нас на это нет, и
    // считать их блок неопределённым нельзя (`flexbox-definite-sizes-*`,
    // `*-subgrid-*`).
    let laid_out_parent = matches!(
        parent.display,
        Some(D::Flex)
            | Some(D::InlineFlex)
            | Some(D::Grid)
            | Some(D::InlineGrid)
            | Some(D::GridLanes)
            | Some(D::TableCell)
    );
    // Растяжение передаётся дальше: у растянутой коробки высота от полосы, и
    // для её потомков блок определён (`column-align-items-005`).
    c.stretched =
        laid_out_parent && !matches!(own.height, Some(crate::style::values::value::Len::Px(_)));
    // Высота, выведенная из `aspect-ratio` при определённой ширине, —
    // определённая (css-sizing-4 §5.1: «the resulting size is definite if
    // its input sizes are also definite»): проценты детей решаются от неё
    // (`percentage-resolution-001/002`, `flex-aspect-ratio-047/048`).
    let ratio_height = parent.aspect_ratio.is_some()
        && matches!(
            parent.height,
            None | Some(crate::style::values::value::Len::Auto)
        )
        && matches!(parent.width, Some(crate::style::values::value::Len::Px(_)));
    // Ребёнок элемента КОЛОНКИ, у которой главный размер неопределён:
    // блок неопределён, и доля высоты ведёт себя как `auto`
    // (css-flexbox-1 §9.8 п.1-2; `percentage-heights-016/020`), кроме
    // случая, когда сам элемент имеет высоту в точках или основу в точках.
    let indefinite_column_item = parent.flex_main_def == Some(false)
        && !matches!(parent.height, Some(crate::style::values::value::Len::Px(_)))
        && !matches!(parent.flex_basis, Some(crate::style::values::value::Len::Px(_)))
        // Соотношение сторон САМО даёт главный размер: строчную ось
        // элемента колонки решает контейнер, и высота выводится из неё
        // (Blink `AspectRatioProvidesBlockMainSize`;
        // `flex-aspect-ratio-032/033` — ширина у элемента даже не задана).
        && parent.aspect_ratio.is_none();
    c.cb_height_def = !indefinite_column_item
        && (parent.root_box
            || laid_out_parent
            || parent.stretched
            || ratio_height
            || match parent.height {
                Some(crate::style::values::value::Len::Px(_)) => true,
                // Доля высоты у абсолютной/фиксированной коробки решается ВСЕГДА
                // (§10.5: оговорка «not absolutely positioned»; её же держит гейт
                // `paint/apply/`), значит её высота для детей определена — как у
                // Blink, где внепоточная коробка отдаёт детям свой блочный размер.
                // Прежде `.modal {position: fixed; height: stretch}` передавал
                // ложный признак от `body`, и `height: 100%` цепочки гас
                // (`intrinsic-height-abspos-stretch-percentage-child`: 25 %).
                // `calc(100% - 16px)` is a length-percentage: definite exactly when its
                // percentage basis is (css-values-4 §10.9, CSS 2.1 §10.5).
                Some(crate::style::values::value::Len::Calc(i))
                    if crate::style::values::value::calc_get(i, vcx)
                        .pct_px()
                        .is_some_and(|(pct, _)| pct == 0.0) =>
                {
                    true
                }
                Some(
                    crate::style::values::value::Len::Pct(_)
                    | crate::style::values::value::Len::Calc(_),
                ) => {
                    parent.cb_height_def
                        || matches!(
                            parent.position,
                            Some(crate::style::computed::Position::Absolute)
                                | Some(crate::style::computed::Position::Fixed)
                        )
                }
                _ => matches!(
                    parent.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                ),
            });
}

/// Has the box a transform of its own (or a promised one)?
fn transformed(s: &Computed) -> bool {
    s.transform.is_some()
        || s.translate.is_some()
        || s.rotate_prop.is_some()
        || s.scale_prop.is_some()
        || s.will_change & crate::style::computed::wc::BOX != 0
}

#[cfg(test)]
mod tests;
