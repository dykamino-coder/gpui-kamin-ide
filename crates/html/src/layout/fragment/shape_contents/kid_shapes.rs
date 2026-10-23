//! ADR P-19 TRANSITIONAL(F2-1/F2-2/F2-3/F2-4): рекурсивный preplan до consumed/OOF состояния детей.
//! Формы детей (KidShape): рекурсивный замер вырезов каждого ребёнка.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::float::block_like_float;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::edge_break;
use crate::layout::fragment::clone::{monolith_box, solid_box};
use crate::layout::fragment::fragment_size::measure;
use crate::layout::fragment::line_shape::{basis_sized, inline_content};
use crate::layout::fragment::oof_reach::Reach;
use crate::style::values::value::Len;

// Child shapes retain their offsets and break metadata as one record.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn kid_shapes(
    c: &Element,
    depth: u8,
    px_or: impl Fn(&Option<Len>, bool) -> Option<f32>,
    cx: ShapeCx,
    kids: &Vec<&Node>,
    flex_col: bool,
    no_descent: bool,
    vcx: ValueCx<'_>,
) -> Option<
    Vec<(
        f32,
        f32,
        f32,
        Vec<(f32, f32)>,
        Vec<f32>,
        Vec<(f32, f32)>,
        bool,
        bool,
        Reach,
    )>,
> {
    let inner: Option<Vec<KidShape>> = if depth == 0 || no_descent {
        None
    } else {
        kids.iter()
            .map(|n| match n {
                // Parallel positioned flows do not advance the normal-flow
                // cursor. Their continuous extent travels to the corresponding
                // containing block (css-position-3 §abspos-breaking), where it
                // reserves fragmentainers independently of the block's size.
                Node::Element(k) if out_of_flow(&k.style.used()) => {
                    let top = k
                        .style
                        .used()
                        .inset
                        .top
                        .as_ref()
                        .filter(|l| !matches!(l, Len::Auto))
                        .and_then(|_| px_or(&k.style.used().inset.top, false));
                    let (full, up) = measure(k, depth - 1, cx, vcx)
                        .map_or((0.0, Reach::default()), |m| (m.shape.height, m.up));
                    let reach = Reach::positioned(k, full, top, cx, up);
                    Some((
                        0.0,
                        0.0,
                        0.0,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        false,
                        false,
                        reach,
                    ))
                }
                Node::Element(k)
                    if !k.inline
                        && (k.style.used().position.is_none()
                            || k.style.used().position
                                == Some(crate::style::computed::Position::Relative))
                        && (k.style.used().float.unwrap_or(0) == 0
                            || block_like_float(&k.style.used())) =>
                {
                    // Главный размер элемента КОЛОНКИ flex — его `flex-basis`
                    // (css-flexbox-1 §9.2 шаг 3): `content` — по содержимому,
                    // а `height` при этом не действует; в точках — сама база.
                    // Контейнер `height: auto` свободного места не даёт, и
                    // гибкость базу не меняет (§9.7).
                    let based = if flex_col { basis_sized(c, k) } else { None };
                    let k = based.as_ref().unwrap_or(k);
                    measure(k, depth - 1, cx, vcx).map(|m| {
                        let crate::layout::fragment::Shape {
                            height: h,
                            margin_top: mt,
                            margin_bottom: mb,
                            cuts,
                            forced,
                            solid,
                        } = m.shape;
                        // Монолит-потомок — весь диапазон
                        // его высоты; иначе — его собственные
                        // монолиты.
                        // A flex item whose lines were measured keeps those
                        // ranges: the rest of its block size fragments, as
                        // in `flex_lines::row` items (Blink slices a block's
                        // remaining block-size after its content).
                        let lines_only = matches!(
                            c.style.used().display,
                            Some(crate::style::computed::Display::Flex)
                                | Some(crate::style::computed::Display::InlineFlex)
                        ) && inline_content(k)
                            && !monolith_box(k)
                            && !solid.is_empty();
                        let solid = if solid_box(k)
                            && (!inline_content(k) || cuts.is_empty())
                            && !lines_only
                        {
                            vec![(0.0, h)]
                        } else {
                            solid
                        };
                        let (h, reach) = Reach::child(k, h, cx, m.up, m.own);
                        (
                            h,
                            mt,
                            mb,
                            cuts,
                            forced,
                            solid,
                            // Разрыв ПЕРВОГО/ПОСЛЕДНЕГО поточного ребёнка
                            // передаётся коробке (css-break-3 §5.1
                            // break-propagation; Blink `InitialBreakBefore`)
                            // одинаково у страниц и у колонок: правило не
                            // про вид фрагментаинера. Гейт `cx.paged` здесь
                            // не ставить: `single-line-row-flex-
                            // fragmentation-016` с разрывом на внуке
                            // краснеет ровно из-за него.
                            edge_break(k, false, vcx),
                            edge_break(k, true, vcx),
                            // Own flow size and positioned reach travel separately.
                            reach,
                        )
                    })
                }
                _ => None,
            })
            .collect()
    };
    inner
}

pub(super) type KidShape = (
    f32,
    f32,
    f32,
    Vec<(f32, f32)>,
    Vec<f32>,
    Vec<(f32, f32)>,
    bool,
    bool,
    Reach,
);
